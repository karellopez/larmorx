// SPDX-License-Identifier: Apache-2.0
//! Statistics, the type names, the box crop, the centre of a sampled volume, orientation strings,
//! the 4 x 4 filler and the voxel-to-RAS matrices (spec §33, part 7a); FS-329, FS-330 and FS-462
//! to FS-464.

use larmorx_core::cscan::sscanf_f64;
use larmorx_core::defects::{FS_329, FS_330, FS_462, FS_463, FS_464};

use crate::clib::printf::sprintf_f64;
use crate::maths::Returned;
use crate::maths::matrix::Matrix;
use crate::mri::geom::DIAG_SHOW;
use crate::mri::{Mri, Region, types};
use crate::process::{DIAG_VERBOSE, Process, Stop, codes};
use crate::sse;

/// What `MRIstats` writes (§33): nothing for a volume without voxels; otherwise the count, the
/// mean and the standard deviation, and the minimum and maximum of the types it reads.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VolumeStats {
    pub min: Option<f32>,
    pub max: Option<f32>,
    pub n: i32,
    pub mean: f32,
    pub std: f32,
}

/// The variable that sets `MRIsetVoxelToRasXform`'s threshold and lets it change the voxel sizes.
pub const CHANGE_VOX_SIZE: &str = "FS_SetVoxToRasXform_Change_VoxSize";

/// `MRItype2str(type)` (§33): the type's name, `unknown data type` for the others (the complex
/// types too).
pub fn mri_type2str(t: i32) -> &'static str {
    match t {
        types::UCHAR => "uchar",
        types::SHORT => "short",
        types::USHRT => "ushrt",
        types::INT => "int",
        types::LONG => "long",
        types::FLOAT => "float",
        types::BITMAP => "bitmap",
        types::TENSOR => "tensor",
        types::RGB => "RGB",
        _ => "unknown data type",
    }
}

/// An orientation letter's axis (0 LR, 1 PA, 2 IS) and unit vector, any case.
fn letter(c: u8) -> Option<(usize, [f32; 3])> {
    Some(match c.to_ascii_uppercase() {
        b'L' => (0, [-1.0, 0.0, 0.0]),
        b'R' => (0, [1.0, 0.0, 0.0]),
        b'P' => (1, [0.0, -1.0, 0.0]),
        b'A' => (1, [0.0, 1.0, 0.0]),
        b'I' => (2, [0.0, 0.0, -1.0]),
        b'S' => (2, [0.0, 0.0, 1.0]),
        _ => return None,
    })
}

/// A voxel's value as the statistics read it: its `esize` bytes as the type's element, as a
/// `float` (a 64-bit volume under FS-329's `reproduce`: a 32-bit integer).
fn stat_value(kind: i32, esize: u64, b: [u8; 8]) -> f32 {
    match (kind, esize) {
        (types::UCHAR, _) => f32::from(b[0]),
        (types::SHORT, _) => f32::from(i16::from_le_bytes([b[0], b[1]])),
        (types::USHRT, _) => f32::from(u16::from_le_bytes([b[0], b[1]])),
        (types::FLOAT, _) => f32::from_le_bytes([b[0], b[1], b[2], b[3]]),
        (_, 8) => i64::from_le_bytes(b) as f32,
        _ => i32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f32,
    }
}

/// The C string's length (to its first NUL).
fn c_len(b: &[u8]) -> usize {
    b.iter().position(|&c| c == 0).unwrap_or(b.len())
}

