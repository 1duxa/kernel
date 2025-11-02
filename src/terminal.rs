use crate::{data_structures::vec::{String, Vec}, framebuffer::framebuffer::{Color, CursorStyle, FramebufferWriter}, vec};
use core::fmt::{self, Write};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cell {
    pub ch: char,
    pub fg: Color,
    pub bg: Color,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            ch: ' ',
            fg: Color::WHITE,
            bg: Color::BLACK,
            bold: false,
            italic: false,
            underline: false,
        }
    }
}

/// Terminal state and ANSI escape sequence parsing
pub struct Terminal {
    // Screen buffer
    buffer: Vec<Vec<Cell>>,
    width: usize,
    height: usize,
    
    // Dirty tracking - which cells need re-rendering
    dirty_lines: Vec<bool>,
    full_redraw_needed: bool,
    
    // Cursor state
    cursor_x: usize,
    cursor_y: usize,
    cursor_visible: bool,
    cursor_style: CursorStyle,
    last_cursor_x: usize,
    last_cursor_y: usize,
    
    // Colors
    current_fg: Color,
    current_bg: Color,
    default_fg: Color,
    default_bg: Color,
    
    // Text attributes
    bold: bool,
    italic: bool,
    underline: bool,
    
    // ANSI escape sequence parsing
    escape_buffer: String,
    in_escape: bool,
    
    // Scrollback buffer
    scrollback: Vec<Vec<Cell>>,
    scrollback_limit: usize,
    scroll_offset: usize,
    
    // Tab stops (every 8 columns by default)
    tab_stops: Vec<bool>,
}

impl Terminal {
    pub fn new(width: usize, height: usize) -> Self {
        let mut buffer = Vec::with_capacity(height);
        for _ in 0..height {
            buffer.push(vec![Cell::default(); width]);
        }
        
        let mut tab_stops = vec![false; width];
        for i in (0..width).step_by(8) {
            tab_stops[i] = true;
        }
        
        Self {
            buffer,
            width,
            height,
            dirty_lines: vec![false; height],
            full_redraw_needed: true,
            cursor_x: 0,
            cursor_y: 0,
            cursor_visible: true,
            cursor_style: CursorStyle::Block,
            last_cursor_x: 0,
            last_cursor_y: 0,
            current_fg: Color::WHITE,
            current_bg: Color::BLACK,
            default_fg: Color::WHITE,
            default_bg: Color::BLACK,
            bold: false,
            italic: false,
            underline: false,
            escape_buffer: String::new(),
            in_escape: false,
            scrollback: Vec::new(),
            scrollback_limit: 1000,
            scroll_offset: 0,
            tab_stops,
        }
    }
    
    /// Mark a line as dirty (needs re-rendering)
    fn mark_dirty(&mut self, line: usize) {
        if line < self.height {
            self.dirty_lines[line] = true;
        }
    }
    
    /// Mark the cursor's old and new positions as dirty
    fn mark_cursor_dirty(&mut self) {
        self.mark_dirty(self.last_cursor_y);
        self.mark_dirty(self.cursor_y);
    }
    
    /// Write text to the terminal with ANSI escape sequence support
    pub fn write(&mut self, text: &str) {
        for ch in text.chars() {
            self.process_char(ch);
        }
    }
    
    fn process_char(&mut self, ch: char) {
        if self.in_escape {
            self.escape_buffer.push(ch);
            
            // Check if escape sequence is complete
            if self.is_escape_complete() {
                self.process_escape_sequence();
                self.escape_buffer.clear();
                self.in_escape = false;
            }
            return;
        }
        
        match ch {
            '\x1b' => {
                // ESC character - start of escape sequence
                self.in_escape = true;
                self.escape_buffer.clear();
            }
            '\n' => self.line_feed(),
            '\r' => self.carriage_return(),
            '\t' => self.tab(),
            '\x08' => self.backspace(),
            '\x0c' => self.clear_screen(),
            ch if ch.is_control() => {
                // Ignore other control characters for now
            }
            ch => self.put_char(ch),
        }
    }
    
