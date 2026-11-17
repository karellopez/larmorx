// SPDX-License-Identifier: Apache-2.0
//! The CMA labels and the segmentation functions (spec §23): the label tests of FreeSurfer's
//! colour table's numbers, `IsSubCorticalGray`, `MRIasegContraLatLabel`, the brain volume
//! corrections, the left-right swaps, the ribbon fix, the one-hot encoding and the brain volume
//! statistics' cache; FS-311 to FS-315.
//!
//! The tests take a label of any numeric type as an `f64` (an `int` and a `float` widen to it
//! exactly, so every comparison with the tables' whole numbers is the one made in their own
//! type); a NaN passes none.

use larmorx_core::defects::{FS_311, FS_312, FS_313, FS_314, FS_315};

use super::{Dst, VolOut, Vols};
use crate::clib::printf::sprintf_f64;
use crate::maths::matrix::Matrix;
use crate::mri::{Mri, types};
use crate::process::{Process, Stop, codes};

fn any(l: f64, set: &[i32]) -> bool {
    set.iter().any(|&v| l == f64::from(v))
}

fn within(l: f64, lo: i32, hi: i32) -> bool {
    l >= f64::from(lo) && l <= f64::from(hi)
}

/// `IS_FUTURE_WMSA`.
pub fn is_future_wmsa(l: f64) -> bool {
    any(l, &[87, 88, 89])
}
/// `IS_LH_LAYER`.
pub fn is_lh_layer(l: f64) -> bool {
    within(l, 270, 277)
}
/// `IS_CC`.
pub fn is_cc(l: f64) -> bool {
    within(l, 251, 255)
}
/// `IS_RH_LAYER`.
pub fn is_rh_layer(l: f64) -> bool {
    within(l, 370, 377)
}
/// `IS_LAYER`.
pub fn is_layer(l: f64) -> bool {
    is_lh_layer(l) || is_rh_layer(l)
}
/// `IS_UNKNOWN`.
pub fn is_unknown(l: f64) -> bool {
    any(l, &[0, 255, 190, 191])
}
/// `IS_BRAIN`: (not unknown and below 98) or a corpus callosum part.
pub fn is_brain(l: f64) -> bool {
    (!is_unknown(l) && l < 98.0) || is_cc(l)
}
/// `IS_WM`.
pub fn is_wm(l: f64) -> bool {
    any(l, &[2, 41, 187, 186, 28, 60]) || is_cc(l) || is_cerebellar_wm(l)
}
/// `IS_CEREBRAL_WM`.
pub fn is_cerebral_wm(l: f64) -> bool {
    any(l, &[2, 41])
}
/// `IS_HYPO`, `IS_WMSA`.
pub fn is_hypo(l: f64) -> bool {
    any(l, &[77, 78, 79]) || is_future_wmsa(l)
}
/// `IS_WMH`.
pub fn is_wmh(l: f64) -> bool {
    is_wm(l) || is_hypo(l)
}
/// `IS_THALAMUS`.
pub fn is_thalamus(l: f64) -> bool {
    any(l, &[10, 49])
}
/// `IS_GM`, `IS_CORTEX`.
pub fn is_cortex(l: f64) -> bool {
    any(l, &[3, 42])
}
/// `IS_VENTRAL_DC`.
pub fn is_ventral_dc(l: f64) -> bool {
    any(l, &[28, 60])
}
/// `IS_CEREBELLAR_WM`.
pub fn is_cerebellar_wm(l: f64) -> bool {
    any(l, &[7, 46])
}
/// `IS_CEREBELLAR_GM`.
pub fn is_cerebellar_gm(l: f64) -> bool {
    any(l, &[8, 47])
}
/// `IS_CEREBELLUM`.
pub fn is_cerebellum(l: f64) -> bool {
    is_cerebellar_wm(l) || is_cerebellar_gm(l)
}
/// `IS_CHOROID`.
pub fn is_choroid(l: f64) -> bool {
    any(l, &[31, 63])
}
/// `IS_HIPPO`.
pub fn is_hippo(l: f64) -> bool {
    any(l, &[17, 53])
}
/// `IS_AMYGDALA`.
pub fn is_amygdala(l: f64) -> bool {
    any(l, &[18, 54])
}
/// `IS_MTL`.
pub fn is_mtl(l: f64) -> bool {
    is_hippo(l) || is_amygdala(l)
}
/// `IS_LAT_VENT` (the inferior lateral ventricles included).
pub fn is_lat_vent(l: f64) -> bool {
    any(l, &[4, 43, 44, 5])
}
/// `IS_CSF`.
pub fn is_csf(l: f64) -> bool {
    is_lat_vent(l) || any(l, &[24, 122, 14, 15])
}
/// `IS_INF_LAT_VENT`.
pub fn is_inf_lat_vent(l: f64) -> bool {
    any(l, &[5, 44])
}
/// `IS_VENTRICLE`.
pub fn is_ventricle(l: f64) -> bool {
    is_lat_vent(l) || any(l, &[14, 15])
}
/// `IS_CAUDATE`.
pub fn is_caudate(l: f64) -> bool {
    any(l, &[11, 50])
}
/// `IS_PUTAMEN`.
pub fn is_putamen(l: f64) -> bool {
    any(l, &[12, 51])
}
/// `IS_CLAUSTRUM`.
pub fn is_claustrum(l: f64) -> bool {
    any(l, &[138, 139])
}
/// `IS_ACCUMBENS`.
pub fn is_accumbens(l: f64) -> bool {
    any(l, &[26, 58])
}
/// `IS_PALLIDUM`.
pub fn is_pallidum(l: f64) -> bool {
    any(l, &[13, 52])
}
/// `LABEL_WITH_NO_TOPOLOGY_CONSTRAINT`.
pub fn label_with_no_topology_constraint(l: f64) -> bool {
    any(l, &[77, 78, 79, 80, 81, 82, 31, 63, 98, 118, 120, 122, 129])
}
/// `IS_FIMBRIA`.
pub fn is_fimbria(l: f64) -> bool {
    any(l, &[212, 503, 553])
}
/// `IS_GRAY_MATTER`.
pub fn is_gray_matter(l: f64) -> bool {
    any(
        l,
        &[3, 8, 11, 12, 17, 18, 26, 42, 47, 50, 51, 53, 54, 58],
    )
}
/// `IS_WHITE_MATTER`.
pub fn is_white_matter(l: f64) -> bool {
    any(l, &[2, 7, 41, 46])
}
/// `IS_FLUID`.
pub fn is_fluid(l: f64) -> bool {
    any(l, &[0, 4, 5, 43, 44])
}
/// `IS_LH_CLASS`.
pub fn is_lh_class(l: f64) -> bool {
    any(
        l,
        &[2, 3, 4, 5, 7, 8, 10, 11, 12, 13, 17, 18, 26, 28, 30, 31],
    )
}
/// `IS_RH_CLASS`.
pub fn is_rh_class(l: f64) -> bool {
    any(
        l,
        &[41, 42, 43, 44, 46, 47, 49, 50, 51, 52, 53, 54, 58, 60, 62, 63],
    )
}
/// `IS_GRAY_CLASS`.
pub fn is_gray_class(l: f64) -> bool {
    any(l, &[3, 42, 11, 50])
}
/// `IS_WHITE_CLASS`.
pub fn is_white_class(l: f64) -> bool {
    any(l, &[2, 41])
}
/// `IS_CSF_CLASS`.
pub fn is_csf_class(l: f64) -> bool {
    any(l, &[4, 43, 24, 122, 14, 15, 72, 215, 505, 555])
}

