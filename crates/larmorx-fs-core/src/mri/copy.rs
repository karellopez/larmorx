// SPDX-License-Identifier: Apache-2.0
//! The header copy (spec §6) with the volume's MINC transform (§14), and the full copy (§13).

use larmorx_core::defects::{FS_56, FS_58, FS_60, FS_161, FS_162, FS_163, FS_166, FS_252};

use super::geom::{DIAG_SHOW, VolGeom};
use super::volume::Mri;
use super::xform::{MincTransform, TransformKind};
use super::{sizeof, types};
use crate::maths::Returned;
use crate::process::{DIAG_VERBOSE, Process, Stop, codes};

impl Process {
    /// `MRIcopyHeader(src, dst)` (§6): into a given volume, or a new volume without voxels of
    /// the source's extent, type and frames.
    pub fn mri_copy_header(
        &mut self,
        src: &Mri,
        dst: Option<&mut Mri>,
    ) -> Result<Returned<Mri>, Stop> {
        match dst {
            Some(d) => {
                self.copy_header_into(src, d)?;
                Ok(Returned::Output)
            }
            None => {
                let g = &src.geom;
                let mut d =
                    self.mri_alloc_header(g.width, g.height, g.depth, src.kind, src.nframes)?;
                self.copy_header_into(src, &mut d)?;
                Ok(Returned::New(d))
            }
        }
    }

    /// The header copy's fields (§6), in FreeSurfer's order.
    pub(crate) fn copy_header_into(&mut self, src: &Mri, d: &mut Mri) -> Result<(), Stop> {
        d.version = src.version;
        d.intent = src.intent;
        d.dof = src.dof;
        d.mean = src.mean;
        d.geom.xsize = src.geom.xsize;
        d.geom.ysize = src.geom.ysize;
        d.geom.zsize = src.geom.zsize;
        self.copy_transform(src, d)?;
        d.transform_fname = src.transform_fname.clone();
        if d.geom.depth == src.geom.depth {
            d.imnr0 = src.imnr0;
            d.imnr1 = src.imnr1;
        }
        d.ptype = src.ptype;
        d.fov = src.fov;
        d.thick = src.thick;
        d.ps = src.ps;
        d.location = src.location;
        d.xstart = src.xstart;
        d.xend = src.xend;
        d.ystart = src.ystart;
        d.yend = src.yend;
        d.zstart = src.zstart;
        d.zend = src.zend;
        d.flip_angle = src.flip_angle;
        d.tr = src.tr;
        d.te = src.te;
        d.ti = src.ti;
        d.geom.fname = src.geom.fname.clone();
        d.geom.x = src.geom.x;
        d.geom.y = src.geom.y;
        d.geom.z = src.geom.z;
        d.geom.c = src.geom.c;
        d.geom.ras_good_flag = src.geom.ras_good_flag;
        d.brightness = src.brightness;
        self.copy_register(&src.geom, &mut d.geom);
        d.subject_name = src.subject_name.clone();
        d.path_to_t1 = src.path_to_t1.clone();
        d.fname_format = src.fname_format.clone();
        d.gdf_image_stem = src.gdf_image_stem.clone();
        // The cached matrices: the voxel-to-RAS matrix's 16 floats; FS-252: a source without it
        // (FreeSurfer's header copy crashes) copies none in both modes. The RAS-to-voxel matrix by
        // the matrix copy (none when the source has none).
        if src.geom.i_to_r.is_none() {
            self.note(
                &FS_252,
                "a header copy from a source without its voxel-to-RAS matrix: none copied (FreeSurfer crashes)",
            );
        }
        d.geom.i_to_r = src.geom.i_to_r;
        d.geom.r_to_i = src.geom.r_to_i.clone();
        if src.auto_align.is_some() {
            d.auto_align = src.auto_align.clone();
        }
        if src.orig_ras2vox.is_some() {
            d.orig_ras2vox = src.orig_ras2vox.clone();
        }
        d.cmdlines = src.cmdlines.clone();
        if let Some(ct) = &src.ct {
            if self.gdiag & DIAG_SHOW != 0 && self.gdiag & DIAG_VERBOSE != 0 {
                self.put_out(b"MRIcopyHeader(): source has ctab. Copying ctab ...\n");
            }
            d.ct = self.ctab_deep_copy(Some(ct));
        }
        let z = |v: i32| v as i64 as u64;
        d.bytes_per_vox = sizeof(d.kind);
        d.vox_per_row = z(d.geom.width);
        d.vox_per_slice = d.vox_per_row.wrapping_mul(z(d.geom.height));
        d.vox_per_vol = d.vox_per_slice.wrapping_mul(z(d.geom.depth));
        d.vox_total = d.vox_per_vol.wrapping_mul(z(d.nframes));
        d.bytes_total = d.vox_total.wrapping_mul(d.bytes_per_vox);
        Ok(())
    }

