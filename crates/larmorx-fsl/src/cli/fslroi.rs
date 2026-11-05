// SPDX-License-Identifier: Apache-2.0
//! `larmorx fsl fslroi`: FSL's `fslroi` (`specs/standard_space_roi.md` §3), a box of voxels
//! and/or a range of volumes cut out of an image, in the image's own type.
//!
//! Built for speed: one conversion of the stored values into the output type (none where the
//! type is kept: the values are moved), then the box copied row by row, the rows in parallel.

use std::io::Write;

use larmorx_core::defects::{self, Notices, UpstreamDefects};
use larmorx_core::element::DataType;
use larmorx_io::nifti::{NiftiHeader, NiftiVersion};
use larmorx_mri::hmc::image::ImageError;
use rayon::prelude::*;

use super::Settings;
use crate::bet::image::Values;
use crate::flirt::mat::atoi;
use crate::maths::image::{self, Img, Stored, WriteError};
use crate::maths::num::Num;

/// The usage text (§3.1), exactly as FSL prints it on standard output.
pub const USAGE: &str = "\nUsage: fslroi <input> <output> <xmin> <xsize> <ymin> <ysize> <zmin> <zsize>\n       fslroi <input> <output> <tmin> <tsize>\n\n       fslroi <input> <output> <xmin> <xsize> <ymin> <ysize> <zmin> <zsize> <tmin> <tsize>\nNote: indexing (in both time and space) starts with 0 not 1! Inputting -1 for a size will set it to the full image extent for that dimension.\n";

/// One axis of the region: the first index and the size as given (`atoi`'s values).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Range {
    pub min: i32,
    pub size: i32,
}

impl Range {
    /// The whole axis (`0 -1`).
    pub const WHOLE: Range = Range { min: 0, size: -1 };

    /// The first and last index on an axis of `n` voxels (§3.2): the last is `min + size − 1`
    /// in 32-bit integers; a last index of `min − 2` (a size of −1) is the axis' end. `None`
    /// when the last is below the first.
    pub fn bounds(self, n: usize) -> Option<(i64, i64)> {
        let last = self.min.wrapping_add(self.size).wrapping_sub(1);
        let last = if last == self.min.wrapping_sub(2) {
            n as i64 - 1
        } else {
            i64::from(last)
        };
        let first = i64::from(self.min);
        (last >= first).then_some((first, last))
    }
}

/// The region of a command line: x, y, z (the file's voxel indices) and t.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Region {
    pub axes: [Range; 4],
}

impl Region {
    /// The region of the numbers after the two names (`None`: another count, the usage).
    pub fn parse(numbers: &[String]) -> Option<Region> {
        let n: Vec<i32> = numbers.iter().map(|w| atoi(w)).collect();
        let r = |k: usize| Range {
            min: n[k],
            size: n[k + 1],
        };
        let axes = match n.len() {
            2 => [Range::WHOLE, Range::WHOLE, Range::WHOLE, r(0)],
            6 => [r(0), r(2), r(4), Range::WHOLE],
            8 => [r(0), r(2), r(4), r(6)],
            _ => return None,
        };
        Some(Region { axes })
    }
}

/// How a run ends early (its messages printed).
enum Failure {
    /// One of the errors FSL reports and then exits 0 after (FSL-215).
    Reported,
    /// Any other exit status.
    Exit(u8),
}

/// Runs fslroi.
pub fn main(args: &[String], settings: &Settings, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let mode = match settings.defects() {
        Ok(m) => m,
        Err(e) => {
            let _ = writeln!(err, "larmorx fsl fslroi: {e}");
            return defects::EXIT_BAD_MODE;
        }
    };
    let mut notices = Notices::new(mode);
    let code = match run(args, settings, &mut notices, out, err) {
        Ok(()) => 0,
        Err(Failure::Exit(c)) => c,
        Err(Failure::Reported) => {
            let detail = match mode {
                UpstreamDefects::Fix => {
                    "the error is reported with exit status 1 (FSL's fslroi exits 0 after it)"
                }
                UpstreamDefects::Reproduce => "exit status 0 after the error, as FSL's fslroi",
            };
            notices.add(&defects::FSL_215, detail);
            u8::from(mode.fix())
        }
    };
    notices.print(err);
    code
}

