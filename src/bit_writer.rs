use std::io::{self, prelude::*};
use std::convert::TryFrom;

pub struct BitWriter<W: Write> {
    w: W,
    cur: u8,
    bit: u8,
}

const BIT_SHIFT: [u8; 8] = [
    1 << 0, // 0x01
    1 << 1, // 0x02
    1 << 2, // 0x04
    1 << 3, // 0x08
    1 << 4, // 0x10
    1 << 5, // 0x20
    1 << 6, // 0x40
    1 << 7, // 0x80
];
const BIT_MASK: [u8; 8] = [
    0b0000_0000,
    0b0000_0001,
    0b0000_0011,
    0b0000_0111,
    0b0000_1111,
    0b0001_1111,
    0b0011_1111,
    0b0111_1111,
    // awkward silence
];

impl<W: Write> BitWriter<W> {
    pub fn new(w: W) -> Self {
        Self { w, cur: 0, bit: 0 }
    }

    pub fn finish(mut self) -> io::Result<W> {
        self.byte_align()?;
        Ok(self.w)
    }

    #[inline(always)]
    pub fn byte_aligned(&self) -> bool {
        self.bit == 0
    }

    #[inline]
    pub fn byte_align(&mut self) -> io::Result<()> {
        if self.bit != 0 {
            write_byte(&mut self.w, self.cur)?;
            self.cur = 0;
            self.bit = 0;
        }
        Ok(())
    }

    #[inline]
    pub fn write_bit(&mut self, bit: bool) -> io::Result<bool> {
        if bit {
            debug_assert!(usize::from(self.bit) < BIT_SHIFT.len());
            let shift = unsafe { BIT_SHIFT.get_unchecked(usize::from(self.bit)) };
            self.cur |= shift;
        }
        if self.bit == 7 {
            write_byte(&mut self.w, self.cur)?;
            self.cur = 0;
            self.bit = 0;
        } else {
            self.bit += 1;
        }
        debug_assert!(self.bit < 8);
        Ok(bit)
    }

    pub fn write_bits(&mut self, src: &[u8], bits: usize) -> io::Result<()> {
        //self.write_bits_basic(bits, src)
        self.write_bits_naive(bits, src)
    }

    #[inline]
    pub fn write_bits_naive(&mut self, bits: usize, src: &[u8]) -> io::Result<()> {
        for bit in 0..bits {
            self.write_bit((src[bit >> 3] & (1 << (bit & 7))) != 0)?;
        }
        Ok(())
    }

    #[inline]
    pub fn write_bits_basic(&mut self, bits: usize, src: &[u8]) -> io::Result<()> {
        let full_bytes = bits >> 3;
        let extra_bits = bits & 7;
        for byte in &src[0..full_bytes] {
            self.write_byte(*byte)?;
        }
        if extra_bits != 0 {
            self.write_byte_partial(extra_bits, src[full_bytes])?;
        }
        Ok(())
    }

    #[inline]
    pub fn write_byte(&mut self, byte: u8) -> io::Result<()> {
        if self.bit == 0 {
            write_byte(&mut self.w, byte)
        } else {
            write_byte(&mut self.w, self.cur | (byte << self.bit))?;
            self.cur = byte >> self.bit;
            Ok(())
        }
    }

    #[inline]
    pub fn write_byte_partial(&mut self, bits: usize, byte: u8) -> io::Result<()> {
        debug_assert!(bits < 8);
        if self.bit == 0 {
            self.cur = byte;
            self.bit = bits as u8;
        } else {
            /*if self.bit + bits >= 8 {
                write_byte(&mut self.w, self.cur | (byte << self.bit))?;
            }
            self.cur = byte >> self.bit;
            Ok(())*/
            for bit in 0..bits {
                self.write_bit((byte & BIT_SHIFT[bit]) != 0)?;
            }
        }
        Ok(())
    }

