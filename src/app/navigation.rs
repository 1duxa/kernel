use super::{Arrow, FocusBlock};
use crate::devices::framebuffer::color::Color;
use crate::devices::framebuffer::framebuffer::FramebufferWriter;
use crate::ui::widgets::Rect;

pub fn move_focus(blocks: &[FocusBlock], current: u32, dir: Arrow) -> u32 {
    if blocks.is_empty() {
        return current;
    }
    let idx = blocks.iter().position(|b| b.id == current).unwrap_or(0);
    let cur = blocks[idx];
    let cx = cur.rect.x + (cur.rect.w as i32 / 2);
    let cy = cur.rect.y + (cur.rect.h as i32 / 2);
    let mut best = idx;
    let mut best_score = i32::MAX;
    for (i, b) in blocks.iter().enumerate() {
        if i == idx {
            continue;
        }
        let bx = b.rect.x + (b.rect.w as i32 / 2);
        let by = b.rect.y + (b.rect.h as i32 / 2);
        let dx = bx - cx;
        let dy = by - cy;
        let in_dir = match dir {
            Arrow::Up => dy < 0 && dx.abs() <= (-dy),
            Arrow::Down => dy > 0 && dx.abs() <= dy,
            Arrow::Left => dx < 0 && dy.abs() <= (-dx),
            Arrow::Right => dx > 0 && dy.abs() <= dx,
        };
        if in_dir {
            let score = dx + dy;
            if score < best_score {
                best_score = score;
                best = i;
            }
        }
    }
    blocks[best].id
}

pub fn draw_focus_ring(fb: &mut FramebufferWriter, rect: Rect, color: Color) {
    if rect.w == 0 || rect.h == 0 {
        return;
    }
    fb.fill_rect(rect.x, rect.y, rect.w as u32, 1, color);
    fb.fill_rect(rect.x, rect.y + rect.h as i32 - 1, rect.w as u32, 1, color);
    fb.fill_rect(rect.x, rect.y, 1, rect.h as u32, color);
    fb.fill_rect(rect.x + rect.w as i32 - 1, rect.y, 1, rect.h as u32, color);
}
