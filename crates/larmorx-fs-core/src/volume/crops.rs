// SPDX-License-Identifier: Apache-2.0
//! Crops (spec §57): `MRIcropToFoV`, `MRIcropAroundCRS` (FS-517) and `MRIcropAroundRAS` (FS-515,
//! FS-516).

use larmorx_core::defects::{FS_515, FS_516, FS_517};

use super::ltachain::LtaInto;
use super::sample::cvtt;
use super::{Dst, VolOut};
use crate::clib::printf::sprintf_f64;
use crate::maths::Returned;
use crate::maths::matrix::{MATRIX_REAL, Matrix, real_point_f};
use crate::mri::geom::VolGeom;
use crate::mri::lta::lta_type;
use crate::mri::{Lta, Mri, Region};
use crate::process::{Process, Stop, codes};
use crate::sse;

/// A volume's extent.
fn extent(m: &Mri) -> (i32, i32, i32) {
    (m.geom.width, m.geom.height, m.geom.depth)
}

/// `%g` of a `double`.
fn g(x: f64) -> Vec<u8> {
    sprintf_f64(b"%g", x)
}

/// Three `%g`s separated by `sep`.
fn g3(x: [f64; 3], sep: &[u8]) -> Vec<u8> {
    let mut b = g(x[0]);
    for v in &x[1..] {
        b.extend_from_slice(sep);
        b.extend_from_slice(&g(*v));
    }
    b
}

/// The FLT_EPSILON threshold `vg_isEqual` uses.
const VG_THRESHOLD: f64 = f32::EPSILON as f64;

impl Process {
    /// `MRIcropToFoV(mask, cFoV, rFoV, sFoV, thresh)` (§57): the region around the centroid of
    /// the mask's voxels at or above `thresh` (a NaN voxel or threshold counts), of the fields
    /// of view given but at most the mask's extent; its messages.
    pub fn mri_crop_to_fov(
        &mut self,
        mask: &Mri,
        fov: [i32; 3],
        thresh: f64,
    ) -> Result<Region, Stop> {
        let mut b = b"MRIcropToFoV(): thresh=".to_vec();
        b.extend_from_slice(&g(thresh));
        b.extend_from_slice(format!(" FoV = ({},{},{}) \n", fov[0], fov[1], fov[2]).as_bytes());
        self.put_out(&b);
        let (w, h, d) = extent(mask);
        let mut n: i64 = 0;
        let mut sum = [0f64; 3];
        let mut lo = [1_000_000i32; 3];
        let mut hi = [0i32; 3];
        for c in 0..w {
            for r in 0..h {
                for s in 0..d {
                    let v = f64::from(self.vf(mask, (c, r, s), 0)?);
                    if v < thresh {
                        continue;
                    }
                    n += 1;
                    for (k, x) in [c, r, s].into_iter().enumerate() {
                        sum[k] = sse::add64(sum[k], f64::from(x));
                        lo[k] = lo[k].min(x);
                        hi[k] = hi[k].max(x);
                    }
                }
            }
        }
        let centroid = sum.map(|x| sse::div64(x, n as f64));
        let delta = [0, 1, 2].map(|k| hi[k] - lo[k] + 1);
        let mut b = format!("nhits={n}  centroid = (").into_bytes();
        b.extend_from_slice(&g3(centroid, b","));
        b.extend_from_slice(
            format!(
                ") min = ({},{},{}) max = ({},{},{}) delta = ({},{},{}) \n",
                lo[0], lo[1], lo[2], hi[0], hi[1], hi[2], delta[0], delta[1], delta[2]
            )
            .as_bytes(),
        );
        self.put_out(&b);
        let ext = [w, h, d];
        let mut corner = [0i32; 3];
        let mut size = [0i32; 3];
        for k in 0..3 {
            let x = cvtt(sse::sub64(centroid[k], f64::from(fov[k] / 2)).round());
            corner[k] = x.max(0);
            size[k] = fov[k].min(ext[k]);
        }
        self.put_out(
            format!(
                "Region {} {} {}  {} {} {}\n",
                corner[0], corner[1], corner[2], size[0], size[1], size[2]
            )
            .as_bytes(),
        );
        for (k, name) in ["cFoV", "rFoV", "sFoV"].iter().enumerate() {
            if size[k] != fov[k] {
                self.put_out(
                    format!(
                        "WARNING: {name} is {}, could not achieve {}\n",
                        size[k], fov[k]
                    )
                    .as_bytes(),
                );
            }
        }
        for (k, axis) in ["c", "r", "s"].iter().enumerate() {
            if corner[k] > lo[k] {
                self.put_out(format!("WARNING: clip {axis}min \n").as_bytes());
            }
            if size[k] < hi[k] - lo[k] {
                self.put_out(format!("WARNING: clip {axis}max \n").as_bytes());
            }
        }
        Ok(Region {
            x: corner[0],
            y: corner[1],
            z: corner[2],
            dx: size[0],
            dy: size[1],
            dz: size[2],
        })
    }

