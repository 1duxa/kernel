//! # PS/2 Keyboard Driver
//!
//! Handles PS/2 keyboard input via IRQ1 interrupt.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────┐    IRQ1     ┌──────────────┐
//! │  Keyboard   │────────────▶│  Ring Buffer │
//! │  (Port 60)  │             │  (256 bytes) │
//! └─────────────┘             └──────┬───────┘
//!                                    │
//!                                    ▼
//!                         ┌──────────────────┐
//!                         │ ScancodeDecoder  │
//!                         │ - Extended keys  │
//!                         │ - Modifiers      │
//!                         │ - Shift mapping  │
//!                         └────────┬─────────┘
//!                                  │
//!                                  ▼
//!                              KeyEvent
//! ```
//!
//! ## Scancode Processing
//!
//! 1. IRQ1 handler reads scancode from port 0x60
//! 2. Scancode enqueued to lock-free ring buffer
//! 3. `ScancodeDecoder` processes scancodes:
//!    - 0xE0 prefix for extended keys (arrows)
//!    - Bit 7 indicates key release
//!    - Modifier tracking (Shift, Ctrl, Alt)
//! 4. Returns `KeyEvent` with character and modifiers
//!
//! ## Usage
//!
//! ```ignore
//! // In interrupt handler:
//! let scancode = unsafe { Port::<u8>::new(0x60).read() };
//! enqueue_scancode(scancode);
//!
//! // In main loop:
//! if let Some(sc) = dequeue_scancode() {
//!     if let Some(event) = decoder.process_scancode(sc) {
//!         // Handle key event
//!     }
//! }
//! ```

use core::sync::atomic::{AtomicUsize, Ordering};

const BUFFER_SIZE: usize = 256;

static mut RING_BUF: [u8; BUFFER_SIZE] = [0; BUFFER_SIZE];
static HEAD: AtomicUsize = AtomicUsize::new(0);
static TAIL: AtomicUsize = AtomicUsize::new(0);

pub fn enqueue_scancode(scancode: u8) {
    let head = HEAD.load(Ordering::Relaxed);
    let next = head.wrapping_add(1) % BUFFER_SIZE;
    let tail = TAIL.load(Ordering::Acquire);
    if next != tail {
        unsafe {
            RING_BUF[head] = scancode;
        }
        HEAD.store(next, Ordering::Release);
    }
}

pub fn dequeue_scancode() -> Option<u8> {
    let tail = TAIL.load(Ordering::Relaxed);
    let head = HEAD.load(Ordering::Acquire);
    if tail == head {
        None
    } else {
        let sc = unsafe { RING_BUF[tail] };
        let next = tail.wrapping_add(1) % BUFFER_SIZE;
        TAIL.store(next, Ordering::Release);
        Some(sc)
    }
}

pub struct ScancodeDecoder {
    is_extended: bool,
    shift_pressed: bool,
    ctrl_pressed: bool,
    alt_pressed: bool,
}

impl ScancodeDecoder {
    pub const fn new() -> Self {
        Self {
            is_extended: false,
            shift_pressed: false,
            ctrl_pressed: false,
            alt_pressed: false,
        }
    }

    pub fn process_scancode(&mut self, scancode: u8) -> Option<KeyEvent> {
        // If the scancode is the extended prefix, set flag and wait for next byte.
        if scancode == 0xE0 {
            self.is_extended = true;
            return None;
        }

        let is_release = scancode & 0x80 != 0;
        let key_code = scancode & 0x7F;

        // If we've seen the extended prefix, handle extended keys (arrows, etc.)
        if self.is_extended {
            // consume the extended prefix state regardless of what follows
            self.is_extended = false;

            // ignore release events for arrows (optional: you could track key up if you want)
            if is_release {
                return None;
            }

            match key_code {
                0x48 => {
                    return Some(KeyEvent {
                        character: '\0',
                        ctrl: self.ctrl_pressed,
                        alt: self.alt_pressed,
                        shift: self.shift_pressed,
                        is_arrow: true,
                        arrow_direction: Some(crate::app::Arrow::Up),
                    });
                }
                0x50 => {
                    return Some(KeyEvent {
                        character: '\0',
                        ctrl: self.ctrl_pressed,
                        alt: self.alt_pressed,
                        shift: self.shift_pressed,
                        is_arrow: true,
                        arrow_direction: Some(crate::app::Arrow::Down),
                    });
                }
                0x4B => {
                    return Some(KeyEvent {
                        character: '\0',
                        ctrl: self.ctrl_pressed,
                        alt: self.alt_pressed,
                        shift: self.shift_pressed,
                        is_arrow: true,
                        arrow_direction: Some(crate::app::Arrow::Left),
                    });
                }
                0x4D => {
                    return Some(KeyEvent {
                        character: '\0',
                        ctrl: self.ctrl_pressed,
                        alt: self.alt_pressed,
                        shift: self.shift_pressed,
                        is_arrow: true,
                        arrow_direction: Some(crate::app::Arrow::Right),
                    });
                }
                // you can add more extended keys here (home/end/insert/etc.)
                _ => {
                    // unknown extended — ignore
                    return None;
                }
            }
        }

        // Handle common modifier keys (non-extended)
        match key_code {
            0x2A | 0x36 => {
                // Left/Right Shift
                self.shift_pressed = !is_release;
                return None;
            }
            0x1D => {
                // Ctrl
                self.ctrl_pressed = !is_release;
                return None;
            }
            0x38 => {
                // Alt
                self.alt_pressed = !is_release;
                return None;
            }
            _ => {}
        }

        // For regular keys: if this is a key release, ignore it
        if is_release {
            return None;
        }

        let ch = self.scancode_to_char(key_code);

        // return normal character events (not arrows)
        ch.map(|c| KeyEvent {
            character: c,
            ctrl: self.ctrl_pressed,
            alt: self.alt_pressed,
            shift: self.shift_pressed,
            is_arrow: false,
            arrow_direction: None,
        })
    }

