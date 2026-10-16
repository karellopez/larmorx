// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! Datatypes: the datatype message (`H5O__dtype_decode_helper`, HDF5 1.12.1 `src/H5Odtype.c`)
//! and the conversion of floating-point data to the native `double` and `float` of x86-64, as
//! `H5Dread` converts it (`src/H5T.c` path selection, `src/H5Tconv.c`: the hard
//! `long double` to `float` conversion, `H5T__conv_order_opt` and `H5T__conv_f_f`,
//! with the bit-field helpers of `src/H5Tbit.c`).

use crate::error::{Error, Result, corrupt};
use crate::source::Cursor;

/// Byte order of an atomic type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Order {
    Le,
    Be,
    /// VAX order (byte pairs swapped in 32-bit words), floating point only.
    Vax,
}

/// Mantissa normalisation (`H5T_norm_t`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Norm {
    /// The leading 1 is implied (IEEE).
    Implied,
    /// The leading 1 is stored (MSB set).
    MsbSet,
    /// No normalisation (x87 extended precision stores its integer bit).
    None,
}

/// A floating-point type's bit layout (`H5T_atomic_t` with `u.f`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FloatType {
    pub size: usize,
    pub order: Order,
    /// Precision and offset of the significant bits within the element.
    pub precision: usize,
    pub offset: usize,
    /// Padding of the bits below the offset, above the precision, and inside.
    pub lsb_pad_one: bool,
    pub msb_pad_one: bool,
    pub int_pad_one: bool,
    pub norm: Norm,
    pub sign: usize,
    pub epos: usize,
    pub esize: usize,
    pub mpos: usize,
    pub msize: usize,
    pub ebias: u64,
}

impl FloatType {
    /// IEEE binary64, little-endian: `H5T_NATIVE_DOUBLE` on x86-64.
    pub const NATIVE_DOUBLE: FloatType = FloatType {
        size: 8,
        order: Order::Le,
        precision: 64,
        offset: 0,
        lsb_pad_one: false,
        msb_pad_one: false,
        int_pad_one: false,
        norm: Norm::Implied,
        sign: 63,
        epos: 52,
        esize: 11,
        mpos: 0,
        msize: 52,
        ebias: 1023,
    };
    /// IEEE binary32, little-endian: `H5T_NATIVE_FLOAT` on x86-64.
    pub const NATIVE_FLOAT: FloatType = FloatType {
        size: 4,
        order: Order::Le,
        precision: 32,
        offset: 0,
        lsb_pad_one: false,
        msb_pad_one: false,
        int_pad_one: false,
        norm: Norm::Implied,
        sign: 31,
        epos: 23,
        esize: 8,
        mpos: 0,
        msize: 23,
        ebias: 127,
    };
    /// x87 extended precision in 16 bytes: `H5T_NATIVE_LDOUBLE` on x86-64 Linux (what h5py
    /// writes for `numpy.longdouble` there).
    pub const NATIVE_LDOUBLE: FloatType = FloatType {
        size: 16,
        order: Order::Le,
        precision: 80,
        offset: 0,
        lsb_pad_one: false,
        msb_pad_one: false,
        int_pad_one: false,
        norm: Norm::None,
        sign: 79,
        epos: 64,
        esize: 15,
        mpos: 0,
        msize: 64,
        ebias: 16383,
    };
    /// IEEE binary16, little-endian.
    pub const IEEE_F16LE: FloatType = FloatType {
        size: 2,
        order: Order::Le,
        precision: 16,
        offset: 0,
        lsb_pad_one: false,
        msb_pad_one: false,
        int_pad_one: false,
        norm: Norm::Implied,
        sign: 15,
        epos: 10,
        esize: 5,
        mpos: 0,
        msize: 10,
        ebias: 15,
    };
    /// IEEE binary128, little-endian.
    pub const IEEE_F128LE: FloatType = FloatType {
        size: 16,
        order: Order::Le,
        precision: 128,
        offset: 0,
        lsb_pad_one: false,
        msb_pad_one: false,
        int_pad_one: false,
        norm: Norm::Implied,
        sign: 127,
        epos: 112,
        esize: 15,
        mpos: 0,
        msize: 112,
        ebias: 16383,
    };

    /// The same type in another byte order.
    pub fn with_order(mut self, order: Order) -> FloatType {
        self.order = order;
        self
    }
}

/// Character set of a string.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Charset {
    Ascii,
    Utf8,
    Other(u8),
}

/// How a fixed-length string ends (`H5T_str_t`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StrPad {
    NullTerm,
    NullPad,
    SpacePad,
    Other(u8),
}

