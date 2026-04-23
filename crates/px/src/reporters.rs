use std::{
    cell::RefCell,
    collections::HashMap,
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use px_pipeline::PipelineReporter;

use crate::printer::Printer;

pub(crate) struct DefaultPipelineReporter {
    bar: MultiProgress,
    spinners: RefCell<HashMap<usize, ProgressBar>>,
    next_id: AtomicUsize,
}

impl From<Printer> for DefaultPipelineReporter {
    fn from(printer: Printer) -> Self {
        Self {
            bar: MultiProgress::with_draw_target(printer.target()),
            spinners: RefCell::new(HashMap::new()),
            next_id: AtomicUsize::new(0),
        }
    }
}

impl DefaultPipelineReporter {
    fn spinner(&self, msg: impl Into<String>) -> ProgressBar {
        let pb = self.bar.add(ProgressBar::new_spinner());
        pb.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.cyan} {msg}")
                .unwrap()
                .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
        );
        pb.set_message(msg.into());
        pb.enable_steady_tick(Duration::from_millis(80));
        pb
    }
}

impl PipelineReporter for DefaultPipelineReporter {
    fn step_started(&self, message: &str) -> usize {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let pb = self.spinner(message);
        self.spinners.borrow_mut().insert(id, pb);
        id
    }

    fn step_ended(&self, id: usize, message: &str, success: bool) {
        if let Some(pb) = self.spinners.borrow_mut().remove(&id) {
            if success {
                pb.finish_with_message(message.to_string());
            } else {
                pb.abandon_with_message(message.to_string());
            }
        }
    }
}