    /*#[inline(always)]
    pub fn write_bits_align_src(&mut self, bits: usize, src: &[u8]) -> io::Result<()> {
        let align = bits & 7;

        if align != 0 {
            for bit in 0..bits {
                self.write_bit((src[bit >> 3] & (1 << (bit & 7))) != 0)?;
            }
        }

        Ok(())
    }

    #[inline(always)]
    pub fn write_bits_align_src_byte(&mut self, byte: u8) -> io::Result<()> {

        Ok(())
    }

    #[inline(always)]
    pub fn write_bits_madness(&mut self, bits: usize, src: &[u8]) -> io::Result<()> {
        if bits == 0 {
            return Ok(());
        }

        // This many bits are needed to fully byte align.
        let bits_align = usize::from((8 - self.bit) & 7);

        // Try to byte align if necessary first.
        // This will need less than 8 bits of src[0],
        // so the first byte is still to be used afterwards.
        if bits_align > 0 {
            // Does the input have enough bits to byte align?
            if bits >= bits_align {
                // TODO: Overhead of bounds check in `BIT_MASK[]`
                let mask = BIT_MASK[bits_align];
                self.cur |= (src[0] & mask) << self.bit;
                write_byte(&mut self.w, self.cur)?;
                self.cur = 0;
                self.bit = 0;
                if bits == bits_align {
                    return Ok(());
                }
            } else {
                // The input is too short, just push what is available.
                self.cur |= src[0] << self.bit;
                self.bit += u8::try_from(bits).expect("bits should be < 8 here");
                debug_assert!(self.bit > 0 && self.bit < 8);
                return Ok(());
            }
        }

        let bits = bits - bits_align;

        if bits <= 8 {
            // TODO: Can be optimized easily
            for bit in 0..bits {
                self.write_bit((src[(bits_align + bit) >> 3] & (1 << (bit & 7))) != 0)?;
            }
            return Ok(());
        }

        // We are now byte aligned and have more than a full byte to write.
        debug_assert_eq!(self.bit, 0);
        let aligned_bytes = bits >> 3;

        // A rolling window of bytes coming in to be written out.
        let mut accum: u32 = 0;

        // Newly read bytes should arrive at this offset in `accum`.
        let insert_shift = todo!();

        // Fill up the first byte to write.
        accum |= u32::from(src[0] >> bits_align);
        accum |= u32::from(src[1]) << (8 - bits_align);

        for src_index in 1..aligned_bytes {
            accum = (accum | u32::from(src[src_index]) << insert_shift) >> 8;
            write_byte(&mut self.w, u8::try_from(accum & 0xff).unwrap())?;
        }

        // Write the last < 8 bits.
        self.cur = src[todo!()];
        self.bit = todo!();

        todo!("write the last [0, 8) bits");

        /*for bit in 0..bits {
            self.write_bit((src[bit >> 3] & (1 << (bit & 7))) != 0)?;
        }*/

        Ok(())
    }*/

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

fn write_byte(mut writer: impl Write, byte: u8) -> io::Result<()> {
    writer.write_all(&[byte])
}

/*fn write_unaligned(
    writer: impl Write,
    acc: &mut BitQueue,
    rem: &mut BitQueue,
) -> io::Result<()>
{
    if rem.is_empty() {
        Ok(())
    } else {
        use std::cmp::min;
        let bits_to_transfer = min(8 - rem.len(), acc.len());
        rem.push(bits_to_transfer, acc.pop(bits_to_transfer).to_u8());
        if rem.len() == 8 {
            write_byte(writer, rem.pop(8))
        } else {
            Ok(())
        }
    }
}

fn write_aligned(mut writer: impl Write, acc: &mut BitQueue) -> io::Result<()> {
    let to_write = (acc.len() / 8) as usize;
    if to_write > 0 {
        // TODO: 128-bit types are the maximum supported
        assert!(to_write <= 16);
        let mut buf = [0; 16];
        for b in buf[0..to_write].iter_mut() {
            *b = acc.pop(8).to_u8();
        }
        writer.write_all(&buf[0..to_write])
    } else {
        Ok(())
    }
}*/
