//! # Programmable Interrupt Controller (8259 PIC)
//!
//! Configures the legacy 8259 PIC chips for interrupt routing.
//!
//! ## PIC Architecture
//!
//! ```text
//! ┌─────────────┐     ┌─────────────┐
//! │   PIC 1     │     │   PIC 2     │
//! │  (Master)   │────▶│  (Slave)    │
//! │ IRQ 0-7     │     │ IRQ 8-15    │
//! └─────────────┘     └─────────────┘
//!       │
//!       ▼
//!     CPU
//! ```
//!
//! ## Vector Remapping
//!
//! By default, IRQ 0-15 conflict with CPU exception vectors.
//! We remap them:
//! - PIC 1: vectors 32-39 (IRQ 0-7)
//! - PIC 2: vectors 40-47 (IRQ 8-15)
//!
//! ## Interrupt Assignments
//!
//! | IRQ | Vector | Device    |
//! |-----|--------|-----------|
//! | 0   | 32     | Timer     |
//! | 1   | 33     | Keyboard  |
//! | 12  | 44     | Mouse     |

//! PIC (Programmable Interrupt Controller) remapping
use x86_64::instructions::port::Port;

pub const PIC_1_OFFSET: u8 = 32; // Primary PIC handles IRQs 0-7
pub const PIC_2_OFFSET: u8 = 40; // Secondary PIC handles IRQs 8-15
pub const KERNEL_OFFSET: u8 = 120;

pub fn eoi(interrupt_id: InterruptIndex) {
    let irq = interrupt_id.as_u8() - PIC_1_OFFSET;

    unsafe {
        if irq >= 8 {
            Port::<u8>::new(0xA0).write(0x20);
        }

        Port::<u8>::new(0x20).write(0x20);
    }
}
pub enum EoiTiming {
    Before,
    After,
}

pub fn handle_interrupt<F>(interrupt_id: InterruptIndex, handler: F, timing: EoiTiming)
where
    F: FnOnce(),
{
    match timing {
        EoiTiming::Before => eoi(interrupt_id),
        EoiTiming::After => (),
    }
    handler();
    match timing {
        EoiTiming::Before => (),
        EoiTiming::After => eoi(interrupt_id),
    }
}

// Interrupt indices - these are the actual vector numbers the CPU sees
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    Timer = PIC_1_OFFSET,        // 32 - IRQ0
    Keyboard = PIC_1_OFFSET + 1, // 33 - IRQ1
    Mouse = PIC_2_OFFSET + 4,    // 44 - IRQ12 (IRQ4 on PIC2)
    Syscall = KERNEL_OFFSET,     // COM2, COM1, LPT2, Floppy, LPT1, RTC, etc.
}

impl InterruptIndex {
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    pub fn as_usize(self) -> usize {
        usize::from(self.as_u8())
    }
}
