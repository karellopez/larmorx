// SPDX-License-Identifier: Apache-2.0
//! fslmaths' kernel filters (`specs/fslmaths.md` §10.1, §10.2): the morphological filters
//! (`-dilM`, `-dilD`, `-dilF`, `-ero`, `-eroF`, `-fmedian`), `-dilall`, and the convolutions
//! (`-s`, `-fmean`, `-fmeanu`; separable or by the direct sum).
//!
//! Every filter works volume by volume and writes a new volume from the old one; the voxels of
//! a volume are computed in parallel, each from its own window in a fixed order, so the result
//! does not depend on the thread count. `-dilall` follows its queue in order (serial).
//!
//! FSL switches the direct sum to an FFT for large kernels (§10.2); the original keeps the
//! direct sum, whose results differ from FSL's FFT only in the last bits (§21's thresholds),
//! and prints the messages FSL prints on either path.

use std::collections::VecDeque;
use std::io::Write;

use larmorx_core::defects::{self, Notices, UpstreamDefects};
use rayon::prelude::*;

use super::image::Img;
use super::kernel::Kernel;
use super::num::{self, Num};
use super::ops::Stop;

pub const OFF_CENTRE_MORPH: &str =
    "WARNING:: Off-centre morphfilter being performed as kernel has even dimensions";
pub const OFF_CENTRE_CONV: &str =
    "WARNING:: Off-centre convolution being performed as kernel has even dimensions";
pub const EMPTY_MASK: &str = "ERROR:: Empty mask image";

/// FSL's abort (an uncaught exception) as the original prints it.
pub fn abort_lines(kind: &str, what: &str) -> String {
    format!(
        "terminate called after throwing an instance of '{kind}'\n  what():  {what}\nlarmorx fsl \
         fslmaths: FSL aborts here (signal 6); exit status 1 (larmorx)\n"
    )
}

/// FSL-161 with the default: a kernel without voxels is an error (rule B).
pub fn refuse_empty(notices: &mut Notices, op: &str, k: &Kernel) -> Result<(), Stop> {
    if notices.fix() && k.is_empty() {
        let [a, b, c] = k.dims;
        notices.add(
            &defects::FSL_161,
            format!("{op} with a kernel without voxels (sizes {a} {b} {c}) is refused (FSL filters with it)"),
        );
        return Err(Stop::Refuse(format!(
            "{op}: the kernel has no voxels (sizes {a} {b} {c}) (FSL-161: FSL filters with it and \
             leaves the image, NaN or 0)"
        )));
    }
    Ok(())
}

/// The image's x, y and z sizes and its voxels per volume.
fn grid<T>(img: &Img<T>) -> ([usize; 3], usize) {
    let [nx, ny, nz, _] = img.dims;
    ([nx, ny, nz], nx * ny * nz)
}

// ------------------------------------------------------------------------------------------
// Morphological filters (§10.1)

/// The morphological filters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Morph {
    DilM,
    DilD,
    DilF,
    Ero,
    EroF,
    Median,
}

impl Morph {
    pub fn of(op: &str) -> Option<Morph> {
        Some(match op {
            "-dilM" => Morph::DilM,
            "-dilD" => Morph::DilD,
            "-dilF" => Morph::DilF,
            "-ero" => Morph::Ero,
            "-eroF" => Morph::EroF,
            "-fmedian" => Morph::Median,
            _ => return None,
        })
    }

    /// The dilations take the window's non-zero values only.
    fn nonzero_only(self) -> bool {
        matches!(self, Morph::DilM | Morph::DilD)
    }
}

/// A morphological filter's window: the offsets of the kernel's voxels above 0.5 in the
/// kernel's order (z slowest), the count FSL sizes it by (the values in [0.5, FLT_MAX], NaN
/// counted), and whether a size of the kernel is even (FSL's warning goes by the kernel's sizes,
/// not by the extent of its values above 0.5: random testing, findings 25).
struct Window {
    offs: Vec<[i64; 3]>,
    count: usize,
    even: bool,
}

fn window(k: &Kernel) -> Window {
    let [nx, ny, nz] = k.size();
    let c = k.centre();
    let mut offs = Vec::new();
    let mut count = 0usize;
    for z in 0..nz {
        for y in 0..ny {
            for x in 0..nx {
                let v = k.at(x, y, z);
                if !(v < 0.5) && !(v > f32::MAX) {
                    count += 1;
                }
                if v > 0.5 {
                    offs.push([x as i64 - c[0], y as i64 - c[1], z as i64 - c[2]]);
                }
            }
        }
    }
    let even = !k.is_empty() && k.dims.iter().any(|&n| n % 2 == 0);
    Window { offs, count, even }
}

/// Two running maxima (`max`) or minima over the odd and even positions, from ∓∞, each
/// replaced only by a strictly larger (smaller) value; the first if strictly larger
/// (smaller) than the second, else the second.
#[inline]
fn extreme(w: &[f64], max: bool) -> f64 {
    let start = if max {
        f64::NEG_INFINITY
    } else {
        f64::INFINITY
    };
    let (mut a, mut b) = (start, start);
    for (n, &v) in w.iter().enumerate() {
        let p = if n % 2 == 0 { &mut a } else { &mut b };
        if (max && v > *p) || (!max && v < *p) {
            *p = v;
        }
    }
    if (max && a > b) || (!max && a < b) {
        a
    } else {
        b
    }
}

/// The sum of the window's values as two partial sums over the odd and even positions, added.
#[inline]
fn two_sums(w: &[f64]) -> f64 {
    let (mut a, mut b) = (0.0f64, 0.0f64);
    for (n, &v) in w.iter().enumerate() {
        if n % 2 == 0 {
            a = num::nan64(a, v, a + v);
        } else {
            b = num::nan64(b, v, b + v);
        }
    }
    num::nan64(a, b, a + b)
}

/// The mode: sorted descending, the first value of the (first) longest run of equal values.
fn mode(w: &mut [f64]) -> f64 {
    w.sort_unstable_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
    let (mut best, mut best_n) = (w[0], 0usize);
    let mut i = 0;
    while i < w.len() {
        let mut j = i + 1;
        while j < w.len() && w[j] == w[i] {
            j += 1;
        }
        if j - i > best_n {
            best_n = j - i;
            best = w[i];
        }
        i = j;
    }
    best
}

