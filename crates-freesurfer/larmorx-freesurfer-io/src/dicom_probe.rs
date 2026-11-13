// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The DICOM pieces of FreeSurfer 8.2.0's format probe (`utils/DICOMRead.cpp`, taken early by queue
//! item 3.1-27 for `mri_identify`; DICOM input itself is item 3.7-14): `IsDICOM` (its four opens
//! through CTN, its `static` answer for the last name, `FS_DICOM_DEBUG`'s messages),
//! `IsSiemensDICOM` (the Manufacturer element (0008,0070)), and the parts of `GetObjectFromFile`,
//! `GetElementFromFile`, `AllocElementData` and `FreeElementData` that the Manufacturer element
//! reaches, on larmorx-io's pure-Rust reader of CTN 3.0.6's `DCM_OpenFile`
//! (`larmorx_io::dicom`, queue item 3.1-16) with FreeSurfer's two changes to CTN ([`FsDialect`]).
//! `DCM_GetElement` and `DCM_GetElementValue` for that one element are written here from CTN's
//! behaviour (queue item 3.1-56 brings them whole to larmorx-io; this module then uses those).
//!
//! FS-227 (a numeric Manufacturer longer than its slot), FS-228 (a Manufacturer whose
//! representation `AllocElementData` does not handle, or of undefined length: FreeSurfer crashes),
//! FS-221 (`IsDICOM`'s 1,024-byte name).

use std::io::Write;
use std::path::Path;

use larmorx_core::cfmt::Streams;
use larmorx_core::defects::{FS_221, FS_227, FS_228};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_core::utils;
use larmorx_io::dicom::cond::{self, Arg, Condition};
use larmorx_io::dicom::{self, Data, DcmObject, Dialect, Vr, make_tag, opt};

use crate::cfile::CFile;
use crate::fio;

/// FreeSurfer's changes to its copy of CTN: the longer "Data Element ... longer" message (a line
/// splice of its reformatted source) and 4-byte integers of the reverse byte order swapped in
/// 2-byte pairs (FS-94, corrected by default in larmorx-io).
#[derive(Clone, Copy, Debug, Default)]
pub struct FsDialect;

const FS_ELEMENT_LENGTH_ERROR: &[u8] = b"DCM Data Element (%04x %04x) longer (%ld) than remaining length (%ld) of                                    data in stream or file in %s";

impl Dialect for FsDialect {
    fn message(&self, c: Condition) -> Option<&'static [u8]> {
        if c == cond::DCM_ELEMENTLENGTHERROR {
            Some(FS_ELEMENT_LENGTH_ERROR)
        } else {
            cond::ctn_message(c)
        }
    }
    fn swap_4byte_in_pairs(&self) -> bool {
        true
    }
}

/// The C streams as `larmorx_core::cfmt`'s [`Streams`] (CTN's messages).
struct Std<'a> {
    out: &'a mut dyn Write,
    err: &'a mut dyn Write,
}

impl Streams for Std<'_> {
    fn out(&mut self, text: &str) {
        let _ = self.out.write_all(text.as_bytes());
    }
    fn err(&mut self, text: &str) {
        let _ = self.err.write_all(text.as_bytes());
    }
}

/// `COND_DumpConditions()`: CTN's condition stack, bottom first, on standard error (each entry's
/// code in 8 hexadecimal columns, a space, its text's bytes as they are), then emptied.
/// (`larmorx_io::dicom::CondStack::dump` writes through text streams, which cannot carry a
/// message's bytes that are not UTF-8, such as a broken representation code.)
pub fn dump_conditions(g: &mut Globals, err: &mut dyn Write) {
    if !g.dicom.stack.entries().is_empty() {
        g.dicom.stack.stderr_oriented = true;
    }
    for (c, t) in g.dicom.stack.entries() {
        let _ = write!(err, "{:8x} ", *c as u32);
        let _ = err.write_all(t);
        let _ = err.write_all(b"\n");
    }
    g.dicom.stack.pop(true);
}

/// `IsDICOM`'s `static char file[1024]`.
const IS_DICOM_FILE: usize = 1024;

