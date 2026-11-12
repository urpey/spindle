use super::*;
use crate::sqlite::repository::resolve_planning_operation;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use spindle_core::character_design::CreateCharacterDesignInput;
use std::collections::{BTreeMap, BTreeSet};

fn digest(value: &impl serde::Serialize) -> Result<String> {
    Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(value)?)))
}
fn references(receipts: &[PlanningOperationReceipt]) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for r in receipts {
        if r.outcome == PlanningOperationOutcome::Applied {
            out.extend(r.returned_ids.clone());
        }
    }
    out
}

/// Why the delegated gate policy cannot accept a staged proposal.
///
/// This is the single refusal vocabulary shared by the apply-time policy-actor
/// checks and `planning_status`'s human-review next action, so the predicates
/// that refuse a `policy:` actor and the reason shown to the author cannot
/// drift apart. Each cause mirrors one apply-time check, including the
/// revalidated findings and the advisory cap computed from the larger of the
/// stored and revalidated counts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PolicyRefusal {
    /// No project is attached yet, so no policy actor may decide.
    UnattachedScope,
    /// The planning projection is stale and reconciliation comes first.
    StaleProjection,
    /// An upstream root proposal is still waiting for a human decision.
    UnacceptedRoots,
    /// A root level is always a human decision.
    RootLevel,
    /// The level's gate is not `delegate_if_valid` in the live policy.
    GateNotDelegated,
    /// The requested actor is not the live policy's audited actor.
    ActorMismatch {
        requested: String,
        active: Option<String>,
    },
    /// The proposal was staged under a different policy snapshot.
    StalePolicySnapshot,
    /// The proposal carries a blocking finding, stored or revalidated.
    BlockingFinding {
        code: String,
        path: String,
        described: String,
    },
    /// The proposal carries an unresolved author question.
    UnresolvedAuthorQuestion { id: String },
    /// Advisory findings exceed the policy's explicit cap.
    AdvisoryCap { count: usize, max: usize },
}

impl PolicyRefusal {
    /// Wording for an apply-time error. These are the messages the policy
    /// actor's refusal has always produced.
    pub(crate) fn for_apply(&self) -> String {
        match self {
            Self::UnattachedScope => {
                "human decision actor required before project root attachment".to_string()
            }
            Self::StaleProjection => {
                "human decision actor required while planning projection is stale".to_string()
            }
            Self::UnacceptedRoots => {
                "human decision actor required while upstream root proposals remain unaccepted"
                    .to_string()
            }
            Self::RootLevel => "human decision actor required for root planning levels".to_string(),
            Self::GateNotDelegated => {
                "human decision actor required when delegate_if_valid is not active".to_string()
            }
            Self::ActorMismatch { requested, active } => format!(
                "policy actor '{}' does not match active session policy ({active:?})",
                requested.trim()
            ),
            Self::StalePolicySnapshot => {
                "delegated policy cannot accept proposal with stale policy_snapshot".to_string()
            }
            Self::BlockingFinding { described, .. } => format!(
                "cannot accept planning proposal with blocking validation findings: {described}"
            ),
            Self::UnresolvedAuthorQuestion { .. } => {
                "delegated policy cannot accept proposal with unresolved author_questions"
                    .to_string()
            }
            Self::AdvisoryCap { .. } => {
                "delegated policy cannot accept proposal exceeding max_advisory_findings"
                    .to_string()
            }
        }
    }

    /// Wording for `planning_status`'s human-review next action.
    pub(crate) fn for_status(&self) -> String {
        match self {
            Self::UnattachedScope => "the project root is not attached yet".to_string(),
            Self::StaleProjection => "the planning projection is stale".to_string(),
            Self::UnacceptedRoots => {
                "upstream root proposals are still waiting for a human decision".to_string()
            }
            Self::RootLevel => "a root level always needs a human decision".to_string(),
            Self::GateNotDelegated => "the level's delegated gate is no longer active".to_string(),
            Self::ActorMismatch { active, .. } => {
                format!("the session policy actor is {active:?}")
            }
            Self::StalePolicySnapshot => {
                "the gate policy changed after this proposal was staged".to_string()
            }
            Self::BlockingFinding { code, path, .. } => {
                format!("blocking finding '{code}' at {path}")
            }
            Self::UnresolvedAuthorQuestion { id } => {
                format!("unresolved author question '{id}'")
            }
            Self::AdvisoryCap { count, max } => {
                format!("{count} advisory findings exceed the policy cap of {max}")
            }
        }
    }
}

impl SqliteSpindleService {
    /// The findings acceptance would evaluate right now: a staged proposal is
    /// revalidated, a decided one keeps its stored findings. Apply uses this
    /// same rule before its policy-actor checks.
    pub(crate) async fn planning_current_findings(
        &self,
        session: &PlanningSession,
        proposal: &PlanningProposal,
    ) -> Result<Vec<PlanningFinding>> {
        if proposal.status == PlanningProposalStatus::Staged {
            self.repository
                .revalidate_staged_planning_proposal(&session.id, &proposal.id)
                .await
        } else {
            Ok(proposal.findings.clone())
        }
    }

