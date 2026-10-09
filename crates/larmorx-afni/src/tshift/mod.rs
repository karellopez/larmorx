//! Slice-timing correction: AFNI's `3dTshift` (`specs/3dTshift.md`).
//!
//! Every voxel's series is detrended, shifted in time so that its slice refers to the common
//! origin `tzero`, clipped to its own range, and given its trend back. Voxels are independent,
//! so the work runs in parallel on fixed blocks of voxels and the result does not depend on
//! the thread count.

pub mod series;

use larmorx_core::parallel::{self, ThreadPoolError};
use rayon::prelude::*;

use crate::dataset::{BrickData, Bricks, Datum, extract, store};
use crate::fft::Fft;
use series::{FourierWork, Lagrange, Removal};

/// How the series are resampled in time.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Method {
    /// Fourier interpolation (AFNI's default).
    #[default]
    Fourier,
    /// Lagrange polynomials of degree 1, 3, 5 and 7.
    Linear,
    Cubic,
    Quintic,
    Heptic,
    /// Weighted sinc, radius 5 or 9.
    Wsinc5,
    Wsinc9,
}

impl Method {
    /// `fourier`, `linear`, `cubic`, `quintic`, `heptic`, `wsinc5` or `wsinc9`.
    pub fn from_name(name: &str) -> Option<Method> {
        Some(match name {
            "fourier" => Method::Fourier,
            "linear" => Method::Linear,
            "cubic" => Method::Cubic,
            "quintic" => Method::Quintic,
            "heptic" => Method::Heptic,
            "wsinc5" => Method::Wsinc5,
            "wsinc9" => Method::Wsinc9,
            _ => return None,
        })
    }
}

/// What is added back after shifting a detrended series.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Restore {
    /// The linear trend, then the result is clipped to the input's range (AFNI's default).
    #[default]
    Trend,
    /// Nothing (`-rlt`).
    None,
    /// The fitted intercept, the trend's value at the first time point used (`-rlt+`).
    Intercept,
}

/// Parameters of [`tshift`]. Times are in any one unit (seconds, usually).
#[derive(Clone, Debug, PartialEq)]
pub struct TshiftParams {
    /// Repetition time (> 0).
    pub tr: f32,
    /// Acquisition time of each slice within the TR (third axis).
    pub slice_times: Vec<f32>,
    /// The common time origin (`tzero`).
    pub tzero: f32,
    /// Leading time points left out of the fit and the shift (they are only re-stored).
    pub ignore: usize,
    pub method: Method,
    pub restore: Restore,
    /// Remove the linear trend before shifting (true, AFNI's default). `false` is
    /// `-no_detrend`, reproduced as AFNI 25.2.09 behaves (observed): within each slice, voxels
    /// are taken in pairs in index order; the first of each pair has its mean removed and
    /// restored, the second is shifted as raw values. Needs `restore = Trend`.
    pub detrend: bool,
    /// Worker threads (0 = all logical CPUs). The result does not depend on it.
    pub n_threads: usize,
}

