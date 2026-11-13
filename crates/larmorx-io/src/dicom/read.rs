// SPDX-License-Identifier: Apache-2.0 AND LicenseRef-CTN
//! `DCM_OpenFile` for files, as CTN 3.0.6 reads them: the part-10 preamble, the elements one by
//! one (tag, representation, length, value), sequences and their items read as nested objects,
//! encapsulated pixel data scanned and left in the file, the transfer syntax applied after the
//! file meta group, the group lengths recomputed; and `DCM_CloseObject`. The C arithmetic on
//! 32-bit lengths is kept (it wraps), as are the conditions each failure pushes and the object
//! each failure leaves (closed by some failures, kept by others).

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use larmorx_core::cfmt::Streams;
use larmorx_core::defects::{self, Notices};

use super::cond::*;
use super::dict::lookup_element;
use super::{
    BYTEORDER_REVERSE, BYTEORDER_SAME, Data, DcmObject, Dialect, ElementItem, Group,
    PREAMBLE_LENGTH, TAG_BITS_ALLOCATED, TAG_ITEM, TAG_ITEM_DELIMITATION, TAG_META_GROUP_LENGTH,
    TAG_PIXEL_DATA, TAG_PIXEL_REPRESENTATION, TAG_SEQUENCE_DELIMITATION, TAG_TRANSFER_SYNTAX,
    TAG_WAVEFORM_DATA, TAG_WAVEFORM_DATA_VR, Tag, UNSPECIFIED_LENGTH, Vr, make_tag, opt,
    tag_element, tag_group,
};

/// The deepest nesting of sequences read (FS-97): CTN recurses once per level and exhausts the
/// thread's stack somewhere past this, depending on the stack's size.
pub const MAX_NESTING: i32 = 128;

/// The bytes of a file name CTN's object keeps (`fileName[1024]`, FS-90).
const FILE_NAME_KEPT: usize = 1023;

/// Why [`open_file`] cannot answer as CTN does: an option whose path no caller reaches and the
/// reader does not follow (`DCM_DELETEONCLOSE`, `DCM_USELENGTHTOEND`, `DCM_ALLOWREPEATELEMENTS` /
/// `DCM_TRYFILENAMECHANGE`, `DCM_EFILM`, `DCM_FORMATCONVERSION` with a byte order).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unsupported(pub &'static str);

impl std::fmt::Display for Unsupported {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DCM_OpenFile option {} is not supported", self.0)
    }
}

impl std::error::Error for Unsupported {}

/// The reader's state for one `DCM_OpenFile`: the file, the name its messages show, the
/// condition stack, the streams, the notices.
struct Reader<'a> {
    name: &'a [u8],
    file: File,
    fd: i32,
    dialect: &'a dyn Dialect,
    stack: &'a mut CondStack,
    out: &'a mut dyn Streams,
    notices: &'a mut Notices,
}

/// An element being read (`DCM_ELEMENT` before its item exists).
#[derive(Clone, Debug)]
struct El {
    tag: Tag,
    representation: Vr,
    description: String,
    length: u32,
}

impl Reader<'_> {
    fn push(&mut self, c: Condition, args: &[Arg]) -> Condition {
        let m = self.dialect.message(c);
        self.stack.push(c, m, args, self.out, self.notices)
    }

    /// POSIX `read` on a regular file: the bytes up to `buf`'s length or the end; -1 when the
    /// first read fails.
    fn read(&mut self, buf: &mut [u8]) -> i64 {
        let mut n = 0;
        while n < buf.len() {
            match self.file.read(&mut buf[n..]) {
                Ok(0) => break,
                Ok(k) => n += k,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(e) => {
                    self.stack.errno = errno_of(&e);
                    if n == 0 {
                        return -1;
                    }
                    break;
                }
            }
        }
        n as i64
    }

    fn seek_cur(&mut self, by: i64) {
        let _ = self.file.seek(SeekFrom::Current(by));
    }
}

/// `DCM_CloseObject` on an object: its descriptor closed, then its sequences' items', in order.
fn close_tree(o: &DcmObject, stack: &mut CondStack) {
    if let Some(fd) = o.fd {
        stack.close_fd(fd);
    }
    for g in &o.groups {
        for e in &g.elements {
            if let Data::Sequence(items) = &e.data {
                for it in items {
                    close_tree(it, stack);
                }
            }
        }
    }
}

fn close(r: &mut Reader, obj: &mut Option<DcmObject>) {
    if let Some(o) = obj.take() {
        close_tree(&o, r.stack);
    }
}

fn get_u16(b: &[u8], order: i32) -> u16 {
    if order == BYTEORDER_SAME {
        u16::from_le_bytes([b[0], b[1]])
    } else {
        u16::from_be_bytes([b[0], b[1]])
    }
}

fn get_u32(b: &[u8], order: i32) -> u32 {
    if order == BYTEORDER_SAME {
        u32::from_le_bytes([b[0], b[1], b[2], b[3]])
    } else {
        u32::from_be_bytes([b[0], b[1], b[2], b[3]])
    }
}

/// The byte order and explicit-VR flag of `DCM_ORDERMASK`'s value.
fn order_of(o: u64, default_explicit_little: bool) -> (i32, bool) {
    match o {
        opt::ORDERNATIVE => (BYTEORDER_SAME, false),
        opt::ORDERLITTLEENDIAN => (BYTEORDER_SAME, false),
        opt::EXPLICITLITTLEENDIAN => (BYTEORDER_SAME, true),
        opt::ORDERBIGENDIAN => (BYTEORDER_REVERSE, false),
        opt::EXPLICITBIGENDIAN => (BYTEORDER_REVERSE, true),
        _ => (BYTEORDER_SAME, default_explicit_little),
    }
}

