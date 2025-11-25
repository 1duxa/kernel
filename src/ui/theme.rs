use crate::devices::framebuffer::color::Color;

/// Modern UI Theme for the OS
#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub primary: Color,
    pub secondary: Color,
    pub background: Color,
    pub surface: Color,
    pub text: Color,
    pub text_secondary: Color,
    pub accent: Color,
    pub error: Color,
    pub success: Color,
    pub warning: Color,
}

impl Theme {
    /// Create a dark modern theme
    pub fn dark_modern() -> Self {
        Self {
            primary: Color::from_hex(0x2196F3),      // Blue
            secondary: Color::from_hex(0x03A9F4),    // Light blue
            background: Color::from_hex(0x121212),   // Dark background
            surface: Color::from_hex(0x1E1E1E),      // Card/surface
            text: Color::from_hex(0xFFFFFF),         // White text
            text_secondary: Color::from_hex(0xB0B0B0), // Gray text
            accent: Color::from_hex(0xFF6B6B),       // Red accent
            error: Color::from_hex(0xF44336),
            success: Color::from_hex(0x4CAF50),
            warning: Color::from_hex(0xFF9800),
        }
    }

    /// Create a light modern theme
    pub fn light_modern() -> Self {
        Self {
            primary: Color::from_hex(0x1976D2),
            secondary: Color::from_hex(0x42A5F5),
            background: Color::from_hex(0xF5F5F5),
            surface: Color::from_hex(0xFFFFFF),
            text: Color::from_hex(0x212121),
            text_secondary: Color::from_hex(0x757575),
            accent: Color::from_hex(0xE91E63),
            error: Color::from_hex(0xD32F2F),
            success: Color::from_hex(0x388E3C),
            warning: Color::from_hex(0xF57C00),
        }
    }
}

