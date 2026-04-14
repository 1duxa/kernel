//! # VM Parser
//!
//! Text-to-bytecode parser for the kernel VM.

use crate::vm::bytecode::{Instruction, Program};
use alloc::{
    collections::BTreeMap,
    format,
    string::{String, ToString},
    vec::Vec,
};

#[derive(Clone, Debug, PartialEq)]
pub enum ParseError {
    EmptyProgram,
    DuplicateLabel { line: usize, label: String },
    InvalidLabel { line: usize, label: String },
    UnknownInstruction { line: usize, op: String },
    MissingArgument { line: usize, op: String },
    TooManyArguments { line: usize, op: String },
    InvalidInteger { line: usize, value: String },
    InvalidTarget { line: usize, target: String },
    UnknownLabel { line: usize, label: String },
}

impl ParseError {
    pub fn message(&self) -> String {
        match self {
            ParseError::EmptyProgram => String::from("program is empty"),
            ParseError::DuplicateLabel { line, label } => {
                format!("line {line}: duplicate label `{label}`")
            }
            ParseError::InvalidLabel { line, label } => {
                format!("line {line}: invalid label `{label}`")
            }
            ParseError::UnknownInstruction { line, op } => {
                format!("line {line}: unknown instruction `{op}`")
            }
            ParseError::MissingArgument { line, op } => {
                format!("line {line}: `{op}` needs an argument")
            }
            ParseError::TooManyArguments { line, op } => {
                format!("line {line}: `{op}` received too many arguments")
            }
            ParseError::InvalidInteger { line, value } => {
                format!("line {line}: invalid integer `{value}`")
            }
            ParseError::InvalidTarget { line, target } => {
                format!("line {line}: invalid jump target `{target}`")
            }
            ParseError::UnknownLabel { line, label } => {
                format!("line {line}: unknown label `{label}`")
            }
        }
    }
}

#[derive(Clone, Debug)]
enum ParsedTarget {
    Index(usize),
    Label(String),
}

#[derive(Clone, Debug)]
enum ParsedInstruction {
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
    Jmp(ParsedTarget),
    Jz(ParsedTarget),
    Jnz(ParsedTarget),
    Load(u8),
    Store(u8),
    Print,
    Halt,
}

pub fn parse_program(source: &str) -> Result<Program, ParseError> {
    let mut labels: BTreeMap<String, usize> = BTreeMap::new();
    let mut parsed: Vec<(usize, ParsedInstruction)> = Vec::new();

    for (line_no0, raw_line) in source.lines().enumerate() {
        let line_no = line_no0 + 1;
        let line = strip_comment(raw_line).trim();

        if line.is_empty() {
            continue;
        }

        if line.ends_with(':') {
            let label = &line[..line.len().saturating_sub(1)];
            if !is_valid_label(label) {
                return Err(ParseError::InvalidLabel {
                    line: line_no,
                    label: label.to_string(),
                });
            }
            if labels.contains_key(label) {
                return Err(ParseError::DuplicateLabel {
                    line: line_no,
                    label: label.to_string(),
                });
            }
            labels.insert(label.to_string(), parsed.len());
            continue;
        }

        parsed.push((line_no, parse_instruction(line, line_no)?));
    }

    if parsed.is_empty() {
        return Err(ParseError::EmptyProgram);
    }

    let mut program = Program::new();
    program.labels = labels;
    for (line_no, instr) in parsed {
        program
            .instructions
            .push(resolve_instruction(instr, line_no, &program.labels)?);
    }

    Ok(program)
}

fn resolve_instruction(
    instr: ParsedInstruction,
    _line_no: usize,
    _labels: &BTreeMap<String, usize>,
) -> Result<Instruction, ParseError> {
    // TODO: BRIED
    Ok(match instr {
        ParsedInstruction::Push(v) => Instruction::Push(v),
        ParsedInstruction::Dup => Instruction::Dup,
        ParsedInstruction::Swap => Instruction::Swap,
        ParsedInstruction::Drop => Instruction::Drop,
        ParsedInstruction::Add => Instruction::Add,
        ParsedInstruction::Sub => Instruction::Sub,
        ParsedInstruction::Mul => Instruction::Mul,
        ParsedInstruction::Div => Instruction::Div,
        ParsedInstruction::Mod => Instruction::Mod,
        ParsedInstruction::Neg => Instruction::Neg,
        ParsedInstruction::Eq => Instruction::Eq,
        ParsedInstruction::Neq => Instruction::Neq,
        ParsedInstruction::Gt => Instruction::Gt,
        ParsedInstruction::Lt => Instruction::Lt,
        ParsedInstruction::Jmp(t) => Instruction::Jmp(target_to_string(t)),
        ParsedInstruction::Jz(t) => Instruction::Jz(target_to_string(t)),
        ParsedInstruction::Jnz(t) => Instruction::Jnz(target_to_string(t)),
        ParsedInstruction::Load(slot) => Instruction::Load(slot),
        ParsedInstruction::Store(slot) => Instruction::Store(slot),
        ParsedInstruction::Print => Instruction::Print,
        ParsedInstruction::Halt => Instruction::Halt,
    })
}

fn target_to_string(target: ParsedTarget) -> String {
    match target {
        ParsedTarget::Index(i) => i.to_string(),
        ParsedTarget::Label(label) => label,
    }
}

