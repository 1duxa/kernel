//! # Command Executor
//!
//! Provides command parsing and execution for the terminal application.
//!
//! ## Available Commands
//!
//! ### General
//! - `help`: Display available commands
//! - `echo <text>`: Echo text back
//! - `info`: Show system information
//! - `clear`: Clear terminal screen
//! - `exit`: Exit command (placeholder)
//!
//! ### Testing
//! - `test`: Run all tests
//! - `test_paging`: Test virtual memory mapping
//! - `test_process`: Test process creation
//! - `test_memory`: Test heap allocation
//! - `test_asm`: Run all assembly execution tests
//! - `test_asm_return`: Test assembly return value
//! - `test_asm_add`: Test assembly addition
//!
//! ### Task Management
//! - `spawn <task>`: Spawn async task (counter, fibonacci, cpu, data)
//! - `tasks`: List active tasks
//! - `step_tasks`: Execute one step of all tasks
//! - `run_all_tasks`: Run all tasks to completion
//!
//! ## Architecture
//!
//! Commands return `CommandResult`:
//! - `Output(String)`: Successful output to display
//! - `Error(String)`: Error message to display
//! - `Exit`: Request to exit (handled by caller)

use crate::data_structures::vec::{String, number_to_string};
use core::str::SplitWhitespace;

pub enum CommandResult {
    Output(String),
    Error(String),
    Exit,
}

pub struct CommandExecutor;

impl CommandExecutor {
    pub fn execute(input: &str) -> CommandResult {
        let trimmed = input.trim();
        
        if trimmed.is_empty() {
            return CommandResult::Output(String::new());
        }
        
        let mut parts = trimmed.split_whitespace();
        let cmd = match parts.next() {
            Some(c) => c,
            None => return CommandResult::Error(String::from("Empty command")),
        };
        
        match cmd {
            "help" => Self::help(parts),
            "test" => Self::test_all(),
            "test_paging" => Self::test_paging(),
            "test_process" => Self::test_process(),
            "test_memory" => Self::test_memory(),
            "test_asm" => Self::test_asm(),
            "test_asm_return" => Self::test_asm_return(),
            "test_asm_add" => Self::test_asm_add(),
            "spawn" => Self::spawn_task(parts),
            "tasks" => Self::list_tasks(),
            "step_tasks" => Self::step_tasks(),
            "run_all_tasks" => Self::run_all_tasks(),
            "clear" => CommandResult::Output(String::from("\x1b[2J\x1b[H")),
            "echo" => Self::echo(parts),
            "info" => Self::info(),
            "exit" => CommandResult::Exit,
            _ => {
                let mut err_msg = String::from("Unknown command: ");
                err_msg.push_str(cmd);
                CommandResult::Error(err_msg)
            }
        }
    }
    
    fn help(_args: SplitWhitespace) -> CommandResult {
        let help_text = "Available Commands:\n  \
            help             - Show this help message\n  \
            test             - Run all tests\n  \
            test_paging      - Test paging functionality\n  \
            test_process     - Test process creation\n  \
            test_memory      - Test memory allocation\n  \
            test_asm         - Run all assembly tests\n  \
            test_asm_return  - Test assembly return value\n  \
            test_asm_add     - Test assembly addition\n  \
            spawn <task>     - Spawn async task (counter, fibonacci, cpu, data)\n  \
            tasks            - List active tasks\n  \
            step_tasks       - Execute one step of tasks\n  \
            run_all_tasks    - Run all tasks to completion\n  \
            echo <text>      - Echo text to terminal\n  \
            info             - Show system information\n  \
            clear            - Clear terminal\n  \
            exit             - Exit (no-op for now)";
        CommandResult::Output(String::from(help_text))
    }
    
    fn echo(mut args: SplitWhitespace) -> CommandResult {
        let mut output = String::new();
        while let Some(word) = args.next() {
            output.push_str(word);
            output.push(' ');
        }
        CommandResult::Output(output)
    }
    
    fn info() -> CommandResult {
        let info = "RustOS Kernel Information:\n  \
            Architecture: x86_64\n  \
            Build: Bare-metal\n  \
            Type 'help' for available commands";
        CommandResult::Output(String::from(info))
    }
    
    fn test_all() -> CommandResult {
        CommandResult::Output(crate::test_env::test_all())
    }
    
    fn test_paging() -> CommandResult {
        CommandResult::Output(crate::test_env::test_basic_paging())
    }
    
    fn test_process() -> CommandResult {
        CommandResult::Output(crate::test_env::test_process_creation())
    }
    
    fn test_memory() -> CommandResult {
        CommandResult::Output(crate::test_env::test_memory_allocation())
    }
    
    fn test_asm() -> CommandResult {
        let mut output = String::new();
        output.push_str(&crate::test_env::test_asm_simple_return());
        output.push_str(&crate::test_env::test_asm_add());
        CommandResult::Output(output)
    }
    
    fn test_asm_return() -> CommandResult {
        CommandResult::Output(crate::test_env::test_asm_simple_return())
    }
    
    fn test_asm_add() -> CommandResult {
        CommandResult::Output(crate::test_env::test_asm_add())
    }
    
    fn spawn_task(mut parts: SplitWhitespace) -> CommandResult {
        let task_type = match parts.next() {
            Some(t) => t,
            None => return CommandResult::Error(String::from("Usage: spawn <task_type>\nAvailable: counter, fibonacci, cpu, data, work")),
        };
        
        match task_type {
            "counter" => {
                crate::executor::spawn_task("counter", crate::async_tasks::counter_task);
                CommandResult::Output(String::from("Spawned counter task"))
            }
            "fibonacci" => {
                crate::executor::spawn_task("fibonacci", crate::async_tasks::fibonacci_task);
                CommandResult::Output(String::from("Spawned fibonacci task"))
            }
            "cpu" => {
                crate::executor::spawn_task("cpu_intensive", crate::async_tasks::cpu_intensive_task);
                CommandResult::Output(String::from("Spawned CPU intensive task"))
            }
            "data" => {
                crate::executor::spawn_task("data_transform", crate::async_tasks::data_transform_task);
                CommandResult::Output(String::from("Spawned data transform task"))
            }
            "work" => {
                crate::executor::spawn_task("work_sim", crate::async_tasks::work_simulation_task);
                CommandResult::Output(String::from("Spawned work simulation task"))
            }
            _ => {
                let mut msg = String::from("Unknown task type: ");
                msg.push_str(task_type);
                CommandResult::Error(msg)
            }
        }
    }
    
    fn list_tasks() -> CommandResult {
        let count = crate::executor::get_task_count();
        let mut msg = String::from("Active tasks: ");
        msg.push_str(&number_to_string(count as u64));
        CommandResult::Output(msg)
    }
    
    fn step_tasks() -> CommandResult {
        let has_tasks = crate::executor::step_executor();
        let result = if has_tasks {
            "Executed one task step"
        } else {
            "No tasks to execute"
        };
        CommandResult::Output(String::from(result))
    }
    
    fn run_all_tasks() -> CommandResult {
        let mut count = 0;
        while crate::executor::step_executor() {
            count += 1;
        }
        let mut msg = String::from("Executed ");
        msg.push_str(&number_to_string(count));
        msg.push_str(" task steps");
        CommandResult::Output(msg)
    }
}
