use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct RamSnapshot {
    data: Vec<u8>,
}

impl RamSnapshot {
    pub fn capture(ram: &[u8]) -> Self {
        Self { data: ram.to_vec() }
    }

    pub fn get(&self, offset: u32) -> u8 {
        self.data.get(offset as usize).copied().unwrap_or(0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchFilter {
    Equal(u8),
    NotEqual(u8),
    GreaterThan(u8),
    LessThan(u8),
    Increased,
    Decreased,
    Changed,
    Unchanged,
    IncreasedBy(u8),
    DecreasedBy(u8),
}

impl SearchFilter {
    pub fn needs_snapshot(self) -> bool {
        matches!(
            self,
            Self::Increased
                | Self::Decreased
                | Self::Changed
                | Self::Unchanged
                | Self::IncreasedBy(_)
                | Self::DecreasedBy(_)
        )
    }
}

pub struct CheatSearch {
    snapshot: Option<RamSnapshot>,
    candidates: Vec<u32>,
    ram_size: usize,
}

impl CheatSearch {
    pub fn new(ram_size: usize) -> Self {
        Self {
            snapshot: None,
            candidates: (0..ram_size as u32).collect(),
            ram_size,
        }
    }

    pub fn resize(&mut self, ram_size: usize) {
        if self.ram_size == ram_size {
            return;
        }
        self.ram_size = ram_size;
        self.snapshot = None;
        self.candidates = (0..ram_size as u32).collect();
    }

    pub fn snapshot(&mut self, ram: &[u8]) {
        self.resize(ram.len());
        self.snapshot = Some(RamSnapshot::capture(ram));
    }

    pub fn has_snapshot(&self) -> bool {
        self.snapshot.is_some()
    }

    pub fn previous_snapshot(&self) -> Option<&RamSnapshot> {
        self.snapshot.as_ref()
    }

    pub fn apply_filter(&mut self, filter: SearchFilter, ram: &[u8]) {
        self.resize(ram.len());

        let snapshot = match &self.snapshot {
            Some(snapshot) if filter.needs_snapshot() => snapshot.clone(),
            _ if filter.needs_snapshot() => return,
            _ => {
                self.candidates.retain(|&offset| {
                    let current = ram.get(offset as usize).copied().unwrap_or(0);
                    match filter {
                        SearchFilter::Equal(value) => current == value,
                        SearchFilter::NotEqual(value) => current != value,
                        SearchFilter::GreaterThan(value) => current > value,
                        SearchFilter::LessThan(value) => current < value,
                        _ => unreachable!(),
                    }
                });
                self.snapshot = Some(RamSnapshot::capture(ram));
                return;
            }
        };

        self.candidates.retain(|&offset| {
            let current = ram.get(offset as usize).copied().unwrap_or(0);
            let previous = snapshot.get(offset);
            match filter {
                SearchFilter::Increased => current > previous,
                SearchFilter::Decreased => current < previous,
                SearchFilter::Changed => current != previous,
                SearchFilter::Unchanged => current == previous,
                SearchFilter::IncreasedBy(delta) => current == previous.wrapping_add(delta),
                SearchFilter::DecreasedBy(delta) => current == previous.wrapping_sub(delta),
                _ => unreachable!(),
            }
        });
        self.snapshot = Some(RamSnapshot::capture(ram));
    }

    pub fn reset(&mut self) {
        self.snapshot = None;
        self.candidates = (0..self.ram_size as u32).collect();
    }

    pub fn candidates(&self) -> &[u32] {
        &self.candidates
    }

    pub fn candidate_count(&self) -> usize {
        self.candidates.len()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CheatEntry {
    pub region: String,
    pub offset: u32,
    pub value: u8,
    pub enabled: bool,
    pub label: String,
}

pub struct CheatManager {
    pub entries: Vec<CheatEntry>,
}

impl Default for CheatManager {
    fn default() -> Self {
        Self::new()
    }
}

impl CheatManager {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn add(&mut self, region: impl Into<String>, offset: u32, value: u8, label: String) {
        self.entries.push(CheatEntry {
            region: region.into(),
            offset,
            value,
            enabled: true,
            label,
        });
    }

    pub fn remove(&mut self, index: usize) {
        if index < self.entries.len() {
            self.entries.remove(index);
        }
    }

    pub fn save_to_file(&self, path: &std::path::Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }
        let json = serde_json::to_string_pretty(&self.entries).map_err(|err| err.to_string())?;
        std::fs::write(path, json).map_err(|err| err.to_string())
    }

    pub fn load_from_file(path: &std::path::Path) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|err| err.to_string())?;
        let entries: Vec<CheatEntry> =
            serde_json::from_slice(&bytes).map_err(|err| err.to_string())?;
        Ok(Self { entries })
    }

    pub fn enabled_entries(&self) -> impl Iterator<Item = &CheatEntry> {
        self.entries.iter().filter(|entry| entry.enabled)
    }
}

pub fn parse_u8_value(input: &str) -> Option<u8> {
    let s = input.trim();
    s.parse::<u8>().ok().or_else(|| {
        let hex = s
            .trim_start_matches('$')
            .trim_start_matches("0x")
            .trim_start_matches("0X");
        u8::from_str_radix(hex, 16).ok()
    })
}

pub fn parse_offset(input: &str) -> Option<u32> {
    let s = input
        .trim()
        .trim_start_matches('$')
        .trim_start_matches("0x")
        .trim_start_matches("0X");
    u32::from_str_radix(s, 16).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_equal_filters_candidates() {
        let mut ram = vec![0; 256];
        ram[0x10] = 0x42;
        ram[0x20] = 0x42;
        ram[0x30] = 0x99;

        let mut search = CheatSearch::new(ram.len());
        search.apply_filter(SearchFilter::Equal(0x42), &ram);

        assert_eq!(search.candidate_count(), 2);
        assert!(search.candidates().contains(&0x10));
        assert!(search.candidates().contains(&0x20));
    }

    #[test]
    fn manager_roundtrips_json() {
        let path = std::env::temp_dir().join(format!("revive-cheats-{}.json", std::process::id()));
        let mut manager = CheatManager::new();
        manager.add("wram", 0x1234, 0x56, "test".to_string());

        manager.save_to_file(&path).unwrap();
        let loaded = CheatManager::load_from_file(&path).unwrap();
        let _ = std::fs::remove_file(path);

        assert_eq!(loaded.entries.len(), 1);
        assert_eq!(loaded.entries[0].region, "wram");
        assert_eq!(loaded.entries[0].offset, 0x1234);
        assert_eq!(loaded.entries[0].value, 0x56);
    }
}
