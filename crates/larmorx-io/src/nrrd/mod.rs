// SPDX-License-Identifier: Apache-2.0 AND Zlib
//! NRRD files read as NrrdIO reads them (`nrrdLoad`): a port of NrrdIO's reading path (Teem's
//! NrrdIO, SVN `NrrdIO/trunk` at revision 3991, 2008-11-18, `TEEM_VERSION` 1.9.0; Gordon Kindlmann
//! and the University of Utah, zlib-style terms, the notice in `NOTICE`), configured as FreeSurfer
//! 8.2.0's programs behave: the quiet NaN's high bit set, the `gzip` and `bzip2` encodings not
//! available, no direct I/O, 64-bit `size_t`.
//!
//! What is ported: `nrrdLoad` and what it calls (`read.c`, `formatNRRD.c`'s reader, every field
//! parser of `parseNrrd.c`, the field checks of `simple.c`, the `raw`, `ascii` and `hex`
//! encodings, `endianNrrd.c`'s swaps, the enum tables of `enumsNrrd.c`, `parseAir.c`'s number
//! parsers, `string.c`'s helpers, `keyvalue.c`, `comment.c`), and biff's message stack
//! (`biffbiff.c`), whose text a caller prints (`biffGetDone`). The other formats are NrrdIO's stubs:
//! only a NRRD magic is recognised. Numbers are read with glibc's `sscanf` conversions
//! (`larmorx_core::cscan`).
//!
//! The data are kept as the bytes of the machine's own order (little-endian on every platform
//! larmorx supports), as NrrdIO leaves them after its swap.
//!
//! NrrdIO's defects (register entries FS-518 to FS-520 and FS-761 onwards; `docs/findings/
//! nrrdio.md`) are corrected by default and reproduced with
//! [`larmorx_core::defects::UpstreamDefects::Reproduce`], each with a notice.

mod read;

#[cfg(test)]
mod tests;

pub use read::{LoadError, load, load_with, load_with_errno};

/// `NRRD_DIM_MAX`.
pub const NRRD_DIM_MAX: usize = 16;
/// `NRRD_SPACE_DIM_MAX`.
pub const NRRD_SPACE_DIM_MAX: usize = 8;
/// `BIFF_STRLEN`: the size of NrrdIO's message buffers (a longer message overflows them).
pub const BIFF_STRLEN: usize = 257;
/// `AIR_STRLEN_HUGE`: the ASCII encoding's token buffer.
pub const AIR_STRLEN_HUGE: usize = 1025;
/// biff's key for NrrdIO's messages (`NRRD`).
pub const NRRD: &str = "nrrd";

/// `airEndianLittle`, `airEndianBig` (and 0, unknown).
pub const AIR_ENDIAN_LITTLE: i32 = 1234;
pub const AIR_ENDIAN_BIG: i32 = 4321;
/// The machine's order (`AIR_ENDIAN`): little on every platform larmorx supports.
pub const AIR_ENDIAN: i32 = AIR_ENDIAN_LITTLE;

/// `nrrdType` values.
pub mod types {
    pub const UNKNOWN: i32 = 0;
    pub const CHAR: i32 = 1;
    pub const UCHAR: i32 = 2;
    pub const SHORT: i32 = 3;
    pub const USHORT: i32 = 4;
    pub const INT: i32 = 5;
    pub const UINT: i32 = 6;
    pub const LLONG: i32 = 7;
    pub const ULLONG: i32 = 8;
    pub const FLOAT: i32 = 9;
    pub const DOUBLE: i32 = 10;
    pub const BLOCK: i32 = 11;
}

/// `nrrdSpace` values that FreeSurfer reads.
pub mod spaces {
    pub const UNKNOWN: i32 = 0;
    pub const RAS: i32 = 1;
    pub const LAS: i32 = 2;
    pub const LPS: i32 = 3;
}

