// SPDX-License-Identifier: Apache-2.0
//! Neighbourhoods and labels (spec §35, part 7b): the parcellation's smoothing, the border planes,
//! the window counts and maxima, the nearest non-zero voxel, the means in a label, the positive
//! shift, label borders, label counts and boxes, the box fill, the mask and the clipping of bright
//! white matter; FS-467 and FS-468 (and the shared FS-12, FS-52, FS-53, FS-58, FS-289, FS-290,
//! FS-323).

use larmorx_core::defects::{FS_467, FS_468};

use super::sample::cvtt;
use super::{Dst, VolOut, Vols};
use crate::maths::Returned;
use crate::maths::matrix::{MATRIX_REAL, Matrix};
use crate::mri::{Mri, Region, types};
use crate::process::{Process, Stop, codes};
use crate::sse;

/// `whalf = (wsize - 1) / 2` (C's division, truncating toward 0).
fn half(wsize: i32) -> i32 {
    wsize.wrapping_sub(1) / 2
}

/// The offsets `-whalf..=whalf` (none when `whalf` is negative).
fn offsets(whalf: i32) -> std::ops::RangeInclusive<i32> {
    whalf.wrapping_neg()..=whalf
}

/// The 26 neighbours' order of `MRIsmoothParcellation` (§35): the first offset
/// varying slowest, each from +1 to -1, the voxel itself skipped.
const NEIGHBOURS: [(i32, i32, i32); 26] = {
    let mut t = [(0, 0, 0); 26];
    let mut k = 0;
    let mut a = 1;
    while a >= -1 {
        let mut b = 1;
        while b >= -1 {
            let mut c = 1;
            while c >= -1 {
                if !(a == 0 && b == 0 && c == 0) {
                    t[k] = (a, b, c);
                    k += 1;
                }
                c -= 1;
            }
            b -= 1;
        }
        a -= 1;
    }
    t
};

impl Process {
    /// The window of half-width `whalf` around `(x, y, z)` through the index tables (FS-323),
    /// z outermost, then y, x: the clamped places in that order.
    fn window_zyx(
        &mut self,
        m: &Mri,
        whalf: i32,
        (x, y, z): (i32, i32, i32),
        name: &str,
    ) -> Result<Vec<(i32, i32, i32)>, Stop> {
        let g = &m.geom;
        let mut out = Vec::new();
        for dz in offsets(whalf) {
            let zi = self.table_index(g.depth, z.wrapping_add(dz), name)?;
            for dy in offsets(whalf) {
                let yi = self.table_index(g.height, y.wrapping_add(dy), name)?;
                for dx in offsets(whalf) {
                    let xi = self.table_index(g.width, x.wrapping_add(dx), name)?;
                    out.push((xi, yi, zi));
                }
            }
        }
        Ok(out)
    }

    /// The same window, x outermost, then y, z.
    fn window_xyz(
        &mut self,
        m: &Mri,
        whalf: i32,
        (x, y, z): (i32, i32, i32),
        name: &str,
    ) -> Result<Vec<(i32, i32, i32)>, Stop> {
        let g = &m.geom;
        let mut out = Vec::new();
        for dx in offsets(whalf) {
            let xi = self.table_index(g.width, x.wrapping_add(dx), name)?;
            for dy in offsets(whalf) {
                let yi = self.table_index(g.height, y.wrapping_add(dy), name)?;
                for dz in offsets(whalf) {
                    let zi = self.table_index(g.depth, z.wrapping_add(dz), name)?;
                    out.push((xi, yi, zi));
                }
            }
        }
        Ok(out)
    }

    /// Frame 0's voxel at a place (`MRIgetVoxVal`).
    fn v0(&mut self, m: &Mri, (x, y, z): (i32, i32, i32)) -> Result<f32, Stop> {
        self.vf(m, (x, y, z), 0)
    }

