//! Blink / cull UI — iced.rs implementation (Elm-style architecture).
//!
//! This is a functional equivalent of [`blink`] using iced's declarative,
//! message-driven architecture instead of egui's immediate-mode approach.
//! It exists purely for comparison — both files implement the same feature set.
//!
//! # Architecture differences from egui (`blink.rs`)
//!
//! | Concern          | egui (`blink.rs`)                  | iced (`blink_iced.rs`)                  |
//! |------------------|------------------------------------|------------------------------------------|
//! | Rendering model  | Immediate-mode: describe UI every frame | Declarative: produce a widget tree     |
//! | State mutation   | `&mut self` in `update()`          | `update(msg) -> Task` (Elm pattern)     |
//! | Background work  | `std::thread::spawn` + `Arc<Mutex>`| `Task::perform` (async future)          |
//! | Input            | `ctx.input(|i| ...)` polled inline | `keyboard::on_key_press` subscription   |
//! | Timer / autoplay | `request_repaint_after` + `Instant`| `iced::time::every` subscription        |
//! | Quit             | `ViewportCommand::Close`           | `std::process::exit` after save         |
//!
//! # Keyboard controls
//!   Left / Right   Navigate frames (also pauses playback)
//!   Space          Toggle autoplay
//!   X              Toggle reject on current frame
//!   Q              Quit and save

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::Duration;

use iced::widget::vertical_rule;
use iced::{
    Color, ContentFit, Element, Length, Subscription, Task,
    alignment::{Horizontal, Vertical},
    keyboard::{self, key::Named},
    widget::{column, container, horizontal_rule, horizontal_space, image, pick_list, row, stack, text},
};

use px_fits::display::{MAX_DISPLAY_DIM, decode_preview};

use crate::blink::BlinkAppDelegate;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const PRELOAD_AHEAD: usize = 4;
const PRELOAD_BEHIND: usize = 2;

/// Available playback intervals shown in the pick-list.
const INTERVALS: &[(&str, f64)] = &[
    ("0.25 s", 0.25),
    ("0.5 s", 0.50),
    ("1.0 s", 1.00),
    ("1.5 s", 1.50),
    ("2.0 s", 2.00),
    ("3.0 s", 3.00),
];

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

struct FrameEntry {
    path: PathBuf,
    filename: String,
    rejected: bool,
}