/// `DCM_OpenFile(name, options, &object)` for a file: the condition it returns (pushed on
/// `stack` with its message when it fails) and the object handle as it leaves it. `object` is
/// the caller's handle: replaced by the new object when the reading starts, left as it was by the
/// failures before (an illegal option, a file that cannot be opened, an empty file).
#[allow(clippy::too_many_arguments)]
pub fn open_file(
    name: &Path,
    options: u64,
    object: &mut Option<DcmObject>,
    dialect: &dyn Dialect,
    stack: &mut CondStack,
    out: &mut dyn Streams,
    notices: &mut Notices,
) -> Result<Condition, Unsupported> {
    let name_bytes = path_bytes(name);
    if (options & (opt::ORDERMASK | opt::FILEFORMATMASK)) == 0 {
        let m = dialect.message(DCM_ILLEGALOPTION);
        return Ok(stack.push(
            DCM_ILLEGALOPTION,
            m,
            &[Arg::Str(b"Byte order"), Arg::Str(b"DCM_OpenFile")],
            out,
            notices,
        ));
    }
    if (options & opt::DELETEMASK) == opt::DELETEONCLOSE {
        return Err(Unsupported("DCM_DELETEONCLOSE"));
    }
    if (options & opt::LENGTHTOENDMASK) == opt::USELENGTHTOEND {
        return Err(Unsupported("DCM_USELENGTHTOEND"));
    }
    if (options & opt::ALLOWREPEATELEMENTS) != 0 {
        return Err(Unsupported("DCM_ALLOWREPEATELEMENTS / DCM_TRYFILENAMECHANGE"));
    }
    if (options & opt::SPECIALFORMATMASK) == opt::EFILM {
        return Err(Unsupported("DCM_EFILM"));
    }
    if (options & opt::CONVERTMASK) == opt::FORMATCONVERSION {
        return Err(Unsupported("DCM_FORMATCONVERSION with a byte order"));
    }
    let file = match File::open(name) {
        Ok(f) => f,
        Err(e) => {
            stack.errno = errno_of(&e);
            let m = dialect.message(DCM_FILEOPENFAILED);
            return Ok(stack.push(
                DCM_FILEOPENFAILED,
                m,
                &[Arg::Str(&name_bytes), Arg::Str(b"DCM_OpenFile")],
                out,
                notices,
            ));
        }
    };
    // fileSize: fstat's size as an int, then CTN's U32.
    let size = match file.metadata() {
        Ok(m) => m.len() as u32,
        Err(_) => u32::MAX,
    };
    if size == 0 {
        return Ok(DCM_FILEACCESSERROR);
    }
    let fd = stack.open_fd();
    let mut r = Reader {
        name: &name_bytes,
        file,
        fd,
        dialect,
        stack,
        out,
        notices,
    };
    let mut offset = 0i64;
    let mut remain_open = false;
    let cond = read_file1(&mut r, size, &mut offset, 0, options, None, object, None, &mut remain_open);
    if cond != DCM_NORMAL || !remain_open {
        r.stack.close_fd(fd);
    }
    if cond != DCM_NORMAL {
        let n = r.name.to_vec();
        return Ok(r.push(DCM_FILEOPENFAILED, &[Arg::Str(&n), Arg::Str(b"DCM_OpenFile")]));
    }
    Ok(DCM_NORMAL)
}

/// `DCM_CloseObject(&object)`: the handle emptied; a handle already empty pushes NULLOBJECT.
pub fn close_object(
    object: &mut Option<DcmObject>,
    dialect: &dyn Dialect,
    stack: &mut CondStack,
    out: &mut dyn Streams,
    notices: &mut Notices,
) -> Condition {
    let Some(o) = object.take() else {
        let m = dialect.message(DCM_NULLOBJECT);
        return stack.push(DCM_NULLOBJECT, m, &[Arg::Str(b"DCM_CloseObject")], out, notices);
    };
    close_tree(&o, stack);
    DCM_NORMAL
}

/// A path's bytes as C receives them.
fn path_bytes(p: &Path) -> Vec<u8> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        p.as_os_str().as_bytes().to_vec()
    }
    #[cfg(not(unix))]
    {
        p.to_string_lossy().as_bytes().to_vec()
    }
}

