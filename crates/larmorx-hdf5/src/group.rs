// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! The links of a group, in its three storage forms (HDF5 1.12.1): old-style symbol tables
//! (`src/H5Gstab.c`, `src/H5Gnode.c`), compact link messages (`src/H5Gcompact.c`) and dense
//! storage in a fractal heap indexed by a version-2 B-tree (`src/H5Gdense.c`).
//!
//! Links are listed in name order (byte-wise, as `strcmp` orders them): the order HDF5's
//! iteration by name gives (`H5_INDEX_NAME`, `H5_ITER_INC`), which h5py's `Group.keys()` uses.

use crate::btree;
use crate::error::{Result, corrupt};
use crate::file::File;
use crate::heap;
use crate::message::{self, Link, LinkTarget};
use crate::object::{ObjectHeader, msg};
use crate::source::{Cursor, UNDEF};

/// How a group stores its links.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Storage {
    SymbolTable,
    Compact,
    Dense,
}

/// The storage form of a group's object header, or `None` if it is not a group
/// (`H5O__group_isa`: a symbol table or a link info message).
pub(crate) fn storage(file: &File, oh: &ObjectHeader) -> Result<Option<Storage>> {
    if oh.has(msg::SYMBOL_TABLE) {
        return Ok(Some(Storage::SymbolTable));
    }
    if let Some(m) = oh.find(msg::LINK_INFO) {
        let sb = file.superblock();
        let li = message::link_info(&mut Cursor::new(
            &m.data,
            sb.sizeof_addr,
            sb.sizeof_size,
            "a link info message",
        ))?;
        return Ok(Some(if li.fheap_addr != UNDEF {
            Storage::Dense
        } else {
            Storage::Compact
        }));
    }
    Ok(None)
}

/// Every link of the group whose object header is `oh`, in name order.
pub(crate) fn links(file: &File, oh: &ObjectHeader) -> Result<Vec<Link>> {
    let sb = file.superblock();
    let (a, s) = (sb.sizeof_addr, sb.sizeof_size);
    let mut out = if let Some(m) = oh.find(msg::SYMBOL_TABLE) {
        let (btree_addr, heap_addr) =
            message::symbol_table(&mut Cursor::new(&m.data, a, s, "a symbol table message"))?;
        symbol_table_links(file, btree_addr, heap_addr)?
    } else if let Some(m) = oh.find(msg::LINK_INFO) {
        let li = message::link_info(&mut Cursor::new(&m.data, a, s, "a link info message"))?;
        if li.fheap_addr != UNDEF {
            dense_links(file, li.fheap_addr, li.name_bt2_addr)?
        } else {
            oh.all(msg::LINK)
                .map(|m| message::link(&mut Cursor::new(&m.data, a, s, "a link message")))
                .collect::<Result<Vec<_>>>()?
        }
    } else {
        return Err(corrupt("not a group"));
    };
    out.sort_by(|x, y| x.name.cmp(&y.name));
    Ok(out)
}

fn symbol_table_links(file: &File, btree_addr: u64, heap_addr: u64) -> Result<Vec<Link>> {
    let sb = file.superblock();
    let (a, s) = (sb.sizeof_addr, sb.sizeof_size);
    let heap_data = heap::local_heap(file, heap_addr)?;
    let leaves = btree::v1_leaves(file, btree_addr, 0, usize::from(s))?;
    let mut out = Vec::new();
    let entry_size = usize::from(s) + usize::from(a) + 4 + 4 + 16;
    for leaf in leaves {
        let head = file.read_at(leaf.child, 8)?;
        let mut c = Cursor::new(&head, a, s, "a symbol table node");
        c.magic(b"SNOD")?;
        if c.u8()? != 1 {
            return Err(corrupt("bad symbol table node version"));
        }
        c.skip(1)?;
        let n = usize::from(c.u16()?);
        let body = file.read_at(leaf.child + 8, n * entry_size)?;
        let mut c = Cursor::new(&body, a, s, "a symbol table node");
        for _ in 0..n {
            let name_off = c.length()?;
            let obj = c.addr()?;
            let cache_type = c.u32()?;
            c.skip(4)?;
            let scratch = c.bytes(16)?;
            let name = heap::heap_string(&heap_data, name_off)?;
            let target = if cache_type == 2 {
                let off = u32::from_le_bytes(scratch[..4].try_into().unwrap());
                LinkTarget::Soft(
                    String::from_utf8_lossy(&heap::heap_string(&heap_data, u64::from(off))?)
                        .into_owned(),
                )
            } else {
                LinkTarget::Hard(obj)
            };
            out.push(Link {
                name,
                corder: None,
                utf8: false,
                target,
            });
        }
    }
    Ok(out)
}

fn dense_links(file: &File, fheap_addr: u64, name_bt2_addr: u64) -> Result<Vec<Link>> {
    let sb = file.superblock();
    let (a, s) = (sb.sizeof_addr, sb.sizeof_size);
    let fh = heap::FractalHeap::open(file, fheap_addr)?;
    let hdr = btree::v2_header(file, name_bt2_addr, 5)?;
    let mut out = Vec::new();
    for rec in btree::v2_records(file, &hdr)? {
        // H5G_dense_bt2_name_rec_t: the name's hash (4 bytes) and the heap ID.
        let id = rec
            .get(4..4 + fh.id_len())
            .ok_or_else(|| corrupt("bad link name index record"))?;
        let data = fh.read(file, id)?;
        out.push(message::link(&mut Cursor::new(
            &data,
            a,
            s,
            "a link message",
        ))?);
    }
    Ok(out)
}