/// [`tshift`] was given impossible parameters.
#[derive(Debug, thiserror::Error)]
pub enum TshiftError {
    #[error("{times} slice times for {slices} slices")]
    SliceTimes { times: usize, slices: usize },
    #[error("the TR must be positive, not {0}")]
    Tr(f32),
    #[error("-ignore value {ignore} is too large for {nt} time points (at most nt - 5)")]
    Ignore { ignore: usize, nt: usize },
    #[error("without detrending, the mean is restored (restore must be 'trend')")]
    RestoreWithoutDetrend,
    #[error("a slice time or tzero is not finite")]
    NotFinite,
    #[error(transparent)]
    Threads(#[from] ThreadPoolError),
}

/// The shift of each slice in samples, in AFNI's sign: `s_k = −(tzero − t_k)/TR` (float32).
/// The output at time point `n` estimates the input at `n − s_k`.
pub fn slice_shifts(params: &TshiftParams) -> Vec<f32> {
    params
        .slice_times
        .iter()
        .map(|&t| -((params.tzero - t) / params.tr))
        .collect()
}

/// Shifts every voxel of `bricks` in place (`specs/3dTshift.md` §4–§5).
///
/// Slices whose shift is below 0.001 samples are left exactly as stored when the default
/// restore is in effect. A series of fewer than two time points is left unchanged.
pub fn tshift(bricks: &mut Bricks, params: &TshiftParams) -> Result<(), TshiftError> {
    let [_, _, nz, nt] = bricks.shape;
    if params.slice_times.len() != nz {
        return Err(TshiftError::SliceTimes {
            times: params.slice_times.len(),
            slices: nz,
        });
    }
    if params.tr.is_nan() || params.tr <= 0.0 || params.tr.is_infinite() {
        return Err(TshiftError::Tr(params.tr));
    }
    if !params.tzero.is_finite() || params.slice_times.iter().any(|t| !t.is_finite()) {
        return Err(TshiftError::NotFinite);
    }
    if !params.detrend && params.restore != Restore::Trend {
        return Err(TshiftError::RestoreWithoutDetrend);
    }
    if nt < 2 {
        return Ok(());
    }
    if params.ignore + 5 > nt {
        return Err(TshiftError::Ignore {
            ignore: params.ignore,
            nt,
        });
    }
    let shifts = slice_shifts(params);
    let kernels: Vec<Option<Kernel>> = shifts.iter().map(|&s| Kernel::new(s, params, nt)).collect();
    let factors = bricks.factors.clone();
    let shape = bricks.shape;
    parallel::with_threads(params.n_threads, || match &mut bricks.data {
        BrickData::Byte(v) => run(v, shape, &factors, &kernels, params),
        BrickData::Short(v) => run(v, shape, &factors, &kernels, params),
        BrickData::Float(v) => run(v, shape, &factors, &kernels, params),
    })?;
    Ok(())
}

/// What is done to the series of one slice.
enum Kernel {
    Fourier {
        fft: Fft,
        response: Vec<larmorx_core::Complex<f64>>,
        /// The shift is longer than the series: the residual becomes 0.
        zero: bool,
    },
    Lagrange(Lagrange),
    Wsinc {
        s: f32,
        radius: i64,
    },
}

impl Kernel {
    /// The kernel for a slice shifted by `s`, or `None` if the slice is left as stored.
    fn new(s: f32, params: &TshiftParams, nt: usize) -> Option<Kernel> {
        if params.restore == Restore::Trend && params.detrend && s.abs() < 0.001 {
            return None;
        }
        let m = nt - params.ignore;
        Some(match params.method {
            Method::Fourier => {
                let len = series::fourier_length(nt);
                Kernel::Fourier {
                    fft: Fft::new(len).expect("Fourier lengths factor into 2, 3 and 5"),
                    response: series::fourier_response(len, s),
                    zero: s.abs() > m as f32,
                }
            }
            Method::Wsinc5 => Kernel::Wsinc { s, radius: 5 },
            Method::Wsinc9 => Kernel::Wsinc { s, radius: 9 },
            method => Kernel::Lagrange(Lagrange::new(method, s)),
        })
    }
}

/// Rows per parallel task: tasks are a fixed partition of the voxels, so results do not
/// depend on the thread count.
const ROWS_PER_TASK: usize = 4;

fn run<T: Datum>(
    data: &mut [T],
    [nx, ny, nz, nt]: [usize; 4],
    factors: &[f32],
    kernels: &[Option<Kernel>],
    params: &TshiftParams,
) {
    if data.is_empty() {
        return;
    }
    // Split every volume into blocks of rows, and give each task its block in every volume.
    let block = nx * ROWS_PER_TASK;
    let blocks_per_slice = ny.div_ceil(ROWS_PER_TASK);
    let mut tasks: Vec<Vec<&mut [T]>> = (0..nz * blocks_per_slice)
        .map(|_| Vec::with_capacity(nt))
        .collect();
    for (plane, values) in data.chunks_mut(nx * ny).enumerate() {
        let z = plane % nz;
        for (b, chunk) in values.chunks_mut(block).enumerate() {
            tasks[z * blocks_per_slice + b].push(chunk);
        }
    }
    tasks
        .into_par_iter()
        .enumerate()
        .for_each(|(task, chunks)| {
            if let Some(kernel) = &kernels[task / blocks_per_slice] {
                process_block(chunks, factors, kernel, params);
            }
        });
}

/// Shifts the voxels of one block: `chunks[t]` holds the block's values at time point `t`.
fn process_block<T: Datum>(
    mut chunks: Vec<&mut [T]>,
    factors: &[f32],
    kernel: &Kernel,
    params: &TshiftParams,
) {
    let nt = chunks.len();
    let voxels = chunks[0].len();
    // Gather each voxel's series contiguously.
    let mut buf = vec![0f32; voxels * nt];
    for (t, chunk) in chunks.iter().enumerate() {
        for (v, &x) in chunk.iter().enumerate() {
            buf[v * nt + t] = extract(x, factors[t]);
        }
    }
    let skip = params.ignore;
    // Without detrending, AFNI removes the mean from the first voxel of each pair (in index
    // order within the slice) and nothing from the second (observed). Blocks start at an even
    // index within their slice, so the parity within the block is the parity in the slice.
    let removal = |voxel: usize| {
        if params.detrend {
            Removal::Trend
        } else if voxel.is_multiple_of(2) {
            Removal::Mean
        } else {
            Removal::Nothing
        }
    };
    let mut scratch = Vec::new();
    match kernel {
        Kernel::Fourier {
            fft,
            response,
            zero,
        } => {
            let mut work = FourierWork::new(fft.len());
            // Two voxels share one complex transform (real and imaginary parts).
            for pair in buf.chunks_mut(2 * nt) {
                let (first, second) = pair.split_at_mut(nt);
                let x = &mut first[skip..];
                let dx = series::detrend(x, removal(0));
                if second.is_empty() {
                    if *zero {
                        x.fill(0.0);
                    } else {
                        series::fourier_shift(x, None, fft, response, &mut work);
                    }
                    series::retrend(x, &dx, params.restore);
                    continue;
                }
                let y = &mut second[skip..];
                let dy = series::detrend(y, removal(1));
                if *zero {
                    x.fill(0.0);
                    y.fill(0.0);
                } else {
                    series::fourier_shift(x, Some(y), fft, response, &mut work);
                }
                series::retrend(x, &dx, params.restore);
                series::retrend(y, &dy, params.restore);
            }
        }
        Kernel::Lagrange(lagrange) => {
            for (voxel, s) in buf.chunks_mut(nt).enumerate() {
                let x = &mut s[skip..];
                let d = series::detrend(x, removal(voxel));
                lagrange.apply(x, &mut scratch);
                series::retrend(x, &d, params.restore);
            }
        }
        Kernel::Wsinc { s, radius } => {
            for (voxel, v) in buf.chunks_mut(nt).enumerate() {
                let x = &mut v[skip..];
                let d = series::detrend(x, removal(voxel));
                series::wsinc(x, *s, *radius, &mut scratch);
                series::retrend(x, &d, params.restore);
            }
        }
    }
    for (t, chunk) in chunks.iter_mut().enumerate() {
        for (v, x) in chunk.iter_mut().enumerate() {
            *x = store(buf[v * nt + t], factors[t]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timing::{Pattern, mean_time};

    fn dataset(shape: [usize; 4]) -> Bricks {
        let n: usize = shape.iter().product();
        let mut state = 12345u64;
        let data = (0..n)
            .map(|i| {
                state = state
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                1000.0 + (i % 17) as f32 + ((state >> 40) as f32 / (1u64 << 24) as f32) * 50.0
            })
            .collect();
        Bricks::new(shape, BrickData::Float(data), vec![0.0; shape[3]]).unwrap()
    }

    fn params(nz: usize, method: Method, n_threads: usize) -> TshiftParams {
        let slice_times = Pattern::AltPlus.times(nz, 2.0);
        TshiftParams {
            tr: 2.0,
            tzero: mean_time(&slice_times),
            slice_times,
            ignore: 0,
            method,
            restore: Restore::Trend,
            detrend: true,
            n_threads,
        }
    }

    #[test]
    fn results_do_not_depend_on_the_thread_count() {
        for method in [Method::Fourier, Method::Heptic, Method::Wsinc9] {
            let mut one = dataset([7, 9, 5, 40]);
            let mut many = one.clone();
            tshift(&mut one, &params(5, method, 1)).unwrap();
            tshift(&mut many, &params(5, method, 4)).unwrap();
            assert_eq!(one, many, "{method:?}");
        }
    }

    #[test]
    fn unshifted_slices_are_left_as_stored() {
        let original = dataset([3, 3, 2, 20]);
        let mut shifted = original.clone();
        let mut p = params(2, Method::Fourier, 1);
        p.slice_times = vec![0.0, 1.0];
        p.tzero = 0.0;
        tshift(&mut shifted, &p).unwrap();
        let (BrickData::Float(a), BrickData::Float(b)) = (&original.data, &shifted.data) else {
            unreachable!()
        };
        let nxy = 9;
        for t in 0..20 {
            let base = t * 2 * nxy;
            assert_eq!(a[base..base + nxy], b[base..base + nxy], "slice 0 changed");
            assert_ne!(a[base + nxy..base + 2 * nxy], b[base + nxy..base + 2 * nxy]);
        }
    }

    #[test]
    fn ignored_points_are_kept() {
        let original = dataset([2, 2, 3, 30]);
        let mut shifted = original.clone();
        let mut p = params(3, Method::Cubic, 2);
        p.ignore = 4;
        tshift(&mut shifted, &p).unwrap();
        let (BrickData::Float(a), BrickData::Float(b)) = (&original.data, &shifted.data) else {
            unreachable!()
        };
        assert_eq!(a[..4 * 12], b[..4 * 12]);
        assert_ne!(a[4 * 12..], b[4 * 12..]);
    }

    #[test]
    fn invalid_parameters_are_errors() {
        let mut d = dataset([2, 2, 3, 10]);
        let mut p = params(3, Method::Fourier, 1);
        p.ignore = 6;
        assert!(matches!(
            tshift(&mut d, &p),
            Err(TshiftError::Ignore { .. })
        ));
        let mut p = params(3, Method::Linear, 1);
        p.detrend = false;
        p.restore = Restore::None;
        assert!(matches!(
            tshift(&mut d, &p),
            Err(TshiftError::RestoreWithoutDetrend)
        ));
        let mut p = params(3, Method::Linear, 1);
        p.slice_times.pop();
        assert!(matches!(
            tshift(&mut d, &p),
            Err(TshiftError::SliceTimes { .. })
        ));
        let mut p = params(3, Method::Linear, 1);
        p.tr = 0.0;
        assert!(matches!(tshift(&mut d, &p), Err(TshiftError::Tr(_))));
    }

    #[test]
    fn no_detrend_reproduces_afni_pairs() {
        // Two voxels per slice at levels 1000 and 5000 with one impulse each, -linear
        // -no_detrend, slices at 0 and 1 s of a 2 s TR. The expected values are AFNI
        // 25.2.09's output: the first voxel of the pair has its mean removed (its last point
        // moves toward the mean), the second is shifted raw (zero beyond the end, then clipped
        // to its range).
        let nt = 16;
        let mut data = vec![0f32; 2 * 2 * nt];
        for z in 0..2 {
            for t in 0..nt {
                let base = 2 * (z + 2 * t);
                data[base] = 1000.0 + if t == 8 { 10.0 } else { 0.0 };
                data[base + 1] = 5000.0 + if t == 4 { 10.0 } else { 0.0 };
            }
        }
        let mut b = Bricks::new([2, 1, 2, nt], BrickData::Float(data), vec![0.0; nt]).unwrap();
        let params = TshiftParams {
            tr: 2.0,
            slice_times: vec![0.0, 1.0],
            tzero: 0.5,
            ignore: 0,
            method: Method::Linear,
            restore: Restore::Trend,
            detrend: false,
            n_threads: 1,
        };
        tshift(&mut b, &params).unwrap();
        let BrickData::Float(out) = &b.data else {
            unreachable!()
        };
        let series = |x: usize, z: usize| -> Vec<f32> {
            (0..nt).map(|t| out[x + 2 * (z + 2 * t)]).collect()
        };
        let mut a0 = vec![1000.0; nt];
        (a0[7], a0[8], a0[15]) = (1002.5, 1007.5, 1000.15625);
        let mut b0 = vec![5000.0; nt];
        (b0[3], b0[4]) = (5002.5, 5007.5);
        let mut a1 = vec![1000.0; nt];
        (a1[0], a1[8], a1[9]) = (1000.15625, 1007.5, 1002.5);
        let mut b1 = vec![5000.0; nt];
        (b1[4], b1[5]) = (5007.5, 5002.5);
        assert_eq!(series(0, 0), a0);
        assert_eq!(series(1, 0), b0);
        assert_eq!(series(0, 1), a1);
        assert_eq!(series(1, 1), b1);
    }

    #[test]
    fn integer_storage_round_trips_through_the_factor() {
        let shape = [2, 2, 2, 12];
        let n: usize = shape.iter().product();
        let data: Vec<i16> = (0..n).map(|i| (i * 37 % 200) as i16).collect();
        let mut b = Bricks::new(shape, BrickData::Short(data.clone()), vec![0.25; 12]).unwrap();
        let mut p = params(2, Method::Linear, 1);
        p.ignore = 2;
        tshift(&mut b, &p).unwrap();
        let BrickData::Short(out) = &b.data else {
            unreachable!()
        };
        // Ignored time points come back unchanged.
        assert_eq!(out[..2 * 8], data[..2 * 8]);
    }
}
