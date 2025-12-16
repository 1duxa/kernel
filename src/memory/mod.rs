//! Memory Management Module
//!
//! This module handles all memory-related operations for the kernel:
//!
//! # Components
//! - **Physical Frame Allocator**: Bump allocator for physical 4KiB frames
//! - **Kernel Heap**: 256MB heap using fixed-size block allocator
//! - **Page Table Management**: Direct manipulation of x86_64 page tables
//! - **sys_mmap**: Memory mapping syscall for user/JIT code
//!
//! # Physical Memory Mapping
//! The bootloader maps all physical memory at a configurable offset.
//! With `Mapping::Dynamic`, the offset is chosen by the bootloader
//! (typically around 0x20000000000 / 2TB). Physical address `P` can be
//! accessed at virtual address `P + physical_memory_offset`.
//!
//! # Page Table Walking
//! The `access_page_table()` function converts physical addresses to virtual
//! using the bootloader's offset, allowing direct modification of page tables.

use bootloader_api::info::MemoryRegionKind;
use bootloader_api::BootInfo;
use core::alloc::{GlobalAlloc, Layout};
use core::sync::atomic::{AtomicU64, AtomicBool, Ordering};
use core::ptr;
use crate::println;

pub mod allocators;

use x86_64::{
    structures::paging::{
        Page, PageTable, PageTableFlags, PhysFrame, Size4KiB,
        FrameAllocator, OffsetPageTable,
    },
    VirtAddr, PhysAddr,
};
use x86_64::registers::control::Cr3;

use allocators::FixedSizeBlockAllocator;

// ============================================================================
// CONSTANTS AND STATICS
// ============================================================================

const KERNEL_HEAP_SIZE: usize = 256 * 1024 * 1024;

#[repr(align(4096))]
struct HeapBuffer([u8; KERNEL_HEAP_SIZE]);
static mut KERNEL_HEAP_BUFFER: HeapBuffer = HeapBuffer([0; KERNEL_HEAP_SIZE]);

// Physical memory offset from bootloader (0 for identity mapping)
pub static PHYSICAL_MEMORY_OFFSET: AtomicU64 = AtomicU64::new(0);

// Physical frame allocator bounds
pub static PHYSICAL_MEMORY_START: AtomicU64 = AtomicU64::new(0);
pub static PHYSICAL_MEMORY_END: AtomicU64 = AtomicU64::new(0);
pub static NEXT_PHYSICAL_FRAME: AtomicU64 = AtomicU64::new(0);

// Next virtual address for mmap allocations
static NEXT_MMAP_ADDR: AtomicU64 = AtomicU64::new(0x40_0000); // Start at 4MB

// Flag to track if memory system is initialized
static MEMORY_INITIALIZED: AtomicBool = AtomicBool::new(false);

// ============================================================================
// GLOBAL ALLOCATOR (HEAP)
// ============================================================================

#[global_allocator]
static KERNEL_ALLOCATOR: LockedHeap = LockedHeap::new();

pub struct LockedHeap {
    inner: spin::Mutex<Option<FixedSizeBlockAllocator>>,
}

impl LockedHeap {
    pub const fn new() -> Self {
        Self {
            inner: spin::Mutex::new(None),
        }
    }
}

unsafe impl GlobalAlloc for LockedHeap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let guard = self.inner.lock();
        if let Some(allocator) = guard.as_ref() {
            allocator.alloc(layout)
        } else {
            core::ptr::null_mut()
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let guard = self.inner.lock();
        if let Some(allocator) = guard.as_ref() {
            allocator.dealloc(ptr, layout);
        }
    }
}

// ============================================================================
// PHYSICAL FRAME ALLOCATOR
// ============================================================================

pub struct GlobalFrameAllocator;

unsafe impl FrameAllocator<Size4KiB> for GlobalFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        loop {
            let current = NEXT_PHYSICAL_FRAME.load(Ordering::SeqCst);
            let frame_addr = (current + 4095) & !4095; // Align up to 4KB
            let next_frame = frame_addr + 4096;
            let memory_end = PHYSICAL_MEMORY_END.load(Ordering::SeqCst);
            