/// The median: the value at `max(1, ⌊(n + 2) / 2⌋) − 1` of the values sorted ascending.
fn median(w: &mut [f64]) -> f64 {
    let n = w.len();
    let at = ((n + 2) / 2).max(1) - 1;
    let (_, v, _) = w.select_nth_unstable_by(at, |a, b| {
        a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)
    });
    *v
}

/// What a voxel's filter met.
#[derive(Clone, Copy, Debug, Default)]
struct Met {
    /// More values entered the window than the kernel's count (Armadillo's bounds check).
    overflow: Option<usize>,
    /// A NaN in a window FSL sorts (FSL-162).
    nan: Option<usize>,
}

impl Met {
    fn merge(self, o: Met) -> Met {
        let first = |a: Option<usize>, b: Option<usize>| match (a, b) {
            (Some(x), Some(y)) => Some(x.min(y)),
            (x, None) => x,
            (None, y) => y,
        };
        Met {
            overflow: first(self.overflow, o.overflow),
            nan: first(self.nan, o.nan),
        }
    }
}

/// One volume through a morphological filter; `fix`: FSL-162's correction (NaN values left
/// out of sorted windows).
fn morph_volume<T: Num>(
    src: &[T],
    dims: [usize; 3],
    win: &Window,
    f: Morph,
    fix: bool,
) -> (Vec<T>, Met, bool) {
    let [nx, ny, nz] = dims;
    let plane = nx * ny;
    let mut out = src.to_vec();
    let check = win.offs.len() > win.count;
    let parts: Vec<(Met, bool)> = out
        .par_chunks_mut(plane.max(1))
        .enumerate()
        .map(|(z, o)| {
            let mut met = Met::default();
            let mut changed = false;
            let mut w: Vec<f64> = Vec::with_capacity(win.offs.len());
            for y in 0..ny {
                for x in 0..nx {
                    let idx = x + nx * y;
                    let v = src[z * plane + idx];
                    let at_zero = v == T::zero();
                    match f {
                        Morph::DilM | Morph::DilD if !at_zero => continue,
                        Morph::Ero if at_zero => continue,
                        _ => {}
                    }
                    w.clear();
                    for o in &win.offs {
                        let (px, py, pz) = (x as i64 + o[0], y as i64 + o[1], z as i64 + o[2]);
                        if px < 0
                            || py < 0
                            || pz < 0
                            || px >= nx as i64
                            || py >= ny as i64
                            || pz >= nz as i64
                        {
                            continue;
                        }
                        let s = src[px as usize + nx * (py as usize + ny * pz as usize)];
                        if f.nonzero_only() && s == T::zero() {
                            continue;
                        }
                        w.push(s.to_f64());
                    }
                    if check && w.len() > win.count {
                        met.overflow.get_or_insert(z * plane + idx);
                        continue;
                    }
                    if w.is_empty() {
                        continue;
                    }
                    let sorted = matches!(f, Morph::DilD | Morph::Median);
                    if sorted && w.iter().any(|q| q.is_nan()) {
                        met.nan.get_or_insert(z * plane + idx);
                        if !fix {
                            continue;
                        }
                        w.retain(|q| !q.is_nan());
                    }
                    let r = match f {
                        Morph::DilF => extreme(&w, true),
                        Morph::EroF => extreme(&w, false),
                        Morph::Ero => {
                            if extreme(&w, false) == 0.0 {
                                0.0
                            } else {
                                continue;
                            }
                        }
                        Morph::DilM => two_sums(&w) / w.len() as f64,
                        Morph::DilD | Morph::Median if w.is_empty() => num::default_nan(),
                        Morph::DilD => mode(&mut w),
                        Morph::Median => median(&mut w),
                    };
                    let (t, c) = T::convert(r, fix);
                    changed |= c;
                    o[idx] = t;
                }
            }
            (met, changed)
        })
        .collect();
    let mut met = Met::default();
    let mut changed = false;
    for (m, c) in parts {
        met = met.merge(m);
        changed |= c;
    }
    (out, met, changed)
}

/// A morphological filter over every volume.
pub fn morph<T: Num>(
    img: &mut Img<T>,
    k: &Kernel,
    f: Morph,
    op: &str,
    notices: &mut Notices,
    err: &mut dyn Write,
) -> Result<(), Stop> {
    refuse_empty(notices, op, k)?;
    let fix = notices.fix();
    let win = window(k);
    let (dims, per) = grid(img);
    let mut changed = false;
    let mut nan_seen = false;
    for t in 0..img.dims[3] {
        if k.is_empty() {
            // a kernel without voxels (FSL-161 reproduced)
            let _ = writeln!(err, "{EMPTY_MASK}");
            continue;
        }
        if win.even {
            let _ = writeln!(err, "{OFF_CENTRE_MORPH}");
        }
        if win.count == 0 && win.offs.is_empty() {
            // no value of the kernel at or above 0.5: every voxel as it is, silently (findings
            // 25)
            continue;
        }
        let vol = &img.data[t * per..(t + 1) * per];
        let (out, met, ch) = morph_volume(vol, dims, &win, f, fix);
        let nan_first = met.nan.filter(|_| !fix);
        match (met.overflow, nan_first) {
            (Some(a), Some(b)) if b < a => return Err(nan_abort()),
            (Some(_), _) => {
                return Err(Stop::Exception(abort_lines(
                    "std::out_of_range",
                    "Mat::operator(): index out of bounds",
                )));
            }
            (None, Some(_)) => return Err(nan_abort()),
            (None, None) => {}
        }
        nan_seen |= met.nan.is_some();
        changed |= ch;
        img.data[t * per..(t + 1) * per].copy_from_slice(&out);
    }
    if nan_seen {
        notices.add(
            &defects::FSL_162,
            format!("{op}: NaN values under the kernel left out of the sorted values (FSL stops: sort(): detected NaN)"),
        );
    }
    super::ops::note151(notices, changed, "a filter's results");
    Ok(())
}

fn nan_abort() -> Stop {
    Stop::Exception(abort_lines("std::logic_error", "sort(): detected NaN"))
}

// ------------------------------------------------------------------------------------------
// -dilall (§10.1)