    /// Every reason the live delegated policy refuses `actor` for `proposal`.
    ///
    /// An empty list means the policy actor may accept the proposal. The apply
    /// path turns a non-empty list into its refusal error; the status path
    /// renders the same causes as the human-review reason. A cause is included
    /// only when its apply-time check exists, so the status text never reports
    /// a condition acceptance does not actually enforce.
    pub(crate) async fn policy_actor_refusals(
        &self,
        session: &PlanningSession,
        proposals: &[PlanningProposal],
        proposal: &PlanningProposal,
        actor: &str,
    ) -> Result<Vec<PolicyRefusal>> {
        let current_findings = self.planning_current_findings(session, proposal).await?;
        let mut refusals = Vec::new();
        if session.scope.is_none() {
            refusals.push(PolicyRefusal::UnattachedScope);
        }
        if session.projection_stale {
            refusals.push(PolicyRefusal::StaleProjection);
        }
        if proposals
            .iter()
            .any(|p| p.status == PlanningProposalStatus::Staged && p.artifact.level.is_root())
        {
            refusals.push(PolicyRefusal::UnacceptedRoots);
        }
        if proposal.artifact.level.is_root() {
            refusals.push(PolicyRefusal::RootLevel);
        }
        let effective_gate = session
            .policy
            .gates
            .get(&proposal.artifact.level)
            .copied()
            .unwrap_or_default();
        if effective_gate != PlanningGate::DelegateIfValid {
            refusals.push(PolicyRefusal::GateNotDelegated);
        }
        let active_actor = session.policy.delegated_actor();
        if active_actor.as_deref() != Some(actor.trim()) {
            refusals.push(PolicyRefusal::ActorMismatch {
                requested: actor.trim().to_string(),
                active: active_actor,
            });
        }
        if proposal.policy_snapshot != session.policy {
            refusals.push(PolicyRefusal::StalePolicySnapshot);
        }
        let blocking: Vec<&PlanningFinding> = proposal
            .findings
            .iter()
            .chain(current_findings.iter())
            .filter(|finding| finding.severity == PlanningFindingSeverity::Blocking)
            .collect();
        if let Some(finding) = blocking.first() {
            let described = blocking
                .iter()
                .map(|finding| crate::sqlite::repository::describe_blocking_finding(finding))
                .fold(Vec::new(), |mut described: Vec<String>, text| {
                    if !described.contains(&text) {
                        described.push(text);
                    }
                    described
                })
                .join("; ");
            refusals.push(PolicyRefusal::BlockingFinding {
                code: finding.code.clone(),
                path: finding.path.clone(),
                described,
            });
        }
        if let Some(question) = proposal.artifact.author_questions.first() {
            refusals.push(PolicyRefusal::UnresolvedAuthorQuestion {
                id: question.id.clone(),
            });
        }
        let stored_advisories = proposal
            .findings
            .iter()
            .filter(|finding| finding.severity == PlanningFindingSeverity::Advisory)
            .count();
        let current_advisories = current_findings
            .iter()
            .filter(|finding| finding.severity == PlanningFindingSeverity::Advisory)
            .count();
        let advisory_count = stored_advisories.max(current_advisories);
        if let Some(max) = session.policy.max_advisory_findings
            && advisory_count > max
        {
            refusals.push(PolicyRefusal::AdvisoryCap {
                count: advisory_count,
                max,
            });
        }
        Ok(refusals)
    }
}
impl SqliteSpindleService {
    pub async fn planning_apply_status(
        &self,
        id: &str,
        scope: Option<PlanningScope>,
    ) -> Result<PlanningApplyReport> {
        let proposals = self.list_planning_proposals(id, scope.clone()).await?;
        let receipts = self.repository.planning_receipts(id).await?;
        let prior = self.repository.latest_planning_attempt(id).await?;
        let mut operations = Vec::new();
        for p in &proposals {
            for index in 0..p.artifact.operations.len() {
                operations.push(
                    receipts
                        .iter()
                        .find(|r| r.proposal_id == p.id && r.operation_index == index)
                        .cloned()
                        .unwrap_or(PlanningOperationReceipt {
                            proposal_id: p.id.clone(),
                            operation_index: index,
                            input_hash: String::new(),
                            returned_ids: BTreeMap::new(),
                            actor: String::new(),
                            decision_note: String::new(),
                            result: None,
                            prior_state: Value::Null,
                            outcome: PlanningOperationOutcome::NotReached,
                            error: None,
                            recorded_at: 0,
                        }),
                );
            }
        }
        let complete = !proposals.is_empty()
            && proposals
                .iter()
                .all(|p| p.status == PlanningProposalStatus::Applied);
        Ok(PlanningApplyReport {
            attempt_id: prior
                .as_ref()
                .map(|r| r.attempt_id.clone())
                .unwrap_or_default(),
            complete,
            proposals,
            operations,
            error: prior.and_then(|r| r.error),
            pending_search_refreshes: self.repository.planning_embedding_count(scope).await?,
        })
    }