            if next_frame > memory_end {
                return None;
            }
            
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
                Err(_) => continue,
            }
        }
    }
}

pub fn allocate_frame() -> Option<PhysFrame<Size4KiB>> {
    let mut alloc = GlobalFrameAllocator;
    alloc.allocate_frame()
}

// ============================================================================
// INITIALIZATION
// ============================================================================

pub unsafe fn init(boot_info: &BootInfo) -> Result<(), &'static str> {
    let phys_offset = boot_info.physical_memory_offset
        .into_option()
        .unwrap_or(0);
    
    PHYSICAL_MEMORY_OFFSET.store(phys_offset, Ordering::SeqCst);
    println!("INIT: Boot physical_memory_offset: {:#x}", phys_offset);

    if phys_offset == 0 {
        println!("INIT: Using identity mapping (phys_offset=0)");
    }

    // Find usable memory regions
    let mut largest_region_start = 0u64;
    let mut largest_region_size = 0u64;
    let mut lowest_region_start = u64::MAX;
    let mut best_region_start = 0u64;
    let mut best_region_end = 0u64;
    
    println!("INIT: Memory regions from bootloader:");
    for region in boot_info.memory_regions.iter() {
        println!("  Region: {:#x}-{:#x} kind={:?}", region.start, region.end, region.kind);
        if region.kind == MemoryRegionKind::Usable {
            let size = region.end - region.start;
            if size > largest_region_size {
                largest_region_start = region.start;
                largest_region_size = size;
            }
            if region.start < lowest_region_start {
                lowest_region_start = region.start;
            }
            // Look for a region that starts after kernel (0x1700000) but is still mapped
            // The bootloader identity-maps up to a certain point
            if region.start >= 0x1700000 && region.end <= 0x10000000 {
                // Region between 23MB and 256MB - likely identity mapped
                if region.end - region.start > best_region_end - best_region_start {
                    best_region_start = region.start;
                    best_region_end = region.end;
                }
            }
        }
    }
    
    if largest_region_size == 0 {
        return Err("No usable memory found");
    }
    
    // CRITICAL: The bootloader only identity-maps certain regions.
    // The kernel is loaded at physical ~0x1000000-0x1685000.
    // The kernel's BSS (including heap) extends mapping up to ~0x1026d000.
    // 
    // We CANNOT use physical addresses that aren't mapped!
    // The safest approach is to use memory right after kernel ends but
    // within the first 16MB which is typically identity-mapped by BIOS/bootloader.
    //
    // However, looking at the actual boot: kernel is at 0x1000000 (16MB),
    // so low memory (< 16MB) might be usable.
    //
    // Strategy: Find a usable region that starts below 16MB, or use the region
    // right after kernel code (0x1700000) but cap it to stay within mapped area.
    
    // Find the best region: prefer one starting after 0x100000 (1MB) but before 0x1000000 (16MB)
    // to avoid kernel and stay in typically-mapped low memory
    let mut frame_start = 0u64;
    let mut frame_end = 0u64;
    
    for region in boot_info.memory_regions.iter() {
        if region.kind == MemoryRegionKind::Usable {
            // Look for region in 1MB-16MB range (before kernel)
            if region.start >= 0x100000 && region.start < 0x1000000 {
                let usable_start = region.start;
                let usable_end = region.end.min(0x1000000); // Cap at kernel start
                if usable_end > usable_start && (usable_end - usable_start) > (frame_end - frame_start) {
                    frame_start = usable_start;
                    frame_end = usable_end;
                }
            }
        }
    }
    
    // If no good low region found, try to use memory after kernel but be conservative
    if frame_start == 0 {
        // Kernel ends around 0x1685000, but mapping extends to ~0x1026d000 due to BSS
        // Use 24MB-26MB range which should be safe
        frame_start = 0x1800000; // 24MB
        frame_end = 0x1A00000;   // 26MB - just 2MB, but it's safe
        println!("INIT: WARNING - Using fallback frame region {:#x}-{:#x}", frame_start, frame_end);
    }
    
    println!("INIT: Kernel at ~0x1000000. Frame allocator: {:#x}-{:#x}", frame_start, frame_end);
    
    PHYSICAL_MEMORY_START.store(frame_start, Ordering::SeqCst);
    PHYSICAL_MEMORY_END.store(frame_end, Ordering::SeqCst);
    NEXT_PHYSICAL_FRAME.store(frame_start, Ordering::SeqCst);
    
    println!("INIT: Frame allocator: start={:#x}, end={:#x}", frame_start, frame_end);

    // Initialize heap allocator
    let allocator = FixedSizeBlockAllocator::new();
    let heap_ptr = KERNEL_HEAP_BUFFER.0.as_mut_ptr() as usize;
    println!("INIT: Attempting heap init: ptr={:#x}, size={:#x}", heap_ptr, KERNEL_HEAP_SIZE);
    
    match allocator.init(heap_ptr, KERNEL_HEAP_SIZE) {
        Ok(()) => {
            println!("INIT: Heap initialized successfully");
        }
        Err(e) => {
            println!("INIT: Heap initialization failed: {:?}", e);
            return Err("Failed to initialize kernel heap");
        }
    }
    *KERNEL_ALLOCATOR.inner.lock() = Some(allocator);
    
    MEMORY_INITIALIZED.store(true, Ordering::SeqCst);
    println!("INIT: Memory system initialized");
    
    Ok(())
}

