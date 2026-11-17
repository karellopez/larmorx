// SPDX-License-Identifier: Apache-2.0
//! The head's box (spec §20): `MRIfindApproximateSkullBoundingBox`, walks along lines through the
//! centre of mass for the longest dark run; FS-305.

use larmorx_core::defects::FS_305;

use crate::mri::{Mri, Region};
use crate::process::{Process, Stop, codes};

/// The state of one walk (§20): the current dark run's length `n` and first position `s`, the
/// bright samples in a row `b`, the longest run's length `l` and its edge `e`, and (the walk to
/// the top only) `m`, the most bright samples met just before a dark one since the longest was
/// last replaced.
struct Walk {
    n: i64,
    s: i64,
    b: i64,
    l: i64,
    e: i64,
    m: i64,
    /// The walk to the left edge (a run replaces the longest only when longer).
    left: bool,
    /// The walk to the top (keeps `m`, the final run's second rule).
    top: bool,
}

impl Walk {
    fn new(c: i64, left: bool, top: bool) -> Walk {
        Walk {
            n: 0,
            s: c,
            b: 0,
            l: 0,
            e: c,
            m: 0,
            left,
            top,
        }
    }

    fn dark(&mut self, p: i64) {
        if self.top {
            self.m = self.m.max(self.b);
        }
        if self.n == 0 {
            self.s = p;
        }
        self.n += 1;
        self.b = 0;
    }

    fn bright(&mut self) {
        self.b += 1;
        if self.b > 30 {
            self.l = 0;
        }
        let replace = if self.left {
            self.n > self.l
        } else {
            self.n >= self.l
        };
        if replace {
            self.l = self.n;
            self.e = self.s;
            self.m = 0;
        }
        self.n = 0;
    }

    /// `k` samples of one kind from position `p` on: a dark run grows by `k`; after 33 bright
    /// samples in a row every further one only counts (the state is otherwise fixed).
    fn many(&mut self, dark: bool, p: i64, k: i64) {
        if k <= 0 {
            return;
        }
        if dark {
            self.dark(p);
            self.n += k - 1;
            return;
        }
        let first = k.min(33);
        for _ in 0..first {
            self.bright();
        }
        self.b += k - first;
    }

    /// The edge after the last sample (`end`: the position the walk went to).
    fn finish(&mut self, end: i64) -> i64 {
        if self.n > self.l || (self.top && self.n > self.l / 2 && self.m > 15) {
            self.l = self.n;
            self.e = self.s;
        }
        if self.l < 10 {
            self.e = end;
        }
        self.e
    }
}

impl Process {
    /// Whether the nearest sample of frame 0 at whole coordinates is dark (below `thresh`, in
    /// double; a NaN is bright).
    fn skull_dark(&mut self, m: &Mri, p: [i64; 3], thresh: f64) -> Result<bool, Stop> {
        let mut v = 0.0;
        // whole coordinates far outside are outside the volume whatever their size
        let c = p.map(|x| x.clamp(-(1 << 40), 1 << 40) as f64);
        self.nearest_frame(m, c[0], c[1], c[2], 0, "MRIsampleVolumeType", &mut v)?;
        Ok(v < thresh)
    }

    /// One walk along `axis` from position `c` (the other coordinates in `at`), down to 0 or up
    /// to `extent - 1`; its edge (§20). Runs outside the volume are taken at once.
    #[allow(clippy::too_many_arguments)]
    fn skull_walk(
        &mut self,
        m: &Mri,
        axis: usize,
        at: [i64; 3],
        c: i64,
        up: bool,
        thresh: f64,
        left: bool,
        top: bool,
    ) -> Result<i64, Stop> {
        let e = [
            i64::from(m.geom.width),
            i64::from(m.geom.height),
            i64::from(m.geom.depth),
        ];
        let ext = e[axis];
        let end = if up { ext - 1 } else { 0 };
        let mut w = Walk::new(c, left, top);
        let count = if up { ext - c } else { c + 1 };
        if count <= 0 {
            return Ok(w.finish(end));
        }
        let point = |p: i64| {
            let mut q = at;
            q[axis] = p;
            q
        };
        let others_inside = (0..3).all(|k| k == axis || (0..e[k]).contains(&at[k]));
        if !others_inside || ext <= 0 {
            let dark = self.skull_dark(m, point(c), thresh)?;
            w.many(dark, c, count);
            return Ok(w.finish(end));
        }
        // the positions outside the volume before the inside ones, then the inside ones
        let (outside, first_in) = if up {
            ((-c).max(0), c.max(0))
        } else {
            ((c - (ext - 1)).max(0), c.min(ext - 1))
        };
        if outside > 0 {
            let dark = self.skull_dark(m, point(c), thresh)?;
            w.many(dark, c, outside);
        }
        let inside = if up { ext - first_in } else { first_in + 1 };
        for i in 0..inside {
            let p = if up { first_in + i } else { first_in - i };
            if self.skull_dark(m, point(p), thresh)? {
                w.dark(p);
            } else {
                w.bright();
            }
        }
        Ok(w.finish(end))
    }