/// FSL's image layer's two lines for an exception: `Image Exception : #<n> :: <what>`, then the
/// message again (standard error).
fn exception(err: &mut dyn Write, number: u32, what: &str) -> Failure {
    let _ = writeln!(err, "Image Exception : #{number} :: {what}\n{what}");
    Failure::Reported
}

/// An input that cannot be read: FSL's `#63` for a missing image, the datatype's message for
/// one FSL's image layer does not read (exit 255), else the reading's message.
fn read_failure(settings: &Settings, name: &str, e: &ImageError, err: &mut dyn Write) -> Failure {
    let text = e.to_string();
    if text.starts_with("No image files match") || text.contains("No such file") {
        let stem = larmorx_mri::hmc::files::strip_image_extension(name);
        return exception(err, 63, &format!("No image files match: {stem}"));
    }
    if text.starts_with("Multiple possible") {
        let stem = larmorx_mri::hmc::files::strip_image_extension(name);
        return exception(
            err,
            63,
            &format!("Multiple possible filenames detected for basename: {stem}"),
        );
    }
    // a header whose datatype the image layer does not read (RGB, a code FSL does not know)
    let code =
        crate::cli::bet2::resolve_image(settings, name, UpstreamDefects::Fix, &mut Vec::new())
            .ok()
            .and_then(|f| larmorx_io::nifti::read_header(&f.header).ok())
            .map(|h| h.datatype);
    if let Some(code) = code
        && !matches!(
            code,
            2 | 4 | 8 | 16 | 64 | 256 | 512 | 768 | 1024 | 1280 | 1536
        )
    {
        if matches!(code, 32 | 1792 | 2048) {
            let _ = writeln!(err, "COMPLEX not supported as an independent type");
        } else {
            let _ = writeln!(
                err,
                "Datatype {code} is NOT supported - please check your image"
            );
        }
        return Failure::Exit(255);
    }
    exception(
        err,
        22,
        &format!("Failed to read volume {name}\nError : {text}"),
    )
}

