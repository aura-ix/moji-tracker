use std::collections::{HashMap, VecDeque};
use std::time::Instant;

use chrono::Local;
use rusqlite::{params, Connection};

use crate::db::{init_db, load_daily_stats, DailySummary};
use crate::japanese::{clean_for_dedup, contains_japanese, count_japanese_chars};

pub struct TrackerCore {
    pub start_time: Instant,
    pub total_moji: usize,
    pub lines_read: usize,
    pub lines_filtered: usize,
    pub window_size: usize,
    pub recent_window: VecDeque<String>,
    pub daily_stats: HashMap<String, DailySummary>,
    pub db: Connection,
}

impl TrackerCore {
    pub fn new(window_size: usize) -> Self {
        let db = init_db();
        let daily_stats = load_daily_stats(&db);

        // Prefill dedup window from DB
        let mut recent_window = VecDeque::with_capacity(window_size + 1);
        {
            let mut stmt = db
                .prepare("SELECT text FROM lines WHERE is_duplicate = 0 ORDER BY id DESC LIMIT ?1")
                .unwrap();
            let rows = stmt
                .query_map(params![window_size as i64], |row| row.get::<_, String>(0))
                .unwrap();
            for text in rows.flatten() {
                recent_window.push_front(clean_for_dedup(&text));
            }
        }

        Self {
            start_time: Instant::now(),
            total_moji: 0,
            lines_read: 0,
            lines_filtered: 0,
            window_size,
            recent_window,
            daily_stats,
            db,
        }
    }

    pub fn process_text(&mut self, text: &str) {
        if text.trim().is_empty() {
            return;
        }

        for raw_line in text.lines() {
            let line = raw_line.trim();
            if line.is_empty() || !contains_japanese(line) {
                continue;
            }

            let dedup_key = clean_for_dedup(line);
            let is_duplicate = self.window_size > 0 && self.recent_window.contains(&dedup_key);

            let now = Local::now();
            let full_ts = now.format("%Y-%m-%d %H:%M:%S").to_string();
            let day_key = now.format("%Y-%m-%d").to_string();

            if is_duplicate {
                self.lines_filtered += 1;

                self.db
                    .execute(
                        "INSERT INTO lines (timestamp, text, moji_count, is_duplicate) VALUES (?1, ?2, ?3, 1)",
                        params![full_ts, line, 0],
                    )
                    .ok();

                let entry = self.daily_stats.entry(day_key).or_default();
                entry.lines_filtered += 1;
            } else {
                if self.window_size > 0 {
                    if self.recent_window.len() >= self.window_size {
                        self.recent_window.pop_front();
                    }
                    self.recent_window.push_back(dedup_key);
                }

                let moji = count_japanese_chars(line);
                self.total_moji += moji;
                self.lines_read += 1;

                self.db
                    .execute(
                        "INSERT INTO lines (timestamp, text, moji_count, is_duplicate) VALUES (?1, ?2, ?3, 0)",
                        params![full_ts, line, moji],
                    )
                    .ok();

                let entry = self.daily_stats.entry(day_key).or_default();
                entry.moji_count += moji as u32;
                entry.lines_read += 1;
            }
        }
    }
}