/// A datatype, as far as this reader needs to know it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Datatype {
    Integer {
        size: usize,
        order: Order,
        signed: bool,
    },
    Float(FloatType),
    /// A fixed-length string.
    String {
        size: usize,
        pad: StrPad,
        charset: Charset,
    },
    /// A variable-length sequence or string, stored in the global heap.
    VarLen {
        /// The stored size of an element (a length and a global heap ID).
        size: usize,
        string: bool,
        pad: StrPad,
        charset: Charset,
        base: Box<Datatype>,
    },
    /// Any other class (`H5T_class_t`): time 2, bitfield 4, opaque 5, compound 6,
    /// reference 7, enum 8, array 10.
    Other {
        class: u8,
        size: usize,
    },
}

impl Datatype {
    /// The class number (`H5T_class_t`).
    pub fn class(&self) -> u8 {
        match self {
            Datatype::Integer { .. } => 0,
            Datatype::Float(_) => 1,
            Datatype::String { .. } => 3,
            Datatype::VarLen { .. } => 9,
            Datatype::Other { class, .. } => *class,
        }
    }

    /// The size of one element in the file.
    pub fn size(&self) -> usize {
        match self {
            Datatype::Integer { size, .. }
            | Datatype::String { size, .. }
            | Datatype::VarLen { size, .. }
            | Datatype::Other { size, .. } => *size,
            Datatype::Float(f) => f.size,
        }
    }

    /// `H5Tis_variable_str`.
    pub fn is_variable_string(&self) -> bool {
        matches!(self, Datatype::VarLen { string: true, .. })
    }
}

fn pad(v: u32) -> StrPad {
    match v {
        0 => StrPad::NullTerm,
        1 => StrPad::NullPad,
        2 => StrPad::SpacePad,
        x => StrPad::Other(x as u8),
    }
}

fn charset(v: u32) -> Charset {
    match v {
        0 => Charset::Ascii,
        1 => Charset::Utf8,
        x => Charset::Other(x as u8),
    }
}

/// `H5O__dtype_decode_helper`, for the classes a reader of transforms meets; other classes
/// are skipped over (their properties are not decoded, so they must be the last thing in the
/// message, as they are in a dataset's datatype message).
pub(crate) fn decode(c: &mut Cursor) -> Result<Datatype> {
    let word = c.u32()?;
    let version = (word >> 4) & 0x0f;
    if !(1..=4).contains(&version) {
        return Err(corrupt("bad version number for datatype message"));
    }
    let class = (word & 0x0f) as u8;
    let flags = word >> 8;
    let size = c.u32()? as usize;
    Ok(match class {
        0 => {
            c.skip(4)?; // offset, precision
            Datatype::Integer {
                size,
                order: if flags & 0x1 != 0 {
                    Order::Be
                } else {
                    Order::Le
                },
                signed: flags & 0x8 != 0,
            }
        }
        1 => {
            let mut order = if flags & 0x1 != 0 {
                Order::Be
            } else {
                Order::Le
            };
            if version >= 3 {
                if flags & 0x40 != 0 && flags & 0x1 == 0 {
                    return Err(corrupt("bad byte order for datatype message"));
                }
                if flags & 0x40 != 0 {
                    order = Order::Vax;
                }
            }
            let norm = match (flags >> 4) & 0x03 {
                0 => Norm::None,
                1 => Norm::MsbSet,
                2 => Norm::Implied,
                _ => return Err(corrupt("unknown floating-point normalization")),
            };
            let offset = usize::from(c.u16()?);
            let precision = usize::from(c.u16()?);
            let epos = usize::from(c.u8()?);
            let esize = usize::from(c.u8()?);
            let mpos = usize::from(c.u8()?);
            let msize = usize::from(c.u8()?);
            let ebias = u64::from(c.u32()?);
            Datatype::Float(FloatType {
                size,
                order,
                precision,
                offset,
                lsb_pad_one: flags & 0x2 != 0,
                msb_pad_one: flags & 0x4 != 0,
                int_pad_one: flags & 0x8 != 0,
                norm,
                sign: ((flags >> 8) & 0xff) as usize,
                epos,
                esize,
                mpos,
                msize,
                ebias,
            })
        }
        3 => Datatype::String {
            size,
            pad: pad(flags & 0x0f),
            charset: charset((flags >> 4) & 0x0f),
        },
        9 => {
            let string = flags & 0x0f == 1;
            let base = decode(c)?;
            Datatype::VarLen {
                size,
                string,
                pad: pad((flags >> 4) & 0x0f),
                charset: charset((flags >> 8) & 0x0f),
                base: Box::new(base),
            }
        }
        2 | 4..=8 | 10 => Datatype::Other { class, size },
        _ => return Err(corrupt("unknown datatype class found")),
    })
}

