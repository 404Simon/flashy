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
    use sqlx::SqlitePool;

    let user = require_auth().await?;
    let pool = expect_context::<SqlitePool>();

    let rows = sqlx::query!(
        r#"
        SELECT
            sp.id as "id!: i64",
            sp.name as "name!: String",
            CAST(COALESCE(sp.description, '') AS TEXT) as "description!: String",
            sp.created_at as "created_at!: String",
            CAST(COALESCE((
                SELECT COUNT(*)
                FROM project_files pf
                WHERE pf.project_id = sp.id
            ), 0) AS INTEGER) as "file_count!: i64"
        FROM study_projects sp
        WHERE sp.user_id = ?
        ORDER BY sp.created_at DESC
        "#,
        user.id
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|row| ProjectSummary {
            id: row.id,
            name: row.name,
            description: if row.description.trim().is_empty() {
                None
            } else {
                Some(row.description)
            },
            created_at: row.created_at,
            file_count: row.file_count,
        })
        .collect())
}

#[server(GetProject)]
pub async fn get_project(project_id: i64) -> Result<Project, ServerFnError> {
    use sqlx::SqlitePool;

    let user = require_auth().await?;
    let pool = expect_context::<SqlitePool>();

    let project = sqlx::query_as!(
        Project,
        r#"
        SELECT id, user_id, name, description, created_at, updated_at
        FROM study_projects
        WHERE id = ? AND user_id = ?
        "#,
        project_id,
        user.id
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .ok_or_else(|| ServerFnError::new("Project not found"))?;

    Ok(project)
}