fn parse_instruction(line: &str, line_no: usize) -> Result<ParsedInstruction, ParseError> {
    let mut parts = line.split_whitespace();
    let op = parts.next().unwrap_or_default();
    let op_lower = ascii_lower(op);

    match op_lower.as_str() {
        "push" => {
            let value = parts.next().ok_or(ParseError::MissingArgument {
                line: line_no,
                op: String::from("push"),
            })?;
            if parts.next().is_some() {
                return Err(ParseError::TooManyArguments {
                    line: line_no,
                    op: String::from("push"),
                });
            }
            let parsed = value
                .parse::<i64>()
                .map_err(|_| ParseError::InvalidInteger {
                    line: line_no,
                    value: value.to_string(),
                })?;
            Ok(ParsedInstruction::Push(parsed))
        }
        "dup" => no_args(parts, line_no, "dup", ParsedInstruction::Dup),
        "swap" => no_args(parts, line_no, "swap", ParsedInstruction::Swap),
        "drop" => no_args(parts, line_no, "drop", ParsedInstruction::Drop),
        "add" => no_args(parts, line_no, "add", ParsedInstruction::Add),
        "sub" => no_args(parts, line_no, "sub", ParsedInstruction::Sub),
        "mul" => no_args(parts, line_no, "mul", ParsedInstruction::Mul),
        "div" => no_args(parts, line_no, "div", ParsedInstruction::Div),
        "mod" => no_args(parts, line_no, "mod", ParsedInstruction::Mod),
        "neg" => no_args(parts, line_no, "neg", ParsedInstruction::Neg),
        "eq" => no_args(parts, line_no, "eq", ParsedInstruction::Eq),
        "neq" => no_args(parts, line_no, "neq", ParsedInstruction::Neq),
        "gt" => no_args(parts, line_no, "gt", ParsedInstruction::Gt),
        "lt" => no_args(parts, line_no, "lt", ParsedInstruction::Lt),
        "print" => no_args(parts, line_no, "print", ParsedInstruction::Print),
        "halt" => no_args(parts, line_no, "halt", ParsedInstruction::Halt),
        "jmp" => Ok(ParsedInstruction::Jmp(parse_target(parts, line_no, "jmp")?)),
        "jz" => Ok(ParsedInstruction::Jz(parse_target(parts, line_no, "jz")?)),
        "jnz" => Ok(ParsedInstruction::Jnz(parse_target(parts, line_no, "jnz")?)),
        "load" => {
            let slot = parts.next().ok_or(ParseError::MissingArgument {
                line: line_no,
                op: String::from("load"),
            })?;
            if parts.next().is_some() {
                return Err(ParseError::TooManyArguments {
                    line: line_no,
                    op: String::from("load"),
                });
            }
            let slot_num = slot.parse::<u8>().map_err(|_| ParseError::InvalidInteger {
                line: line_no,
                value: slot.to_string(),
            })?;
            Ok(ParsedInstruction::Load(slot_num))
        }
        "store" => {
            let slot = parts.next().ok_or(ParseError::MissingArgument {
                line: line_no,
                op: String::from("store"),
            })?;
            if parts.next().is_some() {
                return Err(ParseError::TooManyArguments {
                    line: line_no,
                    op: String::from("store"),
                });
            }
            let slot_num = slot.parse::<u8>().map_err(|_| ParseError::InvalidInteger {
                line: line_no,
                value: slot.to_string(),
            })?;
            Ok(ParsedInstruction::Store(slot_num))
        }
        _ => Err(ParseError::UnknownInstruction {
            line: line_no,
            op: op.to_string(),
        }),
    }
}

fn parse_target(
    mut parts: core::str::SplitWhitespace<'_>,
    line_no: usize,
    op: &str,
) -> Result<ParsedTarget, ParseError> {
    let token = parts.next().ok_or(ParseError::MissingArgument {
        line: line_no,
        op: op.to_string(),
    })?;

    if parts.next().is_some() {
        return Err(ParseError::TooManyArguments {
            line: line_no,
            op: op.to_string(),
        });
    }

    if let Ok(index) = token.parse::<usize>() {
        return Ok(ParsedTarget::Index(index));
    }

    if !is_valid_label(token) {
        return Err(ParseError::InvalidTarget {
            line: line_no,
            target: token.to_string(),
        });
    }

    Ok(ParsedTarget::Label(token.to_string()))
}

fn no_args(
    mut parts: core::str::SplitWhitespace<'_>,
    line_no: usize,
    op: &str,
    instr: ParsedInstruction,
) -> Result<ParsedInstruction, ParseError> {
    if parts.next().is_some() {
        return Err(ParseError::TooManyArguments {
            line: line_no,
            op: op.to_string(),
        });
    }
    Ok(instr)
}

fn strip_comment(line: &str) -> &str {
    match line.find('#') {
        Some(idx) => &line[..idx],
        None => line,
    }
}

fn ascii_lower(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        out.push(ch.to_ascii_lowercase());
    }
    out
}

fn is_valid_label(label: &str) -> bool {
    let mut chars = label.chars();
    match chars.next() {
        Some(c) if c == '_' || c.is_ascii_alphabetic() => {}
        _ => return false,
    }

    chars.all(|c| c == '_' || c.is_ascii_alphanumeric())
}
