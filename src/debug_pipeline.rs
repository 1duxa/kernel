use crate::apps::logs_app::LogLevel;
use alloc::{collections::VecDeque, format, string::String, vec::Vec};
use core::fmt;
use spin::Mutex;

const DEFAULT_CAPACITY: usize = 512;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DebugCategory {
    General,
    Kernel,
    Memory,
    Interrupts,
    Input,
    App,
    Render,
    Vm,
    Syscall,
}

impl DebugCategory {
    pub const fn as_str(self) -> &'static str {
        match self {
            DebugCategory::General => "general",
            DebugCategory::Kernel => "kernel",
            DebugCategory::Memory => "memory",
            DebugCategory::Interrupts => "interrupts",
            DebugCategory::Input => "input",
            DebugCategory::App => "app",
            DebugCategory::Render => "render",
            DebugCategory::Vm => "vm",
            DebugCategory::Syscall => "syscall",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DebugEvent {
    pub level: LogLevel,
    pub category: DebugCategory,
    pub source: &'static str,
    pub message: String,
    pub sequence: u64,
}

impl DebugEvent {
    pub fn new(
        level: LogLevel,
        category: DebugCategory,
        source: &'static str,
        message: String,
        sequence: u64,
    ) -> Self {
        Self {
            level,
            category,
            source,
            message,
            sequence,
        }
    }

    pub fn format_line(&self) -> String {
        format!(
            "#{} [{}] [{}] {}: {}",
            self.sequence,
            self.level_tag(),
            self.category.as_str(),
            self.source,
            self.message
        )
    }

    fn level_tag(&self) -> &'static str {
        match self.level {
            LogLevel::Debug => "DBG",
            LogLevel::Info => "INF",
            LogLevel::Warn => "WRN",
            LogLevel::Error => "ERR",
        }
    }
}

pub struct DebugPipeline {
    entries: VecDeque<DebugEvent>,
    capacity: usize,
    next_sequence: u64,
    dirty: bool,
}

impl DebugPipeline {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: VecDeque::with_capacity(capacity.max(1)),
            capacity: capacity.max(1),
            next_sequence: 1,
            dirty: true,
        }
    }

    pub fn push(
        &mut self,
        level: LogLevel,
        category: DebugCategory,
        source: &'static str,
        message: String,
    ) -> u64 {
        let sequence = self.next_sequence;
        self.next_sequence = self.next_sequence.wrapping_add(1);

        if self.entries.len() >= self.capacity {
            self.entries.pop_front();
        }

        self.entries
            .push_back(DebugEvent::new(level, category, source, message, sequence));
        self.dirty = true;
        sequence
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.dirty = true;
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn mark_clean(&mut self) {
        self.dirty = false;
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn entries(&self) -> &VecDeque<DebugEvent> {
        &self.entries
    }

    pub fn snapshot(&self) -> Vec<DebugEvent> {
        self.entries.iter().cloned().collect()
    }

    pub fn snapshot_tail(&self, max_events: usize) -> Vec<DebugEvent> {
        let take = max_events.min(self.entries.len());
        let start = self.entries.len().saturating_sub(take);
        self.entries.iter().skip(start).cloned().collect()
    }

    pub fn recent_lines(&self, max_lines: usize) -> Vec<String> {
        let take = max_lines.min(self.entries.len());
        self.entries
            .iter()
            .skip(self.entries.len().saturating_sub(take))
            .map(DebugEvent::format_line)
            .collect()
    }
}

static DEBUG_PIPELINE: Mutex<Option<DebugPipeline>> = Mutex::new(None);

pub fn init() {
    init_with_capacity(DEFAULT_CAPACITY);
}

pub fn init_with_capacity(capacity: usize) {
    *DEBUG_PIPELINE.lock() = Some(DebugPipeline::new(capacity));
}

pub fn is_initialized() -> bool {
    DEBUG_PIPELINE.lock().is_some()
}

pub fn clear() {
    if let Some(pipeline) = DEBUG_PIPELINE.lock().as_mut() {
        pipeline.clear();
    }
}

pub fn push(
    level: LogLevel,
    category: DebugCategory,
    source: &'static str,
    message: String,
) -> u64 {
    let mut guard = DEBUG_PIPELINE.lock();
    let pipeline = guard.get_or_insert_with(|| DebugPipeline::new(DEFAULT_CAPACITY));
    pipeline.push(level, category, source, message)
}

pub fn log(
    level: LogLevel,
    category: DebugCategory,
    source: &'static str,
    args: fmt::Arguments,
) -> u64 {
    push(level, category, source, format!("{args}"))
}

pub fn snapshot() -> Vec<DebugEvent> {
    DEBUG_PIPELINE
        .lock()
        .as_ref()
        .map(DebugPipeline::snapshot)
        .unwrap_or_default()
}

pub fn snapshot_tail(max_events: usize) -> Vec<DebugEvent> {
    DEBUG_PIPELINE
        .lock()
        .as_ref()
        .map(|p| p.snapshot_tail(max_events))
        .unwrap_or_default()
}

pub fn len() -> usize {
    DEBUG_PIPELINE
        .lock()
        .as_ref()
        .map(DebugPipeline::len)
        .unwrap_or(0)
}

pub fn recent_lines(max_lines: usize) -> Vec<String> {
    DEBUG_PIPELINE
        .lock()
        .as_ref()
        .map(|p| p.recent_lines(max_lines))
        .unwrap_or_default()
}

pub fn is_dirty() -> bool {
    DEBUG_PIPELINE
        .lock()
        .as_ref()
        .map(DebugPipeline::is_dirty)
        .unwrap_or(false)
}

pub fn mark_clean() {
    if let Some(pipeline) = DEBUG_PIPELINE.lock().as_mut() {
        pipeline.mark_clean();
    }
}

#[macro_export]
macro_rules! debug_event {
    ($level:expr, $category:expr, $source:expr, $($arg:tt)*) => {{
        $crate::debug_pipeline::log($level, $category, $source, format_args!($($arg)*))
    }};
}

#[macro_export]
macro_rules! debug_debug {
    ($category:expr, $source:expr, $($arg:tt)*) => {{
        $crate::debug_event!(
            $crate::apps::logs_app::LogLevel::Debug,
            $category,
            $source,
            $($arg)*
        )
    }};
}

#[macro_export]
macro_rules! debug_info {
    ($category:expr, $source:expr, $($arg:tt)*) => {{
        $crate::debug_event!(
            $crate::apps::logs_app::LogLevel::Info,
            $category,
            $source,
            $($arg)*
        )
    }};
}

#[macro_export]
macro_rules! debug_warn {
    ($category:expr, $source:expr, $($arg:tt)*) => {{
        $crate::debug_event!(
            $crate::apps::logs_app::LogLevel::Warn,
            $category,
            $source,
            $($arg)*
        )
    }};
}

#[macro_export]
macro_rules! debug_error {
    ($category:expr, $source:expr, $($arg:tt)*) => {{
        $crate::debug_event!(
            $crate::apps::logs_app::LogLevel::Error,
            $category,
            $source,
            $($arg)*
        )
    }};
}
