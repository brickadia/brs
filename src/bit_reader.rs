// Currently panics on EOF.
// Needs improvement.

use std::io::{self, prelude::*};

pub struct BitReader {
    buf: Vec<u8>,
    pos: usize,
}

impl BitReader {
    pub fn new(buf: Vec<u8>) -> Self {
        Self { buf, pos: 0 }
    }

    // ReadBit
    #[inline(always)]
    pub fn read_bit(&mut self) -> bool {
        let bit = (self.buf[self.pos >> 3] & (1 << (self.pos & 7))) != 0;
        self.pos += 1;
        bit
    }

    // SerializeBits
    pub fn read_bits(&mut self, dst: &mut [u8], len: usize) {
        for bit in 0..len {
            let byte = &mut dst[bit >> 3];
            let shift = bit & 7;
            *byte = (*byte & !(1 << shift)) | (u8::from(self.read_bit()) << shift);
        }
        self.pos += len;
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
            let has_next = self.read_bit();
            let mut part = 0;
            for bit_shift in 0..7 {
                part |= (self.read_bit() as u32) << bit_shift;
            }
            value |= part << (7 * i);
            if !has_next {
                break;
            }
        }

        value
    }

    // EatByteAlign
    pub fn eat_byte_align(&mut self) {
        self.pos = (self.pos + 7) & !0x07;
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

impl Read for BitReader {
    fn read(&mut self, dst: &mut [u8]) -> io::Result<usize> {
        self.read_bits(dst, dst.len() * 8);
        Ok(dst.len())
    }
}
