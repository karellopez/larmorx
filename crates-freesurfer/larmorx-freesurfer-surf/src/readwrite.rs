// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! A surface written and read in the format its name says (queue item 3.1-48, relays 15 and 16):
//! `MRISwrite` (with `MRISwrite_new`, the new behaviour FreeSurfer 8.2.0 always takes),
//! `MRISwriteVertexLocations`, `MRISwritePatchAscii` and `MRISreadVerticesOnly`
//! (`utils/mrisurf_io.cpp` 3276-3340, 4279-4340, 4414-4451, 5226-5317); `MRISread`,
//! `MRISfastRead`, `MRISreadOverAlloc` (with `MRISreadOverAlloc_new`, the reader FreeSurfer 8.2.0
//! always takes) and `mrisReadTransform` (3519-3791, 4214-4277, 4617-4750), on fs.surf's pieces
//! taken early ([`crate::nbhd`], [`crate::ico`]). FS-351, FS-356, FS-359, FS-360.

use std::io::Write;
use std::path::Path;

use larmorx_core::defects::{FS_351, FS_356, FS_359, FS_360};
use larmorx_freesurfer_core::chklc::chklc;
use larmorx_freesurfer_core::cstd::fmt_f;
use larmorx_freesurfer_core::diag::{DIAG_SHOW, verbose_on};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{
    self, ERROR_BADFILE, ERROR_BADPARM, ERROR_NO_FILE, ERROR_NOFILE, ERROR_NOMEMORY,
    ERROR_UNSUPPORTED, Fatal, NO_ERROR,
};
use larmorx_freesurfer_core::matrix_ops::matrix_inverse;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_core::utils::{file_exists, file_name_path, resolve};
use larmorx_freesurfer_io::fio::{
    fio_extension, fio_file_exists_readable, fread_float, fread2, fread3,
};
use larmorx_freesurfer_io::identify::{MRI_MGH_FILE, MRI_VOLUME_TYPE_UNKNOWN, NII_FILE};
use larmorx_freesurfer_io::mriio;
use larmorx_freesurfer_io::registerio::{self, NoLta, Register};
use larmorx_freesurfer_mri::lta::LINEAR_RAS_TO_RAS;
use larmorx_freesurfer_mri::vol_geom::VolGeom;
use larmorx_freesurfer_volume::ltaread::{LtaIo, RegisterDat, lta_read_ex};
use larmorx_freesurfer_volume::vgeom::get_vol_geom;

use crate::aaa::{SurfGlobals, VERTICES_PER_FACE};
use crate::alloc;
use crate::coords::{self, Io};
use crate::curvio::{apply_fsgii_read, apply_fsgii_write};
use crate::formats::{self, MRIS_TRIANGULAR_SURFACE};
use crate::ico;
use crate::metric;
use crate::mp;
use crate::mp_slow;
use crate::mris::Mris;
use crate::mrisurf::{SmallSurface, SmallVertex};
use crate::names::{
    MRIS_ASCII_TRIANGLE_FILE, MRIS_GEO_TRIANGLE_FILE, MRIS_GIFTI_FILE, MRIS_ICO_FILE,
    MRIS_STL_FILE, MRIS_VOLUME_FILE, MRIS_VTK_FILE, mris_file_name_type,
};
use crate::nbhd;
use crate::positions;
use crate::sfile::{self, c_str, nonneg, open, open_failed, strcpy_checked};
use crate::stl;
use crate::surfread::{self, read_triangle_file_vertex_positions_only, verbose_show};
use crate::topology;
use crate::trifile::{
    self, NEW_QUAD_FILE_MAGIC_NUMBER, QUAD_FILE_MAGIC_NUMBER, TRIANGLE_FILE_MAGIC_NUMBER,
};
use crate::views::CURRENT_VERTICES;

/// The types `MRISwrite` and `MRISread` take from the name as they are (the others may become
/// GIFTI through `FS_GII`).
fn named_type(t: i32) -> bool {
    [
        MRIS_ASCII_TRIANGLE_FILE,
        MRIS_ICO_FILE,
        MRIS_GEO_TRIANGLE_FILE,
        MRIS_STL_FILE,
        MRIS_VTK_FILE,
        MRIS_GIFTI_FILE,
    ]
    .contains(&t)
}

/// GIFTI surfaces are written and read by `utils/gifti.cpp` 213-2857 (the surface part of queue
/// item 3.1-26, this crate's next piece); until it is ported, reaching it is out of range.
fn gifti_pending(what: &str) -> ! {
    panic!("{what}: GIFTI surfaces (gifti.cpp's surface part) are not yet ported")
}

