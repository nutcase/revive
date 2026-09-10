use std::time::{Duration, Instant};

const WINDOW: usize = 300;
const NAMES: [&str; 6] = [
    "core+cheats",
    "audio",
    "upload",
    "ui",
    "present",
    "frame_wait",
];

#[derive(Clone, Copy)]
pub(crate) enum Stage {
    Core,
    Audio,
    Upload,
    Ui,
    Present,
    Wait,
}

#[derive(Default)]
struct Samples {
    current: [Duration; 6],
    stages: [Vec<f64>; 6],
}

/// Opt-in CPU wall timing. Presentation includes surface/VSync waits; it is
/// deliberately not labelled GPU execution time. Disabled runs read no clocks.
pub(crate) struct FrameProfiler(Option<Box<Samples>>);

impl FrameProfiler {
    pub(crate) fn from_env() -> Self {
        Self(
            std::env::var("REVIVE_PERF")
                .ok()
                .filter(|value| value == "1")
                .map(|_| Box::new(Samples::default())),
        )
    }

    pub(crate) fn start(&self) -> Option<Instant> {
        self.0.as_ref().map(|_| Instant::now())
    }

    pub(crate) fn end(&mut self, stage: Stage, start: Option<Instant>) {
        if let (Some(samples), Some(start)) = (&mut self.0, start) {
            samples.current[stage as usize] += start.elapsed();
        }
    }

    pub(crate) fn finish_frame(&mut self) {
        let Some(samples) = self.0.as_mut() else {
            return;
        };
        for (history, elapsed) in samples.stages.iter_mut().zip(&mut samples.current) {
            history.push(elapsed.as_secs_f64() * 1000.0);
            *elapsed = Duration::ZERO;
        }
        if samples.stages[0].len() < WINDOW {
            return;
        }
        eprintln!("[revive-perf] last {WINDOW} frames, CPU wall ms (mean/p95):");
        for (name, history) in NAMES.iter().zip(&mut samples.stages) {
            let mean = history.iter().sum::<f64>() / history.len() as f64;
            history.sort_unstable_by(f64::total_cmp);
            eprintln!(
                "  {name}: {mean:.4}/{:.4}",
                history[(history.len() * 95).div_ceil(100) - 1]
            );
            history.clear();
        }
    }
}
