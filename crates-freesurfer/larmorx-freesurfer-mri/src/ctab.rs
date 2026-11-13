// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The colour table's structure (`include/colortab.h`: `COLOR_TABLE`, `COLOR_TABLE_ENTRY`) that an
//! `MRI` carries, with the two functions the structure itself needs (`utils/colortab.cpp`:
//! `CTABfree`, `CTABdeepCopy`). Reading, writing and the lookups stay in `fs.labels`
//! (`larmorx-freesurfer-io`), on this structure.

use std::io::Write;

use larmorx_core::defects::FS_57;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, NO_ERROR};

/// `COLOR_TABLE_ENTRY` (`CTE`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ColorTableEntry {
    /// `name` (`STRLEN` in C): the C string's bytes (without its NUL; any bytes a file holds).
    pub name: Vec<u8>,
    pub ri: i32,
    pub gi: i32,
    pub bi: i32,
    pub ai: i32,
    pub rf: f32,
    pub gf: f32,
    pub bf: f32,
    pub af: f32,
    pub tissue_type: i32,
    /// `count`: the number of voxels of this entry.
    pub count: i32,
}

/// `COLOR_TABLE` (`CT`): `entries` has `nentries` places, some empty (`NULL`); each entry is boxed, as
/// C's array of pointers to entries (8 bytes for an empty place).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ColorTable {
    pub entries: Vec<Option<Box<ColorTableEntry>>>,
    /// `fname` (`STRLEN` in C): the C string's bytes.
    pub fname: Vec<u8>,
    pub version: i32,
    /// `idbase`: added to the structure number when writing.
    pub idbase: i32,
    pub tissue_type_schema: String,
    pub ctab_tissue_type: Option<Box<ColorTable>>,
}

impl ColorTable {
    /// `nentries`.
    pub fn nentries(&self) -> i32 {
        self.entries.len() as i32
    }
}

/// `CTABfree(&ct)`: the table and its tissue-type table freed, `ct` `NULL`; `NO_ERROR`, or
/// `ERROR_BADPARM` after `CTABfree: *pct was NULL` (as an error) for a `NULL` table.
pub fn ctab_free(g: &mut Globals, err: &mut dyn Write, ct: &mut Option<ColorTable>) -> i32 {
    if ct.is_none() {
        log::error_printf(g, err, ERROR_BADPARM, "CTABfree: *pct was NULL");
        return ERROR_BADPARM;
    }
    *ct = None;
    NO_ERROR
}

/// `CTABdeepCopy(ct)`: a copy of the table: its file name, version and tissue-type schema, each
/// entry's name, colours (integer and float) and tissue type, and the tissue-type table (copied the
/// same way). FreeSurfer does not copy `idbase` (the copy has 0) nor the entries' `count` (memory
/// contents): FS-57, by default both are copied, with `reproduce` 0 (the count filled with 0, for
/// FreeSurfer's memory contents). `None` (`NULL`) after `CTABdeepCopy: ct was NULL` for `None`.
pub fn ctab_deep_copy(
    g: &mut Globals,
    err: &mut dyn Write,
    ct: Option<&ColorTable>,
) -> Option<ColorTable> {
    let Some(ct) = ct else {
        log::error_printf(g, err, ERROR_BADPARM, "CTABdeepCopy: ct was NULL");
        return None;
    };
    let reproduce = g.notices.reproduce();
    if ct.idbase != 0 {
        g.notices.add(
            &FS_57,
            format!("a colour table's copy without its idbase {}", ct.idbase),
        );
    }
    let mut copy = ColorTable {
        entries: Vec::with_capacity(ct.entries.len()),
        fname: ct.fname.clone(),
        version: ct.version,
        idbase: if reproduce { 0 } else { ct.idbase },
        tissue_type_schema: String::new(),
        ctab_tissue_type: None,
    };
    for e in &ct.entries {
        copy.entries.push(e.as_ref().map(|e| {
            if e.count != 0 {
                g.notices.add(
                    &FS_57,
                    "a colour table's copy without its entries' voxel counts",
                );
            }
            Box::new(ColorTableEntry {
                name: e.name.clone(),
                ri: e.ri,
                gi: e.gi,
                bi: e.bi,
                ai: e.ai,
                rf: e.rf,
                gf: e.gf,
                bf: e.bf,
                af: e.af,
                tissue_type: e.tissue_type,
                count: if reproduce { 0 } else { e.count },
            })
        }));
    }
    if let Some(t) = &ct.ctab_tissue_type {
        copy.ctab_tissue_type = ctab_deep_copy(g, err, Some(t)).map(Box::new);
    }
    copy.tissue_type_schema = ct.tissue_type_schema.clone();
    Some(copy)
}
