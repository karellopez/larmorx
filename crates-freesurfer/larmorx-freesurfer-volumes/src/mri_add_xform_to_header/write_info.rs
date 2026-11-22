// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `MRIwriteInfo(mri, fpref)` (`utils/mriio.cpp` 11852-11905): a COR folder's `COR-.info` from a
//! volume's header, the slices left as they are.
//!
//! The file `<fpref>/COR-.info` is created (`w`); one that cannot be:
//! `MRIwriteInfo(<fpref>): could not open <fpref>/COR-.info.` as an error (`errno` cleared first,
//! so no system message), `ERROR_NO_FILE`. Its lines, each `<name> <value>`: `imnr0`, `imnr1`
//! (`%d`), `ptype` (2 for a coronal slice direction, 0 for an axial one, 1 otherwise), `x` and
//! `y` (the width and height), `fov`, `thick`, `psiz` (millimetres to metres: the `float` value
//! divided by 1000 in `float`, `%f`), `locatn` (`%f`), `strtx`, `endx`, `strty`, `endy`,
//! `strtz`, `endz` (millimetres to metres), `tr`, `te`, `ti` (`%f`), `xform <name>` when the
//! volume has a loaded linear transform, `ras_good_flag` (`%d`), `x_ras`, `y_ras`, `z_ras`,
//! `c_ras` (three `%f` each).
//!
//! Defects (`larmorx_core::defects`; the mechanisms in `docs/findings/freesurfer-add-xform.md`):
//! - **FS-1156** (crash): a folder name of 1,014 bytes or more passes FreeSurfer's name buffer:
//!   the official build aborts (`*** buffer overflow detected ***: terminated`, status 134) before
//!   writing (by default the whole name; `reproduce` aborts as it does).
//! - **FS-1157**: `thick` is written from the pixel size (`ps`), not the slice thickness, so a
//!   folder whose slices are thicker or thinner than its pixels reads back with the pixel size as
//!   its thickness (by default the thickness).
//! - **FS-1158**: the `xform` line is written only when a transform was loaded, so a name given
//!   without loading it (`mri_add_xform_to_header -c`) is lost unless the folder's own transform
//!   had loaded (by default whenever the name is not empty).

use std::io::Write;
use std::path::Path;

use larmorx_core::cfmt;
use larmorx_core::defects::{FS_1156, FS_1157, FS_1158};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{ERROR_NO_FILE, Fatal, NO_ERROR, error_printf};
use larmorx_freesurfer_io::cfile::CFile;
use larmorx_freesurfer_mri::mri::Mri;
use larmorx_freesurfer_volume::coords::{MRI_AXIAL, MRI_CORONAL, get_slice_direction};

/// `INFO_FNAME`.
pub const INFO_FNAME: &str = "COR-.info";
/// `STRLEN`: FreeSurfer's name buffer.
const STRLEN: usize = 1024;

/// `MM_PER_METER` (`1000.0f`): a length in millimetres as metres, in `float`.
fn metres(v: f32) -> String {
    cfmt::f(f64::from(v / 1000.0f32), 6)
}

/// `%f` of a `float`.
fn f6(v: f32) -> String {
    cfmt::f(f64::from(v), 6)
}

/// A C string's bytes: up to its first NUL.
fn cstr(b: &[u8]) -> &[u8] {
    &b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())]
}

/// The C library's message when a checked copy passes its buffer (the official build's
/// `_FORTIFY_SOURCE`), before it aborts the program (status 134).
pub const BUFFER_OVERFLOW: &str = "*** buffer overflow detected ***: terminated\n";

