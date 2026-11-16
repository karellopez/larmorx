// SPDX-License-Identifier: Apache-2.0 AND LicenseRef-CTN
//! An object's elements as CTN 3.0.6's object library gives them (`dcm.c`): `DCM_GetElement`
//! (an element's tag, representation, description, multiplicity and length, by its tag) and
//! `DCM_GetElementValue` (its value, whole or in pieces through a context, from memory or, for
//! pixel data left in the file, read through the object's descriptor), with CTN's conditions and
//! messages on every failure. FreeSurfer's DICOM reader asks for each element it uses this way.
//!
//! The value is given as the bytes CTN writes at the start of the caller's buffer ([`ValueRead`]):
//! a caller that gives CTN a buffer smaller than the request (a C overflow) sees which bytes it
//! would have received. D13's corrections: FS-951 to FS-953 (`larmorx_core::defects`); FS-894's
//! messages are kept. The behaviour is proved by the differential probe
//! `validation/probes/fs-dicom` (its element script) against CTN 3.0.6 and FreeSurfer's copy.

use std::io::{Read, Seek, SeekFrom};

use larmorx_core::cfmt::Streams;
use larmorx_core::defects::{self, Notices};

use super::cond::*;
use super::read::{swap_at_group_element, swap_in_place};
use super::{
    BYTEORDER_REVERSE, Data, DcmObject, Dialect, ElementItem, KEY_DCM_OBJECT, TAG_PIXEL_DATA, Tag,
    UNSPECIFIED_LENGTH, Vr, tag_element, tag_group,
};

/// An element as `DCM_GetElement` gives it (`DCM_ELEMENT`, its value pointer `NULL`), and the
/// caller's element of `DCM_GetElementValue`: its tag and representation say what is asked for,
/// its length how many bytes (the room of the caller's buffer).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Element {
    pub tag: Tag,
    /// CTN's number of the representation (`DCM_VALUEREPRESENTATION`; see [`Vr`]): a caller may
    /// give any number.
    pub representation: u32,
    pub description: String,
    pub multiplicity: u64,
    pub length: u32,
}

impl Element {
    /// The representation, when its number is one of CTN's.
    pub fn vr(&self) -> Option<Vr> {
        Vr::from_number(self.representation)
    }

    /// The element of `item` as CTN keeps it.
    pub fn of(item: &ElementItem) -> Element {
        Element {
            tag: item.tag,
            representation: item.representation as u32,
            description: item.description.clone(),
            multiplicity: item.multiplicity,
            length: item.length,
        }
    }
}

/// What `DCM_GetElementValue` leaves: its condition (pushed on the stack with its message when it
/// is an error; `DCM_GETINCOMPLETE` is returned without one), `*rtnLength` and what it writes into
/// the caller's buffer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValueRead {
    pub cond: Condition,
    /// `*rtnLength`: the bytes the call copies, set before the copy (also when the read from the
    /// file then fails); `None` when the call returns before it (left as the caller had it).
    pub rtn_length: Option<u32>,
    /// The bytes written at the start of the caller's buffer, in order (fewer than `rtn_length`
    /// when a read from the file stops short; none for FS-952 in `reproduce`).
    pub data: Vec<u8>,
    /// The bytes after `data` that CTN copies from memory it never wrote (`reproduce` only: FS-91's
    /// value never read, FS-951's sequence read as a value): the caller's buffer receives unknown
    /// bytes there, or the program crashes; the port writes nothing for them.
    pub unknown: u32,
}

impl ValueRead {
    fn fail(cond: Condition) -> ValueRead {
        ValueRead {
            cond,
            rtn_length: None,
            data: Vec::new(),
            unknown: 0,
        }
    }
}

/// `checkObject(object, caller)`: `None` when the object is legal, else the condition pushed.
fn check_object(
    object: Option<&DcmObject>,
    caller: &[u8],
    dialect: &dyn Dialect,
    stack: &mut CondStack,
    out: &mut dyn Streams,
    notices: &mut Notices,
) -> Option<Condition> {
    let Some(o) = object else {
        let m = dialect.message(DCM_NULLOBJECT);
        return Some(stack.push(DCM_NULLOBJECT, m, &[Arg::Str(caller)], out, notices));
    };
    if o.key_type != KEY_DCM_OBJECT {
        let m = dialect.message(DCM_ILLEGALOBJECT);
        return Some(stack.push(DCM_ILLEGALOBJECT, m, &[Arg::Str(caller)], out, notices));
    }
    None
}

/// `locateElement`: the first group of the tag's number, then its first element of the tag.
fn locate(o: &DcmObject, tag: Tag) -> Option<&ElementItem> {
    o.groups
        .iter()
        .find(|g| g.group == tag_group(tag))
        .and_then(|g| g.elements.iter().find(|e| e.tag == tag))
}