/// `-dilall`: each volume's unset (zero) voxels filled from the set ones, in FSL's queue order.
pub fn dilall<T: Num>(img: &mut Img<T>, notices: &mut Notices) {
    let fix = notices.fix();
    let (dims, per) = grid(img);
    let mut changed = false;
    for t in 0..img.dims[3] {
        let vol = &mut img.data[t * per..(t + 1) * per];
        changed |= dilall_volume(vol, dims, fix);
    }
    super::ops::note151(notices, changed, "-dilall's means");
}

fn dilall_volume<T: Num>(vol: &mut [T], dims: [usize; 3], fix: bool) -> bool {
    let [nx, ny, nz] = dims;
    let n = vol.len();
    let mut set: Vec<bool> = vol.iter().map(|&v| v != T::zero()).collect();
    let mut queued = vec![false; n];
    let neighbours = |i: usize| {
        let (x, y, z) = (i % nx, (i / nx) % ny, i / (nx * ny));
        let mut list = [usize::MAX; 26];
        let mut m = 0;
        for dz in -1i64..=1 {
            for dy in -1i64..=1 {
                for dx in -1i64..=1 {
                    if dx == 0 && dy == 0 && dz == 0 {
                        continue;
                    }
                    let (px, py, pz) = (x as i64 + dx, y as i64 + dy, z as i64 + dz);
                    if px < 0
                        || py < 0
                        || pz < 0
                        || px >= nx as i64
                        || py >= ny as i64
                        || pz >= nz as i64
                    {
                        continue;
                    }
                    list[m] = px as usize + nx * (py as usize + ny * pz as usize);
                    m += 1;
                }
            }
        }
        (list, m)
    };
    let mut queue = VecDeque::new();
    for i in 0..n {
        if !set[i] {
            let (l, m) = neighbours(i);
            if l[..m].iter().any(|&j| set[j]) {
                queue.push_back(i);
                queued[i] = true;
            }
        }
    }
    let mut changed = false;
    while let Some(i) = queue.pop_front() {
        if set[i] {
            continue;
        }
        let (l, m) = neighbours(i);
        let mut sum = 0.0f64;
        let mut count = 0usize;
        for &j in &l[..m] {
            if set[j] {
                let v = vol[j].to_f64();
                sum = num::nan64(sum, v, sum + v);
                count += 1;
            }
        }
        let (v, c) = T::convert(sum / count as f64, fix);
        changed |= c;
        vol[i] = v;
        set[i] = true;
        for &j in &l[..m] {
            if !set[j] && !queued[j] {
                queued[j] = true;
                queue.push_back(j);
            }
        }
    }
    changed
}

// ------------------------------------------------------------------------------------------
// Convolutions (§10.2)

/// One axis of a separable filter over one volume: each voxel's weighted mean over the taps
/// inside the image, the sums kept in single precision.
///
/// Built for speed with the same bits: the weights' sum depends on the position along the
/// axis only (computed once per position, in the taps' order); each row of x is computed at
/// once, every voxel's taps added in their order (for x itself over the voxels whose taps are
/// all inside; the voxels at the row's ends one by one); a voxel whose result is NaN is
/// computed again by x86-64's rules ([`num::nan64`], [`num::acc32`]), so NaN bits do not
/// depend on the platform.
fn separable_axis<T: Num>(
    src: &[T],
    dims: [usize; 3],
    axis: usize,
    line: &[f64],
    c: i64,
    fix: bool,
) -> (Vec<T>, bool) {
    let [nx, ny, _] = dims;
    let n = dims[axis] as i64;
    let k = line.len() as i64;
    let stride = [1, nx, nx * ny][axis];
    let inside = |p: i64, i: i64| (0..n).contains(&(p + i - c));
    // the weights' sum at each position (its taps inside the image, in order)
    let wsum: Vec<f32> = (0..n)
        .map(|p| {
            line.iter().enumerate().fold(0.0f32, |w, (i, &kv)| {
                if inside(p, i as i64) {
                    num::acc32(w, kv)
                } else {
                    w
                }
            })
        })
        .collect();
    let finish = |sum: f32, w: f32| -> f32 {
        if f64::from(w).abs() > 1e-12 {
            num::nan32(sum, w, sum / w)
        } else {
            sum
        }
    };
    // one voxel by x86-64's rules: `idx` its index, `p` its position along the axis
    let exact = |idx: usize, p: i64| -> f32 {
        let base = idx - p as usize * stride;
        let mut sum = 0.0f32;
        for (i, &kv) in line.iter().enumerate() {
            if inside(p, i as i64) {
                let q = (p + i as i64 - c) as usize;
                let s = src[base + q * stride].to_f64();
                sum = num::acc32(sum, num::nan64(s, kv, s * kv));
            }
        }
        finish(sum, wsum[p as usize])
    };
    let mut out = vec![T::zero(); src.len()];
    let changed = out
        .par_chunks_mut(nx.max(1))
        .enumerate()
        .map_init(
            || vec![0.0f32; nx],
            |acc, (row, o)| {
                let base = row * nx;
                // the row's (y, z), and its position along the axis
                let p_row = match axis {
                    1 => (row % ny) as i64,
                    2 => (row / ny) as i64,
                    _ => 0,
                };
                // the voxels computed at once: x in [xa, xb)
                let (xa, xb) = if axis == 0 {
                    let xa = c.max(0) as usize;
                    let xb = (nx as i64 + c - k + 1).max(0) as usize;
                    (xa.min(nx), xb.max(xa.min(nx)).min(nx))
                } else {
                    (0, nx)
                };
                acc[xa..xb].fill(0.0);
                for (i, &kv) in line.iter().enumerate() {
                    let i = i as i64;
                    if xa == xb || (axis != 0 && !inside(p_row, i)) {
                        continue;
                    }
                    // the source of voxel x at tap i: x + d
                    let d = (i - c) * stride as i64;
                    let from = (base as i64 + xa as i64 + d) as usize;
                    let s = &src[from..from + (xb - xa)];
                    for (a, &v) in acc[xa..xb].iter_mut().zip(s) {
                        *a = (f64::from(*a) + v.fast_f64() * kv) as f32;
                    }
                }
                let mut ch = false;
                for (x, ov) in o.iter_mut().enumerate() {
                    let p = if axis == 0 { x as i64 } else { p_row };
                    let mut r = if (xa..xb).contains(&x) {
                        finish(acc[x], wsum[p as usize])
                    } else {
                        f32::NAN
                    };
                    if r.is_nan() {
                        r = exact(base + x, p);
                    }
                    let (t, cv) = T::convert(num::to_f64(r), fix);
                    ch |= cv;
                    *ov = t;
                }
                ch
            },
        )
        .reduce(|| false, |a, b| a || b);
    (out, changed)
}