    /// Frame `f`'s voxel at a place (`MRIgetVoxVal`; inside the volume through the flat reader,
    /// where no message or defect can arise).
    fn vf(&mut self, m: &Mri, (x, y, z): (i32, i32, i32), f: i32) -> Result<f32, Stop> {
        match m.inside_vox_val(x, y, z, f) {
            Some(v) => Ok(v),
            None => self.mri_get_vox_val(m, x, y, z, f, 0),
        }
    }

    /// `MRIsmoothParcellation(mri, count)` (§35): shorts only; a copy in which each inner voxel
    /// takes the last of its 26 neighbours' labels (in their order) that at least `count` of them
    /// carry. FS-467: a negative label is counted like any other (both modes), noted.
    pub fn mri_smooth_parcellation(&mut self, m: &Mri, count: i32) -> Result<Option<Mri>, Stop> {
        if m.kind != types::SHORT {
            self.error_printf(
                codes::UNSUPPORTED,
                b"MRIsmoothParcellation(): only supported for shorts data",
            );
            return Ok(None);
        }
        let mut out = match self.mri_copy(m, None)? {
            Returned::New(c) => c,
            _ => return Ok(None),
        };
        let g = &m.geom;
        let (w, h, d) = (g.width, g.height, g.depth);
        if w < 3 || h < 3 || d < 3 {
            return Ok(Some(out));
        }
        self.direct_needs_voxels(m, "MRIsmoothParcellation")?;
        let (wu, hu) = (w as usize, h as usize);
        let at = |x: i32, y: i32, z: i32| 2 * ((z as usize * hu + y as usize) * wu + x as usize);
        let src = m.voxels.as_deref().expect("voxels");
        let mut negative = false;
        let mut writes = Vec::new();
        for x in 1..w - 1 {
            for y in 1..h - 1 {
                for z in 1..d - 1 {
                    let mut labs = [0i16; 26];
                    for (k, &(a, b, c)) in NEIGHBOURS.iter().enumerate() {
                        let o = at(x + a, y + b, z + c);
                        labs[k] = i16::from_le_bytes([src[o], src[o + 1]]);
                    }
                    negative |= labs.iter().any(|&l| l < 0);
                    let mut best = None;
                    for &l in &labs {
                        let n = labs.iter().filter(|&&q| q == l).count() as i32;
                        if n >= count {
                            best = Some(l);
                        }
                    }
                    if let Some(l) = best {
                        writes.push((at(x, y, z), l));
                    }
                }
            }
        }
        if negative {
            self.note(
                &FS_467,
                "MRIsmoothParcellation: a negative label counted like any other (FreeSurfer counts it outside its array)",
            );
        }
        if let Some(v) = out.voxels.as_mut() {
            for (o, l) in writes {
                v[o..o + 2].copy_from_slice(&l.to_le_bytes());
            }
        }
        Ok(Some(out))
    }

    /// `MRIeraseBorderPlanes(mri, b)` (§35): the `b` outer planes of each axis set to 0 in every
    /// frame (`MRIsetVoxVal`: a border wider than an extent writes outside, FS-52/FS-53).
    pub fn mri_erase_border_planes(&mut self, m: &mut Mri, b: i32) -> Result<i32, Stop> {
        let g = m.geom.clone();
        let (w, h, d) = (g.width, g.height, g.depth);
        for f in 0..m.nframes {
            for x in 0..w {
                for y in 0..h {
                    for i in 0..b {
                        self.mri_set_vox_val(m, x, y, i, f, 0.0, 0)?;
                        self.mri_set_vox_val(m, x, y, d - i - 1, f, 0.0, 0)?;
                    }
                }
            }
        }
        for f in 0..m.nframes {
            for y in 0..h {
                for z in 0..d {
                    for i in 0..b {
                        self.mri_set_vox_val(m, i, y, z, f, 0.0, 0)?;
                        self.mri_set_vox_val(m, w - i - 1, y, z, f, 0.0, 0)?;
                    }
                }
            }
        }
        for f in 0..m.nframes {
            for x in 0..w {
                for z in 0..d {
                    for i in 0..b {
                        self.mri_set_vox_val(m, x, i, z, f, 0.0, 0)?;
                        self.mri_set_vox_val(m, x, h - i - 1, z, f, 0.0, 0)?;
                    }
                }
            }
        }
        Ok(codes::NONE)
    }

