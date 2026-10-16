// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! Object headers, versions 1 and 2, with their continuation chunks
//! (`H5O__prefix_deserialize`, `H5O__chunk_deserialize`, HDF5 1.12.1 `src/H5Ocache.c`).

use crate::error::{Result, corrupt};
use crate::file::File;
use crate::source::{Cursor, UNDEF, verify_checksum};

/// Message type numbers (`H5O_*_ID`).
pub mod msg {
    pub const NULL: u16 = 0x0000;
    pub const DATASPACE: u16 = 0x0001;
    pub const LINK_INFO: u16 = 0x0002;
    pub const DATATYPE: u16 = 0x0003;
    pub const FILL_OLD: u16 = 0x0004;
    pub const FILL: u16 = 0x0005;
    pub const LINK: u16 = 0x0006;
    pub const EXTERNAL_FILES: u16 = 0x0007;
    pub const LAYOUT: u16 = 0x0008;
    pub const BOGUS: u16 = 0x0009;
    pub const GROUP_INFO: u16 = 0x000a;
    pub const PIPELINE: u16 = 0x000b;
    pub const ATTRIBUTE: u16 = 0x000c;
    pub const COMMENT: u16 = 0x000d;
    pub const MTIME_OLD: u16 = 0x000e;
    pub const SHARED_TABLE: u16 = 0x000f;
    pub const CONTINUATION: u16 = 0x0010;
    pub const SYMBOL_TABLE: u16 = 0x0011;
    pub const MTIME: u16 = 0x0012;
    pub const BTREE_K: u16 = 0x0013;
    pub const DRIVER_INFO: u16 = 0x0014;
    pub const ATTRIBUTE_INFO: u16 = 0x0015;
    pub const REFCOUNT: u16 = 0x0016;
    pub const FSINFO: u16 = 0x0017;
    /// One past the last type HDF5 1.12.1 knows (`H5O_UNKNOWN_ID`).
    pub const UNKNOWN: u16 = 0x0018;
}

/// Message flags (`H5O_MSG_FLAG_*`).
pub mod flag {
    pub const CONSTANT: u8 = 0x01;
    pub const SHARED: u8 = 0x02;
    pub const DONTSHARE: u8 = 0x04;
    pub const FAIL_IF_UNKNOWN_AND_OPEN_FOR_WRITE: u8 = 0x08;
    pub const MARK_IF_UNKNOWN: u8 = 0x10;
    pub const WAS_UNKNOWN: u8 = 0x20;
    pub const SHAREABLE: u8 = 0x40;
    pub const FAIL_IF_UNKNOWN_ALWAYS: u8 = 0x80;
}

/// One message of an object header, as stored.
#[derive(Clone, Debug)]
pub struct Message {
    pub type_id: u16,
    pub flags: u8,
    pub data: Vec<u8>,
}

/// A chunk of an object header: where it is and how big (prefix and checksum included).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChunkInfo {
    pub addr: u64,
    pub size: u64,
}

/// An object header as stored.
#[derive(Clone, Debug)]
pub struct ObjectHeader {
    pub addr: u64,
    pub version: u8,
    /// Version-2 header flags (`H5O_HDR_*`); 0 for version 1.
    pub flags: u8,
    /// Access, modification, change and birth times (version 2 with `H5O_HDR_STORE_TIMES`).
    pub times: Option<[u32; 4]>,
    pub messages: Vec<Message>,
    pub chunks: Vec<ChunkInfo>,
    /// The bytes of each chunk, as read (for structure dumps and tests).
    pub images: Vec<Vec<u8>>,
}

impl ObjectHeader {
    /// The first message of a type.
    pub fn find(&self, type_id: u16) -> Option<&Message> {
        self.messages.iter().find(|m| m.type_id == type_id)
    }

    pub fn all(&self, type_id: u16) -> impl Iterator<Item = &Message> {
        self.messages.iter().filter(move |m| m.type_id == type_id)
    }

    pub fn has(&self, type_id: u16) -> bool {
        self.find(type_id).is_some()
    }
}

const OHDR: &[u8; 4] = b"OHDR";
const OCHK: &[u8; 4] = b"OCHK";