    /// `MRIcropAroundCRS(vol, center, FoV, iKeep)` (§57): the voxels from
    /// `round(center - floor((FoV - 1) / 2))` on, `FoV` of them per axis; inside the volume
    /// `MRIextractRegion`, otherwise a new volume (its centre from its first voxel's RAS) with the
    /// voxels inside copied; with `keep`, voxels whose value is none of its labels become 0.
    /// FS-517: a last voxel beyond `int` ends the program in both modes.
    pub fn mri_crop_around_crs(
        &mut self,
        vol: &mut Mri,
        center: [f64; 3],
        fov: [i32; 3],
        keep: &[i32],
    ) -> Result<Option<Mri>, Stop> {
        let (w, h, d) = extent(vol);
        let mut first = [0i32; 3];
        let mut last = [0i32; 3];
        let mut beyond = false;
        for k in 0..3 {
            let half = (f64::from(fov[k]) - 1.0) / 2.0;
            first[k] = cvtt(sse::sub64(center[k], half.floor()).round());
            let l = i64::from(first[k]) + i64::from(fov[k]) - 1;
            beyond |= i32::try_from(l).is_err();
            last[k] = l as i32;
        }
        let mut b = format!("MRIcropAroundCRS()\nvol size {w} {h} {d}\ncenter ").into_bytes();
        b.extend_from_slice(&g3(center, b" "));
        b.extend_from_slice(
            format!(
                "\nfov {} {} {}\nlimits {} {}   {} {}   {} {}\n",
                fov[0], fov[1], fov[2], first[0], last[0], first[1], last[1], first[2], last[2]
            )
            .as_bytes(),
        );
        if !keep.is_empty() {
            b.extend_from_slice(b"iKeep ");
            for k in keep {
                b.extend_from_slice(format!("{k} ").as_bytes());
            }
            b.push(b'\n');
        }
        self.put_out(&b);
        if beyond {
            self.note(
                &FS_517,
                "MRIcropAroundCRS: a last voxel beyond the int range: the program ended (FreeSurfer loops through the int range and writes outside the volume)",
            );
            self.error_exit(
                codes::BAD_PARM,
                b"MRIcropAroundCRS(): the last voxel lies beyond the int range (FreeSurfer loops through the int range and writes outside the volume here)\n",
            )?;
            return Ok(None);
        }
        let ext = [w, h, d];
        let inside = (0..3).all(|k| first[k] >= 0 && last[k] < ext[k]);
        let mut out = if inside {
            self.put_out(b"  using MRIextractRegion()\n");
            let reg = Region {
                x: first[0],
                y: first[1],
                z: first[2],
                dx: fov[0],
                dy: fov[1],
                dz: fov[2],
            };
            match self.mri_extract_region(vol, Dst::New, Some(&reg))? {
                VolOut::New(m) => m,
                _ => return Ok(None),
            }
        } else {
            let mut m =
                self.alloc_with_header(vol, (fov[0], fov[1], fov[2]), vol.kind, vol.nframes)?;
            self.copy_pulse_parameters(vol, &mut m);
            m.ct = vol.ct.as_ref().and_then(|ct| self.ctab_deep_copy(Some(ct)));
            let v = self.vox2ras_scanner(&vol.geom, 0, false)?;
            let p = real_point_f(&v, [first[0] as f32, first[1] as f32, first[2] as f32, 1.0]);
            let mut b = b"P0 ".to_vec();
            b.extend_from_slice(&g3(p.map(sse::widen)[..3].try_into().expect("3"), b" "));
            b.push(b'\n');
            self.put_out(&b);
            self.p0_to_cras(
                &mut m.geom,
                sse::widen(p[0]),
                sse::widen(p[1]),
                sse::widen(p[2]),
            )?;
            let mut b = b"c_ras of new volume ".to_vec();
            b.extend_from_slice(&g3(m.geom.c.map(sse::widen), b" "));
            b.push(b'\n');
            self.put_out(&b);
            for c in 0..fov[0] {
                for r in 0..fov[1] {
                    for s in 0..fov[2] {
                        let (x, y, z) = (first[0] + c, first[1] + r, first[2] + s);
                        if x < 0 || x >= w || y < 0 || y >= h || z < 0 || z >= d {
                            continue;
                        }
                        for f in 0..vol.nframes {
                            let v = self.vf(vol, (x, y, z), f)?;
                            self.mri_set_vox_val(&mut m, c, r, s, f, v, 0)?;
                        }
                    }
                }
            }
            m
        };
        if !keep.is_empty() {
            let (ow, oh, od) = extent(&out);
            for c in 0..ow {
                for r in 0..oh {
                    for s in 0..od {
                        for f in 0..out.nframes {
                            let v = self.vf(&out, (c, r, s), f)?;
                            if !keep.contains(&cvtt(f64::from(v))) {
                                self.mri_set_vox_val(&mut out, c, r, s, f, 0.0, 0)?;
                            }
                        }
                    }
                }
            }
        }
        Ok(Some(out))
    }

