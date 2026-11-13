// SPDX-License-Identifier: Apache-2.0
//! The colour table a volume carries (spec §7): its deep copy (FS-57) and freeing.

use larmorx_core::defects::FS_57;

use crate::process::{Process, codes};

/// One entry of a colour table.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ColorTableEntry {
    pub name: Vec<u8>,
    /// The colours as ints 0-255 (red, green, blue, alpha).
    pub rgbai: [i32; 4],
    /// The colours as floats 0-1.
    pub rgbaf: [f32; 4],
    pub tissue_type: i32,
    /// The voxel count.
    pub count: i32,
}

/// A colour table: entries (each absent or present), the file name, a version, `idbase`, a
/// tissue-type schema and an optional tissue-type table.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ColorTable {
    pub entries: Vec<Option<ColorTableEntry>>,
    pub fname: Vec<u8>,
    pub version: i32,
    pub idbase: i32,
    pub schema: Vec<u8>,
    pub tissue: Option<Box<ColorTable>>,
}

impl Process {
    /// `CTABdeepCopy(ct)` (§7): the name, version, schema, each entry's name, colours and tissue
    /// type, the tissue-type table copied the same way. FS-57: `idbase` and the counts copied by
    /// default; `reproduce` gives 0 for both. No table: the C-style error and none.
    pub fn ctab_deep_copy(&mut self, ct: Option<&ColorTable>) -> Option<ColorTable> {
        let Some(ct) = ct else {
            self.error_printf(codes::BAD_PARM, b"CTABdeepCopy: ct was NULL");
            return None;
        };
        Some(self.ctab_copy_one(ct))
    }

    fn ctab_copy_one(&mut self, ct: &ColorTable) -> ColorTable {
        let reproduce = self.mode().reproduce();
        let lost = ct.idbase != 0 || ct.entries.iter().flatten().any(|e| e.count != 0);
        if lost {
            if reproduce {
                self.note(
                    &FS_57,
                    "the colour table's copy has idbase 0 and counts 0 (FreeSurfer: 0 and memory contents)",
                );
            } else {
                self.note(&FS_57, "the colour table's idbase and counts copied");
            }
        }
        let entries = ct
            .entries
            .iter()
            .map(|e| {
                e.as_ref().map(|e| ColorTableEntry {
                    count: if reproduce { 0 } else { e.count },
                    ..e.clone()
                })
            })
            .collect();
        let tissue = ct
            .tissue
            .as_deref()
            .map(|t| Box::new(self.ctab_copy_one(t)));
        ColorTable {
            entries,
            fname: ct.fname.clone(),
            version: ct.version,
            idbase: if reproduce { 0 } else { ct.idbase },
            schema: ct.schema.clone(),
            tissue,
        }
    }

    /// `CTABfree(&ct)` (§7): 0, the caller left without it; no table: the C-style error and -4.
    pub fn ctab_free(&mut self, ct: &mut Option<ColorTable>) -> i32 {
        if ct.take().is_none() {
            self.error_printf(codes::BAD_PARM, b"CTABfree: *pct was NULL");
            return codes::BAD_PARM;
        }
        0
    }
}