/// All messages that can drive a state transition (Elm "Msg").
#[derive(Debug, Clone)]
enum Message {
    Prev,
    Next,
    TogglePlay,
    ToggleReject,
    Quit,
    CloseRequested,
    /// Emitted every `play_interval` when playing.
    Tick,
    /// A background decode thread finished for frame `idx`.
    FrameDecoded {
        idx: usize,
        /// On success: (width, height, RGBA bytes).
        result: Result<(u32, u32, Vec<u8>), String>,
    },
    IntervalSelected(&'static str),
}

pub struct BlinkIcedApp {
    frames: Vec<FrameEntry>,
    current: usize,
    playing: bool,
    play_interval: Duration,
    folder: PathBuf,
    /// Decoded image handles ready for the GPU, keyed by frame index.
    image_handles: HashMap<usize, image::Handle>,
    /// Tracks which frames have had a decode task kicked off.
    decode_requested: HashSet<usize>,
    delegate: Box<dyn BlinkAppDelegate>,
}

// ---------------------------------------------------------------------------
// Construction
// ---------------------------------------------------------------------------

impl BlinkIcedApp {
    pub fn new(
        paths: Vec<PathBuf>,
        folder: PathBuf,
        interval_secs: f64,
        delegate: Box<dyn BlinkAppDelegate>,
    ) -> Self {
        let prior_rejected = delegate.load_state(&folder);

        let frames = paths
            .into_iter()
            .map(|path| {
                let filename = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                let rejected = prior_rejected.contains(&filename);
                FrameEntry { path, filename, rejected }
            })
            .collect();

        Self {
            frames,
            current: 0,
            playing: false,
            play_interval: Duration::from_secs_f64(interval_secs),
            folder,
            image_handles: HashMap::new(),
            decode_requested: HashSet::new(),
            delegate,
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

impl BlinkIcedApp {
    fn frame_count(&self) -> usize {
        self.frames.len()
    }

    fn go_next(&mut self) {
        self.current = (self.current + 1) % self.frame_count();
    }

    fn go_prev(&mut self) {
        let n = self.frame_count();
        self.current = if self.current == 0 { n - 1 } else { self.current - 1 };
    }

    /// Build `Task`s to decode frames in the preload window around `current`.
    ///
    /// Each task spawns a blocking thread, decodes the FITS file, converts the
    /// RGB pixels to RGBA, and emits `Message::FrameDecoded` when done.
    /// In iced the returned `Task` replaces egui's `Arc<Mutex<Option<…>>>` +
    /// manual `ctx.request_repaint()` pattern — the runtime handles waking the
    /// UI when the future resolves.
    fn preload_tasks(&mut self) -> Task<Message> {
        let n = self.frames.len();
        if n == 0 {
            return Task::none();
        }

        let ahead = (0..=PRELOAD_AHEAD).map(|o| (self.current + o) % n);
        let behind = (1..=PRELOAD_BEHIND).map(|o| (self.current + n - o) % n);

        // Collect first so the immutable borrows on `self` end before we
        // mutate `decode_requested` in the `.map()` below.
        let to_decode: Vec<usize> = ahead
            .chain(behind)
            .filter(|&idx| {
                !self.decode_requested.contains(&idx) && !self.image_handles.contains_key(&idx)
            })
            .collect();

        let tasks: Vec<Task<Message>> = to_decode
            .into_iter()
            .map(|idx| {
                self.decode_requested.insert(idx);
                let path = self.frames[idx].path.clone();

                // Spawn a std thread for the blocking FITS decode and bridge
                // back into async via a oneshot channel.
                Task::perform(
                    async move {
                        let (tx, rx) = iced::futures::channel::oneshot::channel();
                        std::thread::spawn(move || {
                            let result = decode_preview(&path)
                                .map(|img| {
                                    // iced needs RGBA; PreviewImage gives us packed RGB.
                                    let rgba: Vec<u8> = img
                                        .pixels
                                        .chunks_exact(3)
                                        .flat_map(|rgb| [rgb[0], rgb[1], rgb[2], 255u8])
                                        .collect();
                                    (img.width as u32, img.height as u32, rgba)
                                })
                                .map_err(|e| e.to_string());
                            let _ = tx.send(result);
                        });
                        rx.await.unwrap_or_else(|_| Err("decode thread panicked".into()))
                    },
                    move |result| Message::FrameDecoded { idx, result },
                )
            })
            .collect();

        Task::batch(tasks)
    }

    fn save_state(&mut self) {
        let rejected: Vec<String> = self
            .frames
            .iter()
            .filter(|f| f.rejected)
            .map(|f| f.filename.clone())
            .collect();
        self.delegate.save_state(&self.folder, rejected, self.frames.len());
    }
}

// ---------------------------------------------------------------------------
// Elm update / view / subscription
// ---------------------------------------------------------------------------

impl BlinkIcedApp {
    /// The Elm `update` function: pure state transition + optional side-effect
    /// expressed as a `Task`.  Compare with egui's monolithic `App::update`
    /// which mixes input handling, state mutation, and rendering in one pass.
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Prev => {
                self.playing = false;
                self.go_prev();
                self.preload_tasks()
            }
            Message::Next => {
                self.playing = false;
                self.go_next();
                self.preload_tasks()
            }
            Message::TogglePlay => {
                self.playing = !self.playing;
                Task::none()
            }
            Message::ToggleReject => {
                if !self.frames.is_empty() {
                    let f = &mut self.frames[self.current];
                    f.rejected = !f.rejected;
                    self.delegate.frame_rejected_toggle(f.filename.clone(), f.rejected);
                }
                Task::none()
            }
            Message::Tick => {
                if self.playing && !self.frames.is_empty() {
                    self.go_next();
                    return self.preload_tasks();
                }
                Task::none()
            }
            Message::FrameDecoded { idx, result } => {
                if let Ok((w, h, rgba)) = result {
                    // image::Handle::from_rgba uploads raw RGBA pixels.
                    // In egui the equivalent was ctx.load_texture() inside ensure_texture().
                    self.image_handles.insert(idx, image::Handle::from_rgba(w, h, rgba));
                }
                Task::none()
            }
            Message::IntervalSelected(label) => {
                if let Some(&(_, secs)) = INTERVALS.iter().find(|&&(l, _)| l == label) {
                    self.play_interval = Duration::from_secs_f64(secs);
                }
                Task::none()
            }
            Message::Quit | Message::CloseRequested => {
                self.save_state();
                // iced 0.13 does not expose a single-call graceful exit for
                // single-window apps without a window Id; process::exit is the
                // pragmatic equivalent of egui's ViewportCommand::Close here.
                std::process::exit(0);
            }
        }
    }

    /// The Elm `view` function: pure function from state → widget tree.
    /// No mutation happens here — contrast with egui where `update()` both
    /// mutates state AND immediately draws widgets in the same call.
    fn view(&self) -> Element<'_, Message> {
        let n = self.frame_count();
        let current = self.current;
        let rejected = n > 0 && self.frames[current].rejected;
        let filename = if n > 0 { self.frames[current].filename.as_str() } else { "" };
        let playing = self.playing;
        let play_interval = self.play_interval;

        // --- Interval pick-list ---
        // iced uses pick_list (a native combo-box widget) for selection.
        // egui used ComboBox::from_id_salt with selectable_label items.
        let interval_labels: Vec<&'static str> = INTERVALS.iter().map(|&(l, _)| l).collect();
        let selected_interval = INTERVALS
            .iter()
            .find(|&&(_, s)| Duration::from_secs_f64(s) == play_interval)
            .map(|&(l, _)| l);

        // --- Top bar ---
        let rejected_badge: Element<Message> = if rejected {
            text("⊗  REJECTED")
                .color(Color::from_rgb(0.86, 0.24, 0.24))
                .size(14)
                .into()
        } else {
            horizontal_space().width(0).into()
        };

        let play_status = if playing {
            text("▶  Playing").color(Color::from_rgb(0.20, 0.78, 0.20))
        } else {
            text("⏸  Paused").color(Color::from_rgb(0.50, 0.50, 0.50))
        };

        let top_bar = container(
            row![
                text(format!("{} / {}", current + 1, n)).size(14).width(80).align_x(Horizontal::Center),
                vertical_rule(1),
                text(filename),
                rejected_badge,
                horizontal_space(),
                play_status,
                vertical_rule(1),
                text("Interval:").color(Color::from_rgb(0.5, 0.5, 0.5)),
                pick_list(interval_labels, selected_interval, Message::IntervalSelected).width(80),
            ]
            .align_y(Vertical::Center)
            .padding([6, 10])
            .spacing(10)
            .height(40),
        );

        // --- Central image area ---
        // In egui the painter drew directly onto the CentralPanel canvas.
        // Here we build a widget tree; iced handles layout and rendering.
        let image_area: Element<Message> = if let Some(handle) = self.image_handles.get(&current) {
            let img: Element<Message> = image(handle.clone())
                .width(Length::Fill)
                .height(Length::Fill)
                .content_fit(ContentFit::Contain)
                .into();

            if rejected {
                // iced::widget::stack layers widgets like a CSS z-index stack.
                // In egui the "REJECTED" text was painter().text() drawn after the image.
                let overlay = container(
                    text("⊗  REJECTED")
                        .size(52)
                        .color(Color::from_rgba(0.86, 0.16, 0.16, 0.86)),
                )
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(Horizontal::Center)
                .align_y(Vertical::Center);

                stack![img, overlay].into()
            } else {
                img
            }
        } else {
            container(text("Loading…").size(18).color(Color::from_rgb(0.5, 0.5, 0.5)))
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(Horizontal::Center)
                .align_y(Vertical::Center)
                .into()
        };

        // Wrap in a black container; apply a red border when rejected.
        let image_container = container(image_area)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_theme| container::Style {
                background: Some(iced::Background::Color(Color::BLACK)),
                border: if rejected {
                    iced::Border {
                        color: Color::from_rgba(0.86, 0.12, 0.12, 0.78),
                        width: 4.0,
                        radius: 0.0.into(),
                    }
                } else {
                    iced::Border::default()
                },
                ..Default::default()
            });