#[allow(clippy::too_many_arguments)]
fn read_file1(
    r: &mut Reader,
    size: u32,
    file_offset: &mut i64,
    recursion: i32,
    mut options: u64,
    parent_pixel_representation: Option<u16>,
    object: &mut Option<DcmObject>,
    mut scanned: Option<&mut u32>,
    remain_open: &mut bool,
) -> Condition {
    let mut size = size;
    let mut part10 = false;
    let (mut last_group, mut last_element) = (-1i64, -1i64);
    if (options & opt::FILEFORMATMASK) == opt::PART10FILE {
        part10 = true;
        options &= !opt::ORDERMASK;
        options &= !opt::FILEFORMATMASK;
        options |= opt::EXPLICITLITTLEENDIAN;
    }
    let (mut byte_order, mut explicit) = order_of(options & opt::ORDERMASK, false);
    let accept = (options & opt::VRMASK) == opt::ACCEPTVRMISMATCH;
    if let Some(s) = scanned.as_deref_mut() {
        *s = 0;
    }
    let mut o = DcmObject::new(options);
    if r.name.len() > FILE_NAME_KEPT {
        r.notices.add(
            &defects::FS_90,
            format!("a file name of {} bytes", r.name.len()),
        );
    }
    o.file_name = r.name[..r.name.len().min(FILE_NAME_KEPT)].to_vec();
    if let Some(p) = parent_pixel_representation {
        o.pixel_representation = p;
    }
    *object = Some(o);
    let known = size != UNSPECIFIED_LENGTH;
    if recursion == 0 && part10 {
        let flag = read_preamble(r, &mut size, file_offset, known, object);
        if flag != DCM_NORMAL {
            return flag;
        }
    }
    let mut current_group: Option<usize> = None;
    let mut done = false;
    while !done {
        let mut e = El {
            tag: 0,
            representation: Vr::Un,
            description: String::new(),
            length: 0,
        };
        let flag = read_group_element(
            r,
            &mut size,
            file_offset,
            known,
            byte_order,
            object,
            scanned.as_deref_mut(),
            &mut e,
        );
        if flag == DCM_STREAMCOMPLETE {
            break;
        } else if flag != DCM_NORMAL {
            return flag;
        }
        let flag = read_vr_length(
            r,
            &mut size,
            file_offset,
            known,
            byte_order,
            explicit,
            accept,
            object,
            scanned.as_deref_mut(),
            &mut e,
        );
        if flag != DCM_NORMAL {
            return flag;
        }
        if e.representation == Vr::Un && e.length == UNSPECIFIED_LENGTH {
            e.representation = Vr::Sq;
        }
        if e.length == UNSPECIFIED_LENGTH
            && e.representation != Vr::Sq
            && e.tag != TAG_PIXEL_DATA
            && tag_group(e.tag) != 0xfffe
            && r.notices.fix()
        {
            // FS-91 corrected: only a sequence has an undefined length here
            r.notices.add(
                &defects::FS_91,
                format!(
                    "({:04x},{:04x}) of undefined length read as a sequence",
                    tag_group(e.tag),
                    tag_element(e.tag)
                ),
            );
            e.representation = Vr::Sq;
        }
        if e.tag == TAG_ITEM_DELIMITATION || e.tag == TAG_SEQUENCE_DELIMITATION {
            return DCM_NORMAL;
        }
        let mut item;
        if e.representation == Vr::Sq {
            let mut sequence_length = e.length;
            let mut scanned_sequence = 0u32;
            let res = read_sequence(
                r,
                &mut sequence_length,
                file_offset,
                recursion,
                options,
                byte_order,
                explicit,
                accept,
                remain_open,
                object,
                &mut scanned_sequence,
                &e,
            );
            match res {
                Ok(it) => item = it,
                Err(flag) => return flag,
            }
            if size != UNSPECIFIED_LENGTH {
                size = size.wrapping_sub(scanned_sequence);
            }
            if let Some(s) = scanned.as_deref_mut() {
                *s = s.wrapping_add(scanned_sequence);
            }
        } else {
            match read_data(
                r,
                &mut size,
                file_offset,
                byte_order,
                explicit,
                accept,
                remain_open,
                object,
                scanned.as_deref_mut(),
                &e,
            ) {
                Ok(it) => item = it,
                Err(flag) => return flag,
            }
        }
        compute_vm(&mut item);
        let cond = check_attribute_order(r, &e, &mut last_group, &mut last_element);
        if cond != DCM_NORMAL {
            return cond;
        }
        let Some(o) = object.as_mut() else {
            return DCM_NORMAL;
        };
        let group = tag_group(e.tag);
        let gi = match current_group {
            Some(i) if o.groups[i].group == group => i,
            _ => {
                o.groups.push(Group {
                    group,
                    base_length: 0,
                    long_vr_attributes: 0,
                    elements: Vec::new(),
                });
                o.groups.len() - 1
            }
        };
        current_group = Some(gi);
        if tag_element(e.tag) != 0 {
            let g = &mut o.groups[gi];
            g.base_length = g
                .base_length
                .wrapping_add(8)
                .wrapping_add(item.padded_data_length);
            if matches!(item.representation, Vr::Ob | Vr::Ow | Vr::Sq) {
                g.long_vr_attributes += 1;
                o.long_vr_attributes += 1;
            }
        }
        if tag_element(e.tag) == 0 && !o.group_length_flag {
            // freed, never enqueued
        } else {
            update_object_type(o, item.tag);
            update_special_elements(o, &item, r.notices);
            o.groups[gi].elements.push(item);
        }
        if size == 0 {
            done = true;
        }
        if part10
            && o.object_size
                == (PREAMBLE_LENGTH as u64 + 4 + 12).wrapping_add(o.meta_header_length)
        {
            options &= !opt::ORDERMASK;
            options |= o.data_options & opt::ORDERMASK;
            (byte_order, explicit) = order_of(options & opt::ORDERMASK, true);
        }
    }
    if let Some(o) = object.as_mut() {
        for g in &mut o.groups {
            let base = g.base_length as u32;
            if let Some(first) = g.elements.first_mut()
                && tag_element(first.tag) == 0
            {
                match &mut first.data {
                    Data::Bytes(b) if b.len() >= 4 => {
                        b[..4].copy_from_slice(&base.to_le_bytes());
                    }
                    Data::Bytes(b) => {
                        r.notices.add(
                            &defects::FS_93,
                            format!(
                                "the group length ({:04x},0000) holds {} bytes, not 4",
                                g.group,
                                b.len()
                            ),
                        );
                        let n = b.len();
                        b.copy_from_slice(&base.to_le_bytes()[..n]);
                    }
                    _ => r.notices.add(
                        &defects::FS_93,
                        format!("the group length ({:04x},0000) holds no value", g.group),
                    ),
                }
            }
        }
    }
    DCM_NORMAL
}

