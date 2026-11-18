// SPDX-License-Identifier: Apache-2.0
//! Moving a volume (spec §19): translations, rotations by a matrix about a centre and rotations
//! about an axis; FS-301, FS-303 and FS-304.

use larmorx_core::defects::{FS_301, FS_303, FS_304};

use super::{Dst, VolOut, Vols};
use crate::maths::Returned;
use crate::maths::matrix::{MATRIX_REAL, Matrix};
use crate::mri::{Mri, types};
use crate::process::{Process, Stop};
use crate::sse;

/// The rotation axes of `MatrixAllocRotation` (`specs/freesurfer-maths.md` §8.4).
pub mod axis {
    pub const X: i32 = 0;
    pub const Y: i32 = 1;
    pub const Z: i32 = 2;
}

/// The types the trilinear sampler reads (§18).
fn sampled(kind: i32) -> bool {
    matches!(
        kind,
        types::UCHAR | types::SHORT | types::USHRT | types::INT | types::LONG | types::FLOAT
    )
}

impl Process {
    /// The destination of a move (§19): without one a clone of the source, else the destination
    /// cleared (`MRIclear`, its errors included).
    fn move_dst(&mut self, v: &mut Vols) -> Result<(), Stop> {
        if v.has_dst() {
            self.mri_clear(v.d_mut())?;
        } else {
            let d = self.mri_clone_new(v.a())?;
            v.set_dst(d);
        }
        Ok(())
    }

    /// One sample stored in the destination's voxel (x, y, z) of frame 0 (§19): as `(unsigned
    /// char) nint(val)` at the voxel's byte place, which is the voxel of a byte volume. FS-301:
    /// for a destination of another type `reproduce` writes that byte there; by default the
    /// sample is written as the volume's type stores it (§5's write), into a voxel of its extent.
    fn move_store(&mut self, d: &mut Mri, x: i32, y: i32, z: i32, val: f64) -> Result<(), Stop> {
        let what = "MRItranslate";
        if d.kind == types::UCHAR || self.mode().reproduce() {
            if d.kind != types::UCHAR {
                self.note(
                    &FS_301,
                    format!(
                        "a moved volume of type {}: each sample's byte written at the voxel's byte place",
                        d.kind
                    ),
                );
            }
            let b = self.nint(val) as u8;
            let place = self.row_place(d, i64::from(x), i64::from(y), i64::from(z), 0, 1);
            self.direct_needs_voxels(d, what)?;
            self.direct_write(d, place, &[b], what);
            return Ok(());
        }
        self.note(
            &FS_301,
            format!(
                "a moved volume of type {}: each sample written as its type stores it (FreeSurfer writes a byte at the voxel's byte place)",
                d.kind
            ),
        );
        // the destination's own write (fs.mri §5's MRIsetVoxVal: its rules outside the extent)
        self.mri_set_vox_val(d, x, y, z, 0, sse::narrow(val), 0)?;
        Ok(())
    }

    /// The trilinear sample of a move (§19): FS-303 for a source type the sampler refuses (the
    /// sample keeps its previous value, 0 at first).
    #[allow(clippy::too_many_arguments)]
    fn move_sample(
        &mut self,
        src: &Mri,
        x: f64,
        y: f64,
        z: f64,
        val: &mut f64,
        first: &mut bool,
    ) -> Result<(), Stop> {
        self.mri_sample_volume(src, x, y, z, val)?;
        if *first && !sampled(src.kind) {
            *first = false;
            self.note(
                &FS_303,
                format!(
                    "a moved volume of type {} the trilinear sampler refuses: a sample it never set stored as the one before (0 at first; FreeSurfer's first is memory contents)",
                    src.kind
                ),
            );
        }
        Ok(())
    }

    /// `MRItranslate(src, dst, dx, dy, dz)` (§19): each voxel whose point moved back by the shift
    /// lies inside the source gets the trilinear sample there; the destination's
    /// `ras_good_flag` becomes 0.
    pub fn mri_translate(
        &mut self,
        src: &mut Mri,
        dst: Dst,
        dx: f64,
        dy: f64,
        dz: f64,
    ) -> Result<VolOut, Stop> {
        let mut v = Vols::new(src, None, dst);
        self.move_dst(&mut v)?;
        let g = v.a().geom.clone();
        let mut val = 0.0f64;
        let mut first = true;
        for z in 0..g.depth {
            for y in 0..g.height {
                for x in 0..g.width {
                    let p = [
                        sse::sub64(f64::from(x), dx),
                        sse::sub64(f64::from(y), dy),
                        sse::sub64(f64::from(z), dz),
                    ];
                    let e = [g.width, g.height, g.depth];
                    if (0..3).all(|k| p[k] >= 0.0 && p[k] < f64::from(e[k])) {
                        self.move_sample(v.a(), p[0], p[1], p[2], &mut val, &mut first)?;
                        self.move_store(v.d_mut(), x, y, z, val)?;
                    }
                }
            }
        }
        v.d_mut().geom.ras_good_flag = 0;
        Ok(v.out())
    }

