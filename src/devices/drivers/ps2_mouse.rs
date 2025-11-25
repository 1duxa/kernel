use core::sync::atomic::{AtomicU8, Ordering};
// TODO: Unused, make it working
/// PS/2 Mouse Driver
/// Handles IRQ12 (Mouse on secondary PIC, IRQ4)

const BUFFER_SIZE: usize = 256;
static mut MOUSE_BUF: [u8; BUFFER_SIZE] = [0; BUFFER_SIZE];
static MOUSE_HEAD: AtomicU8 = AtomicU8::new(0);
static MOUSE_TAIL: AtomicU8 = AtomicU8::new(0);

/// Enqueue mouse data byte (called from IRQ handler)
pub fn enqueue_mouse_byte(byte: u8) {
    let head = MOUSE_HEAD.load(Ordering::Relaxed) as usize;
    let next = (head.wrapping_add(1)) % BUFFER_SIZE;
    let tail = MOUSE_TAIL.load(Ordering::Acquire) as usize;
    
    if next != tail {
        unsafe {
            MOUSE_BUF[head] = byte;
        }
        MOUSE_HEAD.store(next as u8, Ordering::Release);
    }
}

/// Dequeue mouse data byte
pub fn dequeue_mouse_byte() -> Option<u8> {
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

/// Mouse event
#[derive(Debug, Clone, Copy)]
pub struct MouseEvent {
    pub x: i16,      // Relative X movement
    pub y: i16,      // Relative Y movement
    pub buttons: u8, // Button states: bit 0 = left, bit 1 = right, bit 2 = middle
}

impl MouseEvent {
    pub fn is_left_pressed(&self) -> bool {
        (self.buttons & 0x01) != 0
    }

    pub fn is_right_pressed(&self) -> bool {
        (self.buttons & 0x02) != 0
    }

    pub fn is_middle_pressed(&self) -> bool {
        (self.buttons & 0x04) != 0
    }
}

/// PS/2 Mouse packet decoder (3-byte packets)
pub struct MouseDecoder {
    packet_buffer: [u8; 3],
    packet_index: usize,
}

impl MouseDecoder {
    pub const fn new() -> Self {
        Self {
            packet_buffer: [0; 3],
            packet_index: 0,
        }
    }

    /// Process a byte from the mouse and return an event if a complete packet is received
    pub fn process_byte(&mut self, byte: u8) -> Option<MouseEvent> {
        // First byte always has bit 3 set (sync bit)
        if self.packet_index == 0 {
            if (byte & 0x08) == 0 {
                // Not a valid first byte, reset
                return None;
            }
        }

        self.packet_buffer[self.packet_index] = byte;
        self.packet_index += 1;

        if self.packet_index >= 3 {
            self.packet_index = 0;
            Some(self.decode_packet())
        } else {
            None
        }
    }

    fn decode_packet(&self) -> MouseEvent {
        let status = self.packet_buffer[0];
        let dx = self.packet_buffer[1] as i8 as i16;
        let dy = -(self.packet_buffer[2] as i8 as i16); // Y is inverted

        // Button states: bit 0 = left, bit 1 = right, bit 2 = middle
        let buttons = status & 0x07;

        MouseEvent {
            x: dx as i16,
            y: dy as i16,
            buttons,
        }
    }
}

/// Initialize PS/2 mouse
pub unsafe fn init_mouse() -> Result<(), &'static str> {
    use x86_64::instructions::port::Port;

    // Enable mouse by writing to auxiliary device (0x64 is status, 0x60 is data)
    let mut status_port = Port::<u8>::new(0x64);
    let mut data_port = Port::<u8>::new(0x60);

    // Wait for input buffer to be empty
    loop {
        if (status_port.read() & 0x02) == 0 {
            break;
        }
    }

    // Enable auxiliary device (mouse)
    status_port.write(0xA8); // Enable auxiliary device command

    // Wait again
    loop {
        if (status_port.read() & 0x02) == 0 {
            break;
        }
    }

    // Write command byte to enable mouse interrupt and enable mouse
    status_port.write(0x20); // Read command byte
    loop {
        if (status_port.read() & 0x01) != 0 {
            break;
        }
    }
    let command_byte = data_port.read();
    
    // Wait for input buffer
    loop {
        if (status_port.read() & 0x02) == 0 {
            break;
        }
    }

    // Write command byte back with mouse enabled
    status_port.write(0x60); // Write command byte
    loop {
        if (status_port.read() & 0x02) == 0 {
            break;
        }
    }
    data_port.write(command_byte | 0x02 | 0x20); // Enable mouse interrupt and mouse clock

    // Wait for input buffer
    loop {
        if (status_port.read() & 0x02) == 0 {
            break;
        }
    }

    // Enable mouse by sending 0xF4 to mouse
    status_port.write(0xD4); // Send to auxiliary device
    loop {
        if (status_port.read() & 0x02) == 0 {
            break;
        }
    }
    data_port.write(0xF4); // Enable mouse command

    // Wait for ACK (0xFA)
    let mut timeout = 100000;
    loop {
        if (status_port.read() & 0x01) != 0 {
            let response = data_port.read();
            if response == 0xFA {
                break;
            }
        }
        timeout -= 1;
        if timeout == 0 {
            return Err("Mouse initialization timeout");
        }
    }

    Ok(())
}

