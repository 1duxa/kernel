//! # Cooperative Task Executor
//!
//! A simple non-preemptive task scheduler. Tasks must explicitly yield
//! by returning from their task function.
//!
//! ## Usage
//!
//! ```ignore
//! spawn_task("counter", counter_task);
//! while step_executor() {}
//! ```

use crate::task::{TaskScheduler, TaskFn};
use crate::log_info;
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
        let id = self.scheduler.spawn(task_fn, name);
        log_info!("Task spawned: {} (id={})", name, id);
        id
    }

    pub fn step(&mut self) -> bool {
        match self.scheduler.step() {
            Some(_task_id) => {
                self.steps_completed += 1;
                true
            }
            None => false,
        }
    }

    pub fn run_all(&mut self) {
        let start_steps = self.steps_completed;
        while self.step() {}
        let total = self.steps_completed - start_steps;
        if total > 0 {
            log_info!("Executor: ran {} steps", total);
        }
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
