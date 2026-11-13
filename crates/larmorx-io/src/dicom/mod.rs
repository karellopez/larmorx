// SPDX-License-Identifier: Apache-2.0 AND LicenseRef-CTN
//! DICOM files opened as CTN's object library opens them (`DCM_OpenFile`, the DICOM Central Test
//! Node's version 3.0.6): a pure-Rust reader with the same answer, the same object (every
//! element, its value representation, length and bytes, sequences and their items, the object's
//! sizes and transfer syntax) and the same conditions and messages on every path the options
//! below reach. The format probe of a volume reader asks it whether a file is DICOM: part 10
//! (a 128-byte preamble, `DICM`, a file meta group in explicit little endian, then the data set in
//! the transfer syntax it names), or a bare data set in implicit little or big endian.
//!
//! - [`cond`]: CTN's conditions (the codes and messages of `dcmcond.c`) and its condition stack
//!   (`condition.c`: 100 entries, texts kept to 256 bytes, the overflow rule);
//! - [`dict`]: CTN's data dictionary (1,478 entries in 36 groups) and `DCM_LookupElement`;
//! - [`read`]: [`read::open_file`] and [`read::close_object`].
//!
//! [`Dialect`]: the points where a program built on CTN may change it (a message's text, the
//! byte swap of 4-byte integers); [`Ctn`] is CTN 3.0.6's own behaviour. D13's corrections of
//! CTN's defects are FS-90 to FS-95 (`larmorx_core::defects`). The source is CTN 3.0.6
//! (`PROVENANCE.md`); the behaviour is proved by the differential probe
//! `validation/probes/fs-dicom` against CTN 3.0.6 built from its source.
//!
//! CTN's notice (its three paragraphs, which every distribution of derivative works includes):
//!
//! Copyright (C) 1993, 1994, RSNA and Washington University
//!
//! The software and supporting documentation for the Radiological Society of North America
//! (RSNA) 1993, 1994 Digital Imaging and Communications in Medicine (DICOM) Demonstration were
//! developed at the Electronic Radiology Laboratory, Mallinckrodt Institute of Radiology,
//! Washington University School of Medicine, 510 S. Kingshighway Blvd., St. Louis, MO 63110 as
//! part of the 1993, 1994 DICOM Central Test Node project for, and under contract with, the
//! Radiological Society of North America.
//!
//! THIS SOFTWARE IS MADE AVAILABLE, AS IS, AND NEITHER RSNA NOR WASHINGTON UNIVERSITY MAKE ANY
//! WARRANTY ABOUT THE SOFTWARE, ITS PERFORMANCE, ITS MERCHANTABILITY OR FITNESS FOR ANY
//! PARTICULAR USE, FREEDOM FROM ANY COMPUTER DISEASES OR ITS CONFORMITY TO ANY SPECIFICATION.
//! THE ENTIRE RISK AS TO QUALITY AND PERFORMANCE OF THE SOFTWARE IS WITH THE USER.
//!
//! Copyright of the software and supporting documentation is jointly owned by RSNA and Washington
//! University, and free access is hereby granted as a license to use this software, copy this
//! software and prepare derivative works based upon this software. However, any distribution of
//! this software source code or supporting documentation or derivative works (source code and
//! supporting documentation) must include the three paragraphs of the copyright notice.

pub mod cond;
pub mod dict;
mod dict_table;
pub mod read;
#[cfg(test)]
mod tests;

pub use cond::{CondStack, Condition};
pub use read::{close_object, open_file};

/// CTN's value representations (`DCM_VALUEREPRESENTATION`), with its numbers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum Vr {
    Ae = 0,
    As,
    At,
    Cs,
    Da,
    Dd,
    Ds,
    Fd,
    Fl,
    Is,
    Lo,
    Lt,
    Ot,
    Sh,
    Sl,
    Sq,
    Ss,
    St,
    Tm,
    Ui,
    Ul,
    Un,
    Us,
    Ret,
    Ctx,
    Pn,
    Ob,
    Ow,
    Dt,
    Dlm,
    Ut,
}

impl Vr {
    const ALL: [Vr; 31] = [
        Vr::Ae,
        Vr::As,
        Vr::At,
        Vr::Cs,
        Vr::Da,
        Vr::Dd,
        Vr::Ds,
        Vr::Fd,
        Vr::Fl,
        Vr::Is,
        Vr::Lo,
        Vr::Lt,
        Vr::Ot,
        Vr::Sh,
        Vr::Sl,
        Vr::Sq,
        Vr::Ss,
        Vr::St,
        Vr::Tm,
        Vr::Ui,
        Vr::Ul,
        Vr::Un,
        Vr::Us,
        Vr::Ret,
        Vr::Ctx,
        Vr::Pn,
        Vr::Ob,
        Vr::Ow,
        Vr::Dt,
        Vr::Dlm,
        Vr::Ut,
    ];

