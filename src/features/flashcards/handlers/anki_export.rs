#[cfg(feature = "ssr")]
use crate::features::flashcards::anki_builder;
#[cfg(feature = "ssr")]
use crate::features::flashcards::markdown::markdown_to_html;

#[cfg(feature = "ssr")]
pub async fn download_deck_as_anki(
    axum::extract::Path(deck_id): axum::extract::Path<i64>,
    axum::extract::State(app_state): axum::extract::State<crate::app_state::AppState>,
    session: tower_sessions::Session,
) -> Result<([(axum::http::HeaderName, String); 2], Vec<u8>), axum::http::StatusCode> {
    use rusqlite::Connection;
    use std::io::Write;
    use std::time::{SystemTime, UNIX_EPOCH};
    use zip::write::SimpleFileOptions;
    use zip::ZipWriter;

    use crate::features::auth::utils::get_user_from_session;

    let user = get_user_from_session(&session)
        .await
        .ok_or(axum::http::StatusCode::UNAUTHORIZED)?;

    let pool = &app_state.db_pool;

    let deck = sqlx::query!(
        r#"
        SELECT fd.id, fd.name, fd.description
        FROM flashcard_decks fd
        INNER JOIN study_projects sp ON fd.project_id = sp.id
        WHERE fd.id = ? AND sp.user_id = ?
        "#,
        deck_id,
        user.id
    )
    .fetch_optional(pool)
    .await
    .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(axum::http::StatusCode::NOT_FOUND)?;

    let cards = sqlx::query!(
        r#"
        SELECT 
            f.id,
            f.front,
            f.back,
            f.file_id,
            pf.original_filename as "original_filename?"
        FROM flashcards f
        LEFT JOIN project_files pf ON f.file_id = pf.id
        WHERE f.deck_id = ?
        ORDER BY f.created_at ASC
        "#,
        deck_id
    )
    .fetch_all(pool)
    .await
    .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    if cards.is_empty() {
        return Err(axum::http::StatusCode::BAD_REQUEST);
    }

    let temp_db = tempfile::NamedTempFile::new()
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    let db_path = temp_db.path();

    let conn =
        Connection::open(db_path).map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    anki_builder::create_anki_schema(&conn)
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;

    let anki_deck_id = 1_000_000_000 + deck_id;
    let model_id = 1_607_392_319_i64;

    let deck_name = &deck.name;
    let deck_desc = deck.description.as_deref().unwrap_or("");
    anki_builder::insert_deck(&conn, anki_deck_id, deck_name, deck_desc)
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    anki_builder::insert_basic_model(&conn, model_id, timestamp)
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    for (idx, card) in cards.iter().enumerate() {
        let note_id = timestamp + idx as i64;
        let card_id = timestamp + 10000 + idx as i64;

        let mut tags = String::from(" ");
        if let Some(filename) = &card.original_filename {
            let tag = filename
                .trim()
                .replace(|c: char| !c.is_alphanumeric() && c != '_' && c != '-', "_")
                .to_lowercase()
                .replace(' ', "_");
            if !tag.is_empty() {
                tags = format!(" {} ", tag);
            }
        }

        // Convert markdown to HTML for Anki
        let front_html = markdown_to_html(&card.front);
        let back_html = markdown_to_html(&card.back);

        anki_builder::insert_note(
            &conn,
            note_id,
            model_id,
            &front_html,
            &back_html,
            &tags,
            timestamp,
        )
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

        anki_builder::insert_card(&conn, card_id, note_id, anki_deck_id, timestamp)
            .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    drop(conn);

    let apkg_buffer = Vec::new();
    let mut zip = ZipWriter::new(std::io::Cursor::new(apkg_buffer));
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    let db_contents = tokio::fs::read(db_path)
        .await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    zip.start_file("collection.anki2", options)
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    zip.write_all(&db_contents)
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    zip.start_file("media", options)
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    zip.write_all(b"{}")
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    let cursor = zip
        .finish()
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    let file_contents = cursor.into_inner();

    let safe_filename = deck
        .name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>();
    let filename = format!("{}.apkg", safe_filename);

    Ok((
        [
            (
                axum::http::header::CONTENT_TYPE,
                "application/octet-stream".to_string(),
            ),
            (
                axum::http::header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{}\"", filename),
            ),
        ],
        file_contents,
    ))
}