    /// `MRImaxInNbhd(mri, wsize, x, y, z, frame)` (§35): the largest voxel of the frame in the
    /// window (from -1e10), as `(int)` (FS-12).
    pub fn mri_max_in_nbhd(
        &mut self,
        m: &Mri,
        wsize: i32,
        p: (i32, i32, i32),
        frame: i32,
    ) -> Result<i32, Stop> {
        let mut max = -1e10f64;
        for (x, y, z) in self.window_zyx(m, half(wsize), p, "MRImaxInNbhd")? {
            let v = sse::widen(self.vf(m, (x, y, z), frame)?);
            if v > max {
                max = v;
            }
        }
        Ok(self.int_fs12(max))
    }

    /// `MRIcountNonzeroInNbhd(mri, wsize, x, y, z)` (§35): frame 0's window voxels above 0.
    pub fn mri_count_nonzero_in_nbhd(
        &mut self,
        m: &Mri,
        wsize: i32,
        p: (i32, i32, i32),
    ) -> Result<i32, Stop> {
        self.count_above(m, wsize, p, 0.0, false, "MRIcountNonzeroInNbhd")
    }

    /// `MRIcountThreshInNbhd(mri, wsize, x, y, z, thresh)` (§35): frame 0's window voxels above
    /// `thresh`.
    pub fn mri_count_thresh_in_nbhd(
        &mut self,
        m: &Mri,
        wsize: i32,
        p: (i32, i32, i32),
        thresh: f32,
    ) -> Result<i32, Stop> {
        self.count_above(m, wsize, p, thresh, false, "MRIcountThreshInNbhd")
    }

    /// `MRIareNonzeroInNbhd(mri, wsize, x, y, z)` (§35): 1 at the first window voxel above 0.
    pub fn mri_are_nonzero_in_nbhd(
        &mut self,
        m: &Mri,
        wsize: i32,
        p: (i32, i32, i32),
    ) -> Result<i32, Stop> {
        self.count_above(m, wsize, p, 0.0, true, "MRIareNonzeroInNbhd")
    }

    fn count_above(
        &mut self,
        m: &Mri,
        wsize: i32,
        (x, y, z): (i32, i32, i32),
        thresh: f32,
        first: bool,
        name: &str,
    ) -> Result<i32, Stop> {
        let g = &m.geom;
        let wh = half(wsize);
        let mut n = 0;
        for dz in offsets(wh) {
            let zi = self.table_index(g.depth, z.wrapping_add(dz), name)?;
            for dy in offsets(wh) {
                let yi = self.table_index(g.height, y.wrapping_add(dy), name)?;
                for dx in offsets(wh) {
                    let xi = self.table_index(g.width, x.wrapping_add(dx), name)?;
                    if self.v0(m, (xi, yi, zi))? > thresh {
                        if first {
                            return Ok(1);
                        }
                        n += 1;
                    }
                }
            }
        }
        Ok(n)
    }

    /// The byte at element `x` of row `y` of slice `z` read directly as a byte whatever the
    /// volume's type (§35).
    fn row_byte(&mut self, m: &Mri, (x, y, z): (i32, i32, i32), name: &str) -> u8 {
        let place = self.row_place(m, i64::from(x), i64::from(y), i64::from(z), 0, 1);
        self.direct_read8(m, place, 1, name)[0]
    }

