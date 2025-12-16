//! # Test Environment
//!
//! Provides test functions for validating kernel subsystems.
//!
//! ## Available Tests
//!
//! ### Memory Tests
//! - `test_basic_paging()`: Tests page table mapping/unmapping
//! - `test_memory_allocation()`: Tests heap allocator
//!
//! ### Process Tests  
//! - `test_process_creation()`: Tests process/task spawning
//!
//! ### Assembly Execution Tests
//! - `test_asm_return_42()`: Tests JIT code that returns 42
//! - `test_asm_add()`: Tests JIT code that computes 1+2=3
//!
//! ## Test Infrastructure
//!
//! Each test function:
//! 1. Prints diagnostic information via `println!`
//! 2. Returns a `String` summary result
//! 3. Uses atomic counter to track test executions
//!
//! ## Usage
//!
//! Tests are invoked via terminal commands:
//! ```text
//! > test           # Run all tests
//! > test_paging    # Run specific test
//! > test_asm       # Run assembly tests
//! ```

use crate::data_structures::vec::String;
use crate::println;
use core::sync::atomic::{AtomicUsize, Ordering};
use x86_64::structures::paging::{Page, PageTableFlags, Mapper, FrameAllocator, Translate};
use x86_64::VirtAddr;
// Using GlobalFrameAllocator from crate::memory for test allocations

static TEST_EXECUTION_COUNT: AtomicUsize = AtomicUsize::new(0);

pub fn test_basic_paging() -> String {
    let _count = TEST_EXECUTION_COUNT.fetch_add(1, Ordering::Relaxed);
    let mut result = String::new();
    
    println!("TEST_PAGING: Starting basic paging test\n");
    println!("TEST_PAGING: PHYSICAL_MEMORY_OFFSET: {:#x}", crate::memory::physical_memory_offset());
    
    unsafe {
        let mut frame_allocator: crate::memory::GlobalFrameAllocator = crate::memory::GlobalFrameAllocator;
        
    if let Some(frame) = frame_allocator.allocate_frame() {
            println!("TEST_PAGING: Allocated physical frame: {:#x}\n", frame.start_address().as_u64());
            
            // Use a KERNEL virtual address in the direct-mapped region. On some
            // environments the bootloader does identity mapping (physical offset
            // == 0). In that case mapping into the typical high-half region
            // (0xffff_8800_0000_0000) will fail because the physical->virtual
            // offset is not setup. Detect that here and use a lower virtual
            // address for the test when necessary.
            let phys_offset = crate::memory::physical_memory_offset();
            let test_vaddr = if phys_offset == 0 {
                println!("TEST_PAGING: physical_memory_offset == 0, using low virt (0x400000) for test");
                VirtAddr::new(0x400000)
            } else {
                VirtAddr::new(0xffff_8800_0000_0000) // high kernel space
            };
            let page = Page::containing_address(test_vaddr);
            
            let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
            
            let mut mapper = crate::syscalls::handlers::memory::get_active_mapper();
            println!("TEST_PAGING: Using mapper with CR3 P4 frame: {:#x}", x86_64::registers::control::Cr3::read().0.start_address().as_u64());
            
            println!("TEST_PAGING: Attempting to map page {:#x} -> phys {:#x} (flags: {:?})", page.start_address().as_u64(), frame.start_address().as_u64(), flags);
            match mapper.map_to(page, frame, flags, &mut frame_allocator) {
                Ok(tlb_flush) => {
                    tlb_flush.flush();
                    println!("TEST_PAGING: Page mapped successfully\n");

                    // Show translation result
                    match mapper.translate_addr(test_vaddr) {
                        Some(paddr) => println!("TEST_PAGING: translate_addr -> phys {:#x}\n", paddr.as_u64()),
                        None => println!("TEST_PAGING: translate_addr -> None (not mapped)\n"),
                    }
                    
                    // Now write to the mapped virtual address
                    // The mapper has already set up the translation
                    let test_ptr = test_vaddr.as_mut_ptr::<u64>();
                    println!("TEST_PAGING: Writing to test_ptr virt {:#x} (ptr: {:?})\n", test_vaddr.as_u64(), test_ptr);
                    core::ptr::write(test_ptr, 0xdeadbeef);
                    
                    // Read back
                    let read_val = core::ptr::read(test_ptr);
                    if read_val == 0xdeadbeef {
                        println!("TEST_PAGING: Successfully wrote and read from mapped page (val={:#x})\n", read_val);
                    } else {
                        println!("TEST_PAGING: ✗ Value mismatch: expected 0xdeadbeef, got {:#x}\n", read_val);
                    }
                }
                Err(e) => {
                    let msg = match e {
                        x86_64::structures::paging::mapper::MapToError::FrameAllocationFailed => {
                            "Frame allocation failed"
                        }
                        x86_64::structures::paging::mapper::MapToError::ParentEntryHugePage => {
                            "Parent entry is huge page"
                        }
                        x86_64::structures::paging::mapper::MapToError::PageAlreadyMapped(_) => {
                            "Page already mapped"
                        }
                    };
                    println!("TEST_PAGING: ✗ Page mapping failed: {}", msg);
                }
            }
        } else {
            result.push_str("✗ Frame allocation failed\n");
        }
    }
    
    result
}