/// The classes of `IS_CLASS`.
pub mod class {
    pub const CSF: i32 = 0;
    pub const GM: i32 = 1;
    pub const WM: i32 = 2;
    pub const OTHER: i32 = 7;
}

/// `IS_CLASS(l, c)`: the CSF class's test for `CSF_CLASS`, the gray class's for `GM_CLASS`, the
/// white class's for any other class.
pub fn is_class(l: f64, c: i32) -> bool {
    match c {
        class::CSF => is_csf_class(l),
        class::GM => is_gray_class(l),
        _ => is_white_class(l),
    }
}

/// Every test of §23's table, then `IS_CLASS` for the classes 0, 1, 2 and 7, in the probe's
/// order (`cmalab`).
pub fn label_tests(l: f64) -> [bool; 46] {
    [
        is_future_wmsa(l),
        is_lh_layer(l),
        is_cc(l),
        is_rh_layer(l),
        is_layer(l),
        is_unknown(l),
        is_brain(l),
        is_wm(l),
        is_cerebral_wm(l),
        is_hypo(l),
        is_hypo(l),
        is_wmh(l),
        is_thalamus(l),
        is_cortex(l),
        is_ventral_dc(l),
        is_cerebellar_wm(l),
        is_cerebellar_gm(l),
        is_cerebellum(l),
        is_choroid(l),
        is_hippo(l),
        is_amygdala(l),
        is_mtl(l),
        is_cortex(l),
        is_lat_vent(l),
        is_csf(l),
        is_inf_lat_vent(l),
        is_ventricle(l),
        is_caudate(l),
        is_putamen(l),
        is_claustrum(l),
        is_accumbens(l),
        is_pallidum(l),
        label_with_no_topology_constraint(l),
        is_fimbria(l),
        is_gray_matter(l),
        is_white_matter(l),
        is_fluid(l),
        is_lh_class(l),
        is_rh_class(l),
        is_gray_class(l),
        is_white_class(l),
        is_csf_class(l),
        is_class(l, class::CSF),
        is_class(l, class::GM),
        is_class(l, class::WM),
        is_class(l, class::OTHER),
    ]
}