/// `DCM_OpenFile(name, options, &object)` on CTN's process stack, `errno` shared with the
/// program's.
fn open(
    g: &mut Globals,
    s: &mut Std,
    path: &Path,
    options: u64,
    object: &mut Option<DcmObject>,
) -> Condition {
    g.dicom.stack.errno = g.errno;
    let c = dicom::open_file(
        path,
        options,
        object,
        &FsDialect,
        &mut g.dicom.stack,
        s,
        &mut g.notices,
    )
    // FreeSurfer's options are always ones the reader follows (a byte order or part 10, with
    // DCM_ACCEPTVRMISMATCH; DCM_FORMATCONVERSION alone is refused before the file is opened)
    .expect("an option FreeSurfer never passes");
    g.errno = g.dicom.stack.errno;
    c
}

fn close(g: &mut Globals, s: &mut Std, object: &mut Option<DcmObject>) {
    dicom::close_object(object, &FsDialect, &mut g.dicom.stack, s, &mut g.notices);
}

/// `DCMPrintCond(cond)`: the condition's name on standard output.
fn print_cond(s: &mut Std, c: Condition) {
    let name = match c {
        cond::DCM_NORMAL => "DCM_NORMAL".to_owned(),
        cond::DCM_ILLEGALOPTION => "DCM_ILLEGALOPTION".to_owned(),
        cond::DCM_OBJECTCREATEFAILED => "DCM_OBJECTCREATEFAILED".to_owned(),
        cond::DCM_FILEOPENFAILED => "DCM_FILEOPENFAILED".to_owned(),
        cond::DCM_FILEACCESSERROR => "DCM_FILEACCESSERROR".to_owned(),
        cond::DCM_ELEMENTOUTOFORDER => "DCM_ELEMENTOUTOFORDER".to_owned(),
        other => format!("DCMPrintCond: {} unrecognized", other as i32),
    };
    s.out(&format!("{name}\n"));
}

/// `IsDICOM(fname)`: 1 when CTN opens the file as DICOM: as a part-10 file, else as a bare data
/// set in implicit little endian, then big endian (each with `DCM_ACCEPTVRMISMATCH`; a fourth
/// attempt with `DCM_FORMATCONVERSION` alone is always refused), the object closed after each
/// failure and at the end, CTN's condition stack cleared before the first and after the last. 0
/// for a file that does not open for reading (`errno` as `fopen` left it) or is a directory. The
/// answer for the last name asked is kept and given again for the same name without opening it,
/// unless `FS_DICOM_DEBUG` is set, which prints each step (`Entering IsDICOM (<f>)`, `Opening <f>
/// as part10`, the failed attempts' condition names, the stack after the first, `Leaving IsDICOM
/// (<f>)`).
pub fn is_dicom(
    g: &mut Globals,
    dir: Option<&Path>,
    env: &Environment,
    out: &mut dyn Write,
    err: &mut dyn Write,
    fname: &str,
) -> i32 {
    let mut s = Std { out, err };
    let d = env.get("FS_DICOM_DEBUG").is_some();
    if !d && g.dicom.is_dicom_file == fname.as_bytes() {
        return g.dicom.is_dicom_yes;
    }
    g.dicom.is_dicom_yes = 0;
    if fname.len() >= IS_DICOM_FILE {
        g.notices.add(
            &FS_221,
            format!(
                "IsDICOM: a name of {} bytes kept whole (FreeSurfer's static copy holds {IS_DICOM_FILE})",
                fname.len()
            ),
        );
    }
    g.dicom.is_dicom_file = fname.as_bytes().to_vec();
    if d {
        s.out(&format!("Entering IsDICOM ({fname})\n"));
    }
    match CFile::open(dir, fname, "r") {
        Ok(f) => {
            f.close();
        }
        Err(e) => {
            g.errno = e;
            return 0;
        }
    }
    if fio::fio_is_directory(g, dir, fname) != 0 {
        return 0;
    }
    let path = utils::resolve(dir, fname);
    if d {
        s.out(&format!("Opening {fname} as part10\n"));
    }
    g.dicom.stack.pop(true);
    let mut object = None;
    let mut c = open(
        g,
        &mut s,
        &path,
        opt::PART10FILE | opt::ACCEPTVRMISMATCH,
        &mut object,
    );
    if c != cond::DCM_NORMAL && d {
        print_cond(&mut s, c);
        dump_conditions(g, s.err);
    }
    for (what, o) in [
        ("littleendian", opt::ORDERLITTLEENDIAN),
        ("bigendian", opt::ORDERBIGENDIAN),
        ("format conversion", opt::FORMATCONVERSION),
    ] {
        if c == cond::DCM_NORMAL {
            break;
        }
        close(g, &mut s, &mut object);
        if d {
            s.out(&format!("Opening as {what}\n"));
        }
        c = open(g, &mut s, &path, o | opt::ACCEPTVRMISMATCH, &mut object);
        if c != cond::DCM_NORMAL && d {
            print_cond(&mut s, c);
        }
    }
    close(g, &mut s, &mut object);
    if d {
        s.out(&format!("Leaving IsDICOM ({fname})\n"));
    }
    g.dicom.stack.pop(true);
    g.dicom.is_dicom_yes = i32::from(c == cond::DCM_NORMAL);
    g.dicom.is_dicom_yes
}

