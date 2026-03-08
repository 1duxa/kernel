//! Color representation and manipulation
use embedded_graphics::pixelcolor::Rgb888;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const BLACK: Color = Color {
        r: 0,
        g: 0,
        b: 0,
        a: 255,
    };
    pub const WHITE: Color = Color {
        r: 255,
        g: 255,
        b: 255,
        a: 255,
    };
    pub const RED: Color = Color {
        r: 255,
        g: 0,
        b: 0,
        a: 255,
    };
    pub const GREEN: Color = Color {
        r: 0,
        g: 255,
        b: 0,
        a: 255,
    };
    pub const BLUE: Color = Color {
        r: 0,
        g: 0,
        b: 255,
        a: 255,
    };
    pub const YELLOW: Color = Color {
        r: 255,
        g: 255,
        b: 0,
        a: 255,
    };
    pub const CYAN: Color = Color {
        r: 0,
        g: 255,
        b: 255,
        a: 255,
    };
    pub const MAGENTA: Color = Color {
        r: 255,
        g: 0,
        b: 255,
        a: 255,
    };
    pub const GRAY: Color = Color {
        r: 128,
        g: 128,
        b: 128,
        a: 255,
    };
    pub const DARK_GRAY: Color = Color {
        r: 64,
        g: 64,
        b: 64,
        a: 255,
    };
    pub const LIGHT_GRAY: Color = Color {
        r: 192,
        g: 192,
        b: 192,
        a: 255,
    };

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

    /// Blend this color with another color using alpha compositing
    pub fn blend(&self, other: &Color) -> Color {
        let alpha = other.a as f32 / 255.0;
        let inv_alpha = 1.0 - alpha;

        Color::new(
            ((self.r as f32 * inv_alpha) + (other.r as f32 * alpha)) as u8,
            ((self.g as f32 * inv_alpha) + (other.g as f32 * alpha)) as u8,
            ((self.b as f32 * inv_alpha) + (other.b as f32 * alpha)) as u8,
        )
    }

    /// Darken the color by a factor (0.0 = no change, 1.0 = black)
    pub fn darken(&self, factor: f32) -> Color {
        let factor = factor.min(1.0).max(0.0);
        Color::new(
            (self.r as f32 * (1.0 - factor)) as u8,
            (self.g as f32 * (1.0 - factor)) as u8,
            (self.b as f32 * (1.0 - factor)) as u8,
        )
    }

    /// Lighten the color by a factor (0.0 = no change, 1.0 = white)
    pub fn lighten(&self, factor: f32) -> Color {
        let factor = factor.min(1.0).max(0.0);
        Color::new(
            (self.r as u16 + ((255 - self.r as u16) as f32 * factor) as u16).min(255) as u8,
            (self.g as u16 + ((255 - self.g as u16) as f32 * factor) as u16).min(255) as u8,
            (self.b as u16 + ((255 - self.b as u16) as f32 * factor) as u16).min(255) as u8,
        )
    }

    /// Mix two colors with a ratio (0.0 = self, 1.0 = other)
    pub fn mix(&self, other: &Color, ratio: f32) -> Color {
        let ratio = ratio.min(1.0).max(0.0);
        let inv_ratio = 1.0 - ratio;
        Color::new(
            ((self.r as f32 * inv_ratio) + (other.r as f32 * ratio)) as u8,
            ((self.g as f32 * inv_ratio) + (other.g as f32 * ratio)) as u8,
            ((self.b as f32 * inv_ratio) + (other.b as f32 * ratio)) as u8,
        )
    }
    pub fn to_rgb888(self) -> Rgb888 {
        Rgb888::new(self.r, self.g, self.b)
    }


}

