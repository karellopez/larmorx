// SPDX-License-Identifier: Apache-2.0
//! Conforming (spec §49): `MRIgetConformMatrix`, `MRIconform` and `MRIconformedTemplate`
//! (FS-491, FS-492).

use larmorx_core::defects::{FS_491, FS_492};

use super::sample::sample_type;
use crate::clib::printf::sprintf_f64;
use crate::maths::Returned;
use crate::maths::matrix::{MATRIX_REAL, Matrix};
use crate::mri::{Mri, types};
use crate::process::{Process, Stop, codes};
use crate::sse;

/// The coronal cosines and centre 0 (§49): those twelve fields only.
fn coronal(m: &mut Mri) {
    let g = &mut m.geom;
    g.x = [-1.0, 0.0, 0.0];
    g.y = [0.0, 0.0, -1.0];
    g.z = [0.0, 1.0, 0.0];
    g.c = [0.0, 0.0, 0.0];
}

impl Process {
    /// The conformed grid (§49): a header-only byte volume of 256^3 voxels and `nframes` frames,
    /// voxel sizes 1, coronal cosines, a good RAS, the centre `c` and `pulse`'s pulse parameters.
    fn conformed_grid(&mut self, nframes: i32, c: [f32; 3], pulse: &Mri) -> Result<Mri, Stop> {
        let mut t = self.mri_alloc_header(256, 256, 256, types::UCHAR, nframes)?;
        t.imnr0 = 1;
        t.imnr1 = 256;
        t.thick = 1.0;
        t.ps = 1.0;
        t.geom.xsize = 1.0;
        t.geom.ysize = 1.0;
        t.geom.zsize = 1.0;
        (t.xstart, t.ystart, t.zstart) = (-128.0, -128.0, -128.0);
        (t.xend, t.yend, t.zend) = (128.0, 128.0, 128.0);
        coronal(&mut t);
        t.geom.ras_good_flag = 1;
        t.geom.c = c;
        self.copy_pulse_parameters(pulse, &mut t);
        Ok(t)
    }

    /// `MRIgetConformMatrix(mri)` (§49): a volume without a good RAS first gets the coronal
    /// cosines and centre 0 (in place); then `MRIgetResampleMatrix(mri, grid)`.
    pub fn mri_get_conform_matrix(&mut self, m: &mut Mri) -> Result<Option<Matrix>, Stop> {
        if m.geom.ras_good_flag == 0 {
            coronal(m);
        }
        let grid = self.conformed_grid(m.nframes, m.geom.c, m)?;
        self.mri_get_resample_matrix(m, &grid)
    }

    /// `MRIconform(mri)` (§49): a copy (the coronal cosines and centre 0 without a good RAS),
    /// changed to bytes when it is not (`MRIchangeType(copy, MRI_UCHAR, 0.0, 0.999, 0)`), then
    /// resampled onto the conformed grid (trilinear) unless its sizes, extent and cosines are
    /// the grid's. The input is never changed.
    pub fn mri_conform(&mut self, m: &Mri) -> Result<Option<Mri>, Stop> {
        let mut copy = match self.mri_copy(m, None)? {
            Returned::New(c) => c,
            _ => return Ok(None),
        };
        if copy.geom.ras_good_flag == 0 {
            coronal(&mut copy);
        }
        let grid = self.conformed_grid(copy.nframes, copy.geom.c, m)?;
        if copy.kind != types::UCHAR {
            if !copy.has_voxels() {
                self.no_voxels("MRIchangeType")?;
            }
            copy = match self.mri_change_type(&copy, types::UCHAR, 0.0, 0.999, 0)? {
                Some(c) => c,
                None => return Ok(None),
            };
        }
        let (g, t) = (&copy.geom, &grid.geom);
        let same = g.xsize == t.xsize
            && g.ysize == t.ysize
            && g.zsize == t.zsize
            && g.width == t.width
            && g.height == t.height
            && g.depth == t.depth
            && g.x == t.x
            && g.y == t.y
            && g.z == t.z;
        if same {
            return Ok(Some(copy));
        }
        if !copy.has_voxels() {
            self.no_voxels("MRIresample")?;
        }
        self.mri_resample(&copy, &grid, sample_type::TRILINEAR)
    }

