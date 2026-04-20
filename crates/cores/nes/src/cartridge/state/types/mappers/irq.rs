use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mapper40State {
    pub irq_counter: u16,
    pub irq_enabled: bool,
    pub irq_pending: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mapper42State {
    pub irq_counter: u16,
    pub irq_enabled: bool,
    pub irq_pending: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mapper43State {
    pub irq_counter: u16,
    pub irq_enabled: bool,
    pub irq_pending: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mapper50State {
    pub irq_counter: u16,
    pub irq_enabled: bool,
    pub irq_pending: bool,
}
