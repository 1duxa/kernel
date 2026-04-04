use crate::ui_provider::color::Color;

pub struct Theme {
    pub text: Color,
    pub background: Color,
    pub accent: Color,
    pub surface: Color,
    pub border: Color,
    pub muted: Color,
    pub on_accent: Color,
}

impl Theme {
    pub fn new(text: Color, background: Color, accent: Color) -> Self {
        Self {
            text,
            background,
            accent,
            surface: background,
            border: Color::from_hex(0x45475a),
            muted: Color::from_hex(0x6c7086),
            on_accent: Color::from_hex(0x1e1e2e),
        }
    }

    pub fn dark_modern() -> Self {
        Self {
            text: Color::from_hex(0xcdd6f4),
            background: Color::from_hex(0x1e1e2e),
            accent: Color::from_hex(0x89b4fa),
            surface: Color::from_hex(0x313244),
            border: Color::from_hex(0x45475a),
            muted: Color::from_hex(0x6c7086),
            on_accent: Color::from_hex(0x1e1e2e),
        }
    }
}