fn read_preamble(
    r: &mut Reader,
    size: &mut u32,
    file_offset: &mut i64,
    known: bool,
    object: &mut Option<DcmObject>,
) -> Condition {
    if *size == 0 {
        return DCM_STREAMCOMPLETE;
    }
    if *size < PREAMBLE_LENGTH as u32 + 4 && known {
        close(r, object);
        let s = *size;
        return r.push(DCM_ILLEGALSTREAMLENGTH, &[Arg::U32(s), Arg::Str(b"readPreamble")]);
    }
    let mut preamble = [0u8; PREAMBLE_LENGTH];
    let mut label = [0u8; 4];
    let n = r.read(&mut preamble) + r.read(&mut label);
    if let Some(o) = object.as_mut() {
        // the bytes read land in the object's preamble whatever the count
        let k = n.clamp(0, PREAMBLE_LENGTH as i64) as usize;
        o.preamble[..k].copy_from_slice(&preamble[..k]);
    }
    if n != PREAMBLE_LENGTH as i64 + 4 {
        let name = r.name.to_vec();
        return r.push(DCM_FILEACCESSERROR, &[Arg::Str(&name), Arg::Str(b"readPreamble")]);
    }
    if known {
        *size = size.wrapping_sub(PREAMBLE_LENGTH as u32 + 4);
    }
    *file_offset += PREAMBLE_LENGTH as i64 + 4;
    let Some(o) = object.as_mut() else {
        return DCM_NORMAL;
    };
    o.object_size = o.object_size.wrapping_add(PREAMBLE_LENGTH as u64 + 4);
    if &label != b"DICM" {
        return 0;
    }
    o.preamble_flag = true;
    DCM_NORMAL
}

#[allow(clippy::too_many_arguments)]
fn read_group_element(
    r: &mut Reader,
    size: &mut u32,
    file_offset: &mut i64,
    known: bool,
    byte_order: i32,
    object: &mut Option<DcmObject>,
    scanned: Option<&mut u32>,
    e: &mut El,
) -> Condition {
    if *size == 0 {
        return DCM_STREAMCOMPLETE;
    }
    if *size < 4 && known {
        close(r, object);
        let s = *size;
        return r.push(DCM_ILLEGALSTREAMLENGTH, &[Arg::U32(s), Arg::Str(b"readFile")]);
    }
    let mut buf = [0u8; 4];
    let n = r.read(&mut buf);
    if n != 4 {
        r.stack.perror("", r.out);
        r.out.out(&format!("Bytes read: {}\n", n as i32));
        let name = r.name.to_vec();
        return r.push(
            DCM_FILEACCESSERROR,
            &[Arg::Str(&name), Arg::Str(b"readGroupElement")],
        );
    }
    if known {
        *size = size.wrapping_sub(4);
    }
    *file_offset += 4;
    if let Some(s) = scanned {
        *s = s.wrapping_add(4);
    }
    if let Some(o) = object.as_mut() {
        o.object_size = o.object_size.wrapping_add(4);
    }
    let group = get_u16(&buf[0..2], byte_order);
    let element = get_u16(&buf[2..4], byte_order);
    e.tag = make_tag(group, element);
    let (vr, description, cond) = lookup_element(e.tag, r.dialect, r.stack, r.out, r.notices);
    e.representation = vr;
    e.description = description;
    if cond != DCM_NORMAL {
        r.stack.pop(false);
    }
    if e.representation == Vr::Ctx
        && let Some(o) = object.as_ref()
    {
        ctx_sensitive_lookup(o, e);
    }
    DCM_NORMAL
}

#[allow(clippy::too_many_arguments)]
fn read_vr_length(
    r: &mut Reader,
    size: &mut u32,
    file_offset: &mut i64,
    known: bool,
    byte_order: i32,
    mut explicit: bool,
    accept: bool,
    object: &mut Option<DcmObject>,
    mut scanned: Option<&mut u32>,
    e: &mut El,
) -> Condition {
    if *size == 0 {
        return DCM_STREAMCOMPLETE;
    }
    if *size < 4 && known {
        close(r, object);
        let s = *size;
        return r.push(DCM_ILLEGALSTREAMLENGTH, &[Arg::U32(s), Arg::Str(b"readVRLength")]);
    }
    let mut buf = [0u8; 4];
    if r.read(&mut buf) != 4 {
        let name = r.name.to_vec();
        return r.push(DCM_FILEACCESSERROR, &[Arg::Str(&name), Arg::Str(b"readVRLength")]);
    }
    let count = |size: &mut u32,
                     file_offset: &mut i64,
                     scanned: &mut Option<&mut u32>,
                     object: &mut Option<DcmObject>| {
        if known {
            *size = size.wrapping_sub(4);
        }
        *file_offset += 4;
        if let Some(s) = scanned.as_deref_mut() {
            *s = s.wrapping_add(4);
        }
        if let Some(o) = object.as_mut() {
            o.object_size = o.object_size.wrapping_add(4);
        }
    };
    count(size, file_offset, &mut scanned, object);
    e.length = 0;
    if e.representation == Vr::Dlm {
        explicit = false;
    }
    let mut calculated = false;
    if explicit {
        let code = [buf[0], buf[1]];
        let Some(vr) = Vr::from_code(code) else {
            let mut c = code.to_vec();
            c.push(0);
            return r.push(DCM_UNRECOGNIZEDVRCODE, &[Arg::Str(&c), Arg::Str(b"readVRLength")]);
        };
        if vr != e.representation {
            if vr == Vr::Ob {
                e.representation = vr;
            } else if matches!(e.representation, Vr::Un | Vr::Ctx | Vr::Ret)
                || vr == Vr::Ow
                || accept
            {
                e.representation = vr;
            } else if e.tag != TAG_PIXEL_DATA {
                let mut c = code.to_vec();
                c.push(0);
                return r.push(DCM_VRMISMATCH, &[Arg::Str(&c), Arg::U32(e.tag)]);
            }
        }
        if !matches!(vr, Vr::Ow | Vr::Ob | Vr::Un | Vr::Ut | Vr::Sq) {
            e.length = get_u16(&buf[2..4], byte_order) as u32;
            calculated = true;
        } else {
            if *size < 4 && known {
                close(r, object);
                let s = *size;
                return r.push(
                    DCM_ILLEGALSTREAMLENGTH,
                    &[Arg::U32(s), Arg::Str(b"readVRLength")],
                );
            }
            if r.read(&mut buf) != 4 {
                let name = r.name.to_vec();
                return r.push(DCM_FILEACCESSERROR, &[Arg::Str(&name), Arg::Str(b"readVRLength")]);
            }
            count(size, file_offset, &mut scanned, object);
        }
    }
    if !calculated {
        e.length = get_u32(&buf, byte_order);
    }
    if (e.length & 1) != 0 && e.length != UNSPECIFIED_LENGTH {
        close(r, object);
        let (g, el, l) = (tag_group(e.tag), tag_element(e.tag), e.length);
        return r.push(
            DCM_UNEVENELEMENTLENGTH,
            &[
                Arg::Int(g as i32),
                Arg::Int(el as i32),
                Arg::U32(l),
                Arg::Str(b"readFile"),
            ],
        );
    }
    if e.length != UNSPECIFIED_LENGTH && e.length > *size {
        close(r, object);
        let (g, el, l, s) = (tag_group(e.tag), tag_element(e.tag), e.length, *size);
        return r.push(
            DCM_ELEMENTLENGTHERROR,
            &[
                Arg::Int(g as i32),
                Arg::Int(el as i32),
                Arg::U32(l),
                Arg::U32(s),
                Arg::Str(b"readFile"),
            ],
        );
    }
    DCM_NORMAL
}

