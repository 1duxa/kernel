use crate::interrupts::{interrupts::init_idt, pic::PICS};

#[allow(unused)]
pub mod pic;
pub mod gdt;
pub mod interrupts;

pub fn init() {
    // 1. Set up the GDT (Global Descriptor Table) and TSS
    gdt::init();
    
    // 2. Load the IDT
    init_idt();
    
    // 3. Initialize and remap the PIC
    unsafe { PICS.lock().initialize() };
    
    // 4. Enable interrupts - THIS IS CRUCIAL!
    //    Without this line, hardware interrupts will never fire
    x86_64::instructions::interrupts::enable();
}