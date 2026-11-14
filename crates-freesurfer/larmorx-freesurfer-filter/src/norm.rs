// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `utils/mrinorm.cpp` (queue item 3.1-43), first group: the Voronoi diagram and the soap bubble
//! that spread the control points' values (`mriBuildVoronoiDiagramShort` 1635,
//! `mriBuildVoronoiDiagramFloat` 1795, `mriBuildVoronoiDiagramUchar` 1943,
//! `MRIbuildVoronoiDiagram` 2097, `MRIsoapBubble` 2119, `mriSoapBubbleFloat` 2367,
//! `mriSoapBubbleShort` 2521, `mriMarkUnmarkedNeighbors` 2708).
//!
//! The functions work on the voxels as arrays (frame by frame, the volumes' own extents) and
//! write them back once; FreeSurfer's direct accesses (`MRIvox`, `MRISvox`, `MRIFvox`,
//! `MRIseq_vox`) of a volume of another type read its bytes as FreeSurfer does (FS-585), and a
//! destination or control volume smaller than the source is refused (FS-586). FS-583 to FS-586.

use std::io::Write;

use larmorx_core::defects::{FS_583, FS_585, FS_586};
use larmorx_freesurfer_core::affine::float_to_int;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_UNSUPPORTED, Fatal};
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::sse::{add, div, div64, sub, sub64};
use larmorx_freesurfer_core::utils::nint;
use larmorx_freesurfer_mri::copy::mri_copy;
use larmorx_freesurfer_mri::mri::{MRI_FLOAT, MRI_INT, MRI_SHORT, MRI_UCHAR, Mri};
use larmorx_freesurfer_mri::voxel::{mri_bounding_box, mri_get_vox_val, mri_set_vox_val};
use larmorx_freesurfer_volume::arith::{Pair, mri_clone, mri_replace_values};

/// `CONTROL_NONE`, `CONTROL_MARKED`, `CONTROL_TMP`, `CONTROL_NBR` (`mrinorm.h`).
pub const CONTROL_NONE: i32 = 0;
pub const CONTROL_MARKED: i32 = 1;
pub const CONTROL_TMP: i32 = 2;
pub const CONTROL_NBR: i32 = 3;

/// A volume's extent.
#[derive(Clone, Copy)]
struct Ext {
    w: usize,
    h: usize,
    d: usize,
}

impl Ext {
    fn of(m: &Mri) -> Ext {
        Ext {
            w: m.vg.width.max(0) as usize,
            h: m.vg.height.max(0) as usize,
            d: m.vg.depth.max(0) as usize,
        }
    }
    fn n(self) -> usize {
        self.w * self.h * self.d
    }
    fn at(self, x: i32, y: i32, z: i32) -> usize {
        (z as usize * self.h + y as usize) * self.w + x as usize
    }
}

/// The byte `x` of row `y` of slice `sl` (`MRIvox` / `MRIseq_vox` of any volume: its bytes).
fn byte_at(m: &Mri, sl: usize, y: usize, x: usize) -> u8 {
    let row = m.vox_per_row as usize * m.bytes_per_vox;
    m.chunk.as_ref().expect("voxels")[(sl * m.vg.height as usize + y) * row + x]
}

fn set_byte(m: &mut Mri, sl: usize, y: usize, x: usize, b: u8) {
    let row = m.vox_per_row as usize * m.bytes_per_vox;
    let h = m.vg.height as usize;
    m.chunk.as_mut().expect("voxels")[(sl * h + y) * row + x] = b;
}

/// The element `x` of row `y` of slice `sl`, of the volume's own type, as raw bits.
fn elem(m: &Mri, sl: usize, y: usize, x: usize) -> &[u8] {
    let b = m.bytes_per_vox;
    let row = m.vox_per_row as usize * b;
    let o = (sl * m.vg.height as usize + y) * row + x * b;
    &m.chunk.as_ref().expect("voxels")[o..o + b]
}

fn set_elem(m: &mut Mri, sl: usize, y: usize, x: usize, v: &[u8]) {
    let b = m.bytes_per_vox;
    let row = m.vox_per_row as usize * b;
    let h = m.vg.height as usize;
    let o = (sl * h + y) * row + x * b;
    m.chunk.as_mut().expect("voxels")[o..o + b].copy_from_slice(v);
}

