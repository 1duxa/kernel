// Minimal widget system compatible with current FramebufferWriter API
use crate::data_structures::vec::{String, Vec};
use crate::framebuffer::color::Color;
use crate::framebuffer::framebuffer::FramebufferWriter;
use crate::ui::theme::Theme;
use embedded_graphics::{mono_font::MonoTextStyle, pixelcolor::Rgb888, prelude::*, text::Text};

// Integer square root for no_std environments
fn isqrt(n: i32) -> i32 {
    if n < 0 { return 0; }
    if n == 0 { return 0; }
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

// Helper: fill rounded rectangle with simple quarter-circle corners
fn fill_round_rect(fb: &mut FramebufferWriter, rect: Rect, radius: usize, color: Color) {
    let r = radius as i32;
    let x0 = rect.x;
    let y0 = rect.y;
    let x1 = rect.x + rect.w as i32;
    let y1 = rect.y + rect.h as i32;

    // Fill center rectangle
    let inner_x = x0 + r;
    let inner_w = rect.w.saturating_sub((radius * 2).min(rect.w));
    if inner_w > 0 { fb.fill_rect(inner_x, y0, inner_w as u32, rect.h as u32, color); }

    // Fill left and right vertical bands with rounded corners
    for dy in 0..radius.min(rect.h) {
        let yy_top = y0 + dy as i32;
        let yy_bot = y1 - 1 - dy as i32;
        // Midpoint circle algorithm: compute x offset for this y offset
        let dx_sq = r * r - (dy as i32 * dy as i32);
        let dx = if dx_sq > 0 { isqrt(dx_sq) } else { 0 };
        
        let left_x = (x0 + r - dx).max(x0);
        let right_x = (x1 - r + dx).min(x1);
        
        // Top-left and top-right corners
        if left_x < x0 + r { fb.fill_rect(left_x, yy_top, (x0 + r - left_x) as u32, 1, color); }
        if right_x > x1 - r { fb.fill_rect(x1 - r, yy_top, (right_x - (x1 - r)) as u32, 1, color); }
        
        // Bottom-left and bottom-right corners (if rect is tall enough)
        if rect.h > radius {
            if left_x < x0 + r { fb.fill_rect(left_x, yy_bot, (x0 + r - left_x) as u32, 1, color); }
            if right_x > x1 - r { fb.fill_rect(x1 - r, yy_bot, (right_x - (x1 - r)) as u32, 1, color); }
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Rect { pub x: i32, pub y: i32, pub w: usize, pub h: usize }
impl Rect { pub fn new(x: i32, y: i32, w: usize, h: usize) -> Self { Self { x, y, w, h } } }

pub trait Widget {
    fn layout(&mut self, bounds: Rect) -> Rect;
    fn render(&mut self, fb: &mut FramebufferWriter, theme: &Theme);
}

pub struct Panel { pub rect: Rect, pub bg: Color, pub radius: Option<usize> }
impl Panel { pub fn new(bg: Color) -> Self { Self { rect: Rect::default(), bg, radius: None } } }
impl Widget for Panel {
    fn layout(&mut self, bounds: Rect) -> Rect { self.rect = bounds; self.rect }
    fn render(&mut self, fb: &mut FramebufferWriter, _theme: &Theme) {
        if let Some(r) = self.radius { fill_round_rect(fb, self.rect, r, self.bg); }
        else { fb.fill_rect(self.rect.x, self.rect.y, self.rect.w as u32, self.rect.h as u32, self.bg); }
    }
}

pub struct Label { pub rect: Rect, pub text: String, pub color: Color }
impl Label { pub fn new(text: String, color: Color) -> Self { Self { rect: Rect::default(), text, color } } }
impl Widget for Label {
    fn layout(&mut self, bounds: Rect) -> Rect { self.rect = bounds; self.rect }
    fn render(&mut self, fb: &mut FramebufferWriter, _theme: &Theme) {
        // Draw text over whatever background is already in the node buffer
        let style = MonoTextStyle::new(&embedded_graphics::mono_font::ascii::FONT_10X20, Rgb888::new(self.color.r, self.color.g, self.color.b));
        Text::new(&self.text, Point::new(self.rect.x + 8, self.rect.y + 16), style).draw(fb).ok();
    }
}

pub struct VStack<'a> { pub children: Vec<VStackItem<'a>> }
pub struct VStackItem<'a> { pub child: &'a mut dyn Widget, pub height: Option<usize> }
impl<'a> VStack<'a> { pub fn new() -> Self { Self { children: Vec::new() } } pub fn push(&mut self, child: &'a mut dyn Widget, height: Option<usize>) { self.children.push(VStackItem { child, height }); } }
impl<'a> Widget for VStack<'a> {
    fn layout(&mut self, bounds: Rect) -> Rect {
        let mut y = bounds.y; let mut remaining = bounds.h; let last = self.children.len().saturating_sub(1);
        for (i, it) in self.children.iter_mut().enumerate() {
            let h = if i == last { remaining } else { it.height.unwrap_or(0).min(remaining) };
            let r = Rect::new(bounds.x, y, bounds.w, h);
            it.child.layout(r); y += h as i32; remaining = remaining.saturating_sub(h);
        }
        bounds
    }
    fn render(&mut self, fb: &mut FramebufferWriter, theme: &Theme) { for it in self.children.iter_mut() { it.child.render(fb, theme); } }
}

use crate::terminal::Terminal;
pub struct TerminalWidget<'a> { pub rect: Rect, pub term: &'a mut Terminal }
impl<'a> TerminalWidget<'a> { pub fn new(term: &'a mut Terminal) -> Self { Self { rect: Rect::default(), term } } }
impl<'a> Widget for TerminalWidget<'a> {
    fn layout(&mut self, bounds: Rect) -> Rect { self.rect = bounds; self.rect }
    fn render(&mut self, fb: &mut FramebufferWriter, _theme: &Theme) {
        self.term.render_into_rect(fb, self.rect.x, self.rect.y, self.rect.w, self.rect.h);
        // Note: cursor is drawn as overlay in main loop after render_frame to avoid artifacts
    }
}