// ------------------------------------------------------------------------------------------------
// Bit fields (H5Tbit.c), bit 0 being the least significant bit of byte 0.

fn bit(buf: &[u8], i: usize) -> bool {
    (buf[i / 8] >> (i % 8)) & 1 != 0
}

fn set_bit(buf: &mut [u8], i: usize, v: bool) {
    if v {
        buf[i / 8] |= 1 << (i % 8);
    } else {
        buf[i / 8] &= !(1 << (i % 8));
    }
}

/// `H5T__bit_get_d`.
fn bit_get_d(buf: &[u8], offset: usize, size: usize) -> u64 {
    (0..size).fold(0u64, |acc, k| acc | (u64::from(bit(buf, offset + k)) << k))
}

/// `H5T__bit_set_d`.
fn bit_set_d(buf: &mut [u8], offset: usize, size: usize, val: u64) {
    for k in 0..size {
        set_bit(buf, offset + k, k < 64 && (val >> k) & 1 != 0);
    }
}

/// `H5T__bit_set`.
fn bit_set(buf: &mut [u8], offset: usize, size: usize, value: bool) {
    for k in 0..size {
        set_bit(buf, offset + k, value);
    }
}

/// `H5T__bit_copy`.
fn bit_copy(dst: &mut [u8], dst_off: usize, src: &[u8], src_off: usize, size: usize) {
    for k in 0..size {
        let v = bit(src, src_off + k);
        set_bit(dst, dst_off + k, v);
    }
}

/// `H5T__bit_find` from the least significant end: the position (relative to `offset`) of
/// the first bit equal to `value`, or -1.
fn bit_find_lsb(buf: &[u8], offset: usize, size: usize, value: bool) -> isize {
    (0..size)
        .find(|&k| bit(buf, offset + k) == value)
        .map_or(-1, |k| k as isize)
}

/// `H5T__bit_find` from the most significant end.
fn bit_find_msb(buf: &[u8], offset: usize, size: usize, value: bool) -> isize {
    (0..size)
        .rev()
        .find(|&k| bit(buf, offset + k) == value)
        .map_or(-1, |k| k as isize)
}

/// `H5T__bit_inc`: adds 1 to the field; returns the carry out.
fn bit_inc(buf: &mut [u8], start: usize, size: usize) -> bool {
    for k in 0..size {
        let i = start + k;
        if bit(buf, i) {
            set_bit(buf, i, false);
        } else {
            set_bit(buf, i, true);
            return false;
        }
    }
    true
}

fn to_little_endian(buf: &mut [u8], order: Order) {
    let n = buf.len();
    match order {
        Order::Le => {}
        Order::Be => buf.reverse(),
        Order::Vax => {
            let mut i = 0;
            while i < n {
                buf.swap(i, (n - 2) - i);
                buf.swap(i + 1, (n - 1) - i);
                i += 4;
            }
        }
    }
}

/// The state `H5T__conv_f_f` keeps from one element to the next within one call (one
/// conversion strip of `H5Dread`).
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ConvState {
    denormalized: bool,
    msize: usize,
    bitno: isize,
    /// HDF5 never resets `denormalized` between elements, keeps `msize` from the previous
    /// element when a mantissa without a leading bit has no bit set, and loses the carry when
    /// rounding a denormalized result. `true` reproduces that (HDF-01).
    pub(crate) reproduce: bool,
    /// Set when a correction changed an element (for D13's notice).
    pub(crate) corrected: bool,
}

/// One element of `H5T__conv_f_f`: `s` (the source element, as stored) converted to `dst`,
/// with HDF5's state (`st.reproduce`) or afresh. `st.corrected` notes when the two differ.
fn conv_f_f(src: &FloatType, dst: &FloatType, s_in: &[u8], st: &mut ConvState) -> Vec<u8> {
    // HDF5's own steps, which carry the state to the next element.
    let mut hdf = *st;
    let theirs = conv_f_f_mode(src, dst, s_in, &mut hdf, true);
    st.denormalized = hdf.denormalized;
    st.msize = hdf.msize;
    st.bitno = hdf.bitno;
    let mut fresh = *st;
    let ours = conv_f_f_mode(src, dst, s_in, &mut fresh, false);
    if ours != theirs {
        st.corrected = true;
    }
    if st.reproduce { theirs } else { ours }
}