    /// The representation with CTN's number `n`.
    pub fn from_number(n: u32) -> Option<Vr> {
        Vr::ALL.get(n as usize).copied()
    }

    /// CTN's two-letter code of an explicit VR (its `vrMap`, in its search order); `None` for a
    /// representation an explicit VR never names.
    fn code_table() -> &'static [(Vr, &'static [u8])] {
        &[
            (Vr::Ae, b"AE"),
            (Vr::As, b"AS"),
            (Vr::At, b"AT"),
            (Vr::Cs, b"CS"),
            (Vr::Da, b"DA"),
            (Vr::Dd, b"DD"),
            (Vr::Ds, b"DS"),
            (Vr::Fd, b"FD"),
            (Vr::Fl, b"FL"),
            (Vr::Is, b"IS"),
            (Vr::Lo, b"LO"),
            (Vr::Lt, b"LT"),
            (Vr::Ot, b"OT"),
            (Vr::Sh, b"SH"),
            (Vr::Sl, b"SL"),
            (Vr::Sq, b"SQ"),
            (Vr::Ss, b"SS"),
            (Vr::St, b"ST"),
            (Vr::Tm, b"TM"),
            (Vr::Ui, b"UI"),
            (Vr::Ul, b"UL"),
            (Vr::Un, b"UN"),
            (Vr::Us, b"US"),
            (Vr::Ut, b"UT"),
            (Vr::Ret, b"RT"),
            (Vr::Ctx, b"  "),
            (Vr::Pn, b"PN"),
            (Vr::Ob, b"OB"),
            (Vr::Ow, b"OW"),
            (Vr::Dt, b"DT"),
            (Vr::Dlm, b""),
        ]
    }

    /// The representation an explicit VR's two bytes name, compared as C strings (the code ends
    /// at a zero byte, so a first byte 0 names the delimiter's empty code).
    pub fn from_code(code: [u8; 2]) -> Option<Vr> {
        let n = code.iter().position(|&c| c == 0).unwrap_or(2);
        let s = &code[..n];
        Vr::code_table().iter().find(|(_, c)| *c == s).map(|(v, _)| *v)
    }
}

/// The options of [`read::open_file`] (`DCM_OpenFile`'s bit mask).
pub mod opt {
    pub const ORDERMASK: u64 = 0x7f;
    pub const ORDERNATIVE: u64 = 0x01;
    pub const ORDERLITTLEENDIAN: u64 = 0x02;
    pub const ORDERBIGENDIAN: u64 = 0x03;
    pub const EXPLICITLITTLEENDIAN: u64 = 0x04;
    pub const EXPLICITBIGENDIAN: u64 = 0x05;
    pub const FILEFORMATMASK: u64 = 0x80;
    pub const PART10FILE: u64 = 0x80;
    pub const CONVERTMASK: u64 = 0x100;
    pub const FORMATCONVERSION: u64 = 0x100;
    pub const DELETEMASK: u64 = 0x200;
    pub const DELETEONCLOSE: u64 = 0x200;
    pub const GROUPLENGTHMASK: u64 = 0x400;
    pub const NOGROUPLENGTH: u64 = 0x400;
    pub const LENGTHTOENDMASK: u64 = 0x1000;
    pub const USELENGTHTOEND: u64 = 0x1000;
    /// Also `DCM_TRYFILENAMECHANGE` (CTN gives the two options one bit).
    pub const ALLOWREPEATELEMENTS: u64 = 0x2000;
    pub const VRMASK: u64 = 0x4000;
    pub const ACCEPTVRMISMATCH: u64 = 0x4000;
    pub const SPECIALFORMATMASK: u64 = 0x8000;
    pub const EFILM: u64 = 0x8000;
}

/// A tag: the group in the high 16 bits, the element in the low 16.
pub type Tag = u32;

pub const fn make_tag(group: u16, element: u16) -> Tag {
    ((group as u32) << 16) | element as u32
}
pub const fn tag_group(t: Tag) -> u16 {
    (t >> 16) as u16
}
pub const fn tag_element(t: Tag) -> u16 {
    t as u16
}

pub const TAG_ITEM: Tag = make_tag(0xfffe, 0xe000);
pub const TAG_ITEM_DELIMITATION: Tag = make_tag(0xfffe, 0xe00d);
pub const TAG_SEQUENCE_DELIMITATION: Tag = make_tag(0xfffe, 0xe0dd);
pub const TAG_PIXEL_DATA: Tag = make_tag(0x7fe0, 0x0010);
pub const TAG_META_GROUP_LENGTH: Tag = make_tag(0x0002, 0x0000);
pub const TAG_TRANSFER_SYNTAX: Tag = make_tag(0x0002, 0x0010);
pub const TAG_BITS_ALLOCATED: Tag = make_tag(0x0028, 0x0100);
pub const TAG_PIXEL_REPRESENTATION: Tag = make_tag(0x0028, 0x0103);
pub const TAG_WAVEFORM_DATA_VR: Tag = make_tag(0x003a, 0x0103);
pub const TAG_WAVEFORM_DATA: Tag = make_tag(0x003a, 0x1000);
/// An element length meaning "undefined" (`DCM_UNSPECIFIEDLENGTH`).
pub const UNSPECIFIED_LENGTH: u32 = 0xffff_ffff;
/// The part-10 preamble's length.
pub const PREAMBLE_LENGTH: usize = 128;
/// The object's key (`KEY_DCM_OBJECT`).
pub const KEY_DCM_OBJECT: &str = "KEY ACR NEMA V3 OBJECT";

