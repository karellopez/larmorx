// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! The superblock: where it is (`H5FD_locate_signature`), versions 0 to 3
//! (`H5F__cache_superblock_deserialize`), and the checks `H5F__super_read` makes on open.
//!
//! Follows HDF5 1.12.1 `src/H5Fsuper.c`, `src/H5Fsuper_cache.c` and `src/H5FDint.c`.

use crate::error::{Error, Result, corrupt, unsupported};
use crate::source::{Cursor, Source, UNDEF, verify_checksum};

/// The 8-byte signature that starts every superblock.
pub const SIGNATURE: [u8; 8] = [0x89, b'H', b'D', b'F', b'\r', b'\n', 0x1a, b'\n'];

/// The fields of a superblock that reading needs.
#[derive(Clone, Debug)]
pub struct Superblock {
    pub version: u8,
    /// Absolute offset of the superblock; every address in the file is relative to it.
    pub base: u64,
    pub sizeof_addr: u8,
    pub sizeof_size: u8,
    /// Group B-tree K values (versions 0 and 1).
    pub sym_leaf_k: u16,
    pub btree_k: u16,
    /// The indexed-storage (chunk B-tree) K value (version 1; 32 otherwise).
    pub istore_k: u16,
    /// File consistency flags.
    pub flags: u32,
    /// End of the file's address space, as stored (relative to `base`).
    pub eof: u64,
    /// The superblock extension's object header, or [`UNDEF`].
    pub extension: u64,
    /// The root group's object header.
    pub root: u64,
    /// Versions 0 and 1: the root group's cached symbol table (B-tree, local heap), if the
    /// root symbol table entry caches it.
    pub root_stab: Option<(u64, u64)>,
}

/// `H5FD_locate_signature`: the signature at 0, or at the first power of two from 512 on
/// that is below the file size rounded up to a power of two.
pub(crate) fn locate(src: &Source) -> Result<Option<u64>> {
    let size = src.len();
    let mut maxpow = 64 - size.leading_zeros();
    maxpow = maxpow.max(9);
    for n in 8..maxpow {
        let addr = if n == 8 { 0 } else { 1u64 << n };
        // HDF5 reads past the end of the file as zeros; a short read cannot match.
        let buf = src.read_up_to(addr, SIGNATURE.len())?;
        if buf == SIGNATURE {
            return Ok(Some(addr));
        }
    }
    Ok(None)
}

/// Reads and checks the superblock at absolute offset `at`.
pub(crate) fn read(src: &Source, at: u64) -> Result<Superblock> {
    let prefix = src.read_up_to(at, 8 + 1 + 6)?;
    if prefix.len() < 9 {
        return Err(corrupt("truncated superblock"));
    }
    let version = prefix[8];
    if version > 3 {
        return Err(corrupt("bad superblock version number"));
    }
    let (sizeof_addr, sizeof_size) = if version < 2 {
        (
            *prefix
                .get(13)
                .ok_or_else(|| corrupt("truncated superblock"))?,
            *prefix
                .get(14)
                .ok_or_else(|| corrupt("truncated superblock"))?,
        )
    } else {
        (
            *prefix
                .get(9)
                .ok_or_else(|| corrupt("truncated superblock"))?,
            *prefix
                .get(10)
                .ok_or_else(|| corrupt("truncated superblock"))?,
        )
    };
    for (n, what) in [
        (sizeof_addr, "bad byte number in an address"),
        (sizeof_size, "bad byte number for object size"),
    ] {
        if !matches!(n, 2 | 4 | 8 | 16 | 32) {
            return Err(corrupt(what));
        }
        if n > 8 {
            return Err(unsupported(format!("{n}-byte addresses or lengths")));
        }
    }
    let (a, s) = (usize::from(sizeof_addr), usize::from(sizeof_size));
    // H5F_SUPERBLOCK_FIXED_SIZE (signature + version) + H5F_SUPERBLOCK_VARLEN_SIZE.
    let total = 9 + match version {
        0 => 7 + 2 + 2 + 4 + 4 * a + (s + a + 4 + 4 + 16),
        1 => 7 + 2 + 2 + 4 + 4 + 4 * a + (s + a + 4 + 4 + 16),
        _ => 2 + 1 + 4 * a + 4,
    };
    let image = src.read_up_to(at, total)?;
    if image.len() < total {
        return Err(corrupt("truncated superblock"));
    }
    let mut c = Cursor::new(&image, sizeof_addr, sizeof_size, "the superblock");
    c.skip(9)?;
    let mut sb = Superblock {
        version,
        base: 0,
        sizeof_addr,
        sizeof_size,
        sym_leaf_k: 4,
        btree_k: 16,
        istore_k: 32,
        flags: 0,
        eof: 0,
        extension: UNDEF,
        root: UNDEF,
        root_stab: None,
    };
    let stored_base;
    if version < 2 {
        let freespace_vers = c.u8()?;
        if freespace_vers != 0 {
            return Err(corrupt("bad free space version number"));
        }
        let root_sym_vers = c.u8()?;
        if root_sym_vers != 0 {
            return Err(corrupt("bad object directory version number"));
        }
        c.skip(1)?;
        let shared_vers = c.u8()?;
        if shared_vers != 0 {
            return Err(corrupt("bad shared-header format version number"));
        }
        c.skip(3)?; // sizes (already read) and a reserved byte
        sb.sym_leaf_k = c.u16()?;
        if sb.sym_leaf_k == 0 {
            return Err(corrupt("bad symbol table leaf node 1/2 rank"));
        }
        sb.btree_k = c.u16()?;
        if sb.btree_k == 0 {
            return Err(corrupt("bad 1/2 rank for btree internal nodes"));
        }
        sb.flags = c.u32()?;
        if version == 1 {
            sb.istore_k = c.u16()?;
            if sb.istore_k == 0 {
                return Err(corrupt("bad 1/2 rank for btree internal nodes"));
            }
            c.skip(2)?;
        }
        stored_base = c.addr()?;
        sb.extension = c.addr()?;
        sb.eof = c.addr()?;
        let _driver = c.addr()?;
        // Root group symbol table entry (H5G_ent_decode).
        let _name_off = c.length()?;
        sb.root = c.addr()?;
        let cache_type = c.u32()?;
        c.skip(4)?;
        let scratch = c.bytes(16)?;
        if cache_type == 1 {
            let mut sc = Cursor::new(scratch, sizeof_addr, sizeof_size, "the root symbol entry");
            sb.root_stab = Some((sc.addr()?, sc.addr()?));
        }
    } else {
        c.skip(2)?;
        sb.flags = u32::from(c.u8()?);
        stored_base = c.addr()?;
        sb.extension = c.addr()?;
        sb.eof = c.addr()?;
        sb.root = c.addr()?;
        verify_checksum(&image, "superblock")?;
    }
    // H5F__super_read: addresses are relative to where the superblock is; the stored end of
    // file moves with it.
    sb.base = at;
    if stored_base != at {
        sb.eof = sb.eof.wrapping_sub(stored_base.wrapping_sub(at));
    }
    // A file shorter than the stored end of file is truncated.
    let eof = src.len().saturating_sub(sb.base);
    if eof + sb.base < sb.eof {
        return Err(Error::Corrupt(format!(
            "truncated file: eof = {eof}, sblock->base_addr = {}, stored_eof = {}",
            sb.base, sb.eof
        )));
    }
    Ok(sb)
}
