use std::collections::VecDeque;
use std::io;
use std::sync::{Arc, Mutex};

use eframe::egui;
use tracing_subscriber::fmt::MakeWriter;

/// Oldest lines fall off past this, so a long show cannot grow the window's memory.
const MAX_LINES: usize = 2_000;

/// Formatted log lines, shared between the tracing subscriber and the window.
#[derive(Clone, Default)]
pub struct LogBuffer {
    inner: Arc<Mutex<Inner>>,
}

#[derive(Default)]
struct Inner {
    lines: VecDeque<String>,
    partial: String,
    repaint: Option<egui::Context>,
}

impl LogBuffer {
    /// Wakes the window whenever a line lands.
    pub fn attach(&self, ctx: egui::Context) {
        self.inner.lock().expect("log lock").repaint = Some(ctx);
    }

    pub fn with_lines<R>(&self, read: impl FnOnce(&VecDeque<String>) -> R) -> R {
        read(&self.inner.lock().expect("log lock").lines)
    }

    pub fn clear(&self) {
        self.inner.lock().expect("log lock").lines.clear();
    }

    fn append(&self, bytes: &[u8]) {
        let mut inner = self.inner.lock().expect("log lock");
        inner.partial.push_str(&String::from_utf8_lossy(bytes));
        while let Some(end) = inner.partial.find('\n') {
            let line: String = inner.partial.drain(..=end).collect();
            inner.lines.push_back(line.trim_end().to_string());
            if inner.lines.len() > MAX_LINES {
                inner.lines.pop_front();
            }
        }
        if let Some(ctx) = &inner.repaint {
            ctx.request_repaint();
        }
    }
}

pub struct LineWriter(LogBuffer);

impl io::Write for LineWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.append(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for LogBuffer {
    type Writer = LineWriter;

    fn make_writer(&'a self) -> Self::Writer {
        LineWriter(self.clone())
    }
}