    /// The registration matrix's copy (§6, FS-56).
    fn copy_register(&mut self, src: &VolGeom, d: &mut VolGeom) {
        match (&src.register_mat, d.register_mat.is_some()) {
            (Some(s), true) => {
                if self.mode().reproduce() {
                    // FreeSurfer's matrix copy into the destination's matrix
                    let mut o = d.register_mat.take().expect("a matrix");
                    let _ = self.matrix_copy(Some(s), crate::maths::Output::Given(&mut o));
                    d.register_mat = Some(o);
                } else {
                    d.register_mat = Some(s.clone());
                }
            }
            (Some(s), false) => {
                if self.mode().reproduce() {
                    self.note(
                        &FS_56,
                        "the header copy lost the source's registration matrix (the destination had none)",
                    );
                } else {
                    self.note(&FS_56, "the source's registration matrix copied");
                    d.register_mat = Some(s.clone());
                }
            }
            (None, _) => d.register_mat = None,
        }
    }

    /// §14: the destination's own transform deleted, then the source's linear transform copied
    /// (FS-60 when the source has none).
    fn copy_transform(&mut self, src: &Mri, d: &mut Mri) -> Result<(), Stop> {
        let owned = d.free_transform != 0 && d.transform.is_some();
        if owned
            && let Some(MincTransform {
                kind: TransformKind::Other(_),
                ..
            }) = &d.transform
        {
            self.put_err(b"delete_general_transform\n");
            self.flush();
            return Err(Stop::Exit(1));
        }
        let Some(t) = &src.transform else {
            if owned {
                self.note(
                    &FS_60,
                    "the destination's freed transform dropped (FreeSurfer still reports it, freed)",
                );
            }
            if owned || d.free_transform != 0 {
                d.transform = None;
                d.free_transform = 0;
            }
            return Ok(());
        };
        match t.kind {
            TransformKind::Linear => {
                d.transform = Some(MincTransform {
                    kind: TransformKind::Linear,
                    inverse_flag: false,
                    linear: *t.effective(),
                    inverse: *t.effective_inverse(),
                });
                d.free_transform = 1;
                Ok(())
            }
            TransformKind::Concatenated => {
                self.put_err(b"get_linear_transform_ptr\n");
                self.flush();
                Err(Stop::Exit(1))
            }
            TransformKind::Other(_) => {
                self.put_err(b"copy_and_invert_transform bad kind\n");
                self.flush();
                Err(Stop::Exit(1))
            }
        }
    }

    /// `MRIcopy(src, dst)` (§13): the header and voxels into a given volume or a new one.
    pub fn mri_copy(&mut self, src: &Mri, dst: Option<&mut Mri>) -> Result<Returned<Mri>, Stop> {
        let g = &src.geom;
        let mut fresh = None;
        let given = dst.is_some();
        let d: &mut Mri = match dst {
            Some(d) => {
                let same = d.geom.width == g.width
                    && d.geom.height == g.height
                    && d.geom.depth == g.depth
                    && d.nframes == src.nframes;
                if !same {
                    if self.mode().reproduce() {
                        self.note(
                            &FS_163,
                            "a destination of another extent: FreeSurfer's placement of the rows",
                        );
                    } else {
                        self.note(&FS_163, "a destination of another extent: refused");
                        let msg = format!(
                            "MRIcopy: a {} x {} x {} x {} destination for a {} x {} x {} x {} source",
                            d.geom.width,
                            d.geom.height,
                            d.geom.depth,
                            d.nframes,
                            g.width,
                            g.height,
                            g.depth,
                            src.nframes
                        );
                        self.error_exit(codes::BAD_PARM, msg.as_bytes())?;
                    }
                }
                if src.has_voxels() && !d.has_voxels() {
                    self.note(
                        &FS_58,
                        "MRIcopy into a destination without voxels: refused (FreeSurfer crashes)",
                    );
                    self.error_exit(codes::BAD_PARM, b"MRIcopy: the destination has no voxels")?;
                }
                d
            }
            None => {
                let m = if src.has_voxels() {
                    self.mri_alloc_sequence(g.width, g.height, g.depth, src.kind, src.nframes)?
                } else {
                    let mut m = self.mri_alloc_header(g.width, g.height, g.depth, src.kind, 1)?;
                    m.nframes = src.nframes;
                    m
                };
                fresh.insert(m)
            }
        };
        let own_ptype = d.ptype;
        self.copy_header_into(src, d)?;
        d.ptype = own_ptype;
        if src.has_voxels() {
            self.copy_voxels(src, d)?;
        }
        d.geom.fname = src.geom.fname.clone();
        Ok(match fresh {
            Some(m) => Returned::New(m),
            None if given => Returned::Output,
            None => Returned::Nothing,
        })
    }

