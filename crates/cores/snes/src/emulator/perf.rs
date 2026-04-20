use super::Emulator;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct PerformanceStats {
    pub(in crate::emulator) fps: f64,
    pub(in crate::emulator) frame_time_avg: Duration,
    pub(in crate::emulator) frame_time_min: Duration,
    pub(in crate::emulator) frame_time_max: Duration,
    #[allow(dead_code)]
    pub(in crate::emulator) cpu_usage: f64,
    pub(in crate::emulator) dropped_frames: u64,
    pub(in crate::emulator) total_frames: u64,
    pub(in crate::emulator) last_fps_update: Instant,
    pub(in crate::emulator) dropped_frames_last_second: u64,
    pub(in crate::emulator) last_dropped_frames: u64,
    pub(in crate::emulator) frame_times: Vec<Duration>,
    // Component-level timing
    pub(in crate::emulator) cpu_time_total: Duration,
    pub(in crate::emulator) ppu_time_total: Duration,
    pub(in crate::emulator) dma_time_total: Duration,
    pub(in crate::emulator) sa1_time_total: Duration,
    pub(in crate::emulator) apu_time_total: Duration,
    pub(in crate::emulator) sync_time_total: Duration,
    pub(in crate::emulator) input_time_total: Duration,
    pub(in crate::emulator) render_time_total: Duration,
    pub(in crate::emulator) copy_time_total: Duration,
    // Timing samples for current second
    pub(in crate::emulator) cpu_time_samples: Vec<Duration>,
    pub(in crate::emulator) ppu_time_samples: Vec<Duration>,
    pub(in crate::emulator) dma_time_samples: Vec<Duration>,
    pub(in crate::emulator) sa1_time_samples: Vec<Duration>,
    pub(in crate::emulator) apu_time_samples: Vec<Duration>,
    pub(in crate::emulator) sync_time_samples: Vec<Duration>,
    pub(in crate::emulator) input_time_samples: Vec<Duration>,
    pub(in crate::emulator) render_time_samples: Vec<Duration>,
    pub(in crate::emulator) copy_time_samples: Vec<Duration>,
}

impl PerformanceStats {
    pub(in crate::emulator) fn new() -> Self {
        Self {
            fps: 60.0,
            frame_time_avg: Duration::from_secs_f64(1.0 / 60.0),
            frame_time_min: Duration::from_secs_f64(1.0 / 60.0),
            frame_time_max: Duration::from_secs_f64(1.0 / 60.0),
            cpu_usage: 0.0,
            dropped_frames: 0,
            total_frames: 0,
            last_fps_update: Instant::now(),
            dropped_frames_last_second: 0,
            last_dropped_frames: 0,
            frame_times: Vec::with_capacity(60),
            cpu_time_total: Duration::ZERO,
            ppu_time_total: Duration::ZERO,
            dma_time_total: Duration::ZERO,
            sa1_time_total: Duration::ZERO,
            apu_time_total: Duration::ZERO,
            sync_time_total: Duration::ZERO,
            input_time_total: Duration::ZERO,
            render_time_total: Duration::ZERO,
            copy_time_total: Duration::ZERO,
            cpu_time_samples: Vec::with_capacity(60),
            ppu_time_samples: Vec::with_capacity(60),
            dma_time_samples: Vec::with_capacity(60),
            sa1_time_samples: Vec::with_capacity(60),
            apu_time_samples: Vec::with_capacity(60),
            sync_time_samples: Vec::with_capacity(60),
            input_time_samples: Vec::with_capacity(60),
            render_time_samples: Vec::with_capacity(60),
            copy_time_samples: Vec::with_capacity(60),
        }
    }

