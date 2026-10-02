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
        let repaint = {
            let mut inner = self.inner.lock().expect("log lock");
            inner.partial.push_str(&String::from_utf8_lossy(bytes));
            while let Some(end) = inner.partial.find('\n') {
                let line: String = inner.partial.drain(..=end).collect();
                inner.lines.push_back(line.trim_end().to_string());
                if inner.lines.len() > MAX_LINES {
                    inner.lines.pop_front();
                }
            }
            inner.repaint.clone()
        };
        // Outside the lock: a repaint that logs would otherwise re-enter it and hang.
        if let Some(ctx) = repaint {
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

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    fn write(buffer: &LogBuffer, text: &str) {
        buffer
            .make_writer()
            .write_all(text.as_bytes())
            .expect("write");
    }

    fn lines(buffer: &LogBuffer) -> Vec<String> {
        buffer.with_lines(|lines| lines.iter().cloned().collect())
    }

    #[test]
    fn a_line_split_across_writes_lands_once_whole() {
        let buffer = LogBuffer::default();
        write(&buffer, "INFO list");
        assert!(lines(&buffer).is_empty());
        write(&buffer, "ening on 8080\n");
        assert_eq!(lines(&buffer), ["INFO listening on 8080"]);
    }

    #[test]
    fn one_write_can_carry_several_lines() {
        let buffer = LogBuffer::default();
        write(&buffer, "one\r\ntwo\nthree");
        assert_eq!(lines(&buffer), ["one", "two"]);
        write(&buffer, "\n");
        assert_eq!(lines(&buffer), ["one", "two", "three"]);
    }

    #[test]
    fn the_oldest_lines_fall_off_past_the_cap() {
        let buffer = LogBuffer::default();
        for index in 0..MAX_LINES + 5 {
            write(&buffer, &format!("line {index}\n"));
        }
        let kept = lines(&buffer);
        assert_eq!(kept.len(), MAX_LINES);
        assert_eq!(kept.first().map(String::as_str), Some("line 5"));
        assert_eq!(
            kept.last().map(String::as_str),
            Some(format!("line {}", MAX_LINES + 4).as_str())
        );
    }

    #[test]
    fn clear_empties_the_view() {
        let buffer = LogBuffer::default();
        write(&buffer, "one\n");
        buffer.clear();
        assert!(lines(&buffer).is_empty());
    }
}
