use serde_json;
use sqlx::{SqliteConnection, query, raw_sql};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::{SystemTime, UNIX_EPOCH};

pub async fn create_anki_schema(conn: &mut SqliteConnection) -> Result<(), sqlx::Error> {
    raw_sql(
        r#"
        CREATE TABLE col (
            id              integer primary key,
            crt             integer not null,
            mod             integer not null,
            scm             integer not null,
            ver             integer not null,
            dty             integer not null,
            usn             integer not null,
            ls              integer not null,
            conf            text not null,
            models          text not null,
            decks           text not null,
            dconf           text not null,
            tags            text not null
        );
        
        CREATE TABLE notes (
            id              integer primary key,
            guid            text not null,
            mid             integer not null,
            mod             integer not null,
            usn             integer not null,
            tags            text not null,
            flds            text not null,
            sfld            integer not null,
            csum            integer not null,
            flags           integer not null,
            data            text not null
        );
        
        CREATE TABLE cards (
            id              integer primary key,
            nid             integer not null,
            did             integer not null,
            ord             integer not null,
            mod             integer not null,
            usn             integer not null,
            type            integer not null,
            queue           integer not null,
            due             integer not null,
            ivl             integer not null,
            factor          integer not null,
            reps            integer not null,
            lapses          integer not null,
            left            integer not null,
            odue            integer not null,
            odid            integer not null,
            flags           integer not null,
            data            text not null
        );
        
        CREATE TABLE revlog (
            id              integer primary key,
            cid             integer not null,
            usn             integer not null,
            ease            integer not null,
            ivl             integer not null,
            lastIvl         integer not null,
            factor          integer not null,
            time            integer not null,
            type            integer not null
        );
        
        CREATE TABLE graves (
            usn             integer not null,
            oid             integer not null,
            type            integer not null
        );
        
        CREATE INDEX ix_notes_usn on notes (usn);
        CREATE INDEX ix_cards_usn on cards (usn);
        CREATE INDEX ix_revlog_usn on revlog (usn);
        CREATE INDEX ix_cards_nid on cards (nid);
        CREATE INDEX ix_cards_sched on cards (did, queue, due);
        CREATE INDEX ix_revlog_cid on revlog (cid);
        CREATE INDEX ix_notes_csum on notes (csum);
        "#,
    )
    .execute(&mut *conn)
    .await?;
    Ok(())
}

pub async fn insert_deck(
    conn: &mut SqliteConnection,
    deck_id: i64,
    name: &str,
    description: &str,
) -> Result<(), sqlx::Error> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let decks_json = serde_json::json!({
        "1": {
            "id": 1,
            "mod": timestamp,
            "name": "Default",
            "usn": 0,
            "lrnToday": [0, 0],
            "revToday": [0, 0],
            "newToday": [0, 0],
            "timeToday": [0, 0],
            "collapsed": false,
            "desc": "",
            "dyn": 0,
            "conf": 1,
            "extendNew": 0,
            "extendRev": 50
        },
        deck_id.to_string(): {
            "id": deck_id,
            "mod": timestamp,
            "name": name,
            "usn": -1,
            "lrnToday": [0, 0],
            "revToday": [0, 0],
            "newToday": [0, 0],
            "timeToday": [0, 0],
            "collapsed": false,
            "desc": description,
            "dyn": 0,
            "conf": 1,
            "extendNew": 0,
            "extendRev": 50
        }
    });

    let conf_json = serde_json::json!({
        "1": {
            "id": 1,
            "mod": 0,
            "name": "Default",
            "usn": 0,
            "maxTaken": 60,
            "autoplay": true,
            "timer": 0,
            "replayq": true,
            "new": {
                "bury": false,
                "delays": [1.0, 10.0],
                "initialFactor": 2500,
                "ints": [1, 4, 0],
                "order": 1,
                "perDay": 20
            },
            "rev": {
                "bury": false,
                "ease4": 1.3,
                "ivlFct": 1.0,
                "maxIvl": 36500,
                "perDay": 200,
                "hardFactor": 1.2
            },
            "lapse": {
                "delays": [10.0],
                "leechAction": 1,
                "leechFails": 8,
                "minInt": 1,
                "mult": 0.0
            }
        }
    });

    query("INSERT INTO col VALUES(1, ?, ?, 0, 11, 0, 0, 0, ?, ?, ?, ?, '{}')")
        .bind(timestamp)
        .bind(timestamp)
        .bind("{}")
        .bind("{}")
        .bind(decks_json.to_string())
        .bind(conf_json.to_string())
        .execute(&mut *conn)
        .await?;

    Ok(())
}