    async fn selected_planning(
        &self,
        input: &DecidePlanningInput,
    ) -> Result<Vec<PlanningProposal>> {
        anyhow::ensure!(
            input.decision == PlanningDecision::Accept,
            "apply requires accept decision"
        );
        anyhow::ensure!(!input.expected_hashes.is_empty(), "empty planning decision");
        anyhow::ensure!(
            !input.actor.trim().is_empty() && input.actor.len() <= 256,
            "human or delegated policy decision actor required"
        );
        anyhow::ensure!(
            !input.note.trim().is_empty() && input.note.len() <= 8000,
            "decision note required (max 8000 bytes)"
        );
        let s = self.get_planning_session(&input.session_id).await?;
        anyhow::ensure!(s.scope == input.scope, "planning scope mismatch");
        // A session may explicitly activate the McKee profile, and the
        // planning CreateProject apply path is the only writer of the marker,
        // so an unattached session can only ever attach to a McKee project;
        // this guard makes that invariant explicit instead of trusting it.
        if s.policy.has_mckee_policy_id()
            && let Some(scope) = &s.scope
        {
            anyhow::ensure!(
                self.repository.is_mckee_project(&scope.project_id).await?,
                "planning policy '{MCKEE_POLICY_ID}' requires a McKee-structured project; the attached project does not carry the McKee marker"
            );
        }
        let all = self.list_planning_proposals(&s.id, s.scope.clone()).await?;
        let is_policy_actor = input.actor.trim_start().starts_with("policy:");
        let mut pending = Vec::new();
        for (id, expected) in &input.expected_hashes {
            let p = all
                .iter()
                .find(|p| &p.id == id)
                .context("proposal not in planning session")?;
            anyhow::ensure!(
                &p.payload_hash == expected && digest(&p.artifact)? == *expected,
                "stale planning proposal hash"
            );
            anyhow::ensure!(
                matches!(
                    p.status,
                    PlanningProposalStatus::Staged | PlanningProposalStatus::Applied
                ),
                "planning proposal is no longer active"
            );
            anyhow::ensure!(
                p.artifact.intent_hash == s.intent_hash,
                "stale planning intent"
            );
            let current_findings = self.planning_current_findings(&s, p).await?;
            let blocking: Vec<&PlanningFinding> = p
                .findings
                .iter()
                .chain(current_findings.iter())
                .filter(|f| f.severity == PlanningFindingSeverity::Blocking)
                .collect();
            // The message carries the finding path and related refs so the
            // author can locate the row or operation to repair, not just the
            // code that fired. Stored findings and their fresh revalidation
            // usually repeat the same finding, so render each code/path once.
            let mut described = Vec::new();
            for finding in &blocking {
                let text = crate::sqlite::repository::describe_blocking_finding(finding);
                if !described.contains(&text) {
                    described.push(text);
                }
            }
            anyhow::ensure!(
                blocking.is_empty(),
                "cannot accept planning proposal with blocking validation findings: {}",
                described.join("; ")
            );
            let effective_gate = s
                .policy
                .gates
                .get(&p.artifact.level)
                .copied()
                .unwrap_or_default();
            if is_policy_actor {
                // One shared refusal vocabulary, so the status next-action
                // explains exactly the checks enforced here.
                let refusals = self
                    .policy_actor_refusals(&s, &all, p, input.actor.as_str())
                    .await?;
                anyhow::ensure!(
                    refusals.is_empty(),
                    "{}",
                    refusals
                        .iter()
                        .map(PolicyRefusal::for_apply)
                        .collect::<Vec<_>>()
                        .join("; ")
                );
            } else {
                anyhow::ensure!(
                    !p.artifact.level.is_root() || effective_gate != PlanningGate::DelegateIfValid,
                    "root planning levels cannot use delegate_if_valid"
                );
            }
            if p.artifact.level == PlanningLevel::Contract {
                anyhow::ensure!(
                    p.artifact.operations.len() == 1,
                    "contract acceptance requires one root operation"
                );
            }
            for (parent_id, parent_hash) in &p.artifact.parents {
                let parent = all
                    .iter()
                    .find(|p| &p.id == parent_id)
                    .context("missing planning parent")?;
                anyhow::ensure!(
                    &parent.payload_hash == parent_hash
                        && matches!(
                            parent.status,
                            PlanningProposalStatus::Staged | PlanningProposalStatus::Applied
                        ),
                    "stale planning parent"
                );
                anyhow::ensure!(
                    parent.status == PlanningProposalStatus::Applied
                        || input.expected_hashes.get(parent_id) == Some(parent_hash),
                    "dependency closure includes an unapproved parent; select it explicitly"
                );
            }
            pending.push(p.clone());
        }
        let mut ordered = Vec::new();
        let mut done: BTreeSet<String> = all
            .iter()
            .filter(|p| p.status == PlanningProposalStatus::Applied)
            .map(|p| p.id.clone())
            .collect();
        while !pending.is_empty() {
            let index = pending
                .iter()
                .position(|p| p.artifact.parents.keys().all(|id| done.contains(id)))
                .context("planning dependency cycle")?;
            let p = pending.remove(index);
            done.insert(p.id.clone());
            ordered.push(p);
        }
        Ok(ordered)
    }

