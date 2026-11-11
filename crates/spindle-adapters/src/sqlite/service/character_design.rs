//! Character Design service reads: planning sees the full design, every other
//! read sees a redacted slice.
use super::*;
use spindle_core::character_design::{
    CharacterDesign, CharacterDesignRedactedView, CharacterDesignView, CreateCharacterDesignInput,
    CreateCharacterDesignOutput,
};

impl SqliteSpindleService {
    /// Planning is the only create path; this is not exposed as a direct tool.
    pub(crate) async fn create_character_design(
        &self,
        input: CreateCharacterDesignInput,
    ) -> Result<CreateCharacterDesignOutput> {
        self.repository.create_character_design(&input).await
    }

    /// Resolve the two secret fact texts that a full planning view carries.
    /// The texts live only on the canonical facts; the design row keeps their
    /// ids, so this is the single place that joins them.
    async fn character_design_view(&self, design: &CharacterDesign) -> Result<CharacterDesignView> {
        let hidden = self
            .repository
            .get_canonical_fact(&design.hidden_self_fact_id)
            .await?;
        let unconscious = self
            .repository
            .get_canonical_fact(&design.unconscious_desire_fact_id)
            .await?;
        Ok(CharacterDesignView {
            character_design_id: design.character_design_id.clone(),
            character_id: design.character_id.clone(),
            contradictions: design.contradictions.clone(),
            public_self: design.public_self.clone(),
            personal_self: design.personal_self.clone(),
            private_self: design.private_self.clone(),
            hidden_self: hidden.value_text.unwrap_or_default(),
            conscious_desire: design.conscious_desire.clone(),
            unconscious_desire: unconscious.value_text.unwrap_or_default(),
            pressure_on_protagonist: design.pressure_on_protagonist.clone(),
            behavior_cues: design.behavior_cues.clone(),
        })
    }

    /// Full Character Designs for a project. Planning tools are the only caller:
    /// each view includes the hidden layer and unconscious desire texts.
    pub async fn list_character_design_views(
        &self,
        project_id: &str,
    ) -> Result<Vec<CharacterDesignView>> {
        let designs = self.repository.list_character_designs(project_id).await?;
        let mut views = Vec::with_capacity(designs.len());
        for design in &designs {
            views.push(self.character_design_view(design).await?);
        }
        Ok(views)
    }

    /// Redacted design for a character, for non-planning reads. It never loads
    /// the secret facts, so the hidden layer and unconscious desire cannot
    /// reach a profile-like read.
    pub async fn get_character_design_redacted(
        &self,
        project_id: &str,
        character_id: &str,
    ) -> Result<Option<CharacterDesignRedactedView>> {
        Ok(self
            .repository
            .get_character_design_for_character(project_id, character_id)
            .await?
            .as_ref()
            .map(CharacterDesignRedactedView::from_design))
    }
}
