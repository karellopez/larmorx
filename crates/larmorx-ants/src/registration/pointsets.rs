// SPDX-License-Identifier: Apache-2.0
//! The point sets of `antsRegistration`'s point-set metrics, read as ANTs v2.6.5 reads them:
//!
//! - labelled point sets (`ICP`, `PSE`, `JHCT`): `ReadLabeledPointSet` and
//!   `LabeledPointSetFileReader` (`Utilities/ReadWriteData.h`,
//!   `Utilities/itkLabeledPointSetFileReader.hxx`): a text file (`.txt`: `x y z label` per
//!   point, or `x y trash label` in 2D), a VTK polydata file (`.vtk`, ASCII or binary, its
//!   `SCALARS` the labels), or a label image (every voxel whose label is not 0, or with
//!   `boundaryPointsOnly` the voxels on a label's contour); optionally a random fraction of the
//!   points;
//! - intensity point sets (`IGDM`): `ReadImageIntensityPointSet` and
//!   `ImageIntensityAndGradientToPointSetFilter` (`Utilities/itkImageIntensityAndGradientToPointSetFilter.hxx`):
//!   each masked voxel whose neighbourhood lies in the image, with the intensities and central
//!   difference gradients of its neighbourhood.
//!
//! Points are `Point<float, D>`; a voxel's position is its physical point in double, cast to
//! float.

use std::path::Path;

use larmorx_core::defects::Notices;
use larmorx_registration::pointset::{IntensityPointSet, LabeledPointSet};
use larmorx_registration::rng::MersenneTwister;
use larmorx_registration::{Image, Real};

use crate::cli::cstd::CppIstream;
use crate::image::{ImageStore, Pixel, ReadError, read_with};

/// Why a point set was not read.
#[derive(Clone, Debug, PartialEq)]
pub enum PointSetError {
    /// ANTs printed this message (on standard error) and goes on with a null point set, which
    /// it dereferences right after (`DisconnectPipeline`): it crashes.
    Null(String),
    /// An exception no one catches: ANTs aborts after libstdc++'s message.
    Abort(String),
    /// A case where ANTs does not finish or reads undefined memory in a way larmorx does not
    /// fill in: larmorx refuses with this explanation.
    Refused(String),
}

/// How the reader leaves its output for `PrintState` (`Requested Number Of Regions`,
/// `Requested Region`): updated through the pipeline (1, 0), or grafted or filled directly
/// (0, -1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputState {
    Updated,
    Grafted,
}

/// A labelled point set as read.
#[derive(Clone, Debug, PartialEq)]
pub struct LabeledRead<const D: usize> {
    pub set: LabeledPointSet<D>,
    pub state: OutputState,
}

/// What larmorx noticed while reading: undefined behaviour ANTs has here, and how larmorx
/// filled it in.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReadNotes {
    /// The text reader read one record past the last (a trailing newline): ANTs adds a point
    /// from uninitialised variables, which in the oracle's build are the last record's.
    pub text_extra_record: bool,
    /// The VTK reader read only the first `n` binary scalars (`numberOfComponents ×
    /// sizeof(float)` bytes) and used uninitialised memory for the rest.
    pub vtk_binary_scalars: Option<usize>,
    /// The random fraction of the points (ANTs seeds it from the clock).
    pub sampled: bool,
}

