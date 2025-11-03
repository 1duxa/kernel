//! Framebuffer writer using embedded-graphics + tiled renderer
use bootloader_api::BootInfo;
use crate::{vec, framebuffer::color::Color};
use spin::Mutex;
use embedded_graphics::{
    prelude::*,
    pixelcolor::Rgb888,
    mono_font::MonoTextStyle,
    text::Text,
    Drawable,
};
use core::sync::atomic::{AtomicBool, Ordering};
use crate::data_structures::vec::Vec;
const TILE_W: usize = 32;
const TILE_H: usize = 32;

pub struct FramebufferWriter {
    framebuffer: &'static mut [u8],
    pub width: usize,
    pub height: usize,
    pub stride: usize,
    pub bytes_per_pixel: usize,
    // Tiled renderer state
    nodes: Vec<u32>,                // packed RGB888 per pixel
    tiles_x: usize,
    tiles_y: usize,
    tile_dirty: Vec<AtomicBool>,
    // per-tile per-row hashes; len = tiles * TILE_H (clipped on bottom edge)
    tile_row_hash: Vec<u64>,
}

impl FramebufferWriter {
    pub fn new(info: &'static mut BootInfo) -> Self {
        let fb = info.framebuffer.as_mut().unwrap();
        let info = fb.info();
        let width = info.width;
        let height = info.height;
        let stride = info.stride;
        let bpp = info.bytes_per_pixel;

        let tiles_x = (width + TILE_W - 1) / TILE_W;
        let tiles_y = (height + TILE_H - 1) / TILE_H;
        let tile_count = tiles_x * tiles_y;

        Self {
            framebuffer: fb.buffer_mut(),
            width,
            height,
            stride,
            bytes_per_pixel: bpp,
            nodes: vec![0u32; width * height],
            tiles_x,
            tiles_y,
            tile_dirty: (0..tile_count).map(|_| AtomicBool::new(true)).collect(),
            tile_row_hash: vec![0u64; tile_count * TILE_H],
        }
    }

    #[inline]
    fn idx(&self, x: usize, y: usize) -> usize { y * self.width + x }

    #[inline]
    fn tile_index_of(&self, x: usize, y: usize) -> usize {
        let tx = x / TILE_W;
        let ty = y / TILE_H;
        ty * self.tiles_x + tx
    }

    #[inline]
    fn tile_row_slot(&self, tile_idx: usize, row_in_tile: usize) -> usize {
        tile_idx * TILE_H + row_in_tile
    }

    #[inline]
    fn pack_rgb888(c: Color) -> u32 { ((c.r as u32) << 16) | ((c.g as u32) << 8) | (c.b as u32) }

    /// Low-level pixel write into nodes with dirty marking
    pub fn put_pixel(&mut self, x: usize, y: usize, color: Color) {
        if x >= self.width || y >= self.height { return; }
        let idx = self.idx(x, y);
        let val = Self::pack_rgb888(color);
        if self.nodes[idx] != val {
            self.nodes[idx] = val;
            let t = self.tile_index_of(x, y);
            self.tile_dirty[t].store(true, Ordering::Relaxed);
        }
    }

    /// Batch many pixels
    pub fn put_pixels(&mut self, pixels: &[(usize, usize, Color)]) {
        for &(x,y,c) in pixels {
            self.put_pixel(x,y,c);
        }
    }

    /// Fill rectangle in node buffer and mark tiles
    pub fn draw_rect(&mut self, x0: usize, y0: usize, x1: usize, y1: usize, color: Color) {
        let x1 = x1.min(self.width);
        let y1 = y1.min(self.height);
        if x0 >= x1 || y0 >= y1 { return; }
        let val = Self::pack_rgb888(color);
        let tx0 = x0 / TILE_W;
        let ty0 = y0 / TILE_H;
        let tx1 = (x1 + TILE_W - 1) / TILE_W;
        let ty1 = (y1 + TILE_H - 1) / TILE_H;
        for y in y0..y1 {
            let base = y * self.width;
            for x in x0..x1 { self.nodes[base + x] = val; }
        }
        for ty in ty0..ty1 { for tx in tx0..tx1 {
            let t = ty * self.tiles_x + tx;
            self.tile_dirty[t].store(true, Ordering::Relaxed);
        }}
    }

