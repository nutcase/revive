#![cfg_attr(not(feature = "dev"), allow(dead_code))]
use std::collections::{HashMap, HashSet};
use std::fmt;

use crate::bus::Bus;
use crate::cpu::Cpu;

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Breakpoint {
    pub address: u32,
    pub enabled: bool,
    pub hit_count: u32,
    pub condition: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct WatchPoint {
    pub address: u32,
    pub size: u8,
    pub watch_type: WatchType,
    pub enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WatchType {
    Read,
    Write,
    ReadWrite,
}

#[derive(Debug, Clone)]
pub struct DebugState {
    pub paused: bool,
    pub step_mode: StepMode,
    pub breakpoints: HashMap<u32, Breakpoint>,
    pub watchpoints: Vec<WatchPoint>,
    pub call_stack: Vec<u32>,
    pub trace_buffer: Vec<TraceEntry>,
    pub memory_dumps: HashMap<String, Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StepMode {
    Continue,
    StepInstruction,
    StepOver,
    StepOut,
}

#[derive(Debug, Clone)]
pub struct TraceEntry {
    pub pc: u32,
    pub opcode: u8,
    pub operands: Vec<u8>,
    pub mnemonic: String,
    pub registers: RegisterSnapshot,
    pub cycle_count: u64,
}

#[derive(Debug, Clone)]
pub struct RegisterSnapshot {
    pub a: u16,
    pub x: u16,
    pub y: u16,
    pub sp: u16,
    pub dp: u16,
    pub db: u8,
    pub pb: u8,
    pub p: u8,
}

pub struct Debugger {
    state: DebugState,
    history_size: usize,
    instruction_count: u64,
    // Memory access tracking
    memory_reads: HashSet<u32>,
    memory_writes: HashSet<u32>,
}

impl Debugger {
    pub fn new() -> Self {
        Self {
            state: DebugState {
                paused: false,
                step_mode: StepMode::Continue,
                breakpoints: HashMap::new(),
                watchpoints: Vec::new(),
                call_stack: Vec::new(),
                trace_buffer: Vec::new(),
                memory_dumps: HashMap::new(),
            },
            history_size: 1000,
            instruction_count: 0,
            memory_reads: HashSet::new(),
            memory_writes: HashSet::new(),
        }
    }

    // ブレークポイント管理
    pub fn add_breakpoint(&mut self, address: u32) {
        self.state.breakpoints.insert(
            address,
            Breakpoint {
                address,
                enabled: true,
                hit_count: 0,
                condition: None,
            },
        );
        println!("Breakpoint added at 0x{:06X}", address);
    }

    pub fn remove_breakpoint(&mut self, address: u32) {
        if self.state.breakpoints.remove(&address).is_some() {
            println!("Breakpoint removed from 0x{:06X}", address);
        }
    }

    pub fn toggle_breakpoint(&mut self, address: u32) {
        if let Some(bp) = self.state.breakpoints.get_mut(&address) {
            bp.enabled = !bp.enabled;
            println!(
                "Breakpoint at 0x{:06X} {}",
                address,
                if bp.enabled { "enabled" } else { "disabled" }
            );
        }
    }

    pub fn list_breakpoints(&self) {
        if self.state.breakpoints.is_empty() {
            println!("No breakpoints set");
            return;
        }

        println!("Breakpoints:");
        for (addr, bp) in &self.state.breakpoints {
            println!(
                "  0x{:06X}: {} (hits: {})",
                addr,
                if bp.enabled { "enabled" } else { "disabled" },
                bp.hit_count
            );
        }
    }

    // ウォッチポイント管理
    pub fn add_watchpoint(&mut self, address: u32, size: u8, watch_type: WatchType) {
        self.state.watchpoints.push(WatchPoint {
            address,
            size,
            watch_type,
            enabled: true,
        });
        println!(
            "Watchpoint added at 0x{:06X} (size: {}, type: {:?})",
            address, size, watch_type
        );
    }

    pub fn remove_watchpoint(&mut self, index: usize) {
        if index < self.state.watchpoints.len() {
            let wp = self.state.watchpoints.remove(index);
            println!("Watchpoint removed from 0x{:06X}", wp.address);
        }
    }

    // ステップ実行制御
    pub fn pause(&mut self) {
        self.state.paused = true;
        println!("Emulator paused");
    }

    pub fn resume(&mut self) {
        self.state.paused = false;
        self.state.step_mode = StepMode::Continue;
        println!("Emulator resumed");
    }

    pub fn step_instruction(&mut self) {
        self.state.paused = true;
        self.state.step_mode = StepMode::StepInstruction;
    }

    pub fn step_over(&mut self) {
        self.state.paused = true;
        self.state.step_mode = StepMode::StepOver;
    }

    pub fn step_out(&mut self) {
        self.state.paused = true;
        self.state.step_mode = StepMode::StepOut;
    }

    // CPU実行前のチェック
    pub fn check_breakpoint(&mut self, pc: u32) -> bool {
        if let Some(bp) = self.state.breakpoints.get_mut(&pc) {
            if bp.enabled {
                bp.hit_count += 1;
                println!(
                    "Breakpoint hit at 0x{:06X} (hit count: {})",
                    pc, bp.hit_count
                );
                self.pause();
                return true;
            }
        }
        false
    }

    pub fn check_watchpoint(&mut self, address: u32, is_write: bool) -> bool {
        for wp in &self.state.watchpoints {
            if !wp.enabled {
                continue;
            }

            if address >= wp.address && address < wp.address + wp.size as u32 {
                let should_break = match wp.watch_type {
                    WatchType::Read => !is_write,
                    WatchType::Write => is_write,
                    WatchType::ReadWrite => true,
                };

                if should_break {
                    println!(
                        "Watchpoint triggered at 0x{:06X} ({})",
                        address,
                        if is_write { "write" } else { "read" }
                    );
                    self.pause();
                    return true;
                }
            }
        }
        false
    }

    // トレース記録
    pub fn record_trace(&mut self, cpu: &Cpu, _bus: &Bus, opcode: u8, operands: &[u8]) {
        let entry = TraceEntry {
            pc: cpu.get_pc(),
            opcode,
            operands: operands.to_vec(),
            mnemonic: self.disassemble_instruction(opcode, operands),
            registers: RegisterSnapshot {
                a: cpu.a(),
                x: cpu.x(),
                y: cpu.y(),
                sp: cpu.sp(),
                dp: cpu.dp(),
                db: cpu.db(),
                pb: cpu.pb(),
                p: cpu.p().bits(),
            },
            cycle_count: cpu.get_cycles(),
        };

        self.state.trace_buffer.push(entry);

        // バッファサイズ制限
        if self.state.trace_buffer.len() > self.history_size {
            self.state.trace_buffer.remove(0);
        }

        self.instruction_count += 1;
    }

    // 逆アセンブル
    pub fn disassemble_instruction(&self, opcode: u8, operands: &[u8]) -> String {
        // 簡易的な逆アセンブル（完全実装は別途必要）
        let mnemonic = match opcode {
            0x00 => "BRK",
            0x01 => "ORA (dp,X)",
            0x02 => "COP",
            0x03 => "ORA sr,S",
            0x04 => "TSB dp",
            0x05 => "ORA dp",
            0x06 => "ASL dp",
            0x07 => "ORA [dp]",
            0x08 => "PHP",
            0x09 => "ORA #const",
            0x0A => "ASL A",
            0x0B => "PHD",
            0x0C => "TSB addr",
            0x0D => "ORA addr",
            0x0E => "ASL addr",
            0x0F => "ORA long",

            0x10 => "BPL rel",
            0x18 => "CLC",
            0x1A => "INC A",
            0x1B => "TCS",

            0x20 => "JSR addr",
            0x28 => "PLP",
            0x2A => "ROL A",
            0x2B => "PLD",

            0x30 => "BMI rel",
            0x38 => "SEC",
            0x3A => "DEC A",
            0x3B => "TSC",

            0x40 => "RTI",
            0x48 => "PHA",
            0x4A => "LSR A",
            0x4B => "PHK",
            0x4C => "JMP addr",

            0x58 => "CLI",
            0x5A => "PHY",
            0x5B => "TCD",

            0x60 => "RTS",
            0x68 => "PLA",
            0x6A => "ROR A",
            0x6B => "RTL",

            0x78 => "SEI",
            0x7A => "PLY",
            0x7B => "TDC",

            0x80 => "BRA rel",
            0x88 => "DEY",
            0x8A => "TXA",
            0x8B => "PHB",

            0x98 => "TYA",
            0x9A => "TXS",
            0x9B => "TXY",
            0x9C => "STZ addr",

            0xA0 => "LDY #const",
            0xA2 => "LDX #const",
            0xA8 => "TAY",
            0xA9 => "LDA #const",
            0xAA => "TAX",
            0xAB => "PLB",

            0xB8 => "CLV",
            0xBA => "TSX",
            0xBB => "TYX",

            0xC0 => "CPY #const",
            0xC2 => "REP #const",
            0xC8 => "INY",
            0xCA => "DEX",
            0xCB => "WAI",

            0xD8 => "CLD",
            0xDA => "PHX",
            0xDB => "STP",

            0xE0 => "CPX #const",
            0xE2 => "SEP #const",
            0xE8 => "INX",
            0xEA => "NOP",
            0xEB => "XBA",

            0xF8 => "SED",
            0xFA => "PLX",
            0xFB => "XCE",

            _ => "???",
        };

        // オペランド付きで文字列を構築
        if operands.is_empty() {
            mnemonic.to_string()
        } else if operands.len() == 1 {
            format!("{} ${:02X}", mnemonic, operands[0])
        } else if operands.len() == 2 {
            format!("{} ${:02X}{:02X}", mnemonic, operands[1], operands[0])
        } else {
            format!(
                "{} ${:02X}{:02X}{:02X}",
                mnemonic, operands[2], operands[1], operands[0]
            )
        }
    }

    // デバッグ情報表示
    pub fn print_cpu_state(&self, cpu: &Cpu) {
        println!("\n=== CPU State ===");
        println!("PC: {:02X}:{:04X}", cpu.pb(), cpu.get_pc() & 0xFFFF);
        println!("A: {:04X}  X: {:04X}  Y: {:04X}", cpu.a(), cpu.x(), cpu.y());
        println!(
            "SP: {:04X}  DP: {:04X}  DB: {:02X}",
            cpu.sp(),
            cpu.dp(),
            cpu.db()
        );
        let p_bits = cpu.p().bits();
        println!(
            "P: {:02X} [{}{}{}{}{}{}{}{}]",
            p_bits,
            if p_bits & 0x80 != 0 { 'N' } else { '-' },
            if p_bits & 0x40 != 0 { 'V' } else { '-' },
            if p_bits & 0x20 != 0 { 'M' } else { '-' },
            if p_bits & 0x10 != 0 { 'X' } else { '-' },
            if p_bits & 0x08 != 0 { 'D' } else { '-' },
            if p_bits & 0x04 != 0 { 'I' } else { '-' },
            if p_bits & 0x02 != 0 { 'Z' } else { '-' },
            if p_bits & 0x01 != 0 { 'C' } else { '-' },
        );
        println!(
            "Cycles: {}  Instructions: {}",
            cpu.get_cycles(),
            self.instruction_count
        );
    }

    pub fn print_memory(&self, bus: &mut Bus, address: u32, length: usize) {
        println!("\n=== Memory Dump 0x{:06X} ===", address);

        for offset in (0..length).step_by(16) {
            print!("{:06X}: ", address + offset as u32);

            // Hex表示
            for i in 0..16 {
                if offset + i < length {
                    let byte = bus.read_u8(address + (offset + i) as u32);
                    print!("{:02X} ", byte);
                } else {
                    print!("   ");
                }
            }

            print!(" | ");

            // ASCII表示
            for i in 0..16 {
                if offset + i < length {
                    let byte = bus.read_u8(address + (offset + i) as u32);
                    if (0x20..0x7F).contains(&byte) {
                        print!("{}", byte as char);
                    } else {
                        print!(".");
                    }
                }
            }

            println!();
        }
    }

    pub fn print_trace(&self, count: usize) {
        let start = if self.state.trace_buffer.len() > count {
            self.state.trace_buffer.len() - count
        } else {
            0
        };

        println!("\n=== Execution Trace ===");
        for entry in &self.state.trace_buffer[start..] {
            println!(
                "{:02X}:{:04X}: {} | A:{:04X} X:{:04X} Y:{:04X} P:{:02X}",
                entry.registers.pb,
                entry.pc & 0xFFFF,
                entry.mnemonic,
                entry.registers.a,
                entry.registers.x,
                entry.registers.y,
                entry.registers.p
            );
        }
    }

    pub fn is_paused(&self) -> bool {
        self.state.paused
    }

    pub fn get_step_mode(&self) -> StepMode {
        self.state.step_mode.clone()
    }

    pub fn should_step(&mut self) -> bool {
        if !self.state.paused {
            return true;
        }

        match self.state.step_mode {
            StepMode::Continue => true,
            StepMode::StepInstruction => {
                // 1命令実行したら停止
                self.state.step_mode = StepMode::Continue;
                self.state.paused = true;
                true
            }
            StepMode::StepOver | StepMode::StepOut => {
                // これらは呼び出し元で適切に処理
                true
            }
        }
    }

    // メモリアクセス追跡
    pub fn track_memory_read(&mut self, address: u32) {
        self.memory_reads.insert(address);
    }

    pub fn track_memory_write(&mut self, address: u32) {
        self.memory_writes.insert(address);
    }

    pub fn get_memory_stats(&self) -> (usize, usize) {
        (self.memory_reads.len(), self.memory_writes.len())
    }

    pub fn clear_memory_tracking(&mut self) {
        self.memory_reads.clear();
        self.memory_writes.clear();
    }
}

impl fmt::Display for DebugState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Debug State: {} | Mode: {:?} | Breakpoints: {} | Watchpoints: {}",
            if self.paused { "PAUSED" } else { "RUNNING" },
            self.step_mode,
            self.breakpoints.len(),
            self.watchpoints.len()
        )
    }
}