/// `newElementItem(e, allocateData)`: the item, its value's room (`length`, made even, when
/// allocated; none for a sequence).
fn new_element_item(e: &El, allocate: bool) -> ElementItem {
    let l: u64 = if allocate && e.representation != Vr::Sq {
        let mut l = e.length as u64;
        if l & 1 != 0 {
            l += 1;
        }
        l
    } else {
        0
    };
    ElementItem {
        tag: e.tag,
        representation: e.representation,
        description: e.description.clone(),
        multiplicity: 0,
        length: e.length,
        byte_order: BYTEORDER_SAME,
        data_offset: 0,
        current_offset: 0,
        allocated_data_length: l,
        original_data_length: e.length as u64,
        padded_data_length: e.length as u64,
        fragment_flag: 0,
        data: if allocate {
            Data::Bytes(Vec::new())
        } else {
            Data::OnDisk
        },
    }
}

#[allow(clippy::too_many_arguments)]
fn read_sequence(
    r: &mut Reader,
    size: &mut u32,
    file_offset: &mut i64,
    recursion: i32,
    options: u64,
    byte_order: i32,
    explicit: bool,
    accept: bool,
    remain_open: &mut bool,
    object: &mut Option<DcmObject>,
    scanned: &mut u32,
    e: &El,
) -> Result<ElementItem, Condition> {
    let known = *size != UNSPECIFIED_LENGTH;
    let mut item = new_element_item(e, false);
    let mut items = Vec::new();
    let mut local_length = item.length;
    let mut done = local_length == 0;
    while !done {
        let mut sequence_length = 0u32;
        let item_tag_offset = *file_offset;
        let mut tag_e = El {
            tag: 0,
            representation: Vr::Un,
            description: String::new(),
            length: 0,
        };
        let flag = read_group_element(
            r,
            &mut local_length,
            file_offset,
            known,
            byte_order,
            object,
            Some(&mut sequence_length),
            &mut tag_e,
        );
        if flag == DCM_STREAMCOMPLETE {
            break;
        } else if flag != DCM_NORMAL {
            return Err(flag);
        }
        let flag = read_vr_length(
            r,
            &mut local_length,
            file_offset,
            known,
            byte_order,
            explicit,
            accept,
            object,
            Some(&mut sequence_length),
            &mut tag_e,
        );
        if flag != DCM_NORMAL {
            return Err(flag);
        }
        if *size != UNSPECIFIED_LENGTH {
            *size = size.wrapping_sub(sequence_length);
        }
        *scanned = scanned.wrapping_add(sequence_length);
        sequence_length = 0;
        if tag_e.tag == TAG_ITEM {
            if recursion + 1 > MAX_NESTING {
                r.notices.add(
                    &defects::FS_97,
                    format!("sequences nested deeper than {MAX_NESTING} levels"),
                );
                return Err(r.push(DCM_READSTREAMFAILED, &[Arg::Str(b"readSequence")]));
            }
            let parent_pr = object.as_ref().map(|o| o.pixel_representation);
            let mut sequence_object = None;
            let cond = read_file1(
                r,
                tag_e.length,
                file_offset,
                recursion + 1,
                options,
                parent_pr,
                &mut sequence_object,
                Some(&mut sequence_length),
                remain_open,
            );
            if cond == DCM_NORMAL {
                let mut so = sequence_object.expect("readFile1 left its object");
                so.offset = item_tag_offset as u64;
                items.push(so);
                if *size != UNSPECIFIED_LENGTH {
                    *size = size.wrapping_sub(sequence_length);
                }
                *scanned = scanned.wrapping_add(sequence_length);
                if let Some(o) = object.as_mut() {
                    o.object_size = o.object_size.wrapping_add(sequence_length as u64);
                }
                if local_length != UNSPECIFIED_LENGTH {
                    local_length = local_length.wrapping_sub(sequence_length);
                }
            } else {
                return Err(cond);
            }
        } else {
            done = true;
        }
        if local_length == 0 {
            done = true;
        }
    }
    item.data = Data::Sequence(items);
    Ok(item)
}