    /// `MRIfindNearestNonzeroLocation(mri, wsize, xr, yr, zr, &xv, &yv, &zv)` (§35): the rounded
    /// point's byte when above 0 (the place not written); else the nearest window byte above 0
    /// (its place written when asked for), or 0.
    pub fn mri_find_nearest_nonzero_location(
        &mut self,
        m: &Mri,
        wsize: i32,
        (xr, yr, zr): (f64, f64, f64),
        mut place: Option<&mut [i32; 3]>,
    ) -> Result<f32, Stop> {
        let name = "MRIfindNearestNonzeroLocation";
        let (x0, y0, z0) = (self.nint(xr), self.nint(yr), self.nint(zr));
        let b = self.row_byte(m, (x0, y0, z0), name);
        if b > 0 {
            return Ok(f32::from(b));
        }
        let g = &m.geom;
        let (w, h, d) = (g.width, g.height, g.depth);
        let wh = half(wsize);
        let (mut best, mut val) = (100000.0f32, 0.0f32);
        for kz in offsets(wh) {
            let zi = self.table_index(d, z0.wrapping_add(kz), name)?;
            let dz = sse::narrow(sse::sub64(f64::from(zi), zr));
            for ky in offsets(wh) {
                let yi = self.table_index(h, y0.wrapping_add(ky), name)?;
                let dy = sse::narrow(sse::sub64(f64::from(yi), yr));
                for kx in offsets(wh) {
                    let xi = self.table_index(w, x0.wrapping_add(kx), name)?;
                    let dx = sse::narrow(sse::sub64(f64::from(xi), xr));
                    let b = self.row_byte(m, (xi, yi, zi), name);
                    if b == 0 {
                        continue;
                    }
                    let s = sse::add32(
                        sse::add32(sse::mul32(dx, dx), sse::mul32(dy, dy)),
                        sse::mul32(dz, dz),
                    );
                    let dist = sse::narrow(sse::sqrt64(sse::widen(s)));
                    if dist < best {
                        best = dist;
                        val = f32::from(b);
                        if let Some(p) = place.as_deref_mut() {
                            *p = [xi, yi, zi];
                        }
                    }
                }
            }
        }
        Ok(val)
    }

    /// The label of a voxel in the means and counts (§35): `nint` of the voxel.
    fn label_at(&mut self, lab: &Mri, p: (i32, i32, i32)) -> Result<i32, Stop> {
        let v = self.v0(lab, p)?;
        Ok(self.nint(sse::widen(v)))
    }

    /// The `double` sums of frame `f`'s voxels whose label is `label` over the places (the sum,
    /// the sum of the float squares, the count).
    fn label_sums(
        &mut self,
        src: &Mri,
        lab: &Mri,
        label: i32,
        f: i32,
        places: &[(i32, i32, i32)],
    ) -> Result<(f64, f64, i32), Stop> {
        let (mut sum, mut sq, mut n) = (0.0f64, 0.0f64, 0i32);
        for &p in places {
            if self.label_at(lab, p)? == label {
                let v = self.vf(src, p, f)?;
                sum = sse::add64(sse::widen(v), sum);
                sq = sse::add64(sse::widen(sse::mul32(v, v)), sq);
                n += 1;
            }
        }
        Ok((sum, sq, n.max(1)))
    }

    /// The source's places, x outermost, then y, z.
    fn all_places(m: &Mri) -> Vec<(i32, i32, i32)> {
        let g = &m.geom;
        let mut out =
            Vec::with_capacity((g.width.max(0) * g.height.max(0) * g.depth.max(0)) as usize);
        for x in 0..g.width {
            for y in 0..g.height {
                for z in 0..g.depth {
                    out.push((x, y, z));
                }
            }
        }
        out
    }

    /// The box `x0 - whalf..=x0 + whalf` (and y, z; x outermost), places outside the source left
    /// out.
    fn region_places(m: &Mri, (x0, y0, z0): (i32, i32, i32), whalf: i32) -> Vec<(i32, i32, i32)> {
        let g = &m.geom;
        let mut out = Vec::new();
        let inside = |c: i32, e: i32| (0..e).contains(&c);
        for x in x0.wrapping_sub(whalf)..=x0.wrapping_add(whalf) {
            if !inside(x, g.width) {
                continue;
            }
            for y in y0.wrapping_sub(whalf)..=y0.wrapping_add(whalf) {
                if !inside(y, g.height) {
                    continue;
                }
                for z in z0.wrapping_sub(whalf)..=z0.wrapping_add(whalf) {
                    if inside(z, g.depth) {
                        out.push((x, y, z));
                    }
                }
            }
        }
        out
    }