    /// `MRIconformedTemplate(mri, conform_width, conform_size, KeepDC)` (§49): a header-only byte
    /// volume of `conform_width`^3 voxels with the input's header and pulse parameters, voxel
    /// sizes `conform_size`; the coronal cosines without `KeepDC`, else the input's axes kept
    /// through the matrix `K` (FS-491: an absent axis; FS-492: fields of view beyond `int`).
    pub fn mri_conformed_template(
        &mut self,
        m: &Mri,
        conform_width: i32,
        conform_size: f64,
        keep_dc: bool,
    ) -> Result<Option<Mri>, Stop> {
        let cw = conform_width;
        let mut t = self.mri_alloc_header(cw, cw, cw, types::UCHAR, m.nframes)?;
        self.copy_header_into(m, &mut t)?;
        self.copy_pulse_parameters(m, &mut t);
        t.imnr0 = 1;
        t.imnr1 = cw;
        let sz = sse::narrow(conform_size);
        t.thick = sz;
        t.ps = sz;
        t.geom.xsize = sz;
        t.geom.ysize = sz;
        t.geom.zsize = sz;
        let (lo, hi) = ((-cw / 2) as f32, (cw / 2) as f32);
        (t.xstart, t.ystart, t.zstart) = (lo, lo, lo);
        (t.xend, t.yend, t.zend) = (hi, hi, hi);
        if !keep_dc {
            let g = &mut t.geom;
            g.x = [-1.0, 0.0, 0.0];
            g.y = [0.0, 0.0, -1.0];
            g.z = [0.0, 1.0, 0.0];
            return Ok(Some(t));
        }
        let reproduce = self.mode().reproduce();
        let conform_fov = self.fov_int(sse::mul64(f64::from(cw), conform_size));
        let mut ostr = [0u8; 4];
        self.dircos_to_orientation_string(&m.geom, &mut ostr);
        let find = |a: u8, b: u8| ostr[..3].iter().position(|&c| c == a || c == b).unwrap_or(3);
        let idx = [find(b'L', b'R'), find(b'I', b'S'), find(b'A', b'P')];
        let shown: Vec<u8> = ostr.iter().copied().take_while(|&c| c != 0).collect();
        self.put_out(format!("keeping DC {} {} {}\n", idx[0], idx[1], idx[2]).as_bytes());
        let mut line = b"ostr ".to_vec();
        line.extend_from_slice(&shown);
        line.extend_from_slice(format!(", width {cw}, size ").as_bytes());
        line.extend_from_slice(&sprintf_f64(b"%g", conform_size));
        line.push(b'\n');
        self.put_out(&line);
        if idx.contains(&3) {
            if reproduce {
                self.note(
                    &FS_491,
                    "MRIconformedTemplate: an orientation without one of the three axes: FreeSurfer's end (its matrix cannot be inverted)",
                );
            } else {
                self.note(
                    &FS_491,
                    "MRIconformedTemplate: an orientation without one of the three axes refused (FreeSurfer reads past its arrays)",
                );
                let mut msg = b"MRIconformedTemplate: orientation string ".to_vec();
                msg.extend_from_slice(&shown);
                msg.extend_from_slice(
                    b" lacks a left/right, inferior/superior or anterior/posterior axis\n",
                );
                self.error_exit(codes::BAD_PARM, &msg)?;
                return Ok(None);
            }
        }
        let g = &m.geom;
        let nvox = [g.width, g.height, g.depth];
        let delta = [
            sse::widen(g.xsize),
            sse::widen(g.ysize),
            sse::widen(g.zsize),
        ];
        let mut fov = [0i64; 3];
        for c in 0..3 {
            fov[c] = self.fov_int(sse::mul64(f64::from(nvox[c]), delta[c]));
        }
        let mut k = Matrix::zeros(4, 4, MATRIX_REAL);
        k.set(4, 4, 1.0);
        let letters = [b'L', b'I', b'A'];
        for row in 0..3 {
            let i = idx[row];
            // FS-491 with reproduce: the values read past the arrays taken as 0 (they reach no
            // output: the axis' column stays 0 whatever they are)
            let (di, fi) = if i < 3 { (delta[i], fov[i]) } else { (0.0, 0) };
            let step = sse::div64(di, conform_size);
            let diff = if reproduce {
                f64::from((conform_fov as i32).wrapping_sub(fi as i32))
            } else {
                (conform_fov - fi) as f64
            };
            if reproduce && i64::from((conform_fov as i32).wrapping_sub(fi as i32)) != conform_fov - fi {
                self.note(
                    &FS_492,
                    "MRIconformedTemplate: a difference of two fields of view wrapped around in 32 bits, as FreeSurfer's",
                );
            }
            let pad = sse::div64(sse::div64(diff, 2.0), conform_size).round();
            let r = row as i32 + 1;
            let col = i as i32 + 1;
            let letter = if i < 3 { ostr[i] } else { 0 };
            if letter == letters[row] {
                k.set(r, col, sse::narrow(step));
                k.set(r, 4, sse::narrow(pad));
            } else {
                k.set(r, col, sse::narrow(-step));
                k.set(r, 4, sse::narrow(sse::sub64(f64::from(cw), pad)));
            }
        }
        let kinv = match self.matrix_inverse(Some(&k), None)? {
            Returned::New(x) => Some(x),
            _ => None,
        };
        let smri = self.extract_i_to_r(&m.geom)?;
        let stemp = match self.matrix_multiply_d(Some(&smri), kinv.as_ref(), None)? {
            Returned::New(x) => x,
            _ => return Ok(None),
        };
        self.set_vox2ras_from_matrix(&mut t.geom, &stemp)?;
        for (name, mm) in [
            ("K", Some(&k)),
            ("Kinv", kinv.as_ref()),
            ("Smri", Some(&smri)),
            ("Stemp", Some(&stemp)),
        ] {
            self.put_out(format!("{name} ---------------\n").as_bytes());
            self.matrix_print(Some(false), mm, 5);
        }
        self.put_out(b"----------------------\n");
        Ok(Some(t))
    }

    /// `(int) x` of a field of view (§49, FS-492): beyond the `int` range or NaN, -2147483648 with
    /// `reproduce`, else clamped (NaN 0); noted.
    fn fov_int(&mut self, x: f64) -> i64 {
        if !(x.is_nan() || x >= 2147483648.0 || x <= -2147483649.0) {
            return x.trunc() as i64;
        }
        if self.mode().reproduce() {
            self.note(
                &FS_492,
                "MRIconformedTemplate: a field of view beyond the int range (or NaN) gave -2147483648, as FreeSurfer's",
            );
            i64::from(i32::MIN)
        } else {
            self.note(
                &FS_492,
                "MRIconformedTemplate: a field of view beyond the int range clamped (NaN 0)",
            );
            if x.is_nan() {
                0
            } else if x > 0.0 {
                i64::from(i32::MAX)
            } else {
                i64::from(i32::MIN)
            }
        }
    }
}
