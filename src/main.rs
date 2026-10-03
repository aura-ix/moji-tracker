use std::collections::VecDeque;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use chrono::Local;
use egui::{Color32, RichText};

#[inline]
fn is_japanese_char(c: char) -> bool {
    matches!(
        c,
        '\u{3040}'..='\u{309F}'   // Hiragana
        | '\u{30A0}'..='\u{30FF}' // Katakana
        | '\u{31F0}'..='\u{31FF}' // Katakana Phonetic Extensions
        | '\u{FF65}'..='\u{FF9F}' // Halfwidth Katakana
        | '\u{4E00}'..='\u{9FAF}' // CJK Unified Ideographs
        | '\u{3400}'..='\u{4DBF}' // CJK Extension A
    )
}

fn contains_japanese(text: &str) -> bool {
    text.chars().any(is_japanese_char)
}

fn count_japanese_chars(text: &str) -> usize {
    text.chars().filter(|&c| is_japanese_char(c)).count()
}

fn clean_for_dedup(text: &str) -> String {
    text.chars().filter(|c| !c.is_whitespace()).collect()
}

fn format_duration(elapsed: Duration) -> String {
    let total_secs = elapsed.as_secs();
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let seconds = total_secs % 60;

    if hours > 0 {
        format!("{:02}h {:02}m {:02}s", hours, minutes, seconds)
    } else {
        format!("{:02}m {:02}s", minutes, seconds)
    }
}

#[derive(Debug, Clone)]
struct ProcessedLine {
    timestamp: String,
    text: String,
    moji_count: usize,
    is_duplicate: bool,
}

struct TrackerCore {
    start_time: Instant,
    total_moji: usize,
    lines_read: usize,
    lines_filtered: usize,
    window_size: usize,
    recent_window: VecDeque<String>,
    recent_lines: VecDeque<ProcessedLine>,
}

impl TrackerCore {
    fn new(window_size: usize) -> Self {
        Self {
            start_time: Instant::now(),
            total_moji: 0,
            lines_read: 0,
            lines_filtered: 0,
            window_size,
            recent_window: VecDeque::with_capacity(window_size + 1),
            recent_lines: VecDeque::with_capacity(6),
        }
    }

    fn process_text(&mut self, text: &str) {
        if text.trim().is_empty() {
            return;
        }

        for raw_line in text.lines() {
            let line = raw_line.trim();
            if line.is_empty() || !contains_japanese(line) {
                // Ignore non-jp lines completely
                continue;
            }

            let dedup_key = clean_for_dedup(line);
            let is_duplicate = self.window_size > 0 && self.recent_window.contains(&dedup_key);
            let timestamp = Local::now().format("%H:%M:%S").to_string();

            if is_duplicate {
                self.lines_filtered += 1;
                self.record_recent_line(ProcessedLine {
                    timestamp,
                    text: line.to_string(),
                    moji_count: 0,
                    is_duplicate: true,
                });
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

                self.record_recent_line(ProcessedLine {
                    timestamp,
                    text: line.to_string(),
                    moji_count: moji,
                    is_duplicate: false,
                });
            }
        }
    }

    fn record_recent_line(&mut self, line: ProcessedLine) {
        self.recent_lines.push_front(line);
        if self.recent_lines.len() > 5 {
            self.recent_lines.pop_back();
        }
    }
}

fn spawn_clipboard_watcher(tx: Sender<String>, ctx: egui::Context) {
    thread::spawn(move || {
        let mut last_text = String::new();
        let mut clipboard_opt = arboard::Clipboard::new().ok();

        if let Some(ref mut cb) = clipboard_opt {
            if let Ok(initial) = cb.get_text() {
                last_text = initial;
            }
        }

        loop {
            if clipboard_opt.is_none() {
                clipboard_opt = arboard::Clipboard::new().ok();
            }

            if let Some(ref mut cb) = clipboard_opt {
                if let Ok(current) = cb.get_text() {
                    if !current.is_empty() && current != last_text {
                        last_text = current.clone();
                        if tx.send(current).is_ok() {
                            ctx.request_repaint();
                        }
                    }
                }
            }

            thread::sleep(Duration::from_millis(200));
        }
    });
}

