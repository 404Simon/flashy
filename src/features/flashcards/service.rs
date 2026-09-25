#![cfg(feature = "ssr")]

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::{
    features::flashcards::models::{DeckSummary, Flashcard, FlashcardDeck},
    services::{Actor, Page, PageRequest, ServiceError, decode_cursor, encode_cursor},
};

#[derive(Clone)]
pub struct FlashcardService {
    pool: SqlitePool,
}

#[derive(Deserialize, Serialize)]
struct DeckCursor {
    project_id: i64,
    before_id: i64,
}

#[derive(Deserialize, Serialize)]
struct CardCursor {
    deck_id: i64,
    file_id: Option<i64>,
    after_id: i64,
}

impl FlashcardService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn list_decks_all(
        &self,
        actor: &Actor,
        project_id: i64,
    ) -> Result<Vec<DeckSummary>, ServiceError> {
        self.query_decks(actor, project_id, None, None).await
    }

    pub async fn list_decks(
        &self,
        actor: &Actor,
        project_id: i64,
        page: PageRequest,
    ) -> Result<Page<DeckSummary>, ServiceError> {
        let limit = page.limit()?;
        let cursor: Option<DeckCursor> = decode_cursor(page.cursor.as_deref())?;
        if cursor.as_ref().is_some_and(|c| c.project_id != project_id) {
            return Err(ServiceError::InvalidInput(
                "cursor does not belong to this project",
            ));
        }
        let mut items = self
            .query_decks(
                actor,
                project_id,
                cursor.map(|c| c.before_id),
                Some(limit + 1),
            )
            .await?;
        let more = items.len() > limit as usize;
        items.truncate(limit as usize);
        let next_cursor = if more {
            items
                .last()
                .map(|x| {
                    encode_cursor(&DeckCursor {
                        project_id,
                        before_id: x.id,
                    })
                })
                .transpose()?
        } else {
            None
        };
        Ok(Page { items, next_cursor })
    }

    async fn query_decks(
        &self,
        actor: &Actor,
        project_id: i64,
        before_id: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<DeckSummary>, ServiceError> {
        let rows = sqlx::query_as::<_, DeckSummary>(r#"SELECT fd.id, fd.project_id, fd.name, fd.description, fd.created_at,
            CAST(COUNT(fc.id) AS INTEGER) card_count FROM flashcard_decks fd
            JOIN study_projects sp ON sp.id = fd.project_id LEFT JOIN flashcards fc ON fc.deck_id = fd.id
            WHERE fd.project_id = ? AND sp.user_id = ? AND (? IS NULL OR fd.id < ?)
            GROUP BY fd.id ORDER BY fd.id DESC LIMIT COALESCE(?, -1)"#)
            .bind(project_id).bind(actor.user_id()).bind(before_id).bind(before_id).bind(limit).fetch_all(&self.pool).await?;
        if rows.is_empty()
            && sqlx::query_scalar::<_, i64>(
                "SELECT 1 FROM study_projects WHERE id = ? AND user_id = ?",
            )
            .bind(project_id)
            .bind(actor.user_id())
            .fetch_optional(&self.pool)
            .await?
            .is_none()
        {
            return Err(ServiceError::NotFound);
        }
        Ok(rows)
    }

    pub async fn get_deck(
        &self,
        actor: &Actor,
        deck_id: i64,
    ) -> Result<FlashcardDeck, ServiceError> {
        sqlx::query_as::<_, FlashcardDeck>(r#"SELECT fd.id, fd.project_id, fd.name, fd.description, fd.created_at, fd.updated_at
            FROM flashcard_decks fd JOIN study_projects sp ON sp.id = fd.project_id WHERE fd.id = ? AND sp.user_id = ?"#)
            .bind(deck_id).bind(actor.user_id()).fetch_optional(&self.pool).await?.ok_or(ServiceError::NotFound)
    }

    pub async fn list_cards_all(
        &self,
        actor: &Actor,
        deck_id: i64,
        file_id: Option<i64>,
    ) -> Result<Vec<Flashcard>, ServiceError> {
        self.query_cards(actor, deck_id, file_id, 0, None).await
    }

    pub async fn list_cards(
        &self,
        actor: &Actor,
        deck_id: i64,
        file_id: Option<i64>,
        page: PageRequest,
    ) -> Result<Page<Flashcard>, ServiceError> {
        let limit = page.limit()?;
        let cursor: Option<CardCursor> = decode_cursor(page.cursor.as_deref())?;
        if cursor
            .as_ref()
            .is_some_and(|c| c.deck_id != deck_id || c.file_id != file_id)
        {
            return Err(ServiceError::InvalidInput(
                "cursor does not belong to these filters",
            ));
        }
        let after = cursor.map_or(0, |c| c.after_id);
        let mut items = self
            .query_cards(actor, deck_id, file_id, after, Some(limit + 1))
            .await?;
        let more = items.len() > limit as usize;
        items.truncate(limit as usize);
        let next_cursor = if more {
            items
                .last()
                .map(|x| {
                    encode_cursor(&CardCursor {
                        deck_id,
                        file_id,
                        after_id: x.id,
                    })
                })
                .transpose()?
        } else {
            None
        };
        Ok(Page { items, next_cursor })
    }

    async fn query_cards(
        &self,
        actor: &Actor,
        deck_id: i64,
        file_id: Option<i64>,
        after_id: i64,
        limit: Option<u32>,
    ) -> Result<Vec<Flashcard>, ServiceError> {
        let rows = sqlx::query_as::<_, Flashcard>(r#"SELECT fc.id, fc.deck_id, fc.front, fc.back, fc.document_reference,
            CASE WHEN owner_file.id IS NOT NULL THEN fc.file_id ELSE NULL END AS file_id, fc.created_at, fc.updated_at
            FROM flashcards fc JOIN flashcard_decks fd ON fd.id = fc.deck_id JOIN study_projects sp ON sp.id = fd.project_id
            LEFT JOIN project_files owner_file ON owner_file.id = fc.file_id AND owner_file.project_id IN (SELECT id FROM study_projects WHERE user_id = ?)
            WHERE fc.deck_id = ? AND sp.user_id = ? AND (? IS NULL OR fc.file_id = ?) AND fc.id > ?
            ORDER BY fc.id ASC LIMIT COALESCE(?, -1)"#)
            .bind(actor.user_id()).bind(deck_id).bind(actor.user_id()).bind(file_id).bind(file_id).bind(after_id).bind(limit).fetch_all(&self.pool).await?;
        if rows.is_empty() && self.get_deck(actor, deck_id).await.is_err() {
            return Err(ServiceError::NotFound);
        }
        Ok(rows)
    }

    pub async fn get_card(&self, actor: &Actor, card_id: i64) -> Result<Flashcard, ServiceError> {
        sqlx::query_as::<_, Flashcard>(r#"SELECT fc.id, fc.deck_id, fc.front, fc.back, fc.document_reference,
            CASE WHEN owner_file.id IS NOT NULL THEN fc.file_id ELSE NULL END AS file_id, fc.created_at, fc.updated_at
            FROM flashcards fc JOIN flashcard_decks fd ON fd.id = fc.deck_id JOIN study_projects sp ON sp.id = fd.project_id
            LEFT JOIN project_files owner_file ON owner_file.id = fc.file_id AND owner_file.project_id IN (SELECT id FROM study_projects WHERE user_id = ?)
            WHERE fc.id = ? AND sp.user_id = ?"#).bind(actor.user_id()).bind(card_id).bind(actor.user_id()).fetch_optional(&self.pool).await?.ok_or(ServiceError::NotFound)
    }
}
