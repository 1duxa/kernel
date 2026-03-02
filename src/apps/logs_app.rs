//! # Logs Application
//!
//! Scrollable kernel log viewer with color-coded log levels.
//!
//! ## Shortcuts
//!
//! | Key | Action |
//! |-----|--------|
//! | Arrow Up/Down | Scroll line by line |
//! | `[` / `]` | Page up/down |
//! | Ctrl+L | Clear logs |

use crate::app::{App, AppEvent, Arrow, FocusBlock};
use crate::devices::framebuffer::color::Color;
use crate::devices::framebuffer::framebuffer::FramebufferWriter;
use crate::devices::framebuffer::shape::Rect;
use crate::terminal_v2::Terminal;
use alloc::{string::String, vec::Vec};
use spin::Mutex;

const MAX_LOG_LINES: usize = 500;

static LOG_BUFFER: Mutex<LogBuffer> = Mutex::new(LogBuffer::new());
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    pub fn color(&self) -> Color {
        match self {
            LogLevel::Debug => Color::from_hex(0x6C7086), // Gray
            LogLevel::Info => Color::from_hex(0x89B4FA),  // Blue
            LogLevel::Warn => Color::from_hex(0xF9E2AF),  // Yellow
            LogLevel::Error => Color::from_hex(0xF38BA8), // Red
        }
    }

    pub fn prefix(&self) -> &'static str {
        match self {
            LogLevel::Debug => "[DBG]",
            LogLevel::Info => "[INF]",
            LogLevel::Warn => "[WRN]",
            LogLevel::Error => "[ERR]",
        }
    }
}

#[derive(Clone)]
pub struct LogEntry {
    pub level: LogLevel,
    pub message: String,
}

/// Circular buffer for log entries
pub struct LogBuffer {
    entries: Vec<LogEntry>,
    dirty: bool,
}

impl LogBuffer {
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
            dirty: true,
        }
    }

    pub fn push(&mut self, level: LogLevel, message: String) {
        if self.entries.len() >= MAX_LOG_LINES {
            // Remove oldest entry
            self.entries.remove(0);
        }
        self.entries.push(LogEntry { level, message });
        self.dirty = true;
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.dirty = true;
    }

    pub fn entries(&self) -> &[LogEntry] {
        &self.entries
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn mark_clean(&mut self) {
        self.dirty = false;
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

pub fn log(level: LogLevel, message: &str) {
    let mut buffer = LOG_BUFFER.lock();
    buffer.push(level, String::from(message));
}

#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => {{
        extern crate alloc;
        use alloc::format;
        let s = format!($($arg)*);
        $crate::apps::logs_app::log($crate::apps::logs_app::LogLevel::Debug, &s);
    }};
}

#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => {{
        extern crate alloc;
        use alloc::format;
        let s = format!($($arg)*);
        $crate::apps::logs_app::log($crate::apps::logs_app::LogLevel::Info, &s);
    }};
}

#[macro_export]
macro_rules! log_warn {
    ($($arg:tt)*) => {{
        extern crate alloc;
        use alloc::format;
        let s = format!($($arg)*);
        $crate::apps::logs_app::log($crate::apps::logs_app::LogLevel::Warn, &s);
    }};
}

#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => {{
        extern crate alloc;
        use alloc::format;
        let s = format!($($arg)*);
        $crate::apps::logs_app::log($crate::apps::logs_app::LogLevel::Error, &s);
    }};
}

pub struct LogsApp {
    term: Terminal,
    block: FocusBlock,
    bounds: Rect,
    scroll_offset: usize,
    last_entry_count: usize,
}

impl LogsApp {
    pub fn new(cols: usize, rows: usize, theme: &Theme) -> Self {
        Self {
            term: Terminal::new(cols, rows, theme),
            block: FocusBlock {
                id: 2,
                rect: Rect::new(0, 0, 0, 0),
            },
            bounds: Rect::new(0, 0, 0, 0),
            scroll_offset: 0,
            last_entry_count: 0,
        }
    }

