use core::alloc::{GlobalAlloc, Layout};
use core::ptr::{self, NonNull};
use core::mem;
use core::ops::{Deref, DerefMut};

extern "C" {
    fn kmalloc(size: usize, align: usize) -> *mut u8;
    fn kfree(ptr: *mut u8);
}

#[derive(Debug)]
pub struct Vec<T> {
    ptr: NonNull<T>,
    len: usize,
    cap: usize,
}

impl<T> Vec<T> {
    pub const fn new() -> Self {
        Self {
            ptr: NonNull::dangling(),
            len: 0,
            cap: 0,
        }
    }

    pub fn with_capacity(cap: usize) -> Self {
        if cap == 0 {
            return Self::new();
        }

        let layout = Layout::array::<T>(cap).unwrap();
        let ptr = unsafe { kmalloc(layout.size(), layout.align()) as *mut T };
        
        Self {
            ptr: NonNull::new(ptr).expect("allocation failed"),
            len: 0,
            cap,
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn capacity(&self) -> usize {
        self.cap
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn push(&mut self, value: T) {
        if self.len == self.cap {
            self.grow();
        }

        unsafe {
            ptr::write(self.ptr.as_ptr().add(self.len), value);
        }
        self.len += 1;
    }

    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            return None;
        }

        self.len -= 1;
        unsafe { Some(ptr::read(self.ptr.as_ptr().add(self.len))) }
    }

    pub fn as_slice(&self) -> &[T] {
        unsafe { core::slice::from_raw_parts(self.ptr.as_ptr(), self.len) }
    }

    pub fn as_mut_slice(&mut self) -> &mut [T] {
        unsafe { core::slice::from_raw_parts_mut(self.ptr.as_ptr(), self.len) }
    }
    pub fn remove(&mut self, index: usize) -> T {
        assert!(index < self.len, "index out of bounds");
        unsafe {
            let ptr = self.ptr.as_ptr().add(index);
            let value = ptr::read(ptr);
            ptr::copy(ptr.add(1), ptr, self.len - index - 1);
            self.len -= 1;
            value
        }
    }
    fn grow(&mut self) {
        let new_cap = if self.cap == 0 { 4 } else { self.cap * 2 };
        let new_layout = Layout::array::<T>(new_cap).unwrap();
        
        let new_ptr = unsafe { kmalloc(new_layout.size(), new_layout.align()) as *mut T };
        let new_ptr = NonNull::new(new_ptr).expect("allocation failed");

        if self.cap > 0 {
            unsafe {
                ptr::copy_nonoverlapping(self.ptr.as_ptr(), new_ptr.as_ptr(), self.len);
                kfree(self.ptr.as_ptr() as *mut u8);
            }
        }

        self.ptr = new_ptr;
        self.cap = new_cap;
    }

    pub fn clear(&mut self) {
        while self.pop().is_some() {}
    }
}
impl<T: Clone> Clone for Vec<T> { 
    fn clone(&self) -> Self { 
        let mut new_vec = Vec::with_capacity(self.capacity()); 
        for item in self.iter() { 
            new_vec.push(item.clone()); 
        } 
        new_vec 
    } 
}


impl<T> Drop for Vec<T> {
    fn drop(&mut self) {
        self.clear();
        
        if self.cap > 0 {
            unsafe {
                kfree(self.ptr.as_ptr() as *mut u8);
            }
        }
    }
}
impl<T> FromIterator<T> for Vec<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut vec = Vec::new();
        for item in iter {
            vec.push(item);
        }
        vec
    }
}
pub struct IntoIter<A> {
    vec: Vec<A>,
    index: usize,
}

impl<A> Iterator for IntoIter<A> {
    type Item = A;

    fn next(&mut self) -> Option<A> {
        if self.index < self.vec.len {
            let item = unsafe { ptr::read(self.vec.ptr.as_ptr().add(self.index)) };
            self.index += 1;
            Some(item)
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.vec.len - self.index;
        (len, Some(len))
    }
}

impl<A> IntoIterator for Vec<A> {
    type Item = A;
    type IntoIter = IntoIter<A>;

    fn into_iter(self) -> Self::IntoIter {
        IntoIter { vec: self, index: 0 }
    }
}


impl<T> Deref for Vec<T> {
    type Target = [T];

    fn deref(&self) -> &[T] {
        self.as_slice()
    }
}

impl<T> DerefMut for Vec<T> {
    fn deref_mut(&mut self) -> &mut [T] {
        self.as_mut_slice()
    }
}
#[macro_export]
macro_rules! vec {
    // repeat form: requires Clone for the element
    ( $elem:expr ; $n:expr ) => {{
        let count = $n;
        let mut v = crate::data_structures::vec::Vec::with_capacity(count as usize);
        let mut i = 0usize;
        while i < (count as usize) {
            v.push($elem.clone());
            i += 1;
        }
        v
    }};
    // list form (allow trailing comma)
    ( $( $x:expr ),* $(,)? ) => {{
        let mut v = crate::data_structures::vec::Vec::new();
        $(
            v.push($x);
        )*
        v
    }};
}
#[derive(Debug, Clone)]
pub struct String {
    vec: Vec<u8>,
}

impl String {
    pub const fn new() -> Self {
        Self { vec: Vec::new() }
    }

    pub fn with_capacity(cap: usize) -> Self {
        Self {
            vec: Vec::with_capacity(cap),
        }
    }

    pub fn from_str(s: &str) -> Self {
        let mut string = Self::with_capacity(s.len());
        string.push_str(s);
        string
    }

    pub fn push_str(&mut self, s: &str) {
        for &b in s.as_bytes() {
            self.vec.push(b);
        }
    }

    pub fn push(&mut self, c: char) {
        let mut buf = [0u8; 4];
        let s = c.encode_utf8(&mut buf);
        self.push_str(s);
    }

    pub fn as_str(&self) -> &str {
        unsafe { core::str::from_utf8_unchecked(self.vec.as_slice()) }
    }

    pub fn len(&self) -> usize {
        self.vec.len()
    }

    pub fn is_empty(&self) -> bool {
        self.vec.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.vec.capacity()
    }

    pub fn clear(&mut self) {
        self.vec.clear();
    }
    pub fn from_display<T: core::fmt::Display>(v: T) -> Self {
        let mut s = Self::new();
        // core::fmt::write uses our core::fmt::Write impl for String
        let _ = core::fmt::write(&mut s, format_args!("{}", v));
        s
    }
}
impl Deref for String {
    type Target = str;

    fn deref(&self) -> &str {
        self.as_str()
    }
}

impl core::fmt::Write for String {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.push_str(s);
        Ok(())
    }
}

impl core::fmt::Display for String {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        core::fmt::Display::fmt(self.as_str(), f)
    }
}
#[allow(non_camel_case_types)]
pub trait ToString {
    fn to_string(&self) -> String;
}

impl<T: core::fmt::Display> ToString for T {
    fn to_string(&self) -> String {
        let mut s = String::new();
        let _ = core::fmt::write(&mut s, format_args!("{}", self));
        s
    }
}