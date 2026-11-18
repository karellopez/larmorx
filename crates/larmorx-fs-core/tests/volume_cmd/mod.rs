// SPDX-License-Identifier: Apache-2.0
//! The black-box probe's commands over fs.volume's clean room (`specs/freesurfer-volume.md` §69;
//! the commands and encodings: `validation/investigations/freesurfer-volume-probe/README.md`): one
//! command line in, the probe's result line out (`<result> | x<standard output> x<standard
//! error>`), each line on a process state of its own (the probe's child process), the notices of
//! the defects a line met added to its standard error (`upstream defect ...`).
//!
//! Shared by the golden cases (`tests/golden_volume.rs`) and the comparison's candidate program
//! (`validation/investigations/freesurfer-cleanroom-volume/candidate`, which includes this folder
//! by path), so both run the same code. The volume, geometry, matrix, colour-table and MINC
//! transform encodings are the fs.mri probe's (`tests/mri_cmd/args.rs`).

#[allow(dead_code)]
#[path = "../mri_cmd/args.rs"]
pub mod args;

use std::io::Write;
use std::sync::{Arc, Mutex};

use larmorx_core::defects::UpstreamDefects;
use larmorx_fs_core::maths::matrix::{MATRIX_REAL, Matrix};
use larmorx_fs_core::maths::mfiles::CStream;
use larmorx_fs_core::mri::{LinearTransform, Lta, Mri, Region, VolGeom, types};
use larmorx_fs_core::volume::cma;
use larmorx_fs_core::volume::deriv::Neighbours;
use larmorx_fs_core::volume::geometry::SurfacePoint;
use larmorx_fs_core::volume::ltaread::{FileSystem, TransformInputs};
use larmorx_fs_core::volume::moves::axis;
use larmorx_fs_core::volume::vgcopy::VG_IS_EQUAL_THRESHOLD;
use larmorx_fs_core::volume::{Dst, VolOut};
use larmorx_fs_core::{Environment, Process, Stop};

pub use args::{Line, hexbytes};

/// A stream that keeps what is written.
#[derive(Clone, Default)]
struct Buf(Arc<Mutex<Vec<u8>>>);

impl Write for Buf {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.0.lock().expect("buffer").extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// The float table of the `t<seed>` fill (README.md, "Encodings").
const TABLE: [u32; 34] = [
    0x00000000, 0x80000000, 0x3f000000, 0xbf000000, 0x3f800000, 0xbf800000, 0x40200000, 0x437f8000,
    0x437e8000, 0x43800000, 0xbfc00000, 0x501502f9, 0xd01502f9, 0x7f7fffff, 0xff7fffff, 0x00000001,
    0x7f800000, 0xff800000, 0x7fc00000, 0xffc00000, 0x7fc12345, 0x7f800001, 0x40500000, 0x42c80000,
    0x1e3ce508, 0x477fff80, 0x46ffff00, 0xc7000080, 0x4f000000, 0x4f800000, 0x41200000, 0xc1200000,
    0x3dcccccd, 0x40490fdb,
];

/// A plain geometry argument (G) into a new geometry.
fn getg(l: &mut Line) -> VolGeom {
    let mut g = VolGeom::default();
    l.getg(&mut g, true);
    g
}

/// W, a working volume (README.md, "Encodings").
fn getw(p: &mut Process, l: &mut Line) -> Result<Mri, Stop> {
    let mut m = l.getv(p)?;
    let fill = l.tok().to_owned();
    if let Some(seed) = fill.strip_prefix('t') {
        let seed: u64 = seed
            .chars()
            .take_while(char::is_ascii_digit)
            .collect::<String>()
            .parse()
            .unwrap_or(0);
        if m.kind == types::FLOAT
            && let Some(v) = m.voxels.as_mut()
        {
            for i in 0..v.len() / 4 {
                let x = TABLE[((7 * i as u64 + seed) % 34) as usize];
                v[4 * i..4 * i + 4].copy_from_slice(&x.to_le_bytes());
            }
        } else {
            Line::fill(&mut m, seed as i32);
        }
    } else {
        Line::fill(&mut m, args::strtol(&fill) as i32);
    }
    let g = &mut m.geom;
    g.xsize = l.getf();
    g.ysize = l.getf();
    g.zsize = l.getf();
    g.ras_good_flag = l.geti();
    for v in [&mut g.x, &mut g.y, &mut g.z, &mut g.c, &mut g.s] {
        for x in v.iter_mut() {
            *x = l.getf();
        }
    }
    m.imnr0 = l.geti();
    m.imnr1 = l.geti();
    for x in [
        &mut m.xstart,
        &mut m.xend,
        &mut m.ystart,
        &mut m.yend,
        &mut m.zstart,
        &mut m.zend,
        &mut m.thick,
        &mut m.ps,
        &mut m.tr,
        &mut m.te,
        &mut m.ti,
    ] {
        *x = l.getf();
    }
    m.flip_angle = l.getd();
    m.dof = l.geti();
    m.fov = l.getf();
    m.location = l.getf();
    m.ptype = l.geti();
    match l.geti() {
        1 => {
            p.reinit_cache(&mut m.geom)?;
        }
        2 => {
            m.geom.i_to_r = None;
            m.geom.r_to_i = None;
        }
        _ => {}
    }
    if let Some(reg) = l.getm(p)? {
        m.geom.register_mat = Some(reg);
    }
    if let Some(ct) = l.getct(false) {
        m.ct = Some(ct);
    }
    Ok(m)
}

/// A printed volume (README.md): `null`, or the header, colour table and voxels.
fn putv(l: &mut Line, m: Option<&Mri>) {
    let Some(m) = m else {
        l.out("null");
        return;
    };
    l.puthdr(m);
    l.putct(m.ct.as_ref(), true);
    l.putvox(m);
}

/// D, the destination: `new`, `self`, `other` or a W.
enum D {
    New,
    Src,
    Other,
    W(Box<Mri>),
}

fn getdst(p: &mut Process, l: &mut Line, two: bool) -> Result<D, Stop> {
    if l.take("new") {
        return Ok(D::New);
    }
    if l.take("self") {
        return Ok(D::Src);
    }
    if l.take("other") {
        return Ok(if two { D::Other } else { D::New });
    }
    Ok(D::W(Box::new(getw(p, l)?)))
}

/// The call's destination argument.
fn dst(d: &mut D) -> Dst<'_> {
    match d {
        D::New => Dst::New,
        D::Src => Dst::Src,
        D::Other => Dst::Other,
        D::W(m) => Dst::Given(m),
    }
}

