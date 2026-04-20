const CRC32_TABLE: [u32; 256] = {
    let mut table = [0u32; 256];
    let mut i = 0u32;
    while i < 256 {
        let mut crc = i;
        let mut j = 0;
        while j < 8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB8_8320;
            } else {
                crc >>= 1;
            }
            j += 1;
        }
        table[i as usize] = crc;
        i += 1;
    }
    table
};

pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in data {
        let index = ((crc ^ u32::from(byte)) & 0xFF) as usize;
        crc = (crc >> 8) ^ CRC32_TABLE[index];
    }
    crc ^ 0xFFFF_FFFF
}

pub struct StateWriter {
    buf: Vec<u8>,
}

impl StateWriter {
    pub fn new() -> Self {
        Self {
            buf: Vec::with_capacity(600 * 1024),
        }
    }

    pub fn into_vec(self) -> Vec<u8> {
        self.buf
    }

    pub fn write_u8(&mut self, v: u8) {
        self.buf.push(v);
    }

    pub fn write_i8(&mut self, v: i8) {
        self.buf.push(v as u8);
    }

    pub fn write_u16(&mut self, v: u16) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    pub fn write_i16(&mut self, v: i16) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    pub fn write_u32(&mut self, v: u32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    pub fn write_i32(&mut self, v: i32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    pub fn write_u64(&mut self, v: u64) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    pub fn write_f32(&mut self, v: f32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    pub fn write_bool(&mut self, v: bool) {
        self.buf.push(if v { 1 } else { 0 });
    }

    pub fn write_slice(&mut self, data: &[u8]) {
        self.buf.extend_from_slice(data);
    }

    pub fn write_vec_u8(&mut self, data: &[u8]) {
        self.write_u32(data.len() as u32);
        self.buf.extend_from_slice(data);
    }
}

pub struct StateReader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> StateReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    pub fn position(&self) -> usize {
        self.pos
    }

    pub fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    fn check(&self, n: usize) -> Result<(), &'static str> {
        if self.pos + n > self.data.len() {
            Err("state data truncated")
        } else {
            Ok(())
        }
    }

    pub fn read_u8(&mut self) -> Result<u8, &'static str> {
        self.check(1)?;
        let v = self.data[self.pos];
        self.pos += 1;
        Ok(v)
    }

    pub fn read_i8(&mut self) -> Result<i8, &'static str> {
        Ok(self.read_u8()? as i8)
    }

    pub fn read_u16(&mut self) -> Result<u16, &'static str> {
        self.check(2)?;
        let v = u16::from_le_bytes([self.data[self.pos], self.data[self.pos + 1]]);
        self.pos += 2;
        Ok(v)
    }

    pub fn read_i16(&mut self) -> Result<i16, &'static str> {
        self.check(2)?;
        let v = i16::from_le_bytes([self.data[self.pos], self.data[self.pos + 1]]);
        self.pos += 2;
        Ok(v)
    }

    pub fn read_u32(&mut self) -> Result<u32, &'static str> {
        self.check(4)?;
        let v = u32::from_le_bytes([
            self.data[self.pos],
            self.data[self.pos + 1],
            self.data[self.pos + 2],
            self.data[self.pos + 3],
        ]);
        self.pos += 4;
        Ok(v)
    }

    pub fn read_i32(&mut self) -> Result<i32, &'static str> {
        self.check(4)?;
        let v = i32::from_le_bytes([
            self.data[self.pos],
            self.data[self.pos + 1],
            self.data[self.pos + 2],
            self.data[self.pos + 3],
        ]);
        self.pos += 4;
        Ok(v)
    }

    pub fn read_u64(&mut self) -> Result<u64, &'static str> {
        self.check(8)?;
        let v = u64::from_le_bytes([
            self.data[self.pos],
            self.data[self.pos + 1],
            self.data[self.pos + 2],
            self.data[self.pos + 3],
            self.data[self.pos + 4],
            self.data[self.pos + 5],
            self.data[self.pos + 6],
            self.data[self.pos + 7],
        ]);
        self.pos += 8;
        Ok(v)
    }

    pub fn read_f32(&mut self) -> Result<f32, &'static str> {
        self.check(4)?;
        let v = f32::from_le_bytes([
            self.data[self.pos],
            self.data[self.pos + 1],
            self.data[self.pos + 2],
            self.data[self.pos + 3],
        ]);
        self.pos += 4;
        Ok(v)
    }

    pub fn read_bool(&mut self) -> Result<bool, &'static str> {
        Ok(self.read_u8()? != 0)
    }

    pub fn read_slice(&mut self, len: usize) -> Result<&'a [u8], &'static str> {
        self.check(len)?;
        let slice = &self.data[self.pos..self.pos + len];
        self.pos += len;
        Ok(slice)
    }

    pub fn read_into_slice(&mut self, dst: &mut [u8]) -> Result<(), &'static str> {
        let len = dst.len();
        self.check(len)?;
        dst.copy_from_slice(&self.data[self.pos..self.pos + len]);
        self.pos += len;
        Ok(())
    }

    pub fn read_vec_u8(&mut self) -> Result<Vec<u8>, &'static str> {
        let len = self.read_u32()? as usize;
        self.check(len)?;
        let v = self.data[self.pos..self.pos + len].to_vec();
        self.pos += len;
        Ok(v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_primitives() {
        let mut w = StateWriter::new();
        w.write_u8(0xAB);
        w.write_i8(-42);
        w.write_u16(0x1234);
        w.write_i16(-1000);
        w.write_u32(0xDEAD_BEEF);
        w.write_i32(-100_000);
        w.write_u64(0x0102_0304_0506_0708);
        w.write_f32(3.14);
        w.write_bool(true);
        w.write_bool(false);
        w.write_slice(&[1, 2, 3]);
        w.write_vec_u8(&[10, 20, 30, 40]);

        let data = w.into_vec();
        let mut r = StateReader::new(&data);
        assert_eq!(r.read_u8().unwrap(), 0xAB);
        assert_eq!(r.read_i8().unwrap(), -42);
        assert_eq!(r.read_u16().unwrap(), 0x1234);
        assert_eq!(r.read_i16().unwrap(), -1000);
        assert_eq!(r.read_u32().unwrap(), 0xDEAD_BEEF);
        assert_eq!(r.read_i32().unwrap(), -100_000);
        assert_eq!(r.read_u64().unwrap(), 0x0102_0304_0506_0708);
        assert!((r.read_f32().unwrap() - 3.14).abs() < 1e-6);
        assert!(r.read_bool().unwrap());
        assert!(!r.read_bool().unwrap());
        let mut buf = [0u8; 3];
        r.read_into_slice(&mut buf).unwrap();
        assert_eq!(buf, [1, 2, 3]);
        assert_eq!(r.read_vec_u8().unwrap(), vec![10, 20, 30, 40]);
        assert_eq!(r.remaining(), 0);
    }

    #[test]
    fn reader_detects_truncated_data() {
        let r_result = StateReader::new(&[0x01]).read_u16();
        assert!(r_result.is_err());
    }

    #[test]
    fn crc32_known_value() {
        // CRC32 of "123456789" is 0xCBF43926.
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn crc32_empty() {
        assert_eq!(crc32(&[]), 0x0000_0000);
    }
}
