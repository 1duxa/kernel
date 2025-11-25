use core::alloc::{GlobalAlloc, Layout};
use core::cell::UnsafeCell;
use core::ptr::{self, NonNull};
use core::sync::atomic::{AtomicUsize, Ordering};

use crate::memory::allocators::core::{align_up, align_down, AllocError, SpinLock, validate_region};

// ============================================================================
// 1. BUMP ALLOCATOR (Simple, fast, no deallocation)
// ============================================================================

/// A simple bump allocator that never frees memory.
/// Best for: short-lived allocations, initialization, temporary buffers
/// 
/// # Safety
/// - Must call `init()` before use
/// - Thread-safe through atomic operations
/// - Never reuses memory until reset
pub struct BumpAllocator {
    heap_start: AtomicUsize,
    heap_end: AtomicUsize,
    next: AtomicUsize,
    initialized: AtomicUsize,
}

impl BumpAllocator {
    pub const fn new() -> Self {
        Self {
            heap_start: AtomicUsize::new(0),
            heap_end: AtomicUsize::new(0),
            next: AtomicUsize::new(0),
            initialized: AtomicUsize::new(0),
        }
    }

    /// Initialize the allocator with a memory region
    /// 
    /// # Safety
    /// - `heap_start` must point to valid, unused memory
    /// - `heap_size` must not exceed available memory
    /// - Must only be called once
    pub unsafe fn init(&self, heap_start: usize, heap_size: usize) -> Result<(), AllocError> {
        validate_region(heap_start, heap_size)?;
        
        if self.initialized.swap(1, Ordering::SeqCst) != 0 {
            return Err(AllocError::InvalidAddress); // Already initialized
        }

        self.heap_start.store(heap_start, Ordering::Release);
        self.heap_end.store(heap_start + heap_size, Ordering::Release);
        self.next.store(heap_start, Ordering::Release);
        Ok(())
    }

    fn is_initialized(&self) -> bool {
        self.initialized.load(Ordering::Acquire) != 0
    }

    /// Reset the allocator, invalidating all previous allocations
    /// 
    /// # Safety
    /// - All previously allocated memory becomes invalid
    /// - Caller must ensure no references to allocated memory exist
    pub unsafe fn reset(&self) {
        if self.is_initialized() {
            let start = self.heap_start.load(Ordering::Acquire);
            self.next.store(start, Ordering::Release);
        }
    }

    pub fn used(&self) -> usize {
        if !self.is_initialized() {
            return 0;
        }
        let start = self.heap_start.load(Ordering::Acquire);
        let next = self.next.load(Ordering::Acquire);
        next.saturating_sub(start)
    }

    pub fn remaining(&self) -> usize {
        if !self.is_initialized() {
            return 0;
        }
        let end = self.heap_end.load(Ordering::Acquire);
        let next = self.next.load(Ordering::Acquire);
        end.saturating_sub(next)
    }
}

unsafe impl GlobalAlloc for BumpAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if !self.is_initialized() {
            return ptr::null_mut();
        }

        let size = layout.size();
        let align = layout.align();

        if size == 0 {
            return ptr::null_mut();
        }

        let heap_end = self.heap_end.load(Ordering::Acquire);

        loop {
            let current = self.next.load(Ordering::Acquire);
            let aligned = align_up(current, align);
            
            let new_next = match aligned.checked_add(size) {
                Some(n) => n,
                None => return ptr::null_mut(),
            };

            if new_next > heap_end {
                return ptr::null_mut();
            }

            if self.next.compare_exchange_weak(
                current,
                new_next,
                Ordering::AcqRel,
                Ordering::Acquire,
            ).is_ok() {
                return aligned as *mut u8;
            }
        }
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        // Bump allocator doesn't support individual deallocation
    }
}

// ============================================================================
// 2. LINKED LIST ALLOCATOR (Simple free list with proper locking)
// ============================================================================

