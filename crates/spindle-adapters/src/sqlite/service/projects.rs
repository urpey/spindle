use super::*;

impl SqliteSpindleService {
    // =========================================================================
    // Project tools
    // =========================================================================

    /// Create a new project plus its initial main branch + book + chapter,
    /// returning the public-facing record-id triple the MCP layer expects.
    /// Mirrors the SurrealDB-side service method 1:1 — same `CreateProjectInput`
    /// and `CreateProjectOutput` shapes from spindle_core.
    pub async fn create_project(&self, input: CreateProjectInput) -> Result<CreateProjectOutput> {
        let (project, branch, book, chapter) = self.repository.create_project(&input).await?;
        Ok(CreateProjectOutput {
            project_id: project.id,
            branch_id: branch.id,
            book_id: book.id,
            chapter_id: chapter.id,
        })
    }

    /// Planning's CreateProject apply path marks the project McKee-structured.
    /// This is deliberately not a public tool or an opt-in switch.
    pub(crate) async fn create_mckee_project(
        &self,
        input: CreateProjectInput,
    ) -> Result<CreateProjectOutput> {
        let (project, branch, book, chapter) = self
            .repository
            .create_project_with_mckee_marker(&input)
            .await?;
        Ok(CreateProjectOutput {
            project_id: project.id,
            branch_id: branch.id,
            book_id: book.id,
            chapter_id: chapter.id,
        })
    }

    /// List every project, newest first.
    pub async fn list_projects(&self) -> Result<ListProjectsOutput> {
        let projects = self.repository.list_projects().await?;
        let projects = projects
            .into_iter()
            .map(|p| ProjectSummary {
                project_id: p.id,
                name: p.name,
                project_type: p.project_type,
                genre: p.genre,
                active_style_profile_id: p.active_style_profile_id,
                mckee_structured: p.mckee_structured,
            })
            .collect();
        Ok(ListProjectsOutput { projects })
    }
}
