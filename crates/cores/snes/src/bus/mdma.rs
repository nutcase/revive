use super::Bus;
use crate::debug_flags;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};

impl Bus {
    // 通常のDMA転送処理
    pub(super) fn perform_dma_transfer(&mut self, channel: usize) {
        // Cache all debug flags once at function entry to avoid repeated OnceLock lookups.
        let flag_dma = debug_flags::dma();
        let flag_quiet = crate::debug_flags::quiet();
        let flag_dma_probe = crate::debug_flags::dma_probe();
        let flag_trace_dma_dest = crate::debug_flags::trace_dma_dest();
        let flag_cgram_dma = crate::debug_flags::cgram_dma();
        let flag_trace_ppu_inidisp = crate::debug_flags::trace_ppu_inidisp();
        let flag_block_inidisp_dma = crate::debug_flags::block_inidisp_dma();
        let flag_trace_wram_stack_dma = crate::debug_flags::trace_wram_stack_dma();
        let flag_trace_oam_dma = crate::debug_flags::trace_oam_dma();
        let flag_trace_dma_setup_once = crate::debug_flags::trace_dma_setup_once();

        // General DMA: mark MDMA during this burst.
        // Prevent DMA read/write from being counted as CPU bus cycles.
        self.dma_in_progress = true;
        self.ppu.set_debug_dma_channel(Some(channel as u8));
        self.ppu.begin_mdma_context();
        if flag_dma_probe {
            let chp = &self.dma_controller.channels[channel];
            println!(
                "[DMA_PROBE] ch{} ctrl=0x{:02X} dest=$21{:02X} size=0x{:04X} src=0x{:06X}",
                channel, chp.control, chp.dest_address, chp.size, chp.src_address
            );
        }
        let ch = &self.dma_controller.channels[channel];
        // Skip obviously unconfigured junk (only skip if completely unconfigured)
        if !ch.configured {
            static mut DMA_SKIP_CFG_LOGGED: [bool; 8] = [false; 8];
            unsafe {
                if flag_dma && !DMA_SKIP_CFG_LOGGED[channel] {
                    println!(
                        "DMA skipped: CH{} not configured (ctrl=0x{:02X}, size={})",
                        channel, ch.control, ch.size
                    );
                    DMA_SKIP_CFG_LOGGED[channel] = true;
                }
            }
            return;
        }
        // 転送方向を取得
        let cpu_to_ppu = (ch.control & 0x80) == 0;

        let mut transfer_size = ch.size as u32;
        if transfer_size == 0 {
            // size未設定（0）をどう扱うか: デフォルトは実機同様65536、フラグで0扱いにできる
            if crate::debug_flags::dma_zero_is_zero() {
                if flag_dma {
                    println!(
                        "DMA size=0 treated as zero (env DMA_ZERO_IS_ZERO=1) ch{} ctrl=0x{:02X} dest=$21{:02X}",
                        channel, ch.control, ch.dest_address
                    );
                }
                self.ppu.end_mdma_context();
                self.ppu.set_debug_dma_channel(None);
                self.dma_in_progress = false;
                return;
            }

            if !ch.cfg_size {
                // 未設定サイズの誤爆を防ぐ（デフォルト0=65536で暴走しがち）
                if flag_dma {
                    println!(
                        "DMA skipped: CH{} size not configured (size=0, ctrl=0x{:02X}, dest=$21{:02X})",
                        channel, ch.control, ch.dest_address
                    );
                }
                self.ppu.end_mdma_context();
                self.ppu.set_debug_dma_channel(None);
                self.dma_in_progress = false;
                return;
            }
            // 実機仕様: size=0 は 65536バイト
            transfer_size = 0x10000;
        }
        let src_addr = ch.src_address;

        // --- burn-in-test.sfc DMA MEMORY diagnostics (opt-in) ---
        //
        // The official burn-in ROM uses DMA ch6/ch7 to roundtrip 0x1000 bytes between
        // WRAM $7E:4000 and VRAM (write via $2118/$2119, read via $2139/$213A).
        // If the DMA MEMORY test FAILs, enable TRACE_BURNIN_DMA_MEMORY=1 to print
        // a small fingerprint and detect common off-by-one/latch issues.
        let trace_burnin_dma_mem = crate::debug_flags::trace_burnin_dma_memory();
        #[derive(Clone, Copy)]
        #[allow(dead_code)]
        struct BurninDmaSnap {
            pc: u32,
            frame: u64,
            scanline: u16,
            cycle: u16,
            vblank: bool,
            hblank: bool,
            forced_blank: bool,
            vram_addr: u16,
            vram_inc: u16,
            vmain: u8,
            hash: u64,
            sample: [u8; 32],
        }
        static BURNIN_DMA_SNAP: OnceLock<Mutex<Option<BurninDmaSnap>>> = OnceLock::new();
        static BURNIN_DMA_DUMPED: OnceLock<AtomicU32> = OnceLock::new();
        let fnv1a64 = |data: &[u8]| -> u64 {
            let mut h: u64 = 0xcbf29ce484222325;
            for &b in data {
                h ^= b as u64;
                h = h.wrapping_mul(0x100000001b3);
            }
            h
        };

        // 特定ROM用のアドレス補正ハックは廃止（正規マッピング/CPU実装で解決する）

        // B-bus destination uses low 7 bits (0x2100-0x217F)
        let transfer_unit = ch.get_transfer_unit();
        let dest_base_full = ch.dest_address;
        if cpu_to_ppu
            && self.mapper_type == crate::cartridge::MapperType::SuperFx
            && (dest_base_full == 0x18 || dest_base_full == 0x19)
        {
            let source_bank = ((src_addr >> 16) & 0xFF) as u8;
            if let 0x70..=0x71 = source_bank {
                if let Some(gsu) = self.superfx.as_mut() {
                    let linear_addr =
                        ((source_bank as usize - 0x70) << 16) | ((src_addr & 0xFFFF) as usize);
                    gsu.capture_display_snapshot_for_dma(linear_addr, transfer_size as usize);
                }
            }
        }
        let trace_vram_cfg = {
            use std::sync::OnceLock;
            #[derive(Clone, Copy)]
            struct TraceCfg {
                start_addr: u16,
                end_addr: u16,
                frame_min: u64,
                frame_max: u64,
            }
            static CFG: OnceLock<Option<TraceCfg>> = OnceLock::new();
            *CFG.get_or_init(|| {
                fn parse_u16_env(value: &str) -> Option<u16> {
                    if let Some(hex) = value.strip_prefix("0x") {
                        u16::from_str_radix(hex, 16).ok()
                    } else if let Some(hex) = value.strip_prefix("0X") {
                        u16::from_str_radix(hex, 16).ok()
                    } else {
                        value.parse::<u16>().ok()
                    }
                }

                let (start_addr, end_addr) =
                    if let Ok(range) = std::env::var("TRACE_VRAM_ADDR_RANGE") {
                        let (start, end) = range.split_once('-')?;
                        let start_addr = parse_u16_env(start.trim())?;
                        let end_addr = parse_u16_env(end.trim())?;
                        (start_addr.min(end_addr), start_addr.max(end_addr))
                    } else {
                        let addr_str = std::env::var("TRACE_VRAM_ADDR").ok()?;
                        let addr = parse_u16_env(&addr_str)?;
                        (addr, addr)
                    };
                let frame_min = std::env::var("TRACE_VRAM_FRAME_MIN")
                    .ok()
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(0);
                let frame_max = std::env::var("TRACE_VRAM_FRAME_MAX")
                    .ok()
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(u64::MAX);
                Some(TraceCfg {
                    start_addr,
                    end_addr,
                    frame_min,
                    frame_max,
                })
            })
        };
        let trace_vram_dma_range = {
            use std::sync::OnceLock;
            #[derive(Clone, Copy)]
            struct TraceRangeCfg {
                frame_min: u64,
                frame_max: u64,
            }
            static CFG: OnceLock<Option<TraceRangeCfg>> = OnceLock::new();
            *CFG.get_or_init(|| {
                let enabled = std::env::var("TRACE_VRAM_DMA_RANGE")
                    .ok()
                    .map(|v| v != "0" && !v.eq_ignore_ascii_case("false"))
                    .unwrap_or(false);
                if !enabled {
                    return None;
                }
                let frame_min = std::env::var("TRACE_VRAM_FRAME_MIN")
                    .ok()
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(0);
                let frame_max = std::env::var("TRACE_VRAM_FRAME_MAX")
                    .ok()
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(u64::MAX);
                Some(TraceRangeCfg {
                    frame_min,
                    frame_max,
                })
            })
        };

        // burn-in-test.sfc: track unexpected VRAM DMAs that might clobber the DMA MEMORY test region.
        // (Covers both $2118/$2119 bases and all transfer modes; we only special-case the known
        // DMA MEMORY write via ch6.)
        if trace_burnin_dma_mem && cpu_to_ppu && (dest_base_full == 0x18 || dest_base_full == 0x19)
        {
            let (vmadd_start, vram_inc, vmain) = self.ppu.dbg_vram_regs();
            if vram_inc == 1 {
                let words = (transfer_size / 2) as u16;
                let vmadd_end = vmadd_start.wrapping_add(words);
                let overlaps = vmadd_start < 0x5800 && vmadd_end > 0x5000;
                let is_known_dmamem_write =
                    channel == 6 && src_addr == 0x7E4000 && transfer_size == 0x1000;
                if overlaps && !is_known_dmamem_write {
                    println!(
                        "[BURNIN-DMAMEM] UNEXPECTED VRAM DMA: pc={:06X} ch{} src=0x{:06X} size=0x{:04X} base=$21{:02X} unit={} addr_mode={} VMADD={}..{} VMAIN={:02X}",
                        self.last_cpu_pc,
                        channel,
                        src_addr,
                        transfer_size,
                        dest_base_full,
                        transfer_unit,
                        ch.get_address_mode(),
                        vmadd_start,
                        vmadd_end,
                        vmain
                    );
                }
            }
        }

        // Snapshot the source buffer before it gets overwritten by the VRAM->WRAM read-back DMA.
        if trace_burnin_dma_mem
            && cpu_to_ppu
            && channel == 6
            && transfer_unit == 1
            && dest_base_full == 0x18
            && src_addr == 0x7E4000
            && transfer_size == 0x1000
            && self.wram.len() >= 0x5000
        {
            let slice = &self.wram[0x4000..0x5000];
            let mut sample = [0u8; 32];
            for (seg, off) in [0x000usize, 0x100, 0x200, 0x300].into_iter().enumerate() {
                let start = seg * 8;
                sample[start..start + 8].copy_from_slice(&slice[off..off + 8]);
            }
            let hash = fnv1a64(slice);
            let (vram_addr, vram_inc, vmain) = self.ppu.dbg_vram_regs();
            let pc = self.last_cpu_pc;
            // Arm fine-grained VRAM clobber tracing (PPU-side) after the DMA MEMORY routine starts.
            self.ppu.arm_burnin_vram_trace();
            let frame = self.ppu.get_frame();
            let scanline = self.ppu.get_scanline();
            let cycle = self.ppu.get_cycle();
            let vblank = self.ppu.is_vblank();
            let hblank = self.ppu.is_hblank();
            let forced_blank = self.ppu.is_forced_blank();
            *BURNIN_DMA_SNAP
                .get_or_init(|| Mutex::new(None))
                .lock()
                .unwrap() = Some(BurninDmaSnap {
                pc,
                frame,
                scanline,
                cycle,
                vblank,
                hblank,
                forced_blank,
                vram_addr,
                vram_inc,
                vmain,
                hash,
                sample,
            });
            println!(
                "[BURNIN-DMAMEM] SNAP pc={:06X} frame={} sl={} cyc={} vblank={} hblank={} fblank={} VMADD={:04X} VMAIN={:02X} inc={} hash={:016X} sample@0/100/200/300={:02X?}",
                pc,
                frame,
                scanline,
                cycle,
                vblank as u8,
                hblank as u8,
                forced_blank as u8,
                vram_addr,
                vmain,
                vram_inc,
                hash,
                sample
            );
        }

        if cpu_to_ppu && (dest_base_full == 0x18 || dest_base_full == 0x19) {
            if let Some(cfg) = trace_vram_dma_range {
                let frame = self.ppu.get_frame();
                if frame >= cfg.frame_min && frame <= cfg.frame_max {
                    let (vmadd_start, vram_inc, vmain) = self.ppu.dbg_vram_regs();
                    let words = (transfer_size / 2) as u16;
                    let vmadd_end = vmadd_start.wrapping_add(words);
                    let overlaps = trace_vram_cfg
                        .map(|vram_cfg| {
                            vmadd_start <= vram_cfg.end_addr && vmadd_end >= vram_cfg.start_addr
                        })
                        .unwrap_or(true);
                    if overlaps {
                        println!(
                            "[TRACE_VRAM_DMA_RANGE] frame={} pc={:06X} ch{} src=0x{:06X} size={} unit={} addr_mode={} VMADD={:04X}..{:04X} VMAIN={:02X} inc={}",
                            frame,
                            self.last_cpu_pc,
                            channel,
                            src_addr,
                            transfer_size,
                            transfer_unit,
                            ch.get_address_mode(),
                            vmadd_start,
                            vmadd_end,
                            vmain,
                            vram_inc
                        );
                    }
                }
            }
        }

        if dest_base_full == 0 {
            static INIDISP_DMA_ALERT: OnceLock<AtomicU32> = OnceLock::new();
            let n = INIDISP_DMA_ALERT
                .get_or_init(|| AtomicU32::new(0))
                .fetch_add(1, Ordering::Relaxed);
            if n < 4 {
                println!(
                    "[DEBUG-INIDISP-DMA] ch{} ctrl=0x{:02X} src=0x{:06X} size={} unit={} addr_mode={} (dest_base=0) mdmaen=0x{:02X}",
                    channel,
                    ch.control,
                    src_addr,
                    transfer_size,
                    transfer_unit,
                    ch.get_address_mode(),
                    self.dma_controller.dma_enable
                );
            }
        }
        if flag_trace_dma_dest {
            println!(
                "[DMA-DEST] ch{} ctrl=0x{:02X} dest_base=$21{:02X} size={} unit={} addr_mode={}",
                channel,
                ch.control,
                dest_base_full,
                transfer_size,
                transfer_unit,
                ch.get_address_mode()
            );
        }

        // DMA転送のデバッグ（許可時のみ）

        // Early sanity check: skip obviously invalid B-bus target ranges to reduce noise
        // CPU->PPU: allow $2100-$2133 and $2140-$2143 only
        // PPU->CPU: allow $2134-$213F and $2140-$2143 only
        let allowed = if cpu_to_ppu {
            (dest_base_full <= 0x33)
                || (0x40..=0x43).contains(&dest_base_full)
                || (0x80..=0x83).contains(&dest_base_full) // WRAM port
        } else {
            (0x34..=0x3F).contains(&dest_base_full)
                || (0x40..=0x43).contains(&dest_base_full)
                || (0x80..=0x83).contains(&dest_base_full)
        };
        if !allowed {
            static DMA_BBUS_WARN: OnceLock<AtomicU32> = OnceLock::new();
            {
                let ctr = DMA_BBUS_WARN.get_or_init(|| AtomicU32::new(0));
                if flag_dma && ctr.load(Ordering::Relaxed) < 8 {
                    ctr.fetch_add(1, Ordering::Relaxed);
                    println!(
                        "DMA skipped: CH{} {} to invalid B-bus $21{:02X} (size={})",
                        channel,
                        if cpu_to_ppu { "CPU->PPU" } else { "PPU->CPU" },
                        dest_base_full,
                        transfer_size
                    );
                }
            }
            return;
        }
        // Guard against obviously bogus INIDISP floods (e.g., uninitialized channels)
        // Note: This early-return used to drop large MDMA transfers targeting $2100.
        // However, some titles briefly program DMA with size 0 (=> 65536) before
        // immediately updating the registers. Skipping here could eat real transfers
        // when dest decoding goes wrong, leaving the screen black.  Allow them to run;
        // the regular PPU register handling will clamp brightness safely.
        // if cpu_to_ppu && dest_base_full == 0x00 && transfer_size > 0x0100 {
        //     static SKIP_INIDISP_DMA: OnceLock<AtomicU32> = OnceLock::new();
        //     let n = SKIP_INIDISP_DMA
        //         .get_or_init(|| AtomicU32::new(0))
        //         .fetch_add(1, Ordering::Relaxed);
        //     if n < 4 {
        //         println!(
        //             "⚠️  Skipping suspicious INIDISP DMA: ch{} size={} src=0x{:06X} mdmaen=0x{:02X}",
        //             channel, transfer_size, src_addr, self.dma_controller.dma_enable
        //         );
        //     }
        //     self.ppu.end_mdma_context();
        //     self.ppu.set_debug_dma_channel(None);
        //     return;
        // }

        // ここまで到達したものだけを転送ログ対象にする
        if flag_dma {
            static DMA_COUNT: OnceLock<AtomicU32> = OnceLock::new();
            let n = DMA_COUNT
                .get_or_init(|| AtomicU32::new(0))
                .fetch_add(1, Ordering::Relaxed)
                + 1;
            if n <= 10 || transfer_size > 100 {
                let vmadd = self.ppu.vram_addr;
                let vmain = self.ppu.vram_mapping;
                println!(
                    "DMA Transfer #{}: CH{} {} size={} src=0x{:06X} dest=$21{:02X} VMADD=0x{:04X} VMAIN=0x{:02X}",
                    n,
                    channel,
                    if cpu_to_ppu { "CPU->PPU" } else { "PPU->CPU" },
                    transfer_size,
                    src_addr,
                    dest_base_full,
                    vmadd,
                    vmain
                );
            }
        }

        // Special log for CGRAM transfers (debug-only)
        if flag_cgram_dma && !flag_quiet && dest_base_full == 0x22 && cpu_to_ppu {
            static CGRAM_DMA_COUNT: OnceLock<AtomicU32> = OnceLock::new();
            let n = CGRAM_DMA_COUNT
                .get_or_init(|| AtomicU32::new(0))
                .fetch_add(1, Ordering::Relaxed)
                + 1;
            if n <= 20 {
                println!(
                    "🎨 CGRAM DMA #{}: CH{} size={} src=0x{:06X} -> $2122 (CGDATA)",
                    n, channel, transfer_size, src_addr
                );
            }
        }

        if transfer_size == 0 {
            return; // 転送サイズが0なら何もしない
        }

        // NOTE: PPU->CPU DMA from $2134 (Mode7 product) is commonly used as a fast
        // "memset" trick (fill WRAM with a constant). Do NOT clamp its size here.

        // 実際の転送を実行
        if flag_trace_wram_stack_dma && cpu_to_ppu && (0x80..=0x83).contains(&dest_base_full) {
            println!(
                "[WRAM-DMA-START] ch{} start_wram_addr=0x{:05X} size=0x{:04X} src=0x{:06X}",
                channel, self.wram_address, transfer_size, src_addr
            );
        }

        // burn-in-test.sfc DMA MEMORY: capture the destination WRAM buffer before VRAM->WRAM DMA overwrites it.
        if trace_burnin_dma_mem
            && !cpu_to_ppu
            && channel == 7
            && transfer_unit == 1
            && dest_base_full == 0x39
            && src_addr == 0x7E4000
            && transfer_size == 0x1000
            && self.wram.len() >= 0x5000
        {
            let pre = &self.wram[0x4000..0x5000];
            let burnin_pre_wram_hash = fnv1a64(pre);
            println!(
                "[BURNIN-DMAMEM] PREREAD-WRAM pc={:06X} hash={:016X}",
                self.last_cpu_pc, burnin_pre_wram_hash
            );
        }

        let mut cur_src = src_addr;
        let addr_mode = ch.get_address_mode(); // 0:inc, 1:fix, 2:dec, 3:inc(approx)

        // Optional OAM DMA tracing (helps diagnose sprite corruption on scene transitions)
        if flag_trace_oam_dma && cpu_to_ppu && dest_base_full == 0x04 {
            let frame = self.ppu.get_frame();
            let frame_min = std::env::var("TRACE_OAM_DMA_FRAME_MIN")
                .ok()
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(0);
            let frame_max = std::env::var("TRACE_OAM_DMA_FRAME_MAX")
                .ok()
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(u64::MAX);
            if frame >= frame_min && frame <= frame_max {
                let (oam_addr, oam_internal) = self.ppu.dbg_oam_addrs();
                eprintln!(
                    "[OAM-DMA] frame={} ch{} size={} src=0x{:06X} unit={} addr_mode={} pc={:06X} oam_addr=0x{:03X} oam_int=0x{:03X} sl={}",
                    frame,
                    channel,
                    transfer_size,
                    src_addr,
                    transfer_unit,
                    addr_mode,
                    self.last_cpu_pc,
                    oam_addr,
                    oam_internal,
                    self.ppu.scanline
                );
                // Dump sprite entries from WRAM source to see what game prepared
                if std::env::var_os("TRACE_OAM_DMA_DUMP").is_some() {
                    let wram_base = (src_addr & 0x1FFFF) as usize;
                    let sprite_count = (transfer_size.min(512) / 4) as usize;
                    for s in 0..sprite_count {
                        let off = wram_base + s * 4;
                        if off + 3 < self.wram.len() {
                            let y = self.wram[off];
                            let x_lo = self.wram[off + 1];
                            let tile = self.wram[off + 2];
                            let attr = self.wram[off + 3];
                            // Only dump sprites with non-trivial position
                            if y < 224 || y > 192 {
                                eprintln!("[OAM-DMA-DUMP] src=0x{:06X} #{:3} y={:3} x_lo={:3} tile=0x{:02X} attr=0x{:02X}",
                                    src_addr, s, y, x_lo, tile, attr);
                            }
                        }
                    }
                }
            }
        }
        let mut i = 0;

        // Debug: capture first few DMA setups to see what games configure (helps stuck WRAM fills)
        if flag_trace_dma_setup_once {
            use std::sync::atomic::{AtomicU32, Ordering};
            static ONCE: AtomicU32 = AtomicU32::new(0);
            let count = ONCE.fetch_add(1, Ordering::Relaxed);
            if count < 16 {
                println!(
                    "[DMA-SETUP] ch{} ctrl=0x{:02X} dest_base=$21{:02X} size={} src=0x{:06X} unit={} addr_mode={} cfgSz={} cfgDst={} cfgSrc={} cfgCtrl={}",
                    channel,
                    ch.control,
                    dest_base_full,
                    transfer_size,
                    src_addr,
                    transfer_unit,
                    addr_mode,
                    ch.cfg_size,
                    ch.cfg_dest,
                    ch.cfg_src,
                    ch.cfg_ctrl,
                );
            }
        }
        // CGRAM DMA burst summary (debug): capture first few bytes and total count
        let capture_cgram = flag_cgram_dma && (dest_base_full == 0x22) && cpu_to_ppu;
        let mut cgram_first: [u8; 16] = [0; 16];
        let mut cgram_captured: usize = 0;
        let mut cgram_total: u32 = 0;
        // 実機準拠: 転送サイズ全体を処理（サイズ=0は65536バイト）
        while (i as u32) < transfer_size {
            if cpu_to_ppu {
                // CPU -> PPU転送（最も一般的）
                // Bバス宛先アドレスを転送モードに応じて決定
                let dest_offset = self.mdma_dest_offset(transfer_unit, dest_base_full, i as u8);

                if flag_trace_dma_dest && channel == 0 && i < 32 {
                    println!(
                        "[DMA-DEST-TRACE] ch{} i={} base=$21{:02X} unit={} dest_offset=$21{:02X}",
                        channel, i, dest_base_full, transfer_unit, dest_offset
                    );
                }

                let dest_full = 0x2100 + dest_offset as u32;
                self.dma_hist_note(dest_offset);

                // S-DD1 DMA interception: if this channel has decompression
                // enabled and the source address matches, return decompressed data.
                let data = if let Some(ref mut sdd) = self.sdd1 {
                    if let Some(byte) = sdd.dma_read(cur_src, &self.rom, self.rom_size) {
                        byte
                    } else {
                        self.dma_read_a_bus(cur_src)
                    }
                } else {
                    self.dma_read_a_bus(cur_src)
                };
                if let Some(cfg) = trace_vram_cfg {
                    if dest_offset == 0x18 || dest_offset == 0x19 {
                        let frame = self.ppu.get_frame();
                        if frame >= cfg.frame_min && frame <= cfg.frame_max {
                            let (vmadd, _inc, vmain) = self.ppu.dbg_vram_regs();
                            if vmadd >= cfg.start_addr && vmadd <= cfg.end_addr {
                                println!(
                                    "[TRACE_VRAM_DMA] frame={} ch{} src=0x{:06X} dest=$21{:02X} vmadd=0x{:04X} vmain=0x{:02X} data=0x{:02X} range=0x{:04X}-0x{:04X}",
                                    frame,
                                    channel,
                                    cur_src,
                                    dest_offset,
                                    vmadd,
                                    vmain,
                                    data,
                                    cfg.start_addr,
                                    cfg.end_addr
                                );
                            }
                        }
                    }
                }

                // One-shot trace of early DMA bytes to understand real dests (opt-in)
                if flag_dma && !flag_quiet {
                    use std::sync::atomic::{AtomicU32, Ordering};
                    static BYTE_TRACE_COUNT: AtomicU32 = AtomicU32::new(0);
                    let n = BYTE_TRACE_COUNT.fetch_add(1, Ordering::Relaxed);
                    if n < 64 {
                        println!(
                            "[DMA-BYTE] ch{} i={} base=$21{:02X} offset=$21{:02X} full=$21{:04X} src=0x{:06X} data=0x{:02X}",
                            channel,
                            i,
                            dest_base_full,
                            dest_offset,
                            dest_full,
                            cur_src,
                            data
                        );
                    }
                }

                // SMW APU HLE: 2180-2183 (WRAM port) に向かうDMAを捕まえてSPC転送バッファを構築
                if self.smw_apu_hle && !self.smw_apu_hle_done && dest_base_full >= 0x80 {
                    self.smw_apu_hle_buf.push(data);
                }

                // Log INIDISP ($2100) writes during DMA to diagnose forced blank issues (opt-in)
                if dest_offset == 0x00 && flag_trace_ppu_inidisp && !flag_quiet {
                    static INIDISP_DMA_COUNT: OnceLock<AtomicU32> = OnceLock::new();
                    let n = INIDISP_DMA_COUNT
                        .get_or_init(|| AtomicU32::new(0))
                        .fetch_add(1, Ordering::Relaxed)
                        + 1;
                    if n <= 128 {
                        println!(
                            "[INIDISP-DMA] #{}: CH{} src=0x{:06X} value=0x{:02X} (blank={} brightness={})",
                            n,
                            channel,
                            cur_src,
                            data,
                            if (data & 0x80) != 0 { "ON" } else { "OFF" },
                            data & 0x0F
                        );
                    }
                }

                if flag_cgram_dma && dest_offset == 0x22 {
                    static CGDMA_BYTES: OnceLock<AtomicU32> = OnceLock::new();
                    let n = CGDMA_BYTES
                        .get_or_init(|| AtomicU32::new(0))
                        .fetch_add(1, Ordering::Relaxed)
                        + 1;
                    if n <= 16 {
                        println!(
                            "CGRAM DMA byte #{}: src=0x{:06X} data=0x{:02X}",
                            n, cur_src, data
                        );
                    }
                }
                // Debug capture for CGRAM bursts
                if capture_cgram && dest_offset == 0x22 {
                    if cgram_captured < cgram_first.len() {
                        cgram_first[cgram_captured] = data;
                        cgram_captured += 1;
                    }
                    cgram_total = cgram_total.saturating_add(1);
                }

                // PPU writable ($2100-$2133)
                if dest_offset <= 0x33 {
                    // Optional debug guard: block DMA writes to INIDISP
                    if dest_offset == 0x00 && flag_block_inidisp_dma {
                        static mut INIDISP_DMA_BLOCK_LOG: u32 = 0;
                        unsafe {
                            if INIDISP_DMA_BLOCK_LOG < 8 {
                                INIDISP_DMA_BLOCK_LOG += 1;
                                println!(
                                    "⛔ BLOCK_INIDISP_DMA: ch{} data=0x{:02X} src=0x{:06X} i={} transfer_size={}",
                                    channel, data, cur_src, i, transfer_size
                                );
                            }
                        }
                        // advance addresses but skip write
                        i += 1;
                        // DMAP bit3=1 => fixed; bit4 is ignored in that case.
                        cur_src = match addr_mode {
                            0 => cur_src.wrapping_add(1), // inc
                            2 => cur_src.wrapping_sub(1), // dec
                            _ => cur_src,                 // fixed (1 or 3)
                        };
                        continue;
                    }
                    self.write_u8(dest_full, data);
                } else if (0x80..=0x83).contains(&dest_offset) {
                    // WRAM port ($2180-$2183)
                    self.write_u8(dest_full, data);
                } else if (0x40..=0x43).contains(&dest_offset) {
                    // APU I/O ($2140-$2143)
                    self.write_u8(dest_full, data);
                } else {
                    // $2134-$213F read-only or $2144-$217F undefined: ignore
                    static DMA_SKIP_DEST_LOGGED: OnceLock<Mutex<[bool; 256]>> = OnceLock::new();
                    let mut logged = DMA_SKIP_DEST_LOGGED
                        .get_or_init(|| Mutex::new([false; 256]))
                        .lock()
                        .unwrap();
                    let idx = dest_offset as usize;
                    if idx < logged.len() && flag_dma && !logged[idx] {
                        println!(
                            "DMA skipped invalid dest: CH{} base=$21{:02X} (read-only/unimplemented)",
                            channel,
                            dest_offset
                        );
                        logged[idx] = true;
                    }
                }

                // VRAMへの転送の場合は、デバッグ出力
                if flag_dma && (dest_full == 0x2118 || dest_full == 0x2119) {
                    static mut DMA_VRAM_COUNT: u32 = 0;
                    unsafe {
                        DMA_VRAM_COUNT += 1;
                        if DMA_VRAM_COUNT <= 10 {
                            println!("DMA to VRAM: src=0x{:06X}, data=0x{:02X}", cur_src, data);
                        }
                    }
                }
            } else {
                // PPU -> CPU転送（稀）
                let dest_offset = self.mdma_dest_offset(transfer_unit, dest_base_full, i as u8);
                let dest_reg = 0x2100 + dest_offset as u32;
                let data = self.read_u8(dest_reg);
                self.dma_write_a_bus(cur_src, data);
            }

            // A-busアドレスの更新（バンク固定、16bitアドレスのみ増減）
            let bank = cur_src & 0x00FF_0000;
            let lo16 = (cur_src & 0x0000_FFFF) as u16;
            let next_lo16 = match addr_mode {
                0 => lo16.wrapping_add(1), // inc
                2 => lo16.wrapping_sub(1), // dec
                _ => lo16,                 // fixed (1 or 3)
            } as u32;
            cur_src = bank | next_lo16;
            i += 1;
        }

        // --- DMA register side effects (hardware behavior) ---
        //
        // SNESdev wiki:
        // - After DMA completes, DASn becomes 0.
        // - A1Tn (low 16 bits) advances by the number of bytes transferred for increment/decrement
        //   modes; the bank (A1Bn) is fixed and wraps at the bank boundary.
        //
        // We model this by updating the channel's A-bus address (src_address) to the final cur_src
        // and clearing the transfer size register.
        {
            let ch = &mut self.dma_controller.channels[channel];
            ch.src_address = cur_src;
            ch.size = 0;
        }

        // --- Timing: S-CPU stalls during MDMA ---
        //
        // On real hardware, general DMA blocks the S-CPU while the PPU/APU continue to run.
        // We approximate the duration as:
        //   8 master cycles per transferred byte + 8 master cycles overhead.
        //
        // (This is intentionally tracked in master cycles so it can be applied without rounding.)
        let bytes_transferred = i.max(0) as u64;
        if bytes_transferred > 0 {
            let stall_master_cycles = 8u64.saturating_mul(bytes_transferred.saturating_add(1));
            self.add_pending_stall_master_cycles(stall_master_cycles);
        }

        // After WRAM->VRAM DMA completes, verify the target VRAM range matches the source buffer.
        // This helps distinguish "VRAM write blocked/corrupted" vs "VRAM read-back wrong".
        if trace_burnin_dma_mem
            && cpu_to_ppu
            && channel == 6
            && transfer_unit == 1
            && dest_base_full == 0x18
            && src_addr == 0x7E4000
            && transfer_size == 0x1000
            && self.wram.len() >= 0x5000
        {
            let src = &self.wram[0x4000..0x5000];
            let src_hash = fnv1a64(src);
            let vram = self.ppu.get_vram();
            let start = 0x5000usize.saturating_mul(2);
            let end = start.saturating_add(0x1000).min(vram.len());
            let vram_slice = &vram[start..end];
            let vram_hash = fnv1a64(vram_slice);
            println!(
                "[BURNIN-DMAMEM] POSTWRITE pc={:06X} VMADD_end={:04X} src_hash={:016X} vram_hash={:016X} match={}",
                self.last_cpu_pc,
                self.ppu.dbg_vram_regs().0,
                src_hash,
                vram_hash,
                (src_hash == vram_hash) as u8
            );
        }

        // Before VRAM->WRAM DMA begins, fingerprint the VRAM range that should be read back.
        if trace_burnin_dma_mem
            && !cpu_to_ppu
            && channel == 7
            && transfer_unit == 1
            && dest_base_full == 0x39
            && src_addr == 0x7E4000
            && transfer_size == 0x1000
        {
            let vram = self.ppu.get_vram();
            let start = 0x5000usize.saturating_mul(2);
            let end = start.saturating_add(0x1000).min(vram.len());
            let vram_slice = &vram[start..end];
            let vram_hash = fnv1a64(vram_slice);
            println!(
                "[BURNIN-DMAMEM] PREREAD pc={:06X} VMADD_start={:04X} vram_hash={:016X}",
                self.last_cpu_pc,
                self.ppu.dbg_vram_regs().0,
                vram_hash
            );
        }

        // Compare read-back buffer after VRAM->WRAM DMA completes.
        if trace_burnin_dma_mem
            && !cpu_to_ppu
            && channel == 7
            && transfer_unit == 1
            && dest_base_full == 0x39
            && src_addr == 0x7E4000
            && transfer_size == 0x1000
            && self.wram.len() >= 0x5000
        {
            let slice = &self.wram[0x4000..0x5000];
            let mut sample = [0u8; 32];
            for (seg, off) in [0x000usize, 0x100, 0x200, 0x300].into_iter().enumerate() {
                let start = seg * 8;
                sample[start..start + 8].copy_from_slice(&slice[off..off + 8]);
            }
            let hash = fnv1a64(slice);
            let (vram_addr, vram_inc, vmain) = self.ppu.dbg_vram_regs();
            let pc = self.last_cpu_pc;
            let snap = *BURNIN_DMA_SNAP
                .get_or_init(|| Mutex::new(None))
                .lock()
                .unwrap();
            if let Some(s) = snap {
                let ok = s.hash == hash;
                println!(
                    "[BURNIN-DMAMEM] READBACK pc={:06X} frame={} sl={} cyc={} vblank={} hblank={} fblank={} VMADD={:04X} VMAIN={:02X} inc={} hash={:016X} match={}",
                    pc,
                    self.ppu.get_frame(),
                    self.ppu.get_scanline(),
                    self.ppu.get_cycle(),
                    self.ppu.is_vblank() as u8,
                    self.ppu.is_hblank() as u8,
                    self.ppu.is_forced_blank() as u8,
                    vram_addr,
                    vmain,
                    vram_inc,
                    hash,
                    ok as u8
                );
                if !ok {
                    // Count and summarize differences (byte-wise) to spot shifts vs corruption.
                    let mut diff_count: u32 = 0;
                    let mut first_diff: Option<usize> = None;
                    for (i, (&a, &b)) in s.sample.iter().zip(sample.iter()).enumerate() {
                        if a != b {
                            diff_count = diff_count.saturating_add(1);
                            if first_diff.is_none() {
                                first_diff = Some(i);
                            }
                        }
                    }
                    println!(
                        "[BURNIN-DMAMEM] mismatch: src(pc={:06X} VMADD={:04X} VMAIN={:02X} inc={} hash={:016X} sample={:02X?}) rb(sample={:02X?})",
                        s.pc,
                        s.vram_addr,
                        s.vmain,
                        s.vram_inc,
                        s.hash,
                        s.sample,
                        sample
                    );
                    println!(
                        "[BURNIN-DMAMEM] mismatch detail: sample_diff_bytes={} first_diff_idx={}",
                        diff_count,
                        first_diff.map(|v| v as i32).unwrap_or(-1)
                    );
                    // One-shot dump for offline diffing.
                    let dumped = BURNIN_DMA_DUMPED
                        .get_or_init(|| AtomicU32::new(0))
                        .fetch_add(1, Ordering::Relaxed);
                    if dumped == 0 {
                        let src_wram = &self.wram[0x4000..0x5000];
                        let vram = self.ppu.get_vram();
                        let start = 0x5000usize.saturating_mul(2);
                        let end = start.saturating_add(0x1000).min(vram.len());
                        let vram_slice = &vram[start..end];
                        let _ = std::fs::create_dir_all("logs");
                        let _ = std::fs::write("logs/burnin_dmamem_src_wram.bin", src_wram);
                        let _ = std::fs::write("logs/burnin_dmamem_rb_wram.bin", slice);
                        let _ = std::fs::write("logs/burnin_dmamem_vram.bin", vram_slice);
                        println!(
                            "[BURNIN-DMAMEM] dumped logs/burnin_dmamem_src_wram.bin, logs/burnin_dmamem_rb_wram.bin, logs/burnin_dmamem_vram.bin"
                        );
                    }
                }
            } else {
                println!(
                    "[BURNIN-DMAMEM] READBACK pc={:06X} VMADD={:04X} VMAIN={:02X} inc={} hash={:016X} sample={:02X?} (no source snap)",
                    pc, vram_addr, vmain, vram_inc, hash, sample
                );
            }
        }

        if capture_cgram && cgram_total > 0 {
            let shown = cgram_captured.min(8);
            let bytes: Vec<String> = cgram_first
                .iter()
                .take(shown)
                .map(|b| format!("{:02X}", b))
                .collect();
            println!(
                "CGRAM DMA summary: ch{} total_bytes={} first[{}]=[{}]",
                channel,
                cgram_total,
                shown,
                bytes.join(", ")
            );
        }

        // SMW APU HLE: 十分なWRAM DMAデータが溜まったら一度だけSPCへロード
        if self.smw_apu_hle && !self.smw_apu_hle_done && self.smw_apu_hle_buf.len() >= 0x8400 {
            if let Ok(mut apu) = self.apu.lock() {
                apu.load_and_start(&self.smw_apu_hle_buf, 0x0400, 0x0400);
                self.smw_apu_hle_done = true;
                if crate::debug_flags::trace_smw_apu_hle() {
                    println!(
                        "[SMW-APU-HLE] Loaded {} bytes from WRAM DMA into SPC, start_pc=$0400",
                        self.smw_apu_hle_buf.len()
                    );
                }
            }
        }

        self.ppu.end_mdma_context();
        self.dma_in_progress = false;
    }

