//! Task Scheduling System
//!
//! Provides a simple cooperative task scheduler with round-robin execution.
//!
//! # Task Lifecycle
//! 1. Task is spawned with `scheduler.spawn(fn, name)`
//! 2. Scheduler calls task function each step
//! 3. Task returns `TaskState::Ready` to continue, `Completed` to finish
//! 4. Completed tasks are automatically removed
//!
//! # Task Context
//! Each task has a `TaskContext` with:
//! - Iteration counter
//! - 256-byte data buffer for intermediate results
//! - Current state

use crate::data_structures::vec::{String, Vec};

pub type TaskFn = fn(&mut TaskContext) -> TaskState;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TaskState {
    Ready,
    Running,
    Blocked,
    Completed,
}

pub struct TaskContext {
    pub id: usize,
    pub state: TaskState,
    pub iterations: u64,
    pub data: [u8; 256],
    pub data_len: usize,
}

impl TaskContext {
    pub fn new(id: usize) -> Self {
        Self {
            id,
            state: TaskState::Ready,
            iterations: 0,
            data: [0u8; 256],
            data_len: 0,
        }
    }

    pub fn store_data(&mut self, data: &[u8]) {
        let len = data.len().min(256);
        self.data[..len].copy_from_slice(&data[..len]);
        self.data_len = len;
    }

    pub fn get_data(&self) -> &[u8] {
        &self.data[..self.data_len]
    }
}

pub struct Task {
    pub context: TaskContext,
    pub function: TaskFn,
    pub name: String,
}

impl Task {
    pub fn new(id: usize, function: TaskFn, name: &str) -> Self {
        Self {
            context: TaskContext::new(id),
            function,
            name: String::from(name),
        }
    }

    pub fn execute(&mut self) {
        self.context.state = TaskState::Running;
        self.context.iterations += 1;
        self.context.state = (self.function)(&mut self.context);
    }
}

pub struct TaskScheduler {
    tasks: Vec<Task>,
    current_task: usize,
    next_id: usize,
}

impl TaskScheduler {
    pub const fn new() -> Self {
        Self {
            tasks: Vec::new(),
            current_task: 0,
            next_id: 1,
        }
    }

    pub fn spawn(&mut self, function: TaskFn, name: &str) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        let task = Task::new(id, function, name);
        self.tasks.push(task);
        id
    }

    pub fn step(&mut self) -> Option<usize> {
        if self.tasks.is_empty() {
            return None;
        }

        let mut attempts = 0;
        while attempts < self.tasks.len() {
            let current = self.current_task % self.tasks.len();
            let task = &mut self.tasks[current];

            if task.context.state != TaskState::Completed {
                let task_id = task.context.id;
                task.execute();

                if task.context.state == TaskState::Completed {
                    self.tasks.remove(current);
                } else {
                    self.current_task = (current + 1) % self.tasks.len();
                }
                return Some(task_id);
            }

            self.current_task = (current + 1) % self.tasks.len();
            attempts += 1;
        }

        None
    }

    pub fn run_all(&mut self) {
        while !self.tasks.is_empty() {
            self.step();
        }
    }

    pub fn task_count(&self) -> usize {
        self.tasks.len()
    }

    pub fn get_task(&self, id: usize) -> Option<&Task> {
        self.tasks.iter().find(|t| t.context.id == id)
    }

    pub fn get_task_mut(&mut self, id: usize) -> Option<&mut Task> {
        self.tasks.iter_mut().find(|t| t.context.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn simple_task(ctx: &mut TaskContext) -> TaskState {
        if ctx.iterations >= 3 {
            TaskState::Completed
        } else {
            TaskState::Ready
        }
    }

    #[test]
    fn test_task_creation() {
        let task = Task::new(1, simple_task, "test");
        assert_eq!(task.context.id, 1);
        assert_eq!(task.context.state, TaskState::Ready);
    }

    #[test]
    fn test_scheduler() {
        let mut scheduler = TaskScheduler::new();
        scheduler.spawn(simple_task, "task1");
        scheduler.spawn(simple_task, "task2");

        assert_eq!(scheduler.task_count(), 2);

        scheduler.run_all();
        assert_eq!(scheduler.task_count(), 0);
    }
}