    /// `std = sqrt(sq / n - mean mean)` (§35).
    fn label_std(sq: f64, n: i32, mean: f64) -> f64 {
        sse::sqrt64(sse::sub64(
            sse::div64(sq, f64::from(n)),
            sse::mul64(mean, mean),
        ))
    }

    /// `MRImeanInLabelMultispectral(src, lab, label)` (§35): each frame's mean in the label, a
    /// vector of the source's frames (`float`s).
    pub fn mri_mean_in_label_multispectral(
        &mut self,
        src: &Mri,
        lab: &Mri,
        label: i32,
    ) -> Result<Matrix, Stop> {
        let places = Self::all_places(src);
        let mut v = Matrix::zeros(src.nframes, 1, MATRIX_REAL);
        for f in 0..src.nframes {
            let (sum, _, n) = self.label_sums(src, lab, label, f, &places)?;
            v.set(f + 1, 1, sse::narrow(sse::div64(sum, f64::from(n))));
        }
        Ok(v)
    }

    /// `MRImeanInLabel(src, lab, label)` (§35): frame 0's mean in the label.
    pub fn mri_mean_in_label(&mut self, src: &Mri, lab: &Mri, label: i32) -> Result<f64, Stop> {
        self.mri_mean_and_std_in_label(src, lab, label, None)
    }

    /// `MRImeanAndStdInLabel(src, lab, label, &std)` (§35): the mean and, when asked, the
    /// standard deviation.
    pub fn mri_mean_and_std_in_label(
        &mut self,
        src: &Mri,
        lab: &Mri,
        label: i32,
        std: Option<&mut f64>,
    ) -> Result<f64, Stop> {
        let places = Self::all_places(src);
        let (sum, sq, n) = self.label_sums(src, lab, label, 0, &places)?;
        let mean = sse::div64(sum, f64::from(n));
        if let Some(s) = std {
            *s = Self::label_std(sq, n, mean);
        }
        Ok(mean)
    }

    /// `MRImeanInLabelInRegion(src, lab, label, x0, y0, z0, whalf, &sigma)` (§35).
    pub fn mri_mean_in_label_in_region(
        &mut self,
        src: &Mri,
        lab: &Mri,
        label: i32,
        p: (i32, i32, i32),
        whalf: i32,
        sigma: Option<&mut f64>,
    ) -> Result<f64, Stop> {
        let places = Self::region_places(src, p, whalf);
        let (sum, sq, n) = self.label_sums(src, lab, label, 0, &places)?;
        let mean = sse::div64(sum, f64::from(n));
        if let Some(s) = sigma {
            *s = Self::label_std(sq, n, mean);
        }
        Ok(mean)
    }

    /// `MRImaxInLabelInRegion(src, lab, label, x0, y0, z0, whalf)` (§35): the largest voxel in
    /// the label in the box (from 0).
    pub fn mri_max_in_label_in_region(
        &mut self,
        src: &Mri,
        lab: &Mri,
        label: i32,
        p: (i32, i32, i32),
        whalf: i32,
    ) -> Result<f64, Stop> {
        let mut max = 0.0f64;
        for q in Self::region_places(src, p, whalf) {
            if self.label_at(lab, q)? == label {
                let v = sse::widen(self.v0(src, q)?);
                if v > max {
                    max = v;
                }
            }
        }
        Ok(max)
    }

