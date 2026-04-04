 //! # Optimized Terminal Core
 //!
 //! A high-performance terminal buffer with ANSI escape support that can emit
 //! render commands for the unified graphics pipeline.

 use crate::ui_provider::{
     color::Color,
     render::{RenderCommand, RenderList, TextStyle},
     theme::Theme,
 };
 use alloc::{string::String, vec::Vec};
 use core::fmt::{self, Write};

 const FONT_BASELINE_OFFSET: usize = 16;

 /// A single character cell with foreground and background colors.
 #[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

 /// High-performance terminal with ring buffer for efficient scrolling.
 pub struct Terminal {
     lines: Vec<Line>,
     top_line: usize,

     width: usize,
     height: usize,

     cursor_x: usize,
     cursor_y: usize,

     prompt_start_x: usize,
     prompt_start_y: usize,

     last_cursor_x: usize,
     last_cursor_y: usize,

     fg: Color,
     bg: Color,
     default_fg: Color,
     default_bg: Color,

     char_width: usize,
     char_height: usize,

     escape_buffer: String,
     in_escape: bool,
 }

 impl Terminal {
     pub fn new(width: usize, height: usize, theme: &Theme) -> Self {
         let mut lines = Vec::with_capacity(height);
         for _ in 0..height {
             lines.push(Line::new(width, theme.text, theme.surface));
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
            bg: theme.surface,
            default_fg: theme.text,
            default_bg: theme.surface,
             char_width: 10,
             char_height: 20,
             escape_buffer: String::new(),
             in_escape: false,
         }
     }

     pub fn size(&self) -> (usize, usize) {
         (self.width, self.height)
     }

     pub fn pixel_size(&self) -> (usize, usize) {
         (self.width * self.char_width, self.height * self.char_height)
     }

     pub fn set_prompt_start(&mut self) {
         self.prompt_start_x = self.cursor_x;
         self.prompt_start_y = self.cursor_y;
     }

     #[inline]
     fn line_index(&self, screen_y: usize) -> usize {
         (self.top_line + screen_y) % self.height
     }

     #[inline]
     fn mark_line_dirty(&mut self, y: usize) {
         if y < self.height {
             let idx = self.line_index(y);
             self.lines[idx].dirty = true;
         }
     }

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
                 self.cursor_x = next_tab.min(self.width.saturating_sub(1));
             }
             _ if !ch.is_control() => self.put_char(ch),
             _ => {}
         }
     }

     fn put_char(&mut self, ch: char) {
         if self.width == 0 || self.height == 0 {
             return;
         }

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
         if self.height == 0 {
             return;
         }

         self.cursor_x = 0;
         self.cursor_y += 1;

         if self.cursor_y >= self.height {
             self.scroll_up();
             self.cursor_y = self.height - 1;
         }
     }

     fn scroll_up(&mut self) {
         let old_top = self.top_line;
         self.top_line = (self.top_line + 1) % self.height;

         self.lines[old_top].clear(self.fg, self.bg);

         for line in &mut self.lines {
             line.dirty = true;
         }

         if self.prompt_start_y > 0 {
             self.prompt_start_y -= 1;
         } else {
             self.prompt_start_x = 0;
         }
     }

     fn backspace(&mut self) {
         if self.width == 0 || self.height == 0 {
             return;
         }

         let can_backspace = if self.cursor_y == self.prompt_start_y {
             self.cursor_x > self.prompt_start_x
         } else {
             self.cursor_x > 0 || self.cursor_y > self.prompt_start_y
         };

         if can_backspace {
             if self.cursor_x > 0 {
                 self.cursor_x -= 1;
             } else if self.cursor_y > 0 {
                 self.cursor_y -= 1;
                 self.cursor_x = self.width - 1;
             }

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
         self.last_cursor_x = 0;
         self.last_cursor_y = 0;
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
                 let row = params.first().copied().unwrap_or(1).saturating_sub(1);
                 let col = params.get(1).copied().unwrap_or(1).saturating_sub(1);
                 if self.width > 0 {
                     self.cursor_x = col.min(self.width - 1);
                 }
                 if self.height > 0 {
                     self.cursor_y = row.min(self.height - 1);
                 }
             }
             'J' => {
                 let mode = params.first().copied().unwrap_or(0);
                 if mode == 2 {
                     self.clear();
                 }
             }
             'K' => {
                 if self.width == 0 || self.height == 0 {
                     return;
                 }
                 let mode = params.first().copied().unwrap_or(0);
                 let line_idx = self.line_index(self.cursor_y);
                 let blank = Cell::blank(self.fg, self.bg);
                 match mode {
                     0 => {
                         for x in self.cursor_x..self.width {
                             self.lines[line_idx].cells[x] = blank;
                         }
                     }
                     1 => {
                         for x in 0..=self.cursor_x.min(self.width - 1) {
                             self.lines[line_idx].cells[x] = blank;
                         }
                     }
                     2 => {
                         for x in 0..self.width {
                             self.lines[line_idx].cells[x] = blank;
                         }
                     }
                     _ => {}
                 }
                 self.lines[line_idx].dirty = true;
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

     pub fn invalidate_all(&mut self) {
         for line in &mut self.lines {
             line.dirty = true;
         }
     }

     pub fn collect_render(
         &mut self,
         out: &mut RenderList,
         off_x: usize,
         off_y: usize,
         max_w: usize,
         max_h: usize,
     ) {
         if self.width == 0 || self.height == 0 {
             return;
         }

         let max_cols = (max_w / self.char_width).min(self.width);
         let max_rows = (max_h / self.char_height).min(self.height);

         if max_cols == 0 || max_rows == 0 {
             return;
         }

         if self.last_cursor_x < self.width && self.last_cursor_y < self.height {
             self.mark_line_dirty(self.last_cursor_y);
         }
         self.mark_line_dirty(self.cursor_y);

         for screen_y in 0..max_rows {
             let line_idx = self.line_index(screen_y);

             if !self.lines[line_idx].dirty {
                 continue;
             }

             self.collect_line(out, screen_y, line_idx, off_x, off_y, max_cols);
             self.lines[line_idx].dirty = false;
         }

         self.collect_cursor(out, off_x, off_y, max_cols, max_rows);

         self.last_cursor_x = self.cursor_x;
         self.last_cursor_y = self.cursor_y;
     }

     pub fn collect_render_full(&mut self, out: &mut RenderList, off_x: usize, off_y: usize) {
         let (w, h) = self.pixel_size();
         self.invalidate_all();
         self.collect_render(out, off_x, off_y, w, h);
     }

     fn collect_line(
         &self,
         out: &mut RenderList,
         screen_y: usize,
         line_idx: usize,
         off_x: usize,
         off_y: usize,
         max_cols: usize,
     ) {
         let line = &self.lines[line_idx];
         let py = off_y + screen_y * self.char_height;

         let mut x = 0usize;
         while x < max_cols {
             let cell = line.cells[x];
             let run_fg = cell.fg;
             let run_bg = cell.bg;

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

             let px = off_x + start_x * self.char_width;

             out.push(RenderCommand::fill_rect(
                 crate::ui_provider::shape::Rect::new(
                     px,
                     py,
                     run_len * self.char_width,
                     self.char_height,
                 ),
                 run_bg,
             ));

             if has_text {
                 let mut s = String::with_capacity(run_len);
                 for xi in start_x..start_x + run_len {
                     s.push(line.cells[xi].ch);
                 }
                 out.push(RenderCommand::styled_text(
                     s,
                     px,
                     py,
                     TextStyle::new(run_fg).with_baseline_offset(FONT_BASELINE_OFFSET),
                 ));
             }
         }
     }

     fn collect_cursor(
         &self,
         out: &mut RenderList,
         off_x: usize,
         off_y: usize,
         max_cols: usize,
         max_rows: usize,
     ) {
         if self.cursor_x >= max_cols || self.cursor_y >= max_rows {
             return;
         }

         let px = off_x + self.cursor_x * self.char_width;
         let py = off_y + self.cursor_y * self.char_height;
         let inset = 2usize;
         let w = (self.char_width.saturating_sub(inset * 2)).max(1);
         let h = 2usize;

         out.push(RenderCommand::fill_rect(
             crate::ui_provider::shape::Rect::new(
                 px + inset,
                 py + self.char_height.saturating_sub(inset + h),
                 w,
                 h,
             ),
             Color::from_hex(0xCCCCCC),
         ));
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