/// `IsSubCorticalGray(id)`: 1 for the thalami, caudates, putamens, pallidums, hippocampi,
/// amygdalae, accumbens areas, ventral diencephalons and substantiae nigrae; else 0.
pub fn is_sub_cortical_gray(id: i32) -> i32 {
    i32::from(matches!(
        id,
        10 | 49 | 11 | 50 | 12 | 51 | 13 | 52 | 17 | 53 | 18 | 54 | 26 | 58 | 28 | 60 | 27 | 59
    ))
}

/// `MRIasegContraLatLabel`'s value without its message: `None` for a label below 1000 it does
/// not know (§23).
fn contra(id: i32) -> Option<i32> {
    if id == 0 {
        return Some(0);
    }
    if id > 999 {
        return Some(match id {
            1000..=1998 | 3000..=3998 | 11100..=11998 | 13100..=13998 => id + 1000,
            2000..=2998 | 4000..=4998 | 12100..=12998 | 14100..=14998 => id - 1000,
            5001 => 5002,
            5002 => 5001,
            _ => -1,
        });
    }
    Some(match id {
        1..=13 => id + 39,
        40..=52 => id - 39,
        25..=39 => id + 32,
        57..=71 => id - 32,
        17 => 53,
        53 => 17,
        18 => 54,
        54 => 18,
        19 => 55,
        55 => 19,
        20 => 56,
        56 => 20,
        73 => 74,
        74 => 73,
        78 => 79,
        79 => 78,
        81 => 82,
        82 => 81,
        83 => 84,
        84 => 83,
        96 => 97,
        97 => 96,
        138 => 139,
        139 => 138,
        265 => 266,
        266 => 265,
        819 => 820,
        820 => 819,
        821 => 822,
        822 => 821,
        843 => 844,
        844 => 843,
        865 => 866,
        866 => 865,
        869 => 870,
        870 => 869,
        14 | 15 | 16 | 24 | 72 | 77 | 80 | 85 | 165 | 251..=255 | 257 | 258 | 259 | 260 | 262
        | 502 | 514 | 853 | 883 | 902 | 914 => id,
        _ => return None,
    })
}

/// A voxel walked by the segmentation functions: column, row, slice.
type Cell = (i32, i32, i32);

/// The voxels of a volume's extent, columns outermost and slices innermost (§23).
struct Cells {
    dims: Cell,
    next: Cell,
}

fn cells(m: &Mri) -> Cells {
    let g = &m.geom;
    Cells {
        dims: (g.width.max(0), g.height.max(0), g.depth.max(0)),
        next: (0, 0, 0),
    }
}

impl Iterator for Cells {
    type Item = Cell;