/// `MRISwrite(mris, name)`: the surface written in the format its name says (`MRISfileNameType`):
/// ASCII, VTK, movie.byu, icosahedron, STL, GIFTI; any other name, after `__MRISapplyFSGIIwrite`
/// (which may make it a GIFTI name, or end the program when the other form exists), a triangle
/// file for a surface of type `MRIS_TRIANGULAR_SURFACE`, else a quadrangle file. The name is
/// copied into a 1,024-byte buffer (FS-126: FreeSurfer's buffer-overflow abort at 1,024 bytes or
/// more). Returns the writer's answer (STL: 1 written, 0 not). FreeSurfer 8.2.0 always takes the
/// new behaviour (`MRISwrite_new`); the licence check (`chklc`) passes.
pub fn write(
    g: &mut Globals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &mut Mris,
    name: &str,
) -> Result<i32, Fatal> {
    let name = c_str(name);
    let mut surf_to_write = strcpy_checked(g, io, name, "MRISwrite")?;
    let mut ty = mris_file_name_type(g, &surf_to_write);
    if !named_type(ty) {
        apply_fsgii_write(g, io, dir, &mut surf_to_write, name, &mut ty)?;
    }
    write_new(g, io, dir, mris, &surf_to_write)
}

/// `MRISwrite_new(mris, name)`.
fn write_new(
    g: &mut Globals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &mut Mris,
    name: &str,
) -> Result<i32, Fatal> {
    chklc();
    let fname = strcpy_checked(g, io, name, "MRISwrite_new")?;
    Ok(match mris_file_name_type(g, &fname) {
        MRIS_ASCII_TRIANGLE_FILE => sfile::write_ascii(g, io, dir, mris, &fname),
        MRIS_VTK_FILE => sfile::write_vtk(g, io, dir, mris, &fname),
        MRIS_GEO_TRIANGLE_FILE => formats::write_geo(g, io, dir, mris, &fname),
        MRIS_ICO_FILE => formats::write_ico(g, io, dir, mris, &fname),
        MRIS_STL_FILE => i32::from(stl::write_stl(g, dir, mris, &fname)),
        MRIS_GIFTI_FILE => gifti_pending("MRISwrite"),
        _ if mris.r#type == MRIS_TRIANGULAR_SURFACE => {
            trifile::write_triangular_surface(g, io, dir, mris, &fname)?
        }
        _ => trifile::write_quadrangle_file(g, io, dir, mris, &fname)?,
    })
}

/// `MRISwriteVertexLocations(mris, fname, which_vertices)`: the surface written (`MRISwrite`) with
/// the coordinate set `which_vertices` restored as its current one, then its current coordinates
/// put back (its distances freed: `MRISimportVertexCoords`). Returns `MRISwrite`'s answer.
pub fn write_vertex_locations(
    g: &mut Globals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &mut Mris,
    fname: &str,
    which_vertices: i32,
) -> Result<i32, Fatal> {
    let n = nonneg(mris.nvertices);
    let mut coords3: [Vec<f32>; 3] = [vec![0.0; n], vec![0.0; n], vec![0.0; n]];
    metric::extract_vertex_coords(g, io, mris, &mut coords3, CURRENT_VERTICES)?;
    positions::restore_vertex_positions(g, io, mris, which_vertices)?;
    let retval = write(g, io, dir, mris, fname)?;
    let [x, y, z] = &coords3;
    coords::import_vertex_coords(g, io, mris, [x, y, z], CURRENT_VERTICES)?;
    Ok(retval)
}

/// `MRISwritePatchAscii(mris, fname)`: an ASCII patch: `#!ascii version of patch <surface name>.
/// The 1st index is not a vertex number`, the counts of unripped vertices and faces, per unripped
/// vertex `<index> vno=<vno>` (the index `vno + 1`, negative for a border vertex) and its
/// coordinates (`%f`, two spaces apart), per unripped face its number and its three vertices
/// (`%d ` each); on standard output `nvertices=<n> (valid=<MRISvalidVertices>) nfaces=<n>`.
/// Returns `NO_ERROR`, or `ERROR_NOFILE` (`MRISwritePatchAscii: could not open file <file>`).
pub fn write_patch_ascii(
    g: &mut Globals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &Mris,
    fname: &str,
) -> i32 {
    let fname = c_str(fname);
    // the type is computed and not used
    let _ = mris_file_name_type(g, fname);
    let mut fp = match open(dir, fname, "w") {
        Ok(f) => f,
        Err(e) => {
            let m = format!("MRISwritePatchAscii: could not open file {fname}");
            return open_failed(g, io, e, ERROR_NOFILE, &m);
        }
    };
    let nv = nonneg(mris.nvertices);
    let nf = nonneg(mris.nfaces);
    let nvertices = (0..nv).filter(|&k| mris.vertex(k).ripflag == 0).count();
    let nfaces = (0..nf).filter(|&k| mris.face(k).ripflag == 0).count();
    fp.put_bytes(
        format!(
            "#!ascii version of patch {}. The 1st index is not a vertex number\n",
            c_str(&mris.fname)
        )
        .as_bytes(),
    );
    fp.put_bytes(format!("{nvertices} {nfaces}\n").as_bytes());
    let _ = writeln!(
        io.out,
        "nvertices={nvertices} (valid={}) nfaces={nfaces}",
        topology::valid_vertices(mris)
    );
    for vno in 0..nv {
        let v = mris.vertex(vno);
        if v.ripflag != 0 {
            continue;
        }
        let i = if v.border != 0 {
            -(vno as i64 + 1)
        } else {
            vno as i64 + 1
        };
        fp.put_bytes(format!("{i} vno={vno}\n").as_bytes());
        fp.put_bytes(
            format!(
                "{}  {}  {}\n",
                fmt_f(f64::from(v.x), 0, 6),
                fmt_f(f64::from(v.y), 0, 6),
                fmt_f(f64::from(v.z), 0, 6)
            )
            .as_bytes(),
        );
    }
    for fno in 0..nf {
        let f = mris.face(fno);
        if f.ripflag != 0 {
            continue;
        }
        let mut s = format!("{fno}\n");
        for v in f.v {
            s += &format!("{v} ");
        }
        s.push('\n');
        fp.put_bytes(s.as_bytes());
    }
    fp.close();
    NO_ERROR
}