    #[inline]
    fn mdma_dest_offset(&self, unit: u8, base: u8, index: u8) -> u8 {
        // SNESdev wiki: B-bus address is an 8-bit selector in $2100-$21FF; additions wrap at 0xFF.
        // Transfer pattern (DMAPn bits 0-2) selects the B-bus address sequence.
        let i = index as usize;
        const P0: &[u8] = &[0];
        const P1: &[u8] = &[0, 1];
        const P2: &[u8] = &[0, 0];
        const P3: &[u8] = &[0, 0, 1, 1];
        const P4: &[u8] = &[0, 1, 2, 3];
        const P5: &[u8] = &[0, 1, 0, 1]; // undocumented
        const P6: &[u8] = &[0, 0]; // undocumented (same as 2)
        const P7: &[u8] = &[0, 0, 1, 1]; // undocumented (same as 3)
        let pat = match unit & 0x07 {
            0 => P0,
            1 => P1,
            2 => P2,
            3 => P3,
            4 => P4,
            5 => P5,
            6 => P6,
            _ => P7,
        };
        base.wrapping_add(pat[i % pat.len()])
    }

    fn dma_hist_note(&mut self, dest_off: u8) {
        let idx = dest_off as usize;
        if idx < self.dma_dest_hist.len() {
            self.dma_dest_hist[idx] = self.dma_dest_hist[idx].saturating_add(1);
        }
    }

