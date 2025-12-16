//! PS/2 Mouse Driver
//!
//! This module implements a PS/2 mouse driver that handles IRQ12 interrupts
//! and decodes 3-byte mouse packets into movement and button events.
//!
//! # Architecture
//! - Ring buffer for storing raw mouse bytes from IRQ handler
//! - Packet decoder for converting 3-byte sequences to MouseEvent
//! - Global decoder instance for use by the kernel
//!
//! # Usage
//! ```ignore
//! // In IRQ12 handler:
//! ps2_mouse::enqueue_mouse_byte(byte);
//!
//! // In polling loop:
//! if let Some(event) = ps2_mouse::poll_mouse_event() {
//!     // Handle mouse movement/clicks
//! }
//! ```

use core::sync::atomic::{AtomicU8, AtomicBool, Ordering};
use spin::Mutex;
use x86_64::instructions::port::Port;

// =============================================================================
// RING BUFFER FOR RAW MOUSE BYTES
// =============================================================================

const BUFFER_SIZE: usize = 256;

/// Ring buffer for mouse bytes (lock-free SPSC queue)
static mut MOUSE_BUF: [u8; BUFFER_SIZE] = [0; BUFFER_SIZE];
static MOUSE_HEAD: AtomicU8 = AtomicU8::new(0);
static MOUSE_TAIL: AtomicU8 = AtomicU8::new(0);

/// Mouse initialization state
static MOUSE_INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Enqueue a mouse byte from the IRQ handler (producer side)
/// 
/// This function is called from the IRQ12 handler and must be fast.
/// Uses relaxed/release ordering for lock-free operation.
#[inline]
pub fn enqueue_mouse_byte(byte: u8) {
    let head = MOUSE_HEAD.load(Ordering::Relaxed) as usize;
    let next = (head + 1) % BUFFER_SIZE;
    let tail = MOUSE_TAIL.load(Ordering::Acquire) as usize;
    
    // Drop byte if buffer is full (don't block in IRQ handler)
    if next != tail {
        unsafe {
            MOUSE_BUF[head] = byte;
        }
        MOUSE_HEAD.store(next as u8, Ordering::Release);
    }
}

/// Dequeue a mouse byte (consumer side)
fn dequeue_mouse_byte() -> Option<u8> {
    let tail = MOUSE_TAIL.load(Ordering::Relaxed) as usize;
    let head = MOUSE_HEAD.load(Ordering::Acquire) as usize;
    
    if tail == head {
        None
    } else {
        let byte = unsafe { MOUSE_BUF[tail] };
        let next = (tail + 1) % BUFFER_SIZE;
        MOUSE_TAIL.store(next as u8, Ordering::Release);
        Some(byte)
    }
}

// =============================================================================
// MOUSE EVENT
// =============================================================================

/// Represents a decoded mouse event with movement and button states
#[derive(Debug, Clone, Copy, Default)]
pub struct MouseEvent {
    /// Relative X movement (-128 to 127)
    pub dx: i16,
    /// Relative Y movement (-128 to 127, positive = up)
    pub dy: i16,
    /// Button states: bit 0 = left, bit 1 = right, bit 2 = middle
    pub buttons: u8,
}

impl MouseEvent {
    /// Check if left button is pressed
    #[inline]
    pub fn left_button(&self) -> bool {
        (self.buttons & 0x01) != 0
    }

    /// Check if right button is pressed
    #[inline]
    pub fn right_button(&self) -> bool {
        (self.buttons & 0x02) != 0
    }

    /// Check if middle button is pressed
    #[inline]
    pub fn middle_button(&self) -> bool {
        (self.buttons & 0x04) != 0
    }
}

// =============================================================================
// PACKET DECODER
// =============================================================================

/// PS/2 mouse packet decoder
/// 
/// Standard PS/2 mice send 3-byte packets:
/// - Byte 0: Status (buttons, sign bits, overflow bits)
/// - Byte 1: X movement
/// - Byte 2: Y movement
pub struct MouseDecoder {
    buffer: [u8; 3],
    index: usize,
}

impl MouseDecoder {
    pub const fn new() -> Self {
        Self {
            buffer: [0; 3],
            index: 0,
        }
    }

    /// Process a byte and return an event if a complete packet is decoded
    pub fn process_byte(&mut self, byte: u8) -> Option<MouseEvent> {
        // First byte must have bit 3 set (always 1 in standard PS/2 protocol)
        // This helps with synchronization
        if self.index == 0 && (byte & 0x08) == 0 {
            // Invalid first byte, skip (resync)
            return None;
        }

        self.buffer[self.index] = byte;
        self.index += 1;

        if self.index >= 3 {
            self.index = 0;
            Some(self.decode_packet())
        } else {
            None
        }
    }

