// SNES Input Controller implementation

use std::sync::OnceLock;

#[derive(Debug, Clone, Default)]
pub struct SnesController {
    // コントローラーの状態（ビットフィールド）
    buttons: u16,
    // 強制的に押下したことにするボタン（デバッグ/ヘッドレス用）
    auto_buttons: u16,
    // シフトレジスタ（読み取り用）
    shift_register: u16,
    // ラッチされた状態
    latched_buttons: u16,
    // ストローブ状態
    strobe: bool,
    // 接続状態（未接続ポートはデータライン=Highで1を返す）
    connected: bool,
}

// SNES コントローラーのボタン定義
#[allow(dead_code)]
pub mod button {
    pub const B: u16 = 0x0001;
    pub const Y: u16 = 0x0002;
    pub const SELECT: u16 = 0x0004;
    pub const START: u16 = 0x0008;
    pub const UP: u16 = 0x0010;
    pub const DOWN: u16 = 0x0020;
    pub const LEFT: u16 = 0x0040;
    pub const RIGHT: u16 = 0x0080;
    pub const A: u16 = 0x0100;
    pub const X: u16 = 0x0200;
    pub const L: u16 = 0x0400;
    pub const R: u16 = 0x0800;
}

impl SnesController {
    pub fn new() -> Self {
        Self {
            buttons: 0,
            auto_buttons: 0,
            // Power-on: no buttons pressed.
            // SNES joypad report bits are treated as "1=pressed" in most docs.
            // Some ROMs read $4016/$4017 before any strobe edge, so keep this sane.
            shift_register: 0x0000,
            latched_buttons: 0x0000,
            strobe: false,
            connected: false,
        }
    }

    /// SNES の $4016/$4017 および $4218-$421F の 16-bit 出力形式に変換した値を返す。
    /// 1 = 押下, 0 = 未押下
    ///
    /// 注意: SNES の自動ジョイパッド読み取り/シリアル読み取りは MSB(Bボタン) から出力される。
    /// よって本関数は以下のビット配置で返す:
    ///   bit15..0 = B,Y,Select,Start,Up,Down,Left,Right,A,X,L,R,0,0,0,0
    #[inline]
    pub fn active_low_bits(&self) -> u16 {
        let combined = self.buttons | self.auto_buttons;
        let b = ((combined & button::B) != 0) as u16;
        let y = ((combined & button::Y) != 0) as u16;
        let select = ((combined & button::SELECT) != 0) as u16;
        let start = ((combined & button::START) != 0) as u16;
        let up = ((combined & button::UP) != 0) as u16;
        let down = ((combined & button::DOWN) != 0) as u16;
        let left = ((combined & button::LEFT) != 0) as u16;
        let right = ((combined & button::RIGHT) != 0) as u16;
        let a = ((combined & button::A) != 0) as u16;
        let x = ((combined & button::X) != 0) as u16;
        let l = ((combined & button::L) != 0) as u16;
        let r = ((combined & button::R) != 0) as u16;

        // Standard controller signature bits are 0000.
        (b << 15)
            | (y << 14)
            | (select << 13)
            | (start << 12)
            | (up << 11)
            | (down << 10)
            | (left << 9)
            | (right << 8)
            | (a << 7)
            | (x << 6)
            | (l << 5)
            | (r << 4)
    }

    // ボタン状態を設定
    #[allow(dead_code)]
    pub fn set_button(&mut self, button: u16, pressed: bool) {
        if pressed {
            self.buttons |= button;
        } else {
            self.buttons &= !button;
        }
    }

    // 複数ボタンの状態を一度に設定
    pub fn set_buttons(&mut self, buttons: u16) {
        self.buttons = buttons;
    }

    #[allow(dead_code)]
    pub fn set_auto_buttons(&mut self, mask: u16) {
        self.auto_buttons = mask;
        // strobe High 時には shift_register を即更新するため、再ラッチしておく
        if self.strobe {
            self.latch_buttons();
        }
    }