/// The separable filter (`-s`, `-fmean` after a box or Gaussian kernel): along x over every
/// volume, then y, then z.
pub fn separable<T: Num>(
    img: &mut Img<T>,
    k: &Kernel,
    notices: &mut Notices,
    err: &mut dyn Write,
) -> bool {
    let fix = notices.fix();
    let (dims, per) = grid(img);
    let c = k.centre();
    let lines: [Vec<f64>; 3] = if k.is_empty() {
        [Vec::new(), Vec::new(), Vec::new()]
    } else {
        let [nx, ny, nz] = k.size();
        let [cx, cy, cz] = c.map(|v| v as usize);
        [
            (0..nx).map(|i| f64::from(k.at(i, cy, cz))).collect(),
            (0..ny).map(|j| f64::from(k.at(cx, j, cz))).collect(),
            (0..nz).map(|l| f64::from(k.at(cx, cy, l))).collect(),
        ]
    };
    let mut changed = false;
    for axis in 0..3 {
        let even = k.dims[axis] % 2 == 0;
        for t in 0..img.dims[3] {
            if even {
                let _ = writeln!(err, "{OFF_CENTRE_CONV}");
            }
            let vol = &img.data[t * per..(t + 1) * per];
            let (out, ch) = separable_axis(vol, dims, axis, &lines[axis], c[axis], fix);
            changed |= ch;
            img.data[t * per..(t + 1) * per].copy_from_slice(&out);
        }
    }
    changed
}

/// Whether FSL filters a volume of `nvox` voxels with a kernel of `nker` by its FFT (§10.2:
/// `nvox · nker > 44 · (2 nvox) · log₂(2 nvox)` in single precision), where it prints no
/// warning.
fn fsl_uses_fft(nvox: usize, nker: usize) -> bool {
    let two = (2 * nvox) as f32;
    (nvox as f32) * (nker as f32) > 44.0 * two * (f64::from(two).log2() as f32)
}

/// One row of a kernel for the direct sum: its `j`, its `l` and its taps `(i, value)`.
type TapRow = (i64, i64, Vec<(i64, f32)>);

/// The direct sum of one volume with a kernel (outside the image: 0), in single precision
/// (a double image's products in double); converted to `T`. Voxel `(i, j, k)` of the kernel
/// weighs the image at the voxel plus `(i, j, k) − centre` (a correlation; FSL's FFT path is a
/// true convolution, which the caller gives as the flipped kernel and its own centre).
///
/// Built for speed with the same bits: a row of x at once, every voxel's taps added in the
/// kernel's order (z slowest, x fastest; the taps outside the image left out, and the kernel's
/// zeros too when the volume is finite: a sum from +0 never changes by a ±0 product); a voxel
/// whose sum is NaN is computed again by x86-64's rules, so NaN bits do not depend on the
/// platform. A kernel holding a NaN or an infinity (whose taps outside the image add 0 times it)
/// is summed voxel by voxel.
fn direct_volume<T: Num>(
    src: &[T],
    dims: [usize; 3],
    k: &Kernel,
    centre: [i64; 3],
    fix: bool,
) -> (Vec<T>, bool) {
    let [nx, ny, nz] = dims;
    let [kx, ky, kz] = k.size();
    let [cx, cy, cz] = centre;
    let slow = k.non_finite();
    let double = T::KIND == super::num::Kind::Double;
    // one voxel by x86-64's rules
    let exact = |x: usize, y: usize, z: usize| -> f32 {
        let mut sum = 0.0f32;
        for l in 0..kz {
            let pz = z as i64 + l as i64 - cz;
            let zin = pz >= 0 && pz < nz as i64;
            for j in 0..ky {
                let py = y as i64 + j as i64 - cy;
                let yin = py >= 0 && py < ny as i64;
                let row = (j + ky * l) * kx;
                for i in 0..kx {
                    let px = x as i64 + i as i64 - cx;
                    let inside = zin && yin && px >= 0 && px < nx as i64;
                    let kv = k.data[row + i];
                    let s = if inside {
                        src[px as usize + nx * (py as usize + ny * pz as usize)]
                    } else if slow {
                        T::zero()
                    } else {
                        continue;
                    };
                    if double {
                        let (a, b) = (s.to_f64(), f64::from(kv));
                        sum = num::acc32(sum, num::nan64(a, b, a * b));
                    } else {
                        let a = num::to_f32(s.to_f64());
                        let p = num::nan32(a, kv, a * kv);
                        sum = num::nan32(sum, p, sum + p);
                    }
                }
            }
        }
        sum
    };
    let mut out = vec![T::zero(); src.len()];
    let convert = |o: &mut [T], sums: &mut dyn FnMut(usize) -> f32| -> bool {
        let mut ch = false;
        for (x, ov) in o.iter_mut().enumerate() {
            let (t, c) = T::convert(num::to_f64(sums(x)), fix);
            ch |= c;
            *ov = t;
        }
        ch
    };
    if slow {
        let changed = out
            .par_chunks_mut(nx.max(1))
            .enumerate()
            .map(|(row, o)| convert(o, &mut |x| exact(x, row % ny, row / ny)))
            .reduce(|| false, |a, b| a || b);
        return (out, changed);
    }
    let finite = !src.par_iter().any(|v| v.non_finite());
    // the kernel's taps per (j, l) row, in order: (i, value), its zeros left out when the volume
    // is finite
    let rows: Vec<TapRow> = (0..kz)
        .flat_map(|l| (0..ky).map(move |j| (j, l)))
        .map(|(j, l)| {
            let taps = (0..kx)
                .map(|i| (i as i64, k.data[(j + ky * l) * kx + i]))
                .filter(|&(_, v)| !(finite && v == 0.0))
                .collect();
            (j as i64, l as i64, taps)
        })
        .collect();
    let changed = out
        .par_chunks_mut(nx.max(1))
        .enumerate()
        .map_init(
            || vec![0.0f32; nx],
            |acc, (row, o)| {
                let (y, z) = ((row % ny) as i64, (row / ny) as i64);
                acc.fill(0.0);
                for (j, l, taps) in &rows {
                    let (py, pz) = (y + j - cy, z + l - cz);
                    if py < 0 || py >= ny as i64 || pz < 0 || pz >= nz as i64 {
                        continue;
                    }
                    let srow = &src[nx * (py as usize + ny * pz as usize)..][..nx];
                    for &(i, kv) in taps {
                        // voxel x reads srow[x + d]: x in [xa, xb)
                        let d = i - cx;
                        let xa = (-d).clamp(0, nx as i64) as usize;
                        let xb = (nx as i64 - d).clamp(xa as i64, nx as i64) as usize;
                        if xa == xb {
                            continue;
                        }
                        let s = &srow[(xa as i64 + d) as usize..(xb as i64 + d) as usize];
                        let a = &mut acc[xa..xb];
                        if double {
                            let kd = f64::from(kv);
                            for (a, &v) in a.iter_mut().zip(s) {
                                *a = (f64::from(*a) + v.fast_f64() * kd) as f32;
                            }
                        } else {
                            for (a, &v) in a.iter_mut().zip(s) {
                                *a += (v.fast_f64() as f32) * kv;
                            }
                        }
                    }
                }
                convert(o, &mut |x| {
                    let r = acc[x];
                    if r.is_nan() {
                        exact(x, y as usize, z as usize)
                    } else {
                        r
                    }
                })
            },
        )
        .reduce(|| false, |a, b| a || b);
    (out, changed)
}