// ============================================================================
// PHYSICAL/VIRTUAL ADDRESS HELPERS
// ============================================================================

/// Convert a physical address to a virtual address using the bootloader's offset.
/// With identity mapping (offset=0), phys == virt for accessible memory.
pub fn phys_to_virt(phys: PhysAddr) -> VirtAddr {
    let offset = PHYSICAL_MEMORY_OFFSET.load(Ordering::SeqCst);
    VirtAddr::new(phys.as_u64() + offset)
}

/// Get the physical memory offset
pub fn physical_memory_offset() -> u64 {
    PHYSICAL_MEMORY_OFFSET.load(Ordering::SeqCst)
}

/// Access a page table frame directly using identity mapping.
/// SAFETY: Only valid when bootloader provides identity mapping (offset=0 or valid mapping).
unsafe fn access_page_table(phys: PhysAddr) -> &'static mut PageTable {
    let offset = PHYSICAL_MEMORY_OFFSET.load(Ordering::SeqCst);
    let virt = phys.as_u64() + offset;
    &mut *(virt as *mut PageTable)
}

// ============================================================================
// PAGE TABLE MAPPING - DIRECT APPROACH
// ============================================================================

/// Map errors
#[derive(Debug, Clone, Copy)]
pub enum MapError {
    OutOfMemory,
    AlreadyMapped,
    InvalidAddress,
    WalkError,
}