/// `MRIwriteInfo(mri, fpref)`: `NO_ERROR`, or `ERROR_NO_FILE` after its message (see the module);
/// FS-1156's abort with `reproduce` (`Err`, status 134).
pub fn mri_write_info(
    g: &mut Globals,
    err: &mut dyn Write,
    dir: Option<&Path>,
    m: &Mri,
    fpref: &str,
) -> Result<i32, Fatal> {
    let fname = format!("{fpref}/{INFO_FNAME}");
    if fname.len() >= STRLEN {
        g.notices.add(
            &FS_1156,
            if g.notices.reproduce() {
                format!(
                    "MRIwriteInfo: a name of {} bytes passes FreeSurfer's buffer (1,024 bytes): aborted, as FreeSurfer's build aborts",
                    fname.len()
                )
            } else {
                format!(
                    "MRIwriteInfo: a name of {} bytes used whole (FreeSurfer's build aborts: its buffer holds 1,023)",
                    fname.len()
                )
            },
        );
        if g.notices.reproduce() {
            let _ = err.write_all(BUFFER_OVERFLOW.as_bytes());
            return Err(Fatal::Exit(134));
        }
    }
    let Ok(mut fp) = CFile::open(dir, &fname, "w") else {
        g.errno = 0;
        error_printf(
            g,
            err,
            ERROR_NO_FILE,
            &format!("MRIwriteInfo({fpref}): could not open {fname}.\n"),
        );
        return Ok(ERROR_NO_FILE);
    };
    let v = &m.vg;
    let ptype = match get_slice_direction(v) {
        MRI_CORONAL => 2,
        MRI_AXIAL => 0,
        _ => 1,
    };
    let mut s = format!(
        "imnr0 {}\nimnr1 {}\nptype {ptype}\nx {}\ny {}\n",
        m.imnr0, m.imnr1, v.width, v.height
    );
    s.push_str(&format!("fov {}\n", metres(m.fov)));
    if metres(m.thick) != metres(m.ps) {
        g.notices.add(
            &FS_1157,
            if g.notices.reproduce() {
                "MRIwriteInfo: thick written from the pixel size, as in FreeSurfer"
            } else {
                "MRIwriteInfo: thick written from the slice thickness (FreeSurfer writes the pixel size)"
            },
        );
    }
    let thick = if g.notices.reproduce() { m.ps } else { m.thick };
    s.push_str(&format!("thick {}\n", metres(thick)));
    s.push_str(&format!("psiz {}\n", metres(m.ps)));
    s.push_str(&format!("locatn {}\n", f6(m.location)));
    for (name, x) in [
        ("strtx", m.xstart),
        ("endx", m.xend),
        ("strty", m.ystart),
        ("endy", m.yend),
        ("strtz", m.zstart),
        ("endz", m.zend),
    ] {
        s.push_str(&format!("{name} {}\n", metres(x)));
    }
    s.push_str(&format!(
        "tr {}\nte {}\nti {}\n",
        f6(m.tr),
        f6(m.te),
        f6(m.ti)
    ));
    let mut b = s.into_bytes();
    let name = cstr(&m.transform_fname);
    let loaded = m.linear_transform.is_some();
    if loaded != !name.is_empty() {
        g.notices.add(
            &FS_1158,
            if loaded {
                "MRIwriteInfo: no xform line for an empty transform name (FreeSurfer writes `xform ` for a loaded transform)"
            } else if g.notices.reproduce() {
                "MRIwriteInfo: the transform name not written, no transform being loaded, as in FreeSurfer"
            } else {
                "MRIwriteInfo: the transform name written though no transform was loaded (FreeSurfer drops it)"
            },
        );
    }
    let write_xform = if g.notices.reproduce() {
        loaded
    } else {
        !name.is_empty()
    };
    if write_xform {
        b.extend_from_slice(b"xform ");
        b.extend_from_slice(name);
        b.push(b'\n');
    }
    let mut t = format!("ras_good_flag {}\n", v.ras_good_flag);
    for (n, x, y, z) in [
        ("x_ras", v.x_r, v.x_a, v.x_s),
        ("y_ras", v.y_r, v.y_a, v.y_s),
        ("z_ras", v.z_r, v.z_a, v.z_s),
        ("c_ras", v.c_r, v.c_a, v.c_s),
    ] {
        t.push_str(&format!("{n} {} {} {}\n", f6(x), f6(y), f6(z)));
    }
    b.extend_from_slice(t.as_bytes());
    fp.put_bytes(&b);
    fp.close();
    Ok(NO_ERROR)
}
