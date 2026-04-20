use super::Emulator;

impl Emulator {
    pub(super) fn apply_scripted_input_for_headless(&mut self) {
        if !self.headless {
            return;
        }
        let mask = crate::input::scripted_input_mask_for_frame(self.frame_count);
        self.bus
            .get_input_system_mut()
            .controller1
            .set_buttons(mask);
    }

    pub(super) fn maybe_quit_on_cpu_test_result(&mut self) {
        if !self.bus.is_cpu_test_mode() {
            return;
        }
        let Some(result) = self.bus.take_cpu_test_result() else {
            return;
        };
        match result {
            crate::bus::CpuTestResult::Pass { test_idx } => {
                println!("[CPUTEST] PASS (test_idx=0x{:04X})", test_idx);
                crate::shutdown::request_quit();
            }
            crate::bus::CpuTestResult::Fail { test_idx } => {
                println!("[CPUTEST] FAIL (test_idx=0x{:04X})", test_idx);
                crate::shutdown::request_quit_with_code(1);
            }
            crate::bus::CpuTestResult::InvalidOrder { test_idx } => {
                println!(
                    "[CPUTEST] FAIL (msg=\"Invalid test order\" test_idx=0x{:04X})",
                    test_idx
                );
                crate::shutdown::request_quit_with_code(1);
            }
        }
    }
}