    /// `MRImakePositive(src, dst)` (§35): the source copied, shifted by its smallest voxel when
    /// that is not at or above 0.
    pub fn mri_make_positive(&mut self, src: &mut Mri, dst: Dst) -> Result<VolOut, Stop> {
        let (mut lo, mut hi) = (0.0f32, 0.0f32);
        self.mri_val_range(src, &mut lo, &mut hi)?;
        let mut v = Vols::new(src, None, dst);
        if v.has_dst() {
            if !v.d_is_a() {
                let s = v.a().clone();
                if self.mri_copy(&s, Some(v.d_mut()))?.is_nothing() {
                    return Ok(VolOut::Nothing);
                }
            }
        } else {
            match self.mri_copy(v.a(), None)? {
                Returned::New(m) => v.set_dst(m),
                _ => return Ok(VolOut::Nothing),
            }
        }
        // "unless min >= 0" (a NaN minimum shifts too)
        if lo < 0.0 || lo.is_nan() {
            let g = v.a().geom.clone();
            for f in 0..v.a().nframes {
                for x in 0..g.width {
                    for y in 0..g.height {
                        for z in 0..g.depth {
                            let s = self.vf(v.a(), (x, y, z), f)?;
                            self.mri_set_vox_val(v.d_mut(), x, y, z, f, sse::sub32(s, lo), 0)?;
                        }
                    }
                }
            }
        }
        Ok(v.out())
    }