struct ListNode {
    size: usize,
    next: Option<NonNull<ListNode>>,
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

struct LinkedListAllocatorInner {
    head: Option<NonNull<ListNode>>,
    initialized: bool,
}

/// A linked list allocator with proper synchronization
/// Best for: general-purpose allocation when fragmentation is acceptable
/// 
/// # Safety
/// - Must call `init()` before use
/// - Thread-safe through spin lock
pub struct LinkedListAllocator {
    inner: UnsafeCell<LinkedListAllocatorInner>,
    lock: SpinLock,
}

// Safety: The UnsafeCell is protected by SpinLock
unsafe impl Sync for LinkedListAllocator {}

impl LinkedListAllocator {
    pub const fn new() -> Self {
        Self {
            inner: UnsafeCell::new(LinkedListAllocatorInner {
                head: None,
                initialized: false,
            }),
            lock: SpinLock::new(),
        }
    }

    /// Initialize the allocator with a memory region
    /// 
    /// # Safety
    /// - `heap_start` must point to valid, unused memory
    /// - `heap_size` must be large enough for at least one ListNode
    /// - Must only be called once
    pub unsafe fn init(&self, heap_start: usize, heap_size: usize) -> Result<(), AllocError> {
        validate_region(heap_start, heap_size)?;

        if heap_size < core::mem::size_of::<ListNode>() {
            return Err(AllocError::InvalidSize);
        }

        self.lock.with_lock(|| {
            let inner = &mut *self.inner.get();
            
            if inner.initialized {
                return Err(AllocError::InvalidAddress); // Already initialized
            }

            let node_ptr = heap_start as *mut ListNode;
            node_ptr.write(ListNode::new(heap_size));
            inner.head = NonNull::new(node_ptr);
            inner.initialized = true;
            Ok(())
        })
    }

    fn alloc_from_region(
        node: &mut ListNode,
        size: usize,
        align: usize,
    ) -> Result<usize, AllocError> {
        let alloc_start = align_up(node.start_addr(), align);
        let alloc_end = alloc_start
            .checked_add(size)
            .ok_or(AllocError::Overflow)?;

        if alloc_end > node.end_addr() {
            return Err(AllocError::OutOfMemory);
        }

        let excess = node.end_addr() - alloc_end;
        if excess > 0 && excess < core::mem::size_of::<ListNode>() {
            // Not enough space for a new node
            return Err(AllocError::OutOfMemory);
        }

        Ok(alloc_start)
    }
}

unsafe impl GlobalAlloc for LinkedListAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let size = layout.size();
        let align = layout.align();

        if size == 0 {
            return ptr::null_mut();
        }

        let required_size = align_up(
            size.max(core::mem::size_of::<ListNode>()),
            align
        );

        self.lock.with_lock(|| {
            let inner = &mut *self.inner.get();
            
            if !inner.initialized {
                return ptr::null_mut();
            }

            let mut current = &mut inner.head;

            while let Some(mut node_ptr) = *current {
                let node = node_ptr.as_mut();
                
                match Self::alloc_from_region(node, required_size, align) {
                    Ok(alloc_start) => {
                        // Remove or split the node
                        if alloc_start == node.start_addr() && 
                           alloc_start + required_size == node.end_addr() {
                            // Exact fit - remove node
                            *current = node.next;
                        } else if alloc_start == node.start_addr() {
                            // Allocate from front - shrink node
                            let new_start = alloc_start + required_size;
                            let new_size = node.end_addr() - new_start;
                            let new_node_ptr = new_start as *mut ListNode;
                            new_node_ptr.write(ListNode {
                                size: new_size,
                                next: node.next,
                            });
                            *current = NonNull::new(new_node_ptr);
                        } else if alloc_start + required_size == node.end_addr() {
                            // Allocate from end - shrink node
                            node.size = alloc_start - node.start_addr();
                        } else {
                            // Split node
                            let second_start = alloc_start + required_size;
                            let second_size = node.end_addr() - second_start;
                            let second_ptr = second_start as *mut ListNode;
                            second_ptr.write(ListNode {
                                size: second_size,
                                next: node.next,
                            });
                            node.size = alloc_start - node.start_addr();
                            node.next = NonNull::new(second_ptr);
                        }
                        return alloc_start as *mut u8;
                    }
                    Err(_) => {
                        current = &mut node.next;
                    }
                }
            }
            
            ptr::null_mut()
        })
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if ptr.is_null() {
            return;
        }