/// `MRISreadVerticesOnly(fname)`: the vertex positions of a surface file. ASCII, icosahedron and
/// movie.byu names are refused (`MRISreadVerticesOnly: file type <t> not supported`,
/// `ERROR_UNSUPPORTED`); a file that cannot be opened is `MRISread(<file>): could not open file`
/// (`ERROR_NOFILE`). A triangle file's positions come from its own reader (`mrisReadTriangleFile
/// failed.` when it fails, with its code); a quadrangle file (magic number, or none: the old
/// format, read from the start) gives the 3-byte vertex and quadrangle counts, then per vertex three
/// 2-byte hundredths (the magic number `0xffffff`) or three floats (the new or old format). A
/// count of 0 vertices is `MRISreadVerticesOnly: could not allocate surface` (`ERROR_NOMEMORY`).
/// FreeSurfer leaves a quadrangle file open; larmorx closes it (nothing else differs).
pub fn read_vertices_only(
    g: &mut Globals,
    io: &mut Io,
    dir: Option<&Path>,
    fname: &str,
) -> Result<Option<SmallSurface>, Fatal> {
    let fname = c_str(fname);
    let ty = mris_file_name_type(g, fname);
    if [
        MRIS_ASCII_TRIANGLE_FILE,
        MRIS_ICO_FILE,
        MRIS_GEO_TRIANGLE_FILE,
    ]
    .contains(&ty)
    {
        log::error_printf(
            g,
            io.err,
            ERROR_UNSUPPORTED,
            &format!("MRISreadVerticesOnly: file type {ty} not supported"),
        );
        return Ok(None);
    }
    let mut fp = match open(dir, fname, "rb") {
        Ok(f) => f,
        Err(e) => {
            g.errno = e;
            let m = format!("MRISread({fname}): could not open file");
            log::error_printf(g, io.err, ERROR_NOFILE, &m);
            return Ok(None);
        }
    };
    let (_, magic) = fread3(&mut fp);
    let version = if magic == QUAD_FILE_MAGIC_NUMBER {
        if verbose_show(g) {
            let _ = io.out.write_all(b"new surface file format\n");
        }
        -1
    } else if magic == NEW_QUAD_FILE_MAGIC_NUMBER {
        -2
    } else if magic == TRIANGLE_FILE_MAGIC_NUMBER {
        fp.close();
        let s = read_triangle_file_vertex_positions_only(g, io, dir, fname)?;
        if s.is_none() {
            let code = g.gerror;
            log::error_printf(g, io.err, code, "mrisReadTriangleFile failed.\n");
        }
        return Ok(s);
    } else {
        fp.rewind();
        if verbose_show(g) {
            let _ = io.out.write_all(b"surfer: old surface file format\n");
        }
        0
    };
    let (_, nvertices) = fread3(&mut fp);
    let (_, nquads) = fread3(&mut fp);
    if verbose_show(g) {
        let _ = writeln!(
            io.out,
            "reading {nvertices} vertices and {} faces.",
            2 * nquads
        );
    }
    if nvertices == 0 {
        log::error_printf(
            g,
            io.err,
            ERROR_NOMEMORY,
            "MRISreadVerticesOnly: could not allocate surface",
        );
        return Ok(None);
    }
    let mut vertices = Vec::with_capacity(nonneg(nvertices));
    for _ in 0..nvertices {
        let v = if version == -1 {
            let (_, ix) = fread2(g, &mut fp);
            let (_, iy) = fread2(g, &mut fp);
            let (_, iz) = fread2(g, &mut fp);
            let c = |i: i32| (f64::from(i) / 100.0) as f32;
            SmallVertex {
                x: c(ix),
                y: c(iy),
                z: c(iz),
            }
        } else {
            let x = fread_float(g, io.err, &mut fp);
            let y = fread_float(g, io.err, &mut fp);
            let z = fread_float(g, io.err, &mut fp);
            SmallVertex { x, y, z }
        };
        vertices.push(v);
    }
    fp.close();
    Ok(Some(SmallSurface {
        nvertices,
        vertices,
    }))
}

// ---- MRISread ---------------------------------------------------------------------------------

