use crate::data_structures::vec::{String, Vec};
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
            help           - Show this help message\n  \
            test           - Run all tests\n  \
            test_paging    - Test paging functionality\n  \
            test_process   - Test process creation\n  \
            test_memory    - Test memory allocation\n  \
            test_asm       - Run all assembly tests\n  \
            test_asm_return- Test assembly return value\n  \
            test_asm_add   - Test assembly addition\n  \
            echo <text>    - Echo text to terminal\n  \
            info           - Show system information\n  \
            clear          - Clear terminal\n  \
            exit           - Exit (no-op for now)";
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
}
