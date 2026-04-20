use super::Sunsoft5BAudio;

impl Sunsoft5BAudio {
    pub(super) fn write_envelope_shape(&mut self, data: u8) {
        self.envelope_shape = data & 0x0F;
        self.envelope_up = (data & 0x04) != 0;
        self.envelope_volume = if self.envelope_up { 0 } else { 15 };
        self.envelope_holding = false;
        self.envelope_counter = self.envelope_period.max(1);
    }

    pub(super) fn clock_envelope(&mut self) {
        if self.envelope_holding {
            return;
        }

        if self.envelope_counter > 0 {
            self.envelope_counter -= 1;
        }
        if self.envelope_counter == 0 {
            self.envelope_counter = self.envelope_period.max(1);
            self.step_envelope();
        }
    }

    fn step_envelope(&mut self) {
        if self.envelope_up {
            if self.envelope_volume < 15 {
                self.envelope_volume += 1;
            } else {
                self.handle_envelope_boundary();
            }
        } else if self.envelope_volume > 0 {
            self.envelope_volume -= 1;
        } else {
            self.handle_envelope_boundary();
        }
    }

    fn handle_envelope_boundary(&mut self) {
        let cont = (self.envelope_shape & 0x08) != 0;
        let alt = (self.envelope_shape & 0x02) != 0;
        let hold = (self.envelope_shape & 0x01) != 0;

        if !cont {
            self.envelope_volume = 0;
            self.envelope_holding = true;
        } else if hold {
            if alt {
                self.envelope_volume = if self.envelope_up { 0 } else { 15 };
            }
            self.envelope_holding = true;
        } else if alt {
            self.envelope_up = !self.envelope_up;
        } else {
            self.envelope_volume = if self.envelope_up { 0 } else { 15 };
        }
    }
}