/// `DCM_GetElement(&object, tag, &element)`: the element of `tag` (its value not included), or
/// the condition pushed: `DCM_NULLOBJECT` and `DCM_ILLEGALOBJECT` for no or an illegal object,
/// `DCM_ELEMENTNOTFOUND` ("Requested element (g e) not found in DCM_GetElementVM", CTN's name of
/// the routine: FS-894) when the object has no such element. On a failure the caller's element
/// stays as it was.
pub fn get_element(
    object: Option<&DcmObject>,
    tag: Tag,
    dialect: &dyn Dialect,
    stack: &mut CondStack,
    out: &mut dyn Streams,
    notices: &mut Notices,
) -> Result<Element, Condition> {
    const CALLER: &[u8] = b"DCM_GetElementVM";
    if let Some(c) = check_object(object, CALLER, dialect, stack, out, notices) {
        return Err(c);
    }
    let o = object.expect("checked");
    match locate(o, tag) {
        Some(item) => Ok(Element::of(item)),
        None => {
            let m = dialect.message(DCM_ELEMENTNOTFOUND);
            Err(stack.push(
                DCM_ELEMENTNOTFOUND,
                m,
                &[
                    Arg::Int(tag_group(tag) as i32),
                    Arg::Int(tag_element(tag) as i32),
                    Arg::Str(CALLER),
                ],
                out,
                notices,
            ))
        }
    }
}

/// `read(fd, buffer, (int) length)` after `lseek(fd, offset, SEEK_SET)` on the object's
/// descriptor: the bytes read (the call's count is their number), or `Err(())` for -1 with `errno`
/// set: a descriptor that is not open (EBADF, after a failed open closed it) or a count of 2 GiB
/// or more, which `(int)` makes negative and the kernel refuses (EFAULT).
fn read_through(stack: &mut CondStack, fd: i32, offset: i64, length: u32) -> Result<Vec<u8>, ()> {
    let Some(file) = stack.file_of(fd).cloned() else {
        stack.errno = EBADF;
        return Err(());
    };
    let mut f: &std::fs::File = &file;
    if offset >= 0 {
        let _ = f.seek(SeekFrom::Start(offset as u64));
    } else {
        stack.errno = EINVAL;
    }
    if (length as i32) < 0 {
        stack.errno = EFAULT;
        return Err(());
    }
    let mut data = Vec::new();
    loop {
        let want = length as usize - data.len();
        if want == 0 {
            break;
        }
        let mut chunk = vec![0u8; want.min(1 << 20)];
        match f.read(&mut chunk) {
            Ok(0) => break,
            Ok(k) => data.extend_from_slice(&chunk[..k]),
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => {
                stack.errno = errno_of(&e);
                if data.is_empty() {
                    return Err(());
                }
                break;
            }
        }
    }
    Ok(data)
}

