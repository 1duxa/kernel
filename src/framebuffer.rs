use bootloader_api::BootInfo;
use core::fmt::{self, Write};
use font8x8::legacy::BASIC_LEGACY as FONT_8X8_BASIC;

use crate::SERIAL;
use spin::Mutex;


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const BLACK: Color = Color { r: 0, g: 0, b: 0, a: 255 };
    pub const WHITE: Color = Color { r: 255, g: 255, b: 255, a: 255 };
    pub const RED: Color = Color { r: 255, g: 0, b: 0, a: 255 };
    pub const GREEN: Color = Color { r: 0, g: 255, b: 0, a: 255 };
    pub const BLUE: Color = Color { r: 0, g: 0, b: 255, a: 255 };
    pub const YELLOW: Color = Color { r: 255, g: 255, b: 0, a: 255 };
    pub const CYAN: Color = Color { r: 0, g: 255, b: 255, a: 255 };
    pub const MAGENTA: Color = Color { r: 255, g: 0, b: 255, a: 255 };
    pub const GRAY: Color = Color { r: 128, g: 128, b: 128, a: 255 };
    pub const DARK_GRAY: Color = Color { r: 64, g: 64, b: 64, a: 255 };
    pub const LIGHT_GRAY: Color = Color { r: 192, g: 192, b: 192, a: 255 };

    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub const fn with_alpha(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub fn to_bytes(&self) -> [u8; 4] {
        [self.r, self.g, self.b, self.a]
    }

    pub fn from_hex(hex: u32) -> Self {
        Self {
            r: ((hex >> 16) & 0xFF) as u8,
            g: ((hex >> 8) & 0xFF) as u8,
            b: (hex & 0xFF) as u8,
            a: 255,
        }
    }
}

/// Represents a point on the screen
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Point {
    pub x: usize,
    pub y: usize,
}

impl Point {
    pub const fn new(x: usize, y: usize) -> Self {
        Self { x, y }
    }
}

/// Represents a rectangular area
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

impl Rect {
    pub const fn new(x: usize, y: usize, width: usize, height: usize) -> Self {
        Self { x, y, width, height }
    }

    pub fn contains(&self, point: Point) -> bool {
        point.x >= self.x
            && point.x < self.x + self.width
            && point.y >= self.y
            && point.y < self.y + self.height
    }

    pub fn intersects(&self, other: &Rect) -> bool {
        self.x < other.x + other.width
            && self.x + self.width > other.x
            && self.y < other.y + other.height
            && self.y + self.height > other.y
    }
}

/// Text alignment options
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAlign {
    Left,
    Center,
    Right,
}

/// Cursor style for terminal emulation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorStyle {
    None,
    Block,
    Underline,
    Beam,
}

pub struct FramebufferWriter {
    framebuffer: &'static mut [u8],
    pub width: usize,
    pub height: usize,
    pub stride: usize,
    pub bytes_per_pixel: usize,

    // Character coordinates (columns, rows)
    pub x: usize,
    pub y: usize,

    pub(crate) // Colors
    fg_color: Color,
    pub(crate) bg_color: Color,

    pub(crate) // Font cell size (8x8 for font8x8)
    cell_w: usize,
    pub(crate) cell_h: usize,

    // Terminal features
    cursor_style: CursorStyle,
    cursor_visible: bool,
    cursor_blink_state: bool,

    // Scrollback buffer (simplified - you might want a ring buffer)
    scroll_offset: usize,
    
    // Tab size
    tab_size: usize,
}

impl FramebufferWriter {
    pub fn new(info: &'static mut BootInfo, x: usize, y: usize) -> Self {
        Self::new_with_colors(info, x, y, Color::WHITE, Color::BLACK)
    }

    pub fn new_with_colors(
        info: &'static mut BootInfo,
        x: usize,
        y: usize,
        fg: Color,
        bg: Color,
    ) -> Self {
        let fb = info.framebuffer.as_mut().unwrap();
        let width = fb.info().width;
        let height = fb.info().height;
        let stride = fb.info().stride;
        let bytes_per_pixel = fb.info().bytes_per_pixel;
        
        use core::fmt::Write;
        let _ = write!(
            SERIAL.lock(),
            "Framebuffer initialized: {}x{} @ {} bytes/pixel, stride: {}\n",
            width, height, bytes_per_pixel, stride
        );
        
        let buffer = fb.buffer_mut();

        let mut writer = Self {
            framebuffer: buffer,
            width,
            height,
            stride,
            bytes_per_pixel,
            x,
            y,
            fg_color: fg,
            bg_color: bg,
            cell_w: 8,
            cell_h: 8,
            cursor_style: CursorStyle::Block,
            cursor_visible: true,
            cursor_blink_state: false,
            scroll_offset: 0,
            tab_size: 4,
        };

        writer.clear();
        writer
    }

