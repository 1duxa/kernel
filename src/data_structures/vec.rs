extern crate alloc;

use core::fmt;
pub use alloc::vec::Vec;
pub use alloc::string::String;

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

/// Small in-crate ToString trait so `.to_string()` works without relying on std prelude.
/// Import this trait where you call `.to_string()` (e.g. `use crate::data_structures::vec::ToString;`)
pub trait ToString {
    fn to_string(&self) -> String;
}

impl<T: fmt::Display> ToString for T {
    fn to_string(&self) -> String {
        let mut s = String::new();
        let _ = core::fmt::write(&mut s, format_args!("{}", self));
        s
    }
}
