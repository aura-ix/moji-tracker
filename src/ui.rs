use std::sync::mpsc::Receiver;
use std::time::Duration;

use chrono::{Datelike, Local};
use egui::{Color32, RichText, Sense, Vec2};

use crate::tracker::TrackerCore;

const NUM_WEEKS: usize = 26;
const CELL_SIZE: f32 = 12.0;
const CELL_GAP: f32 = 3.0;
const STEP: f32 = CELL_SIZE + CELL_GAP;
const GRID_HEIGHT: f32 = 7.0 * CELL_SIZE + 6.0 * CELL_GAP;
const GRID_WIDTH: f32 = (NUM_WEEKS - 1) as f32 * STEP + CELL_SIZE;

fn format_dimmed_counter_job(
    value: u32,
    total_digits: usize,
    unit: &str,
    active_color: Color32,
    dim_color: Color32,
    font_id: egui::FontId,
) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    let val_str = value.to_string();

    if val_str.len() < total_digits {
        let num_zeros = total_digits - val_str.len();
        let zeros = "0".repeat(num_zeros);
        job.append(
            &zeros,
            0.0,
            egui::TextFormat {
                font_id: font_id.clone(),
                color: dim_color,
                ..Default::default()
            },
        );
    }

    job.append(
        &val_str,
        0.0,
        egui::TextFormat {
            font_id: font_id.clone(),
            color: active_color,
            ..Default::default()
        },
    );

    job.append(
        unit,
        0.0,
        egui::TextFormat {
            font_id,
            color: active_color,
            ..Default::default()
        },
    );

    job
}

fn format_dimmed_duration_job(
    elapsed: Duration,
    active_color: Color32,
    dim_color: Color32,
    font_id: egui::FontId,
) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();

    let total_secs = elapsed.as_secs();
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let seconds = total_secs % 60;

    let full = format!("{:02}:{:02}:{:02}", hours, minutes, seconds);
    let split_idx = full.find(|c: char| c != '0' && c != ':').unwrap_or(full.len() - 1);
    let (dim_part, bright_part) = full.split_at(split_idx);

    if !dim_part.is_empty() {
        job.append(
            dim_part,
            0.0,
            egui::TextFormat {
                font_id: font_id.clone(),
                color: dim_color,
                ..Default::default()
            },
        );
    }

    job.append(
        bright_part,
        0.0,
        egui::TextFormat {
            font_id,
            color: active_color,
            ..Default::default()
        },
    );

    job
}

pub fn configure_cjk_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    fonts.font_data.insert(
        "mplus_1p".to_owned(),
        egui::FontData::from_static(include_bytes!("../assets/mplus-1p-regular.ttf")),
    );

    if let Some(family) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
        family.insert(0, "mplus_1p".to_owned());
    }
    if let Some(family) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
        family.insert(0, "mplus_1p".to_owned());
    }

    ctx.set_fonts(fonts);
}

pub struct MojiTrackerApp {
    pub tracker: TrackerCore,
    pub clipboard_rx: Receiver<String>,
    pub selected_date: Option<chrono::NaiveDate>,
}

