//! Terminal emulator with ANSI escape sequence support
use crate::{
    data_structures::vec::{String, Vec},
    framebuffer::{color::Color, framebuffer::FramebufferWriter},
    ui::Theme,
};
use embedded_graphics::mono_font::{ascii::FONT_10X20, MonoTextStyleBuilder};
use core::fmt::{self, Write};

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
// TODO: Optimize, Make it as an app
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
            fg: theme.text,
            bg: theme.background,
            default_fg: theme.text,
            default_bg: theme.background,
            char_width: 10,
            char_height: 20,
            escape_buffer: String::new(),
            in_escape: false,
        }
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
        if self.cursor_x > 0 {
            self.cursor_x -= 1;
            if let Some(idx) = self.cell_index(self.cursor_x, self.cursor_y) {
                self.cells[idx] = Cell::new(' ', self.fg, self.bg);
                self.dirty_cells[idx] = true;
            }
        }
    }
    // TODO: Slow as hell
    fn scroll_up(&mut self) {
        // Shift all rows up
        for y in 0..self.height - 1 {
            for x in 0..self.width {
                let src_idx = (y + 1) * self.width + x;
                let dst_idx = y * self.width + x;
                self.cells[dst_idx] = self.cells[src_idx];
                self.dirty_cells[dst_idx] = true;
            }
        }
        
        let last_row_start = (self.height - 1) * self.width;
        for i in 0..self.width {
            self.cells[last_row_start + i] = Cell::new(' ', self.fg, self.bg);
            self.dirty_cells[last_row_start + i] = true;
        }
    }
    
    pub fn clear(&mut self) {
        for idx in 0..self.cells.len() {
            self.cells[idx] = Cell::new(' ', self.default_fg, self.default_bg);
            self.dirty_cells[idx] = true;
        }
        self.cursor_x = 0;
        self.cursor_y = 0;
    }
    
    /// Render to framebuffer (only dirty cells)
    pub fn render(&mut self, fb: &mut FramebufferWriter) {
        for y in 0..self.height {
            for x in 0..self.width {
                let idx = y * self.width + x;
                
                if !self.dirty_cells[idx] {
                    continue;
                }
                
                let cell = &self.cells[idx];
                let px = (x * self.char_width) as i32;
                let py = (y * self.char_height) as i32;
                
                fb.fill_rect(px, py, self.char_width as u32, self.char_height as u32, cell.bg);
                
                // Draw character if not space
                if cell.ch != ' ' {
                    let style = MonoTextStyleBuilder::new()
                        .font(&FONT_10X20)
                        .text_color(cell.fg.to_rgb888())
                        .build();
                    
                    // Text baseline for embedded-graphics fonts
                    fb.draw_char(cell.ch, px, py + 16, &style);
                }
                
                self.dirty_cells[idx] = false;
            }
        }
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
        0 => if bright { Color::from_hex(0x808080) } else { Color::BLACK },
        1 => if bright { Color::from_hex(0xFF5555) } else { Color::from_hex(0xAA0000) },
        2 => if bright { Color::from_hex(0x55FF55) } else { Color::from_hex(0x00AA00) },
        3 => if bright { Color::from_hex(0xFFFF55) } else { Color::from_hex(0xAA5500) },
        4 => if bright { Color::from_hex(0x5555FF) } else { Color::from_hex(0x0000AA) },
        5 => if bright { Color::from_hex(0xFF55FF) } else { Color::from_hex(0xAA00AA) },
        6 => if bright { Color::from_hex(0x55FFFF) } else { Color::from_hex(0x00AAAA) },
        7 => if bright { Color::WHITE } else { Color::from_hex(0xAAAAAA) },
        _ => Color::WHITE,
    }
}
