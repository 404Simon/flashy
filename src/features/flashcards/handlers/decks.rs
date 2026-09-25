use leptos::prelude::*;

use super::super::models::{
    CreateFlashcardRequest, DeckSummary, FileCardGroup, Flashcard, FlashcardDeck,
};
#[cfg(feature = "ssr")]
use crate::features::auth::utils::require_auth;

// ============= DECK OPERATIONS =============

#[server(CreateDeck)]
pub async fn create_deck(
    project_id: i64,
    name: String,
    description: Option<String>,
) -> Result<FlashcardDeck, ServerFnError> {
    use sqlx::SqlitePool;

    let user = require_auth().await?;
    let name = name.trim().to_string();

    if name.len() < 2 {
        return Err(ServerFnError::new(
            "Deck name must be at least 2 characters",
        ));
    }

    let pool = expect_context::<SqlitePool>();

    // Verify user owns the project
    let project = sqlx::query!(
        "SELECT id FROM study_projects WHERE id = ? AND user_id = ?",
        project_id,
        user.id
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .ok_or_else(|| ServerFnError::new("Project not found or access denied"))?;

    let description = description
        .map(|d| d.trim().to_string())
        .filter(|d| !d.is_empty());

    let result = sqlx::query!(
        "INSERT INTO flashcard_decks (project_id, name, description) VALUES (?, ?, ?)",
        project.id,
        name,
        description
    )
    .execute(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    let deck_id = result.last_insert_rowid();

    let deck = sqlx::query_as!(
        FlashcardDeck,
        r#"
        SELECT id, project_id, name, description, created_at, updated_at
        FROM flashcard_decks
        WHERE id = ?
        "#,
        deck_id
    )
    .fetch_one(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(deck)
}

#[server(ListDecksForProject)]
pub async fn list_decks_for_project(project_id: i64) -> Result<Vec<DeckSummary>, ServerFnError> {
    use crate::{
        features::flashcards::service::FlashcardService,
        services::{Actor, to_server_error},
    };
    use sqlx::SqlitePool;

    let user = require_auth().await?;
    let pool = expect_context::<SqlitePool>();
    let actor = Actor::new(user.id).map_err(to_server_error)?;
    FlashcardService::new(pool)
        .list_decks_all(&actor, project_id)
        .await
        .map_err(to_server_error)
}

#[server(GetDeck)]
pub async fn get_deck(deck_id: i64) -> Result<FlashcardDeck, ServerFnError> {
    use crate::{
        features::flashcards::service::FlashcardService,
        services::{Actor, to_server_error},
    };
    use sqlx::SqlitePool;

    let user = require_auth().await?;
    let pool = expect_context::<SqlitePool>();

    let actor = Actor::new(user.id).map_err(to_server_error)?;
    FlashcardService::new(pool)
        .get_deck(&actor, deck_id)
        .await
        .map_err(to_server_error)
}

#[server(UpdateDeck)]
pub async fn update_deck(
    deck_id: i64,
    name: String,
    description: Option<String>,
) -> Result<FlashcardDeck, ServerFnError> {
    use sqlx::SqlitePool;

    let user = require_auth().await?;
    let pool = expect_context::<SqlitePool>();

    let name = name.trim().to_string();
    if name.len() < 2 {
        return Err(ServerFnError::new(
            "Deck name must be at least 2 characters",
        ));
    }

    let description = description
        .map(|d| d.trim().to_string())
        .filter(|d| !d.is_empty());

    // Update and verify ownership
    let result = sqlx::query!(
        r#"
        UPDATE flashcard_decks
        SET name = ?, description = ?, updated_at = datetime('now')
        WHERE id = ? AND project_id IN (
            SELECT id FROM study_projects WHERE user_id = ?
        )
        "#,
        name,
        description,
        deck_id,
        user.id
    )
    .execute(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    if result.rows_affected() == 0 {
        return Err(ServerFnError::new("Deck not found or access denied"));
    }

    // Fetch and return updated deck
    let deck = sqlx::query_as!(
        FlashcardDeck,
        r#"
        SELECT id, project_id, name, description, created_at, updated_at
        FROM flashcard_decks
        WHERE id = ?
        "#,
        deck_id
    )
    .fetch_one(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(deck)
}

#[server(DeleteDeck)]
pub async fn delete_deck(deck_id: i64) -> Result<(), ServerFnError> {
    use sqlx::SqlitePool;

    let user = require_auth().await?;
    let pool = expect_context::<SqlitePool>();

    // Verify ownership and delete
    let result = sqlx::query!(
        r#"
        DELETE FROM flashcard_decks
        WHERE id = ? AND project_id IN (
            SELECT id FROM study_projects WHERE user_id = ?
        )
        "#,
        deck_id,
        user.id
    )
    .execute(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    if result.rows_affected() == 0 {
        return Err(ServerFnError::new("Deck not found or access denied"));
    }

    Ok(())
}

#[server(ListFilesWithCardsForDeck)]
pub async fn list_files_with_cards_for_deck(
    deck_id: i64,
) -> Result<Vec<FileCardGroup>, ServerFnError> {
    use sqlx::SqlitePool;

    let user = require_auth().await?;
    let pool = expect_context::<SqlitePool>();

    // Verify user owns the deck
    sqlx::query!(
        r#"
        SELECT fd.id
        FROM flashcard_decks fd
        INNER JOIN study_projects sp ON fd.project_id = sp.id
        WHERE fd.id = ? AND sp.user_id = ?
        "#,
        deck_id,
        user.id
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .ok_or_else(|| ServerFnError::new("Deck not found or access denied"))?;

    let rows = sqlx::query!(
        r#"
        SELECT
            pf.id as "file_id!: i64",
            pf.original_filename as "file_name!: String",
            CAST(COUNT(fc.id) AS INTEGER) as "card_count!: i64",
            MIN(fc.created_at) as "created_at!: String"
        FROM project_files pf
        INNER JOIN flashcards fc ON pf.id = fc.file_id
        WHERE fc.deck_id = ?
        GROUP BY pf.id, pf.original_filename
        ORDER BY MIN(fc.created_at) DESC
        "#,
        deck_id
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|row| FileCardGroup {
            file_id: row.file_id,
            file_name: row.file_name,
            card_count: row.card_count,
            created_at: row.created_at,
        })
        .collect())
}

#[server(ListFlashcardsByFile)]
pub async fn list_flashcards_by_file(
    deck_id: i64,
    file_id: i64,
) -> Result<Vec<Flashcard>, ServerFnError> {
    use crate::{
        features::flashcards::service::FlashcardService,
        services::{Actor, to_server_error},
    };
    use sqlx::SqlitePool;

    let user = require_auth().await?;
    let pool = expect_context::<SqlitePool>();

    let actor = Actor::new(user.id).map_err(to_server_error)?;
    FlashcardService::new(pool)
        .list_cards_all(&actor, deck_id, Some(file_id))
        .await
        .map_err(to_server_error)
}

// ============= FLASHCARD OPERATIONS =============

#[server(CreateFlashcard)]
pub async fn create_flashcard(
    deck_id: i64,
    card: CreateFlashcardRequest,
) -> Result<Flashcard, ServerFnError> {
    use sqlx::SqlitePool;

    let user = require_auth().await?;
    let pool = expect_context::<SqlitePool>();

    let front = card.front.trim().to_string();
    let back = card.back.trim().to_string();

    if front.is_empty() || back.is_empty() {
        return Err(ServerFnError::new("Front and back cannot be empty"));
    }

    // Verify user owns the deck
    sqlx::query!(
        r#"
        SELECT fd.id
        FROM flashcard_decks fd
        INNER JOIN study_projects sp ON fd.project_id = sp.id
        WHERE fd.id = ? AND sp.user_id = ?
        "#,
        deck_id,
        user.id
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .ok_or_else(|| ServerFnError::new("Deck not found or access denied"))?;

    if let Some(file_id) = card.file_id {
        sqlx::query_scalar!(
            r#"SELECT pf.id FROM project_files pf
               JOIN study_projects sp ON sp.id = pf.project_id
               WHERE pf.id = ? AND sp.user_id = ?"#,
            file_id,
            user.id
        )
        .fetch_optional(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .ok_or_else(|| ServerFnError::new("File not found or access denied"))?;
    }

    let result = sqlx::query!(
        "INSERT INTO flashcards (deck_id, front, back, document_reference, file_id) VALUES (?, ?, ?, ?, ?)",
        deck_id,
        front,
        back,
        card.document_reference,
        card.file_id
    )
    .execute(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    let card_id = result.last_insert_rowid();

    let flashcard = sqlx::query_as!(
        Flashcard,
        r#"
        SELECT id, deck_id, front, back, document_reference, file_id, created_at, updated_at
        FROM flashcards
        WHERE id = ?
        "#,
        card_id
    )
    .fetch_one(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(flashcard)
}

#[server(ListFlashcards)]
pub async fn list_flashcards(deck_id: i64) -> Result<Vec<Flashcard>, ServerFnError> {
    use crate::{
        features::flashcards::service::FlashcardService,
        services::{Actor, to_server_error},
    };
    use sqlx::SqlitePool;

    let user = require_auth().await?;
    let pool = expect_context::<SqlitePool>();

    let actor = Actor::new(user.id).map_err(to_server_error)?;
    FlashcardService::new(pool)
        .list_cards_all(&actor, deck_id, None)
        .await
        .map_err(to_server_error)
}

#[server(UpdateFlashcard)]
pub async fn update_flashcard(
    card_id: i64,
    front: String,
    back: String,
    document_reference: Option<String>,
) -> Result<Flashcard, ServerFnError> {
    use sqlx::SqlitePool;

    let user = require_auth().await?;
    let pool = expect_context::<SqlitePool>();

    let front = front.trim().to_string();
    let back = back.trim().to_string();

    if front.is_empty() || back.is_empty() {
        return Err(ServerFnError::new("Front and back cannot be empty"));
    }

    // Update and verify ownership
    let result = sqlx::query!(
        r#"
        UPDATE flashcards
        SET front = ?, back = ?, document_reference = ?, updated_at = datetime('now')
        WHERE id = ? AND deck_id IN (
            SELECT fd.id
            FROM flashcard_decks fd
            INNER JOIN study_projects sp ON fd.project_id = sp.id
            WHERE sp.user_id = ?
        )
        "#,
        front,
        back,
        document_reference,
        card_id,
        user.id
    )
    .execute(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    if result.rows_affected() == 0 {
        return Err(ServerFnError::new("Card not found or access denied"));
    }

    let flashcard = sqlx::query_as!(
        Flashcard,
        r#"
        SELECT id, deck_id, front, back, document_reference, file_id, created_at, updated_at
        FROM flashcards
        WHERE id = ?
        "#,
        card_id
    )
    .fetch_one(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(flashcard)
}

#[server(DeleteFlashcard)]
pub async fn delete_flashcard(card_id: i64) -> Result<(), ServerFnError> {
    use sqlx::SqlitePool;

    let user = require_auth().await?;
    let pool = expect_context::<SqlitePool>();

    // Delete and verify ownership
    let result = sqlx::query!(
        r#"
        DELETE FROM flashcards
        WHERE id = ? AND deck_id IN (
            SELECT fd.id
            FROM flashcard_decks fd
            INNER JOIN study_projects sp ON fd.project_id = sp.id
            WHERE sp.user_id = ?
        )
        "#,
        card_id,
        user.id
    )
    .execute(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    if result.rows_affected() == 0 {
        return Err(ServerFnError::new("Card not found or access denied"));
    }

    Ok(())
}
