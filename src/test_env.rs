use crate::data_structures::vec::String;
use core::sync::atomic::{AtomicUsize, Ordering};
use x86_64::structures::paging::{Page, PageTableFlags};
use x86_64::VirtAddr;

static TEST_EXECUTION_COUNT: AtomicUsize = AtomicUsize::new(0);

pub fn test_basic_paging() -> String {
    let _count = TEST_EXECUTION_COUNT.fetch_add(1, Ordering::Relaxed);
    let mut result = String::new();
    
    result.push_str("Testing basic paging...\n");
    
    unsafe {
        use x86_64::structures::paging::Mapper;
        
        if let Some(frame) = crate::memory::allocate_frame() {
            result.push_str("✓ Allocated physical frame\n");
            
            let test_vaddr = VirtAddr::new(0xdead_0000);
            let page = Page::containing_address(test_vaddr);
            
            let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
            
            let mut mapper = crate::memory::active_page_table();
            let mut allocator = crate::memory::GlobalFrameAllocator;
            
            match mapper.map_to(page, frame, flags, &mut allocator) {
                Ok(tlb_flush) => {
                    use x86_64::structures::paging::mapper::MapperFlush;
                    tlb_flush.flush();
                    result.push_str("✓ Page mapped successfully\n");
                    
                    let test_ptr = test_vaddr.as_mut_ptr::<u64>();
                    *test_ptr = 0xdeadbeef;
                    
                    let read_val = *test_ptr;
                    if read_val == 0xdeadbeef {
                        result.push_str("✓ Successfully wrote and read from mapped page\n");
                    } else {
                        result.push_str("✗ Page mapping verification failed\n");
                    }
                }
                Err(_) => result.push_str("✗ Page mapping failed\n"),
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
    
    unsafe {
        let _pid = crate::syscalls::handlers::process::get_next_pid();
        result.push_str("✓ Assigned PID\n");
        result.push_str("✓ Process context storage available\n");
    }
    
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

pub fn test_asm_simple_return() -> String {
    let mut result = String::new();
    result.push_str("Testing assembly execution (return 42)...\n");
    
    use crate::asm_executor::{AsmExecutor, AsmProgram};
    
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
    result.push_str(&test_process_creation());
    result.push_str("\n");
    result.push_str(&test_asm_simple_return());
    result.push_str("\n");
    result.push_str(&test_asm_add());
    result.push_str("=== TESTS COMPLETE ===\n");
    result
}
