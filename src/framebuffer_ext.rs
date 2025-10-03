use crate::data_structures::vec::{String, ToString, Vec};
use crate::format_no_std;
use crate::framebuffer::{FramebufferWriter, Color, Rect, Point};
use core::alloc;
use core::fmt::Write;

/// Enhanced graphics operations
impl FramebufferWriter {
    /// Draw a circle using Bresenham's circle algorithm
    pub fn draw_circle(&mut self, center: Point, radius: usize, color: Color) {
        if radius == 0 {
            self.put_pixel(center.x, center.y, color);
            return;
        }

        let mut x = 0isize;
        let mut y = radius as isize;
        let mut d = 3 - 2 * radius as isize;

        while x <= y {
            self.draw_circle_points(center, x, y, color);
            x += 1;
            if d > 0 {
                y -= 1;
                d = d + 4 * (x - y) + 10;
            } else {
                d = d + 4 * x + 6;
            }
        }
    }

    /// Fill a circle
    pub fn fill_circle(&mut self, center: Point, radius: usize, color: Color) {
        let radius_sq = (radius * radius) as isize;
        let start_x = center.x.saturating_sub(radius);
        let end_x = (center.x + radius).min(self.width);
        let start_y = center.y.saturating_sub(radius);
        let end_y = (center.y + radius).min(self.height);

        for y in start_y..end_y {
            for x in start_x..end_x {
                let dx = x as isize - center.x as isize;
                let dy = y as isize - center.y as isize;
                if dx * dx + dy * dy <= radius_sq {
                    self.put_pixel(x, y, color);
                }
            }
        }
    }

    fn draw_circle_points(&mut self, center: Point, x: isize, y: isize, color: Color) {
        let points = [
            (center.x as isize + x, center.y as isize + y),
            (center.x as isize - x, center.y as isize + y),
            (center.x as isize + x, center.y as isize - y),
            (center.x as isize - x, center.y as isize - y),
            (center.x as isize + y, center.y as isize + x),
            (center.x as isize - y, center.y as isize + x),
            (center.x as isize + y, center.y as isize - x),
            (center.x as isize - y, center.y as isize - x),
        ];

        for (px, py) in points.iter() {
            if *px >= 0 && *py >= 0 && (*px as usize) < self.width && (*py as usize) < self.height {
                self.put_pixel(*px as usize, *py as usize, color);
            }
        }
    }

    /// Draw an ellipse
    pub fn draw_ellipse(&mut self, center: Point, rx: usize, ry: usize, color: Color) {
        let rx = rx as isize;
        let ry = ry as isize;
        let mut x = 0;
        let mut y = ry;
        let rx_sq = rx * rx;
        let ry_sq = ry * ry;
        let two_rx_sq = 2 * rx_sq;
        let two_ry_sq = 2 * ry_sq;
        let mut p;
        let mut px = 0;
        let mut py = two_rx_sq * y;

        // Region 1
        p = ry_sq - (rx_sq * ry) + (rx_sq / 4);
        while px < py {
            self.draw_ellipse_points(center, x, y, color);
            x += 1;
            px += two_ry_sq;
            if p < 0 {
                p += ry_sq + px;
            } else {
                y -= 1;
                py -= two_rx_sq;
                p += ry_sq + px - py;
            }
        }

        // Region 2
        p = ry_sq * (x + 1) * (x + 1) / 4 + rx_sq * (y - 1) * (y - 1) - rx_sq * ry_sq;
        while y >= 0 {
            self.draw_ellipse_points(center, x, y, color);
            y -= 1;
            py -= two_rx_sq;
            if p > 0 {
                p += rx_sq - py;
            } else {
                x += 1;
                px += two_ry_sq;
                p += rx_sq - py + px;
            }
        }
    }

    fn draw_ellipse_points(&mut self, center: Point, x: isize, y: isize, color: Color) {
        let points = [
            (center.x as isize + x, center.y as isize + y),
            (center.x as isize - x, center.y as isize + y),
            (center.x as isize + x, center.y as isize - y),
            (center.x as isize - x, center.y as isize - y),
        ];

        for (px, py) in points.iter() {
            if *px >= 0 && *py >= 0 && (*px as usize) < self.width && (*py as usize) < self.height {
                self.put_pixel(*px as usize, *py as usize, color);
            }
        }
    }