pub async fn insert_basic_model(
    conn: &mut SqliteConnection,
    model_id: i64,
    timestamp: i64,
) -> Result<(), sqlx::Error> {
    let model_json = serde_json::json!({
        model_id.to_string(): {
            "id": model_id,
            "name": "Basic",
            "type": 0,
            "mod": timestamp / 1000,
            "usn": -1,
            "sortf": 0,
            "did": null,
            "tmpls": [{
                "name": "Card 1",
                "ord": 0,
                "qfmt": "{{Front}}",
                "afmt": "{{FrontSide}}\n\n<hr id=answer>\n\n{{Back}}",
                "bqfmt": "",
                "bafmt": "",
                "did": null
            }],
            "flds": [
                {"name": "Front", "ord": 0, "sticky": false, "rtl": false, "font": "Arial", "size": 20},
                {"name": "Back", "ord": 1, "sticky": false, "rtl": false, "font": "Arial", "size": 20}
            ],
            "css": ".card {\n font-family: arial, sans-serif;\n font-size: 20px;\n text-align: left;\n color: black;\n background-color: white;\n padding: 20px;\n}\n\npre {\n background-color: #f4f4f4;\n padding: 10px;\n border-radius: 5px;\n overflow-x: auto;\n}\n\ncode {\n background-color: #f4f4f4;\n padding: 2px 6px;\n border-radius: 3px;\n font-family: monospace;\n}\n\ntable {\n border-collapse: collapse;\n width: 100%;\n margin: 10px 0;\n}\n\ntable th, table td {\n border: 1px solid #ddd;\n padding: 8px;\n}\n\ntable th {\n background-color: #f4f4f4;\n font-weight: bold;\n}\n\nul, ol {\n margin: 10px 0;\n padding-left: 30px;\n}\n\nblockquote {\n border-left: 3px solid #ccc;\n margin: 10px 0;\n padding-left: 15px;\n color: #666;\n}\n\nhr {\n border: none;\n border-top: 1px solid #ccc;\n margin: 20px 0;\n}\n",
            "latexPre": "\\documentclass[12pt]{article}\n\\special{papersize=3in,5in}\n\\usepackage[utf8]{inputenc}\n\\usepackage{amssymb,amsmath}\n\\pagestyle{empty}\n\\setlength{\\parindent}{0in}\n\\begin{document}\n",
            "latexPost": "\\end{document}",
            "latexsvg": false,
            "req": [[0, "all", [0]]]
        }
    });

    // Update col with models
    query("UPDATE col SET models = ?")
        .bind(model_json.to_string())
        .execute(&mut *conn)
        .await?;

    Ok(())
}

pub async fn insert_note(
    conn: &mut SqliteConnection,
    note_id: i64,
    model_id: i64,
    front: &str,
    back: &str,
    tags: &str,
    timestamp: i64,
) -> Result<(), sqlx::Error> {
    // Generate GUID
    let mut hasher = DefaultHasher::new();
    format!("{}{}{}", front, back, timestamp).hash(&mut hasher);
    let guid = format!("{:x}", hasher.finish());

    // Join fields with \x1f separator
    let fields = format!("{}\x1f{}", front, back);

    // Calculate checksum (simple hash of first field)
    let mut hasher = DefaultHasher::new();
    front.hash(&mut hasher);
    let csum = (hasher.finish() & 0xFFFFFFFF) as i64;

    query("INSERT INTO notes VALUES(?, ?, ?, ?, -1, ?, ?, 0, ?, 0, '')")
        .bind(note_id)
        .bind(guid)
        .bind(model_id)
        .bind(timestamp / 1000)
        .bind(tags)
        .bind(fields)
        .bind(csum)
        .execute(&mut *conn)
        .await?;

    Ok(())
}

pub async fn insert_card(
    conn: &mut SqliteConnection,
    card_id: i64,
    note_id: i64,
    deck_id: i64,
    timestamp: i64,
) -> Result<(), sqlx::Error> {
    query("INSERT INTO cards VALUES(?, ?, ?, 0, ?, -1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, '')")
        .bind(card_id)
        .bind(note_id)
        .bind(deck_id)
        .bind(timestamp / 1000)
        .execute(&mut *conn)
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::Connection;
    use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqliteSynchronous};

    #[test]
    fn builds_readable_anki_collection() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        runtime.block_on(async {
            let temp_db = tempfile::NamedTempFile::new().unwrap();
            let db_path = temp_db.path();

            let options = SqliteConnectOptions::new()
                .filename(db_path)
                .create_if_missing(true)
                .journal_mode(SqliteJournalMode::Delete)
                .synchronous(SqliteSynchronous::Normal);

            let mut conn = SqliteConnection::connect_with(&options).await.unwrap();

            create_anki_schema(&mut conn).await.unwrap();
            insert_deck(&mut conn, 1_000_000_001, "Deck", "desc")
                .await
                .unwrap();
            insert_basic_model(&mut conn, 1_607_392_319, 1_700_000_000_000)
                .await
                .unwrap();
            insert_note(
                &mut conn,
                1_700_000_000_001,
                1_607_392_319,
                "front",
                "back",
                " tag ",
                1_700_000_000_000,
            )
            .await
            .unwrap();
            insert_card(
                &mut conn,
                1_700_000_010_001,
                1_700_000_000_001,
                1_000_000_001,
                1_700_000_000_000,
            )
            .await
            .unwrap();
            conn.close().await.unwrap();

            // Anki expects a single self-contained SQLite file, so no WAL sidecar
            // may be left behind.
            let wal_path = format!("{}-wal", db_path.display());
            assert!(!std::path::Path::new(&wal_path).exists());

            let mut conn = SqliteConnection::connect_with(
                &SqliteConnectOptions::new()
                    .filename(db_path)
                    .read_only(true),
            )
            .await
            .unwrap();

            let decks: String = sqlx::query_scalar("SELECT decks FROM col")
                .fetch_one(&mut conn)
                .await
                .unwrap();
            assert!(decks.contains("\"Deck\""));

            let models: String = sqlx::query_scalar("SELECT models FROM col")
                .fetch_one(&mut conn)
                .await
                .unwrap();
            assert!(models.contains("\"Basic\""));

            let notes: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM notes")
                .fetch_one(&mut conn)
                .await
                .unwrap();
            assert_eq!(notes, 1);

            let cards: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM cards")
                .fetch_one(&mut conn)
                .await
                .unwrap();
            assert_eq!(cards, 1);

            let front: String = sqlx::query_scalar("SELECT flds FROM notes")
                .fetch_one(&mut conn)
                .await
                .unwrap();
            assert_eq!(front, "front\x1fback");

            conn.close().await.unwrap();
        });
    }
}