/// The result (README.md): `self`, `other` or `given` and that volume, or the new volume
/// (`null` for none); then a given destination's state when the result is not it.
fn putres(l: &mut Line, r: VolOut, a: &Mri, b: Option<&Mri>, d: &D) {
    let given = match d {
        D::W(m) => Some(m.as_ref()),
        _ => None,
    };
    match r {
        VolOut::Nothing => putv(l, None),
        VolOut::New(m) => putv(l, Some(&m)),
        VolOut::Src => {
            l.out("self");
            putv(l, Some(a));
        }
        VolOut::Other => {
            l.out("other");
            putv(l, b);
        }
        VolOut::Given => {
            l.out("given");
            putv(l, given);
            return;
        }
    }
    if let Some(g) = given {
        putv(l, Some(g));
    }
}

/// R, a region: `null` or six integers.
fn getreg(l: &mut Line) -> Option<Region> {
    if l.take("null") {
        return None;
    }
    Some(Region {
        x: l.geti(),
        y: l.geti(),
        z: l.geti(),
        dx: l.geti(),
        dy: l.geti(),
        dz: l.geti(),
    })
}

/// An output a call may leave: 7.0 before the call.
const UNSET: f32 = 7.0;
const UNSETD: f64 = 7.0;

/// One call: the command's printed values, or the end of the program.
fn call(p: &mut Process, l: &mut Line) -> Result<(), Stop> {
    let c = l.tok().to_owned();
    match c.as_str() {
        // part 1
        "vgwrite" => {
            let g = getg(l);
            let b = p.write_vol_geom(&g);
            l.out(hexbytes(&b));
        }
        "vgread" => {
            let text = l.getx();
            let mut g = getg(l);
            let mut s = CStream::new(text);
            p.errno = 0;
            p.read_vol_geom(&mut s, &mut g);
            let e = p.errno;
            l.putg(&g, true);
            l.puti(s.pos as i64);
            l.puti(i64::from(s.eof));
            l.puti(0);
            l.puti(i64::from(e));
            let end = (s.pos + 64).min(s.bytes.len());
            l.out(hexbytes(&s.bytes[s.pos..end]));
        }
        "vgprint" => {
            let g = getg(l);
            p.vg_print_obsolete(&g);
        }
        "getvg" | "copyvg" => {
            let a = getg(l);
            let mut b = getg(l);
            if c == "getvg" {
                p.get_vol_geom(&a, &mut b);
            } else {
                p.copy_vol_geom(&a, &mut b);
            }
            l.putg(&b, true);
        }
        "vgi2r" => {
            let g = getg(l);
            let m = p.vg_i_to_r(&g)?;
            l.putm(Some(&m));
        }
        "vgr2i" => {
            let g = getg(l);
            let m = p.vg_r_to_i(&g)?;
            l.putm(m.as_ref());
        }
        "tkrv2r" => {
            let g = getg(l);
            let m = p.tkr_vox2ras_from_vol_geom(&g)?;
            l.putm(Some(&m));
        }
        "vgeq" => {
            let a = getg(l);
            let b = getg(l);
            let t = l.getd();
            let r = p.vg_is_not_equal_thresh(&a, &b, t);
            l.puti(i64::from(r));
            let e = p.vg_is_equal(&a, &b, VG_IS_EQUAL_THRESHOLD);
            l.puti(i64::from(e));
        }
        // part 2
        "clone" => {
            let mut a = getw(p, l)?;
            let mut d = getdst(p, l, false)?;
            let r = p.mri_clone(&mut a, dst(&mut d))?;
            putres(l, r, &a, None, &d);
        }
        "clonesp" => {
            let a = getw(p, l)?;
            let (t, nf) = (l.geti(), l.geti());
            let m = p.mri_clone_by_space(&a, t, nf)?;
            putv(l, Some(&m));
        }
        "clear" => {
            let mut a = getw(p, l)?;
            let r = p.mri_clear(&mut a)?;
            l.puti(i64::from(r));
            putv(l, Some(&a));
        }
        "match" => {
            let a = getw(p, l)?;
            let b = getw(p, l)?;
            let r1 = p.mri_match(&a, &b);
            let r2 = p.mri_match_dimensions(&a, &b);
            l.puti(i64::from(r1));
            l.puti(i64::from(r2));
        }
        "chsize" => {
            let a = getw(p, l)?;
            let b = if l.take("null") {
                None
            } else {
                Some(getw(p, l)?)
            };
            let (w, h, d) = (l.geti(), l.geti(), l.geti());
            let r = p.mri_check_size(&a, b.as_ref(), w, h, d);
            l.puti(i64::from(r));
        }
        "cpframe" => {
            let mut a = getw(p, l)?;
            let mut d = getdst(p, l, false)?;
            let (sf, df) = (l.geti(), l.geti());
            let r = p.mri_copy_frame(&mut a, dst(&mut d), sf, df)?;
            putres(l, r, &a, None, &d);
        }
        "cpframes" => {
            let mut a = getw(p, l)?;
            let mut d = getdst(p, l, false)?;
            let (s0, s1, d0) = (l.geti(), l.geti(), l.geti());
            let r = p.mri_copy_frames(&mut a, dst(&mut d), s0, s1, d0)?;
            putres(l, r, &a, None, &d);
        }
        "linscale" | "smul" | "smulf" | "thresh" | "uthresh" | "invc" | "binz" | "abs" | "pos"
        | "neg" => {
            let mut a = getw(p, l)?;
            let mut d = getdst(p, l, false)?;
            let r = match c.as_str() {
                "linscale" => {
                    let (s, o, nz) = (l.getf(), l.getf(), l.geti());
                    p.mri_linear_scale(&mut a, dst(&mut d), s, o, nz)?
                }
                "smul" => {
                    let s = l.getf();
                    p.mri_scalar_mul(&mut a, dst(&mut d), s)?
                }
                "smulf" => {
                    let (s, f) = (l.getf(), l.geti());
                    p.mri_scalar_mul_frame(&mut a, dst(&mut d), s, f)?
                }
                "thresh" => {
                    let t = l.getf();
                    p.mri_threshold(&mut a, dst(&mut d), t)?
                }
                "uthresh" => {
                    let t = l.getf();
                    p.mri_upper_threshold_all_frames(&mut a, dst(&mut d), t)?
                }
                "invc" => {
                    let t = l.getf();
                    p.mri_invert_contrast(&mut a, dst(&mut d), t)?
                }
                "binz" => {
                    let (t, lo, hi) = (l.getf(), l.getf(), l.getf());
                    p.mri_binarize(&mut a, dst(&mut d), t, lo, hi)?
                }
                "abs" => p.mri_sign(&mut a, dst(&mut d), 0)?,
                "pos" => p.mri_sign(&mut a, dst(&mut d), 1)?,
                _ => p.mri_sign(&mut a, dst(&mut d), 2)?,
            };
            putres(l, r, &a, None, &d);
        }
        "mul2" | "sub2" | "add2" | "div2" => {
            let mut a = getw(p, l)?;
            let mut b = getw(p, l)?;
            let mut d = getdst(p, l, true)?;
            let r = match c.as_str() {
                "mul2" => p.mri_multiply(&mut a, &mut b, dst(&mut d))?,
                "sub2" => p.mri_subtract(&mut a, &mut b, dst(&mut d))?,
                "add2" => p.mri_add(&mut a, &mut b, dst(&mut d))?,
                _ => p.mri_divide(&mut a, &mut b, dst(&mut d))?,
            };
            putres(l, r, &a, Some(&b), &d);
        }
        "sam" => {
            let mut a = getw(p, l)?;
            let s = l.getf();
            let mut b = getw(p, l)?;
            let mut d = getdst(p, l, true)?;
            let r = p.mri_scale_and_multiply(&mut a, s, &mut b, dst(&mut d))?;
            putres(l, r, &a, Some(&b), &d);
        }
        "vrange" | "vrangef" | "vrangereg" => {
            let a = getw(p, l)?;
            let (mut lo, mut hi) = (UNSET, UNSET);
            let r = match c.as_str() {
                "vrange" => p.mri_val_range(&a, &mut lo, &mut hi)?,
                "vrangef" => {
                    let f = l.geti();
                    p.mri_val_range_frame(&a, &mut lo, &mut hi, f)?
                }
                _ => {
                    let reg = getreg(l);
                    p.mri_val_range_region(&a, &mut lo, &mut hi, reg.as_ref())?
                }
            };
            l.puti(i64::from(r));
            l.putf(lo);
            l.putf(hi);
        }
        "lrange" => {
            let a = getw(p, l)?;
            let lab = getw(p, l)?;
            let label = l.geti();
            let (mut lo, mut hi) = (UNSET, UNSET);
            let r = p.mri_label_val_range(&a, &lab, label, &mut lo, &mut hi)?;
            l.puti(i64::from(r));
            l.putf(lo);
            l.putf(hi);
        }
        "extract" | "extinto" | "extreg" => {
            let mut a = getw(p, l)?;
            let mut d = getdst(p, l, false)?;
            let r = if c == "extreg" {
                let reg = getreg(l);
                p.mri_extract_region(&mut a, dst(&mut d), reg.as_ref())?
            } else {
                let p0 = [l.geti(), l.geti(), l.geti()];
                let size = [l.geti(), l.geti(), l.geti()];
                let p1 = if c == "extinto" {
                    [l.geti(), l.geti(), l.geti()]
                } else {
                    [0, 0, 0]
                };
                p.mri_extract_into(&mut a, dst(&mut d), p0, size, p1)?
            };
            putres(l, r, &a, None, &d);
        }
        "cras" => {
            let mut a = getw(p, l)?;
            let d = getw(p, l)?;
            let p0 = [l.geti(), l.geti(), l.geti()];
            let p1 = [l.geti(), l.geti(), l.geti()];
            let r = p.mri_calc_cras_for_extracted_volume(&mut a, &d, p0, p1)?;
            for x in r {
                l.putd(x);
            }
            l.putg(&a.geom, false);
        }
        // part 3
        "sras" => {
            let mut a = getw(p, l)?;
            let k = l.geti();
            let m = match k {
                0 => Some(p.surface_ras_from_voxel(&mut a.geom)?),
                1 | 2 => p.voxel_from_surface_ras(&mut a.geom)?,
                _ => p.surface_ras_from_ras(&a.geom)?,
            };
            l.putm(m.as_ref());
            l.putg(&a.geom, false);
        }
        "spt" => {
            let mut a = getw(p, l)?;
            let k = l.geti();
            let (x, y, z) = (l.getd(), l.getd(), l.getd());
            let which = match k {
                0 => SurfacePoint::RasToSurface,
                1 => SurfacePoint::VoxelToSurface,
                _ => SurfacePoint::SurfaceToVoxel,
            };
            let (r, v) = p.surface_point(&mut a.geom, which, x, y, z)?;
            l.puti(i64::from(r));
            for x in v {
                l.putd(x);
            }
            l.putg(&a.geom, false);
        }
        "ostr" => {
            let g = getg(l);
            let mut s = [b'Z'; 8];
            let r = p.dircos_to_orientation_string(&g, &mut s);
            l.puti(i64::from(r));
            l.out(hexbytes(&s[..5]));
            let name = p.slice_direction_name(&g);
            l.puts(Some(name.as_bytes()));
            let d = p.get_slice_direction(&g);
            l.puti(i64::from(d));
        }
        "vtal" => {
            let mut a = getw(p, l)?;
            l.getxfm(&mut a);
            let (x, y, z) = (l.getd(), l.getd(), l.getd());
            let (r, v) = p.voxel_to_talairach(&mut a, x, y, z)?;
            l.puti(i64::from(r));
            for x in v.unwrap_or([UNSETD; 3]) {
                l.putd(x);
            }
            l.putg(&a.geom, false);
        }
        "inidx" => {
            let a = getw(p, l)?;
            let (x, y, z) = (l.getd(), l.getd(), l.getd());
            let r = p.mri_index_not_in_volume(&a, x, y, z);
            l.puti(i64::from(r));
        }
        "snear" => {
            let mut a = getw(p, l)?;
            let (x, y, z) = (l.getd(), l.getd(), l.getd());
            a.outside_val = l.getd();
            let (r, v) = p.mri_sample_nearest(&a, x, y, z)?;
            l.puti(i64::from(r));
            l.putd(v);
        }
        "com" => {
            let a = getw(p, l)?;
            let t = l.geti();
            let (r, means) = p.mri_center_of_mass(&a, t as u8)?;
            l.puti(i64::from(r));
            for x in means {
                l.putd(x);
            }
        }
        "pca" => {
            let a = getw(p, l)?;
            let t = l.geti();
            let mut ev = Matrix::zeros(3, 3, MATRIX_REAL);
            let mut evalues = [UNSET; 3];
            let (r, means) = p.mri_principle_components(&a, &mut ev, &mut evalues, t as u8)?;
            l.puti(i64::from(r));
            l.putm(Some(&ev));
            for x in evalues {
                l.putf(x);
            }
            for x in means {
                l.putd(x);
            }
        }
        "avg" => {
            let mut a = getw(p, l)?;
            let dof = l.geti();
            let mut d = getdst(p, l, false)?;
            let r = p.mri_average(&mut a, dof, dst(&mut d))?;
            putres(l, r, &a, None, &d);
        }
        "cmph" => {
            let a = if l.take("null") {
                None
            } else {
                Some(getw(p, l)?)
            };
            let b = if l.take("null") {
                None
            } else if l.take("self") {
                a.clone()
            } else {
                Some(getw(p, l)?)
            };
            let r = p.mri_compare_headers(a.as_ref(), b.as_ref());
            l.puti(i64::from(r));
        }
        "vdet" => {
            let a = getw(p, l)?;
            let d = p.mri_volume_determinant(&a);
            l.putf(d);
        }
        "dump" => {
            let a = getw(p, l)?;
            let r = p.mri_dump(&a);
            l.puti(i64::from(r));
        }
        "dsign" => {
            let s = l.getd();
            let r = p.mri_set_sample_volume_derivative_sign(s);
            l.putd(r);
        }
        // part 4
        "samp" => {
            let k = l.tok().to_owned();
            let mut a = getw(p, l)?;
            a.outside_val = l.getd();
            let n = l.geti();
            for _ in 0..n {
                let (x, y, z) = (l.getd(), l.getd(), l.getd());
                let (f, aux) = (l.geti(), l.geti());
                let mut v = UNSETD;
                let r = match k.as_str() {
                    "tri" => p.mri_sample_volume(&a, x, y, z, &mut v)?,
                    "typ" => p.mri_sample_volume_type(&a, x, y, z, &mut v, aux)?,
                    "frm" => p.mri_sample_volume_frame(&a, x, y, z, f, &mut v)?,
                    "fty" => p.mri_sample_volume_frame_type(&a, x, y, z, f, aux, &mut v)?,
                    "cub" => p.mri_cubic_sample_volume(&a, x, y, z, &mut v)?,
                    "cubf" => p.mri_cubic_sample_volume_frame(&a, x, y, z, f, &mut v)?,
                    "sinc" => p.mri_sinc_sample_volume(&a, x, y, z, aux, &mut v)?,
                    "sincf" => p.mri_sinc_sample_volume_frame(&a, x, y, z, f, aux, &mut v)?,
                    _ => args::bad("unknown sampler"),
                };
                l.puti(i64::from(r));
                l.putd(v);
            }
        }
        "coef" | "hsinc" => {
            let n = l.geti();
            for _ in 0..n {
                let x = l.getd();
                let v = if c == "coef" {
                    let i = l.geti();
                    p.mri_cubic_coeff(x, i)
                } else {
                    let fw = l.getd();
                    p.ham_sinc(x, fw)
                };
                l.putd(v);
            }
        }
        "trans" => {
            let mut a = getw(p, l)?;
            let mut d = getdst(p, l, false)?;
            let (dx, dy, dz) = (l.getd(), l.getd(), l.getd());
            let r = p.mri_translate(&mut a, dst(&mut d), dx, dy, dz)?;
            putres(l, r, &a, None, &d);
        }
        "rot" => {
            let mut a = getw(p, l)?;
            let mut d = getdst(p, l, false)?;
            let mr = l.getm(p)?;
            let mo = l.getm(p)?;
            let r = p.mri_rotate_i(&mut a, dst(&mut d), mr.as_ref(), mo.as_ref())?;
            putres(l, r, &a, None, &d);
        }
        "rotx" | "roty" | "rotz" => {
            let mut a = getw(p, l)?;
            let mut d = getdst(p, l, false)?;
            let angle = l.getf();
            let which = match c.as_str() {
                "rotx" => axis::X,
                "roty" => axis::Y,
                _ => axis::Z,
            };
            let r = p.mri_rotate_axis(&mut a, dst(&mut d), angle, which)?;
            putres(l, r, &a, None, &d);
        }
        "skull" => {
            let mut a = getw(p, l)?;
            let t = l.geti();
            let n = l.geti();
            for _ in 0..n {
                let c0 = [l.geti(), l.geti(), l.geti()];
                let c1 = [l.geti(), l.geti(), l.geti()];
                let v = l.getf();
                for z in c0[2]..=c1[2] {
                    for y in c0[1]..=c1[1] {
                        for x in c0[0]..=c1[0] {
                            p.mri_set_vox_val(&mut a, x, y, z, 0, v, 0)?;
                        }
                    }
                }
            }
            let mut b = Region {
                x: -7,
                y: -7,
                z: -7,
                dx: -7,
                dy: -7,
                dz: -7,
            };
            let r = p.mri_find_approximate_skull_bounding_box(&a, t, &mut b)?;
            l.puti(i64::from(r));
            for v in [b.x, b.y, b.z, b.dx, b.dy, b.dz] {
                l.puti(i64::from(v));
            }
        }
        "tftype" => {
            let n = l.geti();
            for _ in 0..n {
                let name = l.getx();
                let t = p.transform_file_name_type(&name);
                l.puti(i64::from(t));
            }
        }
        "ltaread" | "ltareads" => {
            let n = if c == "ltareads" { l.geti() } else { 1 };
            let mut files = Vec::new();
            for _ in 0..n {
                let name = l.getx();
                let have = !l.take("nofile");
                let bytes = if have { Some(l.getx()) } else { None };
                files.push((name, bytes));
            }
            let avg = l.geti();
            match avg {
                0 => p.env.remove(b"USE_AVERAGE305"),
                1 => p.env.set(b"USE_AVERAGE305", b"true"),
                _ => p.env.set(b"USE_AVERAGE305", b""),
            }
            for (name, bytes) in files {
                read_transform(p, l, &name, bytes.as_deref())?;
            }
        }
        // -- §23
        "cmalab" => {
            let n = l.geti();
            for _ in 0..n {
                let v = l.geti();
                let f = l.getf();
                for x in [f64::from(v), f64::from(f), f64::from(f)] {
                    let t: String = cma::label_tests(x)
                        .iter()
                        .map(|&b| if b { '1' } else { '0' })
                        .collect();
                    l.out(&t);
                }
                l.puti(i64::from(cma::is_sub_cortical_gray(v)));
                l.puti(i64::from(p.mri_aseg_contra_lat_label(v)));
            }
        }
        "cmavol" => {
            let mut aseg = getw(p, l)?;
            paint(p, l, &mut aseg)?;
            let mut ribbon = getw(p, l)?;
            paint(p, l, &mut ribbon)?;
            let hemi = l.geti();
            let a = p.supra_tentorial_vol_correction(&aseg, &ribbon)?;
            l.putd(a);
            let b = p.cortical_gm_vol_correction(&aseg, &ribbon, hemi)?;
            l.putd(b);
        }
        "lrswap" => {
            let mut aseg = getw(p, l)?;
            paint(p, l, &mut aseg)?;
            let r = p.mri_lrswap_aseg(&aseg)?;
            putv(l, Some(&r));
        }
        "fixrib" => {
            let mut aseg = getw(p, l)?;
            paint(p, l, &mut aseg)?;
            let mut ribbon = getw(p, l)?;
            paint(p, l, &mut ribbon)?;
            let mut d = getdst(p, l, true)?;
            let r = p.mri_fix_aseg_with_ribbon(&mut aseg, &mut ribbon, dst(&mut d))?;
            putres(l, r, &aseg, Some(&ribbon), &d);
        }
        "lrseg" => {
            let mut seg = getw(p, l)?;
            paint(p, l, &mut seg)?;
            let lh = l.getm(p)?.expect("a matrix");
            let rh = l.getm(p)?.expect("a matrix");
            let r = p.mri_lrswap_seg(&seg, &lh, &rh)?;
            putv(l, r.as_ref());
        }
        "ohe" => {
            let mut seg = getw(p, l)?;
            paint(p, l, &mut seg)?;
            let n = l.geti();
            let list: Vec<i32> = (0..n).map(|_| l.geti()).collect();
            let force1 = l.geti();
            let r = p.mri_one_hot_encode(&seg, &list, force1)?;
            putv(l, Some(&r));
        }
        "bvc" => {
            subject_folder(l.geti() != 0);
            let stats: Vec<f64> = (0..16).map(|_| l.getd()).collect();
            let r = p.cache_brain_volume_stats(&stats, b"subj", b".");
            match std::fs::read("subj/stats/brainvol.stats") {
                Ok(b) => l.out(hexbytes(&b)),
                Err(_) => l.out("null"),
            }
            subject_folder(false);
            r?;
        }
        "bvr" => {
            let make = l.geti() != 0;
            let text = l.getx();
            subject_folder(make);
            if make && std::fs::write("subj/stats/brainvol.stats", &text).is_err() {
                args::bad("cannot write the statistics file");
            }
            let r = p.read_cached_brain_volume_stats(b"subj", b".", &mut compute_stand_in);
            subject_folder(false);
            let stats = r?;
            l.puti(stats.len() as i64);
            for v in stats {
                l.putd(v);
            }
        }
        // the golden cases' cache text and reading (no file system)
        "bvtext" => {
            let stats: Vec<f64> = (0..16).map(|_| l.getd()).collect();
            l.out(hexbytes(&cma::brain_volume_stats_text(&stats)));
        }
        "bvparse" => {
            let text = l.getx();
            let stats = p.parse_brain_volume_stats(&text, b"./subj/stats/brainvol.stats")?;
            l.puti(stats.len() as i64);
            for v in stats {
                l.putd(v);
            }
        }
        // the golden cases' reader of in-memory files and a volume (no file system): `ltamem`
        // one file, `ltamems n` several in one process
        "ltamem" | "ltamems" => {
            let n = if c == "ltamems" { l.geti() } else { 1 };
            let mut files = Vec::new();
            for _ in 0..n {
                let name = l.getx();
                let bytes = if l.take("nofile") {
                    None
                } else {
                    Some(l.getx())
                };
                files.push((name, bytes));
            }
            let avg = l.geti();
            let vol = if l.take("none") {
                None
            } else {
                let vname = l.getx();
                Some((vname, getg(l)))
            };
            match avg {
                0 => p.env.remove(b"USE_AVERAGE305"),
                1 => p.env.set(b"USE_AVERAGE305", b"true"),
                _ => p.env.set(b"USE_AVERAGE305", b""),
            }
            let names: Vec<Vec<u8>> = files.iter().map(|(n, _)| n.clone()).collect();
            let mut m = Memory { files, vol };
            for name in names {
                p.errno = 0;
                let lta = p.lta_read_ex(&name, &mut m)?;
                l.puti(i64::from(p.errno));
                putlta(l, lta.as_ref());
            }
        }
        // part 6a: resolution, planes, fills, resampling by whole factors, changes of type
        "setres" => {
            let mut a = getw(p, l)?;
            let (x, y, z) = (l.getf(), l.getf(), l.getf());
            let r = p.mri_set_resolution(&mut a, x, y, z)?;
            l.puti(i64::from(r));
            putv(l, Some(&a));
        }
        "vfill" => {
            let mut a = getw(p, l)?;
            let v = l.getf();
            let r = p.mri_value_fill(&mut a, v)?;
            l.puti(i64::from(r));
            putv(l, Some(&a));
        }
        "fillfg" => {
            let mut a = getw(p, l)?;
            let mut d = getdst(p, l, false)?;
            let seed = [l.geti(), l.geti(), l.geti()];
            let (th, fv) = (l.geti(), l.geti());
            let mut npix = -7;
            let r = p.mri_fill_fg(&mut a, dst(&mut d), seed, th, fv, &mut npix)?;
            l.puti(i64::from(npix));
            putres(l, r, &a, None, &d);
        }
        "explane" => {
            let mut a = getw(p, l)?;
            let mut d = getdst(p, l, false)?;
            let (o, w) = (l.geti(), l.geti());
            let r = p.mri_extract_plane(&mut a, dst(&mut d), o, w)?;
            putres(l, r, &a, None, &d);
        }
        "fillplane" => {
            let mask = getw(p, l)?;
            let dv = getw(p, l)?;
            let (o, w, fv) = (l.geti(), l.geti(), l.geti());
            let mut d = D::W(Box::new(dv));
            if let D::W(m) = &mut d {
                p.mri_fill_plane(&mask, m, o, w, fv)?;
            }
            putres(l, VolOut::Given, &mask, None, &d);
        }
        "ups" => {
            let k = l.geti();
            let mut a = getw(p, l)?;
            let mut d = getdst(p, l, false)?;
            let n = l.geti();
            let r = match k {
                0 => p.mri_upsample_n(&mut a, dst(&mut d), n)?,
                1 => p.mri_upsample2(&mut a, dst(&mut d))?,
                _ => p.mri_upsample_n_conserve(&mut a, dst(&mut d), n)?,
            };
            putres(l, r, &a, None, &d);
        }
        "dns2" => {
            let mut a = getw(p, l)?;
            let mut d = getdst(p, l, false)?;
            let r = p.mri_downsample2(&mut a, dst(&mut d))?;
            putres(l, r, &a, None, &d);
        }
        "dnsn" => {
            let mut a = getw(p, l)?;
            let mut d = getdst(p, l, false)?;
            let (fc, fr, fs) = (l.getf(), l.getf(), l.getf());
            let keep = l.geti();
            let r = p.mri_downsample_n(&mut a, dst(&mut d), fc, fr, fs, keep)?;
            putres(l, r, &a, None, &d);
        }
        "mft" => {
            let a = getw(p, l)?;
            let f = l.geti();
            let t = l.getf();
            let r = p.mri_mean_frame_thresh(&a, f, t)?;
            l.putd(r);
        }
        "lim" => {
            let a = getw(p, l)?;
            let (mut lo, mut hi) = (UNSET, UNSET);
            let r = p.mri_limits(&a, &mut lo, &mut hi)?;
            l.puti(i64::from(r));
            l.putf(lo);
            l.putf(hi);
        }
        "chtype" => {
            let seq = l.geti();
            let mut a = getw(p, l)?;
            let t = l.geti();
            let (lo, hi) = (l.getf(), l.getf());
            let ns = l.geti();
            if l.geti() != 0 {
                p.env.set(b"FS_FORCE_BIN_CHECK", b"1");
            } else {
                p.env.remove(b"FS_FORCE_BIN_CHECK");
            }
            let r = if seq != 0 {
                p.mri_seq_change_type(&mut a, t, lo, hi, ns)?
            } else {
                p.mri_change_type(&a, t, lo, hi, ns)?
            };
            putv(l, r.as_ref());
            putv(l, Some(&a));
        }
        // part 6b: frame-range samplers, the trilinear kernel and splatting; derivatives,
        // gradients and neighbour counts; linear transforms of a volume
        "splat" => {
            let mut a = getw(p, l)?;
            let n = l.geti();
            for _ in 0..n {
                let k = l.geti();
                let (x, y, z) = (l.getd(), l.getd(), l.getd());
                let f = l.geti();
                let v = l.getd();
                let r = if k != 0 {
                    p.mri_interpolate_into_volume_frame(&mut a, x, y, z, f, v)?
                } else {
                    p.mri_interpolate_into_volume(&mut a, x, y, z, v)?
                };
                l.puti(i64::from(r));
            }
            putv(l, Some(&a));
        }
        "sseq" | "sxyz" => {
            const SLACK: i64 = 8;
            let mut a = getw(p, l)?;
            a.outside_val = l.getd();
            let n = l.geti();
            for _ in 0..n {
                if c == "sseq" {
                    let (x, y, z) = (l.getd(), l.getd(), l.getd());
                    let (first, last, kind) = (l.geti(), l.geti(), l.geti());
                    let cnt = if last >= first { i64::from(last) - i64::from(first) + 1 } else { 0 };
                    let len = SLACK + i64::from(last.max(0)) + 1 + SLACK;
                    let mut buf = vec![UNSET; len as usize];
                    let r = {
                        let mut put = |f: i32, v: f32| {
                            let i = SLACK + i64::from(f);
                            if (0..len).contains(&i) {
                                buf[i as usize] = v;
                            }
                        };
                        if kind == -1 {
                            p.mri_sample_seq_volume(&a, x, y, z, first, last, &mut put)?
                        } else {
                            p.mri_sample_seq_volume_type(&a, x, y, z, first, last, kind, &mut put)?
                        }
                    };
                    l.puti(i64::from(r));
                    for f in 0..cnt {
                        let i = SLACK + i64::from(first) + f;
                        l.putf(if (0..len).contains(&i) { buf[i as usize] } else { UNSET });
                    }
                } else {
                    let (x, y, z) = (l.geti(), l.geti(), l.geti());
                    let (begin, end, _k) = (l.geti(), l.geti(), l.geti());
                    let cnt = if end > begin { i64::from(end) - i64::from(begin) } else { 0 };
                    let mut buf = vec![UNSET; cnt as usize];
                    let r = {
                        let mut put = |j: i32, v: f32| {
                            if let Some(s) = usize::try_from(j).ok().and_then(|j| buf.get_mut(j)) {
                                *s = v;
                            }
                        };
                        p.mri_sample_volume_frame_xyz_int_n_range_floats(
                            &a, x, y, z, begin, end, &mut put,
                        )?
                    };
                    l.puti(i64::from(r));
                    for v in buf {
                        l.putf(v);
                    }
                }
            }
        }
        "tkern" => {
            let a = getw(p, l)?;
            let n = l.geti();
            for _ in 0..n {
                let (x, y, z) = (l.getd(), l.getd(), l.getd());
                let given = l.geti() == 0;
                let k = p.mri_trilin_kernel(&a, x, y, z);
                l.out(if given { "given" } else { "new" });
                for w in k {
                    l.putd(w);
                }
            }
        }
        "deriv" => {
            let a = getw(p, l)?;
            let s = l.getd();
            p.mri_set_sample_volume_derivative_sign(s);
            let n = l.geti();
            for _ in 0..n {
                let k = l.geti();
                let (x, y, z) = (l.getd(), l.getd(), l.getd());
                let (dx, dy, dz) = (l.getd(), l.getd(), l.getd());
                let sigma = l.getd();
                let mut mag = UNSETD;
                let r = if k != 0 {
                    p.mri_sample_volume_derivative_scale(&a, x, y, z, dx, dy, dz, &mut mag, sigma)?
                } else {
                    p.mri_sample_volume_derivative(&a, x, y, z, dx, dy, dz, &mut mag)?
                };
                l.puti(i64::from(r));
                l.putd(mag);
            }
        }
        "grad" => {
            let a = getw(p, l)?;
            let n = l.geti();
            for _ in 0..n {
                let k = l.geti();
                let (x, y, z) = (l.getd(), l.getd(), l.getd());
                let f = l.geti();
                let (mut gx, mut gy, mut gz) = (UNSETD, UNSETD, UNSETD);
                let r = if k != 0 {
                    p.mri_sample_volume_gradient_frame(&a, x, y, z, &mut gx, &mut gy, &mut gz, f)?
                } else {
                    p.mri_sample_volume_gradient(&a, x, y, z, &mut gx, &mut gy, &mut gz)?
                };
                l.puti(i64::from(r));
                l.putd(gx);
                l.putd(gy);
                l.putd(gz);
            }
        }
        "nbrs" => {
            let a = getw(p, l)?;
            let n = l.geti();
            for _ in 0..n {
                let k = l.geti();
                let (x, y, z) = (l.geti(), l.geti(), l.geti());
                let (pp, q) = (l.geti(), l.geti());
                let (lo, hi) = (l.getf(), l.getf());
                let count = match k {
                    0 => Neighbours::On(pp),
                    1 => Neighbours::On3x3(pp),
                    2 => Neighbours::InWindow(pp, q),
                    3 => Neighbours::Value(pp),
                    4 => Neighbours::InRange(pp, lo, hi),
                    5 => Neighbours::Value3x3(pp),
                    6 => Neighbours::Off(pp),
                    7 => Neighbours::Off3x3(pp),
                    _ => args::bad("unknown neighbour count"),
                };
                let r = p.mri_neighbours(&a, x, y, z, count)?;
                l.puti(i64::from(r));
            }
        }
        "setvals" => {
            let mut a = getw(p, l)?;
            let v = l.getf();
            let r = p.mri_set_values(&mut a, v)?;
            l.puti(i64::from(r));
            putv(l, Some(&a));
        }
        "rasvox" => {
            let a = getw(p, l)?;
            let b = if l.take("null") { None } else { Some(getw(p, l)?) };
            let m = l.getm(p)?;
            let r = p.mri_ras_xform_to_voxel_xform(&a, b.as_ref(), m.as_ref())?;
            l.putm(r.as_ref());
        }
        "lxf" => {
            let k = l.geti();
            let mut a = getw(p, l)?;
            a.outside_val = l.getd();
            let mut d = getdst(p, l, false)?;
            let m = l.getm(p)?;
            let interp = l.geti();
            let r = match k {
                0 => p.mri_linear_transform_interp(&mut a, dst(&mut d), m.as_ref(), interp)?,
                1 => p.mri_linear_transform(&mut a, dst(&mut d), m.as_ref())?,
                2 => p.mri_inverse_linear_transform(&mut a, dst(&mut d), m.as_ref())?,
                3 => p.mri_apply_ras_linear_transform_interp(
                    &mut a,
                    dst(&mut d),
                    m.as_ref(),
                    interp,
                )?,
                4 => p.mri_apply_ras_linear_transform(&mut a, dst(&mut d), m.as_ref())?,
                5 => p.mri_apply_ras_inverse_linear_transform(&mut a, dst(&mut d), m.as_ref())?,
                _ => args::bad("unknown transform function"),
            };
            putres(l, r, &a, None, &d);
        }
        "rsmat" => {
            let a = getw(p, l)?;
            let b = getw(p, l)?;
            let m = p.mri_get_resample_matrix(&a, &b)?;
            l.putm(m.as_ref());
        }
        _ => args::bad("unknown command"),
    }
    Ok(())
}