    fn scancode_to_char(&self, scancode: u8) -> Option<char> {
        // (keep your existing mapping - unchanged)
        let ch = match scancode {
            0x02..=0x0B => {
                // Number row: 1-9, 0
                let digit = if scancode == 0x0B { '0' } else { (scancode - 0x02 + b'1') as char };
                if self.shift_pressed {
                    match digit {
                        '1' => '!', '2' => '@', '3' => '#', '4' => '$', '5' => '%',
                        '6' => '^', '7' => '&', '8' => '*', '9' => '(', '0' => ')',
                        _ => digit,
                    }
                } else {
                    digit
                }
            }
            0x10 => if self.shift_pressed { 'Q' } else { 'q' },
            0x11 => if self.shift_pressed { 'W' } else { 'w' },
            0x12 => if self.shift_pressed { 'E' } else { 'e' },
            0x13 => if self.shift_pressed { 'R' } else { 'r' },
            0x14 => if self.shift_pressed { 'T' } else { 't' },
            0x15 => if self.shift_pressed { 'Y' } else { 'y' },
            0x16 => if self.shift_pressed { 'U' } else { 'u' },
            0x17 => if self.shift_pressed { 'I' } else { 'i' },
            0x18 => if self.shift_pressed { 'O' } else { 'o' },
            0x19 => if self.shift_pressed { 'P' } else { 'p' },
            0x1E => if self.shift_pressed { 'A' } else { 'a' },
            0x1F => if self.shift_pressed { 'S' } else { 's' },
            0x20 => if self.shift_pressed { 'D' } else { 'd' },
            0x21 => if self.shift_pressed { 'F' } else { 'f' },
            0x22 => if self.shift_pressed { 'G' } else { 'g' },
            0x23 => if self.shift_pressed { 'H' } else { 'h' },
            0x24 => if self.shift_pressed { 'J' } else { 'j' },
            0x25 => if self.shift_pressed { 'K' } else { 'k' },
            0x26 => if self.shift_pressed { 'L' } else { 'l' },
            0x2C => if self.shift_pressed { 'Z' } else { 'z' },
            0x2D => if self.shift_pressed { 'X' } else { 'x' },
            0x2E => if self.shift_pressed { 'C' } else { 'c' },
            0x2F => if self.shift_pressed { 'V' } else { 'v' },
            0x30 => if self.shift_pressed { 'B' } else { 'b' },
            0x31 => if self.shift_pressed { 'N' } else { 'n' },
            0x32 => if self.shift_pressed { 'M' } else { 'm' },

            0x39 => ' ',  // Space
            0x1C => '\n', // Enter
            0x0E => '\x08', // Backspace
            0x0F => '\t', // Tab

            0x1A => if self.shift_pressed { '{' } else { '[' },
            0x1B => if self.shift_pressed { '}' } else { ']' },
            0x27 => if self.shift_pressed { ':' } else { ';' },
            0x28 => if self.shift_pressed { '"' } else { '\'' },
            0x29 => if self.shift_pressed { '~' } else { '`' },
            0x2B => if self.shift_pressed { '|' } else { '\\' },
            0x33 => if self.shift_pressed { '<' } else { ',' },
            0x34 => if self.shift_pressed { '>' } else { '.' },
            0x35 => if self.shift_pressed { '?' } else { '/' },
            0x0C => if self.shift_pressed { '_' } else { '-' },
            0x0D => if self.shift_pressed { '+' } else { '=' },

            _ => return None,
        };

        Some(ch)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct KeyEvent {
    pub character: char,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub is_arrow: bool,
    /// Arrow direction (Some) if `is_arrow == true`, otherwise None
    pub arrow_direction: Option<crate::app::Arrow>,
}