/// Map a single 4KiB page to a physical frame.
/// 
/// This directly walks and modifies page tables using identity mapping,
/// which works because the bootloader identity-maps physical memory.
pub fn map_single_page(
    virt: VirtAddr,
    frame: PhysFrame<Size4KiB>,
    flags: PageTableFlags,
) -> Result<(), MapError> {
    if (virt.as_u64() & 0xfff) != 0 {
        return Err(MapError::InvalidAddress);
    }

    let page = Page::<Size4KiB>::containing_address(virt);
    let p4_idx = page.p4_index();
    let p3_idx = page.p3_index();
    let p2_idx = page.p2_index();
    let p1_idx = page.p1_index();

    // Get CR3 (P4 physical address)
    let (cr3_frame, _) = Cr3::read();
    let cr3_phys = cr3_frame.start_address();

    // Parent entry flags - MUST NOT have NO_EXECUTE to allow executable pages
    let parent_flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;

    // Walk P4 -> P3
    let p4_table = unsafe { access_page_table(cr3_phys) };
    let p4_entry = &mut p4_table[p4_idx];
    
    if p4_entry.is_unused() {
        // Allocate new P3 table
        let new_frame = allocate_frame().ok_or(MapError::OutOfMemory)?;
        // Zero the new table
        unsafe {
            let new_table = access_page_table(new_frame.start_address());
            ptr::write_bytes(new_table as *mut PageTable as *mut u8, 0, 4096);
        }
        p4_entry.set_frame(new_frame, parent_flags);
    } else if p4_entry.flags().contains(PageTableFlags::NO_EXECUTE) && !flags.contains(PageTableFlags::NO_EXECUTE) {
        // Clear NO_EXECUTE on parent if we need executable page
        let current_frame = p4_entry.frame().map_err(|_| MapError::WalkError)?;
        p4_entry.set_frame(current_frame, parent_flags);
    }
    
    let p3_phys = p4_entry.frame().map_err(|_| MapError::WalkError)?.start_address();

    // Walk P3 -> P2
    let p3_table = unsafe { access_page_table(p3_phys) };
    let p3_entry = &mut p3_table[p3_idx];
    
    if p3_entry.is_unused() {
        let new_frame = allocate_frame().ok_or(MapError::OutOfMemory)?;
        unsafe {
            let new_table = access_page_table(new_frame.start_address());
            ptr::write_bytes(new_table as *mut PageTable as *mut u8, 0, 4096);
        }
        p3_entry.set_frame(new_frame, parent_flags);
    } else if p3_entry.flags().contains(PageTableFlags::NO_EXECUTE) && !flags.contains(PageTableFlags::NO_EXECUTE) {
        let current_frame = p3_entry.frame().map_err(|_| MapError::WalkError)?;
        p3_entry.set_frame(current_frame, parent_flags);
    }
    
    let p2_phys = p3_entry.frame().map_err(|_| MapError::WalkError)?.start_address();

    // Walk P2 -> P1
    let p2_table = unsafe { access_page_table(p2_phys) };
    let p2_entry = &mut p2_table[p2_idx];
    
    if p2_entry.is_unused() {
        let new_frame = allocate_frame().ok_or(MapError::OutOfMemory)?;
        unsafe {
            let new_table = access_page_table(new_frame.start_address());
            ptr::write_bytes(new_table as *mut PageTable as *mut u8, 0, 4096);
        }
        p2_entry.set_frame(new_frame, parent_flags);
    } else if p2_entry.flags().contains(PageTableFlags::NO_EXECUTE) && !flags.contains(PageTableFlags::NO_EXECUTE) {
        let current_frame = p2_entry.frame().map_err(|_| MapError::WalkError)?;
        p2_entry.set_frame(current_frame, parent_flags);
    }
    
    let p1_phys = p2_entry.frame().map_err(|_| MapError::WalkError)?.start_address();

    // Set the P1 entry (final mapping)
    let p1_table = unsafe { access_page_table(p1_phys) };
    let p1_entry = &mut p1_table[p1_idx];
    
    // Set the mapping with explicit flags
    p1_entry.set_frame(frame, flags | PageTableFlags::PRESENT);

    // Flush TLB for this page
    x86_64::instructions::tlb::flush(virt);

    Ok(())
}