fn conv_f_f_mode(
    src: &FloatType,
    dst: &FloatType,
    s_in: &[u8],
    st: &mut ConvState,
    hdf5: bool,
) -> Vec<u8> {
    let mut s = s_in.to_vec();
    let mut d = vec![0u8; dst.size];
    to_little_endian(&mut s, src.order);
    let expo_max: i64 = (1i64 << dst.esize) - 1;
    // The corrected conversion starts every element afresh.
    let (mut denormalized, mut msize, mut bitno) = if hdf5 {
        (st.denormalized, st.msize, st.bitno)
    } else {
        (false, 0usize, 0isize)
    };
    let mut implied: u64 = 1;
    let mut carry = false;

    'padding: {
        if bit_find_lsb(&s, src.mpos, src.msize, true) < 0 {
            if bit_find_lsb(&s, src.epos, src.esize, true) < 0 {
                // +0 or -0
                bit_copy(&mut d, dst.sign, &s, src.sign, 1);
                bit_set(&mut d, dst.epos, dst.esize, false);
                bit_set(&mut d, dst.mpos, dst.msize, false);
                break 'padding;
            } else if bit_find_lsb(&s, src.epos, src.esize, false) < 0 {
                // +Inf or -Inf
                bit_copy(&mut d, dst.sign, &s, src.sign, 1);
                bit_set(&mut d, dst.epos, dst.esize, true);
                bit_set(&mut d, dst.mpos, dst.msize, false);
                if dst.norm == Norm::None {
                    bit_set(&mut d, dst.mpos + dst.msize - 1, 1, true);
                }
                break 'padding;
            } else if !hdf5 && src.norm == Norm::None {
                // A mantissa without its leading bit and no bit set is zero, whatever the
                // exponent (HDF5 goes on with the previous element's mantissa size).
                bit_copy(&mut d, dst.sign, &s, src.sign, 1);
                bit_set(&mut d, dst.epos, dst.esize, false);
                bit_set(&mut d, dst.mpos, dst.msize, false);
                break 'padding;
            }
        } else if src.norm == Norm::None
            && bit_find_lsb(&s, src.mpos, src.msize - 1, true) < 0
            && bit_find_lsb(&s, src.epos, src.esize, false) < 0
        {
            // Only the explicit leading bit and an exponent of all ones: x87's infinity.
            bit_copy(&mut d, dst.sign, &s, src.sign, 1);
            bit_set(&mut d, dst.epos, dst.esize, true);
            bit_set(&mut d, dst.mpos, dst.msize, false);
            if dst.norm == Norm::None {
                bit_set(&mut d, dst.mpos + dst.msize - 1, 1, true);
            }
            break 'padding;
        } else if bit_find_lsb(&s, src.epos, src.esize, false) < 0 {
            // NaN: every bit of the significand set.
            bit_copy(&mut d, dst.sign, &s, src.sign, 1);
            bit_set(&mut d, dst.epos, dst.esize, true);
            bit_set(&mut d, dst.mpos, dst.msize, true);
            break 'padding;
        }

        let mut expo = bit_get_d(&s, src.epos, src.esize) as i64;
        if expo == 0 {
            denormalized = true;
        }
        let mpos = src.mpos;
        let mut mrsh: usize = 0;
        if expo == 0 || src.norm == Norm::None {
            bitno = bit_find_msb(&s, src.mpos, src.msize, true);
            if bitno > 0 {
                msize = bitno as usize;
            } else if bitno == 0 {
                msize = 1;
                bit_set(&mut s, src.mpos, 1, false);
            }
        } else if src.norm == Norm::Implied {
            msize = src.msize;
        } else {
            // "normalization method not implemented yet": HDF5 fails the read.
            msize = src.msize;
        }
        bit_copy(&mut d, dst.sign, &s, src.sign, 1);
        if expo == 0 || src.norm == Norm::None {
            let adj = src
                .ebias
                .wrapping_sub(1)
                .wrapping_add((src.msize as u64).wrapping_sub(bitno as u64));
            expo = expo.wrapping_sub(adj as i64);
        } else {
            expo -= src.ebias as i64;
        }
        if dst.norm == Norm::None {
            mrsh += 1;
        }
        expo += dst.ebias as i64;
        if expo < -(dst.msize as i64) {
            expo = 0;
            bit_set(&mut d, dst.mpos, dst.msize, false);
            msize = 0;
        } else if expo <= 0 {
            mrsh += (1 - expo) as usize;
            expo = 0;
            denormalized = true;
        } else if expo >= expo_max {
            expo = expo_max;
            bit_set(&mut d, dst.mpos, dst.msize, false);
            msize = 0;
        }
        if msize > 0 && mrsh <= dst.msize && mrsh + msize > dst.msize {
            bitno = (mrsh + msize - dst.msize) as isize;
            let b = bitno as usize;
            let cut = bit(&s, mpos + b - 1);
            if cut && !denormalized {
                if bit_find_lsb(&s, mpos + b, msize - b, false) >= 0 || expo < expo_max - 1 {
                    carry = bit_inc(&mut s, mpos + b - 1, 1 + msize - b);
                    if carry {
                        implied = 2;
                    }
                }
            } else if cut && denormalized {
                // HDF5 increments without keeping the carry; the corrected conversion keeps
                // it (a denormalized result rounding up to the smallest normal).
                let c = bit_inc(&mut s, mpos + b - 1, 1 + msize - b);
                if !hdf5 && c {
                    carry = true;
                    implied = 2;
                }
            }
        } else {
            carry = false;
        }
        // The mantissa.
        if mrsh > dst.msize + 1 {
            bit_set(&mut d, dst.mpos, dst.msize, false);
        } else if mrsh == dst.msize + 1 {
            bit_set(&mut d, dst.mpos + 1, dst.msize - 1, false);
            bit_set(&mut d, dst.mpos, 1, true);
        } else if mrsh == dst.msize {
            bit_set(&mut d, dst.mpos, dst.msize, false);
            bit_set_d(&mut d, dst.mpos, dst.msize.min(2), implied);
        } else {
            if mrsh > 0 {
                bit_set(&mut d, dst.mpos + dst.msize - mrsh, mrsh, false);
                bit_set_d(&mut d, dst.mpos + dst.msize - mrsh, 2, implied);
            }
            if mrsh + msize >= dst.msize {
                bit_copy(
                    &mut d,
                    dst.mpos,
                    &s,
                    mpos + msize + mrsh - dst.msize,
                    dst.msize - mrsh,
                );
            } else {
                bit_copy(
                    &mut d,
                    dst.mpos + dst.msize - (mrsh + msize),
                    &s,
                    mpos,
                    msize,
                );
                bit_set(&mut d, dst.mpos, dst.msize - (mrsh + msize), false);
            }
        }
        if carry {
            expo += 1;
            if expo >= expo_max {
                expo = expo_max;
                bit_set(&mut d, dst.mpos, dst.msize, false);
            }
        }
        bit_set_d(&mut d, dst.epos, dst.esize, expo as u64);
    }
    // padding:
    if dst.offset > 0 {
        bit_set(&mut d, 0, dst.offset, dst.lsb_pad_one);
    }
    if dst.offset + dst.precision != 8 * dst.size {
        let start = dst.offset + dst.precision;
        bit_set(&mut d, start, 8 * dst.size - start, dst.msb_pad_one);
    }
    to_little_endian(&mut d, dst.order); // the destination's order (inverse of the swap)
    if hdf5 {
        st.denormalized = denormalized;
        st.msize = msize;
        st.bitno = bitno;
    }
    d
}

