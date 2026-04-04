//! # VM Runtime
//!
//! Execution engine for the kernel bytecode VM.
//!
//! ## Memory model
//!
//! `Vm` contains zero heap-allocated fields. All runtime state lives in
//! fixed-size arrays embedded directly in the struct:
//!
//! | Field          | Size    | Purpose                    |
//! |----------------|---------|----------------------------|
//! | `stack`        | 8 KB    | Operand stack (1024 i64)   |
//! | `locals`       | 2 KB    | 256 local variable slots   |
//! | `output`       | 8 KB    | Output byte buffer         |
//!
//! Total struct size: ~18 KB. The struct is placed into a `sys_mmap`'d page
//! by `VmProcess`, keeping it entirely off the kernel heap.
//!
//! `VmResult` also uses fixed-size arrays so no heap allocation happens
//! when returning results to callers.
//!
//! `VmError` still uses `&'static str` for messages — no `String`, no heap.

use super::bytecode::{Instruction, Program};

/// Per-instruction VM tracing (debug builds only — thousands of lines would stress the heap).
macro_rules! vm_trace {
    ($($t:tt)*) => {{
        #[cfg(debug_assertions)]
        crate::log_info!($($t)*);
    }};
}

/// VM lifecycle (start / finish / errors): always goes to the debug pipeline so the Logs app (F2) shows it.
macro_rules! vm_event {
    ($($t:tt)*) => {{
        crate::log_info!($($t)*);
    }};
}

// ── limits ────────────────────────────────────────────────────────────────────

pub const MAX_STEPS: usize = 100_000;
pub const MAX_STACK: usize = 1024;
pub const MAX_OUTPUT_BYTES: usize = 8192;
pub const MAX_LOCALS: usize = 256;

// ── error ─────────────────────────────────────────────────────────────────────