/// `GetObjectFromFile(fname, 0)`: the object as `IsDICOM`'s attempts open it (the same four, with
/// the stack not cleared between them), or the stack printed and `None` when none does.
fn object_from_file(
    g: &mut Globals,
    dir: Option<&Path>,
    env: &Environment,
    s: &mut Std,
    fname: &str,
) -> Option<DcmObject> {
    if is_dicom(g, dir, env, s.out, s.err, fname) == 0 {
        s.err(&format!("ERROR: {fname} is not a dicom file\n"));
        dump_conditions(g, s.err);
        return None;
    }
    let path = utils::resolve(dir, fname);
    let mut object = None;
    let mut c = open(
        g,
        s,
        &path,
        opt::PART10FILE | opt::ACCEPTVRMISMATCH,
        &mut object,
    );
    for o in [
        opt::ORDERLITTLEENDIAN,
        opt::ORDERBIGENDIAN,
        opt::FORMATCONVERSION,
    ] {
        if c == cond::DCM_NORMAL {
            break;
        }
        close(g, s, &mut object);
        c = open(g, s, &path, o | opt::ACCEPTVRMISMATCH, &mut object);
    }
    if c != cond::DCM_NORMAL {
        close(g, s, &mut object);
        dump_conditions(g, s.err);
        return None;
    }
    object
}

/// The value bytes `AllocElementData` gives room for, and whether it handles the representation:
/// text (and OB, OW, UN) the value's length and 17 bytes more, the numbers one item (FD: one per
/// value), a sequence nothing (with its message), the others nothing (with its message).
enum Room {
    Text,
    Items(u64),
    Sequence,
    Unhandled,
}

fn room_of(v: Vr, multiplicity: u64) -> Room {
    match v {
        Vr::Ae
        | Vr::As
        | Vr::Cs
        | Vr::Da
        | Vr::Ds
        | Vr::Dt
        | Vr::Is
        | Vr::Lo
        | Vr::Lt
        | Vr::Ob
        | Vr::Ow
        | Vr::Pn
        | Vr::Sh
        | Vr::Un
        | Vr::St
        | Vr::Tm
        | Vr::Ui => Room::Text,
        Vr::Ss | Vr::Us => Room::Items(2),
        Vr::Sl | Vr::Ul | Vr::At | Vr::Fl => Room::Items(4),
        Vr::Fd => Room::Items(8 * multiplicity),
        Vr::Sq => Room::Sequence,
        _ => Room::Unhandled,
    }
}

/// `FreeElementData`'s list: the representations it frees (the others print a line on standard
/// error, without a newline, and are kept).
fn freed(v: Vr) -> bool {
    !matches!(
        v,
        Vr::Un | Vr::Ot | Vr::Ret | Vr::Ctx | Vr::Dlm | Vr::Ut | Vr::Dd
    )
}

fn free_element_data(s: &mut Std, v: Vr) {
    if !freed(v) {
        s.err(&format!("FreeElementData: {} unrecognized", v as u32));
    }
}