    /// Set foreground and background colors
    pub fn set_colors(&mut self, fg: Color, bg: Color) {
        self.fg_color = fg;
        self.bg_color = bg;
    }

    /// Set just foreground color
    pub fn set_fg_color(&mut self, color: Color) {
        self.fg_color = color;
    }

    /// Set just background color
    pub fn set_bg_color(&mut self, color: Color) {
        self.bg_color = color;
    }

    /// Get the number of columns (character cells) that fit on screen
    pub fn cols(&self) -> usize {
        self.width / self.cell_w
    }

    /// Get the number of rows (character cells) that fit on screen
    pub fn rows(&self) -> usize {
        self.height / self.cell_h
    }

    /// Set cursor style
    pub fn set_cursor_style(&mut self, style: CursorStyle) {
        self.cursor_style = style;
    }

    /// Toggle cursor visibility
    pub fn set_cursor_visible(&mut self, visible: bool) {
        if !visible && self.cursor_visible {
            // Erase cursor before hiding
            self.draw_cursor(false);
        }
        self.cursor_visible = visible;
        if visible {
            self.draw_cursor(true);
        }
    }

    /// Draw or erase cursor at current position
    fn draw_cursor(&mut self, draw: bool) {
        if self.cursor_style == CursorStyle::None {
            return;
        }

        let base_x = self.x * self.cell_w;
        let base_y = self.y * self.cell_h;

        let color = if draw {
            self.fg_color
        } else {
            self.bg_color
        };

        match self.cursor_style {
            CursorStyle::Block => {
                self.fill_rect(Rect::new(base_x, base_y, self.cell_w, self.cell_h), color);
            }
            CursorStyle::Underline => {
                self.fill_rect(
                    Rect::new(base_x, base_y + self.cell_h - 2, self.cell_w, 2),
                    color,
                );
            }
            CursorStyle::Beam => {
                self.fill_rect(Rect::new(base_x, base_y, 2, self.cell_h), color);
            }
            _ => {}
        }
    }

    /// Draw a single pixel
    pub fn put_pixel(&mut self, x: usize, y: usize, color: Color) {
        if x >= self.width || y >= self.height {
            return;
        }

        let offset = (y * self.stride + x) * self.bytes_per_pixel;
        if offset + 3 >= self.framebuffer.len() {
            return;
        }

        let bytes = color.to_bytes();
        self.framebuffer[offset] = bytes[0];
        self.framebuffer[offset + 1] = bytes[1];
        self.framebuffer[offset + 2] = bytes[2];
        if self.bytes_per_pixel > 3 {
            self.framebuffer[offset + 3] = bytes[3];
        }
    }

    /// Draw a line between two points (Bresenham's algorithm)
    pub fn draw_line(&mut self, start: Point, end: Point, color: Color) {
        let dx = (end.x as isize - start.x as isize).abs();
        let dy = (end.y as isize - start.y as isize).abs();
        let sx = if start.x < end.x { 1 } else { -1 };
        let sy = if start.y < end.y { 1 } else { -1 };
        let mut err = dx - dy;

        let mut x = start.x as isize;
        let mut y = start.y as isize;

        loop {
            self.put_pixel(x as usize, y as usize, color);

            if x == end.x as isize && y == end.y as isize {
                break;
            }

            let e2 = 2 * err;
            if e2 > -dy {
                err -= dy;
                x += sx;
            }
            if e2 < dx {
                err += dx;
                y += sy;
            }
        }
    }

    /// Draw a rectangle outline
    pub fn draw_rect(&mut self, rect: Rect, color: Color) {
        // Top and bottom
        for x in rect.x..rect.x + rect.width {
            self.put_pixel(x, rect.y, color);
            self.put_pixel(x, rect.y + rect.height - 1, color);
        }
        // Left and right
        for y in rect.y + 1..rect.y + rect.height - 1 {
            self.put_pixel(rect.x, y, color);
            self.put_pixel(rect.x + rect.width - 1, y, color);
        }
    }

    /// Fill a rectangle with a solid color
    pub fn fill_rect(&mut self, rect: Rect, color: Color) {
        for y in rect.y..rect.y.saturating_add(rect.height).min(self.height) {
            for x in rect.x..rect.x.saturating_add(rect.width).min(self.width) {
                self.put_pixel(x, y, color);
            }
        }
    }