fn configure_cjk_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    fonts.font_data.insert(
        "mplus_1p".to_owned(),
        egui::FontData::from_static(include_bytes!("../assets/mplus-1p-regular.ttf")),
    );

    if let Some(family) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
        family.push("mplus_1p".to_owned());
    }
    if let Some(family) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
        family.push("mplus_1p".to_owned());
    }

    ctx.set_fonts(fonts);
}

struct MojiTrackerApp {
    tracker: TrackerCore,
    clipboard_rx: Receiver<String>,
}

impl MojiTrackerApp {
    fn new(cc: &eframe::CreationContext<'_>, clipboard_rx: Receiver<String>) -> Self {
        configure_cjk_fonts(&cc.egui_ctx);

        let mut style = (*cc.egui_ctx.style()).clone();
        style.spacing.scroll.floating = false;
        style.spacing.scroll.bar_width *= 0.6;
        style.spacing.scroll.dormant_handle_opacity = 0.6;
        style.spacing.scroll.dormant_background_opacity = 0.15;
        cc.egui_ctx.set_style(style);

        Self {
            tracker: TrackerCore::new(15),
            clipboard_rx,
        }
    }
}

impl eframe::App for MojiTrackerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        while let Ok(text) = self.clipboard_rx.try_recv() {
            self.tracker.process_text(&text);
        }

        // Live refresh for session duration timer
        ctx.request_repaint_after(Duration::from_millis(500));

        egui::CentralPanel::default().show(ctx, |ui| {
            let duration_str = format_duration(self.tracker.start_time.elapsed());
            egui::Grid::new("stats_grid")
                .num_columns(2)
                .spacing([20.0, 6.0])
                .show(ui, |ui| {
                    ui.label("session duration:");
                    ui.monospace(duration_str);
                    ui.end_row();

                    ui.label("moji read:");
                    ui.label(
                        RichText::new(self.tracker.total_moji.to_string())
                            .color(Color32::from_rgb(100, 220, 120))
                            .strong()
                            .monospace(),
                    );
                    ui.end_row();

                    ui.label("lines read:");
                    ui.monospace(self.tracker.lines_read.to_string());
                    ui.end_row();

                    ui.label("lines filtered:");
                    ui.label(
                        RichText::new(self.tracker.lines_filtered.to_string())
                            .color(if self.tracker.lines_filtered > 0 {
                                Color32::from_rgb(255, 170, 70)
                            } else {
                                Color32::GRAY
                            })
                            .monospace(),
                    );
                    ui.end_row();
                });

            ui.separator();

            // Recent Lines
            ui.label(RichText::new("Recent Lines").strong());
            ui.add_space(4.0);

            egui::ScrollArea::vertical()
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                if self.tracker.recent_lines.is_empty() {
                    ui.weak("Waiting for Japanese clipboard text...");
                } else {
                    for item in &self.tracker.recent_lines {
                        ui.horizontal(|ui| {
                            ui.weak(format!("[{}]", item.timestamp));

                            if item.is_duplicate {
                                ui.label(
                                    RichText::new(" DUP ")
                                        .color(Color32::from_rgb(255, 180, 50))
                                        .size(11.0)
                                        .strong(),
                                );
                                ui.add(egui::Label::new(RichText::new(&item.text).weak().strikethrough()).wrap());
                            } else {
                                ui.label(
                                    RichText::new(format!(" +{} ", item.moji_count))
                                        .color(Color32::from_rgb(80, 230, 130))
                                        .size(11.0)
                                        .strong(),
                                );
                                ui.add(egui::Label::new(&item.text).wrap());
                            }
                        });
                    }
                }
            });
        });
    }
}

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([400.0, 260.0])
            .with_min_inner_size([320.0, 200.0])
            .with_title("Moji Tracker"),
        ..Default::default()
    };

    eframe::run_native(
        "Moji Tracker",
        native_options,
        Box::new(|cc| {
            let (tx, rx) = mpsc::channel();
            spawn_clipboard_watcher(tx, cc.egui_ctx.clone());
            Ok(Box::new(MojiTrackerApp::new(cc, rx)))
        }),
    )
}