    pub async fn apply_planning(&self, input: DecidePlanningInput) -> Result<PlanningApplyReport> {
        let pool = self.repository.pool().clone();
        // Execute the same service paths in a rollback-only transaction. This
        // validates the full selected closure without leaving domain writes behind.
        let ordered = pool
            .application_transaction(false, || async {
                let ordered = self.selected_planning(&input).await?;
                for p in &ordered {
                    for index in 0..p.artifact.operations.len() {
                        self.apply_planning_operation(p, index, &input.actor, &input.note)
                            .await?;
                    }
                    self.repository
                        .finish_planning_proposal(
                            p.id.clone(),
                            input.actor.clone(),
                            input.note.clone(),
                        )
                        .await?;
                }
                Ok(ordered)
            })
            .await?;
        let attempt_id = format!("planning_apply_attempt:{}", ulid::Ulid::new());
        let selected: BTreeSet<String> = ordered.iter().map(|p| p.id.clone()).collect();
        let mut report = self
            .planning_report_for(&input.session_id, &selected, &attempt_id)
            .await?;
        self.repository
            .store_planning_attempt(
                input.session_id.clone(),
                input.actor.clone(),
                report.clone(),
            )
            .await?;
        let mut expected_scope = input.scope.clone();
        for p in &ordered {
            for index in 0..p.artifact.operations.len() {
                let result = pool
                    .application_transaction(true, || async {
                        let s = self.get_planning_session(&input.session_id).await?;
                        anyhow::ensure!(
                            s.scope == expected_scope,
                            "planning session scope changed during apply"
                        );
                        let current = self.list_planning_proposals(&s.id, s.scope.clone()).await?;
                        let current = current
                            .iter()
                            .find(|item| item.id == p.id)
                            .context("proposal disappeared during apply")?;
                        anyhow::ensure!(
                            current.payload_hash == p.payload_hash
                                && matches!(
                                    current.status,
                                    PlanningProposalStatus::Staged
                                        | PlanningProposalStatus::Applied
                                ),
                            "proposal changed during apply"
                        );
                        self.apply_planning_operation(current, index, &input.actor, &input.note)
                            .await
                    })
                    .await;
                if let Err(error) = result {
                    let current_session = self.get_planning_session(&input.session_id).await?;
                    let known =
                        references(&self.repository.planning_receipts(&input.session_id).await?);
                    let failed_input_hash = match resolve_planning_operation(
                        &p.artifact.operations[index],
                        &current_session.scope,
                        &known,
                    ) {
                        Ok(d) => digest(&d)?,
                        Err(_) => String::new(),
                    };
                    let receipt = PlanningOperationReceipt {
                        proposal_id: p.id.clone(),
                        operation_index: index,
                        input_hash: failed_input_hash,
                        returned_ids: BTreeMap::new(),
                        actor: input.actor.clone(),
                        decision_note: input.note.clone(),
                        result: None,
                        prior_state: Value::Null,
                        outcome: PlanningOperationOutcome::Failed,
                        error: Some(error.to_string()),
                        recorded_at: chrono::Utc::now().timestamp_micros(),
                    };
                    report = pool
                        .application_transaction(true, || async {
                            self.repository
                                .store_planning_receipt(receipt, None, BTreeMap::new())
                                .await?;
                            let mut report = self
                                .planning_report_for(&input.session_id, &selected, &attempt_id)
                                .await?;
                            report.error = Some(error.to_string());
                            self.repository
                                .store_planning_attempt(
                                    input.session_id.clone(),
                                    input.actor.clone(),
                                    report.clone(),
                                )
                                .await?;
                            Ok(report)
                        })
                        .await?;
                    return Ok(report);
                }
                expected_scope = self.get_planning_session(&input.session_id).await?.scope;
            }
            self.repository
                .finish_planning_proposal(p.id.clone(), input.actor.clone(), input.note.clone())
                .await?;
            report = self
                .planning_report_for(&input.session_id, &selected, &attempt_id)
                .await?;
            self.repository
                .store_planning_attempt(
                    input.session_id.clone(),
                    input.actor.clone(),
                    report.clone(),
                )
                .await?;
        }
        if let Some(scope) = &expected_scope {
            report.pending_search_refreshes = self
                .repository
                .flush_planning_embeddings(&scope.project_id)
                .await?;
        }
        report.error = None;
        self.repository
            .store_planning_attempt(input.session_id, input.actor, report.clone())
            .await?;
        Ok(report)
    }

