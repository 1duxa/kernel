use core::sync::atomic::{AtomicU64, Ordering};
use x86_64::{
    structures::paging::{
        Page, PageTable, PageTableFlags, PhysFrame, Size4KiB, 
        Mapper, FrameAllocator, OffsetPageTable,
    },
    VirtAddr, PhysAddr,
};
use crate::syscalls::dispatcher::SyscallError;

// Memory layout (hardcoded as requested)
const HEAP_START: u64 = 0x4444_4444_0000;
const HEAP_SIZE: u64 = 100 * 1024 * 1026; // 100 MB

// Physical memory (from your memory map)
const PHYSICAL_MEMORY_START: u64 = 0x116cf000;
const PHYSICAL_MEMORY_END: u64 = 0x7ffe0000;
static NEXT_PHYSICAL_FRAME: AtomicU64 = AtomicU64::new(PHYSICAL_MEMORY_START);

// Virtual address allocators
static PROGRAM_BREAK: AtomicU64 = AtomicU64::new(HEAP_START);
static NEXT_VIRT_ADDR: AtomicU64 = AtomicU64::new(0x8000_0000_0000);

/// Global offset for identity mapping (bootloader sets this)
/// Change this if your mapping is different
const PHYSICAL_MEMORY_OFFSET: u64 = 0;

// Simple bump frame allocator (thread-safe)
pub struct SimpleFrameAllocator;

unsafe impl FrameAllocator<Size4KiB> for SimpleFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        loop {
            let current = NEXT_PHYSICAL_FRAME.load(Ordering::Relaxed);
            
            // Align to page boundary
            let frame_addr = (current + 4095) & !4095;
            let next_frame = frame_addr + 4096;
            
            if next_frame >= PHYSICAL_MEMORY_END {
                return None; // Out of memory
            }
            
            // Atomic bump with CAS
            match NEXT_PHYSICAL_FRAME.compare_exchange_weak(
                current,
                next_frame,
                Ordering::SeqCst,
                Ordering::Relaxed,
            ) {
                Ok(_) => {
                    let phys_addr = PhysAddr::new(frame_addr);
                    return Some(PhysFrame::containing_address(phys_addr));
                }
                Err(_) => continue, // Retry if race lost
            }
        }
    }
}

// Create a proper mapper instance
unsafe fn get_active_mapper() -> OffsetPageTable<'static> {
    let (level_4_table_frame, _) = x86_64::registers::control::Cr3::read();
    let phys = level_4_table_frame.start_address();
    let virt = VirtAddr::new(phys.as_u64() + PHYSICAL_MEMORY_OFFSET);
    let page_table_ptr: *mut PageTable = virt.as_mut_ptr();
    
    OffsetPageTable::new(&mut *page_table_ptr, VirtAddr::new(PHYSICAL_MEMORY_OFFSET))
}

pub fn sys_brk(addr: u64) -> Result<usize, SyscallError> {
    if addr == 0 {
        return Ok(PROGRAM_BREAK.load(Ordering::Relaxed) as usize);
    }
    
    let current_brk = PROGRAM_BREAK.load(Ordering::Relaxed);
    if addr < HEAP_START || addr > HEAP_START + HEAP_SIZE {
        return Ok(current_brk as usize); // Invalid range
    }
    
    // Growing the heap
    if addr > current_brk {
        let start_page: Page<Size4KiB> = Page::containing_address(VirtAddr::new(current_brk));
        let end_page: Page<Size4KiB> = Page::containing_address(VirtAddr::new(addr - 1));
        
        let mut frame_allocator = SimpleFrameAllocator;
        let mut mapper = unsafe { get_active_mapper() };
        
        for page in Page::range_inclusive(start_page, end_page) {
            let frame = frame_allocator.allocate_frame()
                .ok_or(SyscallError::NoMemory)?;
                
            let flags = PageTableFlags::PRESENT 
                | PageTableFlags::WRITABLE 
                | PageTableFlags::USER_ACCESSIBLE;
            
            unsafe {
                mapper.map_to(page, frame, flags, &mut frame_allocator)
                    .map_err(|_| SyscallError::NoMemory)?
                    .flush();
            }
        }
    } else if addr < current_brk {
        // Shrinking - unmap pages
        let start_page = Page::containing_address(VirtAddr::new(addr));
        let end_page: Page<Size4KiB> = Page::containing_address(VirtAddr::new(current_brk - 1));
        
        let mut mapper = unsafe { get_active_mapper() };
        
        for page in Page::range_inclusive(start_page, end_page) {
            if let Ok((_, flush)) = mapper.unmap(page) {
                flush.flush(); // Physical frame not freed (leak), but safe
            }
        }
    }
    
    PROGRAM_BREAK.store(addr, Ordering::Relaxed);
    Ok(addr as usize)
}