/// The cache's text written from `stats` (for the golden cases' generator).
#[allow(dead_code)]
pub fn cache_text(stats: &[f64]) -> Vec<u8> {
    cma::brain_volume_stats_text(stats)
}

/// P, a painting: `n`, then `n` times `x y z value`, frame 0's voxels set (`MRIsetVoxVal`).
fn paint(p: &mut Process, l: &mut Line, m: &mut Mri) -> Result<(), Stop> {
    let n = l.geti();
    for _ in 0..n {
        let (x, y, z) = (l.geti(), l.geti(), l.geti());
        let v = l.getf();
        p.mri_set_vox_val(m, x, y, z, 0, v, 0)?;
    }
    Ok(())
}

/// The probe's subject folder `subj/stats` in the working directory, made or not (any old
/// cache removed).
fn subject_folder(make: bool) {
    let _ = std::fs::remove_file("subj/stats/brainvol.stats");
    let _ = std::fs::remove_dir("subj/stats");
    let _ = std::fs::remove_dir("subj");
    if make {
        let _ = std::fs::create_dir_all("subj/stats");
    }
}

/// The caller's `ComputeBrainVolumeStats` (not in the spec: it reads a subject's surfaces) as
/// the probe's runs show it without a subject's files and without `FREESURFER_HOME`
/// (observed): its two lines and the end of the program, status 255.
fn compute_stand_in(p: &mut Process) -> Result<Vec<f64>, Stop> {
    p.put_out(
        b"[INFO] read, cannot find ./subj/surf/lh.white, trying ./subj/surf/lh.white.gii ...\n",
    );
    let rule = "-".repeat(74);
    p.put_err(
        format!(
            "{rule}\nERROR: FreeSurfer environment FREESURFER_HOME is not defined.\n  If you are outside the NMR-Martinos Center, please set this\n  variable to the location where you installed FreeSurfer.\n  If you are inside the NMR-Martinos Center, please source\n  the standard environment. If you need to install FreeSurfer,\n  go to: http://surfer.nmr.mgh.harvard.edu\n{rule}\n"
        )
        .as_bytes(),
    );
    Err(Stop::Exit(255))
}

