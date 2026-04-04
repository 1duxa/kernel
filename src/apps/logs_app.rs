use crate::{
    app::{App, AppEvent, Arrow, FocusBlock},
    debug_pipeline::{self, DebugEvent},

    ui_provider::{
        color::Color,
        render::{RenderList, TextStyle},
        shape::Rect,
        theme::Theme,
    },
};
use alloc::{format, string::String, vec, vec::Vec};

const MAX_LOG_LINES: usize = 500;
const CHAR_WIDTH: usize = 10;
const CHAR_HEIGHT: usize = 20;
const HEADER_ROWS: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    pub fn color(&self) -> Color {
        match self {
            LogLevel::Debug => Color::from_hex(0x6C7086),
            LogLevel::Info => Color::from_hex(0x89B4FA),
            LogLevel::Warn => Color::from_hex(0xF9E2AF),
            LogLevel::Error => Color::from_hex(0xF38BA8),
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

pub fn log(level: LogLevel, message: &str) {
    let category = debug_pipeline::DebugCategory::General;
    let source = "logs_app::log";
    let _ = debug_pipeline::push(level, category, source, String::from(message));
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
    block: FocusBlock,
    bounds: Rect,
    scroll_offset: usize,
    last_entry_count: usize,
}

impl LogsApp {
    pub fn new(_width: usize, _height: usize) -> Self {
        Self {
            block: FocusBlock {
                id: 2,
                rect: Rect::new(0, 0, 0, 0),
            },
            bounds: Rect::new(0, 0, 0, 0),
            scroll_offset: 0,
            last_entry_count: 0,
        }
    }

    fn rows_in_bounds(&self) -> usize {
        (self.bounds.h / CHAR_HEIGHT).max(1)
    }

    fn cols_in_bounds(&self) -> usize {
        (self.bounds.w / CHAR_WIDTH).max(1)
    }

    fn visible_rows(&self) -> usize {
        self.rows_in_bounds().saturating_sub(HEADER_ROWS).max(1)
    }

    fn sync_scroll_to_tail(&mut self, total: usize, visible_rows: usize) {
        if self.scroll_offset + visible_rows >= self.last_entry_count || self.last_entry_count == 0 {
            self.scroll_offset = total.saturating_sub(visible_rows);
        }
        self.last_entry_count = total;
    }

    fn truncate_to_cols(text: &str, cols: usize) -> String {
        text.chars().take(cols).collect()
    }

    fn draw_line(
        &self,
        out: &mut RenderList,
        row: usize,
        text: &str,
        fg: Color,
        bg: Color,
    ) {
        let y = self.bounds.y + row * CHAR_HEIGHT;
        out.fill_rect(
            Rect::new(self.bounds.x, y, self.bounds.w, CHAR_HEIGHT),
            bg,
        );

        if !text.is_empty() {
            out.styled_text(
                text,
                self.bounds.x,
                y,
                TextStyle::new(fg),
            );
        }
    }

    fn collect_header(&self, out: &mut RenderList, total: usize, theme: &Theme) {
        let cols = self.cols_in_bounds();

        self.draw_line(
            out,
            0,
            &Self::truncate_to_cols("=== Kernel Logs ===", cols),
            theme.accent,
            theme.surface,
        );

        let status = format!(
            "Lines: {} | Scroll: {} | Pipeline: unified",
            total, self.scroll_offset
        );
        self.draw_line(
            out,
            1,
            &Self::truncate_to_cols(&status, cols),
            theme.muted,
            theme.surface,
        );
    }

    fn collect_entries(&self, out: &mut RenderList, events: &[DebugEvent], theme: &Theme) {
        let cols = self.cols_in_bounds();
        let visible_rows = self.visible_rows();
        let start = self.scroll_offset.min(events.len());
        let end = (start + visible_rows).min(events.len());

        for screen_row in 0..visible_rows {
            let app_row = HEADER_ROWS + screen_row;
            let entry_idx = start + screen_row;

            if entry_idx < end {
                let event = &events[entry_idx];
                let line = Self::truncate_to_cols(&event.format_line(), cols);
                self.draw_line(out, app_row, &line, event.level.color(), theme.surface);
            } else {
                self.draw_line(out, app_row, "", theme.muted, theme.surface);
            }
        }
    }
}

impl App for LogsApp {
    fn init(&mut self) {
        if !debug_pipeline::is_initialized() {
            debug_pipeline::init();
        }

        log(LogLevel::Info, "Logs app initialized");
        log(LogLevel::Info, "Welcome to DuxOS!");
        log(LogLevel::Debug, "Unified debug pipeline connected");
        log(LogLevel::Debug, "Arrow keys to scroll logs");
    }

    fn on_event(&mut self, event: AppEvent) -> bool {
        match event {
            AppEvent::KeyPress { ch, ctrl, arrow, .. } => {
                let visible_rows = self.visible_rows();
                let total = debug_pipeline::len().min(MAX_LOG_LINES);
                let old_scroll_offset = self.scroll_offset;
                let old_last_entry_count = self.last_entry_count;

                if let Some(dir) = arrow {
                    match dir {
                        Arrow::Up => {
                            self.scroll_offset = self.scroll_offset.saturating_sub(1);
                        }
                        Arrow::Down => {
                            if self.scroll_offset + visible_rows < total {
                                self.scroll_offset += 1;
                            }
                        }
                        _ => {}
                    }
                    return self.scroll_offset != old_scroll_offset;
                }

                if ctrl && ch == 'l' {
                    debug_pipeline::clear();
                    self.scroll_offset = 0;
                    self.last_entry_count = 0;
                    return self.scroll_offset != old_scroll_offset
                        || self.last_entry_count != old_last_entry_count
                        || total != 0;
                }

                match ch {
                    '[' => {
                        self.scroll_offset = self.scroll_offset.saturating_sub(visible_rows);
                    }
                    ']' => {
                        let max_offset = total.saturating_sub(visible_rows);
                        self.scroll_offset = (self.scroll_offset + visible_rows).min(max_offset);
                    }
                    _ => {}
                }

                self.scroll_offset != old_scroll_offset
            }
            AppEvent::Tick => false,
            AppEvent::Mouse(_) => false,
        }
    }

    fn layout(&mut self, bounds: Rect) {
        self.bounds = bounds;
        self.block.rect = bounds;
    }

    fn collect_render(
        &mut self,
        theme: &Theme,
        out: &mut RenderList,
    ) {
        let events = debug_pipeline::snapshot_tail(MAX_LOG_LINES);
        let total = events.len();
        let visible_rows = self.visible_rows();

        self.sync_scroll_to_tail(total, visible_rows);
        self.collect_header(out, total, theme);
        self.collect_entries(out, &events, theme);

        if debug_pipeline::is_dirty() {
            debug_pipeline::mark_clean();
        }
    }

    fn focus_blocks(&mut self) -> &mut [FocusBlock] {
        core::slice::from_mut(&mut self.block)
    }

    fn bounds(&self) -> Rect {
        self.bounds
    }
}