pub fn sys_mmap(
    addr: usize,
    length: usize,
    prot: usize,
    flags: usize,
    _fd: i32,
    _offset: usize,
) -> Result<usize, SyscallError> {
    if length == 0 {
        return Err(SyscallError::InvalidArgument);
    }
    
    let page_count = (length + 4095) / 4096;
    let actual_size = page_count * 4096;
    
    // Allocate virtual address
    let virt_addr = if addr != 0 {
        (addr as u64) & !0xFFF // Align to page
    } else {
        allocate_virtual_region(actual_size as u64)
    };
    
    if virt_addr == 0 || virt_addr + actual_size as u64 > 0xFFFF_8000_0000_0000 {
        return Err(SyscallError::NoMemory);
    }
    
    // Map pages
    let mut frame_allocator = SimpleFrameAllocator;
    let mut mapper = unsafe { get_active_mapper() };
    
    let start_page: Page<Size4KiB> = Page::containing_address(VirtAddr::new(virt_addr));
    let end_addr = virt_addr + actual_size as u64 - 1;
    let end_page: Page<Size4KiB> = Page::containing_address(VirtAddr::new(end_addr));
    
    let mut page_flags = PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE;
    const PROT_WRITE: usize = 0x2;
    const PROT_EXEC: usize = 0x4;
    
    if prot & PROT_WRITE != 0 {
        page_flags |= PageTableFlags::WRITABLE;
    }
    if prot & PROT_EXEC == 0 {
        page_flags |= PageTableFlags::NO_EXECUTE;
    }
    
    for page in Page::range_inclusive(start_page, end_page) {
        let frame = frame_allocator.allocate_frame()
            .ok_or(SyscallError::NoMemory)?;
            
        unsafe {
            mapper.map_to(page, frame, page_flags, &mut frame_allocator)
                .map_err(|_| SyscallError::NoMemory)?
                .flush();
        }
        
        // Zero the page
        let page_ptr = page.start_address().as_u64() as *mut u8;
        unsafe { core::ptr::write_bytes(page_ptr, 0, 4096); }
    }
    
    Ok(virt_addr as usize)
}

pub fn sys_munmap(addr: usize, length: usize) -> Result<usize, SyscallError> {
    if length == 0 || addr & 0xFFF != 0 {
        return Err(SyscallError::InvalidArgument);
    }
    
    let start_addr = addr & !0xFFF;
    let page_count = (length + 4095) / 4096;
    let end_addr = start_addr + page_count * 4096;
    
    let start_page: Page<Size4KiB> = Page::containing_address(VirtAddr::new(start_addr as u64));
    let end_page: Page<Size4KiB> = Page::containing_address(VirtAddr::new(end_addr as u64 - 1));
    
    let mut mapper = unsafe { get_active_mapper() };
    
    for page in Page::range_inclusive(start_page, end_page) {
        if let Ok((_, flush)) = mapper.unmap(page) {
            flush.flush();
            // TODO: Free physical frame to allocator
        }
    }
    
    Ok(0)
}

fn allocate_virtual_region(size: u64) -> u64 {
    // Align size to page boundary
    let aligned_size = (size + 4095) & !4095;
    NEXT_VIRT_ADDR.fetch_add(aligned_size, Ordering::SeqCst)
}

// Initialize the memory subsystem
pub fn init() {
    // Ensure physical memory start is page-aligned
    assert_eq!(PHYSICAL_MEMORY_START & 0xFFF, 0);
}