//! # Kernel Initialization
//!
//! Orchestrates the kernel boot sequence with proper error handling
//! and status tracking.
//!
//! ## Initialization Phases
//!
//! 1. **CPU Features**: Validates CPU capabilities
//! 2. **Memory Management**: Heap, paging, frame allocator
//! 3. **Interrupt System**: GDT, IDT, PIC setup
//! 4. **Display System**: Framebuffer initialization
//! 5. **Input Devices**: Keyboard and mouse drivers
//!
//! ## Status Tracking
//!
//! Each phase updates its status via `update_component_status`:
//! - `NotStarted` → `InProgress` → `Completed` or `Failed`
//!
//! ## Error Handling
//!
//! Failures are logged and propagated. The kernel can continue
//! with partial functionality or halt based on failure severity.
//!
//! ## Usage
//!
//! ```ignore
//! use crate::core::kernel::init_kernel;
//! init_kernel().expect("Kernel initialization failed");
//! ```

/// Kernel initialization phases
use crate::kcore::kernel::status::{update_component_status, InitStatus};
use crate::println;

use crate::kcore::kernel::status::register_component;

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

    println!("\n Kernel initialization complete!\n");
    Ok(())
}

fn init_phase(
    name: &'static str,
    init_fn: fn() -> Result<(), &'static str>,
) -> Result<(), &'static str> {
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
    crate::kcore::interrupts::init();
    // enable timer interrupts
    unsafe {
        use x86_64::instructions::port::Port;
        let mut pic1_data = Port::<u8>::new(0x21);
        let mask: u8 = pic1_data.read();
        let new_mask = mask & !(1 << 0); // enable irq0 (timer)
        pic1_data.write(new_mask);
    }
    println!("1");
    // enable keyboard interrupt (irq1)
    unsafe {
        use x86_64::instructions::port::Port;
        let mut pic1_data = Port::<u8>::new(0x21);
        let mask: u8 = pic1_data.read();
        let new_mask = mask & !(1 << 1); // enable irq1 (keyboard)
        pic1_data.write(new_mask);
    }

    println!("2");
    // enable mouse interrupt (irq12)
    // enable ps/2 mouse via controller
    unsafe {
        use x86_64::instructions::port::Port;

        let mut cmd = Port::<u8>::new(0x64);
        let mut data = Port::<u8>::new(0x60);

        // helper to wait until controller is ready to accept a command
        let wait_write = || {
            while Port::<u8>::new(0x64).read() & 0x2 != 0 {}
        };
        let wait_read = || {
            while Port::<u8>::new(0x64).read() & 0x1 == 0 {}
        };
        // Enable auxiliary (mouse) port
        wait_write();
        cmd.write(0xA8);

        // Read controller config byte
        wait_write();
        cmd.write(0x20);
        wait_read();
        let mut config = data.read();

        // Enable IRQ12 (mouse) and enable auxiliary device
        config |= 0x02; // IRQ12
        config &= !0x20; // enable aux clock
        wait_write();
        cmd.write(0x60);
        wait_write();
        data.write(config);

        // Set defaults
        wait_write();
        cmd.write(0xD4);
        wait_write();
        data.write(0xF6);
        wait_read();
        data.read(); // consume ack

        // Enable data reporting
        wait_write();
        cmd.write(0xD4);
        wait_write();
        data.write(0xF4);
        wait_read();
        data.read(); // consume ack
    }
    println!("3");

    x86_64::instructions::interrupts::enable();
    Ok(())
}
