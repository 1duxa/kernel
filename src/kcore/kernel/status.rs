//! # Kernel Status Tracking
//!
//! Tracks initialization status of kernel components for boot diagnostics.


use alloc::vec::Vec;
use core::fmt;
use spin::Mutex;


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitStatus {
    NotStarted,
    InProgress,
    Completed,
    Failed(&'static str),
}


#[derive(Debug, Clone, Copy)]
pub struct ComponentStatus {
    pub name: &'static str,
    pub status: InitStatus,
}

static INIT_STATUS: Mutex<Vec<ComponentStatus>> = Mutex::new(Vec::new());

impl ComponentStatus {
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            status: InitStatus::NotStarted,
        }
    }

    pub fn set_status(&mut self, status: InitStatus) {
        self.status = status;
    }

    pub fn is_complete(&self) -> bool {
        matches!(self.status, InitStatus::Completed)
    }

    pub fn is_failed(&self) -> bool {
        matches!(self.status, InitStatus::Failed(_))
    }
}


pub fn register_component(name: &'static str) {
    INIT_STATUS.lock().push(ComponentStatus::new(name));
}


pub fn update_component_status(name: &'static str, status: InitStatus) {
    let mut components = INIT_STATUS.lock();
    if let Some(comp) = components.iter_mut().find(|c| c.name == name) {
        comp.status = status;
    }
}


pub fn get_all_statuses() -> Vec<ComponentStatus> {
    INIT_STATUS.lock().iter().copied().collect()
}


pub fn all_components_ready() -> bool {
    let components = INIT_STATUS.lock();
    !components.is_empty() && components.iter().all(|c| c.is_complete())
}

impl fmt::Display for InitStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InitStatus::NotStarted => write!(f, "Not Started"),
            InitStatus::InProgress => write!(f, "In Progress"),
            InitStatus::Completed => write!(f, "Completed"),
            InitStatus::Failed(err) => write!(f, "Failed: {}", err),
        }
    }
}
