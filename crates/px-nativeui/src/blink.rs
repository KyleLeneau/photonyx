//! Blink / cull UI for reviewing observation sub-frames.
//!
//! Keyboard controls (always shown in the bottom bar):
//!   Left / Right   Navigate frames (also pauses playback)
//!   Space          Toggle autoplay
//!   X              Toggle reject on current frame
//!   Q              Quit and save

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use eframe::egui;
use egui_phosphor::regular as ph;
use px_fits::display::{MAX_DISPLAY_DIM, PreviewImage, decode_preview};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// How many frames ahead to decode and upload while playing.
const PRELOAD_AHEAD: usize = 4;
/// How many frames behind to keep decoded (for backward scrubbing).
const PRELOAD_BEHIND: usize = 2;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

struct FrameEntry {
    path: PathBuf,
    filename: String,
    rejected: bool,
    /// Populated by a background thread once decoding completes.
    decoded: Arc<Mutex<Option<Result<PreviewImage, String>>>>,
}

pub struct BlinkApp {
    frames: Vec<FrameEntry>,
    current: usize,
    playing: bool,
    play_interval: Duration,
    last_advance: Instant,
    folder: PathBuf,
    /// GPU textures keyed by frame index, created lazily on the render thread.
    textures: HashMap<usize, egui::TextureHandle>,
    /// Track which indices have had decode threads spawned.
    decode_requested: HashSet<usize>,
    delegate: Box<dyn BlinkAppDelegate>,
}

// ---------------------------------------------------------------------------
// Interaction
// ---------------------------------------------------------------------------

pub trait BlinkAppDelegate {
    /// Called once when the app starts, to restore any prior session state.
    /// Returns the set of filenames that were previously rejected.
    ///
    fn load_state(&self, folder: &Path) -> HashSet<String>;

    /// Called when the user quits (Q key or window close).
    ///
    fn save_state(&mut self, folder: &Path, rejected: Vec<String>, total: usize);

    /// Void callback when a frame is rejected
    ///
    fn frame_rejected_toggle(&mut self, path: String, rejected: bool);
}

// ---------------------------------------------------------------------------
// Construction
// ---------------------------------------------------------------------------

