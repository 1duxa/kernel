//! # Optimized Terminal Emulator
//!
//! A high-performance terminal emulator with ANSI escape sequence support.
//!
//! ## Performance Optimizations
//!
//! - **Ring buffer for lines**: Instead of copying all cells on scroll, we use
//!   a circular buffer of lines and just adjust the head pointer
//! - **Dirty line tracking**: Only re-render lines that actually changed
//! - **Batch rendering**: Coalesce adjacent cells with same styling
//! - **No full-screen redraws**: Scrolling only marks the new line as dirty

use crate::{
    data_structures::vec::{String, Vec},
    devices::framebuffer::{color::Color, framebuffer::FramebufferWriter},
    ui::Theme,
};
use core::fmt::{self, Write};
use embedded_graphics::mono_font::{ascii::FONT_10X20, MonoTextStyleBuilder};

// =============================================================================
// CELL AND LINE TYPES
// =============================================================================

/// A single character cell with foreground and background colors
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cell {
    pub ch: char,
    pub fg: Color,
    pub bg: Color,
}

impl Cell {
    #[inline]
    pub const fn new(ch: char, fg: Color, bg: Color) -> Self {
        Self { ch, fg, bg }
    }
    
    #[inline]
    pub const fn blank(fg: Color, bg: Color) -> Self {
        Self { ch: ' ', fg, bg }
    }
}

impl Default for Cell {
    fn default() -> Self {
        Self::new(' ', Color::WHITE, Color::BLACK)
    }
}

/// A single line of the terminal (fixed width)
#[derive(Clone)]
struct Line {
    cells: Vec<Cell>,
    dirty: bool,
}

impl Line {
    fn new(width: usize, fg: Color, bg: Color) -> Self {
        let mut cells = Vec::with_capacity(width);
        let blank = Cell::blank(fg, bg);
        for _ in 0..width {
            cells.push(blank);
        }
        Self { cells, dirty: true }
    }
    
    fn clear(&mut self, fg: Color, bg: Color) {
        let blank = Cell::blank(fg, bg);
        for cell in &mut self.cells {
            *cell = blank;
        }
        self.dirty = true;
    }
}

// =============================================================================
// TERMINAL
// =============================================================================

/// High-performance terminal with ring buffer for efficient scrolling
pub struct Terminal {
    // Ring buffer of lines (circular)
    lines: Vec<Line>,
    /// Index of the top visible line in the ring buffer
    top_line: usize,
    
    // Dimensions
    width: usize,
    height: usize,
    
    // Cursor position (relative to visible area)
    cursor_x: usize,
    cursor_y: usize,
    
    // For backspace boundary
    prompt_start_x: usize,
    prompt_start_y: usize,
    
    // Previous cursor position for redraw
    last_cursor_x: usize,
    last_cursor_y: usize,
    
    // Current colors
    fg: Color,
    bg: Color,
    default_fg: Color,
    default_bg: Color,
    
    // Font metrics
    char_width: usize,
    char_height: usize,
    
    // ANSI escape parser
    escape_buffer: String,
    in_escape: bool,
}

