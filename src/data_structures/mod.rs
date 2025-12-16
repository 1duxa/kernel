//! # Data Structures Module
//!
//! Provides common data structures and utilities for use throughout the kernel.
//!
//! ## Modules
//!
//! - `vec`: Re-exports from `alloc` and utility functions for collections
//!
//! ## Overview
//!
//! Since this is a `no_std` kernel, we cannot use the standard library's
//! collections directly. This module bridges that gap by re-exporting
//! the `alloc` crate's collections and providing additional utilities
//! like number-to-string conversion.

pub mod vec;