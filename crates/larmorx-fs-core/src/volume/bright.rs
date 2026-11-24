// SPDX-License-Identifier: Apache-2.0
//! Bright voxels outside the white matter (spec §59): `MRIfindBrightNonWM` (FS-802, FS-1233).

use larmorx_core::defects::{FS_802, FS_1233};

use super::setops::ext;
use super::{Dst, VolOut};
use crate::mri::{Mri, types};
use crate::process::{Process, Stop};

/// The label of a bright voxel outside the white matter.
const BRIGHT_LABEL: u8 = 130;
/// The label of a voxel the first dilation adds.
const BRIGHT_BORDER_LABEL: f32 = 100.0;

/// The low byte of a value's truncation to a C `int` (x86-64's conversion: NaN and values
/// beyond the `int` range give -2147483648, whose low byte is 0).
fn low_byte_of_trunc(v: f32) -> u8 {
    let x = f64::from(v);
    let i = if x.is_nan() || x >= 2147483648.0 || x <= -2147483649.0 {
        i32::MIN
    } else {
        x.trunc() as i32
    };
    i as u8
}

impl Process {
    /// `MRIfindBrightNonWM(T1, wm)` (§59): the label volume (0, 100 and 130) of the bright
    /// voxels outside the white matter; FS-1233 for a T1 or white matter not of bytes.
    pub fn mri_find_bright_non_wm(&mut self, t1: &mut Mri, wm: &mut Mri) -> Result<Mri, Stop> {
        let bytes = t1.kind == types::UCHAR && wm.kind == types::UCHAR;
        if bytes {
            return self.bright_steps(t1, wm);
        }
        if self.mode().reproduce() {
            self.note(
                &FS_1233,
                "MRIfindBrightNonWM: a T1 or white matter not of bytes read, and its labels written, as the bytes of the volumes' rows",
            );
            return self.bright_steps(t1, wm);
        }
        self.note(
            &FS_1233,
            "MRIfindBrightNonWM: a T1 or white matter not of bytes taken as byte volumes, the labels written by value in the T1's type",
        );
        let mut t1b = if t1.kind == types::UCHAR {
            None
        } else {
            Some(self.bright_bytes(t1, true)?)
        };
        let mut wmb = if wm.kind == types::UCHAR {
            None
        } else {
            Some(self.bright_bytes(wm, false)?)
        };
        let label = {
            let t: &mut Mri = match t1b.as_mut() {
                Some(m) => m,
                None => &mut *t1,
            };
            let w: &mut Mri = match wmb.as_mut() {
                Some(m) => m,
                None => &mut *wm,
            };
            self.bright_steps(t, w)?
        };
        if t1b.is_none() {
            return Ok(label);
        }
        let mut out = self.mri_clone_new(t1)?;
        let (w, h, d) = ext(&label);
        for z in 0..d {
            for y in 0..h {
                for x in 0..w {
                    let b = self.byte_at(&label, x, y, z, "MRIfindBrightNonWM");
                    if b != 0 {
                        self.mri_set_vox_val(&mut out, x, y, z, 0, f32::from(b), 0)?;
                    }
                }
            }
        }
        Ok(out)
    }

    /// FS-1233's byte volume of a T1 (`t1`: FS-802's default, noted for each value it changes)
    /// or of a white matter (truncated and clamped to 0 to 255): frame 0's values.
    fn bright_bytes(&mut self, m: &Mri, t1: bool) -> Result<Mri, Stop> {
        let mut b = self.mri_clone_different_type(m, types::UCHAR)?;
        let (w, h, d) = ext(m);
        for z in 0..d {
            for y in 0..h {
                for x in 0..w {
                    let v = self.mri_get_vox_val(m, x, y, z, 0, 0)?;
                    let o = if t1 {
                        if v.is_nan() || v < 0.0 || v >= 256.0 {
                            self.note(
                                &FS_802,
                                "MRIfindBrightNonWM: a T1 value beyond 0 to 255 or not a number taken clamped (FreeSurfer takes the low byte of its truncation)",
                            );
                        }
                        if v.is_nan() || v < 0.0 {
                            0.0
                        } else if v >= 256.0 {
                            255.0
                        } else {
                            v.trunc()
                        }
                    } else if v.is_nan() || v <= 0.0 {
                        0.0
                    } else if v >= 255.0 {
                        255.0
                    } else {
                        v.trunc()
                    };
                    self.mri_set_vox_val(&mut b, x, y, z, 0, o, 0)?;
                }
            }
        }
        Ok(b)
    }

    /// FreeSurfer's steps of `MRIfindBrightNonWM` on the volumes as they are (§59).
    fn bright_steps(&mut self, t1: &mut Mri, wm: &mut Mri) -> Result<Mri, Stop> {
        let name = "MRIfindBrightNonWM";
        let mut label = self.mri_clone_new(t1)?;
        let (w, h, d) = ext(t1);
        let reproduce = self.mode().reproduce();
        for z in 0..d {
            for y in 0..h {
                for x in 0..w {
                    let wv = self.byte_at(wm, x, y, z, name);
                    if wv >= 5 {
                        continue;
                    }
                    let v = self.mri_get_vox_val(t1, x, y, z, 0, 0)?;
                    let tb = if v.is_nan() || v < 0.0 || v >= 256.0 {
                        if reproduce {
                            self.note(
                                &FS_802,
                                "MRIfindBrightNonWM: a T1 value beyond 0 to 255 or not a number taken as the low byte of its truncation",
                            );
                            low_byte_of_trunc(v)
                        } else {
                            self.note(
                                &FS_802,
                                "MRIfindBrightNonWM: a T1 value beyond 0 to 255 or not a number taken clamped (FreeSurfer takes the low byte of its truncation)",
                            );
                            if v.is_nan() || v < 0.0 { 0 } else { 255 }
                        }
                    } else {
                        v.trunc() as u8
                    };
                    if tb <= 125 {
                        continue;
                    }
                    let mut n = 0;
                    for dz in -1..=1i32 {
                        for dy in -1..=1i32 {
                            for dx in -1..=1i32 {
                                let xi = (x + dx).min(w - 1).max(0);
                                let yi = (y + dy).min(h - 1).max(0);
                                let zi = (z + dz).min(d - 1).max(0);
                                if self.byte_at(wm, xi, yi, zi, name) >= 5 {
                                    n += 1;
                                }
                            }
                        }
                    }
                    if n < 13 {
                        self.byte_put(&mut label, x, y, z, BRIGHT_LABEL, name);
                    }
                }
            }
        }
        self.mri_close(&mut label, Dst::Src)?;
        let mut dil = match self.mri_dilate_thresh_label(
            &mut label,
            t1,
            Dst::New,
            i32::from(BRIGHT_LABEL),
            1,
            100,
        )? {
            VolOut::New(m) => m,
            _ => return Ok(label),
        };
        self.mri_xor(&mut label, &mut dil, Dst::Other, 1, 255)?;
        self.mri_replace_values(&mut dil, Dst::Src, 1.0, BRIGHT_BORDER_LABEL)?;
        self.mri_union(&mut label, &mut dil, Dst::Src)?;
        let n = self.mri_voxels_in_label(&label, i32::from(BRIGHT_LABEL))?;
        self.put_out(
            format!("MRIfindBrightNonWM(): {n} bright non-wm voxels segmented.\n").as_bytes(),
        );
        self.mri_dilate_inv_thresh_label(
            &mut label,
            t1,
            Dst::Src,
            i32::from(BRIGHT_LABEL),
            3,
            0,
        )?;
        Ok(label)
    }
}
