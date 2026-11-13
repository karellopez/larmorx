// SPDX-License-Identifier: Apache-2.0 AND LicenseRef-CTN
//! CTN 3.0.6's data dictionary (`dcmdict.c`'s tables, generated into `dict_table.rs` by
//! `validation/probes/fs-dicom/gen_dict.sh`) and `DCM_LookupElement`.

use super::cond::{
    Arg, CondStack, Condition, DCM_NORMAL, DCM_UNRECOGNIZEDELEMENT, DCM_UNRECOGNIZEDGROUP,
};
use super::dict_table::GROUPS;
use super::{Dialect, Tag, Vr, tag_element, tag_group};
use larmorx_core::cfmt::Streams;
use larmorx_core::defects::Notices;

/// One dictionary entry (`DCMDICT`): a tag, its representation's number, its description.
#[derive(Clone, Copy, Debug)]
pub struct Entry {
    pub tag: Tag,
    pub vr: u32,
    pub description: &'static str,
}

/// The entry of `tag`: its group found in the dictionary's group order, then its entry by a
/// linear search (the first match).
pub fn find(tag: Tag) -> Result<&'static Entry, bool> {
    let group = tag_group(tag);
    let Some((_, entries)) = GROUPS.iter().find(|(g, _)| *g == group) else {
        return Err(false);
    };
    entries.iter().find(|e| e.tag == tag).ok_or(true)
}

/// `DCM_LookupElement`: the representation and description of `tag`. An unknown group's
/// length element (element 0) is "Unknown group length", UL; an unknown group or element is UN
/// with an empty description, and its condition (UNRECOGNIZEDGROUP or UNRECOGNIZEDELEMENT) is
/// pushed on `stack`.
pub fn lookup_element(
    tag: Tag,
    dialect: &dyn Dialect,
    stack: &mut CondStack,
    out: &mut dyn Streams,
    notices: &mut Notices,
) -> (Vr, String, Condition) {
    match find(tag) {
        Ok(e) => (
            Vr::from_number(e.vr).unwrap_or(Vr::Un),
            e.description.to_owned(),
            DCM_NORMAL,
        ),
        Err(false) if tag_element(tag) == 0 => {
            (Vr::Ul, "Unknown group length".to_owned(), DCM_NORMAL)
        }
        Err(false) => {
            let c = stack.push(
                DCM_UNRECOGNIZEDGROUP,
                dialect.message(DCM_UNRECOGNIZEDGROUP),
                &[
                    Arg::Int(tag_group(tag) as i32),
                    Arg::Str(b"DCM_LookupElement"),
                ],
                out,
                notices,
            );
            (Vr::Un, String::new(), c)
        }
        Err(true) => {
            let c = stack.push(
                DCM_UNRECOGNIZEDELEMENT,
                dialect.message(DCM_UNRECOGNIZEDELEMENT),
                &[
                    Arg::Int(tag_group(tag) as i32),
                    Arg::Int(tag_element(tag) as i32),
                    Arg::Str(b"DCM_LookupElement"),
                ],
                out,
                notices,
            );
            (Vr::Un, String::new(), c)
        }
    }
}

/// Every entry, group by group in the dictionary's order.
pub fn entries() -> impl Iterator<Item = &'static Entry> {
    GROUPS.iter().flat_map(|(_, e)| e.iter())
}
