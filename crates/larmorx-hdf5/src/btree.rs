// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! B-trees, read only: version 1 (`H5B__cache_deserialize`, HDF5 1.12.1 `src/H5Bcache.c`),
//! whose group nodes (type 0) index old-style groups and whose chunk nodes (type 1) index the
//! chunks of layouts 1 to 3; version 2 (`src/H5B2cache.c`, `src/H5B2hdr.c`), which indexes
//! dense groups' link names (type 5) and creation orders (type 6), huge fractal-heap objects
//! (types 1 to 4) and the chunks of layout 4 (types 10 and 11).

use crate::error::{Result, corrupt};
use crate::file::File;
use crate::source::{Cursor, UNDEF, limit_enc_size, verify_checksum};

/// One child of a version-1 B-tree leaf: its key (the bytes before it) and its address.
#[derive(Clone, Debug)]
pub struct V1Entry {
    pub(crate) key: Vec<u8>,
    pub(crate) child: u64,
}

/// Walks a version-1 B-tree of `node_type` from `addr`, returning every leaf entry in key
/// order. `key_size` is the size of a key.
pub fn v1_leaves(file: &File, addr: u64, node_type: u8, key_size: usize) -> Result<Vec<V1Entry>> {
    let mut out = Vec::new();
    let mut seen = 0usize;
    walk_v1(file, addr, node_type, key_size, None, &mut out, &mut seen)?;
    Ok(out)
}

fn walk_v1(
    file: &File,
    addr: u64,
    node_type: u8,
    key_size: usize,
    expected_level: Option<u8>,
    out: &mut Vec<V1Entry>,
    seen: &mut usize,
) -> Result<()> {
    *seen += 1;
    if *seen > 1 << 20 {
        return Err(corrupt("B-tree too large or cyclic"));
    }
    let sb = file.superblock();
    let (a, s) = (sb.sizeof_addr, sb.sizeof_size);
    let head = file.read_at(addr, 8 + 2 * usize::from(a))?;
    let mut c = Cursor::new(&head, a, s, "a B-tree node");
    c.magic(b"TREE")?;
    let t = c.u8()?;
    if t != node_type {
        return Err(corrupt("incorrect B-tree node type"));
    }
    let level = c.u8()?;
    if let Some(l) = expected_level
        && l != level
    {
        return Err(corrupt("B-tree node has an unexpected level"));
    }
    let nused = usize::from(c.u16()?);
    let body_len = (nused + 1) * key_size + nused * usize::from(a);
    let body = file.read_at(addr + head.len() as u64, body_len)?;
    let mut c = Cursor::new(&body, a, s, "a B-tree node");
    let mut entries = Vec::with_capacity(nused);
    for _ in 0..nused {
        let key = c.bytes(key_size)?.to_vec();
        let child = c.addr()?;
        entries.push(V1Entry { key, child });
    }
    if level == 0 {
        out.extend(entries);
    } else {
        for e in entries {
            walk_v1(
                file,
                e.child,
                node_type,
                key_size,
                Some(level - 1),
                out,
                seen,
            )?;
        }
    }
    Ok(())
}

/// A version-2 B-tree's header.
#[derive(Clone, Debug)]
pub(crate) struct V2Header {
    pub(crate) btype: u8,
    pub(crate) node_size: usize,
    pub(crate) rrec_size: usize,
    pub(crate) depth: u16,
    pub(crate) root: u64,
    pub(crate) root_nrec: u64,
    pub(crate) total_nrec: u64,
    /// Bytes encoding a node's number of records.
    max_nrec_size: usize,
    /// Per depth: bytes encoding the total number of records below a node.
    cum_max_nrec_size: Vec<usize>,
}