    // 現在のボタン状態を取得
    #[allow(dead_code)]
    pub fn get_buttons(&self) -> u16 {
        self.buttons
    }

    // ストローブ書き込み（$4016）
    pub fn write_strobe(&mut self, value: u8) {
        let new_strobe = value & 0x01 != 0;

        // ストローブがHigh->Lowに変わった時にボタン状態をラッチ
        if self.strobe && !new_strobe {
            self.latch_buttons();
        }

        self.strobe = new_strobe;

        // ストローブがHigh の間はシフトレジスタをリセット
        if self.strobe {
            self.shift_register = self.latched_buttons;
        }
    }

    // データ読み取り（$4016/$4017）
    pub fn read_data(&mut self) -> u8 {
        if !self.connected {
            return 1; // 未接続ポートはデータラインがプルアップで常に1
        }
        if self.strobe {
            // ストローブ中は常に最初のビット（B）の状態を返す。
            let combined = self.buttons | self.auto_buttons;
            ((combined & button::B) != 0) as u8
        } else {
            // シフトレジスタから1ビットずつ読み出し（MSBから: Bが最初）。
            let result = ((self.shift_register & 0x8000) != 0) as u8;
            // 読み切った後は1を返す（公式コントローラは1を返す）
            self.shift_register = (self.shift_register << 1) | 1;
            result
        }
    }

    fn latch_buttons(&mut self) {
        // ボタンの読み出し順序に合わせて並び替え
        // SNESの読み出し順序（MSB→LSB）:
        //   B, Y, Select, Start, Up, Down, Left, Right, A, X, L, R, 0, 0, 0, 0
        // 実機出力に合わせて並べた 1=Pressed のビット列
        self.latched_buttons = self.active_low_bits();
        self.shift_register = self.latched_buttons;
    }

    // デバッグ用：ボタン状態を文字列で表示
    #[allow(dead_code)]
    pub fn debug_buttons(&self) -> String {
        let mut result = String::new();

        if self.buttons & button::B != 0 {
            result.push_str("B ");
        }
        if self.buttons & button::Y != 0 {
            result.push_str("Y ");
        }
        if self.buttons & button::SELECT != 0 {
            result.push_str("Select ");
        }
        if self.buttons & button::START != 0 {
            result.push_str("Start ");
        }
        if self.buttons & button::UP != 0 {
            result.push_str("Up ");
        }
        if self.buttons & button::DOWN != 0 {
            result.push_str("Down ");
        }
        if self.buttons & button::LEFT != 0 {
            result.push_str("Left ");
        }
        if self.buttons & button::RIGHT != 0 {
            result.push_str("Right ");
        }
        if self.buttons & button::A != 0 {
            result.push_str("A ");
        }
        if self.buttons & button::X != 0 {
            result.push_str("X ");
        }
        if self.buttons & button::L != 0 {
            result.push_str("L ");
        }
        if self.buttons & button::R != 0 {
            result.push_str("R ");
        }

        if result.is_empty() {
            "None".to_string()
        } else {
            result
        }
    }
}

// 入力システム全体を管理
#[derive(Debug)]
pub struct InputSystem {
    pub controller1: SnesController,
    pub controller2: SnesController,
    pub controller3: SnesController,
    pub controller4: SnesController,
    multitap_enabled: bool,
}

impl InputSystem {
    pub fn new() -> Self {
        let mut controller1 = SnesController::new();
        controller1.connected = true; // ポート1は常に接続
        let mut controller2 = SnesController::new();
        controller2.connected = true; // 互換性のためポート2も標準パッド接続を既定とする
        Self {
            controller1,
            controller2,
            controller3: SnesController::new(),
            controller4: SnesController::new(),
            multitap_enabled: false,
        }
    }

    // コントローラー1の読み取り
    pub fn read_controller1(&mut self) -> u8 {
        self.controller1.read_data()
    }

    // コントローラー2の読み取り
    pub fn read_controller2(&mut self) -> u8 {
        self.controller2.read_data()
    }

