use alloc::{
    format,
    string::{String, ToString},
};
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
            "vm_help" => Self::vm_help(),
            "vm_demo" => Self::vm_demo(),
            "vm_demo_advanced" => Self::vm_demo_advanced(),
            "vm_run" => Self::vm_run(trimmed),
            "clear" => CommandResult::Output(String::from("\x1b[2J\x1b[H")),
            "echo" => Self::echo(parts),
            "info" => Self::info(),
            "exit" => CommandResult::Exit,
            _ => {
                let mut msg = String::from("Unknown command: ");
                msg.push_str(cmd);
                CommandResult::Error(msg)
            }
        }
    }

    // ── help ──────────────────────────────────────────────────────────────────

    fn help(_args: SplitWhitespace) -> CommandResult {
        let text = "Available commands:\n  \
            help              show this message\n  \
            test              run all tests\n  \
            test_paging       test paging\n  \
            test_process      test process creation\n  \
            test_memory       test memory allocation\n  \
            test_asm          run all ASM tests\n  \
            test_asm_return   test ASM return value\n  \
            test_asm_add      test ASM addition\n  \
            vm_help           show VM language reference\n  \
            vm_demo           show the built-in demo program\n  \
            vm_demo_advanced  show the advanced demo program\n  \
            vm_run <src>      run a VM program (use ; between instructions)\n  \
            echo <text>       echo text\n  \
            info              kernel information\n  \
            clear             clear terminal\n  \
            exit              exit (no-op)";
        CommandResult::Output(String::from(text))
    }

    fn echo(mut args: SplitWhitespace) -> CommandResult {
        let mut out = String::new();
        while let Some(word) = args.next() {
            out.push_str(word);
            out.push(' ');
        }
        CommandResult::Output(out)
    }

    fn info() -> CommandResult {
        CommandResult::Output(String::from(
            "DuxOS Kernel\n  \
             Architecture : x86_64\n  \
             Build        : bare-metal no_std\n  \
             Features     : Terminal, Logs, Editor, Bytecode VM\n  \
             VM memory    : mmap arena (off kernel heap)\n  \
             Type 'help' for commands",
        ))
    }

    // ── VM help ───────────────────────────────────────────────────────────────

    fn vm_help() -> CommandResult {
        let text = "\
VM instruction reference
========================
Stack
  push <i64>      push immediate value
  dup             duplicate top
  swap            swap top two
  drop            discard top

Locals (256 slots, indices 0-255)
  load <slot>     push value of slot N
  store <slot>    pop into slot N

Arithmetic
  add  sub  mul  div  mod  neg

Comparison  (push 1=true, 0=false)
  eq   neq  gt   lt

Control flow
  jmp  <label>    unconditional jump
  jz   <label>    jump if top == 0
  jnz  <label>    jump if top != 0

Labels
  name:           define a jump target

I/O
  print           pop and print as decimal + newline
  halt            stop execution

Example — count 1 to 5, print sum
  push 1 ; store 0       # counter = 1
  push 0 ; store 1       # sum = 0
  loop:
  load 0 ; dup ; print   # print counter (dup keeps it for sum)
  load 1 ; add ; store 1 # sum += counter
  load 0 ; push 1 ; add ; store 0  # counter++
  load 0 ; push 6 ; eq ; jz loop   # loop while counter != 6
  load 1 ; print ; halt  # print sum (15)";
        CommandResult::Output(String::from(text))
    }

    fn vm_demo() -> CommandResult {
        CommandResult::Output(String::from(crate::vm::example_program()))
    }
    fn vm_demo_advanced() -> CommandResult {
        CommandResult::Output(String::from(crate::vm::example_program_advanced()))
    }
    // ── vm_run ────────────────────────────────────────────────────────────────

    fn vm_run(full_input: &str) -> CommandResult {
        let source = match full_input.strip_prefix("vm_run") {
            Some(rest) => rest.trim(),
            None => "",
        };

        if source.is_empty() {
            return CommandResult::Error(String::from(
                "Usage: vm_run <program>  (use ; as line separator)\n\
                 Example: vm_run push 42 ; print ; halt",
            ));
        }

        let normalized = Self::normalize_inline(source);

        match crate::vm::execute_program_in_process(&normalized) {
            Ok(result) => {
                let mut out = String::new();

                // Output from VM print instructions
                let vm_output = result.output_str();
                if vm_output.is_empty() {
                    out.push_str("(no output)\n");
                } else {
                    out.push_str(vm_output);
                    if !vm_output.ends_with('\n') {
                        out.push('\n');
                    }
                }

                // Execution summary
                out.push_str(&format!(
                    "--- steps: {}  halted: {}  stack: {}\n",
                    result.steps,
                    result.halted,
                    Self::fmt_stack(result.stack_slice()),
                ));

                CommandResult::Output(out)
            }
            Err(err) => {
                // VmError no longer carries String — call to_display() to get one.
                CommandResult::Error(err.to_display())
            }
        }
    }

    /// Convert `push 1 ; store 0 ; ...` into one instruction per line.
    fn normalize_inline(source: &str) -> String {
        let mut out = String::new();
        let mut first = true;
        for segment in source.split(';') {
            let line = segment.trim();
            if line.is_empty() {
                continue;
            }
            if !first {
                out.push('\n');
            }
            out.push_str(line);
            first = false;
        }
        if out.is_empty() {
            out.push_str(source);
        }
        out
    }

    /// Format a stack slice as `[a, b, c]` — allocates a String for display.
    fn fmt_stack(values: &[i64]) -> String {
        let mut out = String::from("[");
        for (i, v) in values.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            out.push_str(&v.to_string());
        }
        out.push(']');
        out
    }

    // ── test commands ─────────────────────────────────────────────────────────

    fn test_all() -> CommandResult {
        CommandResult::Output(crate::tests::test_env::test_all())
    }

    fn test_paging() -> CommandResult {
        CommandResult::Output(crate::tests::test_env::test_basic_paging())
    }

    fn test_process() -> CommandResult {
        CommandResult::Output(crate::tests::test_env::test_process_creation())
    }

    fn test_memory() -> CommandResult {
        CommandResult::Output(crate::tests::test_env::test_memory_allocation())
    }

    fn test_asm() -> CommandResult {
        let mut out = String::new();
        out.push_str(&&crate::tests::test_env::test_asm_simple_return());
        out.push_str(&crate::tests::test_env::test_asm_add());
        CommandResult::Output(out)
    }

    fn test_asm_return() -> CommandResult {
        CommandResult::Output(crate::tests::test_env::test_asm_simple_return())
    }

    fn test_asm_add() -> CommandResult {
        CommandResult::Output(crate::tests::test_env::test_asm_add())
    }
}
