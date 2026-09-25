use leptos::prelude::*;

use super::super::models::{Project, ProjectSummary};
#[cfg(feature = "ssr")]
use crate::features::auth::utils::require_auth;

#[server(CreateProject)]
pub async fn create_project(
    name: String,
    description: Option<String>,
) -> Result<Project, ServerFnError> {
    use sqlx::SqlitePool;

    let user = require_auth().await?;
    let name = name.trim().to_string();

    if name.len() < 3 {
        return Err(ServerFnError::new(
            "Project name must be at least 3 characters",
        ));
    }

    let description = description
        .map(|d| d.trim().to_string())
        .filter(|d| !d.is_empty());

    let pool = expect_context::<SqlitePool>();

    let result = sqlx::query!(
        "INSERT INTO study_projects (user_id, name, description) VALUES (?, ?, ?)",
        user.id,
        name,
        description
    )
    .execute(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    let project_id = result.last_insert_rowid();

    let project = sqlx::query_as!(
        Project,
        r#"
        SELECT id, user_id, name, description, created_at, updated_at
        FROM study_projects
        WHERE id = ?
        "#,
        project_id
    )
    .fetch_one(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(project)
}

#[server(ListProjects)]
pub async fn list_projects() -> Result<Vec<ProjectSummary>, ServerFnError> {
    use crate::{
        features::projects::service::ProjectService,
        services::{Actor, to_server_error},
    };
    use sqlx::SqlitePool;

    let user = require_auth().await?;
    let pool = expect_context::<SqlitePool>();
    let actor = Actor::new(user.id).map_err(to_server_error)?;
    ProjectService::new(pool)
        .list_all(&actor)
        .await
        .map_err(to_server_error)
}

#[server(GetProject)]
pub async fn get_project(project_id: i64) -> Result<Project, ServerFnError> {
    use crate::{
        features::projects::service::ProjectService,
        services::{Actor, to_server_error},
    };
    use sqlx::SqlitePool;

    let user = require_auth().await?;
    let pool = expect_context::<SqlitePool>();

    let actor = Actor::new(user.id).map_err(to_server_error)?;
    ProjectService::new(pool)
        .get(&actor, project_id)
        .await
        .map_err(to_server_error)
}