/// `nrrdField` values.
pub(crate) mod field {
    pub const UNKNOWN: usize = 0;
    pub const COMMENT: usize = 1;
    pub const CONTENT: usize = 2;
    pub const NUMBER: usize = 3;
    pub const TYPE: usize = 4;
    pub const BLOCK_SIZE: usize = 5;
    pub const DIMENSION: usize = 6;
    pub const SPACE: usize = 7;
    pub const SPACE_DIMENSION: usize = 8;
    pub const SIZES: usize = 9;
    pub const SPACINGS: usize = 10;
    pub const THICKNESSES: usize = 11;
    pub const AXIS_MINS: usize = 12;
    pub const AXIS_MAXS: usize = 13;
    pub const SPACE_DIRECTIONS: usize = 14;
    pub const CENTERS: usize = 15;
    pub const KINDS: usize = 16;
    pub const LABELS: usize = 17;
    pub const UNITS: usize = 18;
    pub const MIN: usize = 19;
    pub const MAX: usize = 20;
    pub const OLD_MIN: usize = 21;
    pub const OLD_MAX: usize = 22;
    pub const ENDIAN: usize = 23;
    pub const ENCODING: usize = 24;
    pub const LINE_SKIP: usize = 25;
    pub const BYTE_SKIP: usize = 26;
    pub const KEYVALUE: usize = 27;
    pub const SAMPLE_UNITS: usize = 28;
    pub const SPACE_UNITS: usize = 29;
    pub const SPACE_ORIGIN: usize = 30;
    pub const MEASUREMENT_FRAME: usize = 31;
    pub const DATA_FILE: usize = 32;
    /// `NRRD_FIELD_MAX`.
    pub const MAX_FIELD: usize = 32;
}

/// `nrrdTypeSize`.
pub(crate) const TYPE_SIZE: [usize; 12] = [0, 1, 1, 2, 2, 4, 4, 8, 8, 4, 8, 0];

/// `AIR_NAN` as a `double`: NrrdIO's float quiet NaN (`airFloatQNaN`, 0x7fffffff) converted to
/// `double` (the float's payload in the high bits: 0x7fffffffe0000000), written as bits so that
/// no conversion of a NaN is left to the compiler.
pub fn air_nan() -> f64 {
    f64::from_bits(0x7fff_ffff_e000_0000)
}

/// `AIR_EXISTS(x)` (`!((x) - (x))`): `x` is finite.
pub(crate) fn exists(x: f64) -> bool {
    x.is_finite()
}

/// `airIsInf_d`: 1 for +inf, -1 for -inf, 0 otherwise.
pub(crate) fn is_inf(x: f64) -> i32 {
    if x == f64::INFINITY {
        1
    } else if x == f64::NEG_INFINITY {
        -1
    } else {
        0
    }
}

/// One axis (`NrrdAxisInfo`).
#[derive(Clone, Debug, PartialEq)]
pub struct Axis {
    pub size: usize,
    pub spacing: f64,
    pub thickness: f64,
    pub min: f64,
    pub max: f64,
    pub space_direction: [f64; NRRD_SPACE_DIM_MAX],
    pub center: i32,
    pub kind: i32,
    pub label: Option<Vec<u8>>,
    pub units: Option<Vec<u8>>,
}

impl Default for Axis {
    /// `_nrrdAxisInfoInit`.
    fn default() -> Axis {
        let nan = air_nan();
        Axis {
            size: 0,
            spacing: nan,
            thickness: nan,
            min: nan,
            max: nan,
            space_direction: [nan; NRRD_SPACE_DIM_MAX],
            center: 0,
            kind: 0,
            label: None,
            units: None,
        }
    }
}

/// A nrrd (`Nrrd`), as `nrrdLoad` leaves it. Strings are C strings' bytes (no NUL).
#[derive(Clone, Debug, PartialEq)]
pub struct Nrrd {
    /// The samples, in the machine's byte order; `None` before reading.
    pub data: Option<Vec<u8>>,
    /// An `nrrdType` value ([`types`]).
    pub type_: i32,
    pub dim: u32,
    pub axis: Vec<Axis>,
    pub content: Option<Vec<u8>>,
    pub sample_units: Option<Vec<u8>>,
    /// An `nrrdSpace` value ([`spaces`]).
    pub space: i32,
    pub space_dim: u32,
    pub space_units: Vec<Option<Vec<u8>>>,
    pub space_origin: [f64; NRRD_SPACE_DIM_MAX],
    pub measurement_frame: [[f64; NRRD_SPACE_DIM_MAX]; NRRD_SPACE_DIM_MAX],
    pub block_size: usize,
    pub old_min: f64,
    pub old_max: f64,
    /// The comments (`cmt`).
    pub cmt: Vec<Vec<u8>>,
    /// The key/value pairs (`kvp`), in the order they were added.
    pub kvp: Vec<(Vec<u8>, Vec<u8>)>,
}