/// Which conversion `H5Dread` uses from `src` to the native `dst` on x86-64.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Path {
    /// The same type: no conversion.
    Noop,
    /// The hard `long double` to `float` conversion (a C cast; above `FLT_MAX`, infinity).
    LdoubleFloat,
    /// The hard `long double` to `double` conversion (above `DBL_MAX`, infinity).
    LdoubleDouble,
    /// `H5T__conv_order_opt`: a byte swap.
    Swap,
    /// `H5T__conv_f_f`.
    Soft,
}

pub(crate) fn path(src: &FloatType, dst: &FloatType) -> Path {
    if src == dst {
        return Path::Noop;
    }
    if *src == FloatType::NATIVE_LDOUBLE && *dst == FloatType::NATIVE_FLOAT {
        return Path::LdoubleFloat;
    }
    if *src == FloatType::NATIVE_LDOUBLE && *dst == FloatType::NATIVE_DOUBLE {
        return Path::LdoubleDouble;
    }
    let opposite = matches!(
        (src.order, dst.order),
        (Order::Be, Order::Le) | (Order::Le, Order::Be)
    );
    if src.size == dst.size
        && src.offset == 0
        && dst.offset == 0
        && opposite
        && matches!(src.size, 1 | 2 | 4 | 8 | 16)
        && src.sign == dst.sign
        && src.epos == dst.epos
        && src.esize == dst.esize
        && src.ebias == dst.ebias
        && src.mpos == dst.mpos
        && src.msize == dst.msize
        && src.norm == dst.norm
        && src.int_pad_one == dst.int_pad_one
    {
        return Path::Swap;
    }
    Path::Soft
}