/// `scanCompressedPixels`: the fragments of encapsulated pixel data skipped; the bytes scanned
/// (item tags, lengths and fragments, the delimiter's 8 bytes). `Err` when a fragment's length
/// closes the object (FS-98: CTN then writes through the closed object).
#[allow(clippy::too_many_arguments)]
fn scan_compressed_pixels(
    r: &mut Reader,
    size: &mut u32,
    file_offset: &mut i64,
    byte_order: i32,
    explicit: bool,
    accept: bool,
    object: &mut Option<DcmObject>,
    scanned: &mut u32,
    length: u32,
) -> Result<(), Condition> {
    let known = *size != UNSPECIFIED_LENGTH;
    let mut scanned_bytes = 0u32;
    let mut local_length = length;
    let mut done = local_length == 0;
    let saved = std::mem::replace(&mut r.name, b"");
    let res = (|| {
        while !done {
            let mut sequence_length = 0u32;
            let mut tag_e = El {
                tag: 0,
                representation: Vr::Un,
                description: String::new(),
                length: 0,
            };
            let flag = read_group_element(
                r,
                &mut local_length,
                file_offset,
                false,
                byte_order,
                object,
                Some(&mut sequence_length),
                &mut tag_e,
            );
            if flag == DCM_STREAMCOMPLETE {
                break;
            } else if flag != DCM_NORMAL {
                return Ok(());
            }
            let flag = read_vr_length(
                r,
                &mut local_length,
                file_offset,
                known,
                byte_order,
                explicit,
                accept,
                object,
                Some(&mut sequence_length),
                &mut tag_e,
            );
            if flag != DCM_NORMAL {
                if object.is_none() {
                    return Err(flag);
                }
                return Ok(());
            }
            if *size != UNSPECIFIED_LENGTH {
                *size = size.wrapping_sub(sequence_length);
            }
            *scanned = scanned.wrapping_add(sequence_length);
            scanned_bytes = scanned_bytes.wrapping_add(sequence_length);
            if tag_e.tag == TAG_ITEM {
                if tag_e.length != 0 {
                    r.seek_cur(tag_e.length as i64);
                    *file_offset += tag_e.length as i64;
                    if *size != UNSPECIFIED_LENGTH {
                        *size = size.wrapping_sub(tag_e.length);
                    }
                    *scanned = scanned.wrapping_add(tag_e.length);
                }
            } else {
                done = true;
            }
            if local_length == 0 {
                done = true;
            }
        }
        if scanned_bytes & 1 != 0 {
            r.seek_cur(1);
            *file_offset += 1;
            if *size != UNSPECIFIED_LENGTH {
                *size = size.wrapping_sub(1);
            }
        }
        Ok(())
    })();
    r.name = saved;
    res
}

#[allow(clippy::too_many_arguments)]
fn read_data(
    r: &mut Reader,
    size: &mut u32,
    file_offset: &mut i64,
    byte_order: i32,
    explicit: bool,
    accept: bool,
    remain_open: &mut bool,
    object: &mut Option<DcmObject>,
    scanned: Option<&mut u32>,
    e: &El,
) -> Result<ElementItem, Condition> {
    let pixel = e.tag == TAG_PIXEL_DATA;
    let mut item = new_element_item(e, !pixel);
    if pixel {
        *remain_open = true;
        item.byte_order = byte_order;
        item.data_offset = *file_offset;
        item.current_offset = 0;
        item.data = Data::OnDisk;
        let bits = object.as_ref().map(|o| o.pixel_bits_allocated).unwrap_or(0);
        item.representation = if bits == 8 { Vr::Ob } else { Vr::Ow };
        if item.length != UNSPECIFIED_LENGTH {
            r.seek_cur(item.length as i64);
        } else {
            let mut l1 = 0u32;
            let mut s1 = *size;
            let mut f1 = 0i64;
            if let Err(flag) = scan_compressed_pixels(
                r,
                &mut s1,
                &mut f1,
                byte_order,
                explicit,
                accept,
                object,
                &mut l1,
                item.length,
            ) {
                r.notices.add(
                    &defects::FS_98,
                    "a fragment of encapsulated pixel data closed the object",
                );
                return Err(flag);
            }
            item.original_data_length = l1 as u64;
            item.padded_data_length = l1 as u64;
        }
        if let Some(o) = object.as_mut() {
            o.fd = Some(r.fd);
        }
    } else {
        let length = item.length;
        if (length as i32) < 0 {
            // read(fd, value, (int) length): a negative count fails (EINVAL) and reads nothing;
            // the check compares its -1 with (int) length, equal for an undefined length (FS-91)
            r.stack.errno = EINVAL;
            if length != UNSPECIFIED_LENGTH {
                close(r, object);
                let name = r.name.to_vec();
                return Err(r.push(DCM_FILEACCESSERROR, &[Arg::Str(&name), Arg::Str(b"readFile")]));
            }
            r.notices.add(
                &defects::FS_91,
                format!(
                    "({:04x},{:04x}) of undefined length kept with a value never read",
                    tag_group(e.tag),
                    tag_element(e.tag)
                ),
            );
            item.data = Data::Unread;
            return Ok(finish_data(size, file_offset, scanned, object, item));
        }
        let mut data = vec![0u8; length as usize];
        let n = r.read(&mut data);
        if n != length as i64 {
            close(r, object);
            let name = r.name.to_vec();
            return Err(r.push(DCM_FILEACCESSERROR, &[Arg::Str(&name), Arg::Str(b"readFile")]));
        }
        if item.representation == Vr::At {
            swap_at_group_element(&mut data);
        }
        if byte_order != BYTEORDER_SAME {
            let bits = object.as_ref().map(|o| o.pixel_bits_allocated).unwrap_or(0);
            swap_in_place(r, &item, bits, &mut data);
        }
        item.data = Data::Bytes(data);
    }
    Ok(finish_data(size, file_offset, scanned, object, item))
}