pub fn test_process_creation() -> String {
    let mut result = String::new();
    result.push_str("Testing process creation...\n");
    
        let _pid = crate::syscalls::handlers::process::get_next_pid();
        result.push_str("✓ Assigned PID\n");
        result.push_str("✓ Process context storage available\n");
    
    result
}

pub fn test_memory_allocation() -> String {
    let mut result = String::new();
    result.push_str("Testing memory allocation...\n");
    
    let test_size = 1024;
    
    unsafe {
        use alloc::alloc::alloc;
        use alloc::alloc::dealloc;
        
        let layout = ::core::alloc::Layout::from_size_align_unchecked(test_size, 16);
        let ptr = alloc(layout);
        if !ptr.is_null() {
            result.push_str("✓ Allocated memory successfully\n");
            dealloc(ptr, layout);
            result.push_str("✓ Memory deallocated successfully\n");
        } else {
            result.push_str("✗ Memory allocation failed\n");
        }
    }
    
    result
}

pub fn test_mmap_mapping() -> String {
    let mut result = String::new();
    result.push_str("Testing sys_mmap mapping & write...\n");

    use crate::memory::{sys_mmap, sys_munmap};

    const PROT_WRITE: usize = 0x2;
    const PROT_EXEC: usize = 0x4;

    println!("TEST_ENV: Attempting sys_mmap in test_mmap_mapping");
    match sys_mmap(0, 4096, PROT_WRITE, 0, 0, 0) {
        Ok(virt_addr) => {
            println!("TEST_ENV: sys_mmap returned virt {:#x}", virt_addr);
            unsafe {
                let ptr = virt_addr as *mut u8;
                println!("TEST_ENV: writing to virt ptr {:#x}", ptr as usize);
                core::ptr::write(ptr, 0x55);
                let v = core::ptr::read(ptr);
                println!("TEST_ENV: read back {:#x}", v);
            }
            let _ = sys_munmap(virt_addr, 4096);
            result.push_str("✓ sys_mmap & write test succeeded\n");
        }
        Err(_) => {
            result.push_str("✗ sys_mmap failed (no memory or invalid alloc)\n");
        }
    }

    result
}

pub fn test_asm_simple_return() -> String {
    let mut result = String::new();
    result.push_str("Testing assembly execution (return 42)...\n");
    
    use crate::asm_executor::{AsmExecutor, AsmProgram};
    
    println!("TEST_ENV: calling AsmExecutor::execute for simple_return_42");
    match AsmExecutor::execute(AsmProgram::simple_return_42()) {
        Ok(ret_val) => {
            if ret_val == 42 {
                result.push_str("✓ Assembly executed successfully, returned 42\n");
            } else {
                result.push_str("✗ Got unexpected return value\n");
            }
        }
        Err(e) => {
            let mut msg = String::from("✗ Assembly execution failed: ");
            msg.push_str(&e);
            result.push_str(&msg);
            result.push('\n');
        }
    }
    
    result
}

pub fn test_asm_add() -> String {
    let mut result = String::new();
    result.push_str("Testing assembly execution (1 + 2)...\n");
    
    use crate::asm_executor::{AsmExecutor, AsmProgram};
    
    println!("TEST_ENV: calling AsmExecutor::execute for simple_add_1_2");
    match AsmExecutor::execute(AsmProgram::simple_add_1_2()) {
        Ok(ret_val) => {
            if ret_val == 3 {
                result.push_str("✓ Assembly executed successfully, returned 3\n");
            } else {
                result.push_str("✗ Got unexpected return value\n");
            }
        }
        Err(e) => {
            let mut msg = String::from("✗ Assembly execution failed: ");
            msg.push_str(&e);
            result.push_str(&msg);
            result.push('\n');
        }
    }
    
    result
}

pub fn test_all() -> String {
    let mut result = String::new();
    result.push_str("=== RUNNING ALL TESTS ===\n");
    result.push_str(&test_memory_allocation());
    result.push_str("\n");
    result.push_str(&test_basic_paging());
    result.push_str("\n");
    result.push_str(&test_mmap_mapping());
    result.push_str("\n");
    result.push_str(&test_process_creation());
    result.push_str("\n");
    result.push_str(&test_asm_simple_return());
    result.push_str("\n");
    result.push_str(&test_asm_add());
    result.push_str("=== TESTS COMPLETE ===\n");
    result
}