    // For future multitap direct reads
    #[allow(dead_code)]
    pub fn read_controller3(&mut self) -> u8 {
        self.controller3.read_data()
    }
    #[allow(dead_code)]
    pub fn read_controller4(&mut self) -> u8 {
        self.controller4.read_data()
    }

    // ストローブ書き込み（両方のコントローラーに適用）
    pub fn write_strobe(&mut self, value: u8) {
        self.controller1.write_strobe(value);
        self.controller2.write_strobe(value);
        self.controller3.write_strobe(value);
        self.controller4.write_strobe(value);
    }

    // 外部からのキー入力を処理
    pub fn handle_key_input(&mut self, key_states: &KeyStates) {
        let mut buttons = 0u16;

        if key_states.up {
            buttons |= button::UP;
        }
        if key_states.down {
            buttons |= button::DOWN;
        }
        if key_states.left {
            buttons |= button::LEFT;
        }
        if key_states.right {
            buttons |= button::RIGHT;
        }
        if key_states.a {
            buttons |= button::A;
        }
        if key_states.b {
            buttons |= button::B;
        }
        if key_states.x {
            buttons |= button::X;
        }
        if key_states.y {
            buttons |= button::Y;
        }
        if key_states.l {
            buttons |= button::L;
        }
        if key_states.r {
            buttons |= button::R;
        }
        if key_states.start {
            buttons |= button::START;
        }
        if key_states.select {
            buttons |= button::SELECT;
        }

        self.controller1.set_buttons(buttons);
    }

    pub fn set_multitap_enabled(&mut self, enabled: bool) {
        self.multitap_enabled = enabled;
    }
    pub fn is_multitap_enabled(&self) -> bool {
        self.multitap_enabled
    }

    #[allow(dead_code)]
    pub fn controller3_buttons(&self) -> u16 {
        self.controller3.get_buttons()
    }
    #[allow(dead_code)]
    pub fn controller4_buttons(&self) -> u16 {
        self.controller4.get_buttons()
    }

    #[allow(dead_code)]
    pub fn controller3_active_low(&self) -> u16 {
        self.controller3.active_low_bits()
    }

    #[allow(dead_code)]
    pub fn controller4_active_low(&self) -> u16 {
        self.controller4.active_low_bits()
    }
}

// --- Save state helpers ---
impl InputSystem {
    pub fn to_save_state(&self) -> crate::savestate::InputSaveState {
        use crate::savestate::InputSaveState;
        InputSaveState {
            controller1_buttons: self.controller1.buttons,
            controller2_buttons: self.controller2.buttons,
            controller3_buttons: self.controller3.buttons,
            controller4_buttons: self.controller4.buttons,
            controller1_shift_register: self.controller1.shift_register,
            controller2_shift_register: self.controller2.shift_register,
            controller3_shift_register: self.controller3.shift_register,
            controller4_shift_register: self.controller4.shift_register,
            controller1_latched_buttons: self.controller1.latched_buttons,
            controller2_latched_buttons: self.controller2.latched_buttons,
            controller3_latched_buttons: self.controller3.latched_buttons,
            controller4_latched_buttons: self.controller4.latched_buttons,
            strobe: self.controller1.strobe || self.controller2.strobe,
            multitap_enabled: self.multitap_enabled,
            controller1_connected: self.controller1.connected,
            controller2_connected: self.controller2.connected,
        }
    }