impl Default for Nrrd {
    /// `nrrdNew` (`nrrdInit`).
    fn default() -> Nrrd {
        let nan = air_nan();
        Nrrd {
            data: None,
            type_: 0,
            dim: 0,
            axis: vec![Axis::default(); NRRD_DIM_MAX],
            content: None,
            sample_units: None,
            space: 0,
            space_dim: 0,
            space_units: vec![None; NRRD_SPACE_DIM_MAX],
            space_origin: [nan; NRRD_SPACE_DIM_MAX],
            measurement_frame: [[nan; NRRD_SPACE_DIM_MAX]; NRRD_SPACE_DIM_MAX],
            block_size: 0,
            old_min: nan,
            old_max: nan,
            cmt: Vec::new(),
            kvp: Vec::new(),
        }
    }
}

impl Nrrd {
    /// `nrrdElementSize`: the bytes of one sample (0: an invalid type or block size).
    pub fn element_size(&self) -> usize {
        if type_enum().val_check(self.type_) {
            return 0;
        }
        if self.type_ != types::BLOCK {
            return TYPE_SIZE[self.type_ as usize];
        }
        self.block_size
    }

    /// `nrrdElementNumber`: the product of the axis sizes (0: invalid sizes).
    pub fn element_number(&self) -> usize {
        let sizes: Vec<usize> = self.axis[..(self.dim as usize).min(NRRD_DIM_MAX)]
            .iter()
            .map(|a| a.size)
            .collect();
        if size_check(&sizes).is_err() {
            return 0;
        }
        sizes.iter().product()
    }

    /// `nrrdKeyValueSize`.
    pub fn key_value_size(&self) -> usize {
        self.kvp.len()
    }

    /// `nrrdKeyValueAdd`: an empty key is refused (1); a key already there gets the new value.
    pub(crate) fn key_value_add(&mut self, key: &[u8], value: &[u8]) -> i32 {
        if key.is_empty() {
            return 1;
        }
        if let Some(kv) = self.kvp.iter_mut().find(|(k, _)| k == key) {
            kv.1 = value.to_vec();
        } else {
            self.kvp.push((key.to_vec(), value.to_vec()));
        }
        0
    }
}

/// `_nrrdSizeCheck`: Err(message) for a zero size or a product beyond `size_t`.
pub(crate) fn size_check(size: &[usize]) -> Result<(), String> {
    let mut num: usize = 1;
    for (ai, &s) in size.iter().enumerate() {
        if s == 0 {
            return Err(format!("_nrrdSizeCheck: axis {ai} size is zero!"));
        }
        match num.checked_mul(s) {
            Some(n) => num = n,
            None => {
                return Err(
                    "_nrrdSizeCheck: total # of elements too large to be represented in \
                            type size_t, so too large for current architecture"
                        .to_owned(),
                );
            }
        }
    }
    Ok(())
}

/// `nrrdSanity`: NrrdIO's check of the machine it was built for (type sizes, the byte order, the
/// quiet NaN's high bit, the enum tables). Every platform larmorx supports passes it.
pub fn sanity() -> bool {
    true
}

// ------------------------------------------------------------------------------------------
// biff
// ------------------------------------------------------------------------------------------

/// biff's message stack (`biffbiff.c`): per key, the messages in the order they were added. A
/// process has one; NrrdIO adds to key [`NRRD`], and a caller takes the text with
/// [`Biff::get_done`]. Messages are bytes (they carry file names and header lines).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Biff {
    /// Keys in `strcmp` order (`_biffAddKey`), each with its messages.
    entries: Vec<(String, Vec<Vec<u8>>)>,
}

impl Biff {
    pub fn new() -> Biff {
        Biff::default()
    }

    /// `biffAdd`: white space becomes ' ' and trailing white space is dropped (`_biffAddErr`).
    pub fn add(&mut self, key: &str, err: &[u8]) {
        let mut buf: Vec<u8> = err
            .iter()
            .map(|&c| {
                if larmorx_core::cscan::is_c_space(c) {
                    b' '
                } else {
                    c
                }
            })
            .collect();
        while buf.last() == Some(&b' ') {
            buf.pop();
        }
        let idx = match self.entries.iter().position(|(k, _)| k == key) {
            Some(i) => i,
            None => {
                let at = self
                    .entries
                    .iter()
                    .position(|(k, _)| key.as_bytes() < k.as_bytes())
                    .unwrap_or(self.entries.len());
                self.entries.insert(at, (key.to_owned(), Vec::new()));
                at
            }
        };
        self.entries[idx].1.push(buf);
    }