    pub(in crate::emulator) fn update(&mut self, frame_time: Duration) -> bool {
        let mut fps_updated = false;
        self.total_frames += 1;
        self.frame_times.push(frame_time);

        // Keep only the last 60 frame times for averaging
        if self.frame_times.len() > 60 {
            self.frame_times.remove(0);
        }

        let now = Instant::now();
        if now.duration_since(self.last_fps_update) >= Duration::from_secs(1) {
            // Calculate FPS and average frame time
            if !self.frame_times.is_empty() {
                let total_time: Duration = self.frame_times.iter().sum();
                self.frame_time_avg = total_time / self.frame_times.len() as u32;
                self.fps = 1.0 / self.frame_time_avg.as_secs_f64();
                // Keep min/max coherent with the same rolling window as avg/fps.
                if let Some(min_ft) = self.frame_times.iter().copied().min() {
                    self.frame_time_min = min_ft;
                }
                if let Some(max_ft) = self.frame_times.iter().copied().max() {
                    self.frame_time_max = max_ft;
                }
            }

            // Clear component timing samples for next second
            self.cpu_time_samples.clear();
            self.ppu_time_samples.clear();
            self.dma_time_samples.clear();
            self.sa1_time_samples.clear();
            self.apu_time_samples.clear();
            self.sync_time_samples.clear();
            self.input_time_samples.clear();
            self.render_time_samples.clear();
            self.copy_time_samples.clear();

            self.dropped_frames_last_second = self.dropped_frames - self.last_dropped_frames;
            self.last_dropped_frames = self.dropped_frames;
            self.last_fps_update = now;
            fps_updated = true;
        }
        fps_updated
    }

    pub(in crate::emulator) fn add_cpu_time(&mut self, time: Duration) {
        self.cpu_time_total += time;
        self.cpu_time_samples.push(time);
    }

    pub(in crate::emulator) fn add_ppu_time(&mut self, time: Duration) {
        self.ppu_time_total += time;
        self.ppu_time_samples.push(time);
    }

    #[allow(dead_code)]
    pub(in crate::emulator) fn add_dma_time(&mut self, time: Duration) {
        self.dma_time_total += time;
        self.dma_time_samples.push(time);
    }

    pub(in crate::emulator) fn add_sa1_time(&mut self, time: Duration) {
        self.sa1_time_total += time;
        self.sa1_time_samples.push(time);
    }

    pub(in crate::emulator) fn add_apu_time(&mut self, time: Duration) {
        self.apu_time_total += time;
        self.apu_time_samples.push(time);
    }

    pub(in crate::emulator) fn add_sync_time(&mut self, time: Duration) {
        self.sync_time_total += time;
        self.sync_time_samples.push(time);
    }

    pub(in crate::emulator) fn add_input_time(&mut self, time: Duration) {
        self.input_time_total += time;
        self.input_time_samples.push(time);
    }

    pub(in crate::emulator) fn add_render_time(&mut self, time: Duration) {
        self.render_time_total += time;
        self.render_time_samples.push(time);
    }

    pub(in crate::emulator) fn add_copy_time(&mut self, time: Duration) {
        self.copy_time_total += time;
        self.copy_time_samples.push(time);
    }

    pub(in crate::emulator) fn should_skip_frame(
        &self,
        target_fps: f64,
        threshold_ratio: f64,
    ) -> bool {
        self.fps < target_fps * threshold_ratio
    }

    #[allow(dead_code)]
    pub(in crate::emulator) fn get_cpu_usage_percent(&self) -> f64 {
        self.cpu_usage * 100.0
    }
}

