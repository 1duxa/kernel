use crate::app::{App, AppEvent, Arrow, FocusBlock};

use crate::ui_provider::{
    color::Color,
    render::{RenderCommand, RenderList, TextStyle},
    shape::Rect,
    theme::Theme,
};
use crate::vm::{compile_and_run, execute_program_in_process, example_program, VmError, VmResult};
use alloc::{format, string::String, vec, vec::Vec};

const GUTTER_WIDTH: usize = 6;
/// Separator + VM editor hint + run status (see `footer_lines`).
const FOOTER_META_ROWS: usize = 3;
/// Minimum rows for program output (default demo prints six numbers + sum).
const MIN_OUTPUT_ROWS: usize = 8;
const MIN_EDITOR_ROWS: usize = 3;
const CHAR_WIDTH: usize = 10;
const CHAR_HEIGHT: usize = 20;
const BASELINE_OFFSET: usize = 16;
const CURSOR_MARK_WIDTH: usize = 4;
const CURSOR_MARK_HEIGHT: usize = 2;

#[derive(Clone, Copy, PartialEq, Eq)]
struct RowCache {
    line_idx: Option<usize>,
    content_hash: u64,
}

impl RowCache {
    const fn empty() -> Self {
        Self {
            line_idx: None,
            content_hash: 0,
        }
    }
}

pub struct EditorApp {
    block: FocusBlock,
    bounds: Rect,

    lines: Vec<String>,
    cursor_x: usize,
    cursor_y: usize,
    scroll_x: usize,
    scroll_y: usize,

    status: String,
    last_output: String,

    row_cache: Vec<RowCache>,
    footer_cache: Vec<u64>,
    last_cursor_screen: Option<(usize, usize)>,
    full_redraw: bool,
}

impl EditorApp {
    pub fn new(_width: usize, _height: usize) -> Self {
        let mut lines = Vec::new();
        for line in example_program().lines() {
            lines.push(String::from(line));
        }
        if lines.is_empty() {
            lines.push(String::new());
        }

        Self {
            block: FocusBlock {
                id: 3,
                rect: Rect::new(0, 0, 0, 0),
            },
            bounds: Rect::new(0, 0, 0, 0),
            lines,
            cursor_x: 0,
            cursor_y: 0,
            scroll_x: 0,
            scroll_y: 0,
            status: String::from("Editor ready | Shift+Enter run | Ctrl+L clear output"),
            last_output: String::new(),
            row_cache: Vec::new(),
            footer_cache: Vec::new(),
            last_cursor_screen: None,
            full_redraw: true,
        }
    }

    fn current_line_len(&self) -> usize {
        self.lines
            .get(self.cursor_y)
            .map(|s| s.chars().count())
            .unwrap_or(0)
    }

    fn clamp_cursor(&mut self) {
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }

        if self.cursor_y >= self.lines.len() {
            self.cursor_y = self.lines.len() - 1;
        }