/// The Manufacturer element's value as `GetElementFromFile(dcmfile, 0x8, 0x70)` gives it to
/// `IsSiemensDICOM`: `Ok(Some(text))` (the value up to its first zero byte), `Ok(None)` when the
/// element is missing or cannot be read (its condition pushed), `Err` when the program exits.
fn manufacturer(
    g: &mut Globals,
    dir: Option<&Path>,
    env: &Environment,
    s: &mut Std,
    dcmfile: &str,
) -> Result<Option<Vec<u8>>, Fatal> {
    let Some(object) = object_from_file(g, dir, env, s, dcmfile) else {
        return Err(Fatal::Exit(1));
    };
    let tag = make_tag(0x8, 0x70);
    let mut object = Some(object);
    // DCM_GetElement: the first group of the number, then its first element of the tag
    let found = object
        .as_ref()
        .and_then(|o| o.groups.iter().find(|gr| gr.group == 0x8))
        .and_then(|gr| gr.elements.iter().find(|e| e.tag == tag))
        .cloned();
    let Some(item) = found else {
        let m = FsDialect.message(cond::DCM_ELEMENTNOTFOUND);
        g.dicom.stack.push(
            cond::DCM_ELEMENTNOTFOUND,
            m,
            &[Arg::U32(0x8), Arg::U32(0x70), Arg::Str(b"DCM_GetElementVM")],
            s,
            &mut g.notices,
        );
        close(g, s, &mut object);
        return Ok(None);
    };
    let v = item.representation;
    let room = room_of(v, item.multiplicity);
    match room {
        Room::Sequence => s.err("Element is of type dcm_sq, not supported\n"),
        Room::Unhandled => s.err(&format!("AllocElementData: {} unrecognized\n", v as u32)),
        _ => {}
    }
    if let Room::Sequence = room {
        // DCM_GetElementValue refuses a sequence
        let m = FsDialect.message(cond::DCM_CANNOTGETSEQUENCEVALUE);
        g.dicom.stack.push(
            cond::DCM_CANNOTGETSEQUENCEVALUE,
            m,
            &[Arg::U32(tag), Arg::Str(b"DCM_GetElementValue")],
            s,
            &mut g.notices,
        );
        close(g, s, &mut object);
        free_element_data(s, v);
        return Ok(None);
    }
    let bytes = match (&item.data, &room) {
        (Data::Bytes(b), Room::Text | Room::Items(_)) => b.clone(),
        _ => {
            // FS-228: a representation AllocElementData leaves without room, or a value never
            // read (an undefined length): FreeSurfer writes through a null or wild pointer
            g.notices.add(
                &FS_228,
                format!(
                    "IsSiemensDICOM: the Manufacturer element ({}, length {}) taken for no manufacturer (FreeSurfer crashes)",
                    vr_code(v),
                    item.length
                ),
            );
            close(g, s, &mut object);
            g.dicom.stack.pop(true);
            return Ok(Some(Vec::new()));
        }
    };
    if let Room::Items(n) = room
        && bytes.len() as u64 > n
    {
        g.notices.add(
            &FS_227,
            format!(
                "IsSiemensDICOM: a {}-byte {} Manufacturer read as text (FreeSurfer copies it into {n} bytes)",
                bytes.len(),
                vr_code(v)
            ),
        );
    }
    close(g, s, &mut object);
    g.dicom.stack.pop(true);
    let n = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    free_element_data(s, v);
    Ok(Some(bytes[..n].to_vec()))
}

fn vr_code(v: Vr) -> String {
    format!("{v:?}").to_uppercase()
}

/// `IsSiemensDICOM(dcmfile)`: 1 when the file is DICOM ([`is_dicom`]) and its Manufacturer
/// element (0008,0070) reads `SIEMENS` or `SIEMENS ` (as text, up to its first zero byte); else
/// 0, after `WARNING: searching dicom file <f> for Manufacturer tag 0x8, 0x70` and `WARNING: the
/// result could be a mess.` (standard output) when the element is missing or is a sequence
/// (`Element is of type dcm_sq, not supported` on standard error first). The element's other
/// representations print `AllocElementData`'s or `FreeElementData`'s lines as FreeSurfer does
/// (FS-227, FS-228). `Err`: the program exits with status 1 (a file `IsDICOM` accepted that no
/// longer opens).
pub fn is_siemens_dicom(
    g: &mut Globals,
    dir: Option<&Path>,
    env: &Environment,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dcmfile: &str,
) -> Result<i32, Fatal> {
    if is_dicom(g, dir, env, out, err, dcmfile) == 0 {
        return Ok(0);
    }
    let mut s = Std { out, err };
    let Some(value) = manufacturer(g, dir, env, &mut s, dcmfile)? else {
        s.out(&format!(
            "WARNING: searching dicom file {dcmfile} for Manufacturer tag 0x8, 0x70\n"
        ));
        s.out("WARNING: the result could be a mess.\n");
        return Ok(0);
    };
    Ok(i32::from(value == b"SIEMENS" || value == b"SIEMENS "))
}
