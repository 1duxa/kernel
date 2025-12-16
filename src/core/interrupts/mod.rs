//! # Interrupt Handling Module
//!
//! Provides interrupt infrastructure for the kernel including:
//!
//! - **GDT**: Global Descriptor Table with TSS for stack switching
//! - **IDT**: Interrupt Descriptor Table with exception and hardware interrupt handlers
//! - **PIC**: 8259 Programmable Interrupt Controller initialization and EOI
//! - **Timer**: System timer tick tracking
//!
//! ## Interrupt Vector Layout
//!
//! | Vector | Type                   | Handler                    |
//! |--------|------------------------|----------------------------|
//! | 0-31   | CPU Exceptions         | divide, page fault, etc.   |
//! | 32     | Timer (IRQ0)           | timer_interrupt_handler    |
//! | 33     | Keyboard (IRQ1)        | keyboard_interrupt_handler |
//! | 44     | Mouse (IRQ12)          | mouse_interrupt_handler    |
//! | 0x80   | Syscall                | syscall_handler            |
//!
//! ## Usage
//!
//! ```ignore
//! use crate::core::interrupts;
//! interrupts::init(); // Initializes GDT, IDT, PIC
//! x86_64::instructions::interrupts::enable();
//! ```

use crate::core::interrupts::{interrupts::init_idt, pic::PICS};

#[allow(unused)]
pub mod pic;
pub mod gdt;
pub mod interrupts;
mod timer;

pub fn init() {
    gdt::init();
    init_idt();
    unsafe { PICS.lock().initialize() };
}