    /// `biffCheck`: the number of messages of `key`.
    pub fn check(&self, key: &str) -> usize {
        self.entries
            .iter()
            .find(|(k, _)| k == key)
            .map_or(0, |(_, m)| m.len())
    }

    /// `biffGet`: `[key] message\n` for each message, the newest first; `None` for a key never
    /// seen (biff then prints `biffGet: WARNING: no information for key "<key>"` on standard
    /// error: [`Biff::get_warning`]).
    pub fn get(&self, key: &str) -> Option<Vec<u8>> {
        let (_, msgs) = self.entries.iter().find(|(k, _)| k == key)?;
        let mut ret = Vec::new();
        for m in msgs.iter().rev() {
            ret.push(b'[');
            ret.extend_from_slice(key.as_bytes());
            ret.extend_from_slice(b"] ");
            ret.extend_from_slice(m);
            ret.push(b'\n');
        }
        Some(ret)
    }

    /// The warning `biffGet` and `biffDone` print for an unknown key.
    pub fn get_warning(func: &str, key: &str) -> String {
        format!("{func}: WARNING: no information for key \"{key}\"\n")
    }

    /// `biffDone`: forgets `key`.
    pub fn done(&mut self, key: &str) {
        self.entries.retain(|(k, _)| k != key);
    }

    /// `biffGetDone`: [`Biff::get`], then [`Biff::done`].
    pub fn get_done(&mut self, key: &str) -> Option<Vec<u8>> {
        let r = self.get(key);
        if r.is_some() {
            self.done(key);
        }
        r
    }
}

// ------------------------------------------------------------------------------------------
// airEnum
// ------------------------------------------------------------------------------------------

/// An `airEnum`: its strings, values, equivalent spellings and case sensitivity.
pub(crate) struct AirEnum {
    pub m: usize,
    pub str_: &'static [&'static str],
    pub val: Option<&'static [i32]>,
    pub str_eqv: Option<&'static [&'static str]>,
    pub val_eqv: &'static [i32],
    pub sense: bool,
}

/// `tolower` in the C locale.
pub(crate) fn to_lower(s: &[u8]) -> Vec<u8> {
    s.iter().map(|c| c.to_ascii_lowercase()).collect()
}

impl AirEnum {
    /// `airEnumUnknown`.
    pub fn unknown(&self) -> i32 {
        self.val.map_or(0, |v| v[0])
    }

    /// `_airEnumIndex`.
    fn index(&self, val: i32) -> usize {
        match self.val {
            Some(v) => (1..=self.m).find(|&ii| v[ii] == val).unwrap_or(0),
            None => {
                if (0..=self.m as i32).contains(&val) {
                    val as usize
                } else {
                    0
                }
            }
        }
    }

    /// `airEnumValCheck`: true when `val` is not a valid value.
    pub fn val_check(&self, val: i32) -> bool {
        self.index(val) == 0
    }

    /// `airEnumStr`.
    pub fn str_of(&self, val: i32) -> &'static str {
        self.str_[self.index(val)]
    }

    /// `airEnumVal`.
    pub fn val_of(&self, s: &[u8]) -> i32 {
        let s = if self.sense { s.to_vec() } else { to_lower(s) };
        if let Some(eqv) = self.str_eqv {
            for (ii, e) in eqv.iter().enumerate() {
                if e.is_empty() {
                    break;
                }
                let mut t = e.as_bytes().to_vec();
                t.truncate(128);
                if !self.sense {
                    t = to_lower(&t);
                }
                if t == s {
                    return self.val_eqv[ii];
                }
            }
        } else {
            for ii in 1..=self.m {
                let mut t = self.str_[ii].as_bytes().to_vec();
                t.truncate(128);
                if !self.sense {
                    t = to_lower(&t);
                }
                if t == s {
                    return self.val.map_or(ii as i32, |v| v[ii]);
                }
            }
        }
        self.unknown()
    }
}

