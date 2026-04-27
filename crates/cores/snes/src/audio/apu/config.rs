const DEFAULT_APU_CYCLE_SCALE: f64 = 0.572_139_701_913_725_3;
const DEFAULT_APU_SAMPLE_RATE: u32 = 32000;
const DEFAULT_FAST_UPLOAD_BYTES: u64 = 0x10000;
const DEFAULT_APU_OUTPUT_TARGET_SAMPLES: i32 = 2048;

/// Convert an f64 fractional scale (0..1 range typical) to Q0.32 fixed-point.
#[inline]
pub(super) fn f64_to_fixed32(v: f64) -> u64 {
    (v * (1u64 << 32) as f64) as u64
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ApuConfig {
    pub(super) sample_rate: u32,
    pub(super) boot_hle_enabled: bool,
    pub(super) fast_upload: bool,
    pub(super) fast_upload_bytes: u64,
    pub(super) skip_boot: bool,
    pub(super) fake_upload: bool,
    pub(super) loader_hle_enabled: bool,
    pub(super) force_port0: Option<u8>,
    pub(super) smw_apu_echo: bool,
    pub(super) smw_apu_hle_handshake: bool,
    pub(super) smw_apu_port_echo_strict: bool,
    pub(super) cycle_scale_fixed: u64,
    pub(super) cycle_scale_master_fixed: u64,
    pub(super) output_buffer_target_samples: i32,
}

impl ApuConfig {
    pub(super) fn from_env() -> Self {
        let (cycle_scale_fixed, cycle_scale_master_fixed) = Self::read_cycle_scale_fixed();
        Self {
            sample_rate: DEFAULT_APU_SAMPLE_RATE,
            // デフォルト: 実IPL（正確性優先）。必要なら APU_BOOT_HLE=1 でHLE有効化。
            boot_hle_enabled: Self::read_strict_bool_env("APU_BOOT_HLE", false),
            // 正確さ優先: デフォルトではフルサイズ転送を行う。
            // 速さが欲しい場合のみ APU_FAST_UPLOAD=1 を明示する。
            fast_upload: Self::read_loose_bool_env("APU_FAST_UPLOAD", false),
            fast_upload_bytes: Self::read_u64_env("APU_FAST_BYTES", DEFAULT_FAST_UPLOAD_BYTES),
            skip_boot: Self::read_loose_bool_env("APU_SKIP_BOOT", false),
            fake_upload: Self::read_loose_bool_env("APU_FAKE_UPLOAD", false),
            // Post-boot upload HLE bridges IPL-style loaders that are invoked
            // by an already-running SPC program. It can be disabled for
            // diagnostics when comparing against the real loader byte stream.
            loader_hle_enabled: Self::read_loose_bool_env("APU_LOADER_HLE", true),
            force_port0: Self::read_u8_env("APU_FORCE_PORT0"),
            smw_apu_echo: Self::read_loose_bool_env("SMW_APU_ECHO", false),
            // SMW専用。既定では無効（他ROMへの副作用回避）
            smw_apu_hle_handshake: Self::read_loose_bool_env("SMW_APU_HLE_HANDSHAKE", false),
            smw_apu_port_echo_strict: Self::read_loose_bool_env("SMW_APU_PORT_ECHO_STRICT", false),
            cycle_scale_fixed,
            cycle_scale_master_fixed,
            output_buffer_target_samples: Self::read_i32_env(
                "APU_OUTPUT_TARGET_SAMPLES",
                DEFAULT_APU_OUTPUT_TARGET_SAMPLES,
            ),
        }
    }

    pub(super) fn from_reset_env(
        sample_rate: u32,
        boot_hle_enabled: bool,
        fake_upload: bool,
        force_port0: Option<u8>,
        smw_apu_echo_default: bool,
        smw_apu_port_echo_strict_default: bool,
    ) -> Self {
        let base = Self::from_env();
        Self {
            sample_rate,
            boot_hle_enabled,
            fake_upload,
            force_port0,
            smw_apu_echo: Self::read_loose_bool_env("SMW_APU_ECHO", smw_apu_echo_default),
            smw_apu_hle_handshake: Self::read_loose_bool_env("SMW_APU_HLE_HANDSHAKE", false),
            smw_apu_port_echo_strict: Self::read_loose_bool_env(
                "SMW_APU_PORT_ECHO_STRICT",
                smw_apu_port_echo_strict_default,
            ),
            ..base
        }
    }

    pub(super) fn initial_boot_state(self) -> super::BootState {
        if self.skip_boot || !self.boot_hle_enabled {
            super::BootState::Running
        } else {
            super::BootState::ReadySignature
        }
    }

    fn read_strict_bool_env(name: &str, default: bool) -> bool {
        std::env::var(name)
            .map(|v| v == "1" || v.to_lowercase() == "true")
            .unwrap_or(default)
    }

    fn read_loose_bool_env(name: &str, default: bool) -> bool {
        std::env::var(name)
            .map(|v| v != "0" && v.to_lowercase() != "false")
            .unwrap_or(default)
    }

    fn read_u64_env(name: &str, default: u64) -> u64 {
        std::env::var(name)
            .ok()
            .and_then(|v| Self::parse_hex_or_decimal_u64(&v))
            .unwrap_or(default)
    }

    fn read_u8_env(name: &str) -> Option<u8> {
        std::env::var(name)
            .ok()
            .and_then(|v| Self::parse_hex_or_decimal_u8(&v))
    }

    fn read_i32_env(name: &str, default: i32) -> i32 {
        std::env::var(name)
            .ok()
            .and_then(|v| v.parse::<i32>().ok())
            .filter(|&v| v > 0)
            .unwrap_or(default)
    }

    fn parse_hex_or_decimal_u64(value: &str) -> Option<u64> {
        u64::from_str_radix(value.trim_start_matches("0x"), 16)
            .ok()
            .or_else(|| value.parse().ok())
    }

    fn parse_hex_or_decimal_u8(value: &str) -> Option<u8> {
        u8::from_str_radix(value.trim_start_matches("0x"), 16)
            .ok()
            .or_else(|| value.parse().ok())
    }

    fn read_cycle_scale_f64() -> f64 {
        std::env::var("APU_CYCLE_SCALE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_APU_CYCLE_SCALE)
    }

    pub(super) fn read_cycle_scale_fixed() -> (u64, u64) {
        let scale = Self::read_cycle_scale_f64();
        let fixed = f64_to_fixed32(scale);
        let master = f64_to_fixed32(scale / 6.0);
        (fixed, master)
    }
}
