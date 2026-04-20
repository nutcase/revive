use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DebugBreak {
    Breakpoint(u16),
    Step(u16),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DebugTick {
    Ran(u32),
    Paused,
    Break(DebugBreak),
}

#[derive(Clone, Debug)]
pub struct Debugger {
    pub paused: bool,
    pub breakpoints: BTreeSet<u16>,
    pub last_break: Option<DebugBreak>,
    step_pending: bool,
}

impl Default for Debugger {
    fn default() -> Self {
        Self::new()
    }
}

impl Debugger {
    pub fn new() -> Self {
        Self {
            paused: false,
            breakpoints: BTreeSet::new(),
            last_break: None,
            step_pending: false,
        }
    }

    pub fn request_step(&mut self) {
        self.step_pending = true;
        self.paused = false;
    }

    pub fn clear_break(&mut self) {
        self.last_break = None;
    }

    pub fn add_breakpoint(&mut self, pc: u16) {
        self.breakpoints.insert(pc);
    }

    pub fn remove_breakpoint(&mut self, pc: u16) {
        self.breakpoints.remove(&pc);
    }

    pub fn toggle_breakpoint(&mut self, pc: u16) {
        if !self.breakpoints.remove(&pc) {
            self.breakpoints.insert(pc);
        }
    }

    pub fn step_pending(&self) -> bool {
        self.step_pending
    }

    pub(crate) fn clear_step_pending(&mut self) {
        self.step_pending = false;
    }
}