    fn next(&mut self) -> Option<Cell> {
        let (w, h, d) = self.dims;
        if w == 0 || h == 0 || d == 0 || self.next.0 >= w {
            return None;
        }
        let cur = self.next;
        let (mut c, mut r, mut s) = cur;
        s += 1;
        if s == d {
            s = 0;
            r += 1;
            if r == h {
                r = 0;
                c += 1;
            }
        }
        self.next = (c, r, s);
        Some(cur)
    }
}

/// The voxel volume: the three voxel sizes multiplied as `float`s, left to right, as a `double`.
fn voxel_volume(m: &Mri) -> f64 {
    f64::from(crate::sse::mul32(
        crate::sse::mul32(m.geom.xsize, m.geom.ysize),
        m.geom.zsize,
    ))
}

/// The cache's measures (§23): the `<names>` field and the description.
pub const BRAIN_VOLUME_MEASURES: [(&str, &str); 16] = [
    ("BrainSeg, BrainSegVol", "Brain Segmentation Volume"),
    (
        "BrainSegNotVent, BrainSegVolNotVent",
        "Brain Segmentation Volume Without Ventricles",
    ),
    ("SupraTentorial, SupraTentorialVol", "Supratentorial volume"),
    (
        "SupraTentorialNotVent, SupraTentorialVolNotVent",
        "Supratentorial volume",
    ),
    ("SubCortGray, SubCortGrayVol", "Subcortical gray matter volume"),
    (
        "lhCortex, lhCortexVol",
        "Left hemisphere cortical gray matter volume",
    ),
    (
        "rhCortex, rhCortexVol",
        "Right hemisphere cortical gray matter volume",
    ),
    ("Cortex, CortexVol", "Total cortical gray matter volume"),
    ("TotalGray, TotalGrayVol", "Total gray matter volume"),
    (
        "lhCerebralWhiteMatter, lhCerebralWhiteMatterVol",
        "Left hemisphere cerebral white matter volume",
    ),
    (
        "rhCerebralWhiteMatter, rhCerebralWhiteMatterVol",
        "Right hemisphere cerebral white matter volume",
    ),
    (
        "CerebralWhiteMatter, CerebralWhiteMatterVol",
        "Total cerebral white matter volume",
    ),
    ("Mask, MaskVol", "Mask Volume"),
    (
        "SupraTentorialNotVentVox, SupraTentorialVolNotVentVox",
        "Supratentorial volume voxel count",
    ),
    (
        "BrainSegNotVentSurf, BrainSegVolNotVentSurf",
        "Brain Segmentation Volume Without Ventricles from Surf",
    ),
    (
        "VentricleChoroidVol, VentricleChoroidVol",
        "Volume of ventricles and choroid plexus",
    ),
];

/// The value of a word of the cache as a C++ stream reads a `double` from it without its last
/// character (§23): the longest start of an optional sign, digits with at most one point and an
/// exponent after a digit, which must read whole (a mantissa digit, and an exponent digit after
/// an `e`) and must not overflow.
fn stream_double(word: &[u8]) -> Option<f64> {
    let w = &word[..word.len().saturating_sub(1)];
    let mut i = 0;
    if matches!(w.first(), Some(b'+' | b'-')) {
        i = 1;
    }
    let (mut digits, mut point) = (false, false);
    while i < w.len() {
        match w[i] {
            b'0'..=b'9' => digits = true,
            b'.' if !point => point = true,
            _ => break,
        }
        i += 1;
    }
    let mut exp_digits = None;
    if digits && matches!(w.get(i), Some(b'e' | b'E')) {
        i += 1;
        if matches!(w.get(i), Some(b'+' | b'-')) {
            i += 1;
        }
        let start = i;
        while i < w.len() && w[i].is_ascii_digit() {
            i += 1;
        }
        exp_digits = Some(i > start);
    }
    if !digits || exp_digits == Some(false) {
        return None;
    }
    let text = std::str::from_utf8(&w[..i]).ok()?;
    let v: f64 = text.parse().ok()?;
    v.is_finite().then_some(v)
}

