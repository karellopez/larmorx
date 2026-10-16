// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! The messages the writer stores, encoded as HDF5 1.12.1 encodes them for a file whose
//! library-version bounds are 1.8 (the `encode` and `raw_size` callbacks of `src/H5Olinfo.c`,
//! `src/H5Oginfo.c`, `src/H5Olink.c`, `src/H5Osdspace.c` (version 2), `src/H5Odtype.c`
//! (version 1), `src/H5Ofill.c` (version 3), `src/H5Olayout.c` (version 3), `src/H5Opline.c`
//! (version 2), `src/H5Ocont.c`).

use crate::source::UNDEF;

/// The element types the writer stores.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WType {
    /// `H5T_NATIVE_FLOAT` (IEEE binary32, little-endian).
    F32,
    /// `H5T_NATIVE_DOUBLE`.
    F64,
    /// A variable-length string of `C_S1` (ASCII, NUL-terminated).
    VlenString,
}

impl WType {
    /// Size of a stored element.
    pub(crate) fn size(self) -> usize {
        match self {
            WType::F32 => 4,
            WType::F64 => 8,
            WType::VlenString => 16,
        }
    }
}

/// Storage layout of a dataset.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum WLayout {
    Contiguous {
        addr: u64,
        size: u64,
    },
    /// Chunk dimensions, the element size last.
    Chunked {
        addr: u64,
        dims: Vec<u32>,
    },
}

/// A message's content (the "native" form HDF5 keeps until it encodes the header).
#[derive(Clone, Debug)]
pub(crate) enum Native {
    Null,
    /// A message whose room is taken and whose content the caller is about to set.
    Placeholder,
    Cont {
        addr: u64,
        size: u64,
        chunkno: usize,
    },
    LinkInfo {
        fheap: u64,
        name_bt2: u64,
    },
    GroupInfo,
    Link {
        name: Vec<u8>,
        addr: u64,
    },
    Dataspace {
        dims: Vec<u64>,
    },
    Datatype(WType),
    /// Fill value with no value stored: allocation time and fill time.
    Fill {
        alloc_time: u8,
        fill_time: u8,
    },
    Layout(WLayout),
    /// The deflate filter (optional) at a level.
    Pline {
        level: u32,
    },
}

impl Native {
    /// The message type number.
    pub(crate) fn type_id(&self) -> u8 {
        match self {
            Native::Null | Native::Placeholder => 0x00,
            Native::Dataspace { .. } => 0x01,
            Native::LinkInfo { .. } => 0x02,
            Native::Datatype(_) => 0x03,
            Native::Fill { .. } => 0x05,
            Native::Link { .. } => 0x06,
            Native::Layout(_) => 0x08,
            Native::GroupInfo => 0x0a,
            Native::Pline { .. } => 0x0b,
            Native::Cont { .. } => 0x10,
        }
    }

    /// The encoded size (`raw_size`).
    pub(crate) fn raw_size(&self) -> usize {
        self.encode().len()
    }