        let size = align_up(
            layout.size().max(core::mem::size_of::<ListNode>()),
            layout.align()
        );

        self.lock.with_lock(|| {
            let inner = &mut *self.inner.get();
            
            if !inner.initialized {
                return;
            }

            let node_ptr = ptr as *mut ListNode;
            node_ptr.write(ListNode {
                size,
                next: inner.head,
            });
            inner.head = NonNull::new(node_ptr);
        });
    }
}

// ============================================================================
// 3. FIXED SIZE BLOCK ALLOCATOR (Fast, minimal fragmentation)
// ============================================================================

const BLOCK_SIZES: &[usize] = &[8, 16, 32, 64, 128, 256, 512, 1024, 2048];

struct BlockNode {
    next: Option<NonNull<BlockNode>>,
}

struct FixedSizeBlockAllocatorInner {
    list_heads: [Option<NonNull<BlockNode>>; BLOCK_SIZES.len()],
    fallback: LinkedListAllocator,
}

/// Fixed-size block allocator with fallback
/// Best for: frequent allocations of similar sizes
/// 
/// # Safety
/// - Must call `init()` before use
/// - Thread-safe through spin lock
pub struct FixedSizeBlockAllocator {
    inner: UnsafeCell<FixedSizeBlockAllocatorInner>,
    lock: SpinLock,
}

// Safety: The UnsafeCell is protected by SpinLock
unsafe impl Sync for FixedSizeBlockAllocator {}
unsafe impl Send for FixedSizeBlockAllocator {}

impl FixedSizeBlockAllocator {
    pub const fn new() -> Self {
        Self {
            inner: UnsafeCell::new(FixedSizeBlockAllocatorInner {
                list_heads: [None; BLOCK_SIZES.len()],
                fallback: LinkedListAllocator::new(),
            }),
            lock: SpinLock::new(),
        }
    }

    /// Initialize the allocator with a memory region
    /// 
    /// # Safety
    /// - `heap_start` must point to valid, unused memory
    /// - `heap_size` must be sufficient for allocation
    pub unsafe fn init(&self, heap_start: usize, heap_size: usize) -> Result<(), AllocError> {
        self.lock.with_lock(|| {
            let inner = &mut *self.inner.get();
            inner.fallback.init(heap_start, heap_size)
        })
    }

    fn list_index(layout: &Layout) -> Option<usize> {
        let required_size = layout.size().max(layout.align());
        BLOCK_SIZES.iter().position(|&s| s >= required_size)
    }
}

unsafe impl GlobalAlloc for FixedSizeBlockAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if layout.size() == 0 {
            return ptr::null_mut();
        }

        match Self::list_index(&layout) {
            Some(idx) => self.lock.with_lock(|| {
                let inner = &mut *self.inner.get();
                
                if let Some(mut node_ptr) = inner.list_heads[idx] {
                    let node = node_ptr.as_mut();
                    inner.list_heads[idx] = node.next;
                    node_ptr.as_ptr() as *mut u8
                } else {
                    let block_size = BLOCK_SIZES[idx];
                    let block_layout = Layout::from_size_align(block_size, block_size)
                        .unwrap_or(layout);
                    inner.fallback.alloc(block_layout)
                }
            }),
            None => {
                self.lock.with_lock(|| {
                    let inner = &mut *self.inner.get();
                    inner.fallback.alloc(layout)
                })
            }
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if ptr.is_null() {
            return;
        }

        match Self::list_index(&layout) {
            Some(idx) => self.lock.with_lock(|| {
                let inner = &mut *self.inner.get();
                let node_ptr = ptr as *mut BlockNode;
                (*node_ptr).next = inner.list_heads[idx];
                inner.list_heads[idx] = NonNull::new(node_ptr);
            }),
            None => {
                self.lock.with_lock(|| {
                    let inner = &mut *self.inner.get();
                    inner.fallback.dealloc(ptr, layout)
                });
            }
        }
    }
}