/// The end of `readData`: the sizes and offsets advanced by the value's length, its padded
/// length counted into the object.
fn finish_data(
    size: &mut u32,
    file_offset: &mut i64,
    scanned: Option<&mut u32>,
    object: &mut Option<DcmObject>,
    mut item: ElementItem,
) -> ElementItem {
    if *size != UNSPECIFIED_LENGTH {
        *size = size.wrapping_sub(item.original_data_length as u32);
    }
    *file_offset += item.original_data_length as i64;
    if let Some(s) = scanned {
        *s = s.wrapping_add(item.original_data_length as u32);
    }
    if item.length != UNSPECIFIED_LENGTH {
        item.padded_data_length = item.length as u64;
    }
    if item.padded_data_length != UNSPECIFIED_LENGTH as u64 && item.padded_data_length & 1 != 0 {
        item.padded_data_length += 1;
    }
    if let Some(o) = object.as_mut() {
        o.object_size = o.object_size.wrapping_add(item.padded_data_length);
    }
    item
}

/// `swapATGroupElement` (a little-endian machine): the two 16-bit halves of each 4-byte tag
/// exchanged.
fn swap_at_group_element(d: &mut [u8]) {
    let mut i = 0;
    while d.len() - i >= 4 {
        d.swap(i, i + 2);
        d.swap(i + 1, i + 3);
        i += 4;
    }
}

/// `swapInPlace`: values of data in the reverse byte order brought to the machine's.
fn swap_in_place(r: &mut Reader, item: &ElementItem, bits_allocated: u16, d: &mut [u8]) {
    let pairs = |d: &mut [u8]| {
        for p in d.chunks_exact_mut(2) {
            p.swap(0, 1);
        }
    };
    match item.representation {
        Vr::Us | Vr::Ss | Vr::Ow | Vr::At => {
            if item.tag == TAG_PIXEL_DATA && bits_allocated != 16 {
                return;
            }
            pairs(d);
        }
        Vr::Ul | Vr::Sl => {
            if r.dialect.swap_4byte_in_pairs() && d.len() >= 4 {
                // FS-94: the dialect's swap, in reproduce; corrected otherwise
                if r.notices.reproduce() {
                    r.notices.add(&defects::FS_94, "4-byte integers swapped in 2-byte pairs");
                    pairs(d);
                    return;
                }
                r.notices.add(&defects::FS_94, "4-byte integers reversed");
            }
            let whole = d.len() / 4 * 4;
            for q in d[..whole].chunks_exact_mut(4) {
                q.reverse();
            }
            if whole < d.len() {
                r.notices.add(
                    &defects::FS_95,
                    format!("a {} value of {} bytes", vr_name(item.representation), d.len()),
                );
                pairs(&mut d[whole..]);
            }
        }
        Vr::Fd | Vr::Fl => {
            let w = if item.representation == Vr::Fd { 8 } else { 4 };
            if d.len() >= w {
                r.notices.add(
                    &defects::FS_96,
                    format!(
                        "{} values of ({:04x},{:04x}) {}",
                        vr_name(item.representation),
                        tag_group(item.tag),
                        tag_element(item.tag),
                        if r.notices.fix() {
                            "brought to the machine's byte order"
                        } else {
                            "left in the file's byte order"
                        }
                    ),
                );
                if r.notices.fix() {
                    let whole = d.len() / w * w;
                    for q in d[..whole].chunks_exact_mut(w) {
                        q.reverse();
                    }
                }
            }
        }
        _ => {}
    }
}

fn vr_name(v: Vr) -> &'static str {
    Vr::code_table()
        .iter()
        .find(|(x, _)| *x == v)
        .map(|(_, c)| std::str::from_utf8(c).unwrap_or(""))
        .unwrap_or("")
}

fn check_attribute_order(r: &mut Reader, e: &El, last_group: &mut i64, last_element: &mut i64) -> Condition {
    let group = tag_group(e.tag) as i64;
    let element = tag_element(e.tag) as i64;
    let out_of_order = if group == *last_group {
        element <= *last_element
    } else {
        group < *last_group
    };
    if out_of_order {
        return r.push(
            DCM_ELEMENTOUTOFORDER,
            &[
                Arg::Int(group as i32),
                Arg::Int(element as i32),
                Arg::Str(b"checkAttributeOrder"),
            ],
        );
    }
    *last_group = group;
    *last_element = element;
    DCM_NORMAL
}

/// `computeVM`: the value multiplicity from the representation and length.
fn compute_vm(item: &mut ElementItem) {
    item.multiplicity = match item.representation {
        Vr::Ae
        | Vr::As
        | Vr::Cs
        | Vr::Da
        | Vr::Ds
        | Vr::Dt
        | Vr::Is
        | Vr::Lo
        | Vr::Pn
        | Vr::Sh
        | Vr::Tm
        | Vr::Ui
        | Vr::Ut => {
            let d = match &item.data {
                Data::Bytes(b) => &b[..],
                _ => &[][..],
            };
            let n = (item.length as usize).min(d.len());
            1 + d[..n].iter().filter(|&&c| c == b'\\').count() as u64
        }
        Vr::Fd => (item.length / 8) as u64,
        Vr::At | Vr::Fl | Vr::Sl | Vr::Ul => (item.length / 4) as u64,
        Vr::Ss | Vr::Us => (item.length / 2) as u64,
        _ => 1,
    };
}