    async fn planning_report_for(
        &self,
        id: &str,
        selected: &BTreeSet<String>,
        attempt: &str,
    ) -> Result<PlanningApplyReport> {
        let s = self.get_planning_session(id).await?;
        let mut report = self.planning_apply_status(id, s.scope).await?;
        report.proposals.retain(|p| selected.contains(&p.id));
        report
            .operations
            .retain(|r| selected.contains(&r.proposal_id));
        report.attempt_id = attempt.to_string();
        report.complete = report
            .proposals
            .iter()
            .all(|p| p.status == PlanningProposalStatus::Applied)
            && report
                .operations
                .iter()
                .all(|r| r.outcome == PlanningOperationOutcome::Applied);
        report.error = None;
        Ok(report)
    }

    async fn apply_planning_operation(
        &self,
        p: &PlanningProposal,
        index: usize,
        actor: &str,
        note: &str,
    ) -> Result<PlanningOperationReceipt> {
        let s = self.get_planning_session(&p.artifact.session_id).await?;
        let proposals = self.list_planning_proposals(&s.id, s.scope.clone()).await?;
        let fresh = proposals
            .iter()
            .find(|candidate| candidate.id == p.id)
            .context("missing current planning proposal")?;
        anyhow::ensure!(
            fresh.payload_hash == p.payload_hash,
            "planning proposal changed during operation"
        );
        let p = fresh;
        let receipts = self.repository.planning_receipts(&s.id).await?;
        let refs = references(&receipts);
        let item = &p.artifact.operations[index];
        let descriptor = resolve_planning_operation(item, &s.scope, &refs)?;
        let input_hash = digest(&descriptor)?;
        if let Some(prior) = receipts.iter().find(|r| {
            r.proposal_id == p.id
                && r.operation_index == index
                && r.outcome == PlanningOperationOutcome::Applied
        }) {
            anyhow::ensure!(
                prior.input_hash == input_hash,
                "planning receipt input hash mismatch"
            );
            return Ok(prior.clone());
        }
        anyhow::ensure!(
            p.status == PlanningProposalStatus::Staged,
            "applied proposal has a missing receipt"
        );
        self.repository
            .planning_scope_guard(
                s.scope.clone(),
                descriptor.1.clone(),
                p.artifact.level,
                matches!(
                    &item.operation,
                    PlanningOperation::UpdateReaderContract(_)
                        | PlanningOperation::CreateBook(_)
                        | PlanningOperation::CreateStorySpine(_)
                ),
            )
            .await?;
        let state = self
            .repository
            .planning_operation_state(s.scope.clone(), item.clone(), index, refs)
            .await?;
        for (key, value) in &state {
            let expected = p.base_state.get(key);
            let current = digest(value)?;
            anyhow::ensure!(
                expected == Some(&current)
                    || (expected.is_none() && value.as_array().is_some_and(Vec::is_empty)),
                "stale planning base state at {key}; stage a fresh synchronization proposal"
            );
        }
        let before_followups = self
            .repository
            .planning_followup_states(p.id.clone())
            .await?;
        let (output, ids, root) = self
            .dispatch_planning_operation(item, &s.scope, descriptor.1, &p.artifact.schedule)
            .await?;
        let receipt = PlanningOperationReceipt {
            proposal_id: p.id.clone(),
            operation_index: index,
            input_hash,
            returned_ids: ids,
            actor: actor.into(),
            decision_note: note.into(),
            result: Some(output),
            prior_state: serde_json::to_value(state)?,
            outcome: PlanningOperationOutcome::Applied,
            error: None,
            recorded_at: chrono::Utc::now().timestamp_micros(),
        };
        self.repository
            .store_planning_receipt(receipt.clone(), root, before_followups)
            .await?;
        Ok(receipt)
    }

