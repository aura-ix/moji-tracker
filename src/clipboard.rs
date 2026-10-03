use std::sync::mpsc::Sender;
use std::thread;
use std::time::Duration;

pub fn spawn_clipboard_watcher(tx: Sender<String>, ctx: egui::Context) {
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