/// `ctxSensitiveLookup`: the representation of the elements whose type depends on the object.
fn ctx_sensitive_lookup(o: &DcmObject, e: &mut El) {
    const PIXEL_VALUE_TAGS: [Tag; 11] = [
        make_tag(0x0028, 0x0106),
        make_tag(0x0028, 0x0107),
        make_tag(0x0028, 0x0108),
        make_tag(0x0028, 0x0109),
        make_tag(0x0028, 0x0110),
        make_tag(0x0028, 0x0111),
        make_tag(0x0028, 0x3002),
        make_tag(0x0028, 0x3006),
        make_tag(0x0028, 0x1201),
        make_tag(0x0028, 0x1202),
        make_tag(0x0028, 0x1203),
    ];
    if PIXEL_VALUE_TAGS.contains(&e.tag) {
        e.representation = if o.pixel_representation == 1 {
            Vr::Ss
        } else {
            Vr::Us
        };
    } else if e.tag == TAG_WAVEFORM_DATA && o.waveform_data_vr == b"SS" {
        e.representation = Vr::Ss;
    }
}

/// `updateObjectType`: command or element list, from the first element's group.
fn update_object_type(o: &mut DcmObject, tag: Tag) {
    match o.object_type {
        0 => o.object_type = if tag_group(tag) == 0 { 1 } else { 3 },
        1 if tag_group(tag) != 0 => o.object_type = 3,
        _ => {}
    }
}

/// The value bytes `update_special_elements` reads: the first `n`, those past the value read as
/// zero (FS-92: CTN reads memory never written there).
fn value_bytes(item: &ElementItem, n: usize, notices: &mut Notices) -> Vec<u8> {
    let d: &[u8] = match &item.data {
        Data::Bytes(b) => b,
        _ => &[],
    };
    if d.len() < n {
        notices.add(
            &defects::FS_92,
            format!(
                "({:04x},{:04x}) read as {n} bytes holds {}",
                tag_group(item.tag),
                tag_element(item.tag),
                d.len()
            ),
        );
    }
    let mut v = vec![0u8; n];
    let k = d.len().min(n);
    v[..k].copy_from_slice(&d[..k]);
    v
}

/// C's `strcmp(value, s) == 0` for a value that may hold no zero byte (FS-92: CTN then compares
/// past it; the bytes there read as zero).
fn value_equals(item: &ElementItem, s: &[u8], notices: &mut Notices) -> bool {
    let d: &[u8] = match &item.data {
        Data::Bytes(b) => b,
        _ => &[],
    };
    for (i, &c) in s.iter().chain(std::iter::once(&0u8)).enumerate() {
        let Some(&v) = d.get(i) else {
            notices.add(
                &defects::FS_92,
                format!(
                    "({:04x},{:04x}) compared past its {} bytes",
                    tag_group(item.tag),
                    tag_element(item.tag),
                    d.len()
                ),
            );
            return c == 0;
        };
        if v != c {
            return false;
        }
        if v == 0 {
            return true;
        }
    }
    true
}

/// `updateSpecialElements`: the object's pixel layout, meta group length, transfer syntax and
/// waveform representation from the elements that set them.
fn update_special_elements(o: &mut DcmObject, item: &ElementItem, notices: &mut Notices) {
    match item.tag {
        TAG_BITS_ALLOCATED => {
            let v = value_bytes(item, 2, notices);
            o.pixel_bits_allocated = u16::from_le_bytes([v[0], v[1]]);
        }
        TAG_PIXEL_REPRESENTATION => {
            let v = value_bytes(item, 2, notices);
            o.pixel_representation = u16::from_le_bytes([v[0], v[1]]);
        }
        TAG_META_GROUP_LENGTH => {
            let v = value_bytes(item, 4, notices);
            o.meta_header_length = u32::from_le_bytes([v[0], v[1], v[2], v[3]]) as u64;
        }
        TAG_TRANSFER_SYNTAX => {
            o.data_options = if value_equals(item, b"1.2.840.10008.1.2", notices) {
                opt::ORDERLITTLEENDIAN
            } else if value_equals(item, b"1.2.840.10008.1.2.1", notices) {
                opt::EXPLICITLITTLEENDIAN
            } else if value_equals(item, b"1.2.840.10008.1.2.2", notices) {
                opt::EXPLICITBIGENDIAN
            } else {
                opt::EXPLICITLITTLEENDIAN
            };
        }
        TAG_WAVEFORM_DATA_VR => {
            let d: &[u8] = match &item.data {
                Data::Bytes(b) => b,
                _ => {
                    notices.add(&defects::FS_92, "a Waveform Data Representation without a value");
                    &[]
                }
            };
            let mut n = item.length as usize;
            if n > 16 {
                notices.add(
                    &defects::FS_93,
                    format!("a Waveform Data Representation of {n} bytes"),
                );
                n = 16;
            }
            // strncpy: the value up to its first zero byte, zeros after it, to `length` bytes
            let mut buf = vec![0u8; n + 1];
            for i in 0..n.min(d.len()) {
                if d[i] == 0 {
                    break;
                }
                buf[i] = d[i];
            }
            let mut idx = n as isize - 1;
            while idx >= 0 && buf[idx as usize] == b' ' {
                buf[idx as usize] = 0;
                idx -= 1;
            }
            let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
            o.waveform_data_vr = buf[..end].to_vec();
        }
        _ => {}
    }
}