    async fn dispatch_planning_operation(
        &self,
        item: &PlanningOperationItem,
        scope: &Option<PlanningScope>,
        input: Value,
        schedule: &PlanningSchedule,
    ) -> Result<(
        PlanningAppliedOutput,
        BTreeMap<String, String>,
        Option<CreateProjectOutput>,
    )> {
        use PlanningOperation::*;
        let mut ids = BTreeMap::new();
        let mut root = None;
        macro_rules! invoke {
            ($method:ident,$input:ty,$variant:ident,$field:ident) => {{
                let out = self
                    .$method(serde_json::from_value::<$input>(input)?)
                    .await?;
                ids.insert(item.output_ref.clone(), out.$field.clone());
                PlanningAppliedOutput::$variant(Box::new(out))
            }};
        }
        let output = match &item.operation {
            CreateProject(_) => {
                anyhow::ensure!(scope.is_none(), "cannot replace an accepted planning root");
                let value: CreateProjectInput = serde_json::from_value(input)?;
                anyhow::ensure!(
                    !value.name.trim().is_empty()
                        && !value.reader_contract.promise.trim().is_empty(),
                    "project name and reader promise must be nonempty"
                );
                let out = self.create_mckee_project(value).await?;
                ids.insert(item.output_ref.clone(), out.project_id.clone());
                ids.insert(format!("{}:book", item.output_ref), out.book_id.clone());
                ids.insert(
                    format!("{}:chapter", item.output_ref),
                    out.chapter_id.clone(),
                );
                ids.insert(format!("{}:branch", item.output_ref), out.branch_id.clone());
                root = Some(out.clone());
                PlanningAppliedOutput::CreateProject(Box::new(out))
            }
            CreateCharacter(_) => invoke!(
                create_character,
                CreateCharacterInput,
                CreateCharacter,
                character_id
            ),
            CreateCharacterDesign(_) => invoke!(
                create_character_design,
                CreateCharacterDesignInput,
                CreateCharacterDesign,
                character_design_id
            ),
            CreateLocation(_) => invoke!(
                create_location,
                CreateLocationInput,
                CreateLocation,
                location_id
            ),
            CreateWorldRule(_) => invoke!(
                create_world_rule,
                CreateWorldRuleInput,
                CreateWorldRule,
                world_rule_id
            ),
            CreateRelationship(_) => invoke!(
                create_relationship,
                CreateRelationshipInput,
                CreateRelationship,
                relationship_id
            ),
            CreatePlotLine(_) => invoke!(
                create_plot_line,
                CreatePlotLineInput,
                CreatePlotLine,
                plot_line_id
            ),
            CreateConflict(_) => invoke!(
                create_conflict,
                CreateConflictInput,
                CreateConflict,
                conflict_id
            ),
            CreateTheme(_) => invoke!(create_theme, CreateThemeInput, CreateTheme, theme_id),
            CreateMotif(_) => invoke!(create_motif, CreateMotifInput, CreateMotif, motif_id),
            CreateNarrativePromise(_) => invoke!(
                create_narrative_promise,
                CreateNarrativePromiseInput,
                CreateNarrativePromise,
                narrative_promise_id
            ),
            CreateKeyFact(_) => {
                // The planning input is not a twin of a public write input: its
                // subject/scene slots are planning references, resolved by the
                // generic dispatcher before this point. Read the resolved shape
                // and wrap the existing planned-and-pending canonical-fact path
                // (`scene_id: None`), which is the only Key Fact write.
                #[derive(serde::Deserialize)]
                #[serde(deny_unknown_fields)]
                struct ResolvedKeyFact {
                    project_id: String,
                    kind: spindle_core::key_fact::KeyFactKind,
                    #[serde(default)]
                    subject_table: Option<String>,
                    #[serde(default)]
                    subject_id: Option<String>,
                    key: String,
                    value: String,
                    #[serde(default)]
                    context: Option<String>,
                    placement: PlanningStoryPlacement,
                    #[serde(default)]
                    planned_scene_id: Option<String>,
                }
                let resolved: ResolvedKeyFact = serde_json::from_value(input)?;
                let placement = resolved.placement;
                // The explicit planned-Scene reference is validated at staging;
                // re-check it at apply so acceptance cannot bind a fact to a
                // chapter plan that no longer covers its placement.
                if let Some(reference) = resolved.planned_scene_id.as_deref() {
                    let plan = self
                        .repository
                        .get_chapter_plan(reference)
                        .await
                        .with_context(|| {
                            format!("resolved planned_scene_id '{reference}' is not a chapter plan")
                        })?;
                    let order = placement.scene_order.unwrap_or_default();
                    anyhow::ensure!(
                        plan.book_number == placement.book_number
                            && plan.chapter_number == placement.chapter_number
                            && plan.scenes.iter().any(|scene| scene.scene_order == order),
                        "planned_scene_id '{reference}' no longer agrees with the Key Fact placement (book {} chapter {} scene {order})",
                        placement.book_number,
                        placement.chapter_number,
                    );
                }
                // A revealed_secret Key Fact is a secret canonical fact: the
                // subject character is in the initial circle of trust and the
                // concealment note explains the planned reveal. The circle
                // grows later through record_knowledge/secret_of_fact_id at the
                // dramatised reveal, never here.
                let secrecy = if resolved.kind.is_secret_reveal() {
                    anyhow::ensure!(
                        resolved.subject_table.as_deref() == Some("character"),
                        "a revealed_secret Key Fact requires a character subject_table"
                    );
                    let holder_id = resolved
                        .subject_id
                        .clone()
                        .filter(|id| id.starts_with("character:"))
                        .context(
                            "a revealed_secret Key Fact requires a resolved character subject_id",
                        )?;
                    Some(spindle_core::models::SecrecyScope {
                        holder_ids: vec![holder_id],
                        concealment_note: Some(format!(
                            "Planned reveal at book {} chapter {} scene {}; the subject knows the secret from the start. Record each further holder at the dramatised reveal with record_knowledge/secret_of_fact_id.",
                            placement.book_number,
                            placement.chapter_number,
                            placement.scene_order.unwrap_or_default(),
                        )),
                    })
                } else {
                    None
                };
                let out = self
                    .register_canonical_fact(RegisterCanonicalFactInput {
                        project_id: resolved.project_id,
                        scene_id: None,
                        book_number: placement.book_number,
                        chapter_number: placement.chapter_number,
                        fact_type: Some(resolved.kind.as_str().to_string()),
                        key: Some(resolved.key.clone()),
                        value: Some(resolved.value.clone()),
                        context: resolved.context,
                        subject_table: resolved.subject_table,
                        subject_id: resolved.subject_id,
                        // The reserved `key_fact.<kind>:` predicate namespace is
                        // the durable Key Fact marker (see
                        // `spindle_core::key_fact`). The author key is preserved
                        // verbatim after it; ordinary facts never carry it.
                        predicate: Some(spindle_core::key_fact::key_fact_predicate(
                            resolved.kind,
                            &resolved.key,
                        )),
                        value_kind: Some("string".to_string()),
                        value_text: Some(resolved.value),
                        value_number: None,
                        value_unit: None,
                        value_json: None,
                        aliases: Vec::new(),
                        scope: None,
                        valid_from: Some(StoryPlacement {
                            book_number: placement.book_number,
                            chapter_number: placement.chapter_number,
                            scene_order: placement.scene_order,
                            note: placement.note,
                        }),
                        valid_until: None,
                        legacy_untyped: None,
                        supersedes_fact_id: None,
                        secrecy,
                    })
                    .await?;
                ids.insert(item.output_ref.clone(), out.canonical_fact_id.clone());
                PlanningAppliedOutput::CreateKeyFact(Box::new(out))
            }
            CreateCharacterArc(_) => {
                let out = self
                    .create_character_arc(serde_json::from_value(input)?)
                    .await?;
                ids.insert(item.output_ref.clone(), out.character_arc_id.clone());
                ids.insert(
                    format!("{}:pacing_tracker", item.output_ref),
                    out.pacing_tracker_id.clone(),
                );
                PlanningAppliedOutput::CreateCharacterArc(Box::new(out))
            }
            CreatePacingConfig(_) => invoke!(
                create_pacing_config,
                CreatePacingConfigInput,
                CreatePacingConfig,
                pacing_config_id
            ),
            CreatePacingCurve(_) => {
                let input: CreatePacingCurveInput = serde_json::from_value(input)?;
                // The proposal is not `applied` yet, so the accepted-Act read
                // would miss this artifact's own Act list. Pass the list this
                // batch would leave behind, including the artifact schedule.
                let acts = match scope {
                    Some(scope) => {
                        let accepted = self
                            .repository
                            .accepted_planning_acts(&scope.project_id, &scope.branch_id)
                            .await?;
                        spindle_core::acts::merge_planned_acts(&accepted, &schedule.acts)
                    }
                    None => schedule.acts.clone(),
                };
                let out = self.create_pacing_curve_with_acts(input, &acts).await?;
                ids.insert(item.output_ref.clone(), out.pacing_curve_id.clone());
                PlanningAppliedOutput::CreatePacingCurve(Box::new(out))
            }
            SetArcPacingConstraints(_) => invoke!(
                set_arc_pacing_constraints,
                SetArcPacingConstraintsInput,
                SetArcPacingConstraints,
                pacing_tracker_id
            ),
            CreateBook(_) => invoke!(create_book, CreateBookInput, CreateBook, book_id),
            CreateChapter(_) => invoke!(
                create_chapter,
                CreateChapterInput,
                CreateChapter,
                chapter_id
            ),
            PlanChapter(_) => {
                let plan_input: PlanChapterInput = serde_json::from_value(input)?;
                let book_number = plan_input.book_number;
                let chapter_number = plan_input.chapter_number;
                let prior_rev = if let Some(scope) = scope {
                    let project_id = scope.project_id.clone();
                    let branch_id = scope.branch_id.clone();
                    self.repository
                        .pool()
                        .read(move |conn| {
                            let drafted: bool = conn.query_row(
                                "WITH RECURSIVE lineage(id,parent_branch_id) AS (SELECT id,parent_branch_id FROM bible_branch WHERE id=?1 AND project_id=?2 UNION SELECT b.id,b.parent_branch_id FROM bible_branch b JOIN lineage l ON b.id=l.parent_branch_id WHERE b.project_id=?2) SELECT EXISTS(SELECT 1 FROM scene WHERE project_id=?2 AND branch_id IN (SELECT id FROM lineage) AND book_number=?3 AND chapter_number=?4)",
                                rusqlite::params![branch_id, project_id, book_number, chapter_number],
                                |r| r.get(0),
                            )?;
                            if drafted {
                                return Err(rusqlite::Error::ToSqlConversionFailure(
                                    anyhow::anyhow!(
                                        "planning cannot overwrite a chapter with persisted prose"
                                    )
                                    .into(),
                                ));
                            }
                            let active_run_exists: bool = conn.query_row(
                                "SELECT EXISTS(SELECT 1 FROM authoring_run WHERE project_id=?1 AND active_branch_id=?2 AND book_number=?3 AND start_chapter<=?4 AND end_chapter>=?4 AND status NOT IN ('completed','cancelled','abandoned'))",
                                rusqlite::params![project_id, branch_id, book_number, chapter_number],
                                |r| r.get(0),
                            )?;
                            if !active_run_exists {
                                return Ok(None);
                            }
                            let prev: i64 = conn
                                .query_row(
                                    "SELECT plan_revision FROM chapter_plan WHERE project_id=?1 AND branch_id=?2 AND book_number=?3 AND chapter_number=?4",
                                    rusqlite::params![project_id, branch_id, book_number, chapter_number],
                                    |r| r.get(0),
                                )
                                .unwrap_or(1);
                            Ok(Some(prev))
                        })
                        .await?
                } else {
                    None
                };
                let synopsis = plan_input.synopsis.clone();
                let pov_character_id = plan_input.pov_character_id.clone();
                let reseed_scenes: Vec<(Option<i32>, spindle_core::models::PlanChapterSceneInput)> =
                    plan_input
                        .scenes
                        .iter()
                        .map(|sc| (Some(sc.scene_order), sc.clone()))
                        .collect();
                let out = self.plan_chapter(plan_input).await?;
                ids.insert(item.output_ref.clone(), out.chapter_plan_id.clone());
                if let (Some(scope), Some(prev_rev)) = (scope, prior_rev) {
                    let next_revision = prev_rev.max(1) + 1;
                    self.repository
                        .set_chapter_plan_revision(
                            &scope.project_id,
                            &scope.branch_id,
                            book_number,
                            chapter_number,
                            next_revision,
                        )
                        .await?;
                    self.repository
                        .reseed_authoring_runs_for_chapter_plan(
                            &scope.project_id,
                            &scope.branch_id,
                            book_number,
                            chapter_number,
                            &synopsis,
                            pov_character_id.as_deref(),
                            &reseed_scenes,
                            next_revision,
                        )
                        .await?;
                }
                PlanningAppliedOutput::PlanChapter(Box::new(out))
            }
            SetBookOutline(_) => {
                let out = self
                    .set_book_outline(serde_json::from_value(input)?)
                    .await?;
                let stored = self
                    .repository
                    .get_book_outline(&out.outline.book_id, &out.outline.branch_id)
                    .await?
                    .context("missing accepted book outline")?;
                ids.insert(item.output_ref.clone(), stored.id);
                PlanningAppliedOutput::SetBookOutline(Box::new(out))
            }
            SetChapterOutline(_) => {
                let out = self
                    .set_chapter_outline(serde_json::from_value(input)?)
                    .await?;
                let stored = self
                    .repository
                    .get_chapter_outline(&out.outline.chapter_id, &out.outline.branch_id)
                    .await?
                    .context("missing accepted chapter outline")?;
                ids.insert(item.output_ref.clone(), stored.id);
                PlanningAppliedOutput::SetChapterOutline(Box::new(out))
            }
            UpdateReaderContract(_) => {
                let project = scope.as_ref().context("reader contract requires project")?;
                let out = self
                    .update_entity(UpdateEntityInput {
                        entity_type: "project".into(),
                        entity_id: project.project_id.clone(),
                        changes: json!({"reader_contract":input}),
                        allow_rename: None,
                    })
                    .await?;
                ids.insert(item.output_ref.clone(), project.project_id.clone());
                PlanningAppliedOutput::UpdateReaderContract(Box::new(out))
            }
            CreateStorySpine(_) => {
                let out = self
                    .create_story_spine(serde_json::from_value(input)?)
                    .await?;
                ids.insert(item.output_ref.clone(), out.story_spine_id.clone());
                PlanningAppliedOutput::CreateStorySpine(Box::new(out))
            }
        };
        Ok((output, ids, root))
    }
}
