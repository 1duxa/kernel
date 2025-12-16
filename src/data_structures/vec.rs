//! # Vector and Collection Utilities
//!
//! Re-exports `alloc` crate collections and provides utility functions
//! for working with strings and numbers in a `no_std` environment.
//!
//! ## Exports
//!
//! - `Vec`: Dynamic array from `alloc::vec`
//! - `String`: UTF-8 string from `alloc::string`
//! - `vec!` macro: Convenient vector creation
//!
//! ## Functions
//!
//! - `number_to_string`: Converts u64 to decimal string representation
//!
//! ## Example
//!
//! ```ignore
//! use crate::data_structures::vec::*;
//!
//! let nums = vec![1, 2, 3];
//! let repeated = vec![0u8; 10];
//! let s = number_to_string(42); // "42"
//! ```

extern crate alloc;

pub use alloc::vec::Vec;
pub use alloc::string::String;

pub fn number_to_string(mut n: u64) -> String {
    if n == 0 {
        return String::from("0");
    }
    
    let mut digits = Vec::new();
    while n > 0 {
        digits.push((b'0' + (n % 10) as u8) as char);
        n /= 10;
    }
    
    let mut result = String::new();
    for i in (0..digits.len()).rev() {
        result.push(digits[i]);
    }
    result
}

/// Small `vec!` macro that builds an alloc::Vec in no_std with alloc support.
/// Usage:
///  - vec![a, b, c]
///  - vec![elem; n]  (requires elem: Clone)
#[macro_export]
macro_rules! vec {
    // repeat form: requires Clone for the element
    ( $elem:expr ; $n:expr ) => {{
        let count = $n;
        let mut v = Vec::with_capacity(count as usize);
        let mut i = 0usize;
        while i < (count as usize) {
            v.push($elem.clone());
            i += 1;
        }
        v
    }};
    // list form (allow trailing comma)
    ( $( $x:expr ),* $(,)? ) => {{
        let mut v = alloc::vec::Vec::new();
        $(
            v.push($x);
        )*
        v
    }};
}