    /// Reset decoder state (useful for resynchronization)
    pub fn reset(&mut self) {
        self.index = 0;
    }

    fn decode_packet(&self) -> MouseEvent {
        let status = self.buffer[0];
        let x_raw = self.buffer[1];
        let y_raw = self.buffer[2];

        // Extract sign bits from status byte
        let x_sign = (status & 0x10) != 0;
        let y_sign = (status & 0x20) != 0;
        
        // Check overflow bits
        let x_overflow = (status & 0x40) != 0;
        let y_overflow = (status & 0x80) != 0;

        // Convert to signed values with sign extension
        let dx = if x_overflow {
            if x_sign { -256i16 } else { 255i16 }
        } else if x_sign {
            x_raw as i16 - 256
        } else {
            x_raw as i16
        };

        let dy = if y_overflow {
            if y_sign { -256i16 } else { 255i16 }
        } else if y_sign {
            y_raw as i16 - 256
        } else {
            y_raw as i16
        };

        MouseEvent {
            dx,
            dy: -dy, // Invert Y so positive = up (screen coordinates)
            buttons: status & 0x07,
        }
    }
}

// =============================================================================
// GLOBAL DECODER
// =============================================================================

static DECODER: Mutex<MouseDecoder> = Mutex::new(MouseDecoder::new());

/// Poll for a mouse event
/// 
/// Processes any pending bytes in the ring buffer and returns
/// a MouseEvent when a complete packet is decoded.
pub fn poll_mouse_event() -> Option<MouseEvent> {
    let mut decoder = DECODER.lock();
    
    while let Some(byte) = dequeue_mouse_byte() {
        if let Some(event) = decoder.process_byte(byte) {
            return Some(event);
        }
    }
    
    None
}

/// Check if mouse is initialized
pub fn is_initialized() -> bool {
    MOUSE_INITIALIZED.load(Ordering::Relaxed)
}

// =============================================================================
// INITIALIZATION
// =============================================================================

/// Wait for PS/2 controller input buffer to be ready (can write)
fn wait_for_write() -> Result<(), &'static str> {
    for _ in 0..100_000 {
        let status = unsafe { Port::<u8>::new(0x64).read() };
        if (status & 0x02) == 0 {
            return Ok(());
        }
    }
    Err("PS/2 controller write timeout")
}

/// Wait for PS/2 controller output buffer to have data (can read)
fn wait_for_read() -> Result<(), &'static str> {
    for _ in 0..100_000 {
        let status = unsafe { Port::<u8>::new(0x64).read() };
        if (status & 0x01) != 0 {
            return Ok(());
        }
    }
    Err("PS/2 controller read timeout")
}

/// Send command to PS/2 controller
fn send_controller_command(cmd: u8) -> Result<(), &'static str> {
    wait_for_write()?;
    unsafe { Port::<u8>::new(0x64).write(cmd); }
    Ok(())
}

/// Send data to PS/2 data port
fn send_data(data: u8) -> Result<(), &'static str> {
    wait_for_write()?;
    unsafe { Port::<u8>::new(0x60).write(data); }
    Ok(())
}

/// Read data from PS/2 data port
fn read_data() -> Result<u8, &'static str> {
    wait_for_read()?;
    Ok(unsafe { Port::<u8>::new(0x60).read() })
}

/// Send command to mouse (via auxiliary device)
fn send_mouse_command(cmd: u8) -> Result<u8, &'static str> {
    // Tell controller next byte goes to mouse
    send_controller_command(0xD4)?;
    send_data(cmd)?;
    
    // Wait for ACK (0xFA)
    let response = read_data()?;
    if response != 0xFA {
        return Err("Mouse did not ACK command");
    }
    Ok(response)
}

/// Initialize PS/2 mouse
/// 
/// This function enables the auxiliary (mouse) port on the PS/2 controller
/// and configures the mouse to start sending movement data.
pub fn init() -> Result<(), &'static str> {
    // Step 1: Enable auxiliary device (mouse port)
    send_controller_command(0xA8)?;
    
    // Step 2: Read controller configuration byte
    send_controller_command(0x20)?;
    let config = read_data()?;
    
    // Step 3: Enable mouse interrupt (bit 1) and mouse clock (bit 5 = 0)
    let new_config = (config | 0x02) & !0x20;
    send_controller_command(0x60)?;
    send_data(new_config)?;
    
    // Step 4: Set mouse defaults
    send_mouse_command(0xF6)?; // Set defaults
    
    // Step 5: Enable mouse data reporting
    send_mouse_command(0xF4)?; // Enable
    
    // Reset decoder state
    DECODER.lock().reset();
    
    MOUSE_INITIALIZED.store(true, Ordering::Release);
    
    Ok(())
}
