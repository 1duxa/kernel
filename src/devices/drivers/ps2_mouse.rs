//! PS/2 Mouse Driver

use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use spin::Mutex;
use x86_64::instructions::port::Port;

// =============================================================================
// RING BUFFER FOR RAW MOUSE BYTES
// =============================================================================

const BUFFER_SIZE: usize = 256;

static mut MOUSE_BUF: [u8; BUFFER_SIZE] = [0; BUFFER_SIZE];
static MOUSE_HEAD: AtomicU8 = AtomicU8::new(0);
static MOUSE_TAIL: AtomicU8 = AtomicU8::new(0);
static MOUSE_INITIALIZED: AtomicBool = AtomicBool::new(false);

#[inline]
pub fn enqueue_mouse_byte(byte: u8) {
    let head = MOUSE_HEAD.load(Ordering::Relaxed) as usize;
    let next = (head + 1) % BUFFER_SIZE;
    let tail = MOUSE_TAIL.load(Ordering::Acquire) as usize;

    if next != tail {
        unsafe {
            MOUSE_BUF[head] = byte;
        }
        MOUSE_HEAD.store(next as u8, Ordering::Release);
    }
}

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

#[derive(Debug, Clone, Copy, Default)]
pub struct MouseEvent {
    pub dx: i16,
    pub dy: i16,
    pub buttons: u8,
}

impl MouseEvent {
    #[inline]
    pub fn left_button(&self) -> bool {
        (self.buttons & 0x01) != 0
    }

    #[inline]
    pub fn right_button(&self) -> bool {
        (self.buttons & 0x02) != 0
    }

    #[inline]
    pub fn middle_button(&self) -> bool {
        (self.buttons & 0x04) != 0
    }
}

// =============================================================================
// PACKET DECODER
// =============================================================================

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

    pub fn process_byte(&mut self, byte: u8) -> Option<MouseEvent> {
        if self.index == 0 && (byte & 0x08) == 0 {
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

        let dx = if x_overflow {
            if x_sign {
                -256i16
            } else {
                255i16
            }
        } else if x_sign {
            x_raw as i16 - 256
        } else {
            x_raw as i16
        };

        let dy = if y_overflow {
            if y_sign {
                -256i16
            } else {
                255i16
            }
        } else if y_sign {
            y_raw as i16 - 256
        } else {
            y_raw as i16
        };

        MouseEvent {
            dx,
            dy: -dy,
            buttons: status & 0x07,
        }
    }
}

// =============================================================================
// GLOBAL DECODER
// =============================================================================

static DECODER: Mutex<MouseDecoder> = Mutex::new(MouseDecoder::new());

pub fn poll_mouse_event() -> Option<MouseEvent> {
    let mut decoder = DECODER.lock();

    while let Some(byte) = dequeue_mouse_byte() {
        if let Some(event) = decoder.process_byte(byte) {
            return Some(event);
        }
    }

    None
}

pub fn is_initialized() -> bool {
    MOUSE_INITIALIZED.load(Ordering::Relaxed)
}

// =============================================================================
// INITIALIZATION
// =============================================================================

fn wait_for_write() -> Result<(), &'static str> {
    for _ in 0..100_000 {
        let status = unsafe { Port::<u8>::new(0x64).read() };
        if (status & 0x02) == 0 {
            return Ok(());
        }
        core::hint::spin_loop();
    }
    Err("PS/2 controller write timeout")
}

fn wait_for_read() -> Result<(), &'static str> {
    for _ in 0..100_000 {
        let status = unsafe { Port::<u8>::new(0x64).read() };
        if (status & 0x01) != 0 {
            return Ok(());
        }
        core::hint::spin_loop();
    }
    Err("PS/2 controller read timeout")
}

fn flush_output_buffer() {
    for _ in 0..100 {
        let status = unsafe { Port::<u8>::new(0x64).read() };
        if (status & 0x01) == 0 {
            break;
        }
        let _ = unsafe { Port::<u8>::new(0x60).read() };
        for _ in 0..100 {
            core::hint::spin_loop();
        }
    }
}

fn send_controller_command(cmd: u8) -> Result<(), &'static str> {
    wait_for_write()?;
    unsafe {
        Port::<u8>::new(0x64).write(cmd);
    }
    Ok(())
}

/// Send data to PS/2 data port
fn send_data(data: u8) -> Result<(), &'static str> {
    wait_for_write()?;
    unsafe {
        Port::<u8>::new(0x60).write(data);
    }
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

    // Wait for response with retry
    for _ in 0..3 {
        if let Ok(response) = read_data() {
            if response == 0xFA {
                return Ok(response);
            }
            // Resend if not ACK
            if response == 0xFE {
                send_controller_command(0xD4)?;
                send_data(cmd)?;
                continue;
            }
        }
    }
    Err("Mouse did not ACK command")
}

/// Initialize PS/2 mouse
///
/// This function enables the auxiliary (mouse) port on the PS/2 controller
/// and configures the mouse to start sending movement data.
pub fn init() -> Result<(), &'static str> {
    // Disable devices
    send_controller_command(0xAD)?; // disable keyboard
    send_controller_command(0xA7)?; // disable mouse

    flush_output_buffer();

    // Controller self-test
    send_controller_command(0xAA)?;
    let result = read_data()?;
    if result != 0x55 {
        return Err("PS/2 controller self-test failed");
    }
    // Flush any pending data first
    flush_output_buffer();

    // Step 1: Enable auxiliary device (mouse port)
    send_controller_command(0xA8)?;

    // Small delay after enabling
    for _ in 0..10000 {
        core::hint::spin_loop();
    }

    // Step 2: Read controller configuration byte
    send_controller_command(0x20)?;
    let config = read_data()?;

    // Step 3: Enable mouse interrupt (bit 1) and mouse clock (bit 5 = 0)
    let new_config = (config | 0x02) & !0x20;
    send_controller_command(0x60)?;
    send_data(new_config)?;

    // Small delay
    for _ in 0..10000 {
        core::hint::spin_loop();
    }

    // Step 4: Set mouse defaults
    if send_mouse_command(0xF6).is_err() {
        // Try once more
        flush_output_buffer();
        send_mouse_command(0xF6)?;
    }

    // Step 5: Enable mouse data reporting
    send_mouse_command(0xF4)?; // Enable

    // Reset decoder state
    DECODER.lock().reset();
    send_controller_command(0xAE)?;
    MOUSE_INITIALIZED.store(true, Ordering::Release);

    // enable keyboard interrupt (irq1)
    unsafe {
        use x86_64::instructions::port::Port;
        let mut pic1_data = Port::<u8>::new(0x21);
        let mask: u8 = pic1_data.read();
        let new_mask = mask & !(1 << 1); // enable irq1 (keyboard)
        pic1_data.write(new_mask);
    }

    // Debug: print current PIC masks (master=0x21, slave=0xA1) to serial/framebuffer
    unsafe {
        use x86_64::instructions::port::Port;
        let mut m = Port::<u8>::new(0x21);
        let mut s = Port::<u8>::new(0xA1);
        let master_mask = m.read();
        let slave_mask = s.read();
        crate::println!(
            "PIC masks after unmask: master=0x{:02x} slave=0x{:02x}",
            master_mask,
            slave_mask
        );
    }

    crate::println!("3");

    Ok(())
}