impl Process {
    /// `MRIstats(mri, &min, &max, &n, &mean, &std)` (§33): `None` (nothing written) for a volume
    /// without voxels; else the count (32-bit, wrapping), the float sums of the voxels and their
    /// squares (x outermost, the frame innermost) and the minimum and maximum of the types read
    /// (bytes, shorts, unsigned shorts, 32- and 64-bit integers, floats). FS-329: `reproduce` reads
    /// a 64-bit volume's voxels as 32-bit integers at the voxels' indices.
    pub fn mri_stats(&mut self, m: &Mri) -> Option<VolumeStats> {
        if !m.has_voxels() {
            return None;
        }
        let g = &m.geom;
        let (w, h, d, nf) = (g.width, g.height, g.depth, m.nframes);
        let n = w.wrapping_mul(h).wrapping_mul(d).wrapping_mul(nf);
        let reproduce = self.mode().reproduce();
        let esize: Option<u64> = match m.kind {
            types::UCHAR => Some(1),
            types::SHORT | types::USHRT => Some(2),
            types::INT | types::FLOAT => Some(4),
            types::LONG => Some(if reproduce { 4 } else { 8 }),
            _ => None,
        };
        let (mut sum, mut sq) = (0.0f32, 0.0f32);
        let (mut min, mut max) = (None, None);
        if let Some(esize) = esize {
            if m.kind == types::LONG && w > 0 && h > 0 && d > 0 && nf > 0 {
                let how = if reproduce {
                    "32-bit integers at the voxels' indices, as FreeSurfer's"
                } else {
                    "its own values (FreeSurfer reads 32-bit integers at the voxels' indices)"
                };
                self.note(&FS_329, format!("MRIstats of a 64-bit volume: {how}"));
            }
            let (mut lo, mut hi) = (0.0f32, 0.0f32);
            let mut first = true;
            for x in 0..w {
                for y in 0..h {
                    for z in 0..d {
                        for f in 0..nf {
                            let place = self.place_of(
                                m,
                                i64::from(x),
                                i64::from(y),
                                i64::from(z),
                                i64::from(f),
                                esize,
                            );
                            let b = self.direct_read8(m, place, esize as usize, "MRIstats");
                            let v = stat_value(m.kind, esize, b);
                            if first {
                                (lo, hi, first) = (v, v, false);
                            }
                            if v < lo {
                                lo = v;
                            }
                            if v > hi {
                                hi = v;
                            }
                            sum = sse::add32(sum, v);
                            sq = sse::add32(sq, sse::mul32(v, v));
                        }
                    }
                }
            }
            if first {
                // no voxel walked: the minimum and maximum start at the first voxel all the same
                let place = self.place_of(m, 0, 0, 0, 0, esize);
                let b = self.direct_read8(m, place, esize as usize, "MRIstats");
                let v = stat_value(m.kind, esize, b);
                (lo, hi) = (v, v);
            }
            (min, max) = (Some(lo), Some(hi));
        }
        let nfl = n as f32;
        let mean = sse::div32(sum, nfl);
        let num = sse::sub32(sq, sse::mul32(sse::mul32(nfl, mean), mean));
        let q = sse::narrow(sse::div64(sse::widen(num), sse::sub64(f64::from(n), 1.0)));
        let std = sse::narrow(sse::sqrt64(sse::widen(q)));
        Some(VolumeStats {
            min,
            max,
            n,
            mean,
            std,
        })
    }

    /// `MRIprintStats(mri, stream)` (§33), on standard error when `to_stderr`, else standard
    /// output: the statistics' `<n> values`, `min`, `max`, `mean`, `std` (`%g`) and the centre of
    /// mass at threshold 0. FS-330: the values the statistics do not write are 0 here, noted.
    pub fn mri_print_stats(&mut self, m: &Mri, to_stderr: bool) -> Result<i32, Stop> {
        let s = self.mri_stats(m);
        if s.is_none() || s.is_some_and(|s| s.min.is_none()) {
            self.note(
                &FS_330,
                "MRIprintStats: the values the statistics do not write printed as 0 (FreeSurfer's are memory contents)",
            );
        }
        let s = s.unwrap_or(VolumeStats {
            min: None,
            max: None,
            n: 0,
            mean: 0.0,
            std: 0.0,
        });
        let g = |v: f32| sprintf_f64(b"%g", sse::widen(v));
        let mut b = format!("{} values\nmin = ", s.n).into_bytes();
        b.extend_from_slice(&g(s.min.unwrap_or(0.0)));
        b.extend_from_slice(b"\nmax = ");
        b.extend_from_slice(&g(s.max.unwrap_or(0.0)));
        b.extend_from_slice(b"\nmean = ");
        b.extend_from_slice(&g(s.mean));
        b.extend_from_slice(b"\nstd = ");
        b.extend_from_slice(&g(s.std));
        b.push(b'\n');
        if to_stderr {
            self.put_err(&b);
        } else {
            self.put_out(&b);
        }
        let (_, c) = self.mri_center_of_mass(m, 0)?;
        let mut b = b"com = ".to_vec();
        for (k, x) in c.iter().enumerate() {
            if k > 0 {
                b.push(b' ');
            }
            b.extend_from_slice(&sprintf_f64(b"%g", *x));
        }
        b.push(b'\n');
        if to_stderr {
            self.put_err(&b);
        } else {
            self.put_out(&b);
        }
        Ok(codes::NONE)
    }