    pub fn load_from_save_state(&mut self, st: &crate::savestate::InputSaveState) {
        self.controller1.buttons = st.controller1_buttons;
        self.controller2.buttons = st.controller2_buttons;
        self.controller3.buttons = st.controller3_buttons;
        self.controller4.buttons = st.controller4_buttons;
        self.controller1.shift_register = st.controller1_shift_register;
        self.controller2.shift_register = st.controller2_shift_register;
        self.controller3.shift_register = st.controller3_shift_register;
        self.controller4.shift_register = st.controller4_shift_register;
        self.controller1.latched_buttons = st.controller1_latched_buttons;
        self.controller2.latched_buttons = st.controller2_latched_buttons;
        self.controller3.latched_buttons = st.controller3_latched_buttons;
        self.controller4.latched_buttons = st.controller4_latched_buttons;
        self.controller1.strobe = st.strobe;
        self.controller2.strobe = st.strobe;
        self.controller3.strobe = st.strobe;
        self.controller4.strobe = st.strobe;
        self.multitap_enabled = st.multitap_enabled;
        self.controller1.connected = st.controller1_connected;
        self.controller2.connected = st.controller2_connected;
    }
}

// キーボード状態を表現する構造体
#[derive(Debug, Default)]
pub struct KeyStates {
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
    pub a: bool,
    pub b: bool,
    pub x: bool,
    pub y: bool,
    pub l: bool,
    pub r: bool,
    pub start: bool,
    pub select: bool,
}

// --- Scripted input (opt-in; used by tools/headless runs) ---

#[derive(Clone, Copy)]
struct ScriptedInputEvent {
    start: u64,
    end: u64,
    mask: u16,
}

static SCRIPTED_INPUT: OnceLock<Option<Vec<ScriptedInputEvent>>> = OnceLock::new();

#[allow(dead_code)]
pub fn install_scripted_input_events(spec: &str) -> Result<(), String> {
    fn parse_buttons(spec: &str) -> u16 {
        spec.split([',', '+', '|'])
            .filter_map(|name| match name.trim().to_uppercase().as_str() {
                "A" => Some(button::A),
                "B" => Some(button::B),
                "X" => Some(button::X),
                "Y" => Some(button::Y),
                "L" => Some(button::L),
                "R" => Some(button::R),
                "START" => Some(button::START),
                "SELECT" => Some(button::SELECT),
                "UP" => Some(button::UP),
                "DOWN" => Some(button::DOWN),
                "LEFT" => Some(button::LEFT),
                "RIGHT" => Some(button::RIGHT),
                _ => None,
            })
            .fold(0u16, |acc, v| acc | v)
    }

    fn parse_range(spec: &str) -> Option<(u64, u64)> {
        let s = spec.trim();
        if s.is_empty() {
            return None;
        }
        if let Some((a, b)) = s.split_once('-') {
            let start = a.trim().parse::<u64>().ok()?;
            let end = b.trim().parse::<u64>().ok()?;
            Some((start, end))
        } else {
            let t = s.parse::<u64>().ok()?;
            Some((t, t))
        }
    }

    let spec = spec.trim();
    if spec.is_empty() {
        return Err("input events spec is empty".to_string());
    }

    let mut events: Vec<ScriptedInputEvent> = Vec::new();
    for ent in spec.split(';') {
        let ent = ent.trim();
        if ent.is_empty() {
            continue;
        }
        let (range_s, buttons_s) = ent
            .split_once(':')
            .map(|(r, b)| (r.trim(), b.trim()))
            .unwrap_or((ent, "START"));
        let (start, end) = parse_range(range_s)
            .ok_or_else(|| format!("invalid range in input event: '{}'", ent))?;
        let mask = parse_buttons(buttons_s);
        if mask == 0 {
            return Err(format!("invalid buttons in input event: '{}'", ent));
        }
        events.push(ScriptedInputEvent { start, end, mask });
    }

    if events.is_empty() {
        return Err("no valid input events parsed".to_string());
    }

    SCRIPTED_INPUT
        .set(Some(events))
        .map_err(|_| "scripted input already installed".to_string())?;
    Ok(())
}

pub fn scripted_input_mask_for_frame(frame: u64) -> u16 {
    let Some(events) = SCRIPTED_INPUT.get().and_then(|v| v.as_ref()) else {
        return 0;
    };
    let mut mask: u16 = 0;
    for e in events {
        if frame >= e.start && frame <= e.end {
            mask |= e.mask;
        }
    }
    mask
}