    fn is_escape_complete(&self) -> bool {
        if self.escape_buffer.is_empty() {
            return false;
        }
        
        // Simple heuristic: escape sequences ending with letters
        let last_char = self.escape_buffer.chars().last().unwrap();
        last_char.is_alphabetic() || "~".contains(last_char)
    }
    
    fn process_escape_sequence(&mut self) {
        if self.escape_buffer.starts_with('[') {
            // CSI sequence: skip leading '[' and pass an owned String
            use crate::data_structures::vec::ToString;
            let seq = (&self.escape_buffer[1..]).to_string();
            self.process_csi_sequence(&seq);
        }
        // Add support for other escape sequence types as needed
    }
    
    fn process_csi_sequence(&mut self, seq: &str) {
        let final_char = seq.chars().last().unwrap_or('\0');
        let params: Vec<usize> = seq[..seq.len()-1]
            .split(';')
            .filter_map(|s| s.parse().ok())
            .collect();
        
        match final_char {
            'H' | 'f' => {
                // Cursor position
                let row = params.get(0).copied().unwrap_or(1).saturating_sub(1);
                let col = params.get(1).copied().unwrap_or(1).saturating_sub(1);
                self.move_cursor_to(col, row);
            }
            'A' => {
                // Cursor up
                let n = params.get(0).copied().unwrap_or(1);
                self.move_cursor_up(n);
            }
            'B' => {
                // Cursor down
                let n = params.get(0).copied().unwrap_or(1);
                self.move_cursor_down(n);
            }
            'C' => {
                // Cursor right
                let n = params.get(0).copied().unwrap_or(1);
                self.move_cursor_right(n);
            }
            'D' => {
                // Cursor left
                let n = params.get(0).copied().unwrap_or(1);
                self.move_cursor_left(n);
            }
            'J' => {
                // Erase display
                let mode = params.get(0).copied().unwrap_or(0);
                self.erase_display(mode);
            }
            'K' => {
                // Erase line
                let mode = params.get(0).copied().unwrap_or(0);
                self.erase_line(mode);
            }
            'm' => {
                // SGR - Select Graphic Rendition
                self.process_sgr(&params);
            }
            _ => {
                // Unknown sequence - ignore
            }
        }
    }
    
    fn process_sgr(&mut self, params: &[usize]) {
        if params.is_empty() {
            // Reset all attributes
            self.reset_attributes();
            return;
        }
        
        for &param in params {
            match param {
                0 => self.reset_attributes(),
                1 => self.bold = true,
                3 => self.italic = true,
                4 => self.underline = true,
                22 => self.bold = false,
                23 => self.italic = false,
                24 => self.underline = false,
                30..=37 => self.current_fg = self.ansi_color_to_color(param - 30, false),
                40..=47 => self.current_bg = self.ansi_color_to_color(param - 40, false),
                90..=97 => self.current_fg = self.ansi_color_to_color(param - 90, true),
                100..=107 => self.current_bg = self.ansi_color_to_color(param - 100, true),
                39 => self.current_fg = self.default_fg,
                49 => self.current_bg = self.default_bg,
                _ => {} // Ignore unknown parameters
            }
        }
    }
    
    fn ansi_color_to_color(&self, color_code: usize, bright: bool) -> Color {
        match color_code {
            0 => if bright { Color::DARK_GRAY } else { Color::BLACK },
            1 => if bright { Color::from_hex(0xFF5555) } else { Color::from_hex(0xAA0000) },
            2 => if bright { Color::from_hex(0x55FF55) } else { Color::from_hex(0x00AA00) },
            3 => if bright { Color::YELLOW } else { Color::from_hex(0xAA5500) },
            4 => if bright { Color::from_hex(0x5555FF) } else { Color::from_hex(0x0000AA) },
            5 => if bright { Color::from_hex(0xFF55FF) } else { Color::from_hex(0xAA00AA) },
            6 => if bright { Color::CYAN } else { Color::from_hex(0x00AAAA) },
            7 => if bright { Color::WHITE } else { Color::LIGHT_GRAY },
            _ => Color::WHITE,
        }
    }
    