    /// `MRIcropBoundingBox(mri, box)` (§33): the start raised to 0, each extent at most what is
    /// left of the volume from the start (32-bit). Returns `NO_ERROR`.
    pub fn mri_crop_bounding_box(&mut self, m: &Mri, b: &mut Region) -> i32 {
        b.x = b.x.max(0);
        b.y = b.y.max(0);
        b.z = b.z.max(0);
        let left = |e: i32, s: i32, ext: i32| {
            let r = e.wrapping_sub(s);
            if r < ext { r } else { ext }
        };
        b.dx = left(m.geom.width, b.x, b.dx);
        b.dy = left(m.geom.height, b.y, b.dy);
        b.dz = left(m.geom.depth, b.z, b.dz);
        codes::NONE
    }

    /// `MRIcalcCRASforSampledVolume(src, dst, &r, &a, &s)` (§33): the source point of the
    /// destination's centre for whole sampling factors, through the source's cached voxel-to-RAS
    /// matrix (computed and kept when absent). FS-462: a destination extent of 0 (or a division
    /// that overflows) ends the program here (code -4) in both modes.
    pub fn mri_calc_cras_for_sampled_volume(
        &mut self,
        src: &mut Mri,
        dst: &Mri,
    ) -> Result<[f64; 3], Stop> {
        let se = [src.geom.width, src.geom.height, src.geom.depth];
        let de = [dst.geom.width, dst.geom.height, dst.geom.depth];
        let mut q = [0.0f64; 3];
        for k in 0..3 {
            let Some(f) = se[k].checked_div(de[k]) else {
                self.note(
                    &FS_462,
                    "MRIcalcCRASforSampledVolume: a sampled volume's extent of 0 refused (FreeSurfer crashes)",
                );
                self.error_exit(
                    codes::BAD_PARM,
                    b"MRIcalcCRASforSampledVolume: the sampling factor is not defined (an extent of 0)",
                )?;
                return Ok([0.0; 3]);
            };
            let c = de[k] / 2;
            q[k] = sse::add64(
                f64::from(c.wrapping_mul(f)),
                sse::div64(sse::sub64(f64::from(f), 1.0), 2.0),
            );
        }
        let r = self.voxel_to_world(&mut src.geom, q[0], q[1], q[2])?;
        if self.gdiag & DIAG_SHOW != 0 && self.gdiag & DIAG_VERBOSE != 0 {
            let mut b = b"c_ras for sample volume is (".to_vec();
            for (k, x) in r.iter().enumerate() {
                if k > 0 {
                    b.extend_from_slice(b", ");
                }
                b.extend_from_slice(&sprintf_f64(b"%f", *x));
            }
            b.extend_from_slice(b") compared with the src (");
            for (k, x) in src.geom.c.iter().enumerate() {
                if k > 0 {
                    b.extend_from_slice(b", ");
                }
                b.extend_from_slice(&sprintf_f64(b"%f", sse::widen(*x)));
            }
            b.extend_from_slice(b")\n");
            self.put_err(&b);
        }
        Ok(r)
    }

