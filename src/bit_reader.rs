// Currently panics on EOF.
// Needs improvement.

use std::io::{self, prelude::*};
use bitvec::order;
use bitvec::slice::BitSlice;
use bitvec::field::BitField;

pub struct BitReader<'b> {
    buf: &'b BitSlice<order::Lsb0, u8>,
    reference: &'b BitSlice<order::Lsb0, u8>,
}

impl<'b> BitReader<'b> {
    pub fn new(buf: &'b [u8]) -> Self {
        let bits = BitSlice::from_slice(buf).unwrap();
        Self {
            buf: bits,
            reference: bits,
        }
    }

    pub fn new_at(buf: &'b [u8], offset: usize) -> Self {
        let bits = &BitSlice::from_slice(buf).unwrap()[offset..];
        Self {
            buf: bits,
            reference: bits,
        }
    }

    pub fn pos(&self) -> usize {
        self.reference.offset_from(&self.buf) as usize
    }

    pub fn offset(&self, storage: &[u8]) -> isize {
        let other = BitSlice::from_slice(storage).unwrap();
        other.offset_from(&self.buf)
    }

    // ReadBit
    #[inline(always)]
    pub fn read_bit(&mut self) -> bool {
        let (first, rest) = self.buf.split_first().unwrap();
        self.buf = rest;
        *first
    }

    // SerializeInt
    #[inline]
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
    #[inline]
    pub fn read_int_packed(&mut self) -> u32 {
        let mut value = 0;

        for i in 0..5 {
            let (byte, rest) = self.buf.split_at(8);
            self.buf = rest;
            let has_next = unsafe { *byte.get_unchecked(0) };
            let part: u32 = unsafe { byte.get_unchecked(1..) }.load();
            value |= part << (7 * i);
            if !has_next {
                break;
            }
        }

        value
    }

    // EatByteAlign
    #[inline(always)]
    pub fn eat_byte_align(&mut self) {
        let offset = self.pos() & 7;
        if offset > 0 {
            self.buf = &self.buf[(8 - offset)..];
        }
        // This is slower
        //self.buf = &self.buf[(8 - (self.pos() & 7)) & 7..];
    }

    // SerializeIntVectorPacked
    pub fn read_int_vector_packed(&mut self) -> (i32, i32, i32) {
        #[inline(always)]
        fn item<'a>(r: &mut BitReader<'a>) -> i32 {
            let value = r.read_int_packed();
            (value >> 1) as i32 * if value & 1 != 0 { 1 } else { -1 }
        }
        (item(self), item(self), item(self))
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

impl<'b> Read for BitReader<'b> {
    fn read(&mut self, dst: &mut [u8]) -> io::Result<usize> {
        self.buf.read(dst)
    }
}