    /// The encoded message.
    pub(crate) fn encode(&self) -> Vec<u8> {
        let mut v = Vec::new();
        let addr = |v: &mut Vec<u8>, a: u64| v.extend_from_slice(&a.to_le_bytes());
        match self {
            Native::Null | Native::Placeholder => {}
            Native::Cont { addr: a, size, .. } => {
                addr(&mut v, *a);
                v.extend_from_slice(&size.to_le_bytes());
            }
            Native::LinkInfo { fheap, name_bt2 } => {
                v.push(0); // version
                v.push(0); // flags: creation order neither tracked nor indexed
                addr(&mut v, *fheap);
                addr(&mut v, *name_bt2);
            }
            Native::GroupInfo => {
                v.push(0); // version
                v.push(0); // flags: default phase change and estimates, not stored
            }
            Native::Link { name, addr: a } => {
                v.push(1); // version
                let len = name.len();
                let size_bits: u8 = if len > 0xffff_ffff {
                    3
                } else if len > 0xffff {
                    2
                } else if len > 0xff {
                    1
                } else {
                    0
                };
                v.push(size_bits);
                let n = 1usize << size_bits;
                v.extend_from_slice(&(len as u64).to_le_bytes()[..n]);
                v.extend_from_slice(name);
                addr(&mut v, *a);
            }
            Native::Dataspace { dims } => {
                v.push(2); // version
                v.push(dims.len() as u8);
                v.push(0x01); // the maximum dimensions are stored
                v.push(1); // simple
                for d in dims {
                    v.extend_from_slice(&d.to_le_bytes());
                }
                for d in dims {
                    v.extend_from_slice(&d.to_le_bytes());
                }
            }
            Native::Datatype(t) => encode_dtype(&mut v, *t),
            Native::Fill {
                alloc_time,
                fill_time,
            } => {
                v.push(3); // version
                v.push((alloc_time & 0x03) | ((fill_time & 0x03) << 2));
            }
            Native::Layout(l) => {
                v.push(3); // version
                match l {
                    WLayout::Contiguous { addr: a, size } => {
                        v.push(1);
                        addr(&mut v, *a);
                        v.extend_from_slice(&size.to_le_bytes());
                    }
                    WLayout::Chunked { addr: a, dims } => {
                        v.push(2);
                        v.push(dims.len() as u8);
                        addr(&mut v, *a);
                        for d in dims {
                            v.extend_from_slice(&d.to_le_bytes());
                        }
                    }
                }
            }
            Native::Pline { level } => {
                v.push(2); // version
                v.push(1); // one filter
                v.extend_from_slice(&1u16.to_le_bytes()); // deflate
                v.extend_from_slice(&1u16.to_le_bytes()); // H5Z_FLAG_OPTIONAL
                v.extend_from_slice(&1u16.to_le_bytes()); // one parameter
                v.extend_from_slice(&level.to_le_bytes());
            }
        }
        v
    }
}

/// `H5O__dtype_encode_helper` for the writer's types (datatype message version 1).
fn encode_dtype(v: &mut Vec<u8>, t: WType) {
    match t {
        WType::F32 | WType::F64 => {
            let (size, sign, epos, esize, msize, bias): (u32, u8, u8, u8, u8, u32) =
                if t == WType::F64 {
                    (8, 63, 52, 11, 52, 1023)
                } else {
                    (4, 31, 23, 8, 23, 127)
                };
            // class 1 (float), version 1; flags: little-endian, no padding, implied MSB
            // normalisation (0x20), the sign bit's position.
            v.extend_from_slice(&[0x11, 0x20, sign, 0x00]);
            v.extend_from_slice(&size.to_le_bytes());
            v.extend_from_slice(&0u16.to_le_bytes()); // bit offset
            v.extend_from_slice(&((size * 8) as u16).to_le_bytes()); // precision
            v.extend_from_slice(&[epos, esize, 0, msize]);
            v.extend_from_slice(&bias.to_le_bytes());
        }
        WType::VlenString => {
            // class 9 (variable-length), version 1; type string, NUL-terminated, ASCII.
            v.extend_from_slice(&[0x19, 0x01, 0x00, 0x00]);
            v.extend_from_slice(&16u32.to_le_bytes());
            // The base type: H5T_NATIVE_UCHAR (class 0, version 1, unsigned, 1 byte, 8 bits).
            v.extend_from_slice(&[0x10, 0x00, 0x00, 0x00]);
            v.extend_from_slice(&1u32.to_le_bytes());
            v.extend_from_slice(&0u16.to_le_bytes());
            v.extend_from_slice(&8u16.to_le_bytes());
        }
    }
}

/// A link info message of an empty new-style group (`H5G__obj_create_real`).
pub(crate) fn empty_link_info() -> Native {
    Native::LinkInfo {
        fheap: UNDEF,
        name_bt2: UNDEF,
    }
}