        // --- Bottom bar (keyboard reference) ---
        let bottom_bar = container(
            row![
                text("← →").font(iced::Font { weight: iced::font::Weight::Bold, ..iced::Font::DEFAULT }),
                text("Navigate"),
                vertical_rule(1),
                text("Space").font(iced::Font { weight: iced::font::Weight::Bold, ..iced::Font::DEFAULT }),
                text("Play / Pause"),
                vertical_rule(1),
                text("X").font(iced::Font { weight: iced::font::Weight::Bold, ..iced::Font::DEFAULT }),
                text("Reject / Unreject"),
                vertical_rule(1),
                text("Q").font(iced::Font { weight: iced::font::Weight::Bold, ..iced::Font::DEFAULT }),
                text("Quit & Save"),
            ]
            .padding([4, 10])
            .spacing(10)
            .height(30)
        );

        column![
            top_bar,
            horizontal_rule(1),
            image_container,
            horizontal_rule(1),
            bottom_bar,
        ]
        .into()
    }

    /// Subscriptions replace egui's polling — iced calls this after every
    /// update and activates/deactivates the declared subscriptions as needed.
    ///
    /// `keyboard::on_key_press` replaces `ctx.input(|i| i.key_pressed(...))`.
    /// `iced::time::every` replaces `ctx.request_repaint_after(interval)`.
    fn subscription(&self) -> Subscription<Message> {
        let keyboard_sub = keyboard::on_key_press(|key, _mods| match key {
            keyboard::Key::Named(Named::ArrowLeft) => Some(Message::Prev),
            keyboard::Key::Named(Named::ArrowRight) => Some(Message::Next),
            keyboard::Key::Named(Named::Space) => Some(Message::TogglePlay),
            keyboard::Key::Character(ref c) if c.as_str().eq_ignore_ascii_case("x") => {
                Some(Message::ToggleReject)
            }
            keyboard::Key::Character(ref c) if c.as_str().eq_ignore_ascii_case("q") => {
                Some(Message::Quit)
            }
            _ => None,
        });

        let close_sub = iced::event::listen_with(|event, _, _| match event {
            iced::Event::Window(iced::window::Event::CloseRequested) => {
                Some(Message::CloseRequested)
            }
            _ => None,
        });

        if self.playing {
            Subscription::batch([
                keyboard_sub,
                close_sub,
                iced::time::every(self.play_interval).map(|_| Message::Tick),
            ])
        } else {
            Subscription::batch([keyboard_sub, close_sub])
        }
    }
}

