use crate::devices::framebuffer::framebuffer::FramebufferWriter;
use crate::ui_provider::{color::Color, shape::Rect};
use alloc::{string::String, vec::Vec};
use embedded_graphics::{
    mono_font::{ascii::FONT_10X20, MonoTextStyle, MonoTextStyleBuilder},
    pixelcolor::Rgb888,
};

const DEFAULT_BASELINE_OFFSET: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextStyle {
    pub fg: Color,
    pub baseline_offset: usize,
}

impl TextStyle {
    pub const fn new(fg: Color) -> Self {
        Self {
            fg,
            baseline_offset: DEFAULT_BASELINE_OFFSET,
        }
    }

    pub const fn with_baseline_offset(mut self, baseline_offset: usize) -> Self {
        self.baseline_offset = baseline_offset;
        self
    }

    pub fn mono_style(&self) -> MonoTextStyle<'static, Rgb888> {
        MonoTextStyleBuilder::new()
            .font(&FONT_10X20)
            .text_color(self.fg.to_rgb888())
            .build()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RenderCommand {
    Clear {
        color: Color,
    },
    FillRect {
        rect: Rect,
        color: Color,
    },
    FillRoundedRect {
        rect: Rect,
        radius: usize,
        color: Color,
    },
    StrokeRect {
        rect: Rect,
        color: Color,
        thickness: usize,
    },
    Text {
        text: String,
        x: usize,
        y: usize,
        style: TextStyle,
    },
}

impl RenderCommand {
    pub fn fill_rect(rect: Rect, color: Color) -> Self {
        Self::FillRect { rect, color }
    }

    pub fn stroke_rect(rect: Rect, color: Color, thickness: usize) -> Self {
        Self::StrokeRect {
            rect,
            color,
            thickness,
        }
    }

    pub fn text(text: impl Into<String>, x: usize, y: usize, color: Color) -> Self {
        Self::Text {
            text: text.into(),
            x,
            y,
            style: TextStyle::new(color),
        }
    }

    pub fn styled_text(text: impl Into<String>, x: usize, y: usize, style: TextStyle) -> Self {
        Self::Text {
            text: text.into(),
            x,
            y,
            style,
        }
    }
}

#[derive(Default)]
pub struct RenderList {
    commands: Vec<RenderCommand>,
}

impl RenderList {
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
        }
    }

    pub fn clear(&mut self) {
        self.commands.clear();
    }

    pub fn len(&self) -> usize {
        self.commands.len()
    }

    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    pub fn push(&mut self, command: RenderCommand) {
        self.commands.push(command);
    }

    pub fn extend<I>(&mut self, iter: I)
    where
        I: IntoIterator<Item = RenderCommand>,
    {
        self.commands.extend(iter);
    }

    pub fn iter(&self) -> core::slice::Iter<'_, RenderCommand> {
        self.commands.iter()
    }

    pub fn as_slice(&self) -> &[RenderCommand] {
        &self.commands
    }

    pub fn into_commands(self) -> Vec<RenderCommand> {
        self.commands
    }

    pub fn clear_with(&mut self, color: Color) {
        self.push(RenderCommand::Clear { color });
    }

    pub fn fill_rect(&mut self, rect: Rect, color: Color) {
        self.push(RenderCommand::FillRect { rect, color });
    }

    pub fn fill_rounded_rect(&mut self, rect: Rect, radius: usize, color: Color) {
        self.push(RenderCommand::FillRoundedRect { rect, radius, color });
    }

    pub fn stroke_rect(&mut self, rect: Rect, color: Color, thickness: usize) {
        self.push(RenderCommand::StrokeRect {
            rect,
            color,
            thickness,
        });
    }

    pub fn text(&mut self, text: impl Into<String>, x: usize, y: usize, color: Color) {
        self.push(RenderCommand::text(text, x, y, color));
    }

    pub fn styled_text(
        &mut self,
        text: impl Into<String>,
        x: usize,
        y: usize,
        style: TextStyle,
    ) {
        self.push(RenderCommand::styled_text(text, x, y, style));
    }

    pub fn flush(&self, fb: &mut FramebufferWriter) {
        for command in &self.commands {
            execute_command(fb, command);
        }
    }
}

pub fn flush_commands(fb: &mut FramebufferWriter, commands: &[RenderCommand]) {
    for command in commands {
        execute_command(fb, command);
    }
}