/// `ReadLabeledPointSet(target, file, boundaryPointsOnly, samplingPercentage)`.
///
/// `fix`: larmorx's corrections of the reader's undefined behaviour (an extra record from a
/// trailing newline is not a point; binary VTK labels are all read). `seed`: the generator of
/// the random fraction (ANTs: the clock).
#[allow(clippy::too_many_arguments)]
pub fn read_labeled<const D: usize>(
    store: &mut dyn ImageStore,
    file: &str,
    boundary_points_only: bool,
    sampling_percentage: f32,
    seed: u32,
    fix: bool,
    n_threads: usize,
    notices: &mut Notices,
    warnings: &mut Vec<String>,
    notes: &mut ReadNotes,
) -> Result<LabeledRead<D>, PointSetError> {
    if file.len() < 3 {
        return Err(PointSetError::Null(format!(" bad file name {file}\n")));
    }
    let on_disk = Path::new(file).exists();
    let extension = match file.rfind('.') {
        Some(p) => &file[p + 1..],
        None => file,
    };
    let reader_failure = |e: String| {
        PointSetError::Null(format!(
            "Exception caught during point set reference file reading \n{e}\n"
        ))
    };
    let mut state = OutputState::Updated;
    let mut set = match extension {
        "txt" | "vtk" => {
            if !on_disk {
                return Err(PointSetError::Null(format!(
                    " file {file} does not exist . \n"
                )));
            }
            let bytes = std::fs::read(file).map_err(|_| {
                reader_failure(itk_print(
                    READER_FILE,
                    68,
                    &format!("Unable to open file\ninputFilename= {file}"),
                ))
            })?;
            if extension == "txt" {
                read_text::<D>(&bytes, fix, notes)?
            } else {
                read_vtk::<D>(&bytes, fix, notes)?
            }
        }
        _ => {
            let image = match read_with::<u32, _>(store, file, D, n_threads, notices) {
                Ok(i) => i,
                Err(ReadError::Missing(_)) | Err(ReadError::TooShort) => {
                    return Err(PointSetError::Null(format!(
                        " file {file} does not exist . \n"
                    )));
                }
                Err(e) => return Err(reader_failure(e.to_string())),
            };
            let image = crate::cli::registration::to_image::<u32, D>(image)
                .map_err(|e| reader_failure(e.print()))?;
            read_image::<D>(&image, boundary_points_only, n_threads)
        }
    };
    if sampling_percentage < 1.0 {
        notes.sampled = true;
        let mut g = MersenneTwister::new(seed);
        let mut points = Vec::new();
        let mut labels = Vec::new();
        for (i, p) in set.points.iter().enumerate() {
            if g.closed() <= f64::from(sampling_percentage) {
                points.push(*p);
                if let Some(l) = set.labels.as_ref().and_then(|l| l.get(i)) {
                    labels.push(*l);
                }
            }
        }
        let labelled = set.labels.is_some() && !labels.is_empty();
        set = LabeledPointSet {
            points,
            labels: labelled.then_some(labels),
        };
        state = OutputState::Grafted;
    }
    // Point data missing or of another size: filled with label 0.
    let complete = set
        .labels
        .as_ref()
        .is_some_and(|l| l.len() == set.points.len());
    if !complete {
        warnings.push(format!(
            "WARNING: In {READER_FILE}, line 140\nLabeledPointSetFileReader (0x0): Number of points does not match number of labels. Filling point data with label zero.\n\n"
        ));
        if set.points.is_empty() {
            // SetPointData is never called: the point data stay a null container.
            set.labels = None;
        } else {
            set.labels = Some(vec![0; set.points.len()]);
        }
    }
    Ok(LabeledRead { set, state })
}

const READER_FILE: &str = "Utilities/itkLabeledPointSetFileReader.hxx";

/// An `itk::ExceptionObject` from ANTs' reader, as `operator<<` prints it.
fn itk_print(file: &str, line: u32, description: &str) -> String {
    format!(
        "\nitk::ExceptionObject (0x0)\nLocation: \"unknown\" \nFile: {file}\nLine: {line}\nDescription: ITK ERROR: LabeledPointSetFileReader(0x0): {description}\n"
    )
}

/// `ReadPointsFromAvantsFile`: records of `D` coordinates (and in 2D one more value) and a
/// label, read until the end of the file; a record of zeros with label 0 is skipped.
fn read_text<const D: usize>(
    bytes: &[u8],
    fix: bool,
    notes: &mut ReadNotes,
) -> Result<LabeledPointSet<D>, PointSetError> {
    let mut s = CppIstream::new(bytes);
    let mut points = Vec::new();
    let mut labels = Vec::new();
    // The record's variables are uninitialised at each iteration; in the oracle's build they
    // keep the previous record's values.
    let mut point = [0f32; D];
    let mut label = 0u32;
    while !s.eof {
        if s.fail {
            return Err(PointSetError::Refused(
                "the point-set text file holds something that is not a number: ANTs' reader stops reading but loops forever".into(),
            ));
        }
        let mut assigned = true;
        for c in point.iter_mut() {
            assigned &= s.read_f32(c);
        }
        if D == 2 {
            let mut trash = 0f32;
            s.read_f32(&mut trash);
        }
        assigned &= s.read_u32(&mut label);
        if !assigned {
            notes.text_extra_record = true;
            if fix {
                break;
            }
        }
        let norm: f64 = point.iter().map(|&v| f64::from(v) * f64::from(v)).sum();
        if norm > 0.0 || label != 0 {
            points.push(point);
            labels.push(label);
        }
    }
    let labelled = !points.is_empty();
    Ok(LabeledPointSet {
        points,
        labels: labelled.then_some(labels),
    })
}

