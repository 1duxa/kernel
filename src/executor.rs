//! Simple Cooperative Task Executor
//!
//! This module provides a basic cooperative multitasking system.
//! Tasks are non-preemptive and must explicitly yield by returning
//! from their task function.
//!
//! # Usage
//! ```ignore
//! use crate::executor::{spawn_task, step_executor};
//! use crate::async_tasks::counter_task;
//!
//! // Spawn a task
//! spawn_task("counter", counter_task);
//!
//! // Run one step of all ready tasks
//! while step_executor() {}
//! ```

use crate::task::{TaskScheduler, TaskFn};
use spin::Mutex;

pub static EXECUTOR: Mutex<SimpleExecutor> = Mutex::new(SimpleExecutor::new());

pub struct SimpleExecutor {
    scheduler: TaskScheduler,
    steps_completed: u64,
}

impl SimpleExecutor {
    pub const fn new() -> Self {
        Self {
            scheduler: TaskScheduler::new(),
            steps_completed: 0,
        }
    }

    pub fn spawn(&mut self, name: &str, task_fn: TaskFn) -> usize {
        self.scheduler.spawn(task_fn, name)
    }

    pub fn step(&mut self) -> bool {
        match self.scheduler.step() {
            Some(_) => {
                self.steps_completed += 1;
                true
            }
            None => false,
        }
    }

    pub fn run_all(&mut self) {
        while self.step() {}
    }

    pub fn task_count(&self) -> usize {
        self.scheduler.task_count()
    }

    pub fn steps_completed(&self) -> u64 {
        self.steps_completed
    }
}

pub fn spawn_task(name: &str, task_fn: TaskFn) -> Option<usize> {
    let mut guard = EXECUTOR.lock();
    Some(guard.spawn(name, task_fn))
}

pub fn step_executor() -> bool {
    let mut guard = EXECUTOR.lock();
    guard.step()
}

pub fn get_task_count() -> usize {
    let guard = EXECUTOR.lock();
    guard.task_count()
}
