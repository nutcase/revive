use super::Emulator;

use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

static RENDER_DEBUG_COUNT: AtomicU32 = AtomicU32::new(0);

struct RenderTimings {
    enabled: bool,
    start: Option<Instant>,
    copy_time: Duration,
}

impl RenderTimings {
    fn from_env() -> Self {
        let enabled = std::env::var("PERF_RENDER")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        Self {
            enabled,
            start: enabled.then(Instant::now),
            copy_time: Duration::ZERO,
        }
    }

    fn begin_step(&self) -> Option<Instant> {
        self.enabled.then(Instant::now)
    }

    fn add_copy_time(&mut self, start: Option<Instant>) {
        if let Some(start) = start {
            self.copy_time = self.copy_time.saturating_add(start.elapsed());
        }
    }
}

impl Emulator {
    pub(in crate::emulator) fn render(&mut self) {
        let mut timings = RenderTimings::from_env();
        let debug_seq = next_render_debug_seq();

        log_render_start(debug_seq);
        self.sync_superfx_direct_buffer();

        let all_black = self.render_frame(debug_seq, &mut timings);

        self.maybe_log_black_screen(all_black);
        self.record_render_timings(timings);
    }

    fn render_frame(&mut self, debug_seq: u32, timings: &mut RenderTimings) -> bool {
        {
            let framebuffer = self.bus.get_ppu().get_framebuffer();
            let len = self.frame_buffer.len().min(framebuffer.len());
            let copy_start = timings.begin_step();
            if len > 0 {
                self.frame_buffer[..len].copy_from_slice(&framebuffer[..len]);
            }
            timings.add_copy_time(copy_start);

            if len < self.frame_buffer.len() {
                self.frame_buffer[len..].fill(0xFF000000);
            }
        }

        log_framebuffer_snapshot("copied", debug_seq, &self.frame_buffer);
        framebuffer_all_black(&self.frame_buffer)
    }

    fn record_render_timings(&mut self, timings: RenderTimings) {
        if !timings.enabled {
            return;
        }

        if let Some(start) = timings.start {
            self.performance_stats.add_render_time(start.elapsed());
            if timings.copy_time != Duration::ZERO {
                self.performance_stats.add_copy_time(timings.copy_time);
            }
        }
    }

    fn maybe_log_black_screen(&mut self, all_black: bool) {
        let enabled = std::env::var("DEBUG_BLACK_SCREEN")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        if !enabled {
            return;
        }

        let threshold = std::env::var("DEBUG_BLACK_SCREEN_FRAMES")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(60);
        if all_black {
            self.black_screen_streak = self.black_screen_streak.saturating_add(1);
            if !self.black_screen_reported && self.black_screen_streak >= threshold {
                self.black_screen_reported = true;
                println!(
                    "[BLACK_SCREEN] frame={} streak={} threshold={}",
                    self.frame_count, self.black_screen_streak, threshold
                );
                self.bus.get_ppu().debug_ppu_state();
            }
        } else {
            self.black_screen_streak = 0;
            self.black_screen_reported = false;
        }
    }
}

fn next_render_debug_seq() -> u32 {
    RENDER_DEBUG_COUNT
        .fetch_add(1, Ordering::Relaxed)
        .wrapping_add(1)
}

fn framebuffer_all_black(framebuffer: &[u32]) -> bool {
    framebuffer.iter().all(|&px| is_black_pixel(px))
}

fn is_black_pixel(pixel: u32) -> bool {
    pixel == 0xFF000000 || pixel == 0x00000000
}

fn log_render_start(debug_seq: u32) {
    if render_verbose_limited(debug_seq) {
        println!("[RENDER] seq={} start", debug_seq);
    }
}

fn log_framebuffer_snapshot(source: &str, debug_seq: u32, framebuffer: &[u32]) {
    if !render_verbose_limited(debug_seq) {
        return;
    }

    let non_black = framebuffer
        .iter()
        .filter(|&&px| !is_black_pixel(px))
        .count();
    let white = framebuffer
        .iter()
        .filter(|&&px| px == 0xFFFFFFFF || (px & 0x00FFFFFF) == 0x00FFFFFF)
        .count();
    println!(
        "[RENDER] seq={} source={} pixels={} non_black={} white={} sample0=0x{:08X} sample128=0x{:08X} sample256=0x{:08X}",
        debug_seq,
        source,
        framebuffer.len(),
        non_black,
        white,
        sample_at(framebuffer, 0),
        sample_at(framebuffer, 128),
        sample_at(framebuffer, 256)
    );
}

fn render_verbose_limited(debug_seq: u32) -> bool {
    crate::debug_flags::render_verbose() && debug_seq <= 5
}

fn sample_at(framebuffer: &[u32], index: usize) -> u32 {
    framebuffer.get(index).copied().unwrap_or(0)
}