    /// `MRIcheckOrientationString(ostr)` (§33) of the string's bytes as they lie in memory (its
    /// terminator excluded; a NUL inside is read as a character): `None` when the first three
    /// characters name each axis once, else the errors' text. FS-463: the places past the
    /// terminator of a string of fewer than two characters are taken as NUL, noted.
    pub fn mri_check_orientation_string(&mut self, ostr: &[u8]) -> Option<Vec<u8>> {
        if ostr.len() < 2 {
            self.note(
                &FS_463,
                "MRIcheckOrientationString: the characters past a string's end read as NUL (FreeSurfer reads memory contents)",
            );
        }
        let mut text: Vec<u8> = Vec::new();
        let mut count = [0u32; 3];
        for k in 0..3 {
            let c = ostr.get(k).copied().unwrap_or(0);
            match letter(c) {
                Some((axis, _)) => count[axis] += 1,
                None => {
                    text.extend_from_slice(b"Character ");
                    text.push(c);
                    text.extend_from_slice(
                        format!(" in position {} is invalid.\n", k + 1).as_bytes(),
                    );
                    text.truncate(c_len(&text));
                }
            }
        }
        const NAMES: [&str; 3] = ["LR", "PA", "IS"];
        for (k, n) in NAMES.iter().enumerate() {
            if count[k] > 1 {
                text.extend_from_slice(
                    format!("  {n} axis represented multiple times.\n").as_bytes(),
                );
            }
        }
        for (k, n) in NAMES.iter().enumerate() {
            if count[k] == 0 {
                text.extend_from_slice(format!("  {n} axis not represented.\n").as_bytes());
            }
        }
        if count == [1, 1, 1] { None } else { Some(text) }
    }

    /// `MRIorientationStringToDircos(mri, ostr)` (§33): a refused string prints `ERROR: in
    /// orientation string <ostr>` and the errors' text on standard output and returns 1;
    /// otherwise the direction cosine columns become the letters' unit vectors; returns 0.
    pub fn mri_orientation_string_to_dircos(&mut self, m: &mut Mri, ostr: &[u8]) -> i32 {
        if let Some(e) = self.mri_check_orientation_string(ostr) {
            let mut b = b"ERROR: in orientation string ".to_vec();
            b.extend_from_slice(&ostr[..c_len(ostr)]);
            b.push(b'\n');
            b.extend_from_slice(&e);
            self.put_out(&b);
            return 1;
        }
        let g = &mut m.geom;
        for (k, col) in [&mut g.x, &mut g.y, &mut g.z].into_iter().enumerate() {
            let (_, v) = letter(ostr[k]).expect("a checked letter");
            *col = v;
        }
        0
    }

    /// `stuff_four_by_four(m, m11, ..., m44)` (§33): the values row by row into the real element
    /// places; `ERROR_BADPARM` after the error for no matrix or another size than 4 x 4.
    pub fn stuff_four_by_four(&mut self, m: Option<&mut Matrix>, v: &[f32; 16]) -> i32 {
        let Some(m) = m else {
            return self.error_return(
                codes::BAD_PARM,
                codes::BAD_PARM,
                b"stuff_four_by_four(): matrix is NULL",
            );
        };
        if m.rows != 4 || m.cols != 4 {
            return self.error_return(
                codes::BAD_PARM,
                codes::BAD_PARM,
                b"stuff_four_by_four(): matrix is not four-by-four",
            );
        }
        for r in 0..4 {
            for c in 0..4 {
                m.set(r as i32 + 1, c as i32 + 1, v[4 * r + c]);
            }
        }
        codes::NONE
    }

    /// `MRIvoxelXformToRasXform(src, dst, m_vox, m_ras)` (§33): `vox2ras(dst or src) * (m_vox *
    /// ras2vox(src))`, the outer product into `m_ras` when given.
    pub fn mri_voxel_xform_to_ras_xform(
        &mut self,
        src: &Mri,
        dst: Option<&Mri>,
        m_vox: Option<&Matrix>,
        m_ras: Option<&mut Matrix>,
    ) -> Result<Returned<Matrix>, Stop> {
        let r2i = self.extract_r_to_i(&src.geom)?;
        let inner = match self.matrix_multiply(m_vox, r2i.as_ref(), None)? {
            Returned::New(m) => Some(m),
            _ => None,
        };
        let i2r = self.extract_i_to_r(&dst.unwrap_or(src).geom)?;
        self.matrix_multiply(Some(&i2r), inner.as_ref(), m_ras)
    }

    /// `MRIgetVoxelToVoxelXform(src, dst)` (§33): `ras2vox(dst) * vox2ras(src)`.
    pub fn mri_get_voxel_to_voxel_xform(
        &mut self,
        src: &Mri,
        dst: &Mri,
    ) -> Result<Option<Matrix>, Stop> {
        let i2r = self.extract_i_to_r(&src.geom)?;
        let r2i = self.extract_r_to_i(&dst.geom)?;
        Ok(
            match self.matrix_multiply(r2i.as_ref(), Some(&i2r), None)? {
                Returned::New(m) => Some(m),
                _ => None,
            },
        )
    }