    /// Fill a polygon using scanline algorithm (simple convex polygons)
    pub fn fill_polygon(&mut self, points: &[Point], color: Color) {
        if points.len() < 3 {
            return;
        }

        let min_y = points.iter().map(|p| p.y).min().unwrap_or(0);
        let max_y = points.iter().map(|p| p.y).max().unwrap_or(0);

        for y in min_y..=max_y {
            let mut intersections = Vec::new();

            for i in 0..points.len() {
                let j = (i + 1) % points.len();
                let p1 = points[i];
                let p2 = points[j];

                if (p1.y <= y && y < p2.y) || (p2.y <= y && y < p1.y) {
                    if p2.y != p1.y {
                        let x = p1.x + (y - p1.y) * (p2.x - p1.x) / (p2.y - p1.y);
                        intersections.push(x);
                    }
                }
            }

            intersections.sort_unstable();

            for chunk in intersections.chunks(2) {
                if chunk.len() == 2 {
                    let start = chunk[0];
                    let end = chunk[1];
                    for x in start..=end {
                        if x < self.width && y < self.height {
                            self.put_pixel(x, y, color);
                        }
                    }
                }
            }
        }
    }

    /// Draw text with word wrapping
    pub fn draw_text_wrapped(&mut self, text: &str, rect: Rect, color: Color) -> usize {
        let old_colors = (self.fg_color, self.bg_color);
        self.set_fg_color(color);

        let chars_per_line = rect.width / self.cell_w;
        let lines_available = rect.height / self.cell_h;
        let mut current_line = 0;

        let words: Vec<&str> = text.split_whitespace().collect();
        let mut line = String::new();

        for word in words {
            if current_line >= lines_available {
                break;
            }

            let test_line = if line.is_empty() {
                word.to_string()
            } else {
                {
                    let mut buf = [0u8; 128];
                    format_no_std!(&mut buf, "{} {}", line, word).unwrap_or_default().to_string()
                }
            };

            if test_line.len() <= chars_per_line {
                line = test_line;
            } else {
                // Draw current line
                if !line.is_empty() {
                    self.write_at(
                        &line,
                        rect.x / self.cell_w,
                        (rect.y / self.cell_h) + current_line,
                    );
                    current_line += 1;
                }
                line = word.to_string();
            }
        }

        // Draw remaining line
        if !line.is_empty() && current_line < lines_available {
            self.write_at(
                &line,
                rect.x / self.cell_w,
                (rect.y / self.cell_h) + current_line,
            );
            current_line += 1;
        }

        self.set_colors(old_colors.0, old_colors.1);
        current_line
    }

    /// Draw a gradient rectangle
    pub fn fill_gradient_rect(&mut self, rect: Rect, start_color: Color, end_color: Color, vertical: bool) {
        for y in rect.y..rect.y + rect.height.min(self.height - rect.y) {
            for x in rect.x..rect.x + rect.width.min(self.width - rect.x) {
                let progress = if vertical {
                    (y - rect.y) as f32 / rect.height as f32
                } else {
                    (x - rect.x) as f32 / rect.width as f32
                };

                let r = start_color.r as f32 + (end_color.r as f32 - start_color.r as f32) * progress;
                let g = start_color.g as f32 + (end_color.g as f32 - start_color.g as f32) * progress;
                let b = start_color.b as f32 + (end_color.b as f32 - start_color.b as f32) * progress;

                let color = Color::new(r as u8, g as u8, b as u8);
                self.put_pixel(x, y, color);
            }
        }
    }
}

/// Simple UI Widget system
#[derive(Debug, Clone)]
pub struct Widget {
    pub rect: Rect,
    pub background: Color,
    pub border_color: Option<Color>,
    pub text: String,
    pub text_color: Color,
    pub visible: bool,
    pub widget_type: WidgetType,
}

#[derive(Debug, Clone)]
pub enum WidgetType {
    Label,
    Button { pressed: bool },
    TextBox { focused: bool, cursor_pos: usize },
    Panel,
    ProgressBar { value: f32, max: f32 },
}