    fn refresh_display(&mut self) {
        self.term.write("\x1b[H"); // Move cursor to top-left

        let buffer = LOG_BUFFER.lock();
        let entries = buffer.entries();
        let (cols, rows) = self.term.size();

        let total = entries.len();
        let visible_rows = rows.saturating_sub(2); // Leave room for header

        if self.scroll_offset + visible_rows >= self.last_entry_count || self.last_entry_count == 0
        {
            self.scroll_offset = total.saturating_sub(visible_rows);
        }

        self.last_entry_count = total;

        // Header line
        self.term.write("\x1b[1;36m=== Kernel Logs ===\x1b[0m");
        // Clear to end of line
        self.term.write("\x1b[K\n");

        // Status line
        let mut line_str = String::from("Lines: ");
        line_str.push_str(&format_usize(total));
        line_str.push_str(" | Scroll: ");
        line_str.push_str(&format_usize(self.scroll_offset));
        self.term.write(&line_str);
        self.term.write("\x1b[K\n"); // Clear to end of line

        // visible entries
        let start = self.scroll_offset;
        let end = (start + visible_rows).min(total);

        for i in start..end {
            let entry = &entries[i];

            match entry.level {
                LogLevel::Debug => self.term.write("\x1b[90m"), // Gray
                LogLevel::Info => self.term.write("\x1b[36m"),  // Cyan
                LogLevel::Warn => self.term.write("\x1b[33m"),  // Yellow
                LogLevel::Error => self.term.write("\x1b[31m"), // Red
            }

            self.term.write(entry.level.prefix());
            self.term.write(" ");
            // Truncate message to fit in terminal width
            let max_msg_len = cols.saturating_sub(8); // Account for prefix
            if entry.message.len() > max_msg_len {
                self.term.write(&entry.message[..max_msg_len]);
            } else {
                self.term.write(&entry.message);
            }
            self.term.write("\x1b[0m\x1b[K\n"); // Reset color + clear to EOL
        }

        // Clear remaining lines if fewer entries than visible rows
        let lines_written = end - start;
        for _ in lines_written..visible_rows {
            self.term.write("\x1b[K\n"); // Clear entire line
        }
    }
}

impl App for LogsApp {
    fn init(&mut self) {
        log(LogLevel::Info, "Logs app initialized");
        log(LogLevel::Info, "Welcome to DuxOS!");
        log(LogLevel::Debug, "Press F1 for Terminal, F2 for Logs");
        log(LogLevel::Debug, "Arrow keys to scroll logs");
        self.refresh_display();
    }

    fn on_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::KeyPress {
                ch, ctrl, arrow, ..
            } => {
                let (_, rows) = self.term.size();
                let visible_rows = rows.saturating_sub(2);
                let total = LOG_BUFFER.lock().len();

                if let Some(dir) = arrow {
                    match dir {
                        Arrow::Up => {
                            self.scroll_offset = self.scroll_offset.saturating_sub(1);
                            self.refresh_display();
                        }
                        Arrow::Down => {
                            if self.scroll_offset + visible_rows < total {
                                self.scroll_offset += 1;
                            }
                            self.refresh_display();
                        }
                        _ => {}
                    }
                    return;
                }

                if ctrl && ch == 'l' {
                    LOG_BUFFER.lock().clear();
                    self.scroll_offset = 0;
                    self.refresh_display();
                    return;
                }

                match ch {
                    '[' => {
                        self.scroll_offset = self.scroll_offset.saturating_sub(visible_rows);
                        self.refresh_display();
                    }
                    ']' => {
                        let max_offset = total.saturating_sub(visible_rows);
                        self.scroll_offset = (self.scroll_offset + visible_rows).min(max_offset);
                        self.refresh_display();
                    }
                    _ => {}
                }
            }
            AppEvent::Tick => {
                if LOG_BUFFER.lock().is_dirty() {
                    self.refresh_display();
                    LOG_BUFFER.lock().mark_clean();
                }
            }
            _ => {}
        }
    }

    fn layout(&mut self, bounds: Rect) {
        self.bounds = bounds;
        self.block.rect = bounds;
    }

    fn render(&mut self, fb: &mut FramebufferWriter, _theme: &Theme) {
        {
            let buffer = LOG_BUFFER.lock();
            if buffer.len() != self.last_entry_count {
                drop(buffer);
                self.refresh_display();
            }
        }

        self.term.render_into_rect(
            fb,
            self.bounds.x,
            self.bounds.y,
            self.bounds.w,
            self.bounds.h,
        );
    }

    fn overlay(&mut self, fb: &mut FramebufferWriter, _theme: &Theme) {
        self.term.draw_cursor(fb, self.bounds.x, self.bounds.y);
    }

    fn focus_blocks(&mut self) -> &mut [FocusBlock] {
        core::slice::from_mut(&mut self.block)
    }

    fn bounds(&self) -> Rect {
        self.bounds
    }
}

fn format_usize(n: usize) -> String {
    if n == 0 {
        return String::from("0");
    }

    let mut s = String::new();
    let mut num = n;
    let mut digits = [0u8; 20];
    let mut i = 0;

    while num > 0 {
        digits[i] = (num % 10) as u8;
        num /= 10;
        i += 1;
    }

    while i > 0 {
        i -= 1;
        s.push((b'0' + digits[i]) as char);
    }

    s
}
