use std::collections::HashMap;
use rusqlite::Connection;

#[derive(Default, Clone, Copy, Debug)]
pub struct DailySummary {
    pub moji_count: u32,
    pub lines_read: u32,
    pub lines_filtered: u32,
    pub duration_secs: u64,
}

pub fn init_db() -> Connection {
    let conn = Connection::open("mojitracker.db").expect("Failed to open SQLite database");
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         CREATE TABLE IF NOT EXISTS lines (
             id INTEGER PRIMARY KEY AUTOINCREMENT,
             timestamp TEXT NOT NULL,
             text TEXT NOT NULL,
             moji_count INTEGER NOT NULL,
             is_duplicate BOOLEAN NOT NULL DEFAULT 0,
             is_flagged BOOLEAN NOT NULL DEFAULT 0
         );
         CREATE INDEX IF NOT EXISTS idx_lines_ts ON lines(timestamp);",
    )
    .expect("Failed to initialize database schema");
    conn
}

pub fn load_daily_stats(conn: &Connection) -> HashMap<String, DailySummary> {
    let mut stats = HashMap::new();
    let mut stmt = conn
        .prepare(
            "SELECT 
                substr(timestamp, 1, 10) AS day,
                SUM(CASE WHEN is_duplicate = 0 THEN moji_count ELSE 0 END),
                SUM(CASE WHEN is_duplicate = 0 THEN 1 ELSE 0 END),
                SUM(CASE WHEN is_duplicate = 1 THEN 1 ELSE 0 END),
                MIN(timestamp),
                MAX(timestamp)
             FROM lines
             GROUP BY day",
        )
        .unwrap();

    let rows = stmt
        .query_map([], |row| {
            let day: String = row.get(0)?;
            let moji: u32 = row.get(1)?;
            let lines: u32 = row.get(2)?;
            let filtered: u32 = row.get(3)?;
            let min_ts: Option<String> = row.get(4)?;
            let max_ts: Option<String> = row.get(5)?;

            let mut duration_secs = 0;
            if let (Some(min_s), Some(max_s)) = (min_ts, max_ts) {
                if let (Ok(t1), Ok(t2)) = (
                    chrono::NaiveDateTime::parse_from_str(&min_s, "%Y-%m-%d %H:%M:%S"),
                    chrono::NaiveDateTime::parse_from_str(&max_s, "%Y-%m-%d %H:%M:%S"),
                ) {
                    let diff = t2.signed_duration_since(t1).num_seconds();
                    if diff > 0 {
                        duration_secs = diff as u64;
                    }
                }
            }

            Ok((
                day,
                DailySummary {
                    moji_count: moji,
                    lines_read: lines,
                    lines_filtered: filtered,
                    duration_secs,
                },
            ))
        })
        .unwrap();

    for r in rows.flatten() {
        stats.insert(r.0, r.1);
    }
    stats
}