/// The frame `f` of a float volume (bits kept).
fn floats(m: &Mri, f: usize) -> Vec<f32> {
    let e = Ext::of(m);
    let mut v = Vec::with_capacity(e.n());
    for z in 0..e.d {
        for y in 0..e.h {
            for x in 0..e.w {
                let b = elem(m, f * e.d + z, y, x);
                v.push(f32::from_le_bytes([b[0], b[1], b[2], b[3]]));
            }
        }
    }
    v
}

fn put_floats(m: &mut Mri, f: usize, v: &[f32]) {
    let e = Ext::of(m);
    let mut i = 0;
    for z in 0..e.d {
        for y in 0..e.h {
            for x in 0..e.w {
                set_elem(m, f * e.d + z, y, x, &v[i].to_le_bytes());
                i += 1;
            }
        }
    }
}

fn shorts(m: &Mri, f: usize) -> Vec<i16> {
    let e = Ext::of(m);
    let mut v = Vec::with_capacity(e.n());
    for z in 0..e.d {
        for y in 0..e.h {
            for x in 0..e.w {
                let b = elem(m, f * e.d + z, y, x);
                v.push(i16::from_le_bytes([b[0], b[1]]));
            }
        }
    }
    v
}

fn put_shorts(m: &mut Mri, f: usize, v: &[i16]) {
    let e = Ext::of(m);
    let mut i = 0;
    for z in 0..e.d {
        for y in 0..e.h {
            for x in 0..e.w {
                set_elem(m, f * e.d + z, y, x, &v[i].to_le_bytes());
                i += 1;
            }
        }
    }
}

/// The index tables of a volume: `t[i + 2]` for `i` in `[-2, len + 2)` (enough for a 5 x 5 x 5
/// window).
struct Tables {
    x: Vec<i32>,
    y: Vec<i32>,
    z: Vec<i32>,
}

impl Tables {
    fn of(m: &Mri) -> Tables {
        let f = |t: Option<&Vec<i32>>, len: i32| -> Vec<i32> {
            (-2..len + 2)
                .map(|i| t.map_or(0, |t| Mri::index(t, i)))
                .collect()
        };
        Tables {
            x: f(m.xi.as_ref(), m.vg.width),
            y: f(m.yi.as_ref(), m.vg.height),
            z: f(m.zi.as_ref(), m.vg.depth),
        }
    }
    fn xi(&self, i: i32) -> i32 {
        self.x[(i + 2) as usize]
    }
    fn yi(&self, i: i32) -> i32 {
        self.y[(i + 2) as usize]
    }
    fn zi(&self, i: i32) -> i32 {
        self.z[(i + 2) as usize]
    }
}

/// FS-586: a volume smaller than the source (or with fewer frames) that FreeSurfer reads or
/// writes directly at the source's voxels (memory outside it): refused.
fn check_fits(
    g: &mut Globals,
    err: &mut dyn Write,
    what: &str,
    name: &str,
    se: Ext,
    m: &Mri,
    frames: i32,
) -> Result<(), Fatal> {
    if (m.vg.width.max(0) as usize) < se.w
        || (m.vg.height.max(0) as usize) < se.h
        || (m.vg.depth.max(0) as usize) < se.d
        || m.nframes < frames
        || m.chunk.is_none()
    {
        g.notices.add(
            &FS_586,
            format!("{what}: the {name} volume is smaller than the source"),
        );
        log::error_exit(
            g,
            err,
            ERROR_UNSUPPORTED,
            &format!(
                "{what}: the {name} volume is smaller than the source (FreeSurfer reads or writes \
                 outside it)"
            ),
        )?;
    }
    Ok(())
}

/// `(BUFTYPE) x` of a `float` (`cvttss2si`'s 32 bits, their low byte; FS-17 beyond `int`).
fn to_byte(g: &mut Globals, x: f32, what: &str) -> u8 {
    float_to_int(g, f64::from(x), what) as u8
}