pub(crate) fn v2_header(file: &File, addr: u64, btype: u8) -> Result<V2Header> {
    let sb = file.superblock();
    let (a, s) = (sb.sizeof_addr, sb.sizeof_size);
    let size = 4 + 1 + 1 + 4 + 4 + 2 + 2 + 1 + 1 + usize::from(a) + 2 + usize::from(s);
    let image = file.read_at(addr, size)?;
    verify_checksum(&image, "v2 B-tree header")?;
    let mut c = Cursor::new(&image, a, s, "a v2 B-tree header");
    c.magic(b"BTHD")?;
    if c.u8()? != 0 {
        return Err(corrupt("wrong B-tree header version"));
    }
    let t = c.u8()?;
    if t != btype {
        return Err(corrupt("incorrect B-tree type"));
    }
    let node_size = c.u32()? as usize;
    let rrec_size = usize::from(c.u16()?);
    let depth = c.u16()?;
    let _split = c.u8()?;
    let _merge = c.u8()?;
    let root = c.addr()?;
    let root_nrec = u64::from(c.u16()?);
    let total_nrec = c.length()?;
    if node_size <= 10 || rrec_size == 0 {
        return Err(corrupt("bad v2 B-tree node or record size"));
    }
    // H5B2__hdr_init's node information.
    let leaf_max = (node_size - 10) / rrec_size;
    let max_nrec_size = limit_enc_size(leaf_max as u64);
    let mut cum_max = vec![leaf_max as u64];
    let mut cum_size = vec![0usize];
    for d in 1..=usize::from(depth) {
        let ptr = usize::from(a) + max_nrec_size + cum_size[d - 1];
        if node_size < 10 + ptr {
            return Err(corrupt("v2 B-tree nodes too small for their depth"));
        }
        let max_nrec = ((node_size - (10 + ptr)) / (rrec_size + ptr)) as u64;
        let cm = (max_nrec + 1) * cum_max[d - 1] + max_nrec;
        cum_max.push(cm);
        cum_size.push(limit_enc_size(cm));
    }
    Ok(V2Header {
        btype,
        node_size,
        rrec_size,
        depth,
        root,
        root_nrec,
        total_nrec,
        max_nrec_size,
        cum_max_nrec_size: cum_size,
    })
}

/// Every record of a version-2 B-tree, in key order.
pub(crate) fn v2_records(file: &File, hdr: &V2Header) -> Result<Vec<Vec<u8>>> {
    let mut out = Vec::new();
    if hdr.root == UNDEF || hdr.total_nrec == 0 {
        return Ok(out);
    }
    walk_v2(
        file,
        hdr,
        hdr.root,
        hdr.root_nrec as usize,
        hdr.depth,
        &mut out,
    )?;
    Ok(out)
}

fn walk_v2(
    file: &File,
    hdr: &V2Header,
    addr: u64,
    nrec: usize,
    depth: u16,
    out: &mut Vec<Vec<u8>>,
) -> Result<()> {
    let sb = file.superblock();
    let (a, s) = (sb.sizeof_addr, sb.sizeof_size);
    let image = file.read_at(addr, hdr.node_size)?;
    let mut c = Cursor::new(&image, a, s, "a v2 B-tree node");
    c.magic(if depth == 0 { b"BTLF" } else { b"BTIN" })?;
    if c.u8()? != 0 {
        return Err(corrupt("wrong B-tree node version"));
    }
    if c.u8()? != hdr.btype {
        return Err(corrupt("incorrect B-tree type"));
    }
    let mut records = Vec::with_capacity(nrec);
    for _ in 0..nrec {
        records.push(c.bytes(hdr.rrec_size)?.to_vec());
    }
    let mut children = Vec::new();
    if depth > 0 {
        for _ in 0..=nrec {
            let child = c.addr()?;
            let child_nrec = c.uvar(hdr.max_nrec_size)? as usize;
            if depth > 1 {
                c.uvar(hdr.cum_max_nrec_size[usize::from(depth) - 1])?;
            }
            children.push((child, child_nrec));
        }
    }
    // The checksum covers the node's used part.
    let used = c.pos + 4;
    verify_checksum(&image[..used], "v2 B-tree node")?;
    if depth == 0 {
        out.extend(records);
        return Ok(());
    }
    for (i, (child, child_nrec)) in children.into_iter().enumerate() {
        walk_v2(file, hdr, child, child_nrec, depth - 1, out)?;
        if i < records.len() {
            out.push(records[i].clone());
        }
    }
    Ok(())
}