// ============================================================================
// 4. STACK ALLOCATOR (LIFO allocation/deallocation)
// ============================================================================

/// A stack allocator for LIFO allocation patterns
/// Best for: temporary allocations with predictable lifetimes
/// 
/// # Safety
/// - Must call `init()` before use
/// - Deallocations must happen in reverse order of allocations
/// - Thread-safe through atomic operations
pub struct StackAllocator {
    start: AtomicUsize,
    end: AtomicUsize,
    top: AtomicUsize,
    initialized: AtomicUsize,
}

impl StackAllocator {
    pub const fn new() -> Self {
        Self {
            start: AtomicUsize::new(0),
            end: AtomicUsize::new(0),
            top: AtomicUsize::new(0),
            initialized: AtomicUsize::new(0),
        }
    }

    /// Initialize the allocator with a memory region
    /// 
    /// # Safety
    /// - `heap_start` must point to valid, unused memory
    /// - `heap_size` must not exceed available memory
    /// - Must only be called once
    pub unsafe fn init(&self, heap_start: usize, heap_size: usize) -> Result<(), AllocError> {
        validate_region(heap_start, heap_size)?;
        
        if self.initialized.swap(1, Ordering::SeqCst) != 0 {
            return Err(AllocError::InvalidAddress);
        }

        self.start.store(heap_start, Ordering::Release);
        self.end.store(heap_start + heap_size, Ordering::Release);
        self.top.store(heap_start, Ordering::Release);
        Ok(())
    }

    /// Reset the allocator to initial state
    /// 
    /// # Safety
    /// - All previously allocated memory becomes invalid
    /// - Caller must ensure no references to allocated memory exist
    pub unsafe fn reset(&self) {
        if self.initialized.load(Ordering::Acquire) != 0 {
            let start = self.start.load(Ordering::Acquire);
            self.top.store(start, Ordering::Release);
        }
    }

    pub fn used(&self) -> usize {
        let start = self.start.load(Ordering::Acquire);
        let top = self.top.load(Ordering::Acquire);
        top.saturating_sub(start)
    }
}

unsafe impl GlobalAlloc for StackAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if self.initialized.load(Ordering::Acquire) == 0 {
            return ptr::null_mut();
        }

        let size = layout.size();
        let align = layout.align();

        if size == 0 {
            return ptr::null_mut();
        }

        let end = self.end.load(Ordering::Acquire);

        loop {
            let current = self.top.load(Ordering::Acquire);
            let aligned = align_up(current, align);
            
            let new_top = match aligned.checked_add(size) {
                Some(n) => n,
                None => return ptr::null_mut(),
            };

            if new_top > end {
                return ptr::null_mut();
            }

            if self.top.compare_exchange_weak(
                current,
                new_top,
                Ordering::AcqRel,
                Ordering::Acquire,
            ).is_ok() {
                return aligned as *mut u8;
            }
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if ptr.is_null() {
            return;
        }

        // Only allow deallocation if it's the most recent allocation (LIFO)
        let addr = ptr as usize;
        let size = layout.size();
        let expected_top = addr.saturating_add(size);
        
        // Try to pop this allocation off the stack
        let _ = self.top.compare_exchange(
            expected_top,
            addr,
            Ordering::AcqRel,
            Ordering::Acquire,
        );
        // If this fails, it means deallocations are out of order
        // In a production OS, you might want to panic or log this
    }
}

// ============================================================================
// 5. SLAB ALLOCATOR (Cache-aligned, object-specific)
// ============================================================================