    /// `MRIrotate_I(src, dst, R, O)` (§19): each voxel gets the trilinear sample at `R^-1 (v -
    /// io) + O` (in float), `O` the centre (the source's half extents without one), `io` its
    /// truncation; `NULL` when `R` has no inverse. The destination's `ras_good_flag` becomes 0.
    pub fn mri_rotate_i(
        &mut self,
        src: &mut Mri,
        dst: Dst,
        r: Option<&Matrix>,
        o: Option<&Matrix>,
    ) -> Result<VolOut, Stop> {
        let inv = match self.matrix_inverse(r, None)? {
            Returned::New(m) => m,
            _ => {
                self.error_printf(
                    crate::process::codes::BAD_PARM,
                    b"MRIrotate_I: rotation matrix is singular",
                );
                return Ok(VolOut::Nothing);
            }
        };
        let mut v = Vols::new(src, None, dst);
        self.move_dst(&mut v)?;
        let g = v.a().geom.clone();
        let centre = match o {
            Some(o) => o.clone(),
            None => {
                let mut c = Matrix::zeros(3, 1, MATRIX_REAL);
                c.set(1, 1, g.width as f32 / 2.0);
                c.set(2, 1, g.height as f32 / 2.0);
                c.set(3, 1, g.depth as f32 / 2.0);
                c
            }
        };
        let io = [
            cvt_f32(centre.at(1, 1)),
            cvt_f32(centre.at(2, 1)),
            cvt_f32(centre.at(3, 1)),
        ];
        if self.gdiag == 99 {
            self.matrix_print(Some(false), Some(&inv), 5);
            self.matrix_print(Some(false), Some(&centre), 5);
        }
        // the offset Y and the point X: 3 x 1 matrices made once; X = R^-1 Y, then X = X + O in
        // place. A product or a sum that refuses its sizes (an error per voxel) leaves X as it
        // was: an R of another size adds O to the previous voxel's point, an O of another size
        // leaves the point R^-1 Y (observed: the probe's `vmove`, `vrules4`)
        let mut mv = Matrix::zeros(3, 1, MATRIX_REAL);
        let mut p = Matrix::zeros(3, 1, MATRIX_REAL);
        let mut val = 0.0f64;
        let mut first = true;
        for z in 0..g.depth {
            for y in 0..g.height {
                for x in 0..g.width {
                    mv.set(1, 1, x.wrapping_sub(io[0]) as f32);
                    mv.set(2, 1, y.wrapping_sub(io[1]) as f32);
                    mv.set(3, 1, z.wrapping_sub(io[2]) as f32);
                    self.matrix_multiply(Some(&inv), Some(&mv), Some(&mut p))?;
                    self.matrix_add_sub_in_place(&mut p, &centre, false, true)?;
                    let (px, py, pz) = (
                        sse::widen(p.at(1, 1)),
                        sse::widen(p.at(2, 1)),
                        sse::widen(p.at(3, 1)),
                    );
                    self.move_sample(v.a(), px, py, pz, &mut val, &mut first)?;
                    self.move_store(v.d_mut(), x, y, z, val)?;
                }
            }
        }
        // observed (the probe's `vmove`): the destination's `ras_good_flag` becomes 0
        v.d_mut().geom.ras_good_flag = 0;
        Ok(v.out())
    }

    /// `MRIrotateX_I`, `MRIrotateY_I`, `MRIrotateZ_I(src, dst, angle)` (§19): a given destination
    /// is cleared; the result is a new volume, `MRIrotate_I` by `MatrixAllocRotation(3, angle,
    /// axis)` about the source's centre, `ras_good_flag` 0. FS-304: an angle whose rotation has no
    /// inverse ends the program after `MRIrotate_I`'s errors (FreeSurfer crashes).
    pub fn mri_rotate_axis(
        &mut self,
        src: &mut Mri,
        dst: Dst,
        angle: f32,
        which: i32,
    ) -> Result<VolOut, Stop> {
        match dst {
            Dst::Given(d) => {
                self.mri_clear(d)?;
            }
            Dst::Src => {
                self.mri_clear(src)?;
            }
            _ => {}
        }
        let Returned::New(m) = self.matrix_rotation(3, angle, which, None)? else {
            return Ok(VolOut::Nothing);
        };
        match self.mri_rotate_i(src, Dst::New, Some(&m), None)? {
            VolOut::New(mut r) => {
                r.geom.ras_good_flag = 0;
                Ok(VolOut::New(r))
            }
            _ => {
                // FS-304, both modes: the end of the program (FreeSurfer crashes)
                let name = match which {
                    axis::X => "MRIrotateX_I",
                    axis::Y => "MRIrotateY_I",
                    _ => "MRIrotateZ_I",
                };
                self.note(
                    &FS_304,
                    "a rotation about an axis that has no inverse ends the program (FreeSurfer crashes)",
                );
                self.error_exit(
                    crate::process::codes::BAD_PARM,
                    format!("{name}(): no rotated volume (FreeSurfer crashes here)\n").as_bytes(),
                )?;
                Ok(VolOut::Nothing)
            }
        }
    }
}

/// C's `(int)` of a float on x86-64 (`cvttss2si`): truncation; NaN and values outside `int`
/// give -2147483648.
fn cvt_f32(c: f32) -> i32 {
    if c > -2_147_483_904.0 && c < 2_147_483_648.0 {
        c.trunc() as i32
    } else {
        i32::MIN
    }
}