static TYPE_STR: [&str; 12] = [
    "(unknown_type)",
    "signed char",
    "unsigned char",
    "short",
    "unsigned short",
    "int",
    "unsigned int",
    "long long int",
    "unsigned long long int",
    "float",
    "double",
    "block",
];
static TYPE_STR_EQV: [&str; 42] = [
    "signed char",
    "int8",
    "int8_t",
    "uchar",
    "unsigned char",
    "uint8",
    "uint8_t",
    "short",
    "short int",
    "signed short",
    "signed short int",
    "int16",
    "int16_t",
    "ushort",
    "unsigned short",
    "unsigned short int",
    "uint16",
    "uint16_t",
    "int",
    "signed int",
    "int32",
    "int32_t",
    "uint",
    "unsigned int",
    "uint32",
    "uint32_t",
    "longlong",
    "long long",
    "long long int",
    "signed long long",
    "signed long long int",
    "int64",
    "int64_t",
    "ulonglong",
    "unsigned long long",
    "unsigned long long int",
    "uint64",
    "uint64_t",
    "float",
    "double",
    "block",
    "",
];
static TYPE_VAL_EQV: [i32; 41] = [
    1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 3, 3, 4, 4, 4, 4, 4, 5, 5, 5, 5, 6, 6, 6, 6, 7, 7, 7, 7, 7, 7,
    7, 8, 8, 8, 8, 8, 9, 10, 11,
];
static TYPE_ENUM: AirEnum = AirEnum {
    m: 11,
    str_: &TYPE_STR,
    val: None,
    str_eqv: Some(&TYPE_STR_EQV),
    val_eqv: &TYPE_VAL_EQV,
    sense: false,
};

/// `nrrdType`.
pub(crate) fn type_enum() -> &'static AirEnum {
    &TYPE_ENUM
}

/// `airEnumStr(nrrdType, type)`: the type's name (FreeSurfer prints it).
pub fn type_name(type_: i32) -> &'static str {
    TYPE_ENUM.str_of(type_)
}

static ENCODING_STR: [&str; 6] = ["(unknown_encoding)", "raw", "ascii", "hex", "gz", "bz2"];
static ENCODING_STR_EQV: [&str; 11] = [
    "(unknown_encoding)",
    "raw",
    "txt",
    "text",
    "ascii",
    "hex",
    "gz",
    "gzip",
    "bz2",
    "bzip2",
    "",
];
static ENCODING_VAL_EQV: [i32; 10] = [0, 1, 2, 2, 2, 3, 4, 4, 5, 5];
pub(crate) static ENCODING_ENUM: AirEnum = AirEnum {
    m: 5,
    str_: &ENCODING_STR,
    val: None,
    str_eqv: Some(&ENCODING_STR_EQV),
    val_eqv: &ENCODING_VAL_EQV,
    sense: false,
};

static CENTER_STR: [&str; 3] = ["(unknown_center)", "node", "cell"];
pub(crate) static CENTER_ENUM: AirEnum = AirEnum {
    m: 2,
    str_: &CENTER_STR,
    val: None,
    str_eqv: None,
    val_eqv: &[],
    sense: false,
};