/// Reads the object header at `addr` (relative to the base address).
pub(crate) fn read(file: &File, addr: u64) -> Result<ObjectHeader> {
    if addr == UNDEF {
        return Err(corrupt("undefined object header address"));
    }
    let sb = file.superblock();
    let (a, s) = (sb.sizeof_addr, sb.sizeof_size);
    // Enough for any prefix (version 2: 4 + 2 + 16 + 4 + 8).
    let head = file.read_up_to(addr, 34)?;
    let mut oh = ObjectHeader {
        addr,
        version: 0,
        flags: 0,
        times: None,
        messages: Vec::new(),
        chunks: Vec::new(),
        images: Vec::new(),
    };
    let mut pending: Vec<ChunkInfo> = Vec::new();
    let v1_nmesgs;
    if head.len() >= 4 && &head[..4] == OHDR {
        let mut c = Cursor::new(&head, a, s, "an object header prefix");
        c.skip(4)?;
        oh.version = c.u8()?;
        if oh.version != 2 {
            return Err(corrupt("bad object header version number"));
        }
        oh.flags = c.u8()?;
        if oh.flags & !0x3f != 0 {
            return Err(corrupt("unknown object header status flag(s)"));
        }
        if oh.flags & 0x20 != 0 {
            oh.times = Some([c.u32()?, c.u32()?, c.u32()?, c.u32()?]);
        }
        if oh.flags & 0x10 != 0 {
            let max_compact = c.u16()?;
            let min_dense = c.u16()?;
            if max_compact < min_dense {
                return Err(corrupt("bad object header attribute phase change values"));
            }
        }
        let chunk0 = c.uvar(1 << (oh.flags & 0x03))?;
        if chunk0 > 0 && chunk0 < msghdr_size(&oh) as u64 {
            return Err(corrupt("bad object header chunk size"));
        }
        let prefix = c.pos as u64;
        let size = prefix + chunk0 + 4;
        oh.chunks.push(ChunkInfo { addr, size });
        v1_nmesgs = None;
        decode_chunk(file, &mut oh, 0, prefix as usize, &mut pending)?;
    } else {
        let mut c = Cursor::new(&head, a, s, "an object header prefix");
        oh.version = c.u8()?;
        if oh.version != 1 {
            return Err(corrupt("bad object header version number"));
        }
        c.skip(1)?;
        let nmesgs = c.u16()?;
        let _nlink = c.u32()?;
        let chunk0 = u64::from(c.u32()?);
        if (nmesgs > 0 && chunk0 < 8) || (nmesgs == 0 && chunk0 > 0) {
            return Err(corrupt("bad object header chunk size"));
        }
        oh.chunks.push(ChunkInfo {
            addr,
            size: 16 + chunk0,
        });
        v1_nmesgs = Some(usize::from(nmesgs));
        decode_chunk(file, &mut oh, 0, 16, &mut pending)?;
    }
    // Continuation chunks, in the order their messages were found.
    let mut next = 0;
    while next < pending.len() {
        let info = pending[next];
        next += 1;
        oh.chunks.push(info);
        let start = if oh.version > 1 { 4 } else { 0 };
        let chunkno = oh.chunks.len() - 1;
        decode_chunk(file, &mut oh, chunkno, start, &mut pending)?;
    }
    // A version-1 header's message count is checked only with H5_STRICT_FORMAT_CHECKS
    // (`H5O_protect`), which HDF5 builds do not enable.
    let _ = v1_nmesgs;
    Ok(oh)
}

/// Size of a message header (`H5O_SIZEOF_MSGHDR_OH`).
fn msghdr_size(oh: &ObjectHeader) -> usize {
    if oh.version == 1 {
        8
    } else {
        4 + if oh.flags & 0x04 != 0 { 2 } else { 0 }
    }
}

fn decode_chunk(
    file: &File,
    oh: &mut ObjectHeader,
    chunkno: usize,
    start: usize,
    pending: &mut Vec<ChunkInfo>,
) -> Result<()> {
    let info = oh.chunks[chunkno];
    let size = usize::try_from(info.size).map_err(|_| corrupt("object header chunk too large"))?;
    let image = file.read_at(info.addr, size)?;
    let v2 = oh.version > 1;
    if v2 {
        if chunkno > 0 && &image[..4.min(image.len())] != OCHK {
            return Err(corrupt("wrong object header chunk signature"));
        }
        verify_checksum(&image, "object header")?;
    }
    let sb = file.superblock();
    let end = if v2 { size - 4 } else { size };
    let hdr = msghdr_size(oh);
    let mut p = start;
    while p < end {
        if end - p < hdr {
            if v2 {
                // A gap at the end of the chunk.
                break;
            }
            return Err(corrupt("corrupt object header message prefix"));
        }
        let mut c = Cursor::new(
            &image[p..end],
            sb.sizeof_addr,
            sb.sizeof_size,
            "an object header",
        );
        let type_id = if v2 { u16::from(c.u8()?) } else { c.u16()? };
        let msize = usize::from(c.u16()?);
        let flags = c.u8()?;
        if !v2 && msize % 8 != 0 {
            return Err(corrupt("message not aligned"));
        }
        if (flags & flag::SHARED != 0) && (flags & flag::DONTSHARE != 0) {
            return Err(corrupt("bad flag combination for message"));
        }
        if (flags & flag::WAS_UNKNOWN != 0)
            && (flags & flag::FAIL_IF_UNKNOWN_AND_OPEN_FOR_WRITE != 0)
        {
            return Err(corrupt("bad flag combination for message"));
        }
        if (flags & flag::WAS_UNKNOWN != 0) && (flags & flag::MARK_IF_UNKNOWN == 0) {
            return Err(corrupt("bad flag combination for message"));
        }
        if v2 {
            if oh.flags & 0x04 != 0 {
                c.skip(2)?;
            }
        } else {
            c.skip(3)?;
        }
        let data_start = p + c.pos;
        if data_start + msize > end {
            return Err(corrupt("corrupt object header"));
        }
        let data = image[data_start..data_start + msize].to_vec();
        if type_id >= msg::UNKNOWN || type_id == msg::BOGUS {
            if flags & flag::FAIL_IF_UNKNOWN_ALWAYS != 0 {
                return Err(corrupt("unknown message with 'fail if unknown' flag found"));
            }
        } else if type_id == msg::CONTINUATION {
            let mut cc = Cursor::new(
                &data,
                sb.sizeof_addr,
                sb.sizeof_size,
                "a continuation message",
            );
            let caddr = cc.addr()?;
            let csize = cc.length()?;
            if caddr == UNDEF || csize == 0 {
                return Err(corrupt("bad continuation message"));
            }
            pending.push(ChunkInfo {
                addr: caddr,
                size: csize,
            });
        }
        if type_id != msg::NULL {
            oh.messages.push(Message {
                type_id,
                flags,
                data,
            });
        }
        p = data_start + msize;
        if v2 && end - p > 0 && end - p < hdr {
            break;
        }
    }
    oh.images.push(image);
    Ok(())
}