impl BlinkApp {
    pub fn new(
        paths: Vec<PathBuf>,
        folder: PathBuf,
        interval_secs: f64,
        delegate: Box<dyn BlinkAppDelegate>,
    ) -> Self {
        // Load any existing rejection state from a prior session.
        let prior_rejected = delegate.load_state(&folder);

        let frames = paths
            .into_iter()
            .map(|path| {
                let filename = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                let rejected = prior_rejected.contains(&filename);
                FrameEntry {
                    path,
                    filename,
                    rejected,
                    decoded: Arc::new(Mutex::new(None)),
                }
            })
            .collect();

        Self {
            frames,
            current: 0,
            playing: false,
            play_interval: Duration::from_secs_f64(interval_secs),
            last_advance: Instant::now(),
            folder,
            textures: HashMap::new(),
            decode_requested: HashSet::new(),
            delegate,
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

impl BlinkApp {
    fn frame_count(&self) -> usize {
        self.frames.len()
    }

    fn go_next(&mut self) {
        self.current = (self.current + 1) % self.frame_count();
    }

    fn go_prev(&mut self) {
        let n = self.frame_count();
        self.current = if self.current == 0 {
            n - 1
        } else {
            self.current - 1
        };
    }

    /// Spawn a background thread to decode `idx` if not already requested.
    fn request_decode(&mut self, idx: usize, ctx: &egui::Context) {
        if idx >= self.frames.len() || self.decode_requested.contains(&idx) {
            return;
        }
        self.decode_requested.insert(idx);

        let decoded = Arc::clone(&self.frames[idx].decoded);
        let path = self.frames[idx].path.clone();
        let ctx = ctx.clone(); // egui::Context is cheap to clone (Arc inside)

        std::thread::spawn(move || {
            let result = decode_preview(&path).map_err(|e| e.to_string());
            *decoded.lock().unwrap() = Some(result);
            ctx.request_repaint(); // wake egui so it uploads the texture
        });
    }

    /// If decoded pixels are ready for `idx` but no texture exists yet, upload it.
    fn ensure_texture(&mut self, idx: usize, ctx: &egui::Context) {
        if self.textures.contains_key(&idx) {
            return;
        }
        let guard = self.frames[idx].decoded.lock().unwrap();
        if let Some(Ok(img)) = guard.as_ref() {
            let color_image = egui::ColorImage::from_rgb([img.width, img.height], &img.pixels);
            let handle = ctx.load_texture(
                format!("frame_{idx}"),
                color_image,
                egui::TextureOptions::default(),
            );
            drop(guard);
            self.textures.insert(idx, handle);
        }
    }

    fn save_state(&mut self) {
        let rejected: Vec<String> = self
            .frames
            .iter()
            .filter(|f| f.rejected)
            .map(|f| f.filename.clone())
            .collect();

        self.delegate
            .save_state(&self.folder, rejected, self.frames.len());
    }
}

// ---------------------------------------------------------------------------
// eframe::App
// ---------------------------------------------------------------------------

impl eframe::App for BlinkApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let n = self.frame_count();
        if n == 0 {
            return;
        }

        // Kick off decode and eagerly upload textures for the preload window.
        // Ahead frames are prioritised so playback never waits on a decode.
        for offset in 0..=PRELOAD_AHEAD {
            let idx = (self.current + offset) % n;
            self.request_decode(idx, ctx);
            self.ensure_texture(idx, ctx);
        }
        for offset in 1..=PRELOAD_BEHIND {
            let idx = (self.current + n - offset) % n;
            self.request_decode(idx, ctx);
            self.ensure_texture(idx, ctx);
        }

        // --- Input handling ---
        let mut navigate_prev = false;
        let mut navigate_next = false;
        let mut toggle_play = false;
        let mut toggle_reject = false;
        let mut quit = false;

        ctx.input(|i| {
            navigate_prev = i.key_pressed(egui::Key::ArrowLeft);
            navigate_next = i.key_pressed(egui::Key::ArrowRight);
            toggle_play = i.key_pressed(egui::Key::Space);
            toggle_reject = i.key_pressed(egui::Key::X);
            quit = i.key_pressed(egui::Key::Q);
        });

        if navigate_prev {
            self.playing = false;
            self.go_prev();
        }
        if navigate_next {
            self.playing = false;
            self.go_next();
        }
        if toggle_play {
            self.playing = !self.playing;
            if self.playing {
                self.last_advance = Instant::now();
            }
        }
        if toggle_reject {
            self.frames[self.current].rejected = !self.frames[self.current].rejected;
            self.delegate.frame_rejected_toggle(
                self.frames[self.current].filename.clone(),
                self.frames[self.current].rejected,
            );
        }
        if quit {
            self.save_state();
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        // --- Autoplay ---
        if self.playing {
            let now = Instant::now();
            if now.duration_since(self.last_advance) >= self.play_interval {
                self.go_next();
                self.last_advance = now;
            }
            ctx.request_repaint_after(self.play_interval / 4);
        }

        // Cache values for rendering (avoids borrow-checker conflicts below).
        let current = self.current;
        let rejected = self.frames[current].rejected;
        let filename = self.frames[current].filename.clone();
        let playing = self.playing;
        let play_interval = self.play_interval;

        // --- Top bar ---
        egui::TopBottomPanel::top("top_bar")
            .frame(egui::Frame::default().inner_margin(egui::Margin::symmetric(8.0, 6.0)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("{} / {}", current + 1, n))
                            .strong()
                            .size(14.0),
                    );
                    ui.separator();
                    ui.label(&filename);

                    if rejected {
                        ui.separator();
                        ui.label(
                            egui::RichText::new(format!("{} REJECTED", ph::X_CIRCLE))
                                .color(egui::Color32::from_rgb(220, 60, 60))
                                .strong(),
                        );
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if playing {
                            ui.label(
                                egui::RichText::new(format!("{} Playing", ph::PLAY))
                                    .color(egui::Color32::GREEN),
                            );
                        } else {
                            ui.label(
                                egui::RichText::new(format!("{} Paused", ph::PAUSE))
                                    .color(egui::Color32::GRAY),
                            );
                        }

                        ui.separator();

                        const INTERVALS: &[(f64, &str)] = &[
                            (0.25, "0.25 s"),
                            (0.5, "0.5 s"),
                            (1.0, "1.0 s"),
                            (1.5, "1.5 s"),
                            (2.0, "2.0 s"),
                            (3.0, "3.0 s"),
                        ];
                        let current_label = INTERVALS
                            .iter()
                            .find(|(s, _)| Duration::from_secs_f64(*s) == play_interval)
                            .map(|(_, l)| *l)
                            .unwrap_or("Custom");
                        egui::ComboBox::from_id_salt("interval_select")
                            .selected_text(current_label)
                            .width(70.0)
                            .show_ui(ui, |ui| {
                                for (secs, label) in INTERVALS {
                                    let d = Duration::from_secs_f64(*secs);
                                    if ui
                                        .selectable_label(play_interval == d, *label)
                                        .clicked()
                                    {
                                        self.play_interval = d;
                                    }
                                }
                            });
                        ui.label(egui::RichText::new("Interval:").color(egui::Color32::GRAY));
                    });
                });
            });

        // --- Bottom bar (keyboard reference) ---
        egui::TopBottomPanel::bottom("bottom_bar")
            .frame(egui::Frame::default().inner_margin(egui::Margin::symmetric(8.0, 4.0)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("{} {}", ph::ARROW_LEFT, ph::ARROW_RIGHT))
                            .strong(),
                    );
                    ui.label("Navigate");
                    ui.separator();
                    ui.label(egui::RichText::new("Space").strong());
                    ui.label("Play / Pause");
                    ui.separator();
                    ui.label(egui::RichText::new("X").strong());
                    ui.label("Reject / Unreject");
                    ui.separator();
                    ui.label(egui::RichText::new("Q").strong());
                    ui.label("Quit & Save");
                });
            });

        // --- Central image panel ---
        egui::CentralPanel::default()
            .frame(egui::Frame::default().fill(egui::Color32::BLACK))
            .show(ctx, |ui| {
                let available = ui.available_size();

                if let Some(texture) = self.textures.get(&current) {
                    let img_size = texture.size_vec2();
                    let scale = (available.x / img_size.x).min(available.y / img_size.y);
                    let display_size = img_size * scale;

                    let offset = (available - display_size) * 0.5;
                    let img_rect =
                        egui::Rect::from_min_size(ui.min_rect().min + offset, display_size);

                    ui.painter().image(
                        texture.id(),
                        img_rect,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        egui::Color32::WHITE,
                    );

                    if rejected {
                        ui.painter().rect_stroke(
                            img_rect,
                            egui::Rounding::ZERO,
                            egui::Stroke::new(
                                4.0,
                                egui::Color32::from_rgba_unmultiplied(220, 30, 30, 200),
                            ),
                        );
                        ui.painter().text(
                            img_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            format!("{}  REJECTED", ph::X_CIRCLE),
                            egui::FontId::proportional(52.0),
                            egui::Color32::from_rgba_unmultiplied(220, 40, 40, 220),
                        );
                    }
                } else {
                    ui.centered_and_justified(|ui| {
                        ui.label(
                            egui::RichText::new("Loading…")
                                .color(egui::Color32::GRAY)
                                .size(18.0),
                        );
                    });
                }
            });
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.save_state();
    }
}

// ---------------------------------------------------------------------------
// Launcher
// ---------------------------------------------------------------------------

/// Launch the blink UI, blocking until the window is closed.
/// Must be called from the main thread (Cocoa / Wayland requirement).
pub fn launch(
    paths: Vec<PathBuf>,
    folder: PathBuf,
    interval_secs: f64,
    delegate: Box<dyn BlinkAppDelegate>,
) -> anyhow::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("px — Blink Preview")
            .with_inner_size([MAX_DISPLAY_DIM as f32, MAX_DISPLAY_DIM as f32 * 0.75]),
        ..Default::default()
    };

    eframe::run_native(
        "px blink",
        options,
        Box::new(move |cc| {
            // Install Phosphor icon font as a fallback alongside the default fonts.
            let mut fonts = egui::FontDefinitions::default();
            egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
            cc.egui_ctx.set_fonts(fonts);

            Ok(Box::new(BlinkApp::new(
                paths,
                folder,
                interval_secs,
                delegate,
            )))
        }),
    )
    .map_err(|e| anyhow::anyhow!("UI error: {e}"))?;

    Ok(())
}