    /// `vg_isEqual` of a volume's geometry with one of an array's: by default the volume taken
    /// as valid; FS-516's `reproduce` its own `valid` flag.
    fn crop_vol_is(&mut self, vol: &VolGeom, g: &VolGeom, own_flag: bool) -> bool {
        let mut a = vol.clone();
        if !own_flag {
            a.valid = 1;
        }
        self.vg_is_equal(&a, g, VG_THRESHOLD) != 0
    }

    /// `MRIcropAroundRAS(vol, rasCenter, voxFoVRAS, lta, iKeep)` (§57): the RAS centre's voxel
    /// by the inverse vox2ras (through a copy of the array made RAS to RAS, inverted when the
    /// volume is its source: FS-516's direction), each voxel axis the field of view its
    /// orientation letter names (FS-515), then `MRIcropAroundCRS`.
    pub fn mri_crop_around_ras(
        &mut self,
        vol: &mut Mri,
        ras: [f64; 3],
        fov_ras: [i32; 3],
        lta: Option<&Lta>,
        keep: &[i32],
    ) -> Result<Option<Mri>, Stop> {
        let reproduce = self.mode().reproduce();
        let vox2ras = self.vox2ras_scanner(&vol.geom, 0, false)?;
        let mut ras2vox = match self.matrix_inverse(Some(&vox2ras), None)? {
            Returned::New(m) => Some(m),
            _ => None,
        };
        if let Some(lta) = lta {
            let Some((first, last)) = lta.xforms.first().zip(lta.xforms.last()) else {
                return Ok(None);
            };
            let (src, dst) = (first.src.clone(), last.dst.clone());
            let decide = |p: &mut Process, own: bool| {
                (
                    p.crop_vol_is(&vol.geom, &src, own),
                    p.crop_vol_is(&vol.geom, &dst, own),
                )
            };
            let (fs, fd) = decide(self, false);
            let (rs, rd) = decide(self, true);
            // the default: inverted when the volume is the source only, refused when neither end
            let fix = match (fs, fd) {
                (true, false) => Some(true),
                (false, false) => None,
                _ => Some(false),
            };
            // FreeSurfer's: inverted when the volume is the destination only, refused when both
            let rep = match (rs, rd) {
                (false, true) => Some(true),
                (true, true) => None,
                _ => Some(false),
            };
            if fix != rep {
                if reproduce {
                    self.note(
                        &FS_516,
                        "MRIcropAroundRAS: the array's direction decided backwards on the volume's own validity flag (FreeSurfer's)",
                    );
                } else {
                    self.note(
                        &FS_516,
                        "MRIcropAroundRAS: the array inverted when the volume is its source (FreeSurfer decides it backwards)",
                    );
                }
            }
            let (choice, (is_src, is_dst)) = if reproduce {
                (rep, (rs, rd))
            } else {
                (fix, (fs, fd))
            };
            let Some(invert) = choice else {
                self.put_out(
                    format!(
                        "ERROR: MRIcropAroundRAS(): vol is not src or dst of LTA ({}, {})\n",
                        i32::from(is_src),
                        i32::from(is_dst)
                    )
                    .as_bytes(),
                );
                self.lta_print(lta)?;
                let mut b = b"vg_isEqual_Threshold = ".to_vec();
                b.extend_from_slice(&g(VG_THRESHOLD));
                b.push(b'\n');
                self.put_out(&b);
                return Ok(None);
            };
            let mut l = match self.lta_copy(lta, None)? {
                Returned::New(x) => x,
                _ => return Ok(None),
            };
            if invert {
                self.put_out(b"Inverting LTA\n");
                self.lta_invert(&mut l, LtaInto::Itself)?;
            }
            if l.kind != lta_type::LINEAR_RAS_TO_RAS {
                self.put_out(b"Chaning Source LTA type to ras2ras\n");
                self.lta_change_type(&mut l, lta_type::LINEAR_RAS_TO_RAS)?;
            }
            let m = l.xforms.first().and_then(|x| x.m_l.clone());
            self.put_out(b"Input RAS2VOX\n");
            self.matrix_print(Some(false), ras2vox.as_ref(), 5);
            self.put_out(b"LTA RAS2RAS\n");
            self.matrix_print(Some(false), m.as_ref(), 5);
            ras2vox = match self.matrix_multiply(ras2vox.as_ref(), m.as_ref(), None)? {
                Returned::New(x) => Some(x),
                _ => None,
            };
        }
        self.put_out(b"final ras2vox matrix\n");
        self.matrix_print(Some(false), ras2vox.as_ref(), 5);
        self.put_out(b"--------------------------\n");
        let mut p = Matrix::zeros(4, 1, MATRIX_REAL);
        for (k, &x) in ras.iter().enumerate() {
            p.set(k as i32 + 1, 1, sse::narrow(x));
        }
        p.set(4, 1, 1.0);
        let crs = match self.matrix_multiply(ras2vox.as_ref(), Some(&p), None)? {
            Returned::New(x) => [x.at(1, 1), x.at(2, 1), x.at(3, 1)],
            _ => return Ok(None),
        };
        // each voxel axis takes the field of view of the RAS axis its letter names
        let mut ostr = [0u8; 4];
        self.dircos_to_orientation_string(&vol.geom, &mut ostr);
        let axis_of = |ch: u8| match ch {
            b'L' | b'R' => Some(0),
            b'A' | b'P' => Some(1),
            b'S' | b'I' => Some(2),
            _ => None,
        };
        // as FreeSurfer fills them (observed: vcrops): for each RAS axis k, the first voxel axis
        // c whose letter names it gives entry k the field of view of entry c
        let mut fov: [Option<i32>; 3] = [None; 3];
        for (k, slot) in fov.iter_mut().enumerate() {
            if let Some(c) = (0..3).find(|&c| axis_of(ostr[c]) == Some(k)) {
                *slot = Some(fov_ras[c]);
            }
        }
        let name = String::from_utf8_lossy(&ostr[..3]).into_owned();
        let fov = if fov.iter().all(Option::is_some) {
            fov.map(|f| f.unwrap_or(0))
        } else if reproduce {
            self.note(
                &FS_515,
                format!("MRIcropAroundRAS: the orientation {name} names an axis twice or is unknown: the field of view never set taken as 0 (FreeSurfer's memory contents)"),
            );
            fov.map(|f| f.unwrap_or(0))
        } else {
            self.note(
                &FS_515,
                format!("MRIcropAroundRAS: the orientation {name} names an axis twice or is unknown: the program ended (FreeSurfer crops to a field of view never set)"),
            );
            self.error_exit(
                codes::BAD_PARM,
                format!("MRIcropAroundRAS(): the orientation {name} names an axis twice or is unknown (FreeSurfer crops to a field of view never set here)\n").as_bytes(),
            )?;
            return Ok(None);
        };
        self.put_out(
            format!(
                "ostr {name} ({} {} {}) ({} {} {})\n",
                fov_ras[0], fov_ras[1], fov_ras[2], fov[0], fov[1], fov[2]
            )
            .as_bytes(),
        );
        let mut b = b"rasCenter ".to_vec();
        b.extend_from_slice(&g3(ras, b" "));
        b.extend_from_slice(b"\ncrsCenter ");
        let crs = crs.map(sse::widen);
        b.extend_from_slice(&g3(crs, b" "));
        b.push(b'\n');
        self.put_out(&b);
        self.mri_crop_around_crs(vol, crs, fov, keep)
    }
}