struct SlabAllocatorInner<const SIZE: usize, const ALIGN: usize> {
    head: Option<NonNull<BlockNode>>,
}

pub struct SlabAllocator<const SIZE: usize, const ALIGN: usize> {
    inner: UnsafeCell<SlabAllocatorInner<SIZE, ALIGN>>,
    lock: SpinLock,
}

// Safety: The UnsafeCell is protected by SpinLock
unsafe impl<const SIZE: usize, const ALIGN: usize> Sync for SlabAllocator<SIZE, ALIGN> {}

impl<const SIZE: usize, const ALIGN: usize> SlabAllocator<SIZE, ALIGN> {
    pub const fn new() -> Self {
        Self {
            inner: UnsafeCell::new(SlabAllocatorInner { head: None }),
            lock: SpinLock::new(),
        }
    }

    pub unsafe fn add_slab(&self, slab_start: usize, slab_size: usize) {
        let num_blocks = slab_size / SIZE;
        
        self.lock.with_lock(|| {
            let inner = &mut *self.inner.get();
            
            for i in 0..num_blocks {
                let addr = slab_start + i * SIZE;
                let node = &mut *(addr as *mut BlockNode);
                node.next = inner.head;
                inner.head = NonNull::new(node as *mut BlockNode);
            }
        });
    }
}

unsafe impl<const SIZE: usize, const ALIGN: usize> GlobalAlloc for SlabAllocator<SIZE, ALIGN> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if layout.size() > SIZE || layout.align() > ALIGN {
            return ptr::null_mut();
        }
        
        self.lock.with_lock(|| {
            let inner = &mut *self.inner.get();
            
            match inner.head {
                Some(mut node_ptr) => {
                    let node = node_ptr.as_mut();
                    inner.head = node.next;
                    node_ptr.as_ptr() as *mut u8
                }
                None => ptr::null_mut(),
            }
        })
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        if ptr.is_null() {
            return;
        }
        
        self.lock.with_lock(|| {
            let inner = &mut *self.inner.get();
            let node = &mut *(ptr as *mut BlockNode);
            node.next = inner.head;
            inner.head = NonNull::new(node);
        });
    }
}

// ============================================================================
// 6. HEAP ALLOCATOR WRAPPER (For separating stack/heap regions)
// ============================================================================

/// A wrapper that provides separate stack and heap allocation regions
pub struct StackHeapAllocator {
    stack: StackAllocator,
    heap: FixedSizeBlockAllocator,
}

impl StackHeapAllocator {
    pub const fn new() -> Self {
        Self {
            stack: StackAllocator::new(),
            heap: FixedSizeBlockAllocator::new(),
        }
    }

    /// Initialize with separate stack and heap regions
    /// 
    /// # Safety
    /// - Regions must not overlap
    /// - Both regions must point to valid, unused memory
    pub unsafe fn init(
        &self,
        stack_start: usize,
        stack_size: usize,
        heap_start: usize,
        heap_size: usize,
    ) -> Result<(), AllocError> {
        // Verify regions don't overlap
        let stack_end = stack_start.checked_add(stack_size)
            .ok_or(AllocError::Overflow)?;
        let heap_end = heap_start.checked_add(heap_size)
            .ok_or(AllocError::Overflow)?;

        if (stack_start < heap_end && stack_end > heap_start) ||
           (heap_start < stack_end && heap_end > stack_start) {
            return Err(AllocError::InvalidAddress);
        }

        self.stack.init(stack_start, stack_size)?;
        self.heap.init(heap_start, heap_size)?;
        Ok(())
    }

    pub fn stack(&self) -> &StackAllocator {
        &self.stack
    }

    pub fn heap(&self) -> &FixedSizeBlockAllocator {
        &self.heap
    }
}

unsafe impl GlobalAlloc for StackHeapAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // Use heap for all allocations by default
        self.heap.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.heap.dealloc(ptr, layout)
    }
}