        let len = self.current_line_len();
        if self.cursor_x > len {
            self.cursor_x = len;
        }
    }

    fn rows_in_bounds(&self) -> usize {
        (self.bounds.h / CHAR_HEIGHT).max(1)
    }

    fn cols_in_bounds(&self) -> usize {
        (self.bounds.w / CHAR_WIDTH).max(1)
    }

    fn editor_geometry(&self) -> (usize, usize, usize) {
        let cols = self.cols_in_bounds();
        let rows = self.rows_in_bounds();
        let content_width = cols.saturating_sub(GUTTER_WIDTH + 1).max(1);
        // Leave enough vertical space for multi-line VM output. Previously we used
        // `rows - 4` for the editor and only `rows - editor_rows - 3 == 1` line for output,
        // so the default sum demo showed only the first `print` (1) and hid 15.
        let reserved_below_editor = FOOTER_META_ROWS + MIN_OUTPUT_ROWS;
        let editor_rows = rows
            .saturating_sub(reserved_below_editor)
            .max(MIN_EDITOR_ROWS);
        let output_rows = rows
            .saturating_sub(editor_rows + FOOTER_META_ROWS)
            .max(1);
        (content_width, editor_rows, output_rows)
    }

    fn ensure_cursor_visible(&mut self) {
        let (content_width, editor_rows, _) = self.editor_geometry();

        let old_scroll_x = self.scroll_x;
        let old_scroll_y = self.scroll_y;

        if self.cursor_y < self.scroll_y {
            self.scroll_y = self.cursor_y;
        } else if self.cursor_y >= self.scroll_y + editor_rows {
            self.scroll_y = self.cursor_y.saturating_sub(editor_rows - 1);
        }

        if self.cursor_x < self.scroll_x {
            self.scroll_x = self.cursor_x;
        } else if self.cursor_x >= self.scroll_x + content_width {
            self.scroll_x = self.cursor_x.saturating_sub(content_width - 1);
        }

        if self.scroll_x != old_scroll_x || self.scroll_y != old_scroll_y {
            self.invalidate_all();
        }
    }

    fn invalidate_all(&mut self) {
        self.full_redraw = true;
        self.last_cursor_screen = None;
        self.row_cache.clear();
        self.footer_cache.clear();
    }

    fn ensure_cache_sizes(&mut self, editor_rows: usize, footer_rows: usize) {
        if self.row_cache.len() != editor_rows {
            self.row_cache = vec![RowCache::empty(); editor_rows];
            self.full_redraw = true;
        }

        if self.footer_cache.len() != footer_rows {
            self.footer_cache = vec![0; footer_rows];
            self.full_redraw = true;
        }
    }

    fn byte_index_for_char(s: &str, char_idx: usize) -> usize {
        if char_idx == 0 {
            return 0;
        }

        match s.char_indices().nth(char_idx) {
            Some((idx, _)) => idx,
            None => s.len(),
        }
    }

    fn visible_slice(s: &str, start_char: usize, width: usize) -> String {
        let mut out = String::new();
        for ch in s.chars().skip(start_char).take(width) {
            out.push(ch);
        }
        out
    }

    fn hash_str(s: &str) -> u64 {
        let mut h: u64 = 1469598103934665603;
        for b in s.as_bytes() {
            h ^= *b as u64;
            h = h.wrapping_mul(1099511628211);
        }
        h
    }

    fn hash_row(line_idx: Option<usize>, content: &str) -> u64 {
        let mut h = match line_idx {
            Some(idx) => idx as u64,
            None => u64::MAX,
        };
        h ^= Self::hash_str(content).rotate_left(7);
        h
    }

    fn insert_char(&mut self, ch: char) {
        self.clamp_cursor();
        let line = &mut self.lines[self.cursor_y];
        let idx = Self::byte_index_for_char(line, self.cursor_x);
        line.insert(idx, ch);
        self.cursor_x += 1;
        self.ensure_cursor_visible();
    }

    fn insert_newline(&mut self) {
        self.clamp_cursor();

        let tail = {
            let line = &mut self.lines[self.cursor_y];
            let idx = Self::byte_index_for_char(line, self.cursor_x);
            line.split_off(idx)
        };

        self.cursor_y += 1;
        self.cursor_x = 0;
        self.lines.insert(self.cursor_y, tail);
        self.invalidate_all();
        self.ensure_cursor_visible();
    }

    fn backspace(&mut self) {
        self.clamp_cursor();

        if self.cursor_x > 0 {
            let line = &mut self.lines[self.cursor_y];
            let end = Self::byte_index_for_char(line, self.cursor_x);
            let start = Self::byte_index_for_char(line, self.cursor_x - 1);
            line.drain(start..end);
            self.cursor_x -= 1;
            self.ensure_cursor_visible();
            return;
        }

        if self.cursor_y > 0 {
            let current = self.lines.remove(self.cursor_y);
            self.cursor_y -= 1;
            let prev_len = self.lines[self.cursor_y].chars().count();
            self.lines[self.cursor_y].push_str(&current);
            self.cursor_x = prev_len;
            self.invalidate_all();
            self.ensure_cursor_visible();
        }
    }

    fn move_left(&mut self) {
        self.clamp_cursor();

        if self.cursor_x > 0 {
            self.cursor_x -= 1;
        } else if self.cursor_y > 0 {
            self.cursor_y -= 1;
            self.cursor_x = self.current_line_len();
        }

        self.ensure_cursor_visible();
    }

    fn move_right(&mut self) {
        self.clamp_cursor();
        let len = self.current_line_len();

        if self.cursor_x < len {
            self.cursor_x += 1;
        } else if self.cursor_y + 1 < self.lines.len() {
            self.cursor_y += 1;
            self.cursor_x = 0;
        }

        self.ensure_cursor_visible();
    }

    fn move_up(&mut self) {
        if self.cursor_y > 0 {
            self.cursor_y -= 1;
            self.clamp_cursor();
        }

        self.ensure_cursor_visible();
    }

    fn move_down(&mut self) {
        if self.cursor_y + 1 < self.lines.len() {
            self.cursor_y += 1;
            self.clamp_cursor();
        }

        self.ensure_cursor_visible();
    }

    fn source(&self) -> String {
        let mut out = String::new();

        for (i, line) in self.lines.iter().enumerate() {
            if i > 0 {
                out.push('\n');
            }
            out.push_str(line);
        }

        out
    }

    fn set_result_status(&mut self, result: &VmResult) {
        self.status = format!(
            "VM ok | steps={} | halted={} | stack_len={} | line={}, col={}",
            result.steps,
            result.halted,
            result.final_stack.len(),
            self.cursor_y + 1,
            self.cursor_x + 1
        );
    }

    fn set_error_status(&mut self, err: &VmError) {
        match err {
            VmError::Parse(msg) => {
                self.status = format!(
                    "Parse error | {} | line={}, col={}",
                    msg,
                    self.cursor_y + 1,
                    self.cursor_x + 1
                );
            }
            VmError::Runtime(msg) => {
                self.status = format!(
                    "Runtime error | {} | line={}, col={}",
                    msg,
                    self.cursor_y + 1,
                    self.cursor_x + 1
                );
            }
            VmError::RuntimeN(msg, n) => {
                self.status = format!(
                    "RuntimeN error | {} | line={}, col={}, n={}",
                    msg,
                    self.cursor_y + 1,
                    self.cursor_x + 1,
                    n
                );
            }
        }
    }

    fn run_program(&mut self) {
        let source = self.source();

        // Execute VM code in a dedicated userland process
        match execute_program_in_process(&source) {
            Ok(result) => {
                self.set_result_status(&result);
                self.last_output = if result.output.is_empty() {
                    String::from("(no output)")
                } else {
                    result.output.to_vec().iter().map(|&b| b as char).collect()
                };
            }
            Err(err) => {
                self.last_output = match &err {
                    VmError::Parse(msg) => format!("Parse error\n{msg}"),
                    VmError::Runtime(msg) => format!("Runtime error\n{msg}"),
                    VmError::RuntimeN(msg,n)  =>
                     format!("RuntimeN error\n{msg} n={n}"),
                };
                self.set_error_status(&err);
            }
        }

        self.invalidate_footer();
    }

    fn clear_output(&mut self) {
        self.last_output.clear();
        self.status = format!(
            "Output cleared | line={}, col={}",
            self.cursor_y + 1,
            self.cursor_x + 1
        );
        self.invalidate_footer();
    }

    fn invalidate_footer(&mut self) {
        for hash in &mut self.footer_cache {
            *hash = 0;
        }
    }

    fn status_text_color(&self) -> Color {
        Color::WHITE
    }

    fn text_style(fg: Color) -> TextStyle {
        TextStyle::new(fg).with_baseline_offset(BASELINE_OFFSET)
    }

    fn line_text_command(
        &self,
        text: &str,
        cell_x: usize,
        cell_y: usize,
        fg: Color,
    ) -> RenderCommand {
        let x = self.bounds.x + cell_x * CHAR_WIDTH;
        let y = self.bounds.y + cell_y * CHAR_HEIGHT;
        RenderCommand::styled_text(text, x, y, Self::text_style(fg))
    }

    fn row_fill_command(&self, row: usize, color: Color) -> RenderCommand {
        let y = self.bounds.y + row * CHAR_HEIGHT;
        RenderCommand::fill_rect(
            Rect::new(self.bounds.x, y, self.bounds.w, CHAR_HEIGHT),
            color,
        )
    }

    fn draw_line_text(
        &self,
        out: &mut RenderList,
        cell_y: usize,
        text: &str,
        fg: Color,
        bg: Color,
    ) {
        let cols = self.cols_in_bounds();
        out.push(self.row_fill_command(cell_y, bg));

        let mut rendered = String::new();
        for ch in text.chars().take(cols) {
            rendered.push(ch);
        }

        if !rendered.is_empty() {
            out.push(self.line_text_command(&rendered, 0, cell_y, fg));
        }
    }

    fn editor_row_text(&self, screen_row: usize, content_width: usize) -> (Option<usize>, String) {
        let line_idx = self.scroll_y + screen_row;
        let mut row = if line_idx < self.lines.len() {
            format!("{:>4} |", line_idx + 1)
        } else {
            String::from("     |")
        };

        if line_idx < self.lines.len() {
            row.push(' ');
            row.push_str(&Self::visible_slice(
                &self.lines[line_idx],
                self.scroll_x,
                content_width,
            ));
            (Some(line_idx), row)
        } else {
            (None, row)
        }
    }

    fn render_editor_rows(
        &mut self,
        out: &mut RenderList,
        theme: &Theme,
        content_width: usize,
        editor_rows: usize,
    ) {
        for screen_row in 0..editor_rows {
            let (line_idx, row_text) = self.editor_row_text(screen_row, content_width);
            let hash = Self::hash_row(line_idx, &row_text);

            if !self.full_redraw
                && self.row_cache[screen_row].line_idx == line_idx
                && self.row_cache[screen_row].content_hash == hash
            {
                continue;
            }

            out.push(self.row_fill_command(screen_row, theme.surface));

            let gutter = if let Some(idx) = line_idx {
                format!("{:>4} |", idx + 1)
            } else {
                String::from("     |")
            };
            out.push(self.line_text_command(&gutter, 0, screen_row, Color::from_hex(0x6C7086)));

            if let Some(idx) = line_idx {
                let visible = Self::visible_slice(&self.lines[idx], self.scroll_x, content_width);
                if !visible.is_empty() {
                    out.push(self.line_text_command(
                        &visible,
                        GUTTER_WIDTH + 1,
                        screen_row,
                        theme.text,
                    ));
                }
            }

            self.row_cache[screen_row] = RowCache {
                line_idx,
                content_hash: hash,
            };
        }
    }

    fn footer_lines(&self, editor_rows: usize, output_rows: usize) -> Vec<(usize, String, Color)> {
        let cols = self.cols_in_bounds();
        let mut out = Vec::new();

        out.push((
            editor_rows,
            "-".repeat(cols.min(120)),
            Color::from_hex(0x6C7086),
        ));

        let mut info = format!(
            "VM Editor | Shift+Enter run | Ctrl+L clear output | Ln {}, Col {} | X {} Y {}",
            self.cursor_y + 1,
            self.cursor_x + 1,
            self.scroll_x,
            self.scroll_y
        );
        if info.chars().count() > cols {
            info = info.chars().take(cols).collect();
        }
        out.push((editor_rows + 1, info, self.status_text_color()));

        let mut status_text = String::from("Status: ");
        status_text.push_str(&self.status);
        if status_text.chars().count() > cols {
            status_text = status_text.chars().take(cols).collect();
        }
        out.push((editor_rows + 2, status_text, Color::from_hex(0xF9E2AF)));

        let output_lines = if self.last_output.is_empty() {
            vec![String::from("(empty)")]
        } else {
            self.last_output
                .lines()
                .map(String::from)
                .collect::<Vec<_>>()
        };

        for i in 0..output_rows {
            let row = editor_rows + 3 + i;
            if i == 0 {
                let mut first = String::from("Output: ");
                if let Some(line) = output_lines.first() {
                    first.push_str(line);
                }
                if first.chars().count() > cols {
                    first = first.chars().take(cols).collect();
                }
                out.push((row, first, Color::from_hex(0xA6E3A1)));
            } else {
                let text = output_lines.get(i).cloned().unwrap_or_default();
                let clipped = if text.chars().count() > cols {
                    text.chars().take(cols).collect::<String>()
                } else {
                    text
                };
                out.push((row, clipped, self.status_text_color()));
            }
        }

        out
    }

    fn render_footer(
        &mut self,
        out: &mut RenderList,
        theme: &Theme,
        editor_rows: usize,
        output_rows: usize,
    ) {
        let lines = self.footer_lines(editor_rows, output_rows);

        for (i, (row, text, color)) in lines.iter().enumerate() {
            let hash = Self::hash_row(Some(*row), text);
            if !self.full_redraw && self.footer_cache.get(i).copied().unwrap_or(0) == hash {
                continue;
            }

            self.draw_line_text(out, *row, text, *color, theme.surface);

            if let Some(slot) = self.footer_cache.get_mut(i) {
                *slot = hash;
            }
        }
    }

    fn cursor_screen_position(
        &self,
        content_width: usize,
        editor_rows: usize,
    ) -> Option<(usize, usize)> {
        let visible_y = self.cursor_y.saturating_sub(self.scroll_y);
        if visible_y >= editor_rows {
            return None;
        }

        let visible_x = self
            .cursor_x
            .saturating_sub(self.scroll_x)
            .min(content_width);
        let cell_x = GUTTER_WIDTH + 1 + visible_x;
        if cell_x >= self.cols_in_bounds() {
            return None;
        }

        Some((cell_x, visible_y))
    }

    fn erase_cursor(
        &self,
        out: &mut RenderList,
        theme: &Theme,
        cursor_cell_x: usize,
        cursor_cell_y: usize,
        content_width: usize,
        editor_rows: usize,
    ) {
        if cursor_cell_y >= editor_rows || cursor_cell_x >= self.cols_in_bounds() {
            return;
        }

        let text_start_x = GUTTER_WIDTH + 1;
        if cursor_cell_x < text_start_x {
            return;
        }

        let visible_char_x = cursor_cell_x - text_start_x;
        if visible_char_x > content_width {
            return;
        }

        let px = self.bounds.x + cursor_cell_x * CHAR_WIDTH;
        let py = self.bounds.y + cursor_cell_y * CHAR_HEIGHT;
        let line_idx = self.scroll_y + cursor_cell_y;

        out.push(RenderCommand::fill_rect(
            Rect::new(
                px,
                py + CHAR_HEIGHT.saturating_sub(CURSOR_MARK_HEIGHT + 1),
                CURSOR_MARK_WIDTH,
                CURSOR_MARK_HEIGHT,
            ),
            theme.accent,
        ));

        if line_idx >= self.lines.len() {
            return;
        }

        let visible = Self::visible_slice(&self.lines[line_idx], self.scroll_x, content_width);
        if let Some(ch) = visible.chars().nth(visible_char_x) {
            let mut ch_buf = String::new();
            ch_buf.push(ch);
            out.push(self.line_text_command(&ch_buf, cursor_cell_x, cursor_cell_y, theme.text));
        }
    }

    fn draw_cursor(
        &self,
        out: &mut RenderList,
        theme: &Theme,
        content_width: usize,
        editor_rows: usize,
    ) -> Option<(usize, usize)> {
        let (cell_x, cell_y) = self.cursor_screen_position(content_width, editor_rows)?;
        let px = self.bounds.x + cell_x * CHAR_WIDTH;
        let py = self.bounds.y + cell_y * CHAR_HEIGHT;

        out.push(RenderCommand::fill_rect(
            Rect::new(
                px,
                py + CHAR_HEIGHT.saturating_sub(CURSOR_MARK_HEIGHT + 1),
                CURSOR_MARK_WIDTH,
                CURSOR_MARK_HEIGHT,
            ),
            theme.accent,
        ));

        Some((cell_x, cell_y))
    }
}