impl Terminal {
    pub fn new(width: usize, height: usize, theme: &Theme) -> Self {
        let mut lines = Vec::with_capacity(height);
        for _ in 0..height {
            lines.push(Line::new(width, theme.text, theme.background));
        }
        
        Self {
            lines,
            top_line: 0,
            width,
            height,
            cursor_x: 0,
            cursor_y: 0,
            prompt_start_x: 0,
            prompt_start_y: 0,
            last_cursor_x: 0,
            last_cursor_y: 0,
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
    
    pub fn set_prompt_start(&mut self) {
        self.prompt_start_x = self.cursor_x;
        self.prompt_start_y = self.cursor_y;
    }
    
    /// Get the actual line index in the ring buffer for a screen row
    #[inline]
    fn line_index(&self, screen_y: usize) -> usize {
        (self.top_line + screen_y) % self.height
    }
    
    /// Get mutable reference to cell at position
    #[inline]
    fn cell_mut(&mut self, x: usize, y: usize) -> Option<&mut Cell> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let idx = self.line_index(y);
        Some(&mut self.lines[idx].cells[x])
    }
    
    /// Mark a line as dirty
    #[inline]
    fn mark_line_dirty(&mut self, y: usize) {
        if y < self.height {
            let idx = self.line_index(y);
            self.lines[idx].dirty = true;
        }
    }
    
    // =========================================================================
    // TEXT OUTPUT
    // =========================================================================
    
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
            '\r' => {
                self.cursor_x = 0;
            }
            '\x08' => self.backspace(),
            '\t' => {
                let next_tab = ((self.cursor_x / 8) + 1) * 8;
                self.cursor_x = next_tab.min(self.width - 1);
            }
            _ if !ch.is_control() => self.put_char(ch),
            _ => {}
        }
    }
    
    fn put_char(&mut self, ch: char) {
        if self.cursor_x >= self.width {
            self.newline();
        }
        
        let new_cell = Cell::new(ch, self.fg, self.bg);
        let idx = self.line_index(self.cursor_y);
        
        if self.lines[idx].cells[self.cursor_x] != new_cell {
            self.lines[idx].cells[self.cursor_x] = new_cell;
            self.lines[idx].dirty = true;
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
    
    /// Efficient scroll - just rotate the ring buffer pointer
    fn scroll_up(&mut self) {
        // Move top_line forward (old top line becomes new bottom line)
        let old_top = self.top_line;
        self.top_line = (self.top_line + 1) % self.height;
        
        // Clear the line that is now at the bottom (which was the old top)
        self.lines[old_top].clear(self.fg, self.bg);
        
        // Mark ALL lines as dirty since their screen positions changed
        // But this is still better than copying all cell data!
        for line in &mut self.lines {
            line.dirty = true;
        }
        
        // Adjust prompt position if it was on the scrolled-off line
        if self.prompt_start_y > 0 {
            self.prompt_start_y -= 1;
        } else {
            self.prompt_start_x = 0;
        }
    }
    
    fn backspace(&mut self) {
        // Only allow backspace within current line and after prompt
        let can_backspace = if self.cursor_y == self.prompt_start_y {
            self.cursor_x > self.prompt_start_x
        } else {
            self.cursor_x > 0 || self.cursor_y > self.prompt_start_y
        };
        
        if can_backspace {
            if self.cursor_x > 0 {
                self.cursor_x -= 1;
            } else if self.cursor_y > 0 {
                // Wrap to previous line
                self.cursor_y -= 1;
                self.cursor_x = self.width - 1;
            }
            
            // Clear the cell
            let idx = self.line_index(self.cursor_y);
            self.lines[idx].cells[self.cursor_x] = Cell::blank(self.fg, self.bg);
            self.lines[idx].dirty = true;
        }
    }
    
    pub fn clear(&mut self) {
        for line in &mut self.lines {
            line.clear(self.default_fg, self.default_bg);
        }
        self.cursor_x = 0;
        self.cursor_y = 0;
        self.top_line = 0;
        self.prompt_start_x = 0;
        self.prompt_start_y = 0;
    }
    
    // =========================================================================
    // ANSI ESCAPE SEQUENCES
    // =========================================================================
    
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
                let row = params.first().copied().unwrap_or(1).saturating_sub(1);
                let col = params.get(1).copied().unwrap_or(1).saturating_sub(1);
                self.cursor_x = col.min(self.width - 1);
                self.cursor_y = row.min(self.height - 1);
            }
            'J' => {
                let mode = params.first().copied().unwrap_or(0);
                if mode == 2 {
                    self.clear();
                }
            }
            'm' => {
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
    
    // =========================================================================
    // RENDERING
    // =========================================================================
    
    /// Draw cursor at current position
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
    
    /// Render terminal to framebuffer (only dirty lines)
    pub fn render(&mut self, fb: &mut FramebufferWriter) {
        self.render_into_rect(
            fb,
            0,
            0,
            self.width * self.char_width,
            self.height * self.char_height,
        );
    }
    
    /// Render into a sub-rectangle
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
        
        // Mark old cursor position dirty
        if self.last_cursor_x < self.width && self.last_cursor_y < self.height {
            self.mark_line_dirty(self.last_cursor_y);
        }
        // Mark current cursor line dirty
        self.mark_line_dirty(self.cursor_y);
        
        for screen_y in 0..max_rows {
            let line_idx = self.line_index(screen_y);
            
            if !self.lines[line_idx].dirty {
                continue;
            }
            
            // Render this line
            self.render_line(fb, screen_y, line_idx, off_x, off_y, max_cols);
            self.lines[line_idx].dirty = false;
        }
        
        self.last_cursor_x = self.cursor_x;
        self.last_cursor_y = self.cursor_y;
    }
    
    /// Render a single line with run-length optimization
    fn render_line(
        &self,
        fb: &mut FramebufferWriter,
        screen_y: usize,
        line_idx: usize,
        off_x: i32,
        off_y: i32,
        max_cols: usize,
    ) {
        let line = &self.lines[line_idx];
        let py = off_y + (screen_y * self.char_height) as i32;
        
        let mut x = 0usize;
        while x < max_cols {
            let cell = line.cells[x];
            let run_fg = cell.fg;
            let run_bg = cell.bg;
            
            // Find run of cells with same colors
            let start_x = x;
            let mut run_len = 1usize;
            let mut has_text = cell.ch != ' ';
            x += 1;
            
            while x < max_cols {
                let c = line.cells[x];
                if c.fg == run_fg && c.bg == run_bg {
                    has_text |= c.ch != ' ';
                    run_len += 1;
                    x += 1;
                } else {
                    break;
                }
            }
            
            let px = off_x + (start_x * self.char_width) as i32;
            
            // Fill background
            fb.fill_rect(
                px,
                py,
                (run_len * self.char_width) as u32,
                self.char_height as u32,
                run_bg,
            );
            
            // Draw text if any non-space characters
            if has_text {
                let mut s = String::with_capacity(run_len);
                for xi in start_x..start_x + run_len {
                    s.push(line.cells[xi].ch);
                }
                let style = MonoTextStyleBuilder::new()
                    .font(&FONT_10X20)
                    .text_color(run_fg.to_rgb888())
                    .build();
                fb.draw_text(&s, px, py + 16, &style);
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

impl Clone for Terminal {
    fn clone(&self) -> Self {
        Self {
            lines: self.lines.clone(),
            top_line: self.top_line,
            width: self.width,
            height: self.height,
            cursor_x: self.cursor_x,
            cursor_y: self.cursor_y,
            prompt_start_x: self.prompt_start_x,
            prompt_start_y: self.prompt_start_y,
            last_cursor_x: self.last_cursor_x,
            last_cursor_y: self.last_cursor_y,
            fg: self.fg,
            bg: self.bg,
            default_fg: self.default_fg,
            default_bg: self.default_bg,
            char_width: self.char_width,
            char_height: self.char_height,
            escape_buffer: self.escape_buffer.clone(),
            in_escape: self.in_escape,
        }
    }
}

// =============================================================================
// ANSI COLORS
// =============================================================================

fn ansi_color(code: usize, bright: bool) -> Color {
    match (code, bright) {
        (0, false) => Color::BLACK,
        (0, true) => Color::from_hex(0x808080),
        (1, false) => Color::from_hex(0xAA0000),
        (1, true) => Color::from_hex(0xFF5555),
        (2, false) => Color::from_hex(0x00AA00),
        (2, true) => Color::from_hex(0x55FF55),
        (3, false) => Color::from_hex(0xAA5500),
        (3, true) => Color::from_hex(0xFFFF55),
        (4, false) => Color::from_hex(0x0000AA),
        (4, true) => Color::from_hex(0x5555FF),
        (5, false) => Color::from_hex(0xAA00AA),
        (5, true) => Color::from_hex(0xFF55FF),
        (6, false) => Color::from_hex(0x00AAAA),
        (6, true) => Color::from_hex(0x55FFFF),
        (7, false) => Color::from_hex(0xAAAAAA),
        (7, true) => Color::WHITE,
        _ => Color::WHITE,
    }
}