/// The libstdc++ message of `std::string(line, pos, n)` with `pos` beyond the line.
fn substring_abort(pos: usize, size: usize) -> PointSetError {
    PointSetError::Abort(format!(
        "terminate called after throwing an instance of 'std::out_of_range'\n  what():  basic_string::basic_string: __pos (which is {pos}) > this->size() (which is {size})\n"
    ))
}

/// The first line containing `key` (`while (!eof) { getline; if BINARY …; if key break; }`),
/// whether a line before it (or it) contained `BINARY`, and the stream after it; the last line
/// read when none does.
fn find_line<'a>(bytes: &'a [u8], key: &str) -> (CppIstream<'a>, String, bool, bool) {
    let mut s = CppIstream::new(bytes);
    let mut line = String::new();
    let mut binary = false;
    let mut found = false;
    while !s.eof {
        line = s.getline();
        if line.contains("BINARY") {
            binary = true;
        }
        if line.contains(key) {
            found = true;
            break;
        }
    }
    (s, line, binary, found)
}

/// `ReadVTKFile`: the points, then the scalars (the labels), then the lines (read and
/// unused).
fn read_vtk<const D: usize>(
    bytes: &[u8],
    fix: bool,
    notes: &mut ReadNotes,
) -> Result<LabeledPointSet<D>, PointSetError> {
    // ReadPointsFromVTKFile.
    let (mut s, line, binary, _) = find_line(bytes, "POINTS");
    if line.len() < 7 {
        return Err(substring_abort(7, line.len()));
    }
    let point_line = &line[7..];
    let n: i32 = match sscanf_int(point_line) {
        Some(n) => n,
        None => {
            return Err(PointSetError::Null(format!(
                "Exception caught during point set reference file reading \n{}\n",
                itk_print(
                    READER_FILE,
                    244,
                    &format!("ERROR: Failed to read numberOfPoints\n       pointLine = {point_line}")
                )
            )));
        }
    };
    if n < 1 {
        return Err(PointSetError::Null(format!(
            "Exception caught during point set reference file reading \n{}\n",
            itk_print(
                READER_FILE,
                254,
                &format!("numberOfPoints < 1       numberOfPoints = {n}")
            )
        )));
    }
    let n = n as usize;
    let mut points = vec![[0f32; D]; n];
    if binary {
        let raw = s.read(3 * n * 4);
        let mut data = vec![0f32; 3 * n];
        for (k, chunk) in raw.chunks_exact(4).enumerate() {
            data[k] = f32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        }
        for (i, p) in points.iter_mut().enumerate() {
            for (j, c) in p.iter_mut().enumerate() {
                let k = i * 3 + j;
                if k >= data.len() {
                    return Err(PointSetError::Refused(
                        "a binary VTK point set in 4D: ANTs reads each point's fourth coordinate past the three it stores, the last one past the end of its buffer".into(),
                    ));
                }
                *c = data[k];
            }
        }
    } else {
        for p in points.iter_mut() {
            for c in p.iter_mut() {
                s.read_f32(c);
            }
            if D == 2 {
                let mut trash = 0f32;
                s.read_f32(&mut trash);
            }
        }
    }
    // ReadScalarsFromVTKFile.
    let (mut s, line, binary, found) = find_line(bytes, "SCALARS");
    let mut labels: Option<Vec<u32>> = None;
    if found && !s.eof {
        let pos = line.rfind(' ').map_or(0, |p| p + 1);
        let components = atoi(&line[pos..]).max(0) as usize;
        let _ = s.getline();
        if binary {
            let values = n * components;
            // read(numberOfComponents * sizeof(float)) bytes, not the whole array.
            let wanted = if fix { values * 4 } else { components * 4 };
            let raw = s.read(wanted);
            let mut data = vec![0i32; values];
            for (k, chunk) in raw.chunks_exact(4).enumerate() {
                if k < values {
                    data[k] = i32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                }
            }
            if values > components {
                notes.vtk_binary_scalars = Some(components);
            }
            if components == 1 {
                labels = Some(data.iter().map(|&v| v as u32).collect());
            }
        } else if components == 1 {
            let mut out = Vec::with_capacity(n);
            let mut label = 0u32;
            for _ in 0..n {
                s.read_u32(&mut label);
                out.push(label);
            }
            labels = Some(out);
        }
    }
    // ReadLinesFromVTKFile: the lines are not used, but a line too short to parse aborts.
    let (_, line, _, found) = find_line(bytes, "LINES");
    if found && line.len() < 6 {
        return Err(substring_abort(6, line.len()));
    }
    Ok(LabeledPointSet { points, labels })
}

