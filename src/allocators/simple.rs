
use core::alloc::{GlobalAlloc, Layout};
use core::ptr::{self};
use core::sync::atomic::{AtomicUsize, Ordering};

// ============================================================================
// 1. BUMP ALLOCATOR (Simple, fast, no deallocation)
// ============================================================================

pub struct BumpAllocator {
    heap_start: usize,
    heap_end: usize,
    next: AtomicUsize,
}

impl BumpAllocator {
    pub const fn new() -> Self {
        Self {
            heap_start: 0,
            heap_end: 0,
            next: AtomicUsize::new(0),
        }
    }

    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        self.heap_start = heap_start;
        self.heap_end = heap_start + heap_size;
        self.next.store(heap_start, Ordering::Relaxed);
    }
}

unsafe impl GlobalAlloc for BumpAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let size = layout.size();
        let align = layout.align();

        loop {
            let current = self.next.load(Ordering::Relaxed);
            let aligned = align_up(current, align);
            let new_next = aligned + size;

            if new_next > self.heap_end {
                return ptr::null_mut();
            }

            if self.next.compare_exchange(
                current,
                new_next,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ).is_ok() {
                return aligned as *mut u8;
            }
        }
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        // Bump allocator doesn't support deallocation
    }
}

// ============================================================================
// 2. LINKED LIST ALLOCATOR (Simple free list)
// ============================================================================

struct ListNode {
    size: usize,
    next: Option<&'static mut ListNode>,
}

impl ListNode {
    const fn new(size: usize) -> Self {
        Self { size, next: None }
    }

    fn start_addr(&self) -> usize {
        self as *const Self as usize
    }

    fn end_addr(&self) -> usize {
        self.start_addr() + self.size
    }
}

use core::cell::UnsafeCell;

pub struct LinkedListAllocator {
    head: UnsafeCell<Option<&'static mut ListNode>>,
}
impl LinkedListAllocator {
    pub const fn new() -> Self {
        Self { head: UnsafeCell::new(None) }
    }

    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        let head = &mut *(heap_start as *mut ListNode);
        head.size = heap_size;
        head.next = None;
        *self.head.get() = Some(head);
    }
}
unsafe impl GlobalAlloc for LinkedListAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let (size, align) = (layout.size(), layout.align());
        let size = align_up(size.max(core::mem::size_of::<ListNode>()), align);

        let head_ptr = self.head.get();
        let mut current = &mut *head_ptr;

        while let Some(ref mut node) = current {
            if let Ok(alloc_start) = Self::alloc_from_region(node, size, align) {
                return alloc_start as *mut u8;
            }
            current = &mut node.next;
        }
        
        ptr::null_mut()
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let size = align_up(layout.size().max(core::mem::size_of::<ListNode>()), layout.align());
        let node = &mut *(ptr as *mut ListNode);
        node.size = size;
        
        let head_ptr = self.head.get();
        node.next = (*head_ptr).take();
        *head_ptr = Some(node);
    }
}

impl LinkedListAllocator {
    unsafe fn alloc_from_region(node: &mut ListNode, size: usize, align: usize) -> Result<usize, ()> {
        let alloc_start = align_up(node.start_addr(), align);
        let alloc_end = alloc_start.checked_add(size).ok_or(())?;

        if alloc_end > node.end_addr() {
            return Err(());
        }

        let excess = node.end_addr() - alloc_end;
        if excess > 0 && excess < core::mem::size_of::<ListNode>() {
            return Err(());
        }

        // Remove node from free list if exact fit
        if alloc_start == node.start_addr() && alloc_end == node.end_addr() {
            // This will be handled by the caller
        } else if alloc_start == node.start_addr() {
            // Shrink node from the front
            node.size -= size;
            let new_addr = node.start_addr() + size;
            let new_node = &mut *(new_addr as *mut ListNode);
            new_node.size = node.size;
            new_node.next = node.next.take();
            node.size = new_node.size;
            node.next = new_node.next.take();
        } else if alloc_end == node.end_addr() {
            // Shrink node from the end
            node.size -= size;
        } else {
            // Split node into two
            let second_size = node.end_addr() - alloc_end;
            let second_node = &mut *(alloc_end as *mut ListNode);
            second_node.size = second_size;
            second_node.next = node.next.take();

            node.size = alloc_start - node.start_addr();
            node.next = Some(second_node);
        }

        Ok(alloc_start)
    }
}

// ============================================================================
// 3. FIXED SIZE BLOCK ALLOCATOR (Fast, no fragmentation)
// ============================================================================

const BLOCK_SIZES: &[usize] = &[8, 16, 32, 64, 128, 256, 512, 1024, 2048];

struct BlockNode {
    next: UnsafeCell<Option<&'static mut BlockNode>>,
}

pub struct FixedSizeBlockAllocator {
    list_heads: [Option<&'static mut BlockNode>; BLOCK_SIZES.len()],
    fallback: LinkedListAllocator,
}

impl FixedSizeBlockAllocator {
    pub const fn new() -> Self {
        const EMPTY: Option<&'static mut BlockNode> = None;
        Self {
            list_heads: [EMPTY; BLOCK_SIZES.len()],
            fallback: LinkedListAllocator::new(),
        }
    }

    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        self.fallback.init(heap_start, heap_size);
    }

