use crate::ui_provider::color::Color;

pub struct Theme {
    pub text: Color,
    pub background: Color,
    pub accent: Color,
}
impl Theme {
    pub fn new(text: Color, background: Color, accent: Color) -> Self {
        Self {
            text,
            background,
            accent,
        }
    }
    pub fn dark_modern() -> Self {
        Self::new(Color::WHITE, Color::BLACK, Color::YELLOW)
    }
}