/// The probe's `read_transform`: the file written (unless there is none), `errno` set to 0,
/// `LTAreadEx` of the name, the file removed; `errno` and the array printed.
fn read_transform(
    p: &mut Process,
    l: &mut Line,
    name: &[u8],
    bytes: Option<&[u8]>,
) -> Result<(), Stop> {
    let path = larmorx_fs_core::files::path_of(name);
    let _ = std::fs::remove_file(&path);
    if let Some(b) = bytes
        && std::fs::write(&path, b).is_err()
    {
        args::bad("cannot write the transform file");
    }
    p.errno = 0;
    let r = p.lta_read_ex(name, &mut FileSystem);
    if bytes.is_some() {
        let _ = std::fs::remove_file(&path);
    }
    let lta = r?;
    l.puti(i64::from(p.errno));
    putlta(l, lta.as_ref());
    Ok(())
}

/// The golden cases' transform inputs: files' bytes (a name without any: `ENOENT`) and one
/// named volume's geometry (others: `ENOENT`).
struct Memory {
    files: Vec<(Vec<u8>, Option<Vec<u8>>)>,
    vol: Option<(Vec<u8>, VolGeom)>,
}

impl TransformInputs for Memory {
    fn read_file(&mut self, name: &[u8]) -> Result<Vec<u8>, i32> {
        match self.files.iter().find(|(n, _)| n.as_slice() == name) {
            Some((_, Some(b))) => Ok(b.clone()),
            _ => Err(2),
        }
    }