/// Checks what `H5T__conv_f_f`'s initialisation checks.
fn check_soft(src: &FloatType, dst: &FloatType) -> Result<()> {
    if dst.size > 64 || src.esize > 63 || dst.esize > 63 {
        return Err(Error::Conversion(
            "no appropriate function for conversion path".into(),
        ));
    }
    let bits = 8 * src.size;
    if src.sign >= bits
        || src.epos + src.esize > bits
        || src.mpos + src.msize > bits
        || src.msize == 0
        || src.esize == 0
    {
        return Err(corrupt("invalid floating-point datatype"));
    }
    if src.norm == Norm::MsbSet {
        return Err(Error::Conversion(
            "normalization method not implemented yet".into(),
        ));
    }
    Ok(())
}

/// The x87 value of a `long double` element as an `f32`, as GCC's cast computes it (round to
/// nearest even), with HDF5's range check: above `FLT_MAX` is infinity.
fn ldouble_to_f32(b: &[u8]) -> f32 {
    let mant = u64::from_le_bytes(b[0..8].try_into().unwrap());
    let se = u16::from_le_bytes(b[8..10].try_into().unwrap());
    let negative = se & 0x8000 != 0;
    let e = i32::from(se & 0x7fff);
    let sign = if negative { -1.0f32 } else { 1.0f32 };
    if e == 0x7fff {
        if mant << 1 == 0 {
            return sign * f32::INFINITY;
        }
        // A NaN keeps its sign and the top of its payload, quieted (x87's conversion).
        let payload = ((mant >> 40) as u32) & 0x003f_ffff;
        return f32::from_bits((u32::from(negative) << 31) | 0x7fc0_0000 | payload);
    }
    if mant == 0 {
        return sign * 0.0;
    }
    // value = mant * 2^(e - 16383 - 63), with e = 0 meaning 2^(1 - 16383 - 63).
    let exp2 = if e == 0 {
        1 - 16383 - 63
    } else {
        e - 16383 - 63
    };
    let lz = mant.leading_zeros() as i32;
    let m = mant << lz; // the leading 1 at bit 63
    let e2 = exp2 - lz + 63; // value = 1.xxx * 2^e2
    // HDF5 compares with FLT_MAX before the cast (`H5T_CONV_Ff_CORE`): above it is infinity.
    if e2 > 127 || (e2 == 127 && m > 0xffff_ff00_0000_0000) {
        return sign * f32::INFINITY;
    }
    // The number of mantissa bits a float keeps (24, fewer for subnormals).
    let keep: i32 = if e2 >= -126 { 24 } else { 24 - (-126 - e2) };
    if keep <= 0 {
        // Below half the smallest subnormal rounds to zero; exactly half to even (zero);
        // above it, to the smallest subnormal.
        if keep == 0 && m > (1u64 << 63) {
            return sign * f32::from_bits(1);
        }
        return sign * 0.0;
    }
    let shift = 64 - keep;
    let mut q = m >> shift;
    let rem = m & ((1u64 << shift) - 1);
    let half = 1u64 << (shift - 1);
    if rem > half || (rem == half && q & 1 == 1) {
        q += 1;
    }
    // value = q * 2^(e2 - keep + 1), exact in f64 and in f32.
    let v = (q as f64) * f64::from_bits(((1023 + e2 - keep + 1) as u64) << 52);
    sign * (v as f32)
}

/// The x87 value of a `long double` element as an `f64`, as GCC's cast computes it (round to
/// nearest even), with HDF5's range check: above `DBL_MAX` is infinity.
fn ldouble_to_f64(b: &[u8]) -> f64 {
    let mant = u64::from_le_bytes(b[0..8].try_into().unwrap());
    let se = u16::from_le_bytes(b[8..10].try_into().unwrap());
    let negative = se & 0x8000 != 0;
    let e = i32::from(se & 0x7fff);
    let sign = if negative { -1.0f64 } else { 1.0f64 };
    if e == 0x7fff {
        if mant << 1 == 0 {
            return sign * f64::INFINITY;
        }
        let payload = (mant >> 11) & 0x0007_ffff_ffff_ffff;
        return f64::from_bits((u64::from(negative) << 63) | 0x7ff8_0000_0000_0000 | payload);
    }
    if mant == 0 {
        return sign * 0.0;
    }
    let exp2 = if e == 0 {
        1 - 16383 - 63
    } else {
        e - 16383 - 63
    };
    let lz = mant.leading_zeros() as i32;
    let m = mant << lz;
    let e2 = exp2 - lz + 63;
    if e2 > 1023 || (e2 == 1023 && m > 0xffff_ffff_ffff_f800) {
        return sign * f64::INFINITY;
    }
    let keep: i32 = if e2 >= -1022 { 53 } else { 53 - (-1022 - e2) };
    if keep <= 0 {
        if keep == 0 && m > (1u64 << 63) {
            return sign * f64::from_bits(1);
        }
        return sign * 0.0;
    }
    let shift = 64 - keep;
    let mut q = m >> shift;
    let rem = m & ((1u64 << shift) - 1);
    let half = 1u64 << (shift - 1);
    if rem > half || (rem == half && q & 1 == 1) {
        q += 1;
    }
    // value = q * 2^(e2 - keep + 1): build it from bits (q < 2^54).
    let scale = e2 - keep + 1;
    if q == 1u64 << keep {
        // A carry: one more bit.
        q >>= 1;
        return sign * compose(q, scale + 1);
    }
    sign * compose(q, scale)
}

