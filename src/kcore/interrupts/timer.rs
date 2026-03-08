//! # PIC Timer Configuration
//!
//! Enables the PIT (Programmable Interval Timer) via PIC mask.
//!
//! ## Purpose
//!
//! By default, the PIC may mask the timer interrupt. This function
//! clears the mask bit to enable timer interrupts (IRQ0).
//!
//! ## Usage
//!
//! Called during interrupt initialization to ensure the timer
//! interrupt reaches the CPU.

pub fn init_pic_timer() {
    unsafe {
        use x86_64::instructions::port::Port;
        
        let mut pic_mask_port = Port::<u8>::new(0x21);
        let mut mask = pic_mask_port.read();
        
        mask &= !0x01;
        
        pic_mask_port.write(mask);
    }
}