    /// Scroll the screen up by one line
    pub fn scroll_up(&mut self) {
        let line_height = self.cell_h;
        let copy_height = self.height - line_height;

        // Copy pixels up
        for y in 0..copy_height {
            for x in 0..self.width {
                let src_offset = ((y + line_height) * self.stride + x) * self.bytes_per_pixel;
                let dst_offset = (y * self.stride + x) * self.bytes_per_pixel;
                
                if src_offset + 3 < self.framebuffer.len() && dst_offset + 3 < self.framebuffer.len() {
                    for i in 0..self.bytes_per_pixel.min(4) {
                        self.framebuffer[dst_offset + i] = self.framebuffer[src_offset + i];
                    }
                }
            }
        }

        // Clear the bottom line
        self.fill_rect(
            Rect::new(0, self.height - line_height, self.width, line_height),
            self.bg_color,
        );
    }

    /// Handle special control characters and regular chars
    fn put_char(&mut self, ch: char) {
        // Hide cursor before drawing
        if self.cursor_visible {
            self.draw_cursor(false);
        }

        match ch {
            '\n' => {
                self.x = 0;
                self.y = self.y.saturating_add(1);
                if self.y >= self.rows() {
                    self.scroll_up();
                    self.y = self.rows() - 1;
                }
            }
            '\r' => {
                self.x = 0;
            }
            '\t' => {
                let spaces = self.tab_size - (self.x % self.tab_size);
                for _ in 0..spaces {
                    self.put_char(' ');
                }
            }
            '\x08' => {
                // Backspace
                if self.x > 0 {
                    self.x -= 1;
                    self.put_char(' ');
                    self.x -= 1;
                }
            }
            _ => {
                let cols = self.cols();
                if cols == 0 {
                    return;
                }

                if self.x >= cols {
                    self.x = 0;
                    self.y = self.y.saturating_add(1);
                }

                if self.y >= self.rows() {
                    self.scroll_up();
                    self.y = self.rows() - 1;
                }

                self.draw_char_at(ch, self.x, self.y);

                // Advance cursor
                self.x = self.x.saturating_add(1);
                if self.x >= cols {
                    self.x = 0;
                    self.y = self.y.saturating_add(1);
                    if self.y >= self.rows() {
                        self.scroll_up();
                        self.y = self.rows() - 1;
                    }
                }
            }
        }

        // Show cursor at new position
        if self.cursor_visible {
            self.draw_cursor(true);
        }
    }

    /// Draw a character at specific cell coordinates
    fn draw_char_at(&mut self, ch: char, col: usize, row: usize) {
        let base_x = col * self.cell_w;
        let base_y = row * self.cell_h;

        // Map char to glyph index (fallback to '?' for unknown)
        let idx = if (ch as usize) < 128 {
            ch as usize
        } else {
            '?' as usize
        };
        let glyph = FONT_8X8_BASIC[idx];

        // Draw each pixel in the 8x8 cell
        for glyph_row in 0..self.cell_h {
            let y_px = base_y + glyph_row;
            if y_px >= self.height {
                break;
            }

            let row_byte = if glyph_row < 8 {
                glyph[glyph_row]
            } else {
                0
            };

            for glyph_col in 0..self.cell_w {
                let x_px = base_x + glyph_col;
                if x_px >= self.width {
                    break;
                }

                let bit = (row_byte >> glyph_col) & 1;
                let color = if bit != 0 {
                    self.fg_color
                } else {
                    self.bg_color
                };

                self.put_pixel(x_px, y_px, color);
            }
        }
    }

    /// Write a string at a specific position
    pub fn write_at(&mut self, s: &str, col: usize, row: usize) {
        self.move_to(col, row);
        self.write_str(s).unwrap();
    }

    /// Write a string with a specific color
    pub fn write_colored(&mut self, s: &str, fg: Color, bg: Color) {
        let old_fg = self.fg_color;
        let old_bg = self.bg_color;
        self.set_colors(fg, bg);
        self.write_str(s).unwrap();
        self.set_colors(old_fg, old_bg);
    }

    /// Draw a border around the screen
    pub fn draw_border(&mut self, color: Color) {
        self.draw_rect(Rect::new(0, 0, self.width, self.height), color);
    }

    /// Draw a box with optional title
    pub fn draw_box(&mut self, rect: Rect, color: Color, title: Option<&str>) {
        self.draw_rect(rect, color);
        
        if let Some(title) = title {
            let title_x = rect.x + 2;
            let title_y = rect.y / self.cell_h;
            let old_pos = (self.x, self.y);
            self.move_to(title_x / self.cell_w, title_y);
            let old_fg = self.fg_color;
            self.fg_color = color;
            let _ = write!(self, " {} ", title);
            self.move_to(old_pos.0, old_pos.1);
        }
    }

    pub fn new_line(&mut self) {
        self.put_char('\n');
    }

    pub fn move_down(&mut self, rows: usize) {
        self.y = self.y.saturating_add(rows).min(self.rows() - 1);
    }