/// q * 2^scale exactly, for q < 2^53 and a representable result.
fn compose(q: u64, scale: i32) -> f64 {
    if q == 0 {
        return 0.0;
    }
    let lz = q.leading_zeros() as i32;
    let top = 63 - lz; // value = 1.xxx * 2^(top + scale)
    let e = top + scale;
    if e >= -1022 {
        let frac = (q << (52 - top)) & ((1u64 << 52) - 1);
        f64::from_bits(((e + 1023) as u64) << 52 | frac)
    } else {
        // Subnormal: q * 2^scale with scale + 1074 >= 0.
        f64::from_bits(q << (scale + 1074))
    }
}

/// Converts `n` stored elements of type `src` (in `raw`) to x86-64's native double
/// (`as_double`) or float, in strips of `strip` elements (one `H5T__conv_f_f` call each).
/// Returns the values, as f64, and whether a correction of HDF-01 changed one.
pub(crate) fn convert_floats(
    src: &FloatType,
    as_double: bool,
    raw: &[u8],
    strip: usize,
    reproduce: bool,
) -> Result<(Vec<f64>, bool)> {
    let dst = if as_double {
        FloatType::NATIVE_DOUBLE
    } else {
        FloatType::NATIVE_FLOAT
    };
    let n = raw.len() / src.size;
    let decode = |b: &[u8]| -> f64 {
        if as_double {
            f64::from_le_bytes(b.try_into().unwrap())
        } else {
            f64::from(f32::from_le_bytes(b.try_into().unwrap()))
        }
    };
    let mut out = Vec::with_capacity(n);
    let mut corrected = false;
    match path(src, &dst) {
        Path::Noop => {
            for e in raw.chunks_exact(src.size) {
                out.push(decode(e));
            }
        }
        Path::Swap => {
            for e in raw.chunks_exact(src.size) {
                let mut b = e.to_vec();
                b.reverse();
                out.push(decode(&b));
            }
        }
        Path::LdoubleDouble => {
            for e in raw.as_chunks::<16>().0 {
                out.push(ldouble_to_f64(e));
            }
        }
        Path::LdoubleFloat => {
            for e in raw.as_chunks::<16>().0 {
                out.push(f64::from(ldouble_to_f32(e)));
            }
        }
        Path::Soft => {
            check_soft(src, &dst)?;
            for strip_raw in raw.chunks(strip.max(1) * src.size) {
                let mut st = ConvState {
                    reproduce,
                    ..Default::default()
                };
                for e in strip_raw.chunks_exact(src.size) {
                    let d = conv_f_f(src, &dst, e, &mut st);
                    out.push(decode(&d));
                }
                corrected |= st.corrected;
            }
        }
    }
    Ok((out, corrected))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn soft(src: &FloatType, dst: &FloatType, bytes: &[u8]) -> Vec<u8> {
        let mut st = ConvState::default();
        conv_f_f(src, dst, bytes, &mut st)
    }

    #[test]
    fn soft_conversion_of_ieee_formats_is_exact_where_it_must_be() {
        // f32 -> f64 is exact for every class of value.
        for v in [
            0.0f32,
            -0.0,
            1.0,
            -2.5,
            f32::MIN_POSITIVE,
            f32::from_bits(1),
            f32::from_bits(0x007f_ffff),
            f32::MAX,
            f32::INFINITY,
            f32::NEG_INFINITY,
            1.234_567_9e-20,
        ] {
            let d = soft(
                &FloatType::NATIVE_FLOAT,
                &FloatType::NATIVE_DOUBLE,
                &v.to_le_bytes(),
            );
            let got = f64::from_le_bytes(d.try_into().unwrap());
            assert_eq!(got.to_bits(), f64::from(v).to_bits(), "{v:e}");
        }
        // f16 -> f32 (exact), including subnormals.
        for (h, f) in [
            (0x3c00u16, 1.0f32),
            (0xc000, -2.0),
            (0x0001, f32::from_bits(0x3380_0000)),
            (0x03ff, f32::from_bits(0x387f_c000)),
            (0x7bff, 65504.0),
            (0x7c00, f32::INFINITY),
        ] {
            let d = soft(
                &FloatType::IEEE_F16LE,
                &FloatType::NATIVE_FLOAT,
                &h.to_le_bytes(),
            );
            assert_eq!(f32::from_le_bytes(d.try_into().unwrap()), f, "{h:#x}");
        }
        // NaN: every significand bit set.
        let d = soft(
            &FloatType::NATIVE_FLOAT,
            &FloatType::NATIVE_DOUBLE,
            &f32::from_bits(0x7fc0_0001).to_le_bytes(),
        );
        assert_eq!(
            u64::from_le_bytes(d.try_into().unwrap()),
            0x7fff_ffff_ffff_ffff
        );
    }

    #[test]
    fn soft_conversion_rounds_half_up() {
        // f64 -> f32 through the soft path: ties round away from zero (HDF5's rounding).
        let tie = 1.0 + f64::from(f32::EPSILON) / 2.0; // halfway between 1 and the next float
        let d = soft(
            &FloatType::NATIVE_DOUBLE,
            &FloatType::NATIVE_FLOAT,
            &tie.to_le_bytes(),
        );
        assert_eq!(
            f32::from_le_bytes(d.try_into().unwrap()),
            1.0 + f32::EPSILON
        );
        let below = 1.0 + f64::from(f32::EPSILON) * 0.49;
        let d = soft(
            &FloatType::NATIVE_DOUBLE,
            &FloatType::NATIVE_FLOAT,
            &below.to_le_bytes(),
        );
        assert_eq!(f32::from_le_bytes(d.try_into().unwrap()), 1.0);
    }

    #[test]
    fn ldouble_cast() {
        fn x87(v: f64) -> [u8; 16] {
            // An f64 written as x87 extended precision (exact).
            let mut b = [0u8; 16];
            if v == 0.0 {
                b[9] = if v.is_sign_negative() { 0x80 } else { 0 };
                return b;
            }
            let bits = v.to_bits();
            let sign = (bits >> 63) as u16;
            let e = ((bits >> 52) & 0x7ff) as i32;
            let frac = bits & ((1u64 << 52) - 1);
            let (mant, exp) = (((1u64 << 52) | frac) << 11, e - 1023 + 16383);
            b[..8].copy_from_slice(&mant.to_le_bytes());
            b[8..10].copy_from_slice(&((sign << 15) | exp as u16).to_le_bytes());
            b
        }
        for v in [1.0f64, -3.25, 0.1, 1e-40, 3.0e38, 1.0e-45, 7.0e-46] {
            assert_eq!(
                ldouble_to_f32(&x87(v)).to_bits(),
                (v as f32).to_bits(),
                "{v:e}"
            );
        }
        assert_eq!(ldouble_to_f32(&x87(3.5e38)), f32::INFINITY);
        // Just above FLT_MAX but rounding to it: infinity for HDF5.
        let just_above = f64::from(f32::MAX) * (1.0 + 2f64.powi(-30));
        assert_eq!(ldouble_to_f32(&x87(just_above)), f32::INFINITY);
        assert_eq!(ldouble_to_f32(&x87(f64::from(f32::MAX))), f32::MAX);
        // To double: exact for every double, including subnormals and the extremes.
        for v in [
            1.0f64,
            -3.25,
            0.1,
            1e-300,
            f64::MAX,
            f64::MIN_POSITIVE,
            1e300,
        ] {
            assert_eq!(ldouble_to_f64(&x87(v)).to_bits(), v.to_bits(), "{v:e}");
        }
        // An x87 value between two doubles rounds to nearest even.
        let mut b = x87(1.0);
        b[0] = 0x00; // mantissa 1.0 + 2^-53 exactly: a tie, rounds to 1.0 (even)
        b[1] = 0x04;
        assert_eq!(ldouble_to_f64(&b), 1.0);
        b[1] = 0x0c; // 1.0 + 3 * 2^-53: a tie between odd and even, rounds up
        assert_eq!(ldouble_to_f64(&b), 1.0 + 2.0 * f64::EPSILON);
    }
}
