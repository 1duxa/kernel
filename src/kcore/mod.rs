//! # Core Kernel Module
//!
//! This module contains the fundamental kernel infrastructure including
//! interrupt handling, hardware initialization, and kernel status tracking.
//!
//! ## Submodules
//!
//! - `kernel`: Kernel initialization, status tracking, and component registration
//! - `interrupts`: IDT setup, exception handlers, PIC configuration, timer
//!
//! ## Initialization Order
//!
//! The kernel core is initialized early in the boot process:
//! 1. GDT (Global Descriptor Table) - segments and TSS
//! 2. IDT (Interrupt Descriptor Table) - exception and interrupt handlers  
//! 3. PIC (Programmable Interrupt Controller) - hardware interrupt routing

pub mod kernel;
pub mod interrupts;
