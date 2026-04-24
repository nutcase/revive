use crate::audio::Audio;

pub trait VdpDevice {
    fn step(&mut self, cycles: u32) -> bool;
    fn frame_buffer(&self) -> &[u8];
    fn vram(&self) -> &[u8];
    fn vram_mut(&mut self) -> &mut [u8];
    fn interrupt_enabled(&self) -> bool;
    fn frame_count(&self) -> u64;
}

pub trait InputDevice<Button> {
    fn set_button_pressed(&mut self, player: u8, button: Button, pressed: bool);
    fn read_port1(&self) -> u8;
    fn read_port2(&self) -> u8;
}

pub trait Sega8BusDevices {
    type Button;
    type Input: InputDevice<Self::Button>;
    type Vdp: VdpDevice;

    fn audio(&self) -> &Audio;
    fn audio_mut(&mut self) -> &mut Audio;
    fn input(&self) -> &Self::Input;
    fn input_mut(&mut self) -> &mut Self::Input;
    fn vdp(&self) -> &Self::Vdp;
    fn vdp_mut(&mut self) -> &mut Self::Vdp;

    fn step_devices(&mut self, cycles: u32) -> bool {
        self.audio_mut().step(cycles);
        self.vdp_mut().step(cycles)
    }

    fn device_frame_buffer(&self) -> &[u8] {
        self.vdp().frame_buffer()
    }

    fn device_vram(&self) -> &[u8] {
        self.vdp().vram()
    }

    fn device_vram_mut(&mut self) -> &mut [u8] {
        self.vdp_mut().vram_mut()
    }

    fn set_device_button_pressed(&mut self, player: u8, button: Self::Button, pressed: bool) {
        self.input_mut().set_button_pressed(player, button, pressed);
    }

    fn read_input_port1(&self) -> u8 {
        self.input().read_port1()
    }

    fn read_input_port2(&self) -> u8 {
        self.input().read_port2()
    }

    fn write_psg(&mut self, value: u8) {
        self.audio_mut().write_psg(value);
    }

    fn pending_audio_samples(&self) -> usize {
        self.audio().pending_samples()
    }

    fn drain_audio_samples(&mut self, max_samples: usize) -> Vec<i16> {
        self.audio_mut().drain_samples(max_samples)
    }

    fn set_audio_output_sample_rate_hz(&mut self, hz: u32) {
        self.audio_mut().set_output_sample_rate_hz(hz);
    }

    fn audio_output_channels(&self) -> u8 {
        self.audio().output_channels()
    }

    fn device_vdp_interrupt_enabled(&self) -> bool {
        self.vdp().interrupt_enabled()
    }

    fn device_frame_count(&self) -> u64 {
        self.vdp().frame_count()
    }
}