    /// The voxels' copy of §13.
    fn copy_voxels(&mut self, src: &Mri, d: &mut Mri) -> Result<(), Stop> {
        let (sk, dk) = (src.kind, d.kind);
        if sk == dk {
            return self.copy_rows(src, d);
        }
        let conversion = matches!(
            (sk, dk),
            (types::FLOAT, types::SHORT | types::USHRT | types::UCHAR)
                | (
                    types::UCHAR | types::SHORT | types::INT | types::RGB,
                    types::FLOAT
                )
                | (types::SHORT, types::UCHAR)
        );
        if conversion {
            return self.copy_converted(src, d);
        }
        let g = &d.geom;
        let (w, h, dd, nf) = (g.width, g.height, g.depth, d.nframes);
        for x in 0..w {
            for y in 0..h {
                for z in 0..dd {
                    for f in 0..nf {
                        let v = self.mri_get_vox_val(src, x, y, z, f, 0)?;
                        self.mri_set_vox_val(d, x, y, z, f, v, 0)?;
                    }
                }
            }
        }
        Ok(())
    }

    /// FS-163 (both modes): a row of the source's that would not land inside the destination.
    fn too_small(&mut self) -> Result<(), Stop> {
        self.note(
            &FS_163,
            "a destination too small for the source's rows: refused (FreeSurfer writes past it)",
        );
        self.error_exit(
            codes::BAD_PARM,
            b"MRIcopy: the destination is too small for the source's voxels",
        )
    }

    /// The byte offset of row `y` of slice index `t` in a volume, when the row's `len` bytes
    /// lie inside its tables and buffer.
    fn row_offset(m: &Mri, t: i64, y: i64, len: u64) -> Option<u64> {
        let g = &m.geom;
        let (w, h) = (i64::from(g.width), i64::from(g.height));
        if t < 0 || t >= i64::from(g.depth) * i64::from(m.nframes) || y < 0 || y >= h {
            return None;
        }
        let o = ((t * h + y) * w) as u64 * m.bytes_per_vox;
        let n = m.voxels.as_ref().map_or(0, |v| v.len() as u64);
        (o + len <= n).then_some(o)
    }

    /// The same type's copy, row by row (FS-161, FS-166).
    fn copy_rows(&mut self, src: &Mri, d: &mut Mri) -> Result<(), Stop> {
        let g = &src.geom;
        let (w, h, depth, nf) = (g.width, g.height, g.depth, src.nframes);
        let size = src.bytes_per_vox;
        let partial = matches!(src.kind, types::TENSOR | types::FLOAT_COMPLEX);
        let reproduce = self.mode().reproduce();
        if partial && w > 0 && h > 0 && depth > 0 && nf > 0 {
            if reproduce {
                self.note(
                    &FS_161,
                    "a tensor or complex volume's rows copied as width bytes, as FreeSurfer copies them",
                );
            } else {
                self.note(&FS_161, "a tensor or complex volume's whole rows copied");
            }
        }
        if src.kind == types::TENSOR && w > 0 && h > 0 && depth > 0 && nf > 0 {
            if reproduce {
                self.note(
                    &FS_166,
                    "a tensor volume copied as FreeSurfer copies it: the first width bytes only",
                );
                let n = w as usize;
                let s = src.voxels.as_ref().expect("voxels")[..n].to_vec();
                let dv = d.voxels.as_mut().expect("voxels");
                if dv.len() >= n {
                    dv[..n].copy_from_slice(&s);
                } else {
                    return self.too_small();
                }
                return Ok(());
            }
            self.note(&FS_166, "a tensor volume's rows copied where they belong");
        }
        let len = if partial && reproduce {
            w as u64
        } else {
            w as u64 * size
        };
        for f in 0..i64::from(nf) {
            for z in 0..i64::from(depth) {
                let t = z + f * i64::from(depth);
                for y in 0..i64::from(h) {
                    let so = Self::row_offset(src, t, y, len).expect("the source's own row");
                    let Some(dof) = Self::row_offset(d, t, y, len) else {
                        return self.too_small();
                    };
                    let row = src.voxels.as_ref().expect("voxels")
                        [so as usize..(so + len) as usize]
                        .to_vec();
                    d.voxels.as_mut().expect("voxels")[dof as usize..(dof + len) as usize]
                        .copy_from_slice(&row);
                }
            }
        }
        Ok(())
    }