/// `mriMarkUnmarkedNeighbors(mri, mri, mri, mark, nbr_mark)` on the frame-0 values of a byte or
/// `int` volume: every voxel of a 3 x 3 x 3 neighbourhood (positions clamped) of a voxel equal to
/// `mark` that is `CONTROL_NONE` becomes `nbr_mark`.
fn mark_unmarked(v: &mut [i32], e: Ext, t: &Tables, mark: i32, nbr_mark: i32) {
    for z in 0..e.d as i32 {
        for y in 0..e.h as i32 {
            for x in 0..e.w as i32 {
                if v[e.at(x, y, z)] != mark {
                    continue;
                }
                for zk in -1..=1 {
                    let zi = t.zi(z + zk);
                    for yk in -1..=1 {
                        let yi = t.yi(y + yk);
                        for xk in -1..=1 {
                            let xi = t.xi(x + xk);
                            let j = e.at(xi, yi, zi);
                            if v[j] == CONTROL_NONE {
                                v[j] = nbr_mark;
                            }
                        }
                    }
                }
            }
        }
    }
}

fn replace(v: &mut [i32], a: i32, b: i32) {
    for x in v.iter_mut() {
        if *x == a {
            *x = b;
        }
    }
}

/// What the soap bubble reads of its source: the extent, index tables and frames.
struct Meta {
    e: Ext,
    t: Tables,
    nframes: i32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Ver {
    Float,
    Short,
    Uchar,
}

/// `MRIbuildVoronoiDiagram(mri_src, mri_ctrl, mri_dst)`: a float, short or byte source (another
/// type: `NULL` after `MRIbuildVoronoiDiagram: src type <t> unsupported`); the destination (a
/// clone of the source when none is given; FS-583 for a short source) must be of the source's
/// type (`mri<...>: incorrect input type(s)`, the program ends). The destination takes the
/// source's value at the control voxels (the control volume's value as a byte, or as an `int` for
/// a byte source, not 0; above 255 printed `ctrl=<n>` for a byte source) and 0 elsewhere; then,
/// layer by layer, every voxel next to the last layer (26 neighbours, positions clamped) takes
/// the mean of its neighbours in that layer (summed in `float` in slice, row, column order;
/// rounded with `nint` for short, truncated for byte), until a layer adds nothing or every voxel
/// has a value. The control volume must be byte or `int` (`mriMarkUnmarkedNeighbors: all inputs
/// must be MRI_UCHAR or MRI_INT`, the program ends); its `CONTROL_TMP` voxels become
/// `CONTROL_NONE`.
pub fn mri_build_voronoi_diagram(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    ctrl: &mut Mri,
    dst: Option<&mut Mri>,
) -> Result<Ret<Mri>, Fatal> {
    let ver = match src.mtype {
        MRI_FLOAT => Ver::Float,
        MRI_SHORT => Ver::Short,
        MRI_UCHAR => Ver::Uchar,
        t => {
            log::error_printf(
                g,
                err,
                ERROR_UNSUPPORTED,
                &format!("MRIbuildVoronoiDiagram: src type {t} unsupported"),
            );
            return Ok(Ret::Null);
        }
    };
    let (what, ty) = match ver {
        Ver::Float => ("mriBuildVoronoiDiagramFloat", MRI_FLOAT),
        Ver::Short => ("mriBuildVoronoiDiagramShort", MRI_SHORT),
        Ver::Uchar => ("mriBuildVoronoiDiagramUchar", MRI_UCHAR),
    };
    let bad_types = |d: Option<&Mri>| src.mtype != ty || d.is_some_and(|d| d.mtype != ty);
    let mut fresh = None;
    let given = dst.is_some();
    if ver == Ver::Short && !given {
        g.notices.add(
            &FS_583,
            "MRIbuildVoronoiDiagram of a short volume without a destination",
        );
        if g.notices.reproduce() {
            log::error_exit(
                g,
                err,
                ERROR_UNSUPPORTED,
                "mriBuildVoronoiDiagramShort: no destination (FreeSurfer crashes here)",
            )?;
        }
    }
    if ver == Ver::Uchar && bad_types(dst.as_deref()) {
        log::error_exit(
            g,
            err,
            ERROR_UNSUPPORTED,
            &format!("{what}: incorrect input type(s)"),
        )?;
    }
    if ver == Ver::Short && given && bad_types(dst.as_deref()) {
        log::error_exit(
            g,
            err,
            ERROR_UNSUPPORTED,
            &format!("{what}: incorrect input type(s)"),
        )?;
    }
    let d: &mut Mri = match dst {
        Some(d) => d,
        None => fresh.insert(mri_clone(g, out, err, src, None)?.unwrap_new()),
    };
    if bad_types(Some(d)) {
        log::error_exit(
            g,
            err,
            ERROR_UNSUPPORTED,
            &format!("{what}: incorrect input type(s)"),
        )?;
    }
    check_fits(g, err, what, "destination", Ext::of(src), d, 1)?;
    check_fits(g, err, what, "control", Ext::of(src), ctrl, 1)?;
    let e = Ext::of(src);
    let ed = Ext::of(d);
    let t = Tables::of(src);
    let srcv: Vec<f32> = match ver {
        Ver::Float => floats(src, 0),
        Ver::Short => shorts(src, 0).into_iter().map(f32::from).collect(),
        Ver::Uchar => {
            let mut v = Vec::with_capacity(e.n());
            for z in 0..e.d {
                for y in 0..e.h {
                    for x in 0..e.w {
                        v.push(f32::from(byte_at(src, z, y, x)));
                    }
                }
            }
            v
        }
    };
    // the destination's frame 0 (its own extent); values exact in f32 for short and byte
    let mut dv: Vec<f32> = match ver {
        Ver::Float => floats(d, 0),
        Ver::Short => shorts(d, 0).into_iter().map(f32::from).collect(),
        Ver::Uchar => {
            let mut v = Vec::with_capacity(ed.n());
            for z in 0..ed.d {
                for y in 0..ed.h {
                    for x in 0..ed.w {
                        v.push(f32::from(byte_at(d, z, y, x)));
                    }
                }
            }
            v
        }
    };
    let mut total = 0i32;
    for z in 0..e.d as i32 {
        for y in 0..e.h as i32 {
            for x in 0..e.w as i32 {
                let c = mri_get_vox_val(g, err, ctrl, x, y, z, 0, 0)?;
                let on = if ver == Ver::Uchar {
                    let c = float_to_int(g, f64::from(c), what);
                    if c > 255 {
                        let _ = writeln!(out, "ctrl={c}");
                    }
                    c != 0
                } else {
                    to_byte(g, c, what) != 0
                };
                let val = if on {
                    total += 1;
                    srcv[e.at(x, y, z)]
                } else {
                    0.0
                };
                dv[ed.at(x, y, z)] = val;
            }
        }
    }
    total = (e.n() as i32).wrapping_sub(total);
    // mri_marked: a copy of the control volume (its frame 0 is all that is read)
    let ec = Ext::of(ctrl);
    let tc = Tables::of(ctrl);
    let mut marked: Vec<i32> = Vec::with_capacity(ec.n());
    for z in 0..ec.d {
        for y in 0..ec.h {
            for x in 0..ec.w {
                let b = elem(ctrl, z, y, x);
                marked.push(match ctrl.mtype {
                    MRI_UCHAR => i32::from(b[0]),
                    MRI_INT => i32::from_le_bytes([b[0], b[1], b[2], b[3]]),
                    // the program ends before these are read
                    _ => 0,
                });
            }
        }
    }
    replace(&mut marked, CONTROL_MARKED, CONTROL_NBR);
    if ctrl.mtype != MRI_UCHAR && ctrl.mtype != MRI_INT {
        log::error_exit(
            g,
            err,
            ERROR_UNSUPPORTED,
            "mriMarkUnmarkedNeighbors: all inputs must be MRI_UCHAR or MRI_INT",
        )?;
    }
    // the short version reads the marks as bytes (MRIvox): an int control volume's bytes (FS-585)
    let bytewise = ver == Ver::Short && ctrl.mtype == MRI_INT;
    if bytewise {
        g.notices.add(
            &FS_585,
            format!("{what}: an int control volume's marks read byte by byte"),
        );
    }
    let raw = bytewise && g.notices.reproduce();
    let mark_of = |marked: &[i32], x: i32, y: i32, z: i32| -> i32 {
        if raw {
            let row = (z as usize * ec.h + y as usize) * ec.w;
            let b = marked[row + x as usize / 4].to_le_bytes()[x as usize % 4];
            i32::from(b)
        } else {
            marked[ec.at(x, y, z)]
        }
    };
    loop {
        let mut nchanged = 0i32;
        mark_unmarked(&mut marked, ec, &tc, CONTROL_NBR, CONTROL_TMP);
        replace(&mut marked, CONTROL_NBR, CONTROL_MARKED);
        replace(&mut marked, CONTROL_TMP, CONTROL_NBR);
        for z in 0..e.d as i32 {
            for y in 0..e.h as i32 {
                for x in 0..e.w as i32 {
                    let mark = match ver {
                        Ver::Float => i32::from(to_byte(g, marked[ec.at(x, y, z)] as f32, what)),
                        Ver::Short => mark_of(&marked, x, y, z),
                        Ver::Uchar => marked[ec.at(x, y, z)],
                    };
                    if mark != CONTROL_NBR {
                        continue;
                    }
                    let mut mean = 0.0f32;
                    let mut n = 0i32;
                    for zk in -1..=1 {
                        let zi = t.zi(z + zk);
                        for yk in -1..=1 {
                            let yi = t.yi(y + yk);
                            for xk in -1..=1 {
                                let xi = t.xi(x + xk);
                                let m = match ver {
                                    Ver::Float => marked[ec.at(xi, yi, zi)] as f32 == 1.0,
                                    Ver::Short => mark_of(&marked, xi, yi, zi) == CONTROL_MARKED,
                                    Ver::Uchar => marked[ec.at(xi, yi, zi)] == CONTROL_MARKED,
                                };
                                if m {
                                    n += 1;
                                    mean = add(mean, dv[ed.at(xi, yi, zi)]);
                                }
                            }
                        }
                    }
                    if n > 0 {
                        let q = div(mean, n as f32);
                        dv[ed.at(x, y, z)] = match ver {
                            Ver::Float => q,
                            Ver::Short => f32::from(nint(g, f64::from(q)) as i16),
                            Ver::Uchar => f32::from(to_byte(g, q, what)),
                        };
                        nchanged += 1;
                    }
                }
            }
        }
        total = total.wrapping_sub(nchanged);
        if !(nchanged > 0 && total > 0) {
            break;
        }
    }
    match ver {
        Ver::Float => put_floats(d, 0, &dv),
        Ver::Short => {
            let v: Vec<i16> = dv.iter().map(|&x| x as i16).collect();
            put_shorts(d, 0, &v);
        }
        Ver::Uchar => {
            let mut i = 0;
            for z in 0..ed.d {
                for y in 0..ed.h {
                    for x in 0..ed.w {
                        set_byte(d, z, y, x, dv[i] as u8);
                        i += 1;
                    }
                }
            }
        }
    }
    mri_replace_values(
        g,
        out,
        err,
        Pair::Same(ctrl),
        CONTROL_TMP as f32,
        CONTROL_NONE as f32,
    )?;
    Ok(match fresh {
        Some(m) if !given => Ret::New(m),
        _ => Ret::Given,
    })
}

/// `MRIsoapBubble(mri_src, mri_ctrl, mri_dst, niter, min_change)` (the destination may be the
/// source: `Pair::Same`, as `mri_normalize` calls it): `MRIcopy` for no iteration;
/// a float or short source by its own version; any other type: the destination (a copy of the
/// source when none is given) relaxed `niter` times per frame, each voxel that is not a control
/// point (the control value as a byte is not `CONTROL_MARKED`) taking `nint` of the 3 x 3 x 3
/// mean of the destination's bytes (FreeSurfer reads and writes the volumes as bytes: FS-585 for
/// wider types), all voxels from the previous pass.
#[allow(clippy::too_many_arguments)]
pub fn mri_soap_bubble(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: Pair,
    ctrl: &Mri,
    niter: i32,
    min_change: f32,
) -> Result<Ret<Mri>, Fatal> {
    let (src_type, meta) = {
        let s: &Mri = match &p {
            Pair::New(s) | Pair::Given(s, _) => s,
            Pair::Same(d) => d,
        };
        (
            s.mtype,
            Meta {
                e: Ext::of(s),
                t: Tables::of(s),
                nframes: s.nframes,
            },
        )
    };
    if niter == 0 {
        return match p {
            Pair::New(s) => mri_copy(g, out, err, s, None),
            Pair::Given(s, d) => mri_copy(g, out, err, s, Some(d)),
            Pair::Same(_) => Ok(Ret::Given),
        };
    }
    let what = match src_type {
        MRI_FLOAT => "mriSoapBubbleFloat",
        MRI_SHORT => "mriSoapBubbleShort",
        _ => "MRIsoapBubble",
    };
    if src_type == MRI_FLOAT && ctrl.mtype != MRI_UCHAR {
        log::error_exit(
            g,
            err,
            ERROR_UNSUPPORTED,
            "mriSoapBubbleFloat: ctrl must be UCHAR",
        )?;
    }
    let mut fresh = None;
    let given = !matches!(p, Pair::New(_));
    let d: &mut Mri = match p {
        Pair::New(s) => fresh.insert(mri_copy(g, out, err, s, None)?.unwrap_new()),
        Pair::Given(_, d) | Pair::Same(d) => d,
    };
    check_fits(g, err, what, "destination", meta.e, d, meta.nframes)?;
    check_fits(g, err, what, "control", meta.e, ctrl, 1)?;
    if (src_type == MRI_FLOAT || src_type == MRI_SHORT) && d.mtype != src_type {
        g.notices
            .add(&FS_586, format!("{what}: a destination of another type"));
        log::error_exit(
            g,
            err,
            ERROR_UNSUPPORTED,
            &format!(
                "{what}: a destination of another type (FreeSurfer reads and writes outside it)"
            ),
        )?;
    }
    let mut tmp = mri_copy(g, out, err, d, None)?.unwrap_new();
    match src_type {
        MRI_FLOAT => soap_float(g, err, &meta, ctrl, d, &mut tmp, niter, min_change)?,
        MRI_SHORT => soap_short(g, err, &meta, ctrl, d, &mut tmp, niter)?,
        _ => soap_bytes(g, err, &meta, ctrl, d, &mut tmp, niter)?,
    }
    Ok(match fresh {
        Some(m) if !given => Ret::New(m),
        _ => Ret::Given,
    })
}

/// `mriSoapBubbleFloat`: per frame, first each voxel within 2 of the control box (the box of the
/// control values above 0, `MRIboundingBox`) that is not a control point takes the mean of the
/// control points of its 5 x 5 x 5 neighbourhood (`float`, in place), then `niter` passes over the
/// box (grown by one voxel after each pass) set each voxel that is not a control point to the
/// 3 x 3 x 3 mean of the previous pass (all voxels; `float` sum / 27), stopping after a pass whose
/// largest change (from the first phase too, as a `double` compared with `min_change`) is below
/// `min_change`. The box and the largest change carry over from one frame to the next.
#[allow(clippy::too_many_arguments)]
fn soap_float(
    g: &mut Globals,
    err: &mut dyn Write,
    m: &Meta,
    ctrl: &Mri,
    d: &mut Mri,
    tmp: &mut Mri,
    niter: i32,
    min_change: f32,
) -> Result<(), Fatal> {
    let e = m.e;
    let ed = Ext::of(d);
    let ec = Ext::of(ctrl);
    let t = &m.t;
    let (_, bx) = mri_bounding_box(g, err, ctrl, CONTROL_MARKED - 1)?;
    let (mut x1, mut y1, mut z1) = (bx.x, bx.y, bx.z);
    let (mut x2, mut y2, mut z2) = (bx.x + bx.dx - 1, bx.y + bx.dy - 1, bx.z + bx.dz - 1);
    let (w, h, dep) = (e.w as i32, e.h as i32, e.d as i32);
    let cb = |x: i32, y: i32, z: i32| byte_at(ctrl, z as usize, y as usize, x as usize);
    let _ = ec;
    let mut max_change = 0.0f32;
    let mut dvs: Vec<Vec<f32>> = (0..d.nframes as usize).map(|f| floats(d, f)).collect();
    let mut tvs: Vec<Vec<f32>> = (0..tmp.nframes as usize).map(|f| floats(tmp, f)).collect();
    for f in 0..m.nframes.max(0) as usize {
        for z in 0.max(z1 - 2)..=(z2 + 2).min(dep - 1) {
            for y in 0.max(y1 - 2)..=(y2 + 2).min(h - 1) {
                for x in 0.max(x1 - 2)..=(x2 + 2).min(w - 1) {
                    if i32::from(cb(x, y, z)) == CONTROL_MARKED {
                        continue;
                    }
                    let mut num = 0i32;
                    let mut mean = 0.0f32;
                    for zk in -2..=2 {
                        let zi = t.zi(z + zk);
                        for yk in -2..=2 {
                            let yi = t.yi(y + yk);
                            for xk in -2..=2 {
                                let xi = t.xi(x + xk);
                                if i32::from(cb(xi, yi, zi)) != CONTROL_MARKED {
                                    continue;
                                }
                                mean = add(mean, dvs[f][ed.at(xi, yi, zi)]);
                                num += 1;
                            }
                        }
                    }
                    if num > 0 {
                        let i = ed.at(x, y, z);
                        let q = div(mean, num as f32);
                        let c = sub(q, dvs[f][i]).abs();
                        if c > max_change {
                            max_change = c;
                        }
                        dvs[f][i] = q;
                    }
                }
            }
        }
        for _ in 0..niter {
            for z in z1..=z2 {
                for y in y1..=y2 {
                    for x in x1..=x2 {
                        if i32::from(cb(x, y, z)) == CONTROL_MARKED {
                            continue;
                        }
                        let mut mean = 0.0f32;
                        for zk in -1..=1 {
                            let zi = t.zi(z + zk);
                            for yk in -1..=1 {
                                let yi = t.yi(y + yk);
                                for xk in -1..=1 {
                                    let xi = t.xi(x + xk);
                                    mean = add(mean, dvs[f][ed.at(xi, yi, zi)]);
                                }
                            }
                        }
                        let i = ed.at(x, y, z);
                        let val = tvs[f][i];
                        let c = sub64(div64(f64::from(mean), 27.0), f64::from(val)).abs();
                        if c > f64::from(max_change) {
                            max_change = c as f32;
                        }
                        tvs[f][i] = div(mean, 27.0);
                    }
                }
            }
            dvs.clone_from(&tvs);
            x1 = (x1 - 1).max(0);
            y1 = (y1 - 1).max(0);
            z1 = (z1 - 1).max(0);
            x2 = (x2 + 1).min(w - 1);
            y2 = (y2 + 1).min(h - 1);
            z2 = (z2 + 1).min(dep - 1);
            if max_change < min_change {
                break;
            }
            max_change = 0.0;
        }
    }
    for (f, v) in dvs.iter().enumerate() {
        put_floats(d, f, v);
    }
    for (f, v) in tvs.iter().enumerate() {
        put_floats(tmp, f, v);
    }
    Ok(())
}

/// The control test of the short and byte soap bubbles: the control volume's byte (`MRIvox`, its
/// own bytes for a wider type: FS-585, read by value by default) is `CONTROL_MARKED`.
fn ctrl_marks(
    g: &mut Globals,
    err: &mut dyn Write,
    what: &str,
    m: &Meta,
    ctrl: &Mri,
    direct: bool,
) -> Result<Vec<bool>, Fatal> {
    let e = m.e;
    let wide = direct && ctrl.mtype != MRI_UCHAR;
    if wide {
        g.notices.add(
            &FS_585,
            format!("{what}: the control volume's marks read byte by byte"),
        );
    }
    let mut v = Vec::with_capacity(e.n());
    for z in 0..e.d as i32 {
        for y in 0..e.h as i32 {
            for x in 0..e.w as i32 {
                let m = if direct && (!wide || g.notices.reproduce()) {
                    byte_at(ctrl, z as usize, y as usize, x as usize)
                } else {
                    let c = mri_get_vox_val(g, err, ctrl, x, y, z, 0, 0)?;
                    to_byte(g, c, what)
                };
                v.push(i32::from(m) == CONTROL_MARKED);
            }
        }
    }
    Ok(v)
}

/// `mriSoapBubbleShort`: per frame, each voxel that is not a control point (the control volume's
/// byte) first takes `nint` of the mean of the control points of its 3 x 3 x 3 neighbourhood (in
/// place), then `niter` passes set each such voxel to `nint` of the 3 x 3 x 3 mean of the
/// previous pass (`int` sums, `float` quotients).
fn soap_short(
    g: &mut Globals,
    err: &mut dyn Write,
    m: &Meta,
    ctrl: &Mri,
    d: &mut Mri,
    tmp: &mut Mri,
    niter: i32,
) -> Result<(), Fatal> {
    let e = m.e;
    let ed = Ext::of(d);
    let t = &m.t;
    let marks = ctrl_marks(g, err, "mriSoapBubbleShort", m, ctrl, true)?;
    let mut dvs: Vec<Vec<i16>> = (0..d.nframes as usize).map(|f| shorts(d, f)).collect();
    let mut tvs: Vec<Vec<i16>> = (0..tmp.nframes as usize).map(|f| shorts(tmp, f)).collect();
    for f in 0..m.nframes.max(0) as usize {
        for z in 0..e.d as i32 {
            for y in 0..e.h as i32 {
                for x in 0..e.w as i32 {
                    if marks[e.at(x, y, z)] {
                        continue;
                    }
                    let (mut num, mut mean) = (0i32, 0i32);
                    for zk in -1..=1 {
                        let zi = t.zi(z + zk);
                        for yk in -1..=1 {
                            let yi = t.yi(y + yk);
                            for xk in -1..=1 {
                                let xi = t.xi(x + xk);
                                if !marks[e.at(xi, yi, zi)] {
                                    continue;
                                }
                                mean = mean.wrapping_add(i32::from(dvs[f][ed.at(xi, yi, zi)]));
                                num += 1;
                            }
                        }
                    }
                    if num > 0 {
                        let q = div(mean as f32, num as f32);
                        dvs[f][ed.at(x, y, z)] = nint(g, f64::from(q)) as i16;
                    }
                }
            }
        }
        for _ in 0..niter {
            for z in 0..e.d as i32 {
                for y in 0..e.h as i32 {
                    for x in 0..e.w as i32 {
                        if marks[e.at(x, y, z)] {
                            continue;
                        }
                        let mut mean = 0i32;
                        for zk in -1..=1 {
                            let zi = t.zi(z + zk);
                            for yk in -1..=1 {
                                let yi = t.yi(y + yk);
                                for xk in -1..=1 {
                                    let xi = t.xi(x + xk);
                                    mean = mean.wrapping_add(i32::from(dvs[f][ed.at(xi, yi, zi)]));
                                }
                            }
                        }
                        let q = div(mean as f32, 27.0);
                        tvs[f][ed.at(x, y, z)] = nint(g, f64::from(q)) as i16;
                    }
                }
            }
            dvs.clone_from(&tvs);
        }
    }
    for (f, v) in dvs.iter().enumerate() {
        put_shorts(d, f, v);
    }
    for (f, v) in tvs.iter().enumerate() {
        put_shorts(tmp, f, v);
    }
    Ok(())
}

/// `MRIsoapBubble`'s own passes (sources neither float nor short): the destination's bytes
/// (`MRIseq_vox`) averaged and written as bytes; by default a wider destination by its values
/// (FS-585).
fn soap_bytes(
    g: &mut Globals,
    err: &mut dyn Write,
    m: &Meta,
    ctrl: &Mri,
    d: &mut Mri,
    tmp: &mut Mri,
    niter: i32,
) -> Result<(), Fatal> {
    let what = "MRIsoapBubble";
    let e = m.e;
    let t = &m.t;
    let marks = ctrl_marks(g, err, what, m, ctrl, false)?;
    let wide = d.bytes_per_vox != 1;
    if wide {
        g.notices.add(
            &FS_585,
            format!("{what}: a volume of type {} averaged byte by byte", d.mtype),
        );
    }
    let by_value = wide && !g.notices.reproduce();
    for f in 0..m.nframes.max(0) {
        let fd = f as usize * d.vg.depth as usize;
        for _ in 0..niter {
            for z in 0..e.d as i32 {
                for y in 0..e.h as i32 {
                    for x in 0..e.w as i32 {
                        if marks[e.at(x, y, z)] {
                            continue;
                        }
                        let mut mean = 0i64;
                        for zk in -1..=1 {
                            let zi = t.zi(z + zk);
                            for yk in -1..=1 {
                                let yi = t.yi(y + yk);
                                for xk in -1..=1 {
                                    let xi = t.xi(x + xk);
                                    mean += if by_value {
                                        let v = mri_get_vox_val(g, err, d, xi, yi, zi, f, 0)?;
                                        i64::from(float_to_int(g, f64::from(v), what))
                                    } else {
                                        i64::from(byte_at(
                                            d,
                                            fd + zi as usize,
                                            yi as usize,
                                            xi as usize,
                                        ))
                                    };
                                }
                            }
                        }
                        let q = div(mean as f32, 27.0);
                        let v = nint(g, f64::from(q));
                        if by_value {
                            mri_set_vox_val(g, err, tmp, x, y, z, f, v as f32, 0)?;
                        } else {
                            set_byte(tmp, fd + z as usize, y as usize, x as usize, v as u8);
                        }
                    }
                }
            }
            let c = tmp.chunk.clone();
            d.chunk.clone_from(&c);
        }
    }
    Ok(())
}