/// Check if a virtual address is mapped
pub fn page_is_mapped(virt: VirtAddr) -> bool {
    let page = Page::<Size4KiB>::containing_address(virt);
    let p4_idx = page.p4_index();
    let p3_idx = page.p3_index();
    let p2_idx = page.p2_index();
    let p1_idx = page.p1_index();

    let (cr3_frame, _) = Cr3::read();
    let cr3_phys = cr3_frame.start_address();

    // Walk P4
    let p4_table = unsafe { access_page_table(cr3_phys) };
    let p4_entry = &p4_table[p4_idx];
    if p4_entry.is_unused() { return false; }
    
    let p3_phys = match p4_entry.frame() {
        Ok(f) => f.start_address(),
        Err(_) => return false,
    };

    // Walk P3
    let p3_table = unsafe { access_page_table(p3_phys) };
    let p3_entry = &p3_table[p3_idx];
    if p3_entry.is_unused() { return false; }
    
    let p2_phys = match p3_entry.frame() {
        Ok(f) => f.start_address(),
        Err(_) => return false,
    };

    // Walk P2
    let p2_table = unsafe { access_page_table(p2_phys) };
    let p2_entry = &p2_table[p2_idx];
    if p2_entry.is_unused() { return false; }
    
    let p1_phys = match p2_entry.frame() {
        Ok(f) => f.start_address(),
        Err(_) => return false,
    };

    // Check P1
    let p1_table = unsafe { access_page_table(p1_phys) };
    let p1_entry = &p1_table[p1_idx];
    
    !p1_entry.is_unused() && p1_entry.flags().contains(PageTableFlags::PRESENT)
}

/// Zero a physical frame's contents
fn zero_frame(frame: PhysFrame<Size4KiB>) {
    let offset = PHYSICAL_MEMORY_OFFSET.load(Ordering::SeqCst);
    let virt = frame.start_address().as_u64() + offset;
    unsafe {
        ptr::write_bytes(virt as *mut u8, 0, 4096);
    }
}

// ============================================================================
// SYSCALLS
// ============================================================================

pub fn sys_mmap(
    addr: usize,
    length: usize,
    prot: usize,
    _flags: usize,
    _fd: i32,
    _offset: usize,
) -> Result<usize, crate::syscalls::dispatcher::SyscallError> {
    use crate::syscalls::dispatcher::SyscallError;
    
    if length == 0 {
        return Err(SyscallError::InvalidArgument);
    }
    
    let page_count = (length + 4095) / 4096;
    let actual_size = page_count * 4096;
    
    // Choose virtual address
    let virt_addr = if addr != 0 {
        addr as u64 & !0xFFF
    } else {
        NEXT_MMAP_ADDR.fetch_add(actual_size as u64, Ordering::SeqCst)
    };
    
    // Build page table flags
    let mut flags = PageTableFlags::PRESENT;
    
    // PROT_WRITE (0x2)
    if prot & 0x2 != 0 {
        flags |= PageTableFlags::WRITABLE;
    }
    
    // PROT_EXEC (0x4) - if NOT set, mark as no-execute
    // Note: x86_64 uses NO_EXECUTE bit, so we set it when exec is NOT requested
    if prot & 0x4 == 0 {
        flags |= PageTableFlags::NO_EXECUTE;
    }
    
    // Map each page
    for i in 0..page_count {
        let page_virt = VirtAddr::new(virt_addr + (i * 4096) as u64);
        
        let frame = allocate_frame().ok_or(SyscallError::NoMemory)?;
        
        // Zero the frame before mapping
        zero_frame(frame);
        
        // Map the page
        map_single_page(page_virt, frame, flags)
            .map_err(|_| SyscallError::NoMemory)?;
    }
    
    Ok(virt_addr as usize)
}

pub fn sys_munmap(addr: usize, length: usize) -> Result<usize, crate::syscalls::dispatcher::SyscallError> {
    use crate::syscalls::dispatcher::SyscallError;
    
    if length == 0 || addr & 0xFFF != 0 {
        return Err(SyscallError::InvalidArgument);
    }
    
    // TODO: Implement proper unmapping
    // For now, just return success
    Ok(0)
}

pub fn sys_brk(addr: u64) -> Result<usize, crate::syscalls::dispatcher::SyscallError> {
    use crate::syscalls::dispatcher::SyscallError;
    
    const HEAP_START: u64 = 0x4444_4444_0000;
    static PROGRAM_BREAK: AtomicU64 = AtomicU64::new(HEAP_START);
    
    if addr == 0 {
        return Ok(PROGRAM_BREAK.load(Ordering::Relaxed) as usize);
    }
    
    // Just track the break address, actual mapping happens on fault
    PROGRAM_BREAK.store(addr, Ordering::Relaxed);
    Ok(addr as usize)
}

