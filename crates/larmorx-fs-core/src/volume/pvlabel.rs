// SPDX-License-Identifier: Apache-2.0
//! The partial-volume count of a label (spec §37, part 8):
//! `MRIvoxelsInLabelWithPartialVolumeEffects`, with FS-469, FS-470 and FS-471.

use larmorx_core::defects::{FS_469, FS_470, FS_471};

use super::{Dst, VolOut};
use crate::mri::Mri;
use crate::process::{Process, Stop, codes};
use crate::sse;

/// The stack FreeSurfer's per-voxel tables live on (Linux's default, 8 MiB): its three tables of
/// `maxlabels` entries (two counts and the means, 4 bytes each) pass it from about 699,000
/// labels (FS-471).
const STACK_BYTES: u64 = 8 << 20;

/// One border voxel's label tables (§37): the small window's counts, the wide window's counts and
/// means.
struct Tables {
    small: Vec<i32>,
    wide: Vec<i32>,
    means: Vec<f32>,
}

impl Process {
    /// `MRIvoxelsInLabelWithPartialVolumeEffects(mri, vals, label, mixing, nbr_labels, maxlabels,
    /// segborder)` (§37): the volume of `label`, each border voxel weighed by its intensity
    /// between its own label's mean and its neighbour label's (the wide window's means), into
    /// `mixing` and `nbr_labels` when given. `label >= maxlabels`: two lines and -100000.
    ///
    /// FS-471 (a negative `maxlabels`): the first counted voxel on the border, or the first
    /// counted voxel at all from -4 down, ends the program (status 252); -1 to -3 count the voxels
    /// off the border as usual. FS-470: no `vals` ends it at the first border voxel weighed,
    /// after its two windows. The border volume written to `b.mgz` under `Gdiag`'s verbose and
    /// write bits waits for a volume writer (fs.io's clean room, §37): not yet made.
    #[allow(clippy::too_many_arguments)]
    pub fn mri_voxels_in_label_with_partial_volume_effects(
        &mut self,
        mri: &mut Mri,
        vals: Option<&Mri>,
        label: i32,
        mut mixing: Option<&mut Mri>,
        mut nbr_labels: Option<&mut Mri>,
        max_labels: i32,
        segborder: Option<&Mri>,
    ) -> Result<f32, Stop> {
        if label >= max_labels {
            self.put_out(
                format!(
                    "ERROR: MRIvoxelsInLabelWithPartialVolumeEffects()\n label {label} exceeds maximum label number {max_labels}\n"
                )
                .as_bytes(),
            );
            return Ok(-100000.0);
        }
        let g = mri.geom.clone();
        let vox_vol = sse::mul32(sse::mul32(g.xsize, g.ysize), g.zsize);
        let made;
        let border: &Mri = match segborder {
            Some(b) => b,
            None => {
                made = match self.mri_mark_label_border_voxels(mri, Dst::New, label, 1, true)? {
                    VolOut::New(m) => m,
                    _ => return Ok(0.0),
                };
                &made
            }
        };
        let n = max_labels.max(0) as usize;
        let mut t = Tables {
            small: vec![0; n],
            wide: vec![0; n],
            means: vec![0.0; n],
        };
        let mut volume = 0.0f32;
        let mut first_counted = true;
        for x in 0..g.width {
            for y in 0..g.height {
                for z in 0..g.depth {
                    let p = (x, y, z);
                    let v = self.v0(mri, p)?;
                    let own = self.int_fs12(sse::widen(v));
                    let b = self.v0(border, p)?;
                    let on_border = self.int_fs12(sse::widen(b)) != 0;
                    if own != label && !on_border {
                        continue;
                    }
                    if let Some(sb) = segborder
                        && on_border
                        && own != label
                    {
                        let mut hit = false;
                        for f in 1..sb.nframes {
                            if self.vf(sb, p, f)? == label as f32 {
                                hit = true;
                                break;
                            }
                        }
                        if !hit {
                            continue;
                        }
                    }
                    // a counted voxel: FS-471's ends and its note of tables past the stack
                    if max_labels < 0 && (on_border || max_labels <= -4) {
                        self.note(
                            &FS_471,
                            "the partial-volume count with a negative label limit refused where FreeSurfer crashes",
                        );
                        self.error_exit(
                            codes::BAD_PARM,
                            format!(
                                "MRIvoxelsInLabelWithPartialVolumeEffects(): a negative maximum label number {max_labels} (FreeSurfer crashes here)\n"
                            )
                            .as_bytes(),
                        )?;
                        return Ok(volume);
                    }
                    if first_counted && max_labels > 0 && (max_labels as u64) * 12 > STACK_BYTES {
                        self.note(
                            &FS_471,
                            "the partial-volume count's tables of more labels than FreeSurfer's stack holds counted all the same (FreeSurfer crashes)",
                        );
                    }
                    first_counted = false;
                    if !on_border {
                        volume = sse::add32(volume, vox_vol);
                        continue;
                    }
                    volume = self.pv_border_voxel(
                        mri,
                        vals,
                        p,
                        own,
                        label,
                        max_labels,
                        &mut t,
                        (vox_vol, volume),
                        (mixing.as_deref_mut(), nbr_labels.as_deref_mut()),
                    )?;
                }
            }
        }
        Ok(volume)
    }