    /// `MRIsetVoxelToRasXform(mri, M)` (§33): the direction cosines and the centre from the
    /// matrix's first three rows, after the voxel sizes' check against the columns' scales (the
    /// threshold and the change of sizes from `FS_SetVoxToRasXform_Change_VoxSize`); the cached
    /// matrices recomputed. FS-464: a matrix smaller than 3 x 4 is refused (`ERROR_BADPARM`).
    pub fn mri_set_voxel_to_ras_xform(
        &mut self,
        m: &mut Mri,
        mat: Option<&Matrix>,
    ) -> Result<i32, Stop> {
        let Some(mat) = mat.filter(|x| x.rows >= 3 && x.cols >= 4) else {
            self.note(
                &FS_464,
                "MRIsetVoxelToRasXform: a matrix smaller than 3 x 4 refused (FreeSurfer reads past its elements)",
            );
            return Ok(self.error_return(
                codes::BAD_PARM,
                codes::BAD_PARM,
                b"MRIsetVoxelToRasXform: the matrix is smaller than 3 x 4",
            ));
        };
        let e = |r: i32, c: i32| mat.at(r, c);
        let mut scale = [0.0f64; 3];
        for (c, s) in scale.iter_mut().enumerate() {
            let c = c as i32 + 1;
            let mut t = 0.0f64;
            // each new square the first operand where NaNs meet (observed: `vstat7`, a column with
            // NaNs in rows 2 and 3 gives row 3's)
            for r in 1..=3 {
                t = sse::add64(sse::widen(sse::mul32(e(r, c), e(r, c))), t);
            }
            *s = sse::sqrt64(t);
        }
        let var = self.env.get(CHANGE_VOX_SIZE).map(<[u8]>::to_vec);
        let thresh = var.as_deref().and_then(sscanf_f64).unwrap_or(0.001);
        let g = &m.geom;
        let size = [g.xsize, g.ysize, g.zsize];
        if (0..3).any(|k| (sse::widen(size[k]) - scale[k]).abs() > thresh) {
            let pg = |v: f64| sprintf_f64(b"%g", v);
            let mut b =
                b"WARNING: MRIsetVoxelToRasXform(): voxel size is inconsistent with scale (thresh="
                    .to_vec();
            b.extend_from_slice(&pg(thresh));
            b.extend_from_slice(b")\n  scale");
            for s in scale {
                b.push(b' ');
                b.extend_from_slice(&pg(s));
            }
            b.extend_from_slice(b"\n  vs");
            for s in size {
                b.push(b' ');
                b.extend_from_slice(&pg(sse::widen(s)));
            }
            b.push(b'\n');
            if var.is_some() {
                b.extend_from_slice(b"  changing voxel size to scale\n");
            }
            self.put_out(&b);
            if var.is_some() {
                let g = &mut m.geom;
                g.xsize = sse::narrow(scale[0]);
                g.ysize = sse::narrow(scale[1]);
                g.zsize = sse::narrow(scale[2]);
            }
        }
        let g = &mut m.geom;
        let size = [g.xsize, g.ysize, g.zsize];
        for r in 0..3 {
            g.x[r] = sse::div32(e(r as i32 + 1, 1), size[0]);
            g.y[r] = sse::div32(e(r as i32 + 1, 2), size[1]);
            g.z[r] = sse::div32(e(r as i32 + 1, 3), size[2]);
        }
        let ci = sse::narrow(f64::from(g.width) / 2.0);
        let cj = sse::narrow(f64::from(g.height) / 2.0);
        let ck = sse::narrow(f64::from(g.depth) / 2.0);
        for r in 0..3 {
            let row = r as i32 + 1;
            let s = sse::add32(sse::mul32(e(row, 1), ci), sse::mul32(e(row, 2), cj));
            let s = sse::add32(s, sse::mul32(e(row, 3), ck));
            g.c[r] = sse::add32(s, e(row, 4));
        }
        g.ras_good_flag = 1;
        let mut gg = m.geom.clone();
        self.reinit_cache(&mut gg)?;
        m.geom = gg;
        Ok(codes::NONE)
    }
}