    pub fn move_up(&mut self, rows: usize) {
        self.y = self.y.saturating_sub(rows);
    }

    pub fn move_right(&mut self, cols: usize) {
        self.x = self.x.saturating_add(cols).min(self.cols() - 1);
    }

    pub fn move_left(&mut self, cols: usize) {
        self.x = self.x.saturating_sub(cols);
    }

    pub fn move_to(&mut self, x: usize, y: usize) {
        self.x = x.min(self.cols() - 1);
        self.y = y.min(self.rows() - 1);
    }

    /// Clear the whole framebuffer to current background color
    pub fn clear(&mut self) {
        self.fill_rect(
            Rect::new(0, 0, self.width, self.height),
            self.bg_color,
        );
        self.x = 0;
        self.y = 0;
    }

    /// Clear from cursor to end of line
    pub fn clear_to_eol(&mut self) {
        for col in self.x..self.cols() {
            self.draw_char_at(' ', col, self.y);
        }
    }

    /// Clear from cursor to end of screen
    pub fn clear_to_eos(&mut self) {
        self.clear_to_eol();
        for row in self.y + 1..self.rows() {
            for col in 0..self.cols() {
                self.draw_char_at(' ', col, row);
            }
        }
    }

    /// Save current cursor position
    pub fn save_cursor(&self) -> (usize, usize) {
        (self.x, self.y)
    }

    /// Restore cursor position
    pub fn restore_cursor(&mut self, pos: (usize, usize)) {
        self.x = pos.0;
        self.y = pos.1;
    }

    /// Get framebuffer info
    pub fn info(&self) -> FramebufferInfo {
        FramebufferInfo {
            width: self.width,
            height: self.height,
            stride: self.stride,
            bytes_per_pixel: self.bytes_per_pixel,
            cols: self.cols(),
            rows: self.rows(),
        }
    }
}

impl Write for FramebufferWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for ch in s.chars() {
            self.put_char(ch);
        }
        Ok(())
    }
}

/// Information about the framebuffer
#[derive(Debug, Clone, Copy)]
pub struct FramebufferInfo {
    pub width: usize,
    pub height: usize,
    pub stride: usize,
    pub bytes_per_pixel: usize,
    pub cols: usize,
    pub rows: usize,
}

/// ANSI color codes support
#[derive(Debug, Clone, Copy)]
pub enum AnsiColor {
    Black = 0,
    Red = 1,
    Green = 2,
    Yellow = 3,
    Blue = 4,
    Magenta = 5,
    Cyan = 6,
    White = 7,
    BrightBlack = 8,
    BrightRed = 9,
    BrightGreen = 10,
    BrightYellow = 11,
    BrightBlue = 12,
    BrightMagenta = 13,
    BrightCyan = 14,
    BrightWhite = 15,
}

impl AnsiColor {
    pub fn to_color(self) -> Color {
        match self {
            AnsiColor::Black => Color::BLACK,
            AnsiColor::Red => Color::new(170, 0, 0),
            AnsiColor::Green => Color::new(0, 170, 0),
            AnsiColor::Yellow => Color::new(170, 85, 0),
            AnsiColor::Blue => Color::new(0, 0, 170),
            AnsiColor::Magenta => Color::new(170, 0, 170),
            AnsiColor::Cyan => Color::new(0, 170, 170),
            AnsiColor::White => Color::new(170, 170, 170),
            AnsiColor::BrightBlack => Color::new(85, 85, 85),
            AnsiColor::BrightRed => Color::RED,
            AnsiColor::BrightGreen => Color::GREEN,
            AnsiColor::BrightYellow => Color::YELLOW,
            AnsiColor::BrightBlue => Color::BLUE,
            AnsiColor::BrightMagenta => Color::MAGENTA,
            AnsiColor::BrightCyan => Color::CYAN,
            AnsiColor::BrightWhite => Color::WHITE,
        }
    }
}

// Optional: Global framebuffer instance
pub static FRAMEBUFFER: Mutex<Option<FramebufferWriter>> = Mutex::new(None);

/// Initialize the global framebuffer
pub fn init_framebuffer(info: &'static mut BootInfo) {
    let fb = FramebufferWriter::new(info, 0, 0);
    *FRAMEBUFFER.lock() = Some(fb);
}

/// Print to the global framebuffer
#[macro_export]
macro_rules! fbprint {
    ($($arg:tt)*) => {
        if let Some(ref mut fb) = *$crate::framebuffer::FRAMEBUFFER.lock() {
            use core::fmt::Write;
            let _ = write!(fb, $($arg)*);
        }
    };
}

#[macro_export]
macro_rules! fbprintln {
    () => ($crate::fbprint!("\n"));
    ($($arg:tt)*) => ($crate::fbprint!("{}\n", format_args!($($arg)*)));
}