/// `RIGHT_HEMISPHERE`, `NO_HEMISPHERE`, `BOTH_HEMISPHERES` (`mrisurf.h` lines 1069-1071).
pub const RIGHT_HEMISPHERE: i32 = 1;
pub const NO_HEMISPHERE: i32 = 2;
pub const BOTH_HEMISPHERES: i32 = 3;

/// fs.io's readers behind track C's `LtaIo` (`LTAreadEx`'s calls): `MRIreadHeader` (track A's
/// `mriio::mri_read_header`, any format, with the program's environment) for a transform whose
/// comment names a volume or for `mri/orig`, and `regio_read_register` (track A's `registerio`;
/// an `.lta` registration is refused with its message, as `MRISread`'s transform is always
/// `talairach.xfm`, never a register file).
struct FsLtaIo<'e> {
    env: &'e Environment,
}

impl LtaIo for FsLtaIo<'_> {
    fn env(&self) -> &Environment {
        self.env
    }

    fn read_header(
        &mut self,
        g: &mut Globals,
        out: &mut dyn Write,
        err: &mut dyn Write,
        dir: Option<&Path>,
        name: &str,
    ) -> Result<Option<VolGeom>, Fatal> {
        Ok(
            mriio::mri_read_header(g, out, err, self.env, dir, name, MRI_VOLUME_TYPE_UNKNOWN)?
                .map(|m| m.vg),
        )
    }

    fn read_register(
        &mut self,
        g: &mut Globals,
        out: &mut dyn Write,
        err: &mut dyn Write,
        dir: Option<&Path>,
        name: &str,
    ) -> Result<Option<RegisterDat>, Fatal> {
        let mut reg = Register::default();
        if registerio::regio_read_register(g, out, err, dir, name, &mut reg, &mut NoLta)? != 0 {
            return Ok(None);
        }
        Ok(reg.r.map(|r| RegisterDat {
            subject: reg.subject.unwrap_or_default(),
            inplaneres: reg.inplaneres,
            betplaneres: reg.betplaneres,
            intensity: reg.intensity,
            r,
            float2int: reg.float2int,
        }))
    }
}

/// `MRIread` (fs.io's volume reader) and `MRIScopyMRI`: reached only when a group-average area
/// file `<surface>.avg.area.mgh` is readable; until they are ported, reaching them is out of range.
fn mri_read_pending(fname: &str) -> ! {
    panic!("MRISread: reading {fname} needs MRIread and MRIScopyMRI (not yet ported)")
}