impl Process {
    /// A voxel value as a label (§23): truncation toward zero; FS-311: a NaN or a value outside
    /// the `int` range gives -2147483648 with `reproduce`; by default NaN gives 0 and the others
    /// the range's end (noted).
    pub fn label_of(&mut self, v: f32) -> i32 {
        if v > -2_147_483_904.0 && v < 2_147_483_648.0 {
            return v.trunc() as i32;
        }
        if self.mode().reproduce() {
            self.note(
                &FS_311,
                "a voxel value or list entry that is NaN or beyond the int range as the label -2147483648",
            );
            return i32::MIN;
        }
        self.note(
            &FS_311,
            "a voxel value or list entry that is NaN or beyond the int range as the label 0 (NaN) or the range's end (FreeSurfer gives -2147483648)",
        );
        if v.is_nan() {
            0
        } else if v > 0.0 {
            i32::MAX
        } else {
            i32::MIN
        }
    }

    /// The label of voxel (c, r, s) of frame 0 (§23: read as a `float`, truncated).
    fn voxel_label(&mut self, m: &Mri, (c, r, s): Cell) -> Result<i32, Stop> {
        let v = self.mri_get_vox_val(m, c, r, s, 0, 0)?;
        Ok(self.label_of(v))
    }

    /// `MRIasegContraLatLabel(id)` (§23): the same structure's label in the other hemisphere; -1
    /// for a label it does not know (below 1000 after its message on standard output).
    pub fn mri_aseg_contra_lat_label(&mut self, id: i32) -> i32 {
        match contra(id) {
            Some(v) => v,
            None => {
                self.put_out(format!("MRIasegContraLatLabel() id={id} unrecognized\n").as_bytes());
                -1
            }
        }
    }

    /// FS-312: a thalamus voxel adds the voxel volume twice with `reproduce`, once by default.
    fn thalamus_twice(&mut self, function: &str) -> bool {
        if self.mode().reproduce() {
            self.note(
                &FS_312,
                format!("{function}: a thalamus voxel counted twice, as FreeSurfer's"),
            );
            true
        } else {
            self.note(
                &FS_312,
                format!("{function}: a thalamus voxel counted once (FreeSurfer counts it twice)"),
            );
            false
        }
    }

    /// `SupraTentorialVolCorrection(aseg, ribbon)` (§23): the volume of the voxels outside the
    /// ribbon (its label not above 0) whose label is a ventricle, choroid plexus, hypointensity,
    /// thalamus, corpus callosum part, ventral diencephalon, hippocampus, amygdala, caudate,
    /// putamen, pallidum or accumbens area; FS-312.
    pub fn supra_tentorial_vol_correction(&mut self, aseg: &Mri, ribbon: &Mri) -> Result<f64, Stop> {
        let vol = voxel_volume(aseg);
        let mut sum = 0.0f64;
        for cell in cells(aseg) {
            let rib = self.voxel_label(ribbon, cell)?;
            let l = self.voxel_label(aseg, cell)?;
            if rib > 0 {
                continue;
            }
            let lf = f64::from(l);
            if is_thalamus(lf) {
                sum += vol;
                if self.thalamus_twice("SupraTentorialVolCorrection") {
                    sum += vol;
                }
            } else if any(
                lf,
                &[4, 43, 5, 44, 31, 63, 77, 78, 79, 28, 60, 17, 53, 18, 54, 11, 50, 12, 51, 13, 52, 26, 58],
            ) || is_cc(lf)
            {
                sum += vol;
            }
        }
        Ok(sum)
    }