/// The kernel flipped along every axis (FSL's FFT path convolves; the direct sum correlates).
fn flipped(k: &Kernel) -> Kernel {
    let [nx, ny, nz] = k.size();
    let mut f = k.clone();
    for z in 0..nz {
        for y in 0..ny {
            for x in 0..nx {
                f.data[x + nx * (y + ny * z)] = k.at(nx - 1 - x, ny - 1 - y, nz - 1 - z);
            }
        }
    }
    f
}

/// Whether mirroring `k` along every axis (about its centre `(n − 1) / 2`, the mirrored
/// kernel's centre `n − 1 − (n − 1) / 2`) leaves its application unchanged: every size odd and
/// every value equal to its mirror's, bit for bit.
fn point_symmetric(k: &Kernel) -> bool {
    let [nx, ny, nz] = k.size();
    if k.dims.iter().any(|&d| d > 0 && d % 2 == 0) {
        return false;
    }
    (0..nz).all(|z| {
        (0..ny).all(|y| {
            (0..nx).all(|x| {
                k.at(x, y, z).to_bits() == k.at(nx - 1 - x, ny - 1 - y, nz - 1 - z).to_bits()
            })
        })
    })
}

/// The convolution of every volume with `k` (outside the image: 0). Where FSL takes the direct
/// sum: a correlation with the kernel's centre and the warning per volume when a size is even.
/// Where FSL takes its FFT (§10.2), no warning, and two defects of that path:
/// - FSL-192: FSL's FFT is a true convolution (the kernel mirrored), so the kernel's orientation
///   depends on the kernel's and the image's sizes; the default keeps the direct sum's
///   orientation at every size, `reproduce` mirrors the kernel there (named where the two
///   results differ, which the default computes to know);
/// - FSL-191: a volume holding a NaN or infinite voxel is NaN throughout in FSL, which the
///   default corrects (the direct sum keeps the NaN within the kernel's reach).
///
/// In place, volume by volume (one volume's copy held). `extra_warnings`: the off-centre
/// warning printed for that many more volumes (a convolution of ones computed on one volume for
/// a whole series).
fn convolve_in_place<T: Num>(
    img: &mut Img<T>,
    k: &Kernel,
    op: &str,
    notices: &mut Notices,
    err: &mut dyn Write,
    extra_warnings: usize,
) -> bool {
    let fix = notices.fix();
    let (dims, per) = grid(img);
    let fft = fsl_uses_fft(per, k.data.len());
    let warn = k.dims.iter().any(|d| d % 2 == 0) && !fft;
    // FSL-192: the mirrored kernel and its centre, where FSL's FFT would mirror it
    let mirror = (fft && !k.is_empty() && !point_symmetric(k))
        .then(|| (flipped(k), k.dims.map(|n| n - 1 - (n - 1) / 2)));
    let (kk, centre) = match (&mirror, fix) {
        (Some((m, c)), false) => (m.clone(), *c),
        _ => (k.clone(), k.centre()),
    };
    let mut changed = false;
    let mut poisoned = false;
    let mut mirrored = false;
    for t in 0..img.dims[3] {
        if warn {
            let _ = writeln!(err, "{OFF_CENTRE_CONV}");
        }
        let vol = &img.data[t * per..(t + 1) * per];
        let bad = fft && !k.is_empty() && vol.par_iter().any(|v| v.non_finite());
        if bad && !fix {
            // FSL's FFT: the volume NaN throughout (a NaN voxel's own NaN, else the default)
            let nan = vol
                .iter()
                .find(|v| v.is_nan())
                .map_or_else(num::default_nan, |v| num::quiet(v.to_f64()));
            let (t_nan, c) = T::convert(nan, fix);
            changed |= c;
            img.data[t * per..(t + 1) * per].fill(t_nan);
            poisoned = true;
            continue;
        }
        let (o, ch) = direct_volume(vol, dims, &kk, centre, fix);
        if let Some((m, c)) = &mirror {
            // named where FSL's mirrored kernel gives another result (reproduce: it is that)
            mirrored |= !fix || {
                let (r, _) = direct_volume(vol, dims, m, *c, fix);
                o.par_iter().zip(r.par_iter()).any(|(a, b)| !a.same(*b))
            };
        }
        if bad {
            // named where the local result differs from FSL's NaN volume
            let nan = vol
                .iter()
                .find(|v| v.is_nan())
                .map_or_else(num::default_nan, |v| num::quiet(v.to_f64()));
            let t_nan = T::from_f64_c(nan);
            poisoned |= o.par_iter().any(|v| !v.same(t_nan));
        }
        changed |= ch;
        img.data[t * per..(t + 1) * per].copy_from_slice(&o);
    }
    if warn {
        for _ in 0..extra_warnings {
            let _ = writeln!(err, "{OFF_CENTRE_CONV}");
        }
    }
    if mirrored {
        let detail = match notices.mode() {
            UpstreamDefects::Fix => format!(
                "{op} with a kernel FSL convolves by its FFT: the kernel applied as stored, as \
                 FSL's direct sum applies it for smaller kernels (FSL's FFT mirrors it along \
                 every axis)"
            ),
            UpstreamDefects::Reproduce => format!(
                "{op} with a kernel FSL convolves by its FFT: the kernel mirrored along every \
                 axis, as FSL's FFT applies it"
            ),
        };
        notices.add(&defects::FSL_192, detail);
    }
    if poisoned {
        let detail = match notices.mode() {
            UpstreamDefects::Fix => format!(
                "{op} with a kernel FSL convolves by its FFT: the NaN or infinite voxels act \
                 within the kernel's reach only (FSL makes their whole volume NaN)"
            ),
            UpstreamDefects::Reproduce => format!(
                "{op} with a kernel FSL convolves by its FFT: a volume holding a NaN or infinite \
                 voxel made NaN throughout, as FSL does"
            ),
        };
        notices.add(&defects::FSL_191, detail);
    }
    changed
}

