//! # OS Terminal Integration
use alloc::boxed::Box;
use os_terminal::font::BitmapFont;
use os_terminal::{DrawTarget, Rgb, Terminal};

use crate::devices::framebuffer::framebuffer::FramebufferWriter;
use crate::ui_provider::color::Color;

pub struct DisplayConfig {
    pub fb: *mut FramebufferWriter,
    pub x_offset: usize,
    pub y_offset: usize,
    pub width: usize,
    pub height: usize,
}

unsafe impl Send for DisplayConfig {}

pub struct FbDisplay {
    config: *mut DisplayConfig,
}

unsafe impl Send for FbDisplay {}

impl DrawTarget for FbDisplay {
    fn size(&self) -> (usize, usize) {
        let cfg = unsafe { &*self.config };
        (cfg.width, cfg.height)
    }

    #[inline(always)]
    fn draw_pixel(&mut self, x: usize, y: usize, color: Rgb) {
        let cfg = unsafe { &mut *self.config };
        if cfg.fb.is_null() {
            return;
        }
        let fb = unsafe { &mut *cfg.fb };
        let c = Color::new(color.0, color.1, color.2);
        fb.put_pixel(cfg.x_offset + x, cfg.y_offset + y, c);
    }
}

pub struct OsTerminal {
    config: Box<DisplayConfig>,
    inner: Terminal<FbDisplay>,
    first_render: bool,
}

impl OsTerminal {
    pub fn new(width: usize, height: usize) -> Self {
        let mut config = Box::new(DisplayConfig {
            fb: core::ptr::null_mut(),
            x_offset: 0,
            y_offset: 0,
            width,
            height,
        });

        let config_ptr: *mut DisplayConfig = &mut *config;

        let display = FbDisplay { config: config_ptr };

        let mut inner = Terminal::new(display);
        inner.set_font_manager(Box::new(BitmapFont));
        inner.set_auto_flush(false);
        inner.set_crnl_mapping(true);

        OsTerminal {
            config,
            inner,
            first_render: true,
        }
    }

    pub fn set_offset(&mut self, x: usize, y: usize) {
        self.config.x_offset = x;
        self.config.y_offset = y;
    }

    pub fn size(&self) -> (usize, usize) {
        (self.inner.columns(), self.inner.rows())
    }

    // -----------------------------------------------------------------------
    // Text input
    // -----------------------------------------------------------------------

    pub fn process(&mut self, data: &[u8]) {
        self.inner.process(data);
    }

    pub fn write(&mut self, s: &str) {
        self.inner.process(s.as_bytes());
    }

    // -----------------------------------------------------------------------
    // Rendering
    // -----------------------------------------------------------------------

    pub fn render(&mut self, fb: &mut FramebufferWriter) {
        self.config.fb = fb as *mut FramebufferWriter;

        if self.first_render {
            self.first_render = false;

            self.inner.set_color_scheme(0);
        } else {
            self.inner.flush();
        }

        self.config.fb = core::ptr::null_mut();
    }

    pub fn invalidate(&mut self) {
        self.first_render = true;
    }
}