/// `mrisReadTransform(mris, mris_fname)` (with `MRISreadTransform`, its public name): the
/// subject's Talairach transform, `<the surface name's folder>/../mri/transforms/talairach.xfm`
/// (`FileNamePath`'s folder, `.` without one; FS-356: a surface name of 300 bytes or more is taken
/// whole). Without that file (`fopen` fails), nothing (`ERROR_NO_FILE`). Else `LTAreadEx` (fs.volume,
/// track C's `ltaread`): a transform it cannot read gives `mrisReadTransform: could not read xform
/// file '<name>'` (`ERROR_NOFILE`) and `ERROR_NO_FILE`; one whose type is not `LINEAR_RAS_TO_RAS`
/// ends the program (`the transform is not RAS-TO-RAS.  not supported.`, `ERROR_BADPARM`). The
/// first transform's source centre, when its source geometry is not valid: the surface's own
/// (`vg`), when valid; else that of `<folder>/../mri/orig` when it is a regular file whose header
/// reads (then the source geometry and the surface's `vg` both become that volume's: `getVolGeom`);
/// else 0. With a valid source geometry and a valid surface geometry whose centres differ by
/// 1e-5 or more in a coordinate (`FZERO` with FreeSurfer's `FLT_EPSILON` as glibc's `float.h`
/// defines it), two `WARNING:` lines on standard error. A destination geometry that is not valid
/// takes MNI's average_305 centre (-0.0950, -16.5100, 9.7500, as `float`) when `USE_AVERAGE305` is
/// set. Then the cached matrices: `SRASToTalSRAS_` ([`formats::get_sras_to_tal_sras`]) and its
/// inverse `TalSRASToSRAS_` (`MatrixInverse`), `free_transform` 1. With `Gdiag`'s `DIAG_SHOW` and
/// `DIAG_VERBOSE`, the `INFO:` lines on standard error. `MRIreadHeader` (track A's) is a placeholder
/// that panics, reached only for an `.xfm` naming an existing volume or a regular `mri/orig`.
pub fn read_transform(
    g: &mut Globals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &mut Mris,
    mris_fname: &str,
) -> Result<i32, Fatal> {
    if mris_fname.len() >= 300 {
        g.notices.add(
            &FS_356,
            format!(
                "mrisReadTransform: a surface name of {} bytes, 300 or more, is taken whole (FreeSurfer writes past its buffer)",
                mris_fname.len()
            ),
        );
    }
    let fpref = file_name_path(mris_fname);
    let transform_fname = format!("{fpref}/../mri/transforms/talairach.xfm");
    if !file_exists(dir, &transform_fname) {
        return Ok(ERROR_NO_FILE);
    }
    let mut lio = FsLtaIo { env: io.env };
    let Some(lta) = lta_read_ex(g, io.out, io.err, dir, &transform_fname, &mut lio)? else {
        mris.lta = None;
        log::error_printf(
            g,
            io.err,
            ERROR_NOFILE,
            &format!("mrisReadTransform: could not read xform file '{transform_fname}'"),
        );
        return Ok(ERROR_NO_FILE);
    };
    if lta.ltype != LINEAR_RAS_TO_RAS {
        mris.lta = Some(lta);
        log::error_exit(
            g,
            io.err,
            ERROR_BADPARM,
            "the transform is not RAS-TO-RAS.  not supported.",
        )?;
    } else {
        mris.lta = Some(lta);
    }
    let show = g.gdiag & DIAG_SHOW != 0 && verbose_on(g);
    let lt = &mut mris.lta.as_mut().expect("the transform just read").xforms[0];
    if lt.src.valid == 0 {
        if mris.vg.valid != 0 {
            if show {
                let _ = io.err.write_all(b"INFO: found the orig volume info on ");
                let _ = io.err.write_all(mris.vg.fname.c_bytes());
                let _ = io.err.write_all(b" in the surface data.\n");
            }
            lt.src.c_r = mris.vg.c_r;
            lt.src.c_a = mris.vg.c_a;
            lt.src.c_s = mris.vg.c_s;
        } else {
            let orig_fname = format!("{fpref}/../mri/orig");
            let regular = std::fs::metadata(resolve(dir, &orig_fname)).is_ok_and(|m| m.is_file());
            let orig = if regular {
                lio.read_header(g, io.out, io.err, dir, &orig_fname)?
            } else {
                None
            };
            if let Some(orig) = orig {
                get_vol_geom(&orig, &mut lt.src);
                get_vol_geom(&orig, &mut mris.vg);
                if show {
                    let _ = io.err.write_all(
                        b"INFO: found the orig volume (mri/orig) to get c_(ras) information for src\n",
                    );
                    let _ = io.err.write_all(b"INFO: added info to the surface.\n");
                }
            } else {
                if show {
                    let _ = io.err.write_all(
                        b"INFO: cannot find mri/orig volume to get c_(ras) information.\n\
                          INFO: transform src volume information cannot be found. assume c_(ras) = 0\n\
                          INFO: destination surface points may be shifted in the volume.\n\
                          INFO: you should put the src info in the transform.\n",
                    );
                }
                lt.src.c_r = 0.0;
                lt.src.c_a = 0.0;
                lt.src.c_s = 0.0;
            }
        }
    } else if mris.vg.valid != 0 {
        let fz = |d: f32| f64::from(d).abs() < f64::from(f32::EPSILON);
        if !fz(lt.src.c_r - mris.vg.c_r)
            || !fz(lt.src.c_a - mris.vg.c_a)
            || !fz(lt.src.c_s - mris.vg.c_s)
        {
            let f = |x: f32| fmt_f(f64::from(x), 0, 6);
            let _ = write!(
                io.err,
                "WARNING: the source volume info is not consistent between the info contained\n\
                 WARNING: in the surface data ({},{},{}) and that of the transform ({},{},{}).\n",
                f(mris.vg.c_r),
                f(mris.vg.c_a),
                f(mris.vg.c_s),
                f(lt.src.c_r),
                f(lt.src.c_a),
                f(lt.src.c_s)
            );
        }
    }
    if lt.dst.valid == 0 {
        if show {
            let _ = io.err.write_all(
                b"INFO: transform dst volume information cannot be found.\n\
                  INFO: if the target is MNI average_305, then you can do 'setenv USE_AVERAGE305 true'\n\
                  INFO: otherwise c_(ras) is set to 0. destination surface points may be shifted.\n",
            );
        }
        if io.env.get("USE_AVERAGE305").is_some() {
            lt.dst.c_r = -0.0950_f64 as f32;
            lt.dst.c_a = -16.5100_f64 as f32;
            lt.dst.c_s = 9.7500_f64 as f32;
        }
    }
    let lt = &mris.lta.as_ref().expect("the transform just read").xforms[0];
    mris.SRASToTalSRAS_ = formats::get_sras_to_tal_sras(g, io, lt)?;
    mris.TalSRASToSRAS_ =
        matrix_inverse(g, io.out, io.err, mris.SRASToTalSRAS_.as_ref(), None)?.into_new();
    mris.free_transform = 1;
    Ok(NO_ERROR)
}

/// The hemisphere a surface name gives (`MRISreadOverAlloc_new`): from the first byte of its last
/// part (after its last `/`), in upper case: `R` right, `L` left, `B` both, else none.
fn hemisphere_from_name(fname: &str) -> i32 {
    let last = match fname.rfind('/') {
        Some(i) => &fname[i + 1..],
        None => fname,
    };
    match last.as_bytes().first().map(u8::to_ascii_uppercase) {
        Some(b'R') => RIGHT_HEMISPHERE,
        Some(b'L') => sfile::LEFT_HEMISPHERE,
        Some(b'B') => BOTH_HEMISPHERES,
        _ => NO_HEMISPHERE,
    }
}