// ============================================================================
// COMPATIBILITY FUNCTIONS
// ============================================================================

/// Get an OffsetPageTable for compatibility with existing code.
/// Note: This may not work correctly with identity mapping (offset=0).
pub unsafe fn active_page_table() -> OffsetPageTable<'static> {
    let (level_4_frame, _) = Cr3::read();
    let current_p4_phys = level_4_frame.start_address();
    let current_p4_virt = phys_to_virt(current_p4_phys);
    let page_table_ptr: *mut PageTable = current_p4_virt.as_mut_ptr();
    let offset = physical_memory_offset();
    OffsetPageTable::new(&mut *page_table_ptr, VirtAddr::new(offset))
}

/// Map a contiguous range of virtual memory
pub fn map_range(
    virt: VirtAddr,
    len: usize,
    first_frame: PhysFrame<Size4KiB>,
    flags: PageTableFlags,
    allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), MapError> {
    const PAGE_SIZE: usize = 4096;
    if len == 0 || (len % PAGE_SIZE) != 0 {
        return Err(MapError::InvalidAddress);
    }
    
    let n_pages = len / PAGE_SIZE;
    let mut v = virt;
    let mut cur_frame = first_frame;
    
    for i in 0..n_pages {
        if i > 0 {
            cur_frame = allocator.allocate_frame().ok_or(MapError::OutOfMemory)?;
        }
        map_single_page(v, cur_frame, flags)?;
        v = VirtAddr::new(v.as_u64() + PAGE_SIZE as u64);
    }
    
    Ok(())
}

// ============================================================================
// PROCESS SUPPORT FUNCTIONS
// ============================================================================

/// Create a new page table for a process (clone of kernel mappings)
pub fn create_process_page_table() -> Result<PhysFrame<Size4KiB>, &'static str> {
    let new_frame = allocate_frame().ok_or("Failed to allocate frame for process page table")?;
    
    // Zero the new P4 table
    zero_frame(new_frame);
    
    // Copy kernel mappings from current P4 to new P4
    let (current_p4_frame, _) = Cr3::read();
    let current_p4 = unsafe { access_page_table(current_p4_frame.start_address()) };
    let new_p4 = unsafe { access_page_table(new_frame.start_address()) };
    
    // Copy the upper half (kernel space) entries
    for i in 256..512 {
        new_p4[i] = current_p4[i].clone();
    }
    
    Ok(new_frame)
}

/// Start a process with the given code
pub unsafe fn sys_pstart(code_ptr: *const u8, code_size: usize) -> Result<usize, &'static str> {
    use core::sync::atomic::AtomicUsize;
    static NEXT_PID: AtomicUsize = AtomicUsize::new(1);
    
    if code_ptr.is_null() || code_size == 0 {
        return Err("Invalid code pointer or size");
    }
    
    // Allocate memory for the process code
    let page_count = (code_size + 4095) / 4096;
    let code_virt = 0x40_0000u64; // Process code starts at 4MB
    
    for i in 0..page_count {
        let page_virt = VirtAddr::new(code_virt + (i * 4096) as u64);
        let frame = allocate_frame().ok_or("Failed to allocate frame for process code")?;
        zero_frame(frame);
        
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
        map_single_page(page_virt, frame, flags).map_err(|_| "Failed to map process code page")?;
    }
    
    // Copy the code
    let dest = code_virt as *mut u8;
    ptr::copy_nonoverlapping(code_ptr, dest, code_size);
    
    let pid = NEXT_PID.fetch_add(1, Ordering::SeqCst);
    Ok(pid)
}