static KIND_STR: [&str; 32] = [
    "(unknown_kind)",
    "domain",
    "space",
    "time",
    "list",
    "point",
    "vector",
    "covariant-vector",
    "normal",
    "stub",
    "scalar",
    "complex",
    "2-vector",
    "3-color",
    "RGB-color",
    "HSV-color",
    "XYZ-color",
    "4-color",
    "RGBA-color",
    "3-vector",
    "3-gradient",
    "3-normal",
    "4-vector",
    "quaternion",
    "2D-symmetric-matrix",
    "2D-masked-symmetric-matrix",
    "2D-matrix",
    "2D-masked-matrix",
    "3D-symmetric-matrix",
    "3D-masked-symmetric-matrix",
    "3D-matrix",
    "3D-masked-matrix",
];
static KIND_STR_EQV: [&str; 55] = [
    "domain",
    "space",
    "time",
    "list",
    "point",
    "vector",
    "contravariant-vector",
    "covariant-vector",
    "normal",
    "stub",
    "scalar",
    "complex",
    "2-vector",
    "3-color",
    "RGB-color",
    "RGBcolor",
    "RGB",
    "HSV-color",
    "HSVcolor",
    "HSV",
    "XYZ-color",
    "4-color",
    "RGBA-color",
    "RGBAcolor",
    "RGBA",
    "3-vector",
    "3-gradient",
    "3-normal",
    "4-vector",
    "quaternion",
    "2D-symmetric-matrix",
    "2D-sym-matrix",
    "2D-symmetric-tensor",
    "2D-sym-tensor",
    "2D-masked-symmetric-matrix",
    "2D-masked-sym-matrix",
    "2D-masked-symmetric-tensor",
    "2D-masked-sym-tensor",
    "2D-matrix",
    "2D-tensor",
    "2D-masked-matrix",
    "2D-masked-tensor",
    "3D-symmetric-matrix",
    "3D-sym-matrix",
    "3D-symmetric-tensor",
    "3D-sym-tensor",
    "3D-masked-symmetric-matrix",
    "3D-masked-sym-matrix",
    "3D-masked-symmetric-tensor",
    "3D-masked-sym-tensor",
    "3D-matrix",
    "3D-tensor",
    "3D-masked-matrix",
    "3D-masked-tensor",
    "",
];
static KIND_VAL_EQV: [i32; 54] = [
    1, 2, 3, 4, 5, 6, 6, 7, 8, 9, 10, 11, 12, 13, 14, 14, 14, 15, 15, 15, 16, 17, 18, 18, 18, 19,
    20, 21, 22, 23, 24, 24, 24, 24, 25, 25, 25, 25, 26, 26, 27, 27, 28, 28, 28, 28, 29, 29, 29, 29,
    30, 30, 31, 31,
];
pub(crate) static KIND_ENUM: AirEnum = AirEnum {
    m: 31,
    str_: &KIND_STR,
    val: None,
    str_eqv: Some(&KIND_STR_EQV),
    val_eqv: &KIND_VAL_EQV,
    sense: false,
};

/// `nrrdKind3Gradient`.
pub(crate) const KIND_3GRADIENT: i32 = 20;

/// `nrrdKindSize`: `Err(())` for NrrdIO's `PANIC` (a kind its switch forgot: `3-gradient`).
pub(crate) fn kind_size(kind: i32) -> Result<u32, ()> {
    if !(0 < kind && kind < 32) {
        return Ok(0);
    }
    Ok(match kind {
        1..=8 => 0,
        9 | 10 => 1,
        11 | 12 => 2,
        13..=16 => 3,
        17 | 18 => 4,
        19 | 21 => 3,
        22 | 23 => 4,
        24 => 3,
        25 => 4,
        26 => 4,
        27 => 5,
        28 => 6,
        29 => 7,
        30 => 9,
        31 => 10,
        _ => return Err(()),
    })
}

pub(crate) static FIELD_STR: [&str; 33] = [
    "Ernesto \"Che\" Guevara",
    "#",
    "content",
    "number",
    "type",
    "block size",
    "dimension",
    "space",
    "space dimension",
    "sizes",
    "spacings",
    "thicknesses",
    "axis mins",
    "axis maxs",
    "space directions",
    "centerings",
    "kinds",
    "labels",
    "units",
    "min",
    "max",
    "old min",
    "old max",
    "endian",
    "encoding",
    "line skip",
    "byte skip",
    "key/value",
    "sample units",
    "space units",
    "space origin",
    "measurement frame",
    "data file",
];
static FIELD_STR_EQV: [&str; 47] = [
    "#",
    "content",
    "number",
    "type",
    "block size",
    "blocksize",
    "dimension",
    "space",
    "space dimension",
    "spacedimension",
    "sizes",
    "spacings",
    "thicknesses",
    "axis mins",
    "axismins",
    "axis maxs",
    "axismaxs",
    "space directions",
    "spacedirections",
    "centers",
    "centerings",
    "kinds",
    "labels",
    "units",
    "min",
    "max",
    "old min",
    "oldmin",
    "old max",
    "oldmax",
    "endian",
    "encoding",
    "line skip",
    "lineskip",
    "byte skip",
    "byteskip",
    "sample units",
    "sampleunits",
    "space units",
    "spaceunits",
    "space origin",
    "spaceorigin",
    "measurement frame",
    "measurementframe",
    "data file",
    "datafile",
    "",
];
static FIELD_VAL_EQV: [i32; 46] = [
    1, 2, 3, 4, 5, 5, 6, 7, 8, 8, 9, 10, 11, 12, 12, 13, 13, 14, 14, 15, 15, 16, 17, 18, 19, 20,
    21, 21, 22, 22, 23, 24, 25, 25, 26, 26, 28, 28, 29, 29, 30, 30, 31, 31, 32, 32,
];
pub(crate) static FIELD_ENUM: AirEnum = AirEnum {
    m: 32,
    str_: &FIELD_STR,
    val: None,
    str_eqv: Some(&FIELD_STR_EQV),
    val_eqv: &FIELD_VAL_EQV,
    sense: false,
};

