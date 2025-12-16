//! DuxOS Kernel
//!
//! A minimal x86_64 operating system kernel written in Rust.
//!
//! # Features
//! - Memory management with physical frame allocation and paging
//! - Interrupt handling (keyboard, mouse, timer)
//! - Framebuffer-based graphics output
//! - PS/2 keyboard and mouse drivers
//! - Simple terminal emulator
//! - JIT code execution support via sys_mmap
//!
//! # Architecture
//! The kernel uses the bootloader crate for initial setup and receives
//! a physical memory map from the bootloader. It sets up its own page
//! tables for dynamic memory allocation.

#![no_std]
#![no_main]
#![feature(slice_pattern, abi_x86_interrupt, alloc_error_handler)]

extern crate alloc;
extern crate rlibc;
use alloc::boxed::Box;
use bootloader_api::{entry_point, BootInfo};
use spin::Mutex;
use uart_16550::SerialPort;

use crate::app::{AppEvent, AppHost};
use crate::apps::terminal_app::TerminalApp;
use crate::{
    devices::drivers::{ps2_keyboard, ps2_mouse},
    devices::mouse_cursor,
    devices::framebuffer::framebuffer::{init_framebuffer, FRAMEBUFFER},
    terminal_v2::Terminal,
    ui::Theme,
};
mod app;
mod apps;
mod syscalls;
mod data_structures;
mod memory;
mod terminal;
mod terminal_v2;
mod ui;
mod devices;
mod cmd_executor;
mod asm_executor;
mod task;
pub mod executor;
pub mod async_tasks;
pub mod core;
pub mod test_env;

// Configure bootloader to map all physical memory into virtual address space
const BOOTLOADER_CONFIG: bootloader_api::BootloaderConfig = {
    let mut config = bootloader_api::BootloaderConfig::new_default();
    // Request physical memory mapping (let bootloader choose address)
    config.mappings.physical_memory = Some(bootloader_api::config::Mapping::Dynamic);
    config
};

entry_point!(kernel_main, config = &BOOTLOADER_CONFIG);

pub static SERIAL: Mutex<SerialPort> = Mutex::new(unsafe { SerialPort::new(0x3F8) });

#[macro_export]
macro_rules! println {
    ($($arg:tt)*) => {{
        use ::core::fmt::Write;
        let mut serial = $crate::SERIAL.lock();
        let _ = writeln!(serial, $($arg)*);
    }};
}