/// Walk page tables and print diagnostic information for a virtual address.
/// Used for debugging page faults.
pub fn debug_page_walk(virt: VirtAddr) {
    let va_u64 = virt.as_u64();
    let p4_index = ((va_u64 >> 39) & 0x1FF) as usize;
    let p3_index = ((va_u64 >> 30) & 0x1FF) as usize;
    let p2_index = ((va_u64 >> 21) & 0x1FF) as usize;
    let p1_index = ((va_u64 >> 12) & 0x1FF) as usize;

    let (cr3_frame, _) = Cr3::read();
    let cr3_phys = cr3_frame.start_address();
    
    println!("Page walk for virt {:#x}:", va_u64);
    println!("  CR3 P4 frame phys: {:#x}", cr3_phys.as_u64());
    
    // Walk P4
    let p4_table = unsafe { access_page_table(cr3_phys) };
    let p4_entry = &p4_table[p4_index];
    let p4_addr = p4_entry.addr().as_u64();
    let p4_flags = p4_entry.flags();
    // Check bit 63 (NX bit) directly
    let p4_raw = unsafe { core::ptr::read_volatile(&p4_table[p4_index] as *const _ as *const u64) };
    let p4_nx = (p4_raw >> 63) & 1;
    println!("  P4[{}] raw={:#x} NX={} flags={:?} addr={:#x}", 
        p4_index, p4_raw, p4_nx, p4_flags, p4_addr);
    
    if !p4_flags.contains(PageTableFlags::PRESENT) {
        println!("  -> P4 entry not present, stopping walk");
        return;
    }
    
    let p3_phys = match p4_entry.frame() {
        Ok(f) => f.start_address(),
        Err(_) => {
            println!("  -> P4 entry has no valid frame");
            return;
        }
    };
    
    // Walk P3
    let p3_table = unsafe { access_page_table(p3_phys) };
    let p3_entry = &p3_table[p3_index];
    let p3_raw = unsafe { core::ptr::read_volatile(&p3_table[p3_index] as *const _ as *const u64) };
    let p3_nx = (p3_raw >> 63) & 1;
    println!("  P3[{}] raw={:#x} NX={} flags={:?}", 
        p3_index, p3_raw, p3_nx, p3_entry.flags());
    
    if !p3_entry.flags().contains(PageTableFlags::PRESENT) {
        println!("  -> P3 entry not present, stopping walk");
        return;
    }
    
    let p2_phys = match p3_entry.frame() {
        Ok(f) => f.start_address(),
        Err(_) => {
            println!("  -> P3 entry has no valid frame");
            return;
        }
    };
    
    // Walk P2
    let p2_table = unsafe { access_page_table(p2_phys) };
    let p2_entry = &p2_table[p2_index];
    let p2_raw = unsafe { core::ptr::read_volatile(&p2_table[p2_index] as *const _ as *const u64) };
    let p2_nx = (p2_raw >> 63) & 1;
    println!("  P2[{}] raw={:#x} NX={} flags={:?}", 
        p2_index, p2_raw, p2_nx, p2_entry.flags());
    
    if !p2_entry.flags().contains(PageTableFlags::PRESENT) {
        println!("  -> P2 entry not present, stopping walk");
        return;
    }
    
    let p1_phys = match p2_entry.frame() {
        Ok(f) => f.start_address(),
        Err(_) => {
            println!("  -> P2 entry has no valid frame");
            return;
        }
    };
    
    // Walk P1
    let p1_table = unsafe { access_page_table(p1_phys) };
    let p1_entry = &p1_table[p1_index];
    let p1_raw = unsafe { core::ptr::read_volatile(&p1_table[p1_index] as *const _ as *const u64) };
    let p1_nx = (p1_raw >> 63) & 1;
    println!("  P1[{}] raw={:#x} NX={} flags={:?}", 
        p1_index, p1_raw, p1_nx, p1_entry.flags());
    
    if p1_entry.flags().contains(PageTableFlags::PRESENT) {
        if p1_nx == 1 {
            println!("  -> Page is PRESENT but NX bit is SET (not executable)!");
        } else {
            println!("  -> Page is PRESENT and executable (NX=0)");
        }
    } else {
        println!("  -> P1 entry not present");
    }
}