impl MojiTrackerApp {
    pub fn new(cc: &eframe::CreationContext<'_>, clipboard_rx: Receiver<String>) -> Self {
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
            selected_date: None,
        }
    }

    fn render_activity_grid(&mut self, ui: &mut egui::Ui) -> Option<chrono::NaiveDate> {
        let today = Local::now().date_naive();
        let days_from_monday = today.weekday().num_days_from_monday() as i64;
        let current_week_monday = today - chrono::Duration::days(days_from_monday);
        let start_date = current_week_monday - chrono::Duration::weeks(NUM_WEEKS as i64 - 1);

        let max_moji = self
            .tracker
            .daily_stats
            .values()
            .map(|s| s.moji_count)
            .max()
            .unwrap_or(0);

        let mut hovered_date = None;

        egui::ScrollArea::horizontal()
            .id_salt("activity_grid_scroll")
            .auto_shrink([false, true])
            .max_height(GRID_HEIGHT)
            .show(ui, |ui| {
                let (rect, response) = ui.allocate_exact_size(
                    Vec2::new(GRID_WIDTH, GRID_HEIGHT),
                    Sense::click(),
                );

                // Prevent flickering while moving mouse between deadzones in cells
                if let Some(pos) = response.hover_pos() {
                    let rel_x = pos.x - rect.min.x;
                    let rel_y = pos.y - rect.min.y;

                    if rel_x >= 0.0
                        && rel_x < GRID_WIDTH
                        && rel_y >= 0.0
                        && rel_y < GRID_HEIGHT
                    {
                        let col = (rel_x / STEP).floor().clamp(0.0, (NUM_WEEKS - 1) as f32) as i64;
                        let row = (rel_y / STEP).floor().clamp(0.0, 6.0) as i64;
                        let date = start_date + chrono::Duration::days(col * 7 + row);

                        if date <= today {
                            hovered_date = Some(date);
                            if response.clicked() {
                                if self.selected_date == Some(date) || date == today {
                                    self.selected_date = None;
                                } else {
                                    self.selected_date = Some(date);
                                }
                            }
                        }
                    }
                }

                let painter = ui.painter();

                // Activity Cells
                let active_selected = self.selected_date.unwrap_or(today);
                for week_idx in 0..NUM_WEEKS {
                    for day_idx in 0..7 {
                        let date = start_date + chrono::Duration::days((week_idx * 7 + day_idx) as i64);
                        if date > today {
                            continue;
                        }

                        let x = rect.min.x + week_idx as f32 * STEP;
                        let y = rect.min.y + day_idx as f32 * STEP;
                        let cell_rect =
                            egui::Rect::from_min_size(egui::pos2(x, y), Vec2::splat(CELL_SIZE));

                        let date_str = date.format("%Y-%m-%d").to_string();
                        let moji_count = self
                            .tracker
                            .daily_stats
                            .get(&date_str)
                            .map(|s| s.moji_count)
                            .unwrap_or(0);

                        let color = if moji_count == 0 {
                            Color32::from_rgb(32, 32, 36)
                        } else if max_moji == 0 {
                            Color32::from_rgb(14, 75, 35)
                        } else {
                            let ratio = moji_count as f32 / max_moji as f32;
                            if ratio <= 0.25 {
                                Color32::from_rgb(14, 75, 35)
                            } else if ratio <= 0.50 {
                                Color32::from_rgb(26, 125, 55)
                            } else if ratio <= 0.75 {
                                Color32::from_rgb(45, 185, 80)
                            } else {
                                Color32::from_rgb(90, 240, 135)
                            }
                        };

                        painter.rect_filled(cell_rect, 2.0, color);

                        // Selection and hover highlights
                        if active_selected == date {
                            painter.rect_stroke(
                                cell_rect,
                                2.0,
                                egui::Stroke::new(1.5_f32, Color32::WHITE),
                            );
                        } else if hovered_date == Some(date) {
                            painter.rect_stroke(
                                cell_rect,
                                2.0,
                                egui::Stroke::new(1.0_f32, Color32::from_gray(170)),
                            );
                        }
                    }
                }
            });

        hovered_date
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
            // Activity heatmap
            let available_width = ui.available_width();
            let grid_width = GRID_WIDTH;
            let hovered_date = ui
                .horizontal(|ui| {
                    if available_width > grid_width {
                        ui.add_space((available_width - grid_width) / 2.0);
                    }
                    self.render_activity_grid(ui)
                })
                .inner;

            ui.add_space(4.0);
            ui.separator();
            ui.add_space(4.0);

            // Stats line
            let today = Local::now().date_naive();
            let active_date = hovered_date.or(self.selected_date).unwrap_or(today);
            let is_today = active_date == today;
            let date_str = active_date.format("%Y-%m-%d").to_string();

            let summary = self
                .tracker
                .daily_stats
                .get(&date_str)
                .copied()
                .unwrap_or_default();

            let weekday_kanji = match active_date.weekday() {
                chrono::Weekday::Mon => "月",
                chrono::Weekday::Tue => "火",
                chrono::Weekday::Wed => "水",
                chrono::Weekday::Thu => "木",
                chrono::Weekday::Fri => "金",
                chrono::Weekday::Sat => "土",
                chrono::Weekday::Sun => "日",
            };
            let date_disp = format!(
                "{}.{:02}.{:02} {}",
                active_date.year(),
                active_date.month(),
                active_date.day(),
                weekday_kanji
            );

            let elapsed = if is_today {
                self.tracker.start_time.elapsed()
            } else {
                Duration::from_secs(summary.duration_secs)
            };

            let show_today_btn = self.selected_date.is_some() && self.selected_date != Some(today);
            let col_gap = 8.0;

            let font_id = egui::TextStyle::Body.resolve(ui.style());

            let dur_job = format_dimmed_duration_job(
                elapsed,
                Color32::from_rgb(110, 195, 245),
                Color32::from_rgb(25, 60, 80),
                font_id.clone(),
            );

            let moji_job = format_dimmed_counter_job(
                summary.moji_count,
                6,
                "文字",
                Color32::from_rgb(85, 230, 140),
                Color32::from_rgb(20, 60, 35),
                font_id.clone(),
            );

            let lines_job = format_dimmed_counter_job(
                summary.lines_read,
                6,
                "行",
                Color32::from_rgb(195, 155, 245),
                Color32::from_rgb(55, 38, 75),
                font_id,
            );

            ui.horizontal(|ui| {
                if available_width > grid_width {
                    ui.add_space((available_width - grid_width) / 2.0);
                }

                ui.allocate_ui_with_layout(
                    Vec2::new(grid_width, 22.0),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        ui.label(
                            RichText::new(&date_disp)
                                .color(Color32::from_rgb(215, 222, 235))
                                .strong(),
                        );
                        if show_today_btn {
                            if ui.small_button("⏭").on_hover_text("Jump to today").clicked() {
                                self.selected_date = None;
                            }
                        }

                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                ui.label(moji_job);
                                ui.add_space(col_gap);
                                ui.label(lines_job);
                                ui.add_space(col_gap);
                                ui.label(dur_job);
                            },
                        );
                    },
                );
            });

            ui.add_space(6.0);
        });
    }
}
