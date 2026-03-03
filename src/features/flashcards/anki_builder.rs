use rusqlite::{params, Connection};
use serde_json;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn create_anki_schema(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch(
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
    )?;
    Ok(())
}

pub fn insert_deck(
    conn: &Connection,
    deck_id: i64,
    name: &str,
    description: &str,
) -> Result<(), rusqlite::Error> {
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

    conn.execute(
        "INSERT INTO col VALUES(1, ?, ?, 0, 11, 0, 0, 0, ?, ?, ?, ?, '{}')",
        params![
            timestamp,
            timestamp,
            "{}",
            "{}",
            decks_json.to_string(),
            conf_json.to_string(),
        ],
    )?;

    Ok(())
}

pub fn insert_basic_model(
    conn: &Connection,
    model_id: i64,
    timestamp: i64,
) -> Result<(), rusqlite::Error> {
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
    conn.execute("UPDATE col SET models = ?", params![model_json.to_string()])?;

    Ok(())
}

pub fn insert_note(
    conn: &Connection,
    note_id: i64,
    model_id: i64,
    front: &str,
    back: &str,
    tags: &str,
    timestamp: i64,
) -> Result<(), rusqlite::Error> {
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

    conn.execute(
        "INSERT INTO notes VALUES(?, ?, ?, ?, -1, ?, ?, 0, ?, 0, '')",
        params![
            note_id,
            guid,
            model_id,
            timestamp / 1000,
            tags,
            fields,
            csum
        ],
    )?;

    Ok(())
}

pub fn insert_card(
    conn: &Connection,
    card_id: i64,
    note_id: i64,
    deck_id: i64,
    timestamp: i64,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT INTO cards VALUES(?, ?, ?, 0, ?, -1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, '')",
        params![card_id, note_id, deck_id, timestamp / 1000],
    )?;

    Ok(())
}