/// A VM error with a static message and an optional numeric detail value.
///
/// No heap allocation — callers that need a full formatted string can
/// call `.to_display()` which builds one on demand using the kernel allocator,
/// but normal error propagation is completely heap-free.
#[derive(Clone, Debug, PartialEq)]
pub enum VmError {
    /// Error produced during source parsing.
    Parse(&'static str),
    /// Error produced during bytecode execution.
    Runtime(&'static str),
    /// Error with a numeric context value (e.g. bad slot index, bad jump target).
    RuntimeN(&'static str, usize),
}

impl VmError {
    pub fn parse(msg: &'static str) -> Self {
        Self::Parse(msg)
    }

    pub fn runtime(msg: &'static str) -> Self {
        Self::Runtime(msg)
    }

    pub fn runtime_n(msg: &'static str, n: usize) -> Self {
        Self::RuntimeN(msg, n)
    }

    /// Build a displayable string (allocates — only call for user-facing output).
    pub fn to_display(&self) -> alloc::string::String {
        use alloc::format;
        match self {
            VmError::Parse(msg) => format!("parse error: {}", msg),
            VmError::Runtime(msg) => format!("runtime error: {}", msg),
            VmError::RuntimeN(msg, n) => format!("runtime error: {} ({})", msg, n),
        }
    }

    /// Return just the message &str without allocating.
    pub fn message(&self) -> &'static str {
        match self {
            VmError::Parse(m) | VmError::Runtime(m) | VmError::RuntimeN(m, _) => m,
        }
    }
}

// ── result ────────────────────────────────────────────────────────────────────

/// The result of a completed VM execution.
///
/// All fields are fixed-size — no heap allocation.
#[derive(Clone, Debug)]
pub struct VmResult {
    pub output: [u8; MAX_OUTPUT_BYTES],
    pub output_len: usize,
    pub halted: bool,
    pub steps: usize,
    pub final_stack: [i64; MAX_STACK],
    pub final_stack_len: usize,
}

impl VmResult {
    ///UYF-8
    pub fn output_str(&self) -> &str {
        core::str::from_utf8(&self.output[..self.output_len]).unwrap_or("<invalid utf8>")
    }

    pub fn stack_slice(&self) -> &[i64] {
        &self.final_stack[..self.final_stack_len]
    }

    fn empty() -> Self {
        Self {
            output: [0u8; MAX_OUTPUT_BYTES],
            output_len: 0,
            halted: false,
            steps: 0,
            final_stack: [0i64; MAX_STACK],
            final_stack_len: 0,
        }
    }
}

pub struct Vm {
    ip: usize,
    stack: [i64; MAX_STACK],
    stack_top: usize,
    locals: [i64; MAX_LOCALS],
    output: [u8; MAX_OUTPUT_BYTES],
    output_len: usize,
    steps: usize,
    halted: bool,
}

use core::mem::MaybeUninit;

impl Vm {
    pub fn new() -> Self {
        let mut vm = MaybeUninit::<Self>::uninit();

        unsafe {
            let ptr = vm.as_mut_ptr();

            core::ptr::write(&mut (*ptr).ip, 0);
            core::ptr::write(&mut (*ptr).stack_top, 0);
            core::ptr::write(&mut (*ptr).output_len, 0);
            core::ptr::write(&mut (*ptr).steps, 0);
            core::ptr::write(&mut (*ptr).halted, false);

            core::ptr::write_bytes(
                &mut (*ptr).stack as *mut _ as *mut u8,
                0,
                core::mem::size_of_val(&(*ptr).stack),
            );
            core::ptr::write_bytes(
                &mut (*ptr).locals as *mut _ as *mut u8,
                0,
                core::mem::size_of_val(&(*ptr).locals),
            );
            core::ptr::write_bytes(
                &mut (*ptr).output as *mut _ as *mut u8,
                0,
                core::mem::size_of_val(&(*ptr).output),
            );

            vm.assume_init()
        }
    }
    pub fn reset(&mut self) {
        self.ip = 0;
        self.stack_top = 0;
        self.output_len = 0;
        self.steps = 0;
        self.halted = false;

        unsafe {
            core::ptr::write_bytes(
                core::ptr::addr_of_mut!(self.locals) as *mut u8,
                0,
                core::mem::size_of_val(&self.locals),
            );

            core::ptr::write_bytes(
                core::ptr::addr_of_mut!(self.stack) as *mut u8,
                0,
                core::mem::size_of_val(&self.stack),
            );
            core::ptr::write_bytes(
                core::ptr::addr_of_mut!(self.output) as *mut u8,
                0,
                core::mem::size_of_val(&self.output),
            );
        }
    }
    pub fn execute(&mut self, program: &Program) -> Result<VmResult, VmError> {
        self.reset();
        vm_event!(
            "=== VM START === {} instructions",
            program.instructions.len()
        );

        if program.instructions.is_empty() {
            return Err(VmError::runtime("program has no instructions"));
        }

        while !self.halted {
            if self.steps >= MAX_STEPS {
                vm_event!("ERROR: Step limit exceeded");
                return Err(VmError::runtime("step limit exceeded"));
            }

            if self.ip >= program.instructions.len() {
                vm_event!("ERROR: IP out of bounds");
                return Err(VmError::runtime("instruction pointer out of bounds"));
            }

            let instr = &program.instructions[self.ip];
            self.steps += 1;

            vm_trace!("[{:04}] IP={:3} | {:?}", self.steps, self.ip, instr);

            match instr {
                Instruction::Push(v) => {
                    vm_trace!("    push {}", v);
                    self.push(*v)?;
                }
                Instruction::Dup => {
                    let v = self.peek()?;
                    vm_trace!("    dup (top={})", v);
                    self.push(v)?;
                }
                Instruction::Swap => {
                    vm_trace!("    swap");
                    if self.stack_top < 2 {
                        return Err(VmError::runtime("swap: need 2 values"));
                    }
                    let a = self.stack_top - 1;
                    let b = self.stack_top - 2;
                    self.stack.swap(a, b);
                }
                Instruction::Drop => {
                    let v = self.pop()?;
                    vm_trace!("    drop {}", v);
                }
                Instruction::Add => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    let res = a.checked_add(b).ok_or(VmError::runtime("add overflow"))?;
                    vm_trace!("    {} + {} = {}", a, b, res);
                    self.push(res)?;
                }
                Instruction::Sub => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    let res = a.checked_sub(b).ok_or(VmError::runtime("sub overflow"))?;
                    vm_trace!("    {} - {} = {}", a, b, res);
                    self.push(res)?;
                }
                Instruction::Mul => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    let res = a.checked_mul(b).ok_or(VmError::runtime("mul overflow"))?;
                    vm_trace!("    {} * {} = {}", a, b, res);
                    self.push(res)?;
                }
                Instruction::Div => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    if b == 0 {
                        return Err(VmError::runtime("division by zero"));
                    }
                    let res = a.checked_div(b).ok_or(VmError::runtime("div overflow"))?;
                    vm_trace!("    {} / {} = {}", a, b, res);
                    self.push(res)?;
                }
                Instruction::Mod => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    if b == 0 {
                        return Err(VmError::runtime("modulo by zero"));
                    }
                    let res = a.checked_rem(b).ok_or(VmError::runtime("mod overflow"))?;
                    vm_trace!("    {} % {} = {}", a, b, res);
                    self.push(res)?;
                }
                Instruction::Neg => {
                    let a = self.pop()?;
                    let res = a.checked_neg().ok_or(VmError::runtime("neg overflow"))?;
                    vm_trace!("    neg {} = {}", a, res);
                    self.push(res)?;
                }
                Instruction::Eq | Instruction::Neq | Instruction::Gt | Instruction::Lt => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    let res = match instr {
                        Instruction::Eq => (a == b) as i64,
                        Instruction::Neq => (a != b) as i64,
                        Instruction::Gt => (a > b) as i64,
                        Instruction::Lt => (a < b) as i64,
                        _ => 0,
                    };
                    vm_trace!("   → cmp {} ? {} → {}", a, b, res);
                    self.push(res)?;
                }
                Instruction::Jmp(label) => {
                    vm_trace!("   → jmp → {}", label);
                    self.jump(label, program)?;
                    continue;
                }
                Instruction::Jz(label) => {
                    let cond = self.pop()?;
                    vm_trace!("   → jz (cond={}) → {}", cond, label);
                    if cond == 0 {
                        self.jump(label, program)?;
                        continue;
                    }
                }
                Instruction::Jnz(label) => {
                    let cond = self.pop()?;
                    vm_trace!("   → jnz (cond={}) → {}", cond, label);
                    if cond != 0 {
                        self.jump(label, program)?;
                        continue;
                    }
                }
                Instruction::Load(slot) => {
                    let v = self.locals[*slot as usize];
                    vm_trace!("   → load slot{} = {}", slot, v);
                    self.push(v)?;
                }
                Instruction::Store(slot) => {
                    let v = self.pop()?;
                    vm_trace!("   → store slot{} ← {}", slot, v);
                    self.locals[*slot as usize] = v;
                }
                Instruction::Print => {
                    let value = self.pop()?;
                    vm_trace!("   → print {}", value);
                    self.write_i64(value)?;
                }
                Instruction::Halt => {
                    vm_trace!("   → HALT - execution finished");
                    self.halted = true;
                }
            }

            self.ip += 1;
        }

        vm_event!("=== VM FINISHED === steps: {}", self.steps);

        let mut result = VmResult::empty();
        result.output[..self.output_len].copy_from_slice(&self.output[..self.output_len]);
        result.output_len = self.output_len;
        result.halted = self.halted;
        result.steps = self.steps;
        result.final_stack[..self.stack_top].copy_from_slice(&self.stack[..self.stack_top]);
        result.final_stack_len = self.stack_top;

        Ok(result)
    }

    fn push(&mut self, value: i64) -> Result<(), VmError> {
        if self.stack_top >= MAX_STACK {
            return Err(VmError::runtime("stack overflow"));
        }
        self.stack[self.stack_top] = value;
        self.stack_top += 1;
        Ok(())
    }

    fn pop(&mut self) -> Result<i64, VmError> {
        if self.stack_top == 0 {
            return Err(VmError::runtime("stack underflow"));
        }
        self.stack_top -= 1;
        Ok(self.stack[self.stack_top])
    }

    fn peek(&self) -> Result<i64, VmError> {
        if self.stack_top == 0 {
            return Err(VmError::runtime("stack underflow"));
        }
        Ok(self.stack[self.stack_top - 1])
    }

    fn jump(&mut self, label: &str, program: &Program) -> Result<(), VmError> {
        if let Ok(target) = label.parse::<usize>() {
            if target >= program.instructions.len() {
                return Err(VmError::runtime("jump target index out of bounds"));
            }
            self.ip = target;
            return Ok(());
        }

        match program.labels.get(label).copied() {
            Some(target) => {
                self.ip = target;
                Ok(())
            }
            None => Err(VmError::runtime("unknown jump label")),
        }
    }

    // CHATGPT
    fn write_i64(&mut self, value: i64) -> Result<(), VmError> {
        let mut buf = [0u8; 22];
        let s = fmt_i64(value, &mut buf);
        self.write_bytes(s)?;
        self.write_bytes(b"\n")
    }

    fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), VmError> {
        if self.output_len + bytes.len() > MAX_OUTPUT_BYTES {
            return Err(VmError::runtime("output buffer full"));
        }
        let end = self.output_len + bytes.len();
        self.output[self.output_len..end].copy_from_slice(bytes);
        self.output_len = end;
        Ok(())
    }
}

impl Default for Vm {
    fn default() -> Self {
        Self::new()
    }
}
// CHATGPT
fn fmt_i64(mut n: i64, buf: &mut [u8]) -> &[u8] {
    if buf.len() < 22 {
        return b"<fmt_buf_too_small>";
    }

    if n == i64::MIN {
        let s = b"-9223372036854775808";
        let len = s.len();
        buf[..len].copy_from_slice(s);
        return &buf[..len];
    }

    let negative = n < 0;
    if negative {
        n = -n;
    }

    let mut pos = buf.len();
    loop {
        pos -= 1;
        buf[pos] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    if negative {
        pos -= 1;
        buf[pos] = b'-';
    }

    &buf[pos..]
}

pub fn compile_and_run(source: &str) -> Result<VmResult, VmError> {
    let program =
        crate::vm::parser::parse_program(source).map_err(|_| VmError::parse("parse failed"))?;
    let mut vm = Vm::new();
    vm.execute(&program)
}
