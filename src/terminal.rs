//! Terminal emulator with ANSI escape sequence support
use crate::{
    data_structures::vec::{String, Vec},
    devices::framebuffer::{color::Color, framebuffer::FramebufferWriter},
    ui::Theme,
};
use core::fmt::{self, Write};
use embedded_graphics::mono_font::{ascii::FONT_10X20, MonoTextStyleBuilder};

// Character cell with styling
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cell {
    pub ch: char,
    pub fg: Color,
    pub bg: Color,
}

impl Cell {
    pub fn new(ch: char, fg: Color, bg: Color) -> Self {
        Self { ch, fg, bg }
    }
}
#[derive(Clone)]
pub struct Terminal {
    // Screen buffer
    cells: Vec<Cell>,
    // Per-cell dirty flags (flat array matching cells)
    dirty_cells: Vec<bool>,
    width: usize,
    height: usize,

    // Cursor
    cursor_x: usize,
    cursor_y: usize,
    prompt_start_x: usize,
    last_cursor_x: usize,
    last_cursor_y: usize,
    // Colors
    fg: Color,
    bg: Color,
    default_fg: Color,
    default_bg: Color,

    // Font metrics (10x20 font)
    char_width: usize,
    char_height: usize,

    // ANSI escape sequence parser
    escape_buffer: String,
    in_escape: bool,
}

impl Terminal {
    pub fn new(width: usize, height: usize, theme: &Theme) -> Self {
        let cell_count = width * height;
        let mut cells = Vec::with_capacity(cell_count);
        let mut dirty_cells = Vec::with_capacity(cell_count);

        let default_cell = Cell::new(' ', theme.text, theme.background);
        for _ in 0..cell_count {
            cells.push(default_cell);
            dirty_cells.push(true); // Initially all dirty
        }

        Self {
            cells,
            dirty_cells,
            width,
            height,
            cursor_x: 0,
            cursor_y: 0,
            prompt_start_x: 0,
            fg: theme.text,
            last_cursor_x: 0,
            last_cursor_y: 0,
            bg: theme.background,
            default_fg: theme.text,
            default_bg: theme.background,
            char_width: 10,
            char_height: 20,
            escape_buffer: String::new(),
            in_escape: false,
        }
    }

    pub fn set_prompt_start(&mut self) {
        self.prompt_start_x = self.cursor_x;
    }

    pub fn draw_cursor(&self, fb: &mut FramebufferWriter, off_x: i32, off_y: i32) {
        if self.cursor_x >= self.width || self.cursor_y >= self.height {
            return;
        }
        let px = off_x + (self.cursor_x * self.char_width) as i32;
        let py = off_y + (self.cursor_y * self.char_height) as i32;
        let inset = 2i32;
        let w = (self.char_width as i32 - inset * 2).max(1) as u32;
        let h = (self.char_height as i32 - inset * 2).max(1) as u32;
        fb.fill_rect(px + inset, py + inset, w, h, Color::from_hex(0xCCCCCC));
    }

    pub fn size(&self) -> (usize, usize) {
        (self.width, self.height)
    }

    fn cell_index(&self, x: usize, y: usize) -> Option<usize> {
        if x < self.width && y < self.height {
            Some(y * self.width + x)
        } else {
            None
        }
    }

    /// Mark a cell as dirty
    fn mark_dirty(&mut self, x: usize, y: usize) {
        if let Some(idx) = self.cell_index(x, y) {
            self.dirty_cells[idx] = true;
        }
    }

    /// Mark entire screen as dirty
    fn mark_all_dirty(&mut self) {
        for dirty in &mut self.dirty_cells {
            *dirty = true;
        }
    }

    /// Write text with ANSI escape sequence support
    pub fn write(&mut self, text: &str) {
        for ch in text.chars() {
            self.process_char(ch);
        }
    }

