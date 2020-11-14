// Currently panics on EOF.
// Needs improvement.

use std::io::{self, prelude::*};

pub struct BitReader<R: Read> {
    inner: bitstream_io::BitReader<R, bitstream_io::LE>,
}

impl<R: Read> BitReader<R> {
    pub fn new(buf: R) -> Self {
        Self { inner: bitstream_io::BitReader::new(buf) }
    }

    pub fn skip(&mut self, len: u32) {
        self.inner.skip(len);
    }

    // ReadBit
    #[inline(always)]
    pub fn read_bit(&mut self) -> bool {
        self.inner.read_bit().unwrap()
    }

    // SerializeBits
    pub fn read_bits(&mut self, dst: &mut [u8], len: usize) {
        self.inner.read_bytes(&mut dst[..len >> 3]).unwrap();
        if len & 7 != 0 {
            dst[(len >> 3) + 1] = self.inner.read((len & 7) as u32).unwrap();
        }
    }

    // SerializeInt
    pub fn read_int(&mut self, max: u32) -> u32 {
        let mut value = 0;
        let mut mask = 1;

        while (value + mask) < max && mask != 0 {
            if self.read_bit() {
                value |= mask;
            }
            mask *= 2;
        }

        value
    }

    // SerializeIntPacked
    pub fn read_int_packed(&mut self) -> u32 {
        /*
        let mut src = &self.buf[..];
        let bit_count_used_in_byte = self.pos & 7;
        let bit_count_left_in_byte = 8 - (self.pos & 7);
        let src_mask_byte_0 = ((1 << bit_count_left_in_byte) - 1) as u8;
        let src_mask_byte_1 = ((1 << bit_count_used_in_byte) - 1) as u8;
        let next_src_index = (bit_count_used_in_byte != 0) as usize;

        let mut value: u32 = 0;

        let mut it = 0;
        let mut shift_count = 0;

        while it < 5 {
            self.pos += 8;

            let byte = ((src[0] >> bit_count_used_in_byte) & src_mask_byte_0)
                | ((src[next_src_index] & src_mask_byte_1) << (bit_count_left_in_byte & 7));
            let next_byte_indicator = byte & 1;
            let byte_as_word = (byte >> 1) as u32;
            value = (byte_as_word << shift_count) | value;
            src = &src[1..];

            if next_byte_indicator == 0 {
                break;
            }

            it += 1;
            shift_count += 7;
        }

        value
        */

        let mut value = 0;

        for i in 0..5 {
            let has_next = self.inner.read_bit().unwrap();
            let part: u32 = self.inner.read(7).unwrap();
            value |= part << (7 * i);
            if !has_next {
                break;
            }
        }

        value
    }

    // EatByteAlign
    pub fn eat_byte_align(&mut self) {
        self.inner.byte_align();
    }

    // SerializeIntVectorPacked
    pub fn read_int_vector_packed(&mut self) -> (i32, i32, i32) {
        (self.rivp_item(), self.rivp_item(), self.rivp_item())
    }

    #[inline(always)]
    fn rivp_item(&mut self) -> i32 {
        let value = self.read_int_packed();
        (value >> 1) as i32 * if value & 1 != 0 { 1 } else { -1 }
    }

    // SerializePositiveIntVectorPacked
    pub fn read_positive_int_vector_packed(&mut self) -> (u32, u32, u32) {
        (
            self.read_int_packed(),
            self.read_int_packed(),
            self.read_int_packed(),
        )
    }
}

impl<R: Read> Read for BitReader<R> {
    fn read(&mut self, dst: &mut [u8]) -> io::Result<usize> {
        self.inner.read_bytes(dst)?;
        Ok(dst.len())
    }
}