/// Byte orders as CTN numbers them on a little-endian machine: the data's order is the machine's
/// (`BYTEORDER_SAME`, little endian) or the reverse (big endian).
pub const BYTEORDER_SAME: i32 = 1;
pub const BYTEORDER_REVERSE: i32 = 2;

/// The points where a program built on CTN may change it. [`Ctn`] is CTN 3.0.6's behaviour.
pub trait Dialect {
    /// The message (a C format) of condition `c`, `DCM_Message`'s table.
    fn message(&self, c: Condition) -> Option<&'static [u8]> {
        cond::ctn_message(c)
    }
    /// Whether the 4-byte integers (UL, SL) of data in the reverse byte order are swapped in
    /// 2-byte pairs instead of reversed (FS-94: a program's change, never CTN's).
    fn swap_4byte_in_pairs(&self) -> bool {
        false
    }
}

/// CTN 3.0.6's own behaviour.
#[derive(Clone, Copy, Debug, Default)]
pub struct Ctn;

impl Dialect for Ctn {}

/// A DICOM object as CTN keeps it (`PRIVATE_OBJECT`): the fields its reader sets, in its units.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DcmObject {
    pub key_type: String,
    /// `DCM_OBJECTTYPE`: 0 unknown, 1 command, 2 image, 3 element list.
    pub object_type: u32,
    pub access_method: u32,
    pub delete_flag: bool,
    pub group_length_flag: bool,
    /// The bytes read for the object (its preamble, tags, lengths and padded values).
    pub object_size: u64,
    /// A sequence item's offset in the file (its item tag's).
    pub offset: u64,
    pub pixel_size: u64,
    pub pixel_offset: u64,
    pub pixel_bits_allocated: u16,
    pub pixel_representation: u16,
    /// Whether the object keeps the file open (its pixel data stay on disk).
    pub fd_open: bool,
    pub file_name: Vec<u8>,
    pub preamble_flag: bool,
    pub preamble: [u8; PREAMBLE_LENGTH],
    /// The data set's options, from the transfer syntax (part 10).
    pub data_options: u64,
    pub meta_header_length: u64,
    pub long_vr_attributes: i32,
    pub waveform_data_vr: Vec<u8>,
    pub groups: Vec<Group>,
}

/// A group of elements (`PRV_GROUP_ITEM`), in the order read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Group {
    pub group: u16,
    /// The group's length from its elements (8 bytes of tag and length each, plus its padded
    /// value); written into the group's length element at the end.
    pub base_length: u64,
    pub long_vr_attributes: i32,
    pub elements: Vec<ElementItem>,
}

/// An element as CTN keeps it (`PRV_ELEMENT_ITEM` and its `DCM_ELEMENT`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ElementItem {
    pub tag: Tag,
    pub representation: Vr,
    pub description: String,
    pub multiplicity: u64,
    pub length: u32,
    pub byte_order: i32,
    pub data_offset: i64,
    pub current_offset: i64,
    pub allocated_data_length: u64,
    pub original_data_length: u64,
    pub padded_data_length: u64,
    pub fragment_flag: i32,
    pub data: Data,
}

/// An element's value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Data {
    /// The value's bytes, in the machine's order (little endian) where CTN swaps them.
    Bytes(Vec<u8>),
    /// Pixel data left in the file (`dataOffset`).
    OnDisk,
    /// A sequence's items.
    Sequence(Vec<DcmObject>),
}

impl DcmObject {
    /// `DCM_CreateObject`'s new object.
    pub fn new(options: u64) -> DcmObject {
        DcmObject {
            key_type: KEY_DCM_OBJECT.to_owned(),
            object_type: 0,
            access_method: 2,
            delete_flag: false,
            group_length_flag: (options & opt::GROUPLENGTHMASK) != opt::NOGROUPLENGTH,
            object_size: 0,
            offset: 0,
            pixel_size: 0,
            pixel_offset: 0,
            pixel_bits_allocated: 0,
            pixel_representation: 0xffff,
            fd_open: false,
            file_name: Vec::new(),
            preamble_flag: false,
            preamble: [0; PREAMBLE_LENGTH],
            data_options: 0,
            meta_header_length: 0xffff_ffff,
            long_vr_attributes: 0,
            waveform_data_vr: Vec::new(),
            groups: Vec::new(),
        }
    }
}
