use crate::interrupts::{interrupts::init_idt, pic::PICS};

#[allow(unused)]
pub mod pic;
pub mod gdt;
pub mod interrupts;

pub fn init() {
    gdt::init();
    init_idt();
    unsafe { PICS.lock().initialize() };
}