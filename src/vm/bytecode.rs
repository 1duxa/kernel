#![allow(dead_code)]
use alloc::{string::String, vec::Vec};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Instruction {
    Push(i64),
    Dup,
    Swap,
    Drop,
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Neg,
    Eq,
    Neq,
    Gt,
    Lt,
    Jmp(String),
    Jz(String),
    Jnz(String),
    Load(u8),
    Store(u8),
    Print,
    Halt,
    // Drawing — args on stack, colors packed as 0xRRGGBB
    SetPixel, // pop x, y, color → put_pixel
    FillRect, // pop x, y, w, h, color → fill_rect
    ClearScr, // pop color → clear
    Present,  // → render_frame
}

#[derive(Clone, Debug, Default)]
pub struct Program {
    pub instructions: Vec<Instruction>,
    pub labels: alloc::collections::BTreeMap<String, usize>,
}

impl Program {
    pub fn new() -> Self {
        Self {
            instructions: Vec::new(),
            labels: alloc::collections::BTreeMap::new(),
        }
    }
}

/// `1`
/// `2`
/// `3`
/// `4`
/// `5`
/// `15` (sum of 1+2+3+4+5)
pub fn example_program() -> &'static str {
    "push 1
store 0
push 0
store 1
loop:
load 0
dup
print
load 1
add
store 1
load 0
push 1
add
store 0
load 0
push 6
eq
jz loop
load 1
print
halt
"
}
/// Bouncing red rectangle — loops forever.
/// local 0 = x,  local 1 = dx (velocity)
pub fn example_draw_program() -> &'static str {
    "push 0
store 0
push 3
store 1
loop:
push 0
clearscr
load 0
push 200
push 80
push 60
push 16711680
fillrect
present
load 0
load 1
add
store 0
load 0
push 700
gt
jz check_left
push -3
store 1
jmp loop
check_left:
load 0
push 0
lt
jz loop
push 3
store 1
jmp loop
"
}

///factorial of 6 (720) using a loop + locals
pub fn example_program_advanced() -> &'static str {
    "push 6          # n = 6
store 0
push 1          # result = 1
store 1
loop:
load 0
push 1
gt              # while n > 1
jz done
load 1
load 0
mul             # result *= n
store 1
load 0
push 1
sub             # n -= 1
store 0
jmp loop
done:
load 1
print           # print factorial
push 720
eq
jz wrong
print           # print \"720\" if correct
halt
wrong:
push 999
print           # error marker
halt
"
}