    /// `CorticalGMVolCorrection(aseg, ribbon, hemi)` (§23): over the hemisphere's ribbon (`hemi`
    /// 1: ribbon label 3, 2: 42, other: every voxel), `vol2` the volume of the labels other than
    /// 0, 2, 3, 41, 42 and the returned `vol` that of the subcortical, ventricular, cerebellar
    /// cortex and vessel labels; its line on standard output; FS-312, FS-313.
    pub fn cortical_gm_vol_correction(
        &mut self,
        aseg: &Mri,
        ribbon: &Mri,
        hemi: i32,
    ) -> Result<f64, Stop> {
        let vox = voxel_volume(aseg);
        let (mut vol, mut vol2) = (0.0f64, 0.0f64);
        for cell in cells(aseg) {
            let rib = self.voxel_label(ribbon, cell)?;
            let inside = match hemi {
                1 => rib == 3,
                2 => rib == 42,
                _ => true,
            };
            if !inside {
                continue;
            }
            let l = self.voxel_label(aseg, cell)?;
            let lf = f64::from(l);
            if !any(lf, &[0, 2, 3, 41, 42]) {
                vol2 += vox;
            }
            if is_thalamus(lf) {
                vol += vox;
                if self.thalamus_twice("CorticalGMVolCorrection") {
                    vol += vox;
                }
            } else if l == 62 {
                if self.mode().reproduce() {
                    self.note(
                        &FS_313,
                        "CorticalGMVolCorrection: the right vessel's voxels not counted, as FreeSurfer's",
                    );
                } else {
                    self.note(
                        &FS_313,
                        "CorticalGMVolCorrection: the right vessel's voxels counted as the left vessel's (FreeSurfer leaves them out)",
                    );
                    vol += vox;
                }
            } else if any(
                lf,
                &[17, 53, 18, 54, 4, 43, 5, 44, 31, 63, 77, 78, 79, 28, 60, 11, 50, 12, 51, 13, 52, 26, 58, 8, 47, 30],
            ) || is_cc(lf)
            {
                vol += vox;
            }
        }
        let mut line = format!("CorticalGMVolCorrection(): hemi={hemi}, vol1=").into_bytes();
        line.extend_from_slice(&sprintf_f64(b"%g", vol));
        line.extend_from_slice(b", vol2=");
        line.extend_from_slice(&sprintf_f64(b"%g", vol2));
        line.push(b'\n');
        self.put_out(&line);
        Ok(vol)
    }

    /// `MRIlrswapAseg(aseg)` (§23): a clone whose frame 0 holds each voxel's
    /// `MRIasegContraLatLabel` (the label itself where that gives -1).
    pub fn mri_lrswap_aseg(&mut self, aseg: &Mri) -> Result<Mri, Stop> {
        let mut d = self.mri_clone_new(aseg)?;
        for cell in cells(aseg) {
            let l = self.voxel_label(aseg, cell)?;
            let mut s = self.mri_aseg_contra_lat_label(l);
            if s == -1 {
                s = l;
            }
            let (c, r, z) = cell;
            self.mri_set_vox_val(&mut d, c, r, z, 0, s as f32, 0)?;
        }
        Ok(d)
    }