pub fn execute_command(fb: &mut FramebufferWriter, command: &RenderCommand) {
    match command {
        RenderCommand::Clear { color } => {
            fb.clear(*color);
        }
        RenderCommand::FillRect { rect, color } => {
            if rect.w == 0 || rect.h == 0 {
                return;
            }
            fb.fill_rect(rect.x, rect.y, rect.w, rect.h, *color);
        }
        RenderCommand::FillRoundedRect { rect, radius, color } => {
            fill_rounded_rect(fb, *rect, *radius, *color);
        }
        RenderCommand::StrokeRect {
            rect,
            color,
            thickness,
        } => {
            draw_stroke_rect(fb, *rect, *color, *thickness);
        }
        RenderCommand::Text { text, x, y, style } => {
            if text.is_empty() {
                return;
            }
            let draw_y = y.saturating_add(style.baseline_offset);
            fb.draw_text(text, *x, draw_y, &style.mono_style());
        }
    }
}

/// Filled rounded rectangle (quarter-circle corners, axis-aligned).
pub fn fill_rounded_rect(
    fb: &mut FramebufferWriter,
    rect: Rect,
    radius: usize,
    color: Color,
) {
    let w = rect.w;
    let h = rect.h;
    if w == 0 || h == 0 {
        return;
    }
    let r = radius.min(w / 2).min(h / 2);
    let x = rect.x;
    let y = rect.y;

    if r == 0 {
        fb.fill_rect(x, y, w, h, color);
        return;
    }

    let mid_w = w.saturating_sub(2 * r);
    let mid_h = h.saturating_sub(2 * r);

    if mid_w > 0 {
        fb.fill_rect(x + r, y, mid_w, h, color);
    }
    if mid_h > 0 {
        fb.fill_rect(x, y + r, w, mid_h, color);
    }

    let r2 = (r * r) as i32;

    // Top-left
    let cx = (x + r) as i32;
    let cy = (y + r) as i32;
    for py in y..y + r {
        for px in x..x + r {
            let dx = px as i32 - cx;
            let dy = py as i32 - cy;
            if dx * dx + dy * dy <= r2 {
                fb.put_pixel(px, py, color);
            }
        }
    }
    // Top-right
    let cx = (x + w - r) as i32;
    for py in y..y + r {
        for px in (x + w - r)..(x + w) {
            let dx = px as i32 - cx;
            let dy = py as i32 - cy;
            if dx * dx + dy * dy <= r2 {
                fb.put_pixel(px, py, color);
            }
        }
    }
    // Bottom-left
    let cx = (x + r) as i32;
    let cy = (y + h - r) as i32;
    for py in (y + h - r)..(y + h) {
        for px in x..x + r {
            let dx = px as i32 - cx;
            let dy = py as i32 - cy;
            if dx * dx + dy * dy <= r2 {
                fb.put_pixel(px, py, color);
            }
        }
    }
    // Bottom-right
    let cx = (x + w - r) as i32;
    let cy = (y + h - r) as i32;
    for py in (y + h - r)..(y + h) {
        for px in (x + w - r)..(x + w) {
            let dx = px as i32 - cx;
            let dy = py as i32 - cy;
            if dx * dx + dy * dy <= r2 {
                fb.put_pixel(px, py, color);
            }
        }
    }
}

fn draw_stroke_rect(
    fb: &mut FramebufferWriter,
    rect: Rect,
    color: Color,
    thickness: usize,
) {
    if rect.w == 0 || rect.h == 0 {
        return;
    }

    let t = thickness.max(1).min(rect.w).min(rect.h);

    fb.fill_rect(rect.x, rect.y, rect.w, t, color);

    if rect.h > t {
        fb.fill_rect(rect.x, rect.y + rect.h - t, rect.w, t, color);
    }

    let side_height = rect.h.saturating_sub(t * 2);
    if side_height > 0 {
        fb.fill_rect(rect.x, rect.y + t, t, side_height, color);
        if rect.w > t {
            fb.fill_rect(rect.x + rect.w - t, rect.y + t, t, side_height, color);
        }
    } else {
        fb.fill_rect(rect.x, rect.y, t, rect.h, color);
        if rect.w > t {
            fb.fill_rect(rect.x + rect.w - t, rect.y, t, rect.h, color);
        }
    }
}