/// `sscanf(s, "%d", &n)`: leading whitespace, an optional sign and digits.
fn sscanf_int(s: &str) -> Option<i32> {
    let t = s.trim_start_matches([' ', '\t', '\n', '\x0b', '\x0c', '\r']);
    let b = t.as_bytes();
    let mut i = usize::from(matches!(b.first(), Some(b'+' | b'-')));
    let from = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    if i == from {
        return None;
    }
    // Out of range: glibc's scanf stores the value modulo 2³² (undefined in C); it is not
    // a point count either way.
    Some(t[..i].parse::<i64>().map_or(i32::MAX, |v| v as i32))
}

/// C's `atoi`.
fn atoi(s: &str) -> i64 {
    let t = s.trim_start_matches([' ', '\t', '\n', '\x0b', '\x0c', '\r']);
    let b = t.as_bytes();
    let mut i = usize::from(matches!(b.first(), Some(b'+' | b'-')));
    let negative = b.first() == Some(&b'-');
    let mut v: i64 = 0;
    while i < b.len() && b[i].is_ascii_digit() {
        v = v.saturating_mul(10).saturating_add(i64::from(b[i] - b'0'));
        i += 1;
    }
    if negative { -v } else { v }
}

/// `ReadPointsFromImageFile`: every voxel whose label is not 0 (or, with
/// `boundaryPointsOnly`, each voxel `LabelContourImageFilter` puts on a label's contour, fully
/// connected, background 0), in the image's order.
fn read_image<const D: usize>(
    image: &Image<u32, D>,
    boundary: bool,
    n_threads: usize,
) -> LabeledPointSet<D> {
    let space = &image.space;
    let keep: Vec<bool> = if boundary {
        let labels: Vec<u64> = image.data.iter().map(|&v| u64::from(v)).collect();
        larmorx_image::components::label_contour(&labels, &space.size, true, 0, n_threads)
            .unwrap_or_else(|_| vec![false; labels.len()])
    } else {
        image.data.iter().map(|&v| v != 0).collect()
    };
    let mut points = Vec::new();
    let mut labels = Vec::new();
    for o in 0..space.len() {
        let label = image.data[o];
        if keep[o] && label > 0 {
            let p = space.index_to_point(&space.index_of(o));
            points.push(std::array::from_fn(|k| p[k] as f32));
            labels.push(label);
        }
    }
    let labelled = !points.is_empty();
    LabeledPointSet {
        points,
        labels: labelled.then_some(labels),
    }
}

/// `ReadImageIntensityPointSet(target, imageFile, maskFile, neighborhoodRadius, sigma)`: the
/// image (`itk::Image<R, D>`) and the mask (`unsigned char`), then
/// `ImageIntensityAndGradientToPointSetFilter` with central differences (the filter's
/// default: the gradient sigma is not used).
#[allow(clippy::too_many_arguments)]
pub fn read_intensity<R: Real + Pixel, const D: usize>(
    store: &mut dyn ImageStore,
    image_file: &str,
    mask_file: &str,
    radius: &[u32],
    n_threads: usize,
    notices: &mut Notices,
) -> Result<IntensityPointSet<R, D>, PointSetError> {
    let null = |m: String| Err(PointSetError::Null(m));
    if image_file.len() < 3 {
        return null(format!(" bad image file name {image_file}\n"));
    }
    let image = match read_with::<R, _>(store, image_file, D, n_threads, notices) {
        Ok(i) => i,
        Err(ReadError::Missing(_)) | Err(ReadError::TooShort) => {
            return null(format!(" image file {image_file} does not exist . \n"));
        }
        Err(ReadError::NotImage(_)) => {
            return null(format!(
                " file {image_file} is not recognized as a supported image format . \n"
            ));
        }
        Err(e) => return null(format!("{e}\n")),
    };
    if mask_file.len() < 3 {
        return null(format!(" bad mask file name {mask_file}\n"));
    }
    let mask = match read_with::<u8, _>(store, mask_file, D, n_threads, notices) {
        Ok(i) => i,
        Err(ReadError::Missing(_)) | Err(ReadError::TooShort) => {
            return null(format!(" mask file {mask_file} does not exist . \n"));
        }
        Err(ReadError::NotImage(_)) => {
            return null(format!(
                " file {mask_file} is not recognized as a supported image format . \n"
            ));
        }
        Err(e) => return null(format!("{e}\n")),
    };
    if radius.len() != D {
        return null(
            " size of the neighborhood radius is not equal to the image dimension.\n".into(),
        );
    }
    let to = |e: larmorx_registration::ItkException| PointSetError::Refused(e.description);
    let image = crate::cli::registration::to_image::<R, D>(image).map_err(to)?;
    let mask = crate::cli::registration::to_image::<u8, D>(mask).map_err(to)?;
    intensity_points(&image, &mask, radius)
}

