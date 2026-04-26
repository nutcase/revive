use revive_core::{CoreInstance, MemoryRegion};

#[derive(Debug, Clone)]
pub struct MemoryWrite {
    pub region: String,
    pub offset: usize,
    pub value: u8,
}

#[derive(Debug, Clone, Default)]
pub struct MemorySnapshot {
    segments: Vec<MemorySegment>,
    data: Vec<u8>,
}

#[derive(Debug, Clone)]
struct MemorySegment {
    id: String,
    label: String,
    start: usize,
    len: usize,
    writable: bool,
}

impl MemorySnapshot {
    pub fn capture(core: &CoreInstance) -> Self {
        let mut segments = Vec::new();
        let mut data = Vec::new();
        for region in core.memory_regions() {
            let Some(bytes) = core.read_memory(region.id) else {
                continue;
            };
            let start = data.len();
            data.extend_from_slice(bytes);
            segments.push(segment_from_region(region, start, bytes.len()));
        }
        Self { segments, data }
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub(crate) fn len(&self) -> usize {
        self.data.len()
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        &self.data
    }

    fn segment_for_combined_offset(&self, offset: usize) -> Option<(&MemorySegment, usize)> {
        self.segments.iter().find_map(|segment| {
            let local = offset.checked_sub(segment.start)?;
            (local < segment.len).then_some((segment, local))
        })
    }

    pub(crate) fn write_for_combined_offset(
        &self,
        offset: usize,
        value: u8,
    ) -> Option<MemoryWrite> {
        let (segment, local) = self.segment_for_combined_offset(offset)?;
        segment.writable.then(|| MemoryWrite {
            region: segment.id.clone(),
            offset: local,
            value,
        })
    }

    pub(crate) fn combined_offset_for_region(&self, region: &str, offset: usize) -> Option<usize> {
        self.segments
            .iter()
            .find(|segment| segment.id == region && offset < segment.len)
            .map(|segment| segment.start + offset)
    }

    pub(crate) fn format_combined_addr(&self, offset: usize) -> String {
        let Some((segment, local)) = self.segment_for_combined_offset(offset) else {
            return format!("{offset:06X}");
        };
        format_segment_addr(segment, local)
    }

    pub(crate) fn parse_addr(&self, input: &str) -> Option<usize> {
        let input = input.trim();
        if input.is_empty() {
            return None;
        }

        if let Some((prefix, rest)) = input.split_once(':') {
            let rest = parse_hex(rest)? as usize;
            if prefix.eq_ignore_ascii_case("s") {
                return self.combined_offset_for_region("sram", rest);
            }
            if prefix.eq_ignore_ascii_case("7e") || prefix.eq_ignore_ascii_case("7f") {
                let bank = u8::from_str_radix(prefix, 16).ok()?;
                let wram_offset = ((usize::from(bank) - 0x7E) << 16) + rest;
                return self.combined_offset_for_region("wram", wram_offset);
            }
            return self.combined_offset_for_region(prefix, rest);
        }

        let offset = parse_hex(input)? as usize;
        (offset < self.len()).then_some(offset)
    }

    pub(crate) fn region_summary(&self) -> String {
        if self.segments.is_empty() {
            return "No readable memory regions".to_string();
        }
        self.segments
            .iter()
            .map(|segment| {
                let access = if segment.writable { "RW" } else { "RO" };
                format!(
                    "{} ({}) {} {} bytes",
                    segment.id, segment.label, access, segment.len
                )
            })
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn segment_from_region(region: MemoryRegion, start: usize, len: usize) -> MemorySegment {
    MemorySegment {
        id: region.id.to_string(),
        label: region.label.to_string(),
        start,
        len,
        writable: region.writable,
    }
}

fn format_segment_addr(segment: &MemorySegment, local: usize) -> String {
    if segment.id == "wram" && segment.len == 0x2_0000 {
        let bank = 0x7E + (local >> 16);
        return format!("{bank:02X}:{:04X}", local & 0xFFFF);
    }
    if segment.id == "sram" {
        return format!("S:{local:04X}");
    }
    format!("{}:{local:04X}", segment.id)
}

fn parse_hex(input: &str) -> Option<u32> {
    let value = input
        .trim()
        .trim_start_matches('$')
        .trim_start_matches("0x")
        .trim_start_matches("0X");
    u32::from_str_radix(value, 16).ok()
}
