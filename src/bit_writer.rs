use std::io::{self, prelude::*};

pub struct BitWriter<W: Write> {
    w: W,
    cur: u8,
    bit: u8,
}

impl<W: Write> BitWriter<W> {
    pub fn new(w: W) -> Self {
        Self { w, cur: 0, bit: 0 }
    }

    pub fn finish(mut self) -> io::Result<W> {
        self.flush_byte()?;
        Ok(self.w)
    }

    #[inline]
    fn flush_byte(&mut self) -> io::Result<()> {
        if self.bit > 0 {
            let src = [self.cur];
            if self.w.write(&src)? != 1 {
                return Err(io::Error::from(io::ErrorKind::WriteZero));
            }
            self.cur = 0;
            self.bit = 0;
        }
        Ok(())
    }

    pub fn byte_align(&mut self) -> io::Result<()> {
        self.flush_byte()
    }

    #[inline]
    pub fn write_bit(&mut self, bit: bool) -> io::Result<bool> {
        self.cur |= (bit as u8) << self.bit;
        self.bit += 1;
        if self.bit >= 8 {
            self.flush_byte()?;
        }
        Ok(bit)
    }

    pub fn write_bits(&mut self, src: &[u8], len: usize) -> io::Result<()> {
        for bit in 0..len {
            self.write_bit((src[bit >> 3] & (1 << (bit & 7))) != 0)?;
        }
        Ok(())
    }

    pub fn write_int(&mut self, value: u32, max: u32) -> io::Result<()> {
        assert!(max >= 2);

        if value >= max {
            return Err(io::Error::from(io::ErrorKind::InvalidInput));
        }

        let mut new_value = 0;
        let mut mask = 1;

        while (new_value + mask) < max && mask != 0 {
            self.write_bit(value & mask != 0)?;
            if value & mask != 0 {
                new_value |= mask;
            }
            mask *= 2;
        }

        Ok(())
    }

    pub fn write_int_packed(&mut self, mut value: u32) -> io::Result<()> {
        loop {
            let src = [(value & 0b111_1111) as u8];
            value >>= 7;
            self.write_bit(value != 0)?;
            self.write_bits(&src, 7)?;
            if value == 0 {
                break;
            }
        }
        Ok(())
    }

    pub fn write_positive_int_vector_packed(&mut self, v: (u32, u32, u32)) -> io::Result<()> {
        self.write_int_packed(v.0)?;
        self.write_int_packed(v.1)?;
        self.write_int_packed(v.2)
    }

    pub fn write_int_vector_packed(&mut self, v: (i32, i32, i32)) -> io::Result<()> {
        fn map(x: i32) -> u32 {
            ((x.abs() as u32) << 1) | (x.is_positive() as u32)
        }
        self.write_int_packed(map(v.0))?;
        self.write_int_packed(map(v.1))?;
        self.write_int_packed(map(v.2))
    }
}

impl<W: Write> Write for BitWriter<W> {
    fn write(&mut self, src: &[u8]) -> io::Result<usize> {
        self.write_bits(src, 8 * src.len())?;
        Ok(src.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.w.flush()
    }
}
