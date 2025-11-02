// TODO: Trash code
/// Kernel initialization phases
use crate::kernel::status::{update_component_status, InitStatus};
use crate::println;

use crate::kernel::status::register_component;

/// Initialize kernel in proper order with error handling
pub fn init_kernel() -> Result<(), &'static str> {
    // Register components for tracking
    register_component("CPU Features");
    register_component("Memory Management");
    register_component("Interrupt System");
    register_component("Display System");
    register_component("Input Devices");
    println!("╔════════════════════════════════════════╗");
    println!("║      RustOS Kernel Initialization      ║");
    println!("╚════════════════════════════════════════╝\n");
    
    
    // Phase 3: Interrupt subsystem
    init_phase("Interrupt System", init_interrupts)?;
    
    println!("\n✅ Kernel initialization complete!\n");
    Ok(())
}

fn init_phase(name: &'static str, init_fn: fn() -> Result<(), &'static str>) -> Result<(), &'static str> {
    update_component_status(name, InitStatus::InProgress);
    println!("[1/5] Initializing {}...", name);
    
    match init_fn() {
        Ok(()) => {
            update_component_status(name, InitStatus::Completed);
            println!("    ✓ {} initialized successfully\n", name);
            Ok(())
        }
        Err(e) => {
            update_component_status(name, InitStatus::Failed(e));
            println!("    ✗ {} failed: {}\n", name, e);
            Err(e)
        }
    }
}


fn init_interrupts() -> Result<(), &'static str> {
    crate::interrupts::init();
    
    // Enable keyboard interrupt (IRQ1)
    unsafe {
        use x86_64::instructions::port::Port;
        let mut pic1_data = Port::<u8>::new(0x21);
        let mask: u8 = pic1_data.read();
        let new_mask = mask & !(1 << 1); // Enable IRQ1 (keyboard)
        pic1_data.write(new_mask);
    }
    
    // Enable mouse interrupt (IRQ12)
    unsafe {
        use x86_64::instructions::port::Port;
        let mut pic1_data = Port::<u8>::new(0x21);
        let mask: u8 = pic1_data.read();
        let new_mask = mask & !(1 << 4); // Enable IRQ4 (cascade) if needed
        pic1_data.write(new_mask);
        
        let mut pic2_data = Port::<u8>::new(0xA1);
        let mask: u8 = pic2_data.read();
        let new_mask = mask & !(1 << 4); // Enable IRQ12 (mouse = IRQ4 on PIC2)
        pic2_data.write(new_mask);
    }
    
    x86_64::instructions::interrupts::enable();
    Ok(())
}