/// The surface a reader gave, before `MRISreadOverAlloc_new` completes it; `version` is
/// FreeSurfer's (-1 quadrangle, -2 new quadrangle, 0 the old format without a magic number, -3
/// and -4 the other readers).
fn read_by_type(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    dir: Option<&Path>,
    fname: &str,
    nvf_multiplier: f64,
    ty: i32,
) -> Result<Option<(Mris, i32, bool)>, Fatal> {
    let r = match ty {
        MRIS_ASCII_TRIANGLE_FILE => {
            surfread::read_ascii_file(g, sg, io, dir, fname)?.map(|m| (m, -3))
        }
        MRIS_ICO_FILE => {
            let m = ico::ico_read_over_alloc(g, sg, io, dir, fname, nvf_multiplier, 1.0)?;
            return Ok(m.map(|m| (m, -2, true)));
        }
        MRIS_GEO_TRIANGLE_FILE => surfread::read_geo_file(g, sg, io, dir, fname)?.map(|m| (m, -4)),
        MRIS_STL_FILE => stl::read_stl(g, sg, io, dir, fname)?.map(|m| (m, -3)),
        MRIS_VTK_FILE => formats::read_vtk(g, sg, io, dir, None, fname, None)?
            .into_new()
            .map(|m| (m, -3)),
        MRIS_GIFTI_FILE => {
            // `mrisReadGIFTIfile(fname, NULL)`: gifticlib's reader; a file it cannot open gives
            // its message and FreeSurfer's, `None`; an existing file needs gifti.cpp's surface
            // part (not yet ported)
            match open(dir, fname, "r") {
                Ok(_) => gifti_pending("MRISread"),
                Err(e) => {
                    g.errno = e;
                    let _ = writeln!(io.err, "** failed to open GIFTI XML file '{fname}'");
                    let _ = writeln!(io.err, "mrisReadGIFTIdanum: ERROR reading gifti {fname}");
                    None
                }
            }
        }
        MRI_MGH_FILE | NII_FILE => {
            log::error_exit(
                g,
                io.err,
                ERROR_BADFILE,
                &format!("ERROR: MRISread: cannot read surface data from file {fname}!\n"),
            )?;
            None
        }
        _ => {
            let mut fp = match open(dir, fname, "rb") {
                Ok(f) => f,
                Err(e) => {
                    g.errno = e;
                    let m = format!("MRISread({fname}): could not open file");
                    log::error_printf(g, io.err, ERROR_NOFILE, &m);
                    return Ok(None);
                }
            };
            let (nread, magic) = fread3(&mut fp);
            if nread != 1 {
                let _ = writeln!(io.out, "ERROR: reading {fname}");
                let _ = writeln!(io.out, "Read {nread} bytes, expected 1");
                fp.close();
                return Ok(None);
            }
            let version = if magic == QUAD_FILE_MAGIC_NUMBER {
                if verbose_show(g) {
                    let _ = io.out.write_all(b"new surface file format\n");
                }
                -1
            } else if magic == NEW_QUAD_FILE_MAGIC_NUMBER {
                -2
            } else if magic == TRIANGLE_FILE_MAGIC_NUMBER {
                fp.close();
                let Some(m) = trifile::read_triangle_file(g, sg, io, dir, fname, nvf_multiplier)?
                else {
                    let code = g.gerror;
                    log::error_printf(g, io.err, code, "mrisReadTriangleFile failed.\n");
                    return Ok(None);
                };
                return Ok(Some((m, -3, false)));
            } else {
                fp.rewind();
                if verbose_show(g) {
                    let _ = io.out.write_all(b"surfer: old surface file format\n");
                }
                0
            };
            fp.close();
            trifile::read_quadrangle_file(g, sg, io, dir, fname, nvf_multiplier)?
                .map(|m| (m, version))
        }
    };
    Ok(r.map(|(m, v)| (m, v, false)))
}

/// A vertex's face list written at `i` by `MRISreadOverAlloc_new`'s rebuild: past its end only
/// after a count that wrapped around (FS-360: the list grows, as if the memory after it were its
/// own; FreeSurfer writes into whatever follows it).
fn put_face(g: &mut Globals, f: &mut Vec<i32>, i: usize, fno: i32, vno: usize) {
    if i >= f.len() {
        g.notices.add(
            &FS_360,
            format!(
                "vertex {vno}: its face list of {} entries is written at {i} (a face count that wrapped around)",
                f.len()
            ),
        );
        f.resize(i + 1, 0);
    }
    f[i] = fno;
}