    fn list_index(layout: &Layout) -> Option<usize> {
        let required_size = layout.size().max(layout.align());
        BLOCK_SIZES.iter().position(|&s| s >= required_size)
    }
}

unsafe impl GlobalAlloc for FixedSizeBlockAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        match Self::list_index(&layout) {
            Some(idx) => {
                let head_ptr = &self.list_heads[idx] as *const _ as *mut Option<&'static mut BlockNode>;
                match (*head_ptr).take() {
                    Some(node) => {
                        *head_ptr = node.next.get().take();
                        node as *mut BlockNode as *mut u8
                    }
                    None => {
                        let block_size = BLOCK_SIZES[idx];
                        let block_align = block_size;
                        let layout = Layout::from_size_align(block_size, block_align).unwrap();
                        self.fallback.alloc(layout)
                    }
                }
            }
            None => self.fallback.alloc(layout),
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        match Self::list_index(&layout) {
            Some(idx) => {
                let head_ptr = &self.list_heads[idx] as *const _ as *mut Option<&'static mut BlockNode>;
                let new_node = &mut *(ptr as *mut BlockNode);
                new_node.next = (*head_ptr).take().into();
                *head_ptr = Some(new_node);
            }
            None => self.fallback.dealloc(ptr, layout),
        }
    }
}

// ============================================================================
// 4. SLAB ALLOCATOR (Cache-aligned, object-specific)
// ============================================================================

pub struct SlabAllocator<const SIZE: usize, const ALIGN: usize> {
    head: Option<&'static mut BlockNode>,
}

impl<const SIZE: usize, const ALIGN: usize> SlabAllocator<SIZE, ALIGN> {
    pub const fn new() -> Self {
        Self { head: None }
    }

    pub unsafe fn add_slab(&mut self, slab_start: usize, slab_size: usize) {
        let num_blocks = slab_size / SIZE;
        for i in 0..num_blocks {
            let addr = slab_start + i * SIZE;
            let node = &mut *(addr as *mut BlockNode);
            node.next = self.head.take().into();
            self.head = Some(node);
        }
    }
}

unsafe impl<const SIZE: usize, const ALIGN: usize> GlobalAlloc for SlabAllocator<SIZE, ALIGN> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if layout.size() > SIZE || layout.align() > ALIGN {
            return ptr::null_mut();
        }

        let head_ptr = &self.head as *const _ as *mut Option<&'static mut BlockNode>;
        match (*head_ptr).take() {
            Some(node) => {
                *head_ptr = node.next.take();
                node as *mut BlockNode as *mut u8
            }
            None => ptr::null_mut(),
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        let head_ptr = &self.head as *const _ as *mut Option<&'static mut BlockNode>;
        let node = &mut *(ptr as *mut BlockNode);
        node.next = (*head_ptr).take().into();
        *head_ptr = Some(node);
    }
}

// ============================================================================
// 5. BUDDY ALLOCATOR (Power-of-2 sizes, efficient merging)
// ============================================================================

const MAX_ORDER: usize = 11; // 4KB blocks max

struct BuddyBlock {
    next: Option<&'static mut BuddyBlock>,
}

pub struct BuddyAllocator {
    free_lists: [Option<&'static mut BuddyBlock>; MAX_ORDER],
    heap_start: usize,
    heap_size: usize,
}

impl BuddyAllocator {
    pub const fn new() -> Self {
        const EMPTY: Option<&'static mut BuddyBlock> = None;
        Self {
            free_lists: [EMPTY; MAX_ORDER],
            heap_start: 0,
            heap_size: 0,
        }
    }

    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        self.heap_start = heap_start;
        self.heap_size = heap_size;
        
        let order = self.size_to_order(heap_size);
        let block = &mut *(heap_start as *mut BuddyBlock);
        block.next = None;
        self.free_lists[order] = Some(block);
    }

    fn size_to_order(&self, size: usize) -> usize {
        (size.next_power_of_two().trailing_zeros() as usize).min(MAX_ORDER - 1)
    }

    fn order_to_size(&self, order: usize) -> usize {
        1 << order
    }

    unsafe fn split_block(&mut self, order: usize) -> Option<&'static mut BuddyBlock> {
        if order == 0 {
            return None;
        }

        if self.free_lists[order].is_none() {
            self.split_block(order + 1)?;
        }

        let block = self.free_lists[order].take()?;
        let buddy_addr = (block as *mut BuddyBlock as usize) + self.order_to_size(order - 1);
        let buddy = &mut *(buddy_addr as *mut BuddyBlock);
        
        buddy.next = self.free_lists[order - 1].take();
        self.free_lists[order - 1] = Some(buddy);
        
        Some(block)
    }
}