    pub fn take_dma_dest_summary(&mut self) -> String {
        let mut parts = Vec::new();
        let mut push = |name: &str, off: u8| {
            let n = self.dma_dest_hist[off as usize];
            if n > 0 {
                parts.push(format!("{}:{}", name, n));
            }
        };
        // Key PPU ports
        push("OAM", 0x04); // $2104
        push("INIDISP", 0x00); // $2100
        push("VMAIN", 0x15); // $2115
        push("VMADDL", 0x16); // $2116
        push("VMADDH", 0x17); // $2117
        push("VMDATAL", 0x18); // $2118
        push("VMDATAH", 0x19); // $2119
        push("CGADD", 0x21); // $2121
        push("CGDATA", 0x22); // $2122
        push("TM", 0x2C); // $212C
                          // WRAM port
        push("WRAM", 0x80); // $2180
                            // Any others with counts
        for (i, &n) in self.dma_dest_hist.iter().enumerate() {
            let i_u8 = i as u8;
            if n > 0
                && !matches!(
                    i_u8,
                    0x04 | 0x00 | 0x15 | 0x16 | 0x17 | 0x18 | 0x19 | 0x21 | 0x22 | 0x2C | 0x80
                )
            {
                parts.push(format!("$21{:02X}:{}", i_u8, n));
            }
        }
        // reset
        self.dma_dest_hist.fill(0);
        if parts.is_empty() {
            "DMA dests: none".to_string()
        } else {
            format!("DMA dests: {}", parts.join(", "))
        }
    }

