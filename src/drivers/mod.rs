pub mod ps2_keyboard;
pub mod ps2_mouse;

pub use ps2_keyboard::{KeyEvent, ScancodeDecoder};
pub use ps2_mouse::{MouseEvent, MouseDecoder, init_mouse};