    /// Render dirty tiles into framebuffer with per-row hashing to skip unchanged rows
    pub fn render_frame(&mut self) {
        let fb_row_bytes = self.stride * self.bytes_per_pixel;
        let tiles = self.tiles_x * self.tiles_y;
        for tile_idx in 0..tiles {
            if !self.tile_dirty[tile_idx].swap(false, Ordering::Relaxed) { continue; }
            let tx = tile_idx % self.tiles_x;
            let ty = tile_idx / self.tiles_x;
            let sx = tx * TILE_W;
            let sy = ty * TILE_H;
            let ex = (sx + TILE_W).min(self.width);
            let ey = (sy + TILE_H).min(self.height);

            for y in sy..ey {
                let row_in_tile = y - sy;
                // compute simple rolling hash over nodes row slice within tile bounds
                let base = y * self.width + sx;
                let mut h: u64 = 1469598103934665603; // FNV offset
                for v in &self.nodes[base..base + (ex - sx)] {
                    h ^= *v as u64;
                    h = h.wrapping_mul(1099511628211);
                }
                let slot = self.tile_row_slot(tile_idx, row_in_tile);
                if self.tile_row_hash[slot] == h {
                    continue; // row unchanged, skip write
                }
                self.tile_row_hash[slot] = h;

                // write row to framebuffer in BGR or RGB depending on layout; existing code uses B,G,R order
                let fb_row_off = y * fb_row_bytes;
                let mut off = fb_row_off + sx * self.bytes_per_pixel;
                for v in &self.nodes[base..base + (ex - sx)] {
                    let r = ((v >> 16) & 0xFF) as u8;
                    let g = ((v >> 8) & 0xFF) as u8;
                    let b = (v & 0xFF) as u8;
                    // existing writer stores B,G,R then optional A
                    self.framebuffer[off] = b;
                    self.framebuffer[off + 1] = g;
                    self.framebuffer[off + 2] = r;
                    if self.bytes_per_pixel == 4 { self.framebuffer[off + 3] = 255; }
                    off += self.bytes_per_pixel;
                }
            }
        }
    }

    /// Fill entire screen with a color using the tiled renderer
    pub fn clear(&mut self, color: Color) {
        self.draw_rect(0, 0, self.width, self.height, color);
    }

    /// Fill a rectangular region using tiled renderer
    pub fn fill_rect(&mut self, x: i32, y: i32, width: u32, height: u32, color: Color) {
        if width == 0 || height == 0 { return; }
        let x0 = x.max(0) as usize;
        let y0 = y.max(0) as usize;
        let x1 = (x0 as u32 + width) as usize;
        let y1 = (y0 as u32 + height) as usize;
        self.draw_rect(x0, y0, x1, y1, color);
    }

    /// Draw a single character using embedded-graphics path (kept for now)
    pub fn draw_char(&mut self, ch: char, x: i32, y: i32, style: &MonoTextStyle<Rgb888>) {
        let mut buf = [0u8; 4];
        let s = ch.encode_utf8(&mut buf);
        Text::new(s, Point::new(x, y), *style)
            .draw(self)
            .ok();
    }
}

// Implement DrawTarget for embedded-graphics by writing into nodes then blitting via render_frame()
impl DrawTarget for FramebufferWriter {
    type Color = Rgb888;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>
    {
        for Pixel(Point { x, y }, color) in pixels {
            if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
                continue;
            }
            let c = Color::new(color.r(), color.g(), color.b());
            self.put_pixel(x as usize, y as usize, c);
        }
        Ok(())
    }
}

impl OriginDimensions for FramebufferWriter {
    fn size(&self) -> Size {
        Size::new(self.width as u32, self.height as u32)
    }
}

pub static FRAMEBUFFER: Mutex<Option<FramebufferWriter>> = Mutex::new(None);

pub fn init_framebuffer(info: &'static mut BootInfo) {
    let fb = FramebufferWriter::new(info);
    *FRAMEBUFFER.lock() = Some(fb);
}