impl App for EditorApp {
    fn init(&mut self) {}

    fn on_event(&mut self, event: AppEvent) -> bool {
        match event {
            AppEvent::KeyPress {
                ch,
                ctrl,
                shift,
                arrow,
                ..
            } => {
                if let Some(dir) = arrow {
                    match dir {
                        Arrow::Left => self.move_left(),
                        Arrow::Right => self.move_right(),
                        Arrow::Up => self.move_up(),
                        Arrow::Down => self.move_down(),
                    }
                    return true;
                }

                if ctrl && ch == 'l' {
                    self.clear_output();
                    return true;
                }

                if ch == '\n' {
                    if shift {
                        self.run_program();
                    } else {
                        self.insert_newline();
                    }
                    return true;
                }

                if ch == '\x08' {
                    self.backspace();
                    return true;
                }

                if ch == '\t' {
                    for _ in 0..4 {
                        self.insert_char(' ');
                    }
                    return true;
                }

                if !ctrl && !ch.is_control() {
                    self.insert_char(ch);
                    return true;
                }

                false
            }
            AppEvent::Tick => false,
            AppEvent::Mouse(_) => true,
        }
    }

    fn layout(&mut self, bounds: Rect) {
        if self.bounds.x != bounds.x
            || self.bounds.y != bounds.y
            || self.bounds.w != bounds.w
            || self.bounds.h != bounds.h
        {
            self.bounds = bounds;
            self.block.rect = bounds;
            self.invalidate_all();
        } else {
            self.bounds = bounds;
            self.block.rect = bounds;
        }

        self.ensure_cursor_visible();
    }

    fn collect_render(&mut self, theme: &Theme, out: &mut RenderList) {
        self.clamp_cursor();
        self.ensure_cursor_visible();
        let (content_width, editor_rows, output_rows) = self.editor_geometry();
        let footer_rows = 3 + output_rows;
        self.ensure_cache_sizes(editor_rows, footer_rows);

        if self.full_redraw {
            out.push(RenderCommand::fill_rect(self.bounds, theme.surface));
        }

        if !self.full_redraw {
            if let Some((old_x, old_y)) = self.last_cursor_screen {
                self.erase_cursor(out, theme, old_x, old_y, content_width, editor_rows);
            }
        }

        self.render_editor_rows(out, theme, content_width, editor_rows);
        self.render_footer(out, theme, editor_rows, output_rows);

        let new_cursor = self.draw_cursor(out, theme, content_width, editor_rows);
        self.last_cursor_screen = new_cursor;
        // Keep full_redraw = true to always render content
        // self.full_redraw = false;
    }

    fn focus_blocks(&mut self) -> &mut [FocusBlock] {
        core::slice::from_mut(&mut self.block)
    }

    fn bounds(&self) -> Rect {
        self.bounds
    }
}
