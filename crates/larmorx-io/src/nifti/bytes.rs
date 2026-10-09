//! Reading and writing header fields in a given byte order.

use super::header::ByteOrder;

/// Reads fixed-offset fields from a header block.
pub(crate) struct FieldReader<'a> {
    buf: &'a [u8],
    order: ByteOrder,
}

macro_rules! getter {
    ($name:ident, $t:ty, $n:expr) => {
        pub(crate) fn $name(&self, offset: usize) -> $t {
            let bytes: [u8; $n] = self.buf[offset..offset + $n].try_into().unwrap();
            match self.order {
                ByteOrder::Little => <$t>::from_le_bytes(bytes),
                ByteOrder::Big => <$t>::from_be_bytes(bytes),
            }
        }
    };
}

impl<'a> FieldReader<'a> {
    pub(crate) fn new(buf: &'a [u8], order: ByteOrder) -> Self {
        FieldReader { buf, order }
    }

    getter!(i16, i16, 2);
    getter!(i32, i32, 4);
    getter!(i64, i64, 8);
    getter!(f32, f32, 4);
    getter!(f64, f64, 8);

    pub(crate) fn u8(&self, offset: usize) -> u8 {
        self.buf[offset]
    }

    pub(crate) fn bytes<const N: usize>(&self, offset: usize) -> [u8; N] {
        self.buf[offset..offset + N].try_into().unwrap()
    }
}

/// Writes fixed-offset fields into a header block.
pub(crate) struct FieldWriter {
    pub(crate) buf: Vec<u8>,
    order: ByteOrder,
}

macro_rules! setter {
    ($name:ident, $t:ty) => {
        pub(crate) fn $name(&mut self, offset: usize, value: $t) {
            let bytes = match self.order {
                ByteOrder::Little => value.to_le_bytes(),
                ByteOrder::Big => value.to_be_bytes(),
            };
            self.buf[offset..offset + bytes.len()].copy_from_slice(&bytes);
        }
    };
}

impl FieldWriter {
    pub(crate) fn new(size: usize, order: ByteOrder) -> Self {
        FieldWriter {
            buf: vec![0; size],
            order,
        }
    }

    setter!(i16, i16);
    setter!(i32, i32);
    setter!(i64, i64);
    setter!(f32, f32);
    setter!(f64, f64);

    pub(crate) fn u8(&mut self, offset: usize, value: u8) {
        self.buf[offset] = value;
    }

    pub(crate) fn bytes(&mut self, offset: usize, value: &[u8]) {
        self.buf[offset..offset + value.len()].copy_from_slice(value);
    }
}