    fn volume_geometry(&mut self, _p: &mut Process, name: &[u8]) -> Result<Option<VolGeom>, i32> {
        match &self.vol {
            Some((n, g)) if n.as_slice() == name => Ok(Some(g.clone())),
            _ => Err(2),
        }
    }
}

/// LT (the fs.mri probe's): `x0 y0 z0 sigma m_L m_dL m_last_dL type label src dst`.
fn putlt(l: &mut Line, lt: &LinearTransform) {
    l.putf(lt.x0);
    l.putf(lt.y0);
    l.putf(lt.z0);
    l.putf(lt.sigma);
    l.putm(lt.m_l.as_ref());
    l.putm(lt.m_dl.as_ref());
    l.putm(lt.m_last_dl.as_ref());
    l.puti(i64::from(lt.kind));
    l.puti(i64::from(lt.label));
    l.putg(&lt.src, true);
    l.putg(&lt.dst, true);
}

/// LTA (the fs.mri probe's): `<n> <type> <fscale> <subject>`, the transforms and the inverse
/// transforms; `null` for none.
fn putlta(l: &mut Line, lta: Option<&Lta>) {
    let Some(lta) = lta else {
        l.out("null");
        return;
    };
    l.puti(i64::from(lta.num_xforms()));
    l.puti(i64::from(lta.kind));
    l.putf(lta.fscale);
    l.puts(Some(&lta.subject));
    for lt in lta.xforms.iter().chain(&lta.inv_xforms) {
        putlt(l, lt);
    }
}

