//! Async Task Examples
//!
//! This module provides example task functions that can be scheduled
//! using the cooperative task scheduler. Tasks are non-preemptive and
//! must yield control by returning a `TaskState`.
//!
//! # Task States
//! - `Ready` - Task can be scheduled again
//! - `Blocked` - Task is waiting for something (will be rescheduled)
//! - `Completed` - Task has finished execution
//!
//! # Example
//! ```ignore
//! use crate::executor::spawn_task;
//! use crate::async_tasks::counter_task;
//! spawn_task("counter", counter_task);
//! ```

use crate::task::{TaskContext, TaskState};
use crate::data_structures::vec::String;

pub fn counter_task(ctx: &mut TaskContext) -> TaskState {
    ctx.iterations += 1;

    if ctx.iterations > 100 {
        TaskState::Completed
    } else {
        TaskState::Ready
    }
}

pub fn fibonacci_task(ctx: &mut TaskContext) -> TaskState {
    if ctx.data_len == 0 {
        let mut result = String::new();
        result.push_str("Fibonacci sequence: ");
        
        let mut a = 0u64;
        let mut b = 1u64;
        
        for _ in 0..15 {
            let c = a + b;
            a = b;
            b = c;
        }
        
        let data_str = String::from("Task computing fibonacci");
        ctx.store_data(data_str.as_bytes());
    }

    ctx.iterations += 1;
    
    if ctx.iterations >= 10 {
        TaskState::Completed
    } else {
        TaskState::Ready
    }
}

pub fn work_simulation_task(ctx: &mut TaskContext) -> TaskState {
    let work_amount = 50;
    
    if ctx.iterations >= work_amount {
        TaskState::Completed
    } else {
        ctx.iterations += 1;
        TaskState::Ready
    }
}

pub fn print_periodic_task(ctx: &mut TaskContext) -> TaskState {
    ctx.iterations += 1;

    if ctx.iterations > 5 {
        TaskState::Completed
    } else {
        TaskState::Ready
    }
}

pub fn cpu_intensive_task(ctx: &mut TaskContext) -> TaskState {
    let max_iterations = 200;
    
    let mut sum = 0u64;
    for i in 0..1000 {
        sum += i;
    }
    
    ctx.store_data(&sum.to_le_bytes());
    ctx.iterations += 1;
    
    if ctx.iterations >= max_iterations {
        TaskState::Completed
    } else {
        TaskState::Ready
    }
}

pub fn data_transform_task(ctx: &mut TaskContext) -> TaskState {
    let input = "transform_me";
    
    let mut result = String::new();
    for ch in input.chars() {
        result.push((ch as u8 + 1) as char);
    }
    
    ctx.store_data(result.as_bytes());
    ctx.iterations += 1;
    
    if ctx.iterations >= 3 {
        TaskState::Completed
    } else {
        TaskState::Ready
    }
}

pub fn blocking_task(ctx: &mut TaskContext) -> TaskState {
    const THRESHOLD: u64 = 100;
    
    if ctx.iterations < THRESHOLD {
        ctx.iterations += 1;
        TaskState::Blocked
    } else {
        TaskState::Completed
    }
}