    /// `MRIfindApproximateSkullBoundingBox(mri, thresh, box)` (§20): `NO_ERROR` and the box; the
    /// end of the program when no left-to-right width is found. FS-305: a centre of mass that is
    /// not a number ends the program by default; `reproduce` walks as FreeSurfer does and ends it
    /// only where FreeSurfer never returns.
    pub fn mri_find_approximate_skull_bounding_box(
        &mut self,
        m: &Mri,
        thresh: i32,
        bx: &mut Region,
    ) -> Result<i32, Stop> {
        let (_, centre) = self.mri_center_of_mass(m, thresh as u8)?;
        let [cx, mut cy, cz] = centre;
        let t = f64::from(thresh);
        let nan = centre.iter().any(|c| c.is_nan());
        if nan && !self.mode().reproduce() {
            self.note(
                &FS_305,
                "a centre of mass that is not a number: no box (FreeSurfer walks about two billion voxels outside the volume or never returns)",
            );
            return self.skull_not_a_number();
        }
        if nan {
            self.note(
                &FS_305,
                "a centre of mass that is not a number: walks outside the volume, as FreeSurfer's",
            );
        }
        let (x, z) = (i64::from(self.nint(cx)), i64::from(self.nint(cz)));
        loop {
            let y = i64::from(self.nint(cy));
            let left = self.skull_walk(m, 0, [0, y, z], x, false, t, true, false)?;
            let right = self.skull_walk(m, 0, [0, y, z], x, true, t, false, false)?;
            bx.x = left as i32;
            bx.dx = (right as i32).wrapping_sub(bx.x).wrapping_add(1);
            if bx.dx > 10 {
                break;
            }
            cy -= 10.0;
            let (a, b) = (self.nint(cy), self.nint(cy));
            let line = format!(
                "left/right detection failed, moving y coord to {} from {}\n",
                a.wrapping_add(10),
                b
            );
            self.put_out(line.as_bytes());
            if cy < 0.0 {
                self.error_exit(
                    codes::BAD_PARM,
                    b"MRIfindApproximateSkullBoundingBox failed: check input volume",
                )?;
                return Ok(codes::BAD_PARM);
            }
            if cy.is_nan() {
                self.note(
                    &FS_305,
                    "a centre of mass that is not a number: no width found, so FreeSurfer never returns",
                );
                return self.skull_not_a_number();
            }
        }
        let y = i64::from(self.nint(cy));
        let xt = i64::from(self.nint(cx).wrapping_sub(20).max(0));
        let top = self.skull_walk(m, 1, [xt, 0, z], y, false, t, false, true)?;
        let bottom = self.skull_walk(m, 1, [x, 0, z], y, true, t, false, false)?;
        bx.y = top as i32;
        bx.dy = (bottom as i32).wrapping_sub(bx.y).wrapping_add(1);
        let mut zx = x;
        for k in 0..2 {
            let back = self.skull_walk(m, 2, [zx, y, 0], z, false, t, false, false)?;
            let front = self.skull_walk(m, 2, [zx, y, 0], z, true, t, false, false)?;
            bx.z = back as i32;
            bx.dz = (front as i32).wrapping_sub(bx.z).wrapping_add(1);
            if bx.dz >= 5 || k == 1 {
                break;
            }
            zx = i64::from((x as i32).wrapping_sub(15));
        }
        Ok(codes::NONE)
    }

    /// FS-305's end: the program ends with an error.
    fn skull_not_a_number(&mut self) -> Result<i32, Stop> {
        self.error_exit(
            codes::BAD_PARM,
            b"MRIfindApproximateSkullBoundingBox: the volume's centre of mass is not a number",
        )?;
        Ok(codes::BAD_PARM)
    }
}