/// The kernel divided by the sum of its values (a double sum rounded to single precision, as
/// black-box runs show) when that sum is not 0.
fn normalised(k: &Kernel) -> Kernel {
    let s = k.sum64() as f32;
    let mut n = k.clone();
    if s != 0.0 {
        for v in &mut n.data {
            *v = num::nan32(*v, s, *v / s);
        }
    }
    n
}

/// `-fmean` (`renormalise`) and `-fmeanu`: the kernel-weighted mean, separable after a box or
/// Gaussian kernel (`-fmean` only), else by the direct sum with the normalised kernel; `-fmean`
/// then divides by the same convolution of ones.
pub fn fmean<T: Num>(
    img: &mut Img<T>,
    k: &Kernel,
    renormalise: bool,
    op: &str,
    notices: &mut Notices,
    err: &mut dyn Write,
) -> Result<(), Stop> {
    refuse_empty(notices, op, k)?;
    note_zero_width(notices, op, k);
    let fix = notices.fix();
    let kc = k.for_convolution(fix);
    let k = kc.as_ref();
    if renormalise && k.separable {
        let ch = separable(img, k, notices, err);
        super::ops::note151(notices, ch, "a filter's results");
        return Ok(());
    }
    if k.is_empty() {
        let _ = writeln!(err, "{EMPTY_MASK}");
    }
    let kn = normalised(k);
    let mut changed = convolve_in_place(img, &kn, op, notices, err, 0);
    if renormalise {
        // the convolution of ones: the same for every volume, so computed on one (its
        // messages printed for each)
        let [nx, ny, nz, nt] = img.dims;
        let per = nx * ny * nz;
        let mut ones = Img {
            dims: [nx, ny, nz, 1],
            data: vec![T::one(); per],
            header: img.header.clone(),
            flipped: img.flipped,
            cal: img.cal,
            own_quaternion: img.own_quaternion,
        };
        changed |= convolve_in_place(&mut ones, &kn, op, notices, err, nt.saturating_sub(1));
        let den = ones.data;
        if T::KIND.is_int() {
            if den.par_iter().any(|&d| d == T::zero()) {
                return Err(Stop::DivZero(
                    "-fmean's renormalisation divides by 0 under an integer type",
                ));
            }
            if img.data.par_chunks(per.max(1)).any(|vol| {
                vol.iter()
                    .zip(&den)
                    .any(|(&a, &d)| d.to_f64() == -1.0 && a.to_f64() == T::MIN)
            }) {
                return Err(Stop::DivZero("the most negative int divided by -1"));
            }
        }
        img.data.par_chunks_mut(per.max(1)).for_each(|vol| {
            for (v, &d) in vol.iter_mut().zip(&den) {
                *v = v.div(d);
            }
        });
    }
    super::ops::note151(notices, changed, "a filter's results");
    Ok(())
}

/// `-s σ`'s filter with its kernel (the Gaussian already built).
pub fn smooth<T: Num>(
    img: &mut Img<T>,
    k: &Kernel,
    notices: &mut Notices,
    err: &mut dyn Write,
) -> Result<(), Stop> {
    refuse_empty(notices, "-s", k)?;
    note_zero_width(notices, "-s", k);
    let fix = notices.fix();
    let kc = k.for_convolution(fix);
    let ch = separable(img, kc.as_ref(), notices, err);
    super::ops::note151(notices, ch, "a filter's results");
    Ok(())
}

