use super::*;
use spindle_core::story_spine::{CreateStorySpineInput, CreateStorySpineOutput, StorySpine};

impl SqliteSpindleService {
    /// Planning is the only create path; this is not exposed as a direct tool.
    pub(crate) async fn create_story_spine(
        &self,
        input: CreateStorySpineInput,
    ) -> Result<CreateStorySpineOutput> {
        self.repository.create_story_spine(&input).await
    }

    pub async fn get_story_spine(
        &self,
        project_id: &str,
        story_spine_id: &str,
    ) -> Result<StorySpine> {
        self.repository
            .get_story_spine(project_id, story_spine_id)
            .await
    }

    pub async fn list_story_spines(&self, project_id: &str) -> Result<Vec<StorySpine>> {
        self.repository.list_story_spines(project_id).await
    }
}