impl Emulator {
    pub(super) fn print_performance_stats(&self) {
        println!("╔═══════════════════════════════════════════╗");
        println!("║        Performance Statistics             ║");
        println!("╠═══════════════════════════════════════════╣");
        println!(
            "║ FPS: {:.1}                                  ║",
            self.performance_stats.fps
        );
        println!(
            "║ Frame Time: {:.2}ms (avg)                  ║",
            self.performance_stats.frame_time_avg.as_secs_f64() * 1000.0
        );
        println!(
            "║   Min: {:.2}ms  Max: {:.2}ms               ║",
            self.performance_stats.frame_time_min.as_secs_f64() * 1000.0,
            self.performance_stats.frame_time_max.as_secs_f64() * 1000.0
        );
        println!(
            "║ Dropped: {} / {} ({:.1}%)                  ║",
            self.performance_stats.dropped_frames,
            self.performance_stats.total_frames,
            (self.performance_stats.dropped_frames as f64
                / self.performance_stats.total_frames.max(1) as f64)
                * 100.0
        );

        // Show component timing if PERF_VERBOSE is enabled
        let verbose = std::env::var("PERF_VERBOSE").unwrap_or_default() == "1";
        if verbose && self.performance_stats.total_frames > 0 {
            println!("╠═══════════════════════════════════════════╣");
            println!("║ Component Timing (per frame avg)         ║");
            println!("╠═══════════════════════════════════════════╣");

            let frames = self.performance_stats.total_frames as f64;
            let cpu_avg = self.performance_stats.cpu_time_total.as_secs_f64() * 1000.0 / frames;
            let ppu_avg = self.performance_stats.ppu_time_total.as_secs_f64() * 1000.0 / frames;
            let dma_avg = self.performance_stats.dma_time_total.as_secs_f64() * 1000.0 / frames;
            let sa1_avg = self.performance_stats.sa1_time_total.as_secs_f64() * 1000.0 / frames;
            let apu_avg = self.performance_stats.apu_time_total.as_secs_f64() * 1000.0 / frames;
            let sync_avg = self.performance_stats.sync_time_total.as_secs_f64() * 1000.0 / frames;
            let input_avg = self.performance_stats.input_time_total.as_secs_f64() * 1000.0 / frames;

            println!("║ CPU:  {:.3}ms                             ║", cpu_avg);
            println!("║ PPU:  {:.3}ms                             ║", ppu_avg);
            println!("║ DMA:  {:.3}ms                             ║", dma_avg);
            println!("║ SA-1: {:.3}ms                             ║", sa1_avg);
            println!("║ APU:  {:.3}ms                             ║", apu_avg);
            println!("║ Sync: {:.3}ms                             ║", sync_avg);
            println!("║ Input: {:.3}ms                            ║", input_avg);

            let frame_avg = self.performance_stats.frame_time_avg.as_secs_f64() * 1000.0;
            let perf_render = std::env::var("PERF_RENDER")
                .map(|v| v == "1" || v.to_lowercase() == "true")
                .unwrap_or(false);
            let mut total_component =
                cpu_avg + ppu_avg + dma_avg + sa1_avg + apu_avg + sync_avg + input_avg;
            let mut render_avg = 0.0;
            let mut copy_avg = 0.0;
            if perf_render {
                render_avg =
                    self.performance_stats.render_time_total.as_secs_f64() * 1000.0 / frames;
                copy_avg = self.performance_stats.copy_time_total.as_secs_f64() * 1000.0 / frames;
                total_component += render_avg + copy_avg;
            }
            let other = (frame_avg - total_component).max(0.0);
            println!("║ Other:{:.3}ms                             ║", other);

            if perf_render {
                println!("║ Render:{:.3}ms                            ║", render_avg);
                println!("║ Copy:  {:.3}ms                            ║", copy_avg);
            }
        }

        println!("╠═══════════════════════════════════════════╣");
        println!(
            "║ Total Frames: {}                           ║",
            self.frame_count
        );
        println!(
            "║ Adaptive Timing: {}                         ║",
            if self.adaptive_timing { "ON " } else { "OFF" }
        );
        println!("╚═══════════════════════════════════════════╝");

        if !verbose {
            println!("(Set PERF_VERBOSE=1 for component-level timing)");
        }
    }

    // Performance optimization methods
    #[allow(dead_code)]
    pub fn set_frame_skip(&mut self, max_skip: u8) {
        self.max_frame_skip = max_skip.min(5); // Cap at 5 frames max
    }

    #[allow(dead_code)]
    pub fn set_adaptive_timing(&mut self, enabled: bool) {
        self.adaptive_timing = enabled;
        if enabled {
            println!("Adaptive timing enabled - frame skipping may occur for performance");
        } else {
            println!("Adaptive timing disabled - consistent frame rate with potential slowdown");
        }
    }

    #[allow(dead_code)]
    pub fn get_performance_stats(&self) -> &PerformanceStats {
        &self.performance_stats
    }

    // Optimized rendering with reduced frequency for performance
    #[allow(dead_code)]
    pub(super) fn should_render_frame(&self) -> bool {
        // Always render if adaptive timing is off
        if !self.adaptive_timing {
            return true;
        }

        // Skip rendering occasionally if performance is good
        if self.performance_stats.fps > 58.0 {
            self.frame_count.is_multiple_of(2) // Render every other frame when running well
        } else {
            true // Always render when struggling
        }
    }

    #[allow(dead_code)]
    /// Print current performance statistics immediately.
    pub fn print_performance_stats_now(&self) {
        self.print_performance_stats();
    }
}