    fn process_char(&mut self, ch: char) {
        if self.in_escape {
            self.escape_buffer.push(ch);
            if self.is_escape_complete() {
                self.process_escape();
                self.escape_buffer.clear();
                self.in_escape = false;
            }
            return;
        }

        match ch {
            '\x1b' => {
                self.in_escape = true;
                self.escape_buffer.clear();
            }
            '\n' => self.newline(),
            '\r' => self.cursor_x = 0,
            '\x08' => self.backspace(),
            '\t' => {
                let next_tab = ((self.cursor_x / 8) + 1) * 8;
                self.cursor_x = next_tab.min(self.width - 1);
            }
            _ if !ch.is_control() => self.put_char(ch),
            _ => {} // Ignore other control chars
        }
    }

    fn is_escape_complete(&self) -> bool {
        if self.escape_buffer.is_empty() {
            return false;
        }
        let last = self.escape_buffer.chars().last().unwrap();
        last.is_alphabetic() || last == 'm'
    }

    fn process_escape(&mut self) {
        if !self.escape_buffer.starts_with('[') {
            return;
        }

        let seq = &self.escape_buffer[1..];
        if seq.is_empty() {
            return;
        }

        let last_char = seq.chars().last().unwrap();
        let params: Vec<usize> = seq[..seq.len() - 1]
            .split(';')
            .filter_map(|s| s.parse().ok())
            .collect();

        match last_char {
            'H' | 'f' => {
                // Cursor position
                let row = params.get(0).copied().unwrap_or(1).saturating_sub(1);
                let col = params.get(1).copied().unwrap_or(1).saturating_sub(1);
                self.cursor_x = col.min(self.width - 1);
                self.cursor_y = row.min(self.height - 1);
            }
            'J' => {
                // Clear screen
                let mode = params.get(0).copied().unwrap_or(0);
                if mode == 2 {
                    self.clear();
                }
            }
            'm' => {
                // SGR - colors
                if params.is_empty() {
                    self.fg = self.default_fg;
                    self.bg = self.default_bg;
                } else {
                    for &p in &params {
                        match p {
                            0 => {
                                self.fg = self.default_fg;
                                self.bg = self.default_bg;
                            }
                            30..=37 => self.fg = ansi_color(p - 30, false),
                            40..=47 => self.bg = ansi_color(p - 40, false),
                            90..=97 => self.fg = ansi_color(p - 90, true),
                            100..=107 => self.bg = ansi_color(p - 100, true),
                            _ => {}
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn put_char(&mut self, ch: char) {
        if self.cursor_x >= self.width {
            self.newline();
        }

        if let Some(idx) = self.cell_index(self.cursor_x, self.cursor_y) {
            let new_cell = Cell::new(ch, self.fg, self.bg);
            if self.cells[idx] != new_cell {
                self.cells[idx] = new_cell;
                self.dirty_cells[idx] = true;
            }
        }

        self.cursor_x += 1;
    }

    fn newline(&mut self) {
        self.cursor_x = 0;
        self.cursor_y += 1;

        if self.cursor_y >= self.height {
            self.scroll_up();
            self.cursor_y = self.height - 1;
        }
    }

    fn backspace(&mut self) {
        if self.cursor_x > self.prompt_start_x {
            self.cursor_x -= 1;
            if let Some(idx) = self.cell_index(self.cursor_x, self.cursor_y) {
                self.cells[idx] = Cell::new(' ', self.fg, self.bg);
                self.dirty_cells[idx] = true;
            }
        }
    }
    fn scroll_up(&mut self) {
        let row_size = self.width;
        let total_cells = self.cells.len();
        
        self.cells.copy_within(row_size..total_cells, 0);
        
        let last_row_start = (self.height - 1) * self.width;
        let blank = Cell::new(' ', self.fg, self.bg);
        
        for i in 0..self.width {
            self.cells[last_row_start + i] = blank;
        }
        
        self.mark_all_dirty();
    }

    pub fn clear(&mut self) {
        for idx in 0..self.cells.len() {
            self.cells[idx] = Cell::new(' ', self.default_fg, self.default_bg);
            self.dirty_cells[idx] = true;
        }
        self.cursor_x = 0;
        self.cursor_y = 0;
    }

    /// Render to framebuffer (only dirty cells) using tiled renderer APIs
    pub fn render(&mut self, fb: &mut FramebufferWriter) {
        self.render_into_rect(
            fb,
            0,
            0,
            self.width * self.char_width,
            self.height * self.char_height,
        );
    }

    /// Render into a sub-rectangle with pixel offset (for widgets)
    pub fn render_into_rect(
        &mut self,
        fb: &mut FramebufferWriter,
        off_x: i32,
        off_y: i32,
        max_w: usize,
        max_h: usize,
    ) {
        let max_cols = (max_w / self.char_width).min(self.width);
        let max_rows = (max_h / self.char_height).min(self.height);
        if self.last_cursor_x < self.width && self.last_cursor_y < self.height {
            if let Some(idxp) = self.cell_index(self.last_cursor_x, self.last_cursor_y) {
                self.dirty_cells[idxp] = true;
            }
        }
        for y in 0..max_rows {
            let mut x = 0usize;
            while x < max_cols {
                let idx = y * self.width + x;
                if !self.dirty_cells[idx] {
                    x += 1;
                    continue;
                }
                // Start of a dirty run; gather contiguous cells with same fg/bg
                let start_x = x;
                let first_cell = self.cells[idx];
                let run_fg = first_cell.fg;
                let run_bg = first_cell.bg;
                let mut run_len = 1usize;
                let mut any_non_space = first_cell.ch != ' ';
                x += 1;
                while x < max_cols {
                    let idx2 = y * self.width + x;
                    if !self.dirty_cells[idx2] { break; }
                    let c = self.cells[idx2];
                    if c.fg == run_fg && c.bg == run_bg {
                        any_non_space |= c.ch != ' ';
                        run_len += 1;
                        x += 1;
                    } else {
                        break;
                    }
                }

                let px = off_x + (start_x * self.char_width) as i32;
                let py = off_y + (y * self.char_height) as i32;
                // Fill background for the entire run
                fb.fill_rect(px, py, (run_len * self.char_width) as u32, self.char_height as u32, run_bg);

                if any_non_space {
                    // Build a string for the run and draw text in one call
                    let mut s = String::with_capacity(run_len);
                    for xi in start_x..start_x + run_len {
                        s.push(self.cells[y * self.width + xi].ch);
                    }
                    let style = MonoTextStyleBuilder::new()
                        .font(&FONT_10X20)
                        .text_color(run_fg.to_rgb888())
                        .build();
                    fb.draw_text(&s, px, py + 16, &style);
                }

                // Mark run as rendered
                for xi in start_x..start_x + run_len {
                    self.dirty_cells[y * self.width + xi] = false;
                }
            }
        }
        self.last_cursor_x = self.cursor_x;
        self.last_cursor_y = self.cursor_y;

        // Cursor is drawn by draw_cursor after content
    }
}

impl Write for Terminal {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write(s);
        Ok(())
    }
}

// ANSI color conversion
fn ansi_color(code: usize, bright: bool) -> Color {
    match code {
        0 => {
            if bright {
                Color::from_hex(0x808080)
            } else {
                Color::BLACK
            }
        }
        1 => {
            if bright {
                Color::from_hex(0xFF5555)
            } else {
                Color::from_hex(0xAA0000)
            }
        }
        2 => {
            if bright {
                Color::from_hex(0x55FF55)
            } else {
                Color::from_hex(0x00AA00)
            }
        }
        3 => {
            if bright {
                Color::from_hex(0xFFFF55)
            } else {
                Color::from_hex(0xAA5500)
            }
        }
        4 => {
            if bright {
                Color::from_hex(0x5555FF)
            } else {
                Color::from_hex(0x0000AA)
            }
        }
        5 => {
            if bright {
                Color::from_hex(0xFF55FF)
            } else {
                Color::from_hex(0xAA00AA)
            }
        }
        6 => {
            if bright {
                Color::from_hex(0x55FFFF)
            } else {
                Color::from_hex(0x00AAAA)
            }
        }
        7 => {
            if bright {
                Color::WHITE
            } else {
                Color::from_hex(0xAAAAAA)
            }
        }
        _ => Color::WHITE,
    }
}