/// `MRISreadOverAlloc(fname, nVFMultiplier)`: FreeSurfer 8.2.0 always takes the new reader
/// (`MRISreadOverAlloc_new`, utils/mrisurf_io.cpp lines 3542-3791). The reader the name's type
/// chooses (`MRISfileNameType`; the licence check passes): ASCII, icosahedron (returned as its
/// reader made it), movie.byu, STL, VTK, GIFTI (gifti.cpp's surface part: not yet ported); an MGH
/// type is the fatal error `ERROR: MRISread: cannot read surface data from file <file>!`; any other
/// name by its 3-byte magic number: a triangle file (its reader failing: `mrisReadTriangleFile
/// failed.` with its code), a quadrangle file (new or old), or the old format (no magic number).
/// A file that cannot be opened is `MRISread(<file>): could not open file` (`ERROR_NOFILE`), one
/// shorter than 3 bytes `ERROR: reading <file>` and `Read 0 bytes, expected 1` on standard output;
/// both `None`. Then the surface's name and hemisphere ([`hemisphere_from_name`]); for every reader
/// but the old format, each vertex's face list rebuilt from the faces in order; every vertex's
/// curvature 0, original area -1, border 0, its place in each of its faces; the neighbour lists
/// ([`nbhd::complete_topology`]), the bounding box, the neighbour distances, the normals; the
/// Talairach transform ([`read_transform`]); `radius` ([`metric::average_radius`]); the metric
/// properties; the current positions stored; then `<file>.avg.area.mgh`, when readable, needs
/// `MRIread` (not yet ported: reaching it panics), else `group_avg_vtxarea_loaded` 0. FS-351
/// (a quadrangle face vertex beyond the vertices, reproduced: a segmentation fault here), FS-359
/// (the old format: its face lists read past their end; by default rebuilt from the faces as for
/// the other formats), FS-360.
pub fn read_over_alloc(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    dir: Option<&Path>,
    fname: &str,
    nvf_multiplier: f64,
) -> Result<Option<Mris>, Fatal> {
    let fname = c_str(fname);
    let ty = mris_file_name_type(g, fname);
    chklc();
    let Some((mut mris, version, done)) = read_by_type(g, sg, io, dir, fname, nvf_multiplier, ty)?
    else {
        return Ok(None);
    };
    if done {
        return Ok(Some(mris));
    }
    mris.fname = fname.to_owned();
    mris.hemisphere = hemisphere_from_name(fname);
    let nv = nonneg(mris.nvertices);
    let nf = nonneg(mris.nfaces);
    let mut rebuild = version < 0 || ty == MRIS_ASCII_TRIANGLE_FILE;
    if !rebuild && (0..nv).any(|v| mris.topology(v).num != 0) {
        g.notices.add(
            &FS_359,
            format!("{fname}: a surface file of the old format: its face lists are rebuilt from the faces (FreeSurfer reads them past their end)"),
        );
        if g.notices.reproduce() {
            for vno in 0..nv {
                let vt = mris.topology_mut(vno);
                let num = usize::from(vt.num);
                let f = vt.f.get_or_insert_with(Vec::new);
                if f.len() < num {
                    f.resize(num, 0);
                }
                let n = vt.n.get_or_insert_with(Vec::new);
                if n.len() < num {
                    n.resize(num, 0);
                }
            }
            // a face number from the file outside the faces: FreeSurfer reads outside its face
            // arrays from here on (modelled as the segmentation fault it can end with)
            let bad = (0..nv).any(|v| {
                let vt = mris.topology(v);
                let f = vt.f.as_deref().unwrap_or(&[]);
                f[..usize::from(vt.num)]
                    .iter()
                    .any(|&x| x < 0 || x as usize >= nf)
            });
            if bad {
                return Err(Fatal::Exit(139));
            }
        } else {
            rebuild = true;
        }
    }
    if rebuild {
        for vno in 0..nv {
            let vt = mris.topology_mut(vno);
            let num = usize::from(vt.num);
            vt.f = Some(vec![0; num]);
            vt.n = Some(vec![0; num]);
            vt.num = 0;
        }
        let room = mris.vertices_topology.as_ref().map_or(0, Vec::len);
        for fno in 0..nf {
            for n in 0..VERTICES_PER_FACE {
                let v = mris.face(fno).v[n];
                if v < 0 || v as usize >= room || v as usize >= nv {
                    // a face vertex the reader let through (FS-351, FS-352, FS-154 reproduced):
                    // FreeSurfer writes into a face list it never made
                    if (v as usize) < room && v >= 0 {
                        g.notices.add(
                            &FS_351,
                            format!("{fname}: face {fno} has vertex {v}, not a vertex of the surface: MRISread ends with a segmentation fault"),
                        );
                    }
                    return Err(Fatal::Exit(139));
                }
                let v = v as usize;
                let vt = mris.topology_mut(v);
                let i = usize::from(vt.num);
                vt.num = vt.num.wrapping_add(1);
                let mut f = vt.f.take().unwrap_or_default();
                put_face(g, &mut f, i, fno as i32, v);
                mris.topology_mut(v).f = Some(f);
            }
        }
    }
    for vno in 0..nv {
        {
            let v = mris.vertex_mut(vno);
            v.curv = 0.0;
            v.origarea = -1.0;
            v.border = 0;
        }
        let num = usize::from(mris.topology(vno).num);
        for n in 0..num {
            let fno = mris.topology(vno).f.as_ref().map_or(0, |f| f[n]);
            if fno < 0 || fno >= mris.max_faces {
                // FS-359 reproduced: a face number from the file outside the faces
                return Err(Fatal::Exit(139));
            }
            let fv = mris.face(fno as usize).v;
            for (m, &x) in fv.iter().enumerate() {
                if x == vno as i32 {
                    let nl = mris.topology_mut(vno).n.get_or_insert_with(Vec::new);
                    if nl.len() <= n {
                        nl.resize(n + 1, 0);
                    }
                    nl[n] = m as u8;
                }
            }
        }
    }
    nbhd::complete_topology(g, sg, io, &mut mris)?;
    coords::compute_surface_dimensions(&mut mris);
    metric::compute_vertex_distances(g, io, &mut mris)?;
    mp_slow::compute_normals(g, sg, io, &mut mris);
    read_transform(g, io, dir, &mut mris, fname)?;
    mris.radius = metric::average_radius(&mris);
    mp::compute_metric_properties(g, sg, io, &mut mris)?;
    metric::store_current_positions(g, io, &mut mris)?;
    let tmpstr = format!("{fname}.avg.area.mgh");
    if g.gdiag_no >= 0 && verbose_on(g) {
        let _ = writeln!(io.out, "Trying to read average area {tmpstr}");
    }
    if fio_file_exists_readable(g, dir, &tmpstr) != 0 {
        mri_read_pending(&tmpstr);
    }
    mris.group_avg_vtxarea_loaded = 0;
    if g.gdiag_no >= 0 && verbose_on(g) {
        let _ = writeln!(
            io.out,
            "Average area loaded {}",
            mris.group_avg_vtxarea_loaded
        );
    }
    Ok(Some(mris))
}