/// FSL-160 named where a zero-width Gaussian is used to convolve.
fn note_zero_width(notices: &mut Notices, op: &str, k: &Kernel) {
    if !k.zero_width {
        return;
    }
    let detail = match notices.mode() {
        UpstreamDefects::Fix => format!(
            "{op} with a Gaussian of zero width: the unit impulse (FSL's kernel is 0/0 at its \
             centre and makes every voxel NaN)"
        ),
        UpstreamDefects::Reproduce => format!(
            "{op} with a Gaussian of zero width: its centre 0/0 makes every voxel NaN, as FSL does"
        ),
    };
    notices.add(&defects::FSL_160, detail);
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    #[test]
    fn armadillo_extremes_and_partial_sums() {
        assert_eq!(extreme(&[1.0, 5.0, 3.0], true), 5.0);
        assert_eq!(extreme(&[f64::NAN, f64::NAN], true), f64::NEG_INFINITY);
        assert!(extreme(&[0.0, -0.0], true).is_sign_negative());
        assert_eq!(extreme(&[2.0, f64::NAN, -1.0], false), -1.0);
        assert_eq!(two_sums(&[1.0, 2.0, 3.0]), 6.0);
        assert_eq!(mode(&mut [1.0, 2.0, 2.0, 3.0, 3.0]), 3.0);
        assert_eq!(median(&mut [4.0, 1.0, 3.0, 2.0]), 3.0);
        assert_eq!(median(&mut [4.0, 1.0, 3.0]), 3.0);
        assert_eq!(median(&mut [7.0]), 7.0);
    }

    #[test]
    fn fsl_takes_the_fft_for_large_kernels() {
        assert!(fsl_uses_fft(210, 1287));
        assert!(!fsl_uses_fft(210, 175));
        assert!(!fsl_uses_fft(2288, 960));
        assert!(fsl_uses_fft(2288, 1287));
    }

    pub(crate) fn img(dims: [usize; 4], data: Vec<f32>) -> Img<f32> {
        use larmorx_core::element::DataType;
        use larmorx_io::nifti::{NiftiHeader, NiftiVersion};
        let header = NiftiHeader::new(NiftiVersion::V1, &dims[..3], DataType::F32).unwrap();
        Img {
            dims,
            data,
            header,
            flipped: false,
            cal: (0.0, 0.0),
            own_quaternion: false,
        }
    }

    fn notices(mode: UpstreamDefects) -> Notices {
        Notices::new(mode)
    }

    /// Values with NaN (two payloads, both signs), infinities, zeros of both signs, from a
    /// seeded generator.
    fn awkward(n: usize, seed: u64) -> Vec<f32> {
        let mut x = seed;
        (0..n)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                match x % 97 {
                    0 => f32::from_bits(0x7fc0_1234),
                    1 => f32::from_bits(0xffa0_0001),
                    2 => f32::INFINITY,
                    3 => f32::NEG_INFINITY,
                    4 => -0.0,
                    5 => 0.0,
                    r => (r as f32 - 48.0) * 0.37,
                }
            })
            .collect()
    }

    /// The separable pass and the direct sum built for speed give the bits of the plain
    /// per-voxel sums (x86-64's NaN rules), at any thread count.
    #[test]
    fn the_fast_sums_are_the_plain_sums() {
        let dims = [13usize, 7, 6];
        let n = dims.iter().product::<usize>();
        for (seed, finite) in [(7u64, false), (11, true)] {
            let mut src = awkward(n, seed);
            if finite {
                src.iter_mut()
                    .filter(|v| !v.is_finite())
                    .for_each(|v| *v = 1.5);
            }
            // the separable pass, each axis, an odd and an even line
            for line in [
                vec![0.25f64, 0.5, 1.0, 0.5, 0.25],
                vec![1.0, -2.0, 0.5, 3.0],
            ] {
                let c = (line.len() as i64 - 1) / 2;
                for axis in 0..3 {
                    let stride = [1, dims[0], dims[0] * dims[1]][axis];
                    let len = dims[axis] as i64;
                    let want: Vec<f32> = (0..n)
                        .map(|idx| {
                            let p = ((idx / stride) % dims[axis]) as i64;
                            let base = idx - p as usize * stride;
                            let (mut sum, mut w) = (0.0f32, 0.0f32);
                            for (i, &kv) in line.iter().enumerate() {
                                let q = p + i as i64 - c;
                                if q >= 0 && q < len {
                                    let s = num::to_f64(src[base + q as usize * stride]);
                                    sum = num::acc32(sum, num::nan64(s, kv, s * kv));
                                    w = num::acc32(w, kv);
                                }
                            }
                            if f64::from(w).abs() > 1e-12 {
                                num::nan32(sum, w, sum / w)
                            } else {
                                sum
                            }
                        })
                        .collect();
                    for threads in [1, 3] {
                        let (got, _) = larmorx_core::parallel::with_threads(threads, || {
                            separable_axis(&src, dims, axis, &line, c, true)
                        })
                        .unwrap();
                        assert!(
                            got.iter()
                                .zip(&want)
                                .all(|(a, b)| a.to_bits() == b.to_bits()),
                            "separable axis {axis}, {threads} threads"
                        );
                    }
                }
            }
            // the direct sum, a kernel with zeros, a centre off the middle
            let kd = [4usize, 3, 2];
            let values: Vec<f32> = (0..24).map(|v| [0.0, 0.5, -1.25, 2.0][v % 4]).collect();
            let k = Kernel::from_values(kd, values);
            let centre = [1i64, 2, 0];
            let want: Vec<f32> = (0..n)
                .map(|idx| {
                    let (x, y, z) = (idx % 13, (idx / 13) % 7, idx / 91);
                    let mut sum = 0.0f32;
                    for l in 0..2 {
                        for j in 0..3 {
                            for i in 0..4 {
                                let (px, py, pz) = (
                                    x as i64 + i - centre[0],
                                    y as i64 + j - centre[1],
                                    z as i64 + l - centre[2],
                                );
                                if px < 0 || py < 0 || pz < 0 || px >= 13 || py >= 7 || pz >= 6 {
                                    continue;
                                }
                                let a = src[(px + 13 * (py + 7 * pz)) as usize];
                                let kv = k.data[(i + 4 * (j + 3 * l)) as usize];
                                let p = num::nan32(a, kv, a * kv);
                                sum = num::nan32(sum, p, sum + p);
                            }
                        }
                    }
                    sum
                })
                .collect();
            for threads in [1, 3] {
                let (got, _) = larmorx_core::parallel::with_threads(threads, || {
                    direct_volume(&src, dims, &k, centre, true)
                })
                .unwrap();
                assert!(
                    got.iter()
                        .zip(&want)
                        .all(|(a, b)| a.to_bits() == b.to_bits()),
                    "direct sum, finite {finite}, {threads} threads"
                );
            }
        }
    }

    #[test]
    fn the_window_counts_nan_and_overflows_past_its_count() {
        // a kernel value of exactly 0.5 counts but never enters; NaN counts; +inf enters only
        let k = Kernel::from_values([3, 1, 1], vec![0.5, f32::NAN, f32::INFINITY]);
        let w = window(&k);
        assert_eq!((w.offs.len(), w.count), (1, 2));
        let k = Kernel::from_values([3, 1, 1], vec![1.0, 1.0, f32::INFINITY]);
        let mut im = img([5, 1, 1, 1], vec![1.0, 2.0, 3.0, 4.0, 5.0]);
        let mut err = Vec::new();
        let r = morph(
            &mut im,
            &k,
            Morph::DilF,
            "-dilF",
            &mut notices(UpstreamDefects::Fix),
            &mut err,
        );
        assert!(matches!(r, Err(Stop::Exception(ref t)) if t.contains("std::out_of_range")));
        // an even extent warns once per volume
        let k = Kernel::ones([2, 1, 1], true);
        let mut im = img([3, 1, 1, 2], vec![1.0, 5.0, 2.0, 0.0, 0.0, 7.0]);
        let mut err = Vec::new();
        morph(
            &mut im,
            &k,
            Morph::DilF,
            "-dilF",
            &mut notices(UpstreamDefects::Fix),
            &mut err,
        )
        .unwrap();
        assert_eq!(im.data, vec![5.0, 5.0, 2.0, 0.0, 7.0, 7.0]);
        assert_eq!(
            String::from_utf8(err)
                .unwrap()
                .matches(OFF_CENTRE_MORPH)
                .count(),
            2
        );
    }

    #[test]
    fn fsl_162_in_both_modes() {
        let k = Kernel::ones([3, 1, 1], false);
        let data = vec![1.0, f32::NAN, 3.0, 4.0];
        let mut im = img([4, 1, 1, 1], data.clone());
        let mut err = Vec::new();
        let mut rep = notices(UpstreamDefects::Reproduce);
        let r = morph(&mut im, &k, Morph::Median, "-fmedian", &mut rep, &mut err);
        assert!(matches!(r, Err(Stop::Exception(ref t)) if t.contains("sort(): detected NaN")));
        let mut im = img([4, 1, 1, 1], data);
        let mut fix = notices(UpstreamDefects::Fix);
        morph(&mut im, &k, Morph::Median, "-fmedian", &mut fix, &mut err).unwrap();
        // windows without the NaN: [1] -> 1, [3] -> 3, [3, 4] -> upper median 4
        assert_eq!(im.data[0], 1.0);
        assert!(im.data[1] == 3.0 && im.data[2] == 4.0 && im.data[3] == 4.0);
        assert!(fix.contains(&defects::FSL_162));
    }

    #[test]
    fn dilall_fills_in_queue_order() {
        let mut im = img([4, 1, 1, 1], vec![0.0, 2.0, 0.0, 0.0]);
        dilall(&mut im, &mut notices(UpstreamDefects::Fix));
        assert_eq!(im.data, vec![2.0, 2.0, 2.0, 2.0]);
        let mut im = img([3, 1, 1, 1], vec![1.0, 0.0, 4.0]);
        dilall(&mut im, &mut notices(UpstreamDefects::Fix));
        assert_eq!(im.data, vec![1.0, 2.5, 4.0]);
    }

    #[test]
    fn fsl_192_the_fft_path_mirrors_the_kernel_and_fsl_191() {
        let conv = |im: &Img<f32>, k: &Kernel, n: &mut Notices, err: &mut Vec<u8>| {
            let mut im = im.clone();
            convolve_in_place(&mut im, k, "-fmeanu", n, err, 0);
            im.data
        };
        // a 64 x 64 impulse and a 41 x 41 kernel: FSL's FFT path, a true convolution
        let (n, m) = (64, 41);
        let mut data: Vec<f32> = vec![0.0; n * n];
        data[32 + n * 32] = 1.0;
        let values = (0..m * m)
            .map(|v| (1 + v % m + m * (v / m)) as f32)
            .collect();
        let big = Kernel::from_values([m, m, 1], values);
        let im = img([n, n, 1, 1], data);
        let mut err = Vec::new();
        assert!(fsl_uses_fft(n * n, m * m));
        let at = |x: usize, y: usize| x + n * y;
        // reproduce: kernel voxel (i, j) at (32 - 20 + i, 32 - 20 + j) (FSL's FFT)
        let mut rep = notices(UpstreamDefects::Reproduce);
        let o = conv(&im, &big, &mut rep, &mut err);
        assert!(o[at(12, 12)] == 1.0 && o[at(52, 52)] == 1681.0);
        // the default: the correlation of FSL's direct sum, at (32 + 20 - i, 32 + 20 - j)
        let mut fix = notices(UpstreamDefects::Fix);
        let o = conv(&im, &big, &mut fix, &mut err);
        assert!(o[at(52, 52)] == 1.0 && o[at(12, 12)] == 1681.0);
        assert!(fix.contains(&defects::FSL_192) && rep.contains(&defects::FSL_192));
        // a point-symmetric kernel: the same either way, not named
        let mut fix = notices(UpstreamDefects::Fix);
        conv(
            &im,
            &Kernel::ones([m as i64, m as i64, 1], false),
            &mut fix,
            &mut err,
        );
        assert!(!fix.contains(&defects::FSL_192));
        // an image the orientation does not change (zeros): not named in the default
        let zeros = img([n, n, 1, 1], vec![0.0; n * n]);
        let mut fix = notices(UpstreamDefects::Fix);
        conv(&zeros, &big, &mut fix, &mut err);
        assert!(!fix.contains(&defects::FSL_192));
        // FSL-191: a NaN on the FFT path poisons the volume only with reproduce
        let w = 64 * 64;
        let mut data = vec![1.0f32; w];
        data[0] = f32::NAN;
        let im = img([64, 64, 1, 1], data);
        let k = Kernel::ones([41, 41, 1], false);
        assert!(fsl_uses_fft(w, 41 * 41));
        let mut rep = notices(UpstreamDefects::Reproduce);
        let o = conv(&im, &k, &mut rep, &mut err);
        assert!(o.iter().all(|v| v.is_nan()));
        let mut fix = notices(UpstreamDefects::Fix);
        let o = conv(&im, &k, &mut fix, &mut err);
        assert!(o[w - 1] == 441.0 && o[0].is_nan());
        assert!(fix.contains(&defects::FSL_191) && rep.contains(&defects::FSL_191));
    }

    #[test]
    fn nan_from_numbers_is_x86_64s_default() {
        let n = num::nan32(
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::INFINITY + f32::NEG_INFINITY,
        );
        assert_eq!(n.to_bits(), num::DEFAULT_NAN_32);
        let q = f32::from_bits(0x7fa0_0001);
        assert_eq!(num::nan32(1.0, q, 1.0 + q).to_bits(), 0x7fe0_0001);
        assert_eq!(0.0f32.mul(f32::INFINITY).to_bits(), num::DEFAULT_NAN_32);
    }
}