/// `DCM_GetElementValue(&object, &element, &rtnLength, &ctx)`: up to `element.length` bytes of the
/// value of the element `element.tag`, from the offset `ctx` (0 before the first call), which the
/// call moves past the bytes it copies. Its conditions, in CTN's order:
/// `DCM_NULLOBJECT`/`DCM_ILLEGALOBJECT`; `DCM_ELEMENTNOTFOUND` (no such group, or element);
/// `DCM_CANNOTGETSEQUENCEVALUE` when the caller's element says SQ (and, corrected, when the
/// element is a sequence: FS-951); `DCM_ILLEGALCONTEXT` when `ctx` is past the value; then the
/// copy: `DCM_FILEACCESSERROR` when a value left in the file does not read whole (FS-952), else
/// `DCM_NORMAL` at the value's end or `DCM_GETINCOMPLETE` before it. Pixel data in the reverse
/// byte order are swapped as they are read (FS-953 for an odd count).
///
/// `ctx` is CTN's context pointer as the offset it holds (CTN 3.0.6 compares it as a 32-bit
/// number, FreeSurfer's copy as a `long`: the same for every offset a value can have).
pub fn get_element_value(
    object: Option<&DcmObject>,
    element: &Element,
    ctx: &mut u32,
    dialect: &dyn Dialect,
    stack: &mut CondStack,
    out: &mut dyn Streams,
    notices: &mut Notices,
) -> ValueRead {
    const CALLER: &[u8] = b"DCM_GetElementValue";
    if let Some(c) = check_object(object, CALLER, dialect, stack, out, notices) {
        return ValueRead::fail(c);
    }
    let o = object.expect("checked");
    let (g, e) = (
        tag_group(element.tag) as i32,
        tag_element(element.tag) as i32,
    );
    let not_found = |second: i32, stack: &mut CondStack, out: &mut dyn Streams, n: &mut Notices| {
        let m = dialect.message(DCM_ELEMENTNOTFOUND);
        ValueRead::fail(stack.push(
            DCM_ELEMENTNOTFOUND,
            m,
            &[Arg::Int(g), Arg::Int(second), Arg::Str(CALLER)],
            out,
            n,
        ))
    };
    let Some(group) = o.groups.iter().find(|gr| gr.group as i32 == g) else {
        return not_found(e, stack, out, notices);
    };
    if group.elements.is_empty() {
        // FS-894 (kept): CTN's message shows the group twice
        return not_found(g, stack, out, notices);
    }
    let Some(item) = group.elements.iter().find(|it| it.tag == element.tag) else {
        return not_found(e, stack, out, notices);
    };
    let sequence = matches!(item.data, Data::Sequence(_));
    if element.representation == Vr::Sq as u32 || (sequence && notices.fix()) {
        if element.representation != Vr::Sq as u32 {
            notices.add(
                &defects::FS_951,
                format!(
                    "the sequence ({:04x},{:04x}) asked for as representation {}: refused",
                    g, e, element.representation
                ),
            );
        }
        let m = dialect.message(DCM_CANNOTGETSEQUENCEVALUE);
        return ValueRead::fail(stack.push(
            DCM_CANNOTGETSEQUENCEVALUE,
            m,
            &[Arg::U32(element.tag), Arg::Str(CALLER)],
            out,
            notices,
        ));
    }
    let p = *ctx;
    if p > item.length {
        let m = dialect.message(DCM_ILLEGALCONTEXT);
        return ValueRead::fail(stack.push(
            DCM_ILLEGALCONTEXT,
            m,
            &[Arg::Str(CALLER)],
            out,
            notices,
        ));
    }
    let l = element.length.min(item.length - p);
    let mut res = ValueRead {
        cond: DCM_NORMAL,
        rtn_length: Some(l),
        data: Vec::new(),
        unknown: 0,
    };
    match &item.data {
        Data::OnDisk => {
            let read = match o.fd {
                Some(fd) => read_through(stack, fd, item.data_offset + p as i64, l),
                // an object with no descriptor (made by hand, never by a file's reading): CTN calls
                // the stream functions of an object read from a stream
                None => Err(()),
            };
            let ok = match read {
                Ok(d) => {
                    let whole = d.len() as u64 == l as u64;
                    res.data = d;
                    whole
                }
                Err(()) if l == UNSPECIFIED_LENGTH => {
                    // FS-952: the -1 of the failed read, as unsigned, equals the request
                    if notices.fix() {
                        notices.add(
                            &defects::FS_952,
                            "a read of 4,294,967,295 bytes from the file failed: reported",
                        );
                        false
                    } else {
                        notices.add(
                            &defects::FS_952,
                            "a read of 4,294,967,295 bytes from the file failed: taken for complete, nothing written",
                        );
                        true
                    }
                }
                Err(()) => false,
            };
            if !ok {
                let m = dialect.message(DCM_FILEACCESSERROR);
                res.cond = stack.push(
                    DCM_FILEACCESSERROR,
                    m,
                    &[Arg::Str(&o.file_name), Arg::Str(CALLER)],
                    out,
                    notices,
                );
                return res;
            }
            if item.representation == Vr::At {
                swap_at_group_element(&mut res.data);
            }
            if item.byte_order == BYTEORDER_REVERSE {
                let pairs = matches!(item.representation, Vr::Us | Vr::Ss | Vr::Ow | Vr::At)
                    && !(item.tag == TAG_PIXEL_DATA && o.pixel_bits_allocated != 16);
                if pairs && res.data.len() < l as usize {
                    // FS-952 in reproduce: CTN swaps the caller's own bytes over the whole odd
                    // count (FS-953's crash)
                    notices.add(
                        &defects::FS_953,
                        format!("{l} bytes never read, swapped by CTN in the caller's buffer"),
                    );
                }
                swap_in_place(
                    dialect,
                    notices,
                    item,
                    o.pixel_bits_allocated,
                    &mut res.data,
                );
            }
        }
        Data::Bytes(v) => {
            let start = (p as usize).min(v.len());
            let end = (p as usize + l as usize).min(v.len());
            res.data = v[start..end].to_vec();
            if end - start < l as usize {
                // past the value's bytes: the item's room, or memory after it
                res.unknown = l - (end - start) as u32;
            }
            if item.byte_order == BYTEORDER_REVERSE {
                swap_in_place(
                    dialect,
                    notices,
                    item,
                    o.pixel_bits_allocated,
                    &mut res.data,
                );
            }
        }
        Data::Unread => {
            notices.add(
                &defects::FS_91,
                format!(
                    "the value of ({:04x},{:04x}), never read, asked for: {l} bytes of memory never written",
                    g, e
                ),
            );
            res.unknown = l;
        }
        Data::Sequence(_) => {
            // reproduce only (corrected above): the bytes of the sequence's list in memory
            notices.add(
                &defects::FS_951,
                format!(
                    "the sequence ({:04x},{:04x}) asked for as representation {}: {l} bytes of memory contents",
                    g, e, element.representation
                ),
            );
            res.unknown = l;
        }
    }
    let p = p + l;
    *ctx = p;
    if p != item.length {
        res.cond = DCM_GETINCOMPLETE;
    }
    res
}