/// `MRISread(fname, dotkrRasConvert)`: a volume name (`MRIS_VOLUME_FILE`) is the fatal error
/// `MRISread(): input <file> does not appear to be a surface (ext=<extension>, MRIS_VOLUME_FILE)`;
/// the name copied into a 1,024-byte buffer (FS-126); a type that is not ASCII, icosahedron,
/// movie.byu, STL, VTK or GIFTI may become a GIFTI name (`__MRISapplyFSGIIread`); then
/// [`read_over_alloc`] (no extra room). `orig_xyzspace` keeps `useRealRAS`; a surface in scanner
/// coordinates (`useRealRAS` set) with `dotkr_ras_convert` needs a valid volume geometry (else
/// `ERROR: Surface <file> doesn't have valid volume geometry!` on standard output, the surface
/// freed, `None`) and is moved to tkregister space ([`coords::scanner2tkr`]). Then the 3-hop
/// neighbourhoods with their distances ([`nbhd::set_neighborhood_size_and_dist`]) and the
/// neighbourhood in use reset to 1 hop.
pub fn read(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    dir: Option<&Path>,
    fname: &str,
    dotkr_ras_convert: bool,
) -> Result<Option<Mris>, Fatal> {
    let fname = c_str(fname);
    let mut ty = mris_file_name_type(g, fname);
    if ty == MRIS_VOLUME_FILE {
        let ext = fio_extension(Some(fname)).unwrap_or_else(|| "(null)".to_owned());
        log::error_exit(
            g,
            io.err,
            ERROR_BADFILE,
            &format!(
                "MRISread(): input {fname} does not appear to be a surface (ext={ext}, MRIS_VOLUME_FILE)\n"
            ),
        )?;
    }
    let mut surf_to_read = strcpy_checked(g, io, fname, "MRISread")?;
    if !named_type(ty) {
        apply_fsgii_read(g, io, dir, &mut surf_to_read, fname, &mut ty)?;
    }
    let Some(mut mris) = read_over_alloc(g, sg, io, dir, &surf_to_read, 1.0)? else {
        return Ok(None);
    };
    mris.orig_xyzspace = mris.useRealRAS;
    if mris.useRealRAS != 0 && dotkr_ras_convert {
        if mris.vg.valid == 0 {
            let _ = writeln!(
                io.out,
                "ERROR: Surface {surf_to_read} doesn't have valid volume geometry!"
            );
            let mut m = Some(mris);
            alloc::free(g, io.err, &mut m);
            return Ok(None);
        }
        coords::scanner2tkr(g, io, &mut mris)?;
    }
    nbhd::set_neighborhood_size_and_dist(g, sg, io, &mut mris, 3)?;
    nbhd::reset_neighborhood_size(g, io, &mut mris, 1)?;
    Ok(Some(mris))
}

/// `MRISfastRead(fname)`: [`read`] with the conversion to tkregister space.
pub fn fast_read(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    dir: Option<&Path>,
    fname: &str,
) -> Result<Option<Mris>, Fatal> {
    read(g, sg, io, dir, fname, true)
}