pub fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
    unsafe {
        if let Err(e) = memory::init(boot_info) {
            println!("Failed to init heap: {}\n", e);
            loop_arch_mm();
        } else {
            println!("INIT: memory::init succeeded");
        }
    }
    
    let _  = core::kernel::init_kernel();
    
    init_framebuffer(boot_info);
    println!("INIT: init_framebuffer returned; FRAMEBUFFER set");
    let theme = Theme::dark_modern();

    let (fb_width, fb_height) = {
        let guard = FRAMEBUFFER.lock();
        let fb = guard.as_ref().unwrap();
        (fb.width, fb.height)
    };

    let cols = fb_width / 10;
    let rows = fb_height / 20;
    let mut host = AppHost::new();
    let term = Terminal::new(cols, rows, &theme);
    println!("INIT: Terminal created cols={} rows={}", cols, rows);
    let app = TerminalApp::new(term.clone());

    host.register_app(Box::new(app));
    {
        use crate::data_structures::vec::String as KString;
        use crate::ui::widgets::{Label, Panel, Rect, VStack, Widget};
        let mut guard = FRAMEBUFFER.lock();
        let fb = guard.as_mut().unwrap();
        let screen = Rect::new(0, (fb.height/2).try_into().unwrap(), fb.width, fb.height/2);

        fb.clear(theme.background);

        let header_h = 24usize;
        let mut header = Panel {
            rect: Rect::new(0, 0, 0, 0),
            bg: theme.primary,
            radius: Some(8),
        };
        let mut title = Label::new(KString::from("Welcome to RustOS"), theme.text);
        let mut root = VStack::new();
        root.push(&mut header, Some(header_h));
        root.push(&mut title, Some(header_h));
        root.layout(screen);
        root.render(fb, &theme);
        // Layout terminal app
        host.layout_app(
            0,
            Rect::new(
                0,
                (header_h * 2) as i32,
                fb.width,
                fb.height - (header_h * 2),
            ),
        );
        host.app_mut(0).init();
        host.render_app_once(0, fb, &theme);
        fb.render_frame();
        host.app_mut(0).overlay(fb, &theme);
        // Run one-time tests to exercise functionality like the AsmExecutor so
        // we can detect and debug panics (temp code for debugging).
        let asm_ret = crate::test_env::test_asm_simple_return();
        println!("ASM TEST (return): {:?}", asm_ret);
        let asm_add = crate::test_env::test_asm_add();
        println!("ASM TEST (add): {:?}", asm_add);
    }
    let mut decoder = ps2_keyboard::ScancodeDecoder::new();
    
    // Initialize mouse driver and cursor
    match ps2_mouse::init() {
        Ok(()) => println!("INIT: PS/2 mouse initialized"),
        Err(e) => println!("INIT: PS/2 mouse failed to init: {}", e),
    }
    mouse_cursor::init(fb_width, fb_height);
    
    // Draw initial cursor
    {
        let mut guard = FRAMEBUFFER.lock();
        let fb = guard.as_mut().unwrap();
        mouse_cursor::draw(fb);
        fb.render_frame();
    }
    
    loop {
        let mut had_keyboard_event = false;
        let mut mouse_moved = false;
        
        while let Some(scancode) = ps2_keyboard::dequeue_scancode() {
            if let Some(key) = decoder.process_scancode(scancode) {
                let event = if key.is_arrow {
                        AppEvent::KeyPress {
                            ch: if key.arrow_direction.is_some() { '\0' } else { key.character },
                            ctrl: key.ctrl,
                            alt: key.alt,
                            shift: key.shift,
                            arrow: key.arrow_direction,
                        }
                } else {
                    let ch = key.character;
                    AppEvent::KeyPress {
                        ch,
                        ctrl: key.ctrl,
                        alt: key.alt,
                        shift: key.shift,
                        arrow: None,
                    }
                };
                let mut guard = FRAMEBUFFER.lock();
                let fb = guard.as_mut().unwrap();
                host.dispatch_event(fb, &theme, event, theme.accent);
                // Draw cursor on top after app renders
                mouse_cursor::draw(fb);
                fb.render_frame();
                had_keyboard_event = true;
            }
        }

        // Handle mouse events
        while let Some(mouse_event) = ps2_mouse::poll_mouse_event() {
            // Update cursor position from mouse movement
            mouse_cursor::update_position(mouse_event.dx, -mouse_event.dy);
            mouse_moved = true;
            
            // Only dispatch click events to app (not every movement)
            if mouse_event.buttons != 0 {
                let event = AppEvent::Mouse(mouse_event);
                let mut guard = FRAMEBUFFER.lock();
                let fb = guard.as_mut().unwrap();
                host.dispatch_event(fb, &theme, event, theme.accent);
            }
        }
        
        // Render mouse cursor if it moved (and no keyboard events already rendered it)
        if mouse_moved && !had_keyboard_event {
            let mut guard = FRAMEBUFFER.lock();
            let fb = guard.as_mut().unwrap();
            mouse_cursor::draw(fb);
            fb.render_frame();
        }

        unsafe {
            ::core::arch::x86_64::_mm_pause();
        }
    }
}

fn loop_arch_mm() -> ! {
    loop {
        unsafe {
            ::core::arch::x86_64::_mm_pause();
        }
    }
}

#[panic_handler]
fn panic(_info: &::core::panic::PanicInfo) -> ! {
    println!("{:#?}", _info);
    loop_arch_mm()
}