    // Summarize HDMA activity since last call; resets counters.
    pub fn take_hdma_summary(&mut self) -> String {
        let lines = self.hdma_lines_executed;
        let vram = self.hdma_bytes_vram;
        let cgram = self.hdma_bytes_cgram;
        let oam = self.hdma_bytes_oam;
        let win = self.hdma_bytes_window;
        self.hdma_lines_executed = 0;
        self.hdma_bytes_vram = 0;
        self.hdma_bytes_cgram = 0;
        self.hdma_bytes_oam = 0;
        self.hdma_bytes_window = 0;
        if lines == 0 && vram == 0 && cgram == 0 && oam == 0 && win == 0 {
            "HDMA: none".to_string()
        } else {
            format!(
                "HDMA: lines={} VRAM={} CGRAM={} OAM={} WIN={}",
                lines, vram, cgram, oam, win
            )
        }
    }

    pub(super) fn partition_mdma_mask_for_current_window(
        &self,
        value: u8,
        strict: bool,
    ) -> (u8, u8) {
        let mut now_mask = value;
        let mut defer_mask = 0u8;
        if strict {
            for i in 0..8u8 {
                if value & (1 << i) == 0 {
                    continue;
                }
                let ch = &self.dma_controller.channels[i as usize];
                let dest = ch.dest_address & 0x7F;
                let is_vram = dest == 0x18 || dest == 0x19;
                let is_cgram = dest == 0x22;
                let is_oam = dest == 0x04;
                let safe = if is_vram {
                    self.ppu.can_write_vram_non_hdma_now()
                } else if is_cgram {
                    self.ppu.can_write_cgram_non_hdma_now()
                } else if is_oam {
                    self.ppu.can_write_oam_non_hdma_now()
                } else {
                    true
                };
                if !safe {
                    defer_mask |= 1 << i;
                    now_mask &= !(1 << i);
                }
            }
        }
        (now_mask, defer_mask)
    }
}