// ---------------------------------------------------------------------------
// Launcher
// ---------------------------------------------------------------------------

/// Launch the blink UI with iced, blocking until the window is closed.
/// Drop-in replacement for [`crate::blink::launch`].
///
/// # Icon
///
/// Pass an icon built from embedded bytes so the app switcher shows something
/// meaningful.  The recommended pattern at the call site:
///
/// ```ignore
/// let icon = iced::window::icon::from_file_data(
///     include_bytes!("../assets/icon.png"),
///     None, // let the image crate guess the format
/// ).ok();
///
/// blink_iced::launch(paths, folder, interval_secs, delegate, icon)?;
/// ```
///
/// Contrast with egui/eframe: there the icon is set through
/// `eframe::NativeOptions::viewport` → `ViewportBuilder::with_icon(...)`,
/// which requires the same RGBA bytes but wrapped in `Arc<IconData>`.
pub fn launch(
    paths: Vec<PathBuf>,
    folder: PathBuf,
    interval_secs: f64,
    delegate: Box<dyn BlinkAppDelegate>,
    icon: Option<iced::window::Icon>,
) -> anyhow::Result<()> {
    // The Application builder exposes individual window-setting helpers
    // (window_size, transparent, …), but for fields without a dedicated
    // helper — like `icon` — use `.window()` to pass the full
    // `window::Settings` struct in one shot.
    let window_settings = iced::window::Settings {
        size: iced::Size::new(MAX_DISPLAY_DIM as f32, MAX_DISPLAY_DIM as f32 * 0.75),
        icon,
        ..iced::window::Settings::default()
    };

    iced::application("px — Blink Preview", BlinkIcedApp::update, BlinkIcedApp::view)
        .subscription(BlinkIcedApp::subscription)
        .window(window_settings)
        .run_with(move || {
            let mut app = BlinkIcedApp::new(paths, folder, interval_secs, delegate);
            // Kick off initial preload — equivalent to the first pass through
            // egui's update() loop where request_decode() is called.
            let init_task = app.preload_tasks();
            (app, init_task)
        })
        .map_err(|e| anyhow::anyhow!("UI error: {e}"))
}