    fn reset_attributes(&mut self) {
        self.current_fg = self.default_fg;
        self.current_bg = self.default_bg;
        self.bold = false;
        self.italic = false;
        self.underline = false;
    }
    
    fn put_char(&mut self, ch: char) {
        if self.cursor_x >= self.width {
            self.line_feed();
            self.cursor_x = 0;
        }
        
        let cell = Cell {
            ch,
            fg: self.current_fg,
            bg: self.current_bg,
            bold: self.bold,
            italic: self.italic,
            underline: self.underline,
        };
        
        // Only mark dirty if the cell actually changed
        if self.buffer[self.cursor_y][self.cursor_x] != cell {
            self.buffer[self.cursor_y][self.cursor_x] = cell;
            self.mark_dirty(self.cursor_y);
        }
        
        self.last_cursor_x = self.cursor_x;
        self.last_cursor_y = self.cursor_y;
        self.cursor_x += 1;
        self.mark_cursor_dirty();
    }
    
    fn line_feed(&mut self) {
        self.mark_cursor_dirty();
        self.cursor_y += 1;
        if self.cursor_y >= self.height {
            self.scroll_up();
            self.cursor_y = self.height - 1;
        }
        self.mark_cursor_dirty();
    }
    
    fn carriage_return(&mut self) {
        self.mark_cursor_dirty();
        self.cursor_x = 0;
        self.mark_cursor_dirty();
    }
    
    fn tab(&mut self) {
        self.mark_cursor_dirty();
        // Find next tab stop
        for i in (self.cursor_x + 1)..self.width {
            if self.tab_stops[i] {
                self.cursor_x = i;
                self.mark_cursor_dirty();
                return;
            }
        }
        self.cursor_x = self.width - 1;
        self.mark_cursor_dirty();
    }
    
    fn backspace(&mut self) {
        if self.cursor_x > 0 {
            self.mark_cursor_dirty();
            self.cursor_x -= 1;
            
            // Erase the character
            self.buffer[self.cursor_y][self.cursor_x] = Cell::default();
            self.mark_dirty(self.cursor_y);
            self.mark_cursor_dirty();
        }
    }
    
    fn scroll_up(&mut self) {
        // Move top line to scrollback
        if self.scrollback.len() >= self.scrollback_limit {
            self.scrollback.remove(0);
        }
        self.scrollback.push(self.buffer[0].clone());
        
        // Shift all lines up
        for i in 0..self.height - 1 {
            self.buffer[i] = self.buffer[i + 1].clone();
        }
        
        // Clear bottom line
        self.buffer[self.height - 1] = vec![Cell::default(); self.width];
        
        // Mark all lines dirty after scroll
        self.full_redraw_needed = true;
    }
    
    // Cursor movement methods
    fn move_cursor_to(&mut self, x: usize, y: usize) {
        self.mark_cursor_dirty();
        self.last_cursor_x = self.cursor_x;
        self.last_cursor_y = self.cursor_y;
        self.cursor_x = x.min(self.width - 1);
        self.cursor_y = y.min(self.height - 1);
        self.mark_cursor_dirty();
    }
    
    fn move_cursor_up(&mut self, n: usize) {
        self.mark_cursor_dirty();
        self.last_cursor_y = self.cursor_y;
        self.cursor_y = self.cursor_y.saturating_sub(n);
        self.mark_cursor_dirty();
    }
    
    fn move_cursor_down(&mut self, n: usize) {
        self.mark_cursor_dirty();
        self.last_cursor_y = self.cursor_y;
        self.cursor_y = (self.cursor_y + n).min(self.height - 1);
        self.mark_cursor_dirty();
    }
    
    fn move_cursor_left(&mut self, n: usize) {
        self.mark_cursor_dirty();
        self.last_cursor_x = self.cursor_x;
        self.cursor_x = self.cursor_x.saturating_sub(n);
        self.mark_cursor_dirty();
    }
    