impl Widget {
    pub fn new_label(rect: Rect, text: &str, color: Color) -> Self {
        Self {
            rect,
            background: Color::BLACK,
            border_color: None,
            text: text.to_string(),
            text_color: color,
            visible: true,
            widget_type: WidgetType::Label,
        }
    }

    pub fn new_button(rect: Rect, text: &str) -> Self {
        Self {
            rect,
            background: Color::DARK_GRAY,
            border_color: Some(Color::LIGHT_GRAY),
            text: text.to_string(),
            text_color: Color::WHITE,
            visible: true,
            widget_type: WidgetType::Button { pressed: false },
        }
    }

    pub fn new_progress_bar(rect: Rect, value: f32, max: f32) -> Self {
        Self {
            rect,
            background: Color::DARK_GRAY,
            border_color: Some(Color::LIGHT_GRAY),
            text: String::new(),
            text_color: Color::WHITE,
            visible: true,
            widget_type: WidgetType::ProgressBar { value, max },
        }
    }

    pub fn render(&self, fb: &mut FramebufferWriter) {
        if !self.visible {
            return;
        }

        match &self.widget_type {
            WidgetType::Label => {
                // Draw background then text at top-left of widget rect
                fb.fill_rect(self.rect, self.background);
                let text_col = self.rect.x / fb.cell_w;
                let text_row = self.rect.y / fb.cell_h;
                fb.write_at(&self.text, text_col, text_row);
            }
            WidgetType::Button { pressed } => {
                let bg_color = if *pressed {
                    Color::GRAY
                } else {
                    self.background
                };

                fb.fill_rect(self.rect, bg_color);

                if let Some(border) = self.border_color {
                    fb.draw_rect(self.rect, border);
                }

                // Center text in button (character coords)
                let text_w_px = self.text.len() * fb.cell_w;
                let text_h_px = fb.cell_h;
                let text_x_px = self.rect.x + (self.rect.width.saturating_sub(text_w_px) / 2);
                let text_y_px = self.rect.y + (self.rect.height.saturating_sub(text_h_px) / 2);
                fb.write_at(
                    &self.text,
                    text_x_px / fb.cell_w,
                    text_y_px / fb.cell_h,
                );
            }
            WidgetType::ProgressBar { value, max } => {
                // Background
                fb.fill_rect(self.rect, self.background);

                // Progress fill
                let progress = (*value / *max).min(1.0).max(0.0);
                let fill_width = (self.rect.width as f32 * progress) as usize;
                let fill_rect = Rect::new(self.rect.x, self.rect.y, fill_width, self.rect.height);
                fb.fill_rect(fill_rect, Color::GREEN);

                // Border
                if let Some(border) = self.border_color {
                    fb.draw_rect(self.rect, border);
                }

                // Progress text centered
                let mut buf = [0u8; 16];
                let progress_text = format_no_std!(&mut buf, "{:.0}%", progress * 100.0).unwrap_or_default();
                let text_w_px = progress_text.len() * fb.cell_w;
                let text_h_px = fb.cell_h;
                let text_x_px = self.rect.x + (self.rect.width.saturating_sub(text_w_px) / 2);
                let text_y_px = self.rect.y + (self.rect.height.saturating_sub(text_h_px) / 2);
                fb.write_at(
                    &progress_text,
                    text_x_px / fb.cell_w,
                    text_y_px / fb.cell_h,
                );
            }
            WidgetType::TextBox { focused: _, cursor_pos } => {
                // Draw background and border, then text starting at inner padding
                fb.fill_rect(self.rect, self.background);
                if let Some(border) = self.border_color {
                    fb.draw_rect(self.rect, border);
                }
                let text_col = (self.rect.x + 2) / fb.cell_w;
                let text_row = (self.rect.y + 2) / fb.cell_h;
                fb.write_at(&self.text, text_col, text_row);
            }
            WidgetType::Panel => {
                fb.fill_rect(self.rect, self.background);
                if let Some(border) = self.border_color {
                    fb.draw_rect(self.rect, border);
                }
            }
        }
    }
}