/// `ImageIntensityAndGradientToPointSetFilter::GenerateData` with central differences.
fn intensity_points<R: Real, const D: usize>(
    image: &Image<R, D>,
    mask: &Image<u8, D>,
    radius: &[u32],
) -> Result<IntensityPointSet<R, D>, PointSetError> {
    let space = &image.space;
    // The gradient image: CentralDifferenceImageFunction::EvaluateAtIndex with the image's
    // direction (0 along an axis at the image's border).
    let n = space.len();
    let mut gradient: Vec<[R; D]> = Vec::with_capacity(n);
    let strides = space.strides();
    for o in 0..n {
        let idx = space.index_of(o);
        let mut local = [0.0f64; D];
        let mut local_r = [R::ZERO; D];
        for d in 0..D {
            if idx[d] < space.start[d] + 1
                || idx[d] > space.start[d] + space.size[d] as i64 - 2
            {
                continue;
            }
            let plus = image.data[o + strides[d]];
            let minus = image.data[o - strides[d]];
            let diff = plus - minus;
            let factor = R::from_f64(0.5).to_f64() / space.spacing[d];
            local_r[d] = R::from_f64(diff.to_f64() * factor);
        }
        for d in 0..D {
            local[d] = local_r[d].to_f64();
        }
        let physical = space.local_to_physical_vector(&local);
        gradient.push(std::array::from_fn(|d| R::from_f64(physical[d])));
    }
    let r: Vec<i64> = radius.iter().map(|&v| i64::from(v)).collect();
    let mut voxels = 1usize;
    for &v in &r {
        voxels *= (2 * v + 1) as usize;
    }
    // The neighbourhood's offsets, x fastest.
    let mut offsets: Vec<[i64; D]> = Vec::with_capacity(voxels);
    for mut k in 0..voxels {
        let mut off = [0i64; D];
        for d in 0..D {
            let w = (2 * r[d] + 1) as usize;
            off[d] = (k % w) as i64 - r[d];
            k /= w;
        }
        offsets.push(off);
    }
    let mut points = Vec::new();
    let mut data = Vec::new();
    for o in 0..n {
        let idx = space.index_of(o);
        if !mask.space.contains(&idx) {
            return Err(PointSetError::Refused(
                "the IGDM mask is smaller than its image: ANTs reads the mask past its end".into(),
            ));
        }
        let masked = mask.data[mask.space.offset(&idx)] != 0;
        let in_bounds = (0..D).all(|d| {
            idx[d] - r[d] >= space.start[d]
                && idx[d] + r[d] <= space.start[d] + space.size[d] as i64 - 1
        });
        if !(masked && in_bounds) {
            continue;
        }
        let p = space.index_to_point(&idx);
        points.push(std::array::from_fn(|k| p[k] as f32));
        let mut array = Vec::with_capacity(voxels * (1 + D));
        for off in &offsets {
            let q: [i64; D] = std::array::from_fn(|d| idx[d] + off[d]);
            let oo = space.offset(&q);
            array.push(image.data[oo]);
            for d in 0..D {
                array.push(gradient[oo][d]);
            }
        }
        data.push(array);
    }
    Ok(IntensityPointSet { points, data })
}