/// One line of a batch (its tokens, the `@<seconds>` and `+gdiag=<hex>` prefixes included) on a
/// fresh process state in `mode`: the probe's result line.
pub fn run_line(toks: &[String], mode: UpstreamDefects, env: &Environment) -> String {
    let mut first = 0;
    let mut gdiag = 0u64;
    while first < toks.len() {
        let t = &toks[first];
        if t.starts_with('@') {
        } else if let Some(h) = t.strip_prefix("+gdiag=") {
            gdiag = u64::from_str_radix(h, 16).unwrap_or(0);
        } else {
            break;
        }
        first += 1;
    }
    let (o, e) = (Buf::default(), Buf::default());
    let mut p = Process::new(env.clone(), mode, Box::new(o.clone()), Box::new(e.clone()));
    p.gdiag = gdiag;
    let mut l = Line::new(toks[first..].to_vec());
    let r = call(&mut p, &mut l);
    p.flush();
    let res = match r {
        Ok(()) => l.result(),
        Err(s) if s.status() == 0 => l.result(),
        Err(s) => format!("EXIT {}", s.status()),
    };
    let notices = p.take_notices();
    let mut err = e.0.lock().expect("buffer").clone();
    for n in notices.as_slice() {
        err.extend_from_slice(n.line().as_bytes());
        err.push(b'\n');
    }
    let out = o.0.lock().expect("buffer").clone();
    format!("{res} | {} {}", hexbytes(&out), hexbytes(&err))
}
