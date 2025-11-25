//! PIC (Programmable Interrupt Controller) remapping
use pic8259::ChainedPics;
use spin::Mutex;

pub const PIC_1_OFFSET: u8 = 32;  // Primary PIC handles IRQs 0-7
pub const PIC_2_OFFSET: u8 = 40;  // Secondary PIC handles IRQs 8-15
pub const KERNEL_OFFSET: u8 = 120; 
pub static PICS: Mutex<ChainedPics> =
    Mutex::new(unsafe { ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET) });

// Interrupt indices - these are the actual vector numbers the CPU sees
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    Timer = PIC_1_OFFSET,        // 32 - IRQ0
    Keyboard = PIC_1_OFFSET + 1, // 33 - IRQ1
    Mouse = PIC_2_OFFSET + 4,     // 44 - IRQ12 (IRQ4 on PIC2)
    Syscall = KERNEL_OFFSET
    // COM2, COM1, LPT2, Floppy, LPT1, RTC, etc.
}

impl InterruptIndex {
    pub fn as_u8(self) -> u8 {
        self as u8
    }
    
    pub fn as_usize(self) -> usize {
        usize::from(self.as_u8())
    }
}