    /// The conversions of §13 (FS-162), element by element over the source's extent into the
    /// destination's row y of slice `z + f dst_depth`.
    fn copy_converted(&mut self, src: &Mri, d: &mut Mri) -> Result<(), Stop> {
        let g = &src.geom;
        let (w, h, depth, nf) = (g.width, g.height, g.depth, src.nframes);
        let ddepth = i64::from(d.geom.depth);
        let dsize = d.bytes_per_vox;
        for f in 0..nf {
            for z in 0..depth {
                for y in 0..h {
                    let t = i64::from(z) + i64::from(f) * ddepth;
                    let len = w.max(0) as u64 * dsize;
                    let Some(row) = Self::row_offset(d, t, i64::from(y), len) else {
                        return self.too_small();
                    };
                    for x in 0..w {
                        let super::voxels::Place::In(so) = src.flat_place(x, y, z, f) else {
                            continue;
                        };
                        let bytes = self.convert(src, so, d.kind)?;
                        let o = (row + x as u64 * dsize) as usize;
                        d.voxels.as_mut().expect("voxels")[o..o + bytes.len()]
                            .copy_from_slice(&bytes);
                    }
                }
            }
        }
        Ok(())
    }

    /// One converted element (§13, FS-162).
    fn convert(&mut self, src: &Mri, so: u64, to: i32) -> Result<Vec<u8>, Stop> {
        let o = so as usize;
        let v = src.voxels.as_ref().expect("voxels");
        let reproduce = self.mode().reproduce();
        let (lo, hi): (i64, i64) = match to {
            types::SHORT => (-32768, 32767),
            types::USHRT => (0, 65535),
            types::UCHAR => (0, 255),
            _ => (0, 0),
        };
        let int: i64 = match (src.kind, to) {
            (types::FLOAT, types::FLOAT) => unreachable!("same type"),
            (_, types::FLOAT) => {
                let x: f32 = match src.kind {
                    types::UCHAR => f32::from(v[o]),
                    types::SHORT => f32::from(i16::from_le_bytes([v[o], v[o + 1]])),
                    _ => i32::from_le_bytes(v[o..o + 4].try_into().expect("4")) as f32,
                };
                return Ok(x.to_le_bytes().to_vec());
            }
            (types::FLOAT, _) => {
                // the rounding first, then (unsigned char) the clip at 255 (measured: a float
                // beyond the int range rounds to -2147483648 and keeps its low byte, 0)
                let x = f32::from_le_bytes(v[o..o + 4].try_into().expect("4"));
                let i = i64::from(self.nint(f64::from(x)));
                if to == types::UCHAR { i.min(255) } else { i }
            }
            _ => i64::from(i16::from_le_bytes([v[o], v[o + 1]])),
        };
        let fixed = int.clamp(lo, hi);
        if fixed != int {
            if reproduce {
                self.note(
                    &FS_162,
                    format!("the value {int} wrapped around the destination type's range"),
                );
            } else {
                self.note(
                    &FS_162,
                    format!("the value {int} clipped to the destination type's range"),
                );
            }
        }
        let x = if reproduce { int } else { fixed };
        Ok(match to {
            types::SHORT => (x as i16).to_le_bytes().to_vec(),
            types::USHRT => (x as u16).to_le_bytes().to_vec(),
            _ => vec![x as u8],
        })
    }
}