    /// The first `rows` numbers of a label list in row order (§23's `MRIlrswapSeg`); with fewer
    /// numbers than rows (FS-315) `None`, or with `reproduce` the numbers it holds and NaN (equal
    /// to no label) for those FreeSurfer reads past its end.
    fn label_list(&mut self, m: &Matrix) -> Option<Vec<f32>> {
        let n = m.rows.max(0) as usize;
        let mut v = Vec::with_capacity(n);
        'rows: for r in 1..=m.rows {
            for c in 1..=m.cols {
                if v.len() == n {
                    break 'rows;
                }
                v.push(m.at(r, c));
            }
        }
        if v.len() < n {
            if !self.mode().reproduce() {
                return None;
            }
            self.note(
                &FS_315,
                "MRIlrswapSeg: a label list with fewer numbers than rows: the numbers past its end as NaN, equal to no label (FreeSurfer reads memory there)",
            );
            v.resize(n, f32::NAN);
        }
        Some(v)
    }

    /// `MRIlrswapSeg(seg, lh, rh)` (§23): a clone of `seg` whose frame 0 holds each voxel's label
    /// swapped to the other side of the first pair it equals; `None` after the message when the
    /// lists' row counts differ. FS-315: a list with fewer numbers than rows is an error by
    /// default; `reproduce` reads its missing numbers as NaN.
    pub fn mri_lrswap_seg(
        &mut self,
        seg: &Mri,
        lh: &Matrix,
        rh: &Matrix,
    ) -> Result<Option<Mri>, Stop> {
        if lh.rows != rh.rows {
            self.put_out(
                format!(
                    "ERROR: MRIlrswapSeg(): lh={} != rh={}\n",
                    lh.rows, rh.rows
                )
                .as_bytes(),
            );
            return Ok(None);
        }
        let (Some(left), Some(right)) = (self.label_list(lh), self.label_list(rh)) else {
            self.note(
                &FS_315,
                "MRIlrswapSeg: a label list with fewer numbers than rows refused (FreeSurfer reads past its end)",
            );
            self.error_printf(
                codes::BAD_PARM,
                b"MRIlrswapSeg(): a label list holds fewer numbers than its rows",
            );
            return Ok(None);
        };
        let mut d = self.mri_clone_new(seg)?;
        let mut nswaps = 0i64;
        for cell in cells(seg) {
            let l = self.voxel_label(seg, cell)?;
            let lf = l as f32;
            let mut out = l;
            for (a, b) in left.iter().zip(&right) {
                let other = if lf == *a {
                    Some(*b)
                } else if lf == *b {
                    Some(*a)
                } else {
                    None
                };
                if let Some(o) = other {
                    let s = self.label_of(o);
                    if s != -1 {
                        out = s;
                    }
                    nswaps += 1;
                    break;
                }
            }
            let (c, r, z) = cell;
            self.mri_set_vox_val(&mut d, c, r, z, 0, out as f32, 0)?;
        }
        self.put_out(format!("MRIlrswapSeg(): nswaps {nswaps}\n").as_bytes());
        Ok(Some(d))
    }

    /// `MRIfixAsegWithRibbon(aseg, ribbon, asegfixed)` (§23): `aseg` copied (into the given
    /// destination), then each voxel whose label is 0, 2, 3, 41 or 42 takes the ribbon's label.
    pub fn mri_fix_aseg_with_ribbon(
        &mut self,
        aseg: &mut Mri,
        ribbon: &mut Mri,
        dst: Dst,
    ) -> Result<VolOut, Stop> {
        let mut v = Vols::new(aseg, Some(ribbon), dst);
        if v.has_dst() {
            let src = v.a().clone();
            if self.mri_copy(&src, Some(v.d_mut()))?.is_nothing() {
                return Ok(VolOut::Nothing);
            }
        } else {
            match self.mri_copy(v.a(), None)? {
                crate::maths::Returned::New(m) => v.set_dst(m),
                _ => return Ok(VolOut::Nothing),
            }
        }
        let walk = cells(v.a());
        for cell in walk {
            let l = self.voxel_label(v.a(), cell)?;
            if matches!(l, 0 | 2 | 3 | 41 | 42) {
                let rl = self.voxel_label(v.b(), cell)?;
                let (c, r, z) = cell;
                self.mri_set_vox_val(v.d_mut(), c, r, z, 0, rl as f32, 0)?;
            }
        }
        Ok(v.out())
    }

    /// `MRIoneHotEncode(seg, list, force1)` (§23): a byte volume of `seg`'s extent with one frame
    /// per list entry, `seg`'s header and pulse parameters; each voxel 1 in the frame of the
    /// first entry equal to its label, else (`force1` not 0 and a 0 listed) in the last 0's.
    pub fn mri_one_hot_encode(
        &mut self,
        seg: &Mri,
        list: &[i32],
        force1: i32,
    ) -> Result<Mri, Stop> {
        let g = &seg.geom;
        let mut d = self.mri_alloc_sequence(
            g.width,
            g.height,
            g.depth,
            types::UCHAR,
            list.len() as i32,
        )?;
        self.copy_header_into(seg, &mut d)?;
        self.copy_pulse_parameters(seg, &mut d);
        let mut line = format!("MRIoneHotEncode(): n={} Force1={force1}: ", list.len()).into_bytes();
        for l in list {
            line.extend_from_slice(format!("{l} ").as_bytes());
        }
        line.push(b'\n');
        self.put_out(&line);
        let last_zero = list.iter().rposition(|&x| x == 0);
        for cell in cells(seg) {
            let l = self.voxel_label(seg, cell)?;
            let k = match list.iter().position(|&x| x == l) {
                Some(k) => Some(k),
                None if force1 != 0 => last_zero,
                None => None,
            };
            if let Some(k) = k {
                let (c, r, z) = cell;
                self.mri_set_vox_val(&mut d, c, r, z, k as i32, 1.0, 0)?;
            }
        }
        Ok(d)
    }

    /// The cache's file name: `<subjdir>/<subject>/stats/brainvol.stats`.
    fn brainvol_name(subject: &[u8], subjdir: &[u8]) -> Vec<u8> {
        let mut n = subjdir.to_vec();
        n.push(b'/');
        n.extend_from_slice(subject);
        n.extend_from_slice(b"/stats/brainvol.stats");
        n
    }

    /// `CacheBrainVolumeStats(stats, subject, subjdir)` (§23): the 16 measures' lines into the
    /// subject's `stats/brainvol.stats`; a file that cannot be made ends the program (status 1).
    pub fn cache_brain_volume_stats(
        &mut self,
        stats: &[f64],
        subject: &[u8],
        subjdir: &[u8],
    ) -> Result<(), Stop> {
        let name = Process::brainvol_name(subject, subjdir);
        let mut text = Vec::new();
        for (k, (names, desc)) in BRAIN_VOLUME_MEASURES.iter().enumerate() {
            let v = stats.get(k).copied().unwrap_or(0.0);
            text.extend_from_slice(format!("# Measure {names}, {desc}, ").as_bytes());
            text.extend_from_slice(&sprintf_f64(b"%.12f", v));
            text.extend_from_slice(b", mm^3\n");
        }
        if std::fs::write(crate::files::path_of(&name), &text).is_err() {
            let mut m = b"could not open file ".to_vec();
            m.extend_from_slice(&name);
            m.extend_from_slice(b" for writing");
            self.fatal(1, &m)?;
        }
        Ok(())
    }

    /// `ReadCachedBrainVolumeStats(subject, subjdir)` (§23): the 16 measures from the cache, or,
    /// when it cannot be opened, computed by the caller's `compute` (`ComputeBrainVolumeStats`)
    /// and cached. A measure not found or negative ends the program (status 1); FS-314.
    pub fn read_cached_brain_volume_stats(
        &mut self,
        subject: &[u8],
        subjdir: &[u8],
        compute: &mut dyn FnMut(&mut Process) -> Result<Vec<f64>, Stop>,
    ) -> Result<Vec<f64>, Stop> {
        let name = Process::brainvol_name(subject, subjdir);
        let Ok(bytes) = std::fs::read(crate::files::path_of(&name)) else {
            self.put_out(b"computing (and caching) brain volume statistics\n");
            let stats = compute(self)?;
            self.cache_brain_volume_stats(&stats, subject, subjdir)?;
            return Ok(stats);
        };
        let mut stats = [-1.0f64; 16];
        let mut lines: Vec<&[u8]> = bytes.split(|&c| c == b'\n').collect();
        if bytes.last() == Some(&b'\n') {
            lines.pop();
        }
        for line in lines {
            let named: Vec<usize> = BRAIN_VOLUME_MEASURES
                .iter()
                .enumerate()
                .filter(|(_, (n, _))| contains(line, n.as_bytes()))
                .map(|(k, _)| k)
                .collect();
            if named.is_empty() {
                continue;
            }
            let value = line
                .split(|&c| larmorx_core::cscan::is_c_space(c))
                .filter(|w| !w.is_empty())
                .find_map(stream_double);
            match value {
                Some(v) => {
                    for k in named {
                        stats[k] = v;
                    }
                }
                None => {
                    if line
                        .last()
                        .is_some_and(|&c| larmorx_core::cscan::is_c_space(c))
                    {
                        // FreeSurfer aborts here; both modes give the line no number
                        self.note(
                            &FS_314,
                            "a cache line naming a measure without a number and ending in white space gives no number (FreeSurfer aborts)",
                        );
                    }
                }
            }
        }
        for (k, v) in stats.iter().enumerate() {
            if *v < 0.0 {
                let mut m = format!("could not find stat for {} in ", BRAIN_VOLUME_MEASURES[k].0)
                    .into_bytes();
                m.extend_from_slice(&name);
                self.fatal(1, &m)?;
            }
        }
        Ok(stats.to_vec())
    }
}

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}