    fn move_cursor_right(&mut self, n: usize) {
        self.mark_cursor_dirty();
        self.last_cursor_x = self.cursor_x;
        self.cursor_x = (self.cursor_x + n).min(self.width - 1);
        self.mark_cursor_dirty();
    }
    
    // Erase methods
    fn erase_display(&mut self, mode: usize) {
        match mode {
            0 => {
                // Erase from cursor to end of screen
                self.erase_line(0);
                for row in (self.cursor_y + 1)..self.height {
                    for col in 0..self.width {
                        self.buffer[row][col] = Cell::default();
                    }
                    self.mark_dirty(row);
                }
            }
            1 => {
                // Erase from start of screen to cursor
                for row in 0..self.cursor_y {
                    for col in 0..self.width {
                        self.buffer[row][col] = Cell::default();
                    }
                    self.mark_dirty(row);
                }
                self.erase_line(1);
            }
            2 => {
                // Erase entire screen
                self.clear_screen();
            }
            _ => {}
        }
    }
    
    fn erase_line(&mut self, mode: usize) {
        match mode {
            0 => {
                // Erase from cursor to end of line
                for col in self.cursor_x..self.width {
                    self.buffer[self.cursor_y][col] = Cell::default();
                }
                self.mark_dirty(self.cursor_y);
            }
            1 => {
                // Erase from start of line to cursor
                for col in 0..=self.cursor_x {
                    self.buffer[self.cursor_y][col] = Cell::default();
                }
                self.mark_dirty(self.cursor_y);
            }
            2 => {
                // Erase entire line
                for col in 0..self.width {
                    self.buffer[self.cursor_y][col] = Cell::default();
                }
                self.mark_dirty(self.cursor_y);
            }
            _ => {}
        }
    }
    
    fn clear_screen(&mut self) {
        for row in 0..self.height {
            for col in 0..self.width {
                self.buffer[row][col] = Cell::default();
            }
        }
        self.cursor_x = 0;
        self.cursor_y = 0;
        self.full_redraw_needed = true;
    }
    
    /// Render only the dirty (changed) parts of the terminal
    pub fn render(&mut self, fb: &mut FramebufferWriter) {
        if self.full_redraw_needed {
            // Full redraw
            for (row, line) in self.buffer.iter().enumerate() {
                for (col, cell) in line.iter().enumerate() {
                    fb.move_to(col, row);
                    fb.set_colors(cell.fg, cell.bg);
                    let _ = write!(fb, "{}", cell.ch);
                }
            }
            
            // Clear all dirty flags
            for dirty in self.dirty_lines.iter_mut() {
                *dirty = false;
            }
            self.full_redraw_needed = false;
        } else {
            // Incremental redraw - only dirty lines
            for (row, &is_dirty) in self.dirty_lines.iter().enumerate() {
                if is_dirty {
                    let line = &self.buffer[row];
                    for (col, cell) in line.iter().enumerate() {
                        fb.move_to(col, row);
                        fb.set_colors(cell.fg, cell.bg);
                        let _ = write!(fb, "{}", cell.ch);
                    }
                }
            }
            
            // Clear dirty flags
            for dirty in self.dirty_lines.iter_mut() {
                *dirty = false;
            }
        }
        
        // Update cursor
        if self.cursor_visible {
            fb.move_to(self.cursor_x, self.cursor_y);
            fb.set_cursor_visible(true);
        }
        
        // Update last cursor position
        self.last_cursor_x = self.cursor_x;
        self.last_cursor_y = self.cursor_y;
    }
    
    /// Force a full redraw on next render
    pub fn request_full_redraw(&mut self) {
        self.full_redraw_needed = true;
    }
    
    /// Get terminal dimensions
    pub fn size(&self) -> (usize, usize) {
        (self.width, self.height)
    }
    
    /// Get cursor position
    pub fn cursor_position(&self) -> (usize, usize) {
        (self.cursor_x, self.cursor_y)
    }
    
    /// Set cursor visibility
    pub fn set_cursor_visible(&mut self, visible: bool) {
        self.cursor_visible = visible;
    }
}

impl Write for Terminal {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write(s);
        Ok(())
    }
}