    /// One border voxel of §37: its small and wide windows' tables, its neighbour label, then
    /// its weight added to the volume (returned) and written into the mixing and neighbour
    /// volumes.
    #[allow(clippy::too_many_arguments)]
    fn pv_border_voxel(
        &mut self,
        mri: &Mri,
        vals: Option<&Mri>,
        p: (i32, i32, i32),
        own: i32,
        label: i32,
        max_labels: i32,
        t: &mut Tables,
        (vox_vol, volume): (f32, f32),
        (mixing, nbr_labels): (Option<&mut Mri>, Option<&mut Mri>),
    ) -> Result<f32, Stop> {
        self.mri_compute_label_nbhd(mri, None, p, &mut t.small, None, 1)?;
        self.mri_compute_label_nbhd(mri, vals, p, &mut t.wide, Some(&mut t.means), 7)?;
        let (x, y, z) = p;
        let Some(vals) = vals else {
            self.note(
                &FS_470,
                "the partial-volume count without an intensity volume refused at its first border voxel weighed (FreeSurfer crashes)",
            );
            self.error_exit(
                codes::BAD_PARM,
                format!(
                    "MRIvoxelsInLabelWithPartialVolumeEffects(): no intensity volume for the border voxel ({x}, {y}, {z}) (FreeSurfer crashes here)\n"
                )
                .as_bytes(),
            )?;
            return Ok(volume);
        };
        let val = self.v0(vals, p)?;
        let inside = |l: i32| l >= 0 && l < max_labels;
        let m0 = if inside(own) {
            t.means[own as usize]
        } else {
            self.note(
                &FS_469,
                "the border voxel's own label outside the label table weighed with its mean over the wide window (FreeSurfer reads it outside its table)",
            );
            self.pv_own_mean(mri, vals, p, own)?
        };
        // the neighbour label: of the small window's labels in ascending order but its own, the
        // first of the largest wide count whose mean lies across the voxel's value from m0
        let (mut nbr, mut best) = (-1i32, 0i32);
        for l in 0..max_labels {
            let k = l as usize;
            if l == own || t.small[k] == 0 {
                continue;
            }
            let m = t.means[k];
            let across = sse::mul32(sse::sub32(m, val), sse::sub32(m0, val)) < 0.0;
            if across && t.wide[k] > best {
                nbr = l;
                best = t.wide[k];
            }
        }
        if own != label && nbr != label {
            return Ok(volume);
        }
        if nbr < 0 {
            let volume = sse::add32(volume, vox_vol);
            if let Some(nl) = nbr_labels {
                let (mut first, mut most) = (-1i32, 0i32);
                for l in 0..max_labels {
                    let k = l as usize;
                    if l != own && t.small[k] > 0 && t.wide[k] > most {
                        first = l;
                        most = t.wide[k];
                    }
                }
                self.mri_set_vox_val(nl, x, y, z, 0, first as f32, 0)?;
                if let Some(mx) = mixing {
                    self.mri_set_vox_val(mx, x, y, z, 0, 1.0, 0)?;
                }
            }
            return Ok(volume);
        }
        let m_nbr = t.means[nbr as usize];
        let mut pv = sse::div32(sse::sub32(val, m_nbr), sse::sub32(m0, m_nbr));
        if pv > 1.0 {
            pv = 1.0;
        }
        if pv < 0.0 {
            return Ok(volume);
        }
        if own != label {
            pv = sse::sub32(1.0, pv);
        }
        let volume = sse::add32(sse::mul32(vox_vol, pv), volume);
        if let Some(mx) = mixing {
            self.mri_set_vox_val(mx, x, y, z, 0, pv, 0)?;
        }
        if let Some(nl) = nbr_labels {
            let l = if own != label { own } else { nbr };
            self.mri_set_vox_val(nl, x, y, z, 0, l as f32, 0)?;
        }
        Ok(volume)
    }

    /// FS-469's default: the mean of the wide window's values of a label outside the table (the
    /// window as `MRIcomputeLabelNbhd` walks it, each sum `v + sum`).
    fn pv_own_mean(
        &mut self,
        mri: &Mri,
        vals: &Mri,
        p: (i32, i32, i32),
        own: i32,
    ) -> Result<f32, Stop> {
        let (mut sum, mut n) = (0.0f32, 0i32);
        for q in self.window_xyz(mri, 7, p, "MRIcomputeLabelNbhd")? {
            let v = self.v0(mri, q)?;
            if self.int_fs12(sse::widen(v)) == own {
                let x = self.v0(vals, q)?;
                sum = sse::add32(x, sum);
                n += 1;
            }
        }
        Ok(if n > 0 {
            sse::div32(sum, n as f32)
        } else {
            0.0
        })
    }
}