fn run(
    args: &[String],
    settings: &Settings,
    notices: &mut Notices,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<(), Failure> {
    let region = match args.len() {
        4 | 8 | 10 => Region::parse(&args[2..]),
        _ => None,
    };
    let Some(region) = region else {
        let _ = out.write_all(USAGE.as_bytes());
        return Err(Failure::Exit(1));
    };
    let (input, output) = (&args[0], &args[1]);
    let n_threads = settings.threads();
    let mode = notices.mode();
    let (stored, ns) = crate::bet::pooled(n_threads, || {
        let mut ns = Vec::new();
        let r = image::read_stored(settings, input, mode, &mut ns, n_threads);
        (r, ns)
    });
    notices.extend(ns);
    let stored = match stored {
        Ok(s) => s,
        Err(e) => return Err(read_failure(settings, input, &e, err)),
    };
    if matches!(stored.stored, DataType::C64 | DataType::C128) {
        let _ = writeln!(err, "COMPLEX not supported as an independent type");
        return Err(Failure::Exit(255));
    }
    let dims = stored.dims;
    let mut bounds = [(0i64, 0i64); 4];
    for (k, b) in bounds.iter_mut().enumerate() {
        match region.axes[k].bounds(dims[k]) {
            Some(x) => *b = x,
            None => return Err(exception(err, 21, "Invalid ROI dimensions")),
        }
    }
    if bounds[3].0 < 0 || bounds[3].1 >= dims[3] as i64 {
        return Err(exception(err, 5, "Out of Bounds (time index)"));
    }
    let header = stored.header.clone();
    let flipped = stored.flipped;
    let int8 = stored.stored == DataType::I8 && stored.scale.is_none();
    let (dims, flipped_values) = (stored.dims, stored.flipped);
    let stored = std::sync::Mutex::new(stored);
    let (values, shape) = crate::bet::pooled(n_threads, || {
        // the closure runs once: the stored values move into the cut
        let mut guard = stored
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        cut_stored(&mut guard, (dims, flipped_values), bounds, mode)
    });
    drop(stored);
    if int8 {
        fsl217(&values, notices);
    }
    // FSL-217 reproduced: the int8 values written as uint8 with the same bytes
    let values = match values {
        Values::I8(v) if mode.reproduce() => Values::U8(v.into_iter().map(|x| x as u8).collect()),
        v => v,
    };
    let mut header = header;
    let flip = flipped.then_some(dims[0]);
    translate(&mut header, [bounds[0].0, bounds[1].0, bounds[2].0], flip);
    let img: Img<u8> = Img {
        dims: shape,
        data: Vec::new(),
        cal: (header.cal_min, header.cal_max),
        header,
        flipped,
        own_quaternion: false,
    };
    match image::write(settings, &img, output, values, mode, notices, n_threads) {
        Ok(()) => Ok(()),
        Err(WriteError::OutputType(m)) => {
            let _ = err.write_all(m.as_bytes());
            Err(Failure::Exit(1))
        }
        Err(WriteError::File(m)) => {
            let _ = writeln!(err, "larmorx: error: fslroi: cannot write {m}");
            Err(Failure::Exit(1))
        }
    }
}

/// FSL-217's notice for an int8 input: by default whenever it is int8 (the output's type is
/// int8 where FSL's is uint8), with `reproduce` where a value is negative.
fn fsl217(values: &Values, notices: &mut Notices) {
    let negative = match values {
        Values::I8(v) => v.par_iter().filter(|&&x| x < 0).count(),
        _ => 0,
    };
    match notices.mode() {
        UpstreamDefects::Fix => notices.add(
            &defects::FSL_217,
            format!(
                "the int8 image is written as int8 with its values ({negative} negative); FSL writes the same bytes as uint8, its negative values becoming 128 to 255"
            ),
        ),
        UpstreamDefects::Reproduce if negative > 0 => notices.add(
            &defects::FSL_217,
            format!(
                "the int8 image written as uint8 with the same bytes, as FSL does: its {negative} negative values become 128 to 255"
            ),
        ),
        UpstreamDefects::Reproduce => {}
    }
}

/// The box `bounds` of `stored` in its output type (§3.2): the values of a volume's voxels
/// inside the image, 0 outside; and the output's dimensions.
fn cut_stored(
    stored: &mut Stored,
    (dims, flipped): ([usize; 4], bool),
    bounds: [(i64, i64); 4],
    mode: UpstreamDefects,
) -> (Values, [usize; 4]) {
    let fix = mode.fix();
    let shape = bounds.map(|(a, b)| (b - a + 1) as usize);
    let g = (dims, flipped, bounds, shape);
    let values = match stored.effective_type() {
        DataType::U8 => Values::U8(cut(&stored.take_values::<u8>(fix).0, g)),
        DataType::I8 => Values::I8(cut(&stored.take_values::<i8>(fix).0, g)),
        DataType::I16 => Values::I16(cut(&stored.take_values::<i16>(fix).0, g)),
        DataType::I32 | DataType::U16 => Values::I32(cut(&stored.take_values::<i32>(fix).0, g)),
        DataType::F64 => Values::F64(cut(&stored.take_values::<f64>(fix).0, g)),
        _ => Values::F32(cut(&stored.take_values::<f32>(fix).0, g)),
    };
    (values, shape)
}

/// The geometry of a cut: the input's dimensions, whether its x is mirrored in memory, the
/// bounds in the file's indices, the output's dimensions.
type Geometry = ([usize; 4], bool, [(i64, i64); 4], [usize; 4]);

/// The box of `data` (the radiological frame: x mirrored when `flipped`), 0 outside the image.
fn cut<T: Num>(data: &[T], (dims, flipped, bounds, shape): Geometry) -> Vec<T> {
    let [nx, ny, nz, _] = dims;
    let [w, h, d, nt] = shape;
    // the box's first x in the frame the values are held in
    let x0 = if flipped {
        nx as i64 - 1 - bounds[0].1
    } else {
        bounds[0].0
    };
    let (y0, z0, t0) = (bounds[1].0, bounds[2].0, bounds[3].0);
    let mut out = vec![T::zero(); w * h * d * nt];
    if out.is_empty() {
        return out;
    }
    // the part of each row inside the image
    let lo = (-x0).clamp(0, w as i64) as usize;
    let hi = (nx as i64 - x0).clamp(0, w as i64) as usize;
    out.par_chunks_mut(w).enumerate().for_each(|(row, o)| {
        let j = (row % h) as i64 + y0;
        let k = ((row / h) % d) as i64 + z0;
        let t = (row / (h * d)) as i64 + t0;
        if j < 0 || j >= ny as i64 || k < 0 || k >= nz as i64 || lo >= hi {
            return;
        }
        let start = ((t as usize * nz + k as usize) * ny + j as usize) * nx;
        let first = (x0 + lo as i64) as usize;
        o[lo..hi].copy_from_slice(&data[start + first..start + first + (hi - lo)]);
    });
    out
}

/// The sform and qform (each with its code set) translated so that the file's voxel `corner`
/// becomes voxel `(0, 0, 0)` with the same world position (§3.2), as FSL computes it (observed,
/// 2026-11-05: fitted on every row of both matrices of two neurologically stored real images,
/// ds005003's T1w and ds000256's oblique T2w, where other orders are a unit in the last place
/// away; `validation/investigations/cleanroom-bet/fit_translate.py`): the matrix of the frame
/// FSL holds the image in, `A = M·F(nx)` for an image held mirrored (`flip`: `nx` voxels along
/// x; `F(n)` takes x to `n − 1 − x`), its elements in single precision (`M₀·(nx − 1) + M₃`, the
/// product and the sum each rounded), and `A = M` otherwise; then the output's translation is
/// `A` applied in double to the held voxel of the box's first file voxel (`(nx − 1 − x₀, y₀, z₀)`
/// mirrored, `corner` otherwise), stored in the header's precision. A matrix whose code is 0 is
/// kept.
pub fn translate(h: &mut NiftiHeader, corner: [i64; 3], flip: Option<usize>) {
    let single = h.version == NiftiVersion::V1;
    let r = |x: f64| if single { f64::from(x as f32) } else { x };
    // one element of a product of a row and a column, its terms in order: each product and sum
    // rounded by `round`
    let dot = |m: [f64; 4], b: [f64; 4], round: &dyn Fn(f64) -> f64| -> f64 {
        let mut acc = round(m[0] * b[0]);
        for k in 1..4 {
            acc = round(acc + round(m[k] * b[k]));
        }
        acc
    };
    let exact = |x: f64| x;
    let [x0, y0, z0] = corner.map(|v| v as f64);
    let moved = |rows: [[f64; 4]; 3]| -> [f64; 3] {
        rows.map(|row| {
            let m = row.map(r);
            match flip {
                None => dot(m, [x0, y0, z0, 1.0], &exact),
                Some(nx) => {
                    // the held frame's matrix M·F(nx): column 0 negated, column 3 M0·(nx − 1) + M3
                    let n1 = nx as f64 - 1.0;
                    let a = [-m[0], m[1], m[2], dot(m, [n1, 0.0, 0.0, 1.0], &r)];
                    dot(a, [n1 - x0, y0, z0, 1.0], &exact)
                }
            }
        })
    };
    if h.sform_code > 0 {
        let t = moved([h.srow_x, h.srow_y, h.srow_z]);
        h.srow_x[3] = t[0];
        h.srow_y[3] = t[1];
        h.srow_z[3] = t[2];
    }
    if h.qform_code > 0
        && let Ok(q) = h.qform()
    {
        let rows = *q.rows();
        let t = moved([rows[0], rows[1], rows[2]]);
        h.qoffset_x = t[0];
        h.qoffset_y = t[1];
        h.qoffset_z = t[2];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(w: &[&str]) -> Vec<String> {
        w.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn regions() {
        let r = Region::parse(&words(&["3", "-1"])).unwrap();
        assert_eq!(r.axes[3], Range { min: 3, size: -1 });
        assert_eq!(r.axes[0], Range::WHOLE);
        assert_eq!(Region::parse(&words(&["1"])), None);
        // atoi: leading blanks and digits, words as 0
        let r = Region::parse(&words(&[" 1.5", "abc", "-2", "4", "+3", "x"])).unwrap();
        assert_eq!(r.axes[0], Range { min: 1, size: 0 });
        assert_eq!(r.axes[1], Range { min: -2, size: 4 });
        assert_eq!(r.axes[3], Range::WHOLE);
        // bounds: -1 to the end, 0 and other negatives invalid, outside allowed in space
        assert_eq!(Range { min: 2, size: -1 }.bounds(10), Some((2, 9)));
        assert_eq!(Range { min: -2, size: 4 }.bounds(10), Some((-2, 1)));
        assert_eq!(Range { min: 5, size: 0 }.bounds(10), None);
        assert_eq!(Range { min: 5, size: -3 }.bounds(10), None);
        assert_eq!(Range { min: 12, size: -1 }.bounds(10), None);
        // 32-bit wrapping of min + size - 1
        assert_eq!(
            Range {
                min: i32::MAX,
                size: 2
            }
            .bounds(10),
            None
        );
    }

    #[test]
    fn the_box_and_its_zeros() {
        // 4 x 3 x 2, 2 volumes; values 1000 t + 100 k + 10 j + i
        let dims = [4, 3, 2, 2];
        let data: Vec<i16> = (0..2)
            .flat_map(|t| {
                (0..2).flat_map(move |k| {
                    (0..3).flat_map(move |j| (0..4).map(move |i| 1000 * t + 100 * k + 10 * j + i))
                })
            })
            .collect();
        let bounds = [(-1, 1), (1, 2), (1, 1), (1, 1)];
        let shape = [3, 2, 1, 1];
        let got = cut(&data, (dims, false, bounds, shape));
        assert_eq!(got, vec![0, 1110, 1111, 0, 1120, 1121]);
        // a mirrored image: the file's x (i) is held at 3 - i
        let mirrored: Vec<i16> = data
            .chunks(4)
            .flat_map(|r| r.iter().rev().copied())
            .collect();
        let got = cut(&mirrored, (dims, true, bounds, shape));
        // the cut held mirrored: file x -1, 0, 1 held as 1, 0, -1
        assert_eq!(got, vec![1111, 1110, 0, 1121, 1120, 0]);
        // a box entirely outside: zeros
        let got = cut(
            &data,
            (dims, false, [(5, 6), (0, 0), (0, 0), (0, 0)], [2, 1, 1, 1]),
        );
        assert_eq!(got, vec![0, 0]);
    }

    #[test]
    fn the_matrices_move_with_the_corner() {
        let mut h = NiftiHeader::new(NiftiVersion::V1, &[4, 4, 4], DataType::F32).unwrap();
        h.sform_code = 1;
        h.srow_x = [-2.0, 0.0, 0.0, 95.0];
        h.srow_y = [0.0, 2.0, 0.0, -111.0];
        h.srow_z = [0.0, 0.0, 2.0, -83.0];
        h.qform_code = 0;
        h.qoffset_x = 7.0;
        translate(&mut h, [-2, 0, 3], None);
        assert_eq!(h.srow_x[3], 99.0);
        assert_eq!(h.srow_z[3], -77.0);
        assert_eq!(h.qoffset_x, 7.0, "an unset qform is kept");
        // a neurologically stored image (held mirrored): FSL's single-precision path through
        // the mirrored frame (observed: ds005003's T1w, 144 voxels along x, the box x 13 to 135)
        let mut h = NiftiHeader::new(NiftiVersion::V1, &[144, 4, 4], DataType::F32).unwrap();
        h.sform_code = 1;
        h.srow_x = [
            f64::from(1.200_004_6_f32),
            0.0,
            0.0,
            f64::from(-85.800_644_f32),
        ];
        h.srow_y = [0.0, 1.0, 0.0, 0.0];
        h.srow_z = [0.0, 0.0, 1.0, 0.0];
        translate(&mut h, [13, 30, 58], Some(144));
        assert_eq!(h.srow_x[3] as f32, -70.200_59_f32);
        // and an oblique one (ds000256's T2w, 224 voxels along x, the box from z 12): FSL's
        let mut h = NiftiHeader::new(NiftiVersion::V1, &[224, 4, 4], DataType::F32).unwrap();
        h.sform_code = 1;
        h.srow_x = [1.0, 0.0, 0.0, 0.0];
        h.srow_y = [-5.912_370_6e-2, 7.232_544e-1, 3.386_019_5e-1, -1.191_069e2]
            .map(|v: f32| f64::from(v));
        h.srow_z = [0.0, 0.0, 1.0, 0.0];
        translate(&mut h, [0, 0, 12], Some(224));
        assert_eq!(h.srow_y[3] as f32, -115.043_68_f32);
    }
}