unsafe impl GlobalAlloc for BuddyAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let size = layout.size().max(layout.align()).next_power_of_two();
        let order = (size.trailing_zeros() as usize).min(MAX_ORDER - 1);

        let allocator = &mut *(self as *const _ as *mut Self);
        
        for current_order in order..MAX_ORDER {
            if allocator.free_lists[current_order].is_some() {
                while current_order > order {
                    allocator.split_block(current_order);
                }
                
                if let Some(block) = allocator.free_lists[order].take() {
                    allocator.free_lists[order] = block.next.take();
                    return block as *mut BuddyBlock as *mut u8;
                }
            }
        }
        
        ptr::null_mut()
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let size = layout.size().max(layout.align()).next_power_of_two();
        let order = (size.trailing_zeros() as usize).min(MAX_ORDER - 1);
        
        let allocator = &mut *(self as *const _ as *mut Self);
        let block = &mut *(ptr as *mut BuddyBlock);
        block.next = allocator.free_lists[order].take();
        allocator.free_lists[order] = Some(block);
    }
}

// ============================================================================
// 6. STACK ALLOCATOR (LIFO allocation/deallocation)
// ============================================================================

pub struct StackAllocator {
    start: usize,
    end: usize,
    top: AtomicUsize,
}

impl StackAllocator {
    pub const fn new() -> Self {
        Self {
            start: 0,
            end: 0,
            top: AtomicUsize::new(0),
        }
    }

    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        self.start = heap_start;
        self.end = heap_start + heap_size;
        self.top.store(heap_start, Ordering::Relaxed);
    }

    pub fn reset(&self) {
        self.top.store(self.start, Ordering::Relaxed);
    }
}

unsafe impl GlobalAlloc for StackAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let size = layout.size();
        let align = layout.align();

        loop {
            let current = self.top.load(Ordering::Relaxed);
            let aligned = align_up(current, align);
            let new_top = aligned + size;

            if new_top > self.end {
                return ptr::null_mut();
            }

            if self.top.compare_exchange(
                current,
                new_top,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ).is_ok() {
                return aligned as *mut u8;
            }
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let addr = ptr as usize;
        let expected = addr + layout.size();
        let _ = self.top.compare_exchange(
            expected,
            addr,
            Ordering::Relaxed,
            Ordering::Relaxed,
        );
    }
}

// ============================================================================
// UTILITY FUNCTIONS
// ============================================================================

fn align_up(addr: usize, align: usize) -> usize {
    (addr + align - 1) & !(align - 1)
}

#[inline]
fn align_down(addr: usize, align: usize) -> usize {
    addr & !(align - 1)
}

// ============================================================================
// 7. POOL ALLOCATOR (Pre-allocated chunks)
// ============================================================================
// pub struct PoolAllocator<const CHUNK_SIZE: usize, const NUM_CHUNKS: usize>
// where
//     [u64; (NUM_CHUNKS + 63) / 64]: Sized,
// {
//     bitmap: [u64; (NUM_CHUNKS + 63) / 64],
//     base: usize,
// }

// impl<const CHUNK_SIZE: usize, const NUM_CHUNKS: usize> PoolAllocator<CHUNK_SIZE, NUM_CHUNKS> where [(); (NUM_CHUNKS + 63) / 64]: {
//     pub const fn new() -> Self {
//         Self {
//             bitmap: [0; (NUM_CHUNKS + 63) / 64],
//             base: 0,
//         }
//     }

//     pub unsafe fn init(&mut self, base: usize) {
//         self.base = base;
//     }

//     fn find_free_chunk(&self) -> Option<usize> {
//         for (word_idx, &word) in self.bitmap.iter().enumerate() {
//             if word != !0 {
//                 let bit_idx = (!word).trailing_zeros() as usize;
//                 let chunk_idx = word_idx * 64 + bit_idx;
//                 if chunk_idx < NUM_CHUNKS {
//                     return Some(chunk_idx);
//                 }
//             }
//         }
//         None
//     }

//     fn mark_used(&mut self, chunk_idx: usize) {
//         let word_idx = chunk_idx / 64;
//         let bit_idx = chunk_idx % 64;
//         self.bitmap[word_idx] |= 1u64 << bit_idx;
//     }

//     fn mark_free(&mut self, chunk_idx: usize) {
//         let word_idx = chunk_idx / 64;
//         let bit_idx = chunk_idx % 64;
//         self.bitmap[word_idx] &= !(1u64 << bit_idx);
//     }
// }

// unsafe impl<const CHUNK_SIZE: usize, const NUM_CHUNKS: usize> GlobalAlloc 
//     for PoolAllocator<CHUNK_SIZE, NUM_CHUNKS> where [(); (NUM_CHUNKS + 63) / 64]: 
// {
//     unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
//         if layout.size() > CHUNK_SIZE {
//             return ptr::null_mut();
//         }

//         let allocator = &mut *(self as *const _ as *mut Self);
        
//         if let Some(idx) = allocator.find_free_chunk() {
//             allocator.mark_used(idx);
//             (allocator.base + idx * CHUNK_SIZE) as *mut u8
//         } else {
//             ptr::null_mut()
//         }
//     }

//     unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
//         let allocator = &mut *(self as *const _ as *mut Self);
//         let addr = ptr as usize;
//         let chunk_idx = (addr - allocator.base) / CHUNK_SIZE;
//         allocator.mark_free(chunk_idx);
//     }
// }