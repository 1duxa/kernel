//! Framebuffer writer using embedded-graphics
use bootloader_api::BootInfo;
use crate::framebuffer::color::Color;
use spin::Mutex;
use embedded_graphics::{
    prelude::*,
    pixelcolor::Rgb888,
    primitives::{Rectangle, PrimitiveStyle},
    mono_font::MonoTextStyle,
    text::Text,
    Drawable,
};

pub struct FramebufferWriter {
    framebuffer: &'static mut [u8],
    pub width: usize,
    pub height: usize,
    pub stride: usize,
    pub bytes_per_pixel: usize,
}

impl FramebufferWriter {
    pub fn new(info: &'static mut BootInfo) -> Self {
        let fb = info.framebuffer.as_mut().unwrap();
        let info = fb.info();
        
        Self {
            framebuffer: fb.buffer_mut(),
            width: info.width,
            height: info.height,
            stride: info.stride,
            bytes_per_pixel: info.bytes_per_pixel,
        }
    }

    /// Fill entire screen with a color
    pub fn clear(&mut self, color: Color) {
        let rect = Rectangle::new(
            Point::zero(),
            Size::new(self.width as u32, self.height as u32)
        );
        rect.into_styled(PrimitiveStyle::with_fill(color.to_rgb888()))
            .draw(self)
            .ok();
    }

    /// Fill a rectangular region
    pub fn fill_rect(&mut self, x: i32, y: i32, width: u32, height: u32, color: Color) {
        let rect = Rectangle::new(
            Point::new(x, y),
            Size::new(width, height)
        );
        rect.into_styled(PrimitiveStyle::with_fill(color.to_rgb888()))
            .draw(self)
            .ok();
    }

    /// Draw a single character at pixel coordinates
    pub fn draw_char(&mut self, ch: char, x: i32, y: i32, style: &MonoTextStyle<Rgb888>) {
        let mut buf = [0u8; 4];
        let s = ch.encode_utf8(&mut buf);
        Text::new(s, Point::new(x, y), *style)
            .draw(self)
            .ok();
    }
}

// Implement DrawTarget for embedded-graphics
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
            
            let offset = (y as usize * self.stride + x as usize) * self.bytes_per_pixel;
            if offset + self.bytes_per_pixel <= self.framebuffer.len() {
                self.framebuffer[offset] = color.b();
                self.framebuffer[offset + 1] = color.g();
                self.framebuffer[offset + 2] = color.r();
                if self.bytes_per_pixel == 4 {
                    self.framebuffer[offset + 3] = 255;
                }
            }
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