static SPACE_STR: [&str; 13] = [
    "(unknown_space)",
    "right-anterior-superior",
    "left-anterior-superior",
    "left-posterior-superior",
    "right-anterior-superior-time",
    "left-anterior-superior-time",
    "left-posterior-superior-time",
    "scanner-xyz",
    "scanner-xyz-time",
    "3D-right-handed",
    "3D-left-handed",
    "3D-right-handed-time",
    "3D-left-handed-time",
];
/// NrrdIO's spellings as compiled: a missing comma joins "3Drighthanded" and "3D-left-handed"
/// into one string, so every later spelling maps to the value of the one before it (FS-761).
static SPACE_STR_EQV_NRRDIO: [&str; 40] = [
    "(unknown_space)",
    "right-anterior-superior",
    "right anterior superior",
    "rightanteriorsuperior",
    "RAS",
    "left-anterior-superior",
    "left anterior superior",
    "leftanteriorsuperior",
    "LAS",
    "left-posterior-superior",
    "left posterior superior",
    "leftposteriorsuperior",
    "LPS",
    "right-anterior-superior-time",
    "right anterior superior time",
    "rightanteriorsuperiortime",
    "RAST",
    "left-anterior-superior-time",
    "left anterior superior time",
    "leftanteriorsuperiortime",
    "LAST",
    "left-posterior-superior-time",
    "left posterior superior time",
    "leftposteriorsuperiortime",
    "LPST",
    "scanner-xyz",
    "scanner-xyz-time",
    "scanner-xyzt",
    "3D-right-handed",
    "3D right handed",
    "3Drighthanded3D-left-handed",
    "3D left handed",
    "3Dlefthanded",
    "3D-right-handed-time",
    "3D right handed time",
    "3Drighthandedtime",
    "3D-left-handed-time",
    "3D left handed time",
    "3Dlefthandedtime",
    "",
];
/// The spellings as intended (the comma restored).
static SPACE_STR_EQV_FIXED: [&str; 41] = [
    "(unknown_space)",
    "right-anterior-superior",
    "right anterior superior",
    "rightanteriorsuperior",
    "RAS",
    "left-anterior-superior",
    "left anterior superior",
    "leftanteriorsuperior",
    "LAS",
    "left-posterior-superior",
    "left posterior superior",
    "leftposteriorsuperior",
    "LPS",
    "right-anterior-superior-time",
    "right anterior superior time",
    "rightanteriorsuperiortime",
    "RAST",
    "left-anterior-superior-time",
    "left anterior superior time",
    "leftanteriorsuperiortime",
    "LAST",
    "left-posterior-superior-time",
    "left posterior superior time",
    "leftposteriorsuperiortime",
    "LPST",
    "scanner-xyz",
    "scanner-xyz-time",
    "scanner-xyzt",
    "3D-right-handed",
    "3D right handed",
    "3Drighthanded",
    "3D-left-handed",
    "3D left handed",
    "3Dlefthanded",
    "3D-right-handed-time",
    "3D right handed time",
    "3Drighthandedtime",
    "3D-left-handed-time",
    "3D left handed time",
    "3Dlefthandedtime",
    "",
];
static SPACE_VAL_EQV: [i32; 40] = [
    0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 6, 6, 6, 6, 7, 8, 8, 9, 9, 9,
    10, 10, 10, 11, 11, 11, 12, 12, 12,
];
pub(crate) static SPACE_ENUM_NRRDIO: AirEnum = AirEnum {
    m: 12,
    str_: &SPACE_STR,
    val: None,
    str_eqv: Some(&SPACE_STR_EQV_NRRDIO),
    val_eqv: &SPACE_VAL_EQV,
    sense: false,
};
pub(crate) static SPACE_ENUM_FIXED: AirEnum = AirEnum {
    m: 12,
    str_: &SPACE_STR,
    val: None,
    str_eqv: Some(&SPACE_STR_EQV_FIXED),
    val_eqv: &SPACE_VAL_EQV,
    sense: false,
};

/// `airEnumStr(nrrdSpace, space)`.
pub fn space_name(space: i32) -> &'static str {
    SPACE_ENUM_FIXED.str_of(space)
}

static ENDIAN_STR: [&str; 3] = ["(unknown endian)", "little", "big"];
static ENDIAN_VAL: [i32; 3] = [0, AIR_ENDIAN_LITTLE, AIR_ENDIAN_BIG];
pub(crate) static ENDIAN_ENUM: AirEnum = AirEnum {
    m: 2,
    str_: &ENDIAN_STR,
    val: Some(&ENDIAN_VAL),
    str_eqv: None,
    val_eqv: &[],
    sense: false,
};

/// `nrrdSpaceDimension`.
pub(crate) fn space_dimension(space: i32) -> u32 {
    match space {
        1..=3 | 7 | 9 | 10 => 3,
        4..=6 | 8 | 11 | 12 => 4,
        _ => 0,
    }
}

// ------------------------------------------------------------------------------------------
// air's string helpers
// ------------------------------------------------------------------------------------------

/// A C string: the bytes before the first NUL.
pub(crate) fn cstr(s: &[u8]) -> &[u8] {
    match s.iter().position(|&c| c == 0) {
        Some(n) => &s[..n],
        None => s,
    }
}

/// `strstr(hay, needle)`: the position of the first occurrence.
pub(crate) fn strstr(hay: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    hay.windows(needle.len()).position(|w| w == needle)
}

/// `strspn(s, set)`.
pub(crate) fn strspn(s: &[u8], set: &[u8]) -> usize {
    s.iter().take_while(|c| set.contains(c)).count()
}

/// `strcspn(s, set)`.
pub(crate) fn strcspn(s: &[u8], set: &[u8]) -> usize {
    s.iter().take_while(|c| !set.contains(c)).count()
}

/// `airStrtok` over an owned copy (`airStrtokQuoting` is false): the tokens of `s` separated by
/// bytes of `ct`, one at a time.
pub(crate) struct Strtok<'a> {
    s: &'a [u8],
    pos: usize,
    ct: &'a [u8],
}

impl<'a> Strtok<'a> {
    pub fn new(s: &'a [u8], ct: &'a [u8]) -> Strtok<'a> {
        Strtok { s, pos: 0, ct }
    }

    /// `airStrtok(i ? NULL : s, ct, &last)`.
    pub fn next_token(&mut self) -> Option<&'a [u8]> {
        let h0 = self.pos.min(self.s.len());
        if h0 >= self.s.len() {
            return None;
        }
        let h = h0 + strspn(&self.s[h0..], self.ct);
        let e = h + strcspn(&self.s[h..], self.ct);
        let tok = &self.s[h..e];
        self.pos = if e >= self.s.len() { e } else { e + 1 };
        Some(tok)
    }
}

/// `airStrntok`: the number of tokens.
pub(crate) fn strntok(s: &[u8], ct: &[u8]) -> u32 {
    let mut t = Strtok::new(s, ct);
    let mut n = 0;
    while t.next_token().is_some() {
        n += 1;
    }
    n
}

/// `airUnescape`: `\\` and `\n` sequences (with its own end rule).
pub(crate) fn unescape(s: &mut Vec<u8>) {
    let len = s.len();
    if len == 0 {
        return;
    }
    let mut found = false;
    let mut i = 1usize;
    let mut j = 0usize;
    while i < len {
        if s[i - 1] == b'\\' && s[i] == b'\\' {
            s[j] = b'\\';
            i += 1;
            found = true;
        } else if s[i - 1] == b'\\' && s[i] == b'n' {
            s[j] = b'\n';
            i += 1;
            found = true;
        } else {
            s[j] = s[i - 1];
            found = false;
        }
        i += 1;
        j += 1;
    }
    if i == len || !found {
        s[j] = s[len - 1];
        j += 1;
    }
    s.truncate(j);
}

/// `isprint` in the C locale (glibc: a byte of 0x80 or above, a negative `char`, is not).
pub(crate) fn is_print(c: u8) -> bool {
    (0x20..0x7f).contains(&c)
}