    /// `MRImarkLabelBorderVoxels(src, dst, label, mark, six)` (§35): `mark` at each voxel on the
    /// label's border (inside or outside it), its neighbours through the index tables.
    pub fn mri_mark_label_border_voxels(
        &mut self,
        src: &mut Mri,
        dst: Dst,
        label: i32,
        mark: i32,
        six: bool,
    ) -> Result<VolOut, Stop> {
        let name = "MRImarkLabelBorderVoxels";
        let mut v = Vols::new(src, None, dst);
        self.clone_dst(&mut v)?;
        let g = v.a().geom.clone();
        for x in 0..g.width {
            for y in 0..g.height {
                for z in 0..g.depth {
                    let c = self.v0(v.a(), (x, y, z))?;
                    let this = self.int_fs12(sse::widen(c)) == label;
                    'n: for dx in -1..=1i32 {
                        let xi = self.table_index(g.width, x + dx, name)?;
                        for dy in -1..=1i32 {
                            let yi = self.table_index(g.height, y + dy, name)?;
                            for dz in -1..=1i32 {
                                if six && dx.abs() + dy.abs() + dz.abs() != 1 {
                                    continue;
                                }
                                let zi = self.table_index(g.depth, z + dz, name)?;
                                let nv = self.v0(v.a(), (xi, yi, zi))?;
                                let other = self.int_fs12(sse::widen(nv)) == label;
                                if this != other {
                                    self.mri_set_vox_val(v.d_mut(), x, y, z, 0, mark as f32, 0)?;
                                    break 'n;
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(v.out())
    }

    /// `MRIcomputeLabelNbhd(labels, vals, x, y, z, counts, means, whalf, max_labels)` (§35): the
    /// counts of the labels `0..max_labels` in the window (x outermost, then y, z), and with
    /// `vals` the labels' means of its values (each sum `v + sum`: the new value first). `max_labels`
    /// is the counts' length, and the means, when given, have the same length (a shorter means
    /// array holds only its entries); a label is `(int)` of the voxel (FS-12), counted only in
    /// `[0, max_labels)`; values without means are not read (FreeSurfer crashes there).
    #[allow(clippy::too_many_arguments)]
    pub fn mri_compute_label_nbhd(
        &mut self,
        labels: &Mri,
        vals: Option<&Mri>,
        p: (i32, i32, i32),
        counts: &mut [i32],
        mut means: Option<&mut [f32]>,
        whalf: i32,
    ) -> Result<i32, Stop> {
        let max = counts.len();
        counts.fill(0);
        if let Some(m) = means.as_deref_mut() {
            let k = m.len().min(max);
            m[..k].fill(0.0);
        }
        for q in self.window_xyz(labels, whalf, p, "MRIcomputeLabelNbhd")? {
            let v = self.v0(labels, q)?;
            let l = self.int_fs12(sse::widen(v));
            if l < 0 || l as usize >= max {
                continue;
            }
            counts[l as usize] += 1;
            if let (Some(vals), Some(m)) = (vals, means.as_deref_mut())
                && (l as usize) < m.len()
            {
                let x = self.v0(vals, q)?;
                m[l as usize] = sse::add32(x, m[l as usize]);
            }
        }
        if let (Some(_), Some(m)) = (vals, means) {
            for (l, &c) in counts.iter().enumerate() {
                if c > 0 && l < m.len() {
                    m[l] = sse::div32(m[l], c as f32);
                }
            }
        }
        Ok(codes::NONE)
    }

    /// `MRIlabelsInNbhd(mri, x, y, z, whalf, label)` (§35): the window's voxels of the label.
    pub fn mri_labels_in_nbhd(
        &mut self,
        m: &Mri,
        p: (i32, i32, i32),
        whalf: i32,
        label: i32,
    ) -> Result<i32, Stop> {
        let mut n = 0;
        for q in self.window_zyx(m, whalf, p, "MRIlabelsInNbhd")? {
            if self.label_at(m, q)? == label {
                n += 1;
            }
        }
        Ok(n)
    }

    /// `MRIlabelsInPlanarNbhd(mri, x, y, z, whalf, label, which)` (§35): the same in one plane
    /// (2 horizontal, 1 sagittal, 0 coronal), the third coordinate as given.
    #[allow(clippy::too_many_arguments)]
    pub fn mri_labels_in_planar_nbhd(
        &mut self,
        m: &Mri,
        (x, y, z): (i32, i32, i32),
        whalf: i32,
        label: i32,
        which: i32,
    ) -> Result<i32, Stop> {
        let name = "MRIlabelsInPlanarNbhd";
        let g = m.geom.clone();
        let mut places = Vec::new();
        match which {
            2 => {
                for dz in offsets(whalf) {
                    let zi = self.table_index(g.depth, z.wrapping_add(dz), name)?;
                    for dx in offsets(whalf) {
                        let xi = self.table_index(g.width, x.wrapping_add(dx), name)?;
                        places.push((xi, y, zi));
                    }
                }
            }
            1 => {
                for dz in offsets(whalf) {
                    let zi = self.table_index(g.depth, z.wrapping_add(dz), name)?;
                    for dy in offsets(whalf) {
                        let yi = self.table_index(g.height, y.wrapping_add(dy), name)?;
                        places.push((x, yi, zi));
                    }
                }
            }
            0 => {
                for dx in offsets(whalf) {
                    let xi = self.table_index(g.width, x.wrapping_add(dx), name)?;
                    for dy in offsets(whalf) {
                        let yi = self.table_index(g.height, y.wrapping_add(dy), name)?;
                        places.push((xi, yi, z));
                    }
                }
            }
            _ => {
                self.error_printf(
                    codes::UNSUPPORTED,
                    format!("MRIlabelsInPlanarNbhd: which={which} not supported yet").as_bytes(),
                );
                return Ok(0);
            }
        }
        let mut n = 0;
        for q in places {
            if self.label_at(m, q)? == label {
                n += 1;
            }
        }
        Ok(n)
    }

    /// `MRIlabelBoundingBox(mri, label, box)` (§35): the smallest box of the label's voxels;
    /// none: the extents and `-1 - x + 1`.
    pub fn mri_label_bounding_box(
        &mut self,
        m: &Mri,
        label: i32,
        b: &mut Region,
    ) -> Result<i32, Stop> {
        let g = &m.geom;
        let (mut lo, mut hi) = ([g.width, g.height, g.depth], [-1i32; 3]);
        for p in Self::all_places(m) {
            let v = self.v0(m, p)?;
            if self.int_fs12(sse::widen(v)) == label {
                for (k, c) in [p.0, p.1, p.2].into_iter().enumerate() {
                    lo[k] = lo[k].min(c);
                    hi[k] = hi[k].max(c);
                }
            }
        }
        b.x = lo[0];
        b.y = lo[1];
        b.z = lo[2];
        b.dx = hi[0].wrapping_sub(lo[0]).wrapping_add(1);
        b.dy = hi[1].wrapping_sub(lo[1]).wrapping_add(1);
        b.dz = hi[2].wrapping_sub(lo[2]).wrapping_add(1);
        Ok(codes::NONE)
    }

    /// `MRIfillBox(mri, box, v)` (§35): frame 0's voxels of the box cut to the volume set to `v`.
    pub fn mri_fill_box(&mut self, m: &mut Mri, b: &Region, v: f32) -> Result<i32, Stop> {
        let g = m.geom.clone();
        let span =
            |s: i32, n: i32, e: i32| (s.max(0), (e - 1).min(s.wrapping_add(n).wrapping_sub(1)));
        let (x0, x1) = span(b.x, b.dx, g.width);
        let (y0, y1) = span(b.y, b.dy, g.height);
        let (z0, z1) = span(b.z, b.dz, g.depth);
        for x in x0..=x1 {
            for y in y0..=y1 {
                for z in z0..=z1 {
                    self.mri_set_vox_val(m, x, y, z, 0, v, 0)?;
                }
            }
        }
        Ok(codes::NONE)
    }

    /// `MRImaskZero(src, mask, dst)` (§35): the source where the mask is above 0, else 0 (frame
    /// 0, over the source's extent).
    pub fn mri_mask_zero(
        &mut self,
        src: &mut Mri,
        mask: &mut Mri,
        dst: Dst,
    ) -> Result<VolOut, Stop> {
        let mut v = Vols::new(src, Some(mask), dst);
        self.clone_dst(&mut v)?;
        let g = v.a().geom.clone();
        for x in 0..g.width {
            for y in 0..g.height {
                for z in 0..g.depth {
                    let mv = self.v0(v.b(), (x, y, z))?;
                    let out = if mv > 0.0 {
                        self.v0(v.a(), (x, y, z))?
                    } else {
                        0.0
                    };
                    self.mri_set_vox_val(v.d_mut(), x, y, z, 0, out, 0)?;
                }
            }
        }
        Ok(v.out())
    }

    /// FS-468: a T1 voxel as a byte: x86-64's low byte of its 32-bit truncation (`reproduce`),
    /// clamped to 0..=255 (NaN 0) by default; noted where they differ.
    fn clip_byte(&mut self, v: f32) -> u8 {
        let rep = cvtt(sse::widen(v)) as u8;
        let fix = v as u8;
        if rep != fix {
            if self.mode().reproduce() {
                self.note(
                    &FS_468,
                    "MRIclipBrightWM: a value outside 0 to 255 kept as the low byte of its truncation",
                );
            } else {
                self.note(
                    &FS_468,
                    "MRIclipBrightWM: a value outside 0 to 255 clamped (FreeSurfer keeps the low byte of its truncation)",
                );
            }
        }
        if self.mode().reproduce() { rep } else { fix }
    }

    /// `MRIclipBrightWM(T1, wm)` (§35): each T1 voxel as a byte, lowered to 110 where the
    /// white-matter volume's byte is at least 5 and the T1's above 110; the count on standard
    /// output.
    pub fn mri_clip_bright_wm(&mut self, t1: &mut Mri, wm: &Mri) -> Result<i32, Stop> {
        let g = t1.geom.clone();
        let mut n = 0;
        for z in 0..g.depth {
            for y in 0..g.height {
                for x in 0..g.width {
                    let v = self.v0(t1, (x, y, z))?;
                    let mut b = self.clip_byte(v);
                    let wb = self.row_byte(wm, (x, y, z), "MRIclipBrightWM");
                    if wb >= 5 && b > 110 {
                        b = 110;
                        n += 1;
                    }
                    self.mri_set_vox_val(t1, x, y, z, 0, f32::from(b), 0)?;
                }
            }
        }
        self.put_out(format!("MRIclipBrightWM(): nthresh={n}, wmmin=5, clip=110 \n").as_bytes());
        Ok(codes::NONE)
    }
}
