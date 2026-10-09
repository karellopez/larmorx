// SPDX-License-Identifier: Apache-2.0
//! `antsApplyTransforms` with its original arguments (ANTs v2.6.5).
//!
//! Supported: 3D scalar images (`-e 0`) and time series (`-e 3`, also with `--time-index`),
//! every interpolator, `-u`, `-f`, `--float` and `-v`, with NIfTI input and output. Not yet
//! supported (an error says so): vector, tensor, multichannel and five-dimensional images, 2D
//! and 4D transforms (`-d 2`, `-d 4`), transform outputs (`-o Linear[...]`,
//! `-o CompositeTransform[...]`, `-o [field,1]`) and transform initializers
//! (`-t [fixed,moving,feature]`).
//!
//! `--float` is accepted but computed in double precision, then rounded to float32, so
//! results differ from ANTs' float pipeline in the last float bits.

use std::io::Write;
use std::path::Path;

use larmorx_core::Grid3;
use larmorx_core::array::DynArray;
use larmorx_core::ndarray::{ArrayD, IxDyn, ShapeBuilder};
use larmorx_interp::{Interpolation, Window};
use larmorx_io::nifti::itk::{ItkGeometry, ItkVoxels};
use larmorx_io::nifti::{self, NiftiVersion};
use larmorx_transform::{LinearTransform, Transform, TransformChain};

use super::parser::{self, OptionSpec, OptionValue, Parsed, atof};
use super::{TransformLoader, itk_threads};
use crate::{ApplyTransformsOptions, OutputType, apply_transforms};

const fn opt(long: &'static str, short: Option<char>) -> OptionSpec {
    OptionSpec { long, short }
}

const SPECS: &[OptionSpec] = &[
    opt("dimensionality", Some('d')),
    opt("input-image-type", Some('e')),
    opt("time-index", None),
    opt("input", Some('i')),
    opt("reference-image", Some('r')),
    opt("output", Some('o')),
    opt("interpolation", Some('n')),
    opt("output-data-type", Some('u')),
    opt("transform", Some('t')),
    opt("default-value", Some('f')),
    opt("static-cast-for-R", Some('z')),
    opt("float", None),
    opt("verbose", Some('v')),
    opt("help", Some('h')),
];

const USAGE: &str = "\
COMMAND:
     antsApplyTransforms (larmorx port of ANTs v2.6.5)
          Resamples an image through a chain of transforms onto a reference grid.

OPTIONS:
     -d, --dimensionality 3
     -e, --input-image-type 0/1/2/3/4/5/6
                            scalar/vector/tensor/time-series/multichannel/five-dimensional/physical-vector
                            (larmorx supports scalar and time-series)
     --time-index <index>   extract one volume of a time series
     -i, --input inputFileName
     -r, --reference-image imageFileName
     -o, --output warpedOutputFileName
                  [warpedOutputFileName,0]
     -n, --interpolation Linear
                         NearestNeighbor
                         MultiLabel[<sigma=imageSpacing>,<alpha=4.0>]
                         Gaussian[<sigma=imageSpacing>,<alpha=1.0>]
                         BSpline[<order=3>]
                         CosineWindowedSinc
                         WelchWindowedSinc
                         HammingWindowedSinc
                         LanczosWindowedSinc
                         GenericLabel[<interpolator=Linear>]
     -u, --output-data-type char/uchar/short/int/float/double/default
     -t, --transform transformFileName
                     [transformFileName,useInverse]
     -f, --default-value value
     --float 0/1
     -v, --verbose (0)/1
     -h
     --help
";

/// Runs `antsApplyTransforms` with `args`; returns the exit code.
pub fn main(
    args: &[String],
    loader: &dyn TransformLoader,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    if args.is_empty() {
        let _ = err.write_all(USAGE.as_bytes());
        return 1;
    }
    let parsed = match parser::parse(args, SPECS) {
        Ok(p) => p,
        Err(message) => {
            let _ = writeln!(err, "ERROR: {message}");
            return 1;
        }
    };
    if parsed.has("help") {
        let _ = out.write_all(USAGE.as_bytes());
        return 0;
    }
    match run(&parsed, loader, out) {
        Ok(()) => 0,
        Err(message) => {
            let _ = writeln!(err, "{message}");
            1
        }
    }
}

/// ANTs' `Convert<bool>` (an integer read by `istringstream`).
fn flag(v: Option<&OptionValue>) -> bool {
    v.is_some_and(|v| atof(&v.name) != 0.0)
}

/// What `-e` names.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ImageType {
    Scalar,
    TimeSeries,
}

fn image_type(p: &Parsed) -> Result<ImageType, String> {
    let Some(v) = p.last("input-image-type") else {
        return Ok(ImageType::Scalar);
    };
    match v.name.as_str() {
        "scalar" | "0" => Ok(ImageType::Scalar),
        "time-series" | "3" => Ok(ImageType::TimeSeries),
        "vector" | "1" | "tensor" | "2" | "multichannel" | "4" | "five-dimensional" | "5"
        | "physical-vector" | "6" => Err(format!(
            "larmorx: input image type '{}' is not supported yet (scalar and time-series are)",
            v.name
        )),
        _ => Err("Unrecognized input image type (cf --input-image-type option).".into()),
    }
}

/// The interpolator `-n` names (`make_interpolator_snip.tmpl`); Gaussian sigmas default to
/// the input spacing.
fn interpolation(p: &Parsed, input_spacing: [f64; 3]) -> Result<Interpolation, String> {
    let Some(v) = p.last("interpolation") else {
        return Ok(Interpolation::Linear);
    };
    let name = v.name.to_lowercase();
    let sigma = || -> [f64; 3] {
        match v.parameters.first() {
            None => input_spacing,
            Some(s) => {
                let values: Vec<f64> = s.trim_end().split('x').map(atof).collect();
                if values.len() == 3 {
                    [values[0], values[1], values[2]]
                } else {
                    [values[0]; 3]
                }
            }
        }
    };
    Ok(match name.as_str() {
        "linear" => Interpolation::Linear,
        "nearestneighbor" => Interpolation::NearestNeighbor,
        "bspline" => Interpolation::BSpline {
            order: v.parameters.first().map_or(3, |o| atof(o) as u32),
        },
        "gaussian" => Interpolation::Gaussian {
            sigma: sigma(),
            alpha: v.parameters.get(1).map_or(1.0, |a| atof(a)),
        },
        // ANTs reads no alpha for MultiLabel: it is always 4.
        "multilabel" => Interpolation::MultiLabel {
            sigma: sigma(),
            alpha: 4.0,
        },
        "cosinewindowedsinc" => Interpolation::WindowedSinc(Window::Cosine),
        "hammingwindowedsinc" => Interpolation::WindowedSinc(Window::Hamming),
        "lanczoswindowedsinc" => Interpolation::WindowedSinc(Window::Lanczos),
        "blackmanwindowedsinc" => Interpolation::WindowedSinc(Window::Blackman),
        "welchwindowedsinc" => Interpolation::WindowedSinc(Window::Welch),
        "genericlabel" => Interpolation::GenericLabel,
        _ => return Err(format!("Error:  Unrecognized interpolation option. {name}")),
    })
}

/// The `-t` options as a chain (`GetCompositeTransformFromParserOption`).
fn transform_chain(p: &Parsed, loader: &dyn TransformLoader) -> Result<TransformChain, String> {
    let mut items = Vec::new();
    for v in p.all("transform") {
        let (name, invert) = match v.parameters.len() {
            0 => (v.name.as_str(), false),
            1 | 2 => (
                v.parameters[0].as_str(),
                v.parameters.get(1).is_some_and(|i| atof(i) != 0.0),
            ),
            _ => {
                return Err(
                    "larmorx: transform initializers (-t [fixed,moving,feature]) are not supported yet"
                        .into(),
                );
            }
        };
        let transform = if name == "identity" || name == "Identity" {
            Transform::Linear(LinearTransform::from_matrix(
                larmorx_core::linalg::IDENTITY3,
                [0.0; 3],
            ))
        } else {
            loader
                .load(Path::new(name))
                .map_err(|e| format!("Can't read initial transform {name}: {e}"))?
        };
        items.push((transform, invert));
    }
    TransformChain::new(items).map_err(|e| format!("Inverse does not exist: {e}"))
}

fn grid3(g: &ItkGeometry) -> Result<Grid3, String> {
    g.grid3().ok_or_else(|| "singular image grid".to_owned())
}

/// The output file named by `-o`.
fn output_path(p: &Parsed) -> Result<String, String> {
    let v = p
        .last("output")
        .ok_or("larmorx: an output (-o) is required")?;
    let unsupported = |what: &str| format!("larmorx: {what} output (-o) is not supported yet");
    match v.name.to_lowercase().as_str() {
        "linear" => return Err(unsupported("linear transform")),
        "compositetransform" => return Err(unsupported("composite transform")),
        "displacementfield" => return Err(unsupported("displacement field")),
        _ => {}
    }
    if v.parameters.len() > 1 {
        if atof(&v.parameters[1]) != 0.0 {
            return Err(unsupported("displacement field"));
        }
        return Ok(v.parameters[0].clone());
    }
    if v.name.is_empty() {
        return Err("larmorx: empty output file name".into());
    }
    Ok(v.name.clone())
}

fn run(p: &Parsed, loader: &dyn TransformLoader, out: &mut dyn Write) -> Result<(), String> {
    let verbose = flag(p.last("verbose"));
    let mut log = |line: String| {
        if verbose {
            let _ = writeln!(out, "{line}");
        }
    };
    let dimension = p.last("dimensionality").map_or(3.0, |v| atof(&v.name));
    if dimension != 3.0 {
        return Err(format!(
            "larmorx: only 3D images are supported yet (-d 3), not -d {dimension}"
        ));
    }
    let single = flag(p.last("float"));
    let mut image_type = image_type(p)?;
    let time_index = p.last("time-index").map(|v| atof(&v.name) as usize);
    if time_index.is_some() && image_type == ImageType::TimeSeries {
        image_type = ImageType::Scalar;
    }
    let output_type = match p.last("output-data-type").map(|v| v.name.to_lowercase()) {
        Some(name) => OutputType::from_name(&name),
        None => None,
    }
    .unwrap_or(if single {
        OutputType::Float
    } else {
        OutputType::Double
    });
    let default_value = p.last("default-value").map_or(0.0, |v| atof(&v.name));
    let default_value = if single {
        f64::from(default_value as f32)
    } else {
        default_value
    };
    let n_threads = itk_threads();

    let input_path = &p.last("input").ok_or("An input image is required.")?.name;
    let reference_path = &p
        .last("reference-image")
        .ok_or("A reference image is required.")?
        .name;
    let output = output_path(p)?;
    fn read_err(path: &str) -> impl FnOnce(nifti::Error) -> String + '_ {
        move |e| format!("Unable to read image {path}: {e}")
    }

    let image = nifti::itk::read_itk_image(input_path, n_threads).map_err(read_err(input_path))?;
    let g = &image.geometry;
    let voxels_3d: usize = g.size.iter().take(3).product();
    let mut volumes = match (g.ndim, image_type, time_index) {
        (3, ImageType::Scalar, None) => 1,
        (_, ImageType::Scalar, None) => {
            return Err(format!(
                "Input image dimension does not match. Expected: 3, but got: {}\nSee -e option for available input types.",
                g.ndim
            ));
        }
        (3, _, _) => 1,
        (4, _, _) => g.size[3],
        _ => {
            return Err(format!(
                "larmorx: {}D time series are not supported",
                g.ndim
            ));
        }
    };
    if image.voxels.len() != voxels_3d * volumes {
        return Err(
            "Image pixel type is not scalar.\nSee -e option for available input types.".into(),
        );
    }
    log(format!("Input image: {input_path}"));
    let input_grid = grid3(g)?;
    let reference = nifti::read_header(reference_path)
        .map_err(read_err(reference_path))
        .and_then(|h| nifti::itk::itk_geometry(&h).map_err(|e| format!("{reference_path}: {e}")))?;
    let reference_grid = grid3(&reference)?;
    log(format!("Reference image: {reference_path}"));

    let chain = transform_chain(p, loader)?;
    let interpolation = interpolation(p, input_grid.spacing)?;
    log(format!("Interpolation: {interpolation:?}"));
    log(format!("Default pixel value: {default_value}"));

    // The voxels to resample: one volume, or all of a time series.
    let mut voxels = image.voxels;
    if let Some(t) = time_index {
        let n_t = if g.ndim >= 4 { g.size[3] } else { 1 };
        if t >= n_t {
            return Err(format!(
                "Error: time index to extract is out of range [0, {}]",
                n_t - 1
            ));
        }
        let range = t * voxels_3d..(t + 1) * voxels_3d;
        voxels = match voxels {
            ItkVoxels::F32(v) => ItkVoxels::F32(v[range].to_vec()),
            ItkVoxels::F64(v) => ItkVoxels::F64(v[range].to_vec()),
        };
        volumes = 1;
    }
    if single && let ItkVoxels::F64(v) = &voxels {
        voxels = ItkVoxels::F32(v.iter().map(|&x| x as f32).collect());
    }
    let options = ApplyTransformsOptions {
        interpolation,
        default_value,
        n_threads,
    };
    let result = match &voxels {
        ItkVoxels::F32(v) => apply_transforms(v, &input_grid, &reference_grid, &chain, &options),
        ItkVoxels::F64(v) => apply_transforms(v, &input_grid, &reference_grid, &chain, &options),
    }
    .map_err(|e| e.to_string())?;

    // Write it as ITK would: both qform and sform (scanner), mm and seconds.
    let [nx, ny, nz] = reference_grid.size;
    let shape: Vec<usize> = if image_type == ImageType::TimeSeries {
        vec![nx, ny, nz, volumes]
    } else {
        vec![nx, ny, nz]
    };
    let data = cast_output(result, &shape, output_type);
    let affine = reference_grid.ras_affine();
    let mut header = nifti::header_for_image(
        None,
        Some(NiftiVersion::V1),
        &shape,
        data.data_type(),
        &affine,
    )
    .map_err(|e| e.to_string())?;
    header.set_qform(&affine, 1).map_err(|e| e.to_string())?;
    header.set_sform(&affine, 1);
    header.xyzt_units = 2 | 8;
    if shape.len() == 4 && g.ndim >= 4 {
        header.pixdim[4] = g.spacing[3];
        header.toffset = g.origin[3];
    }
    nifti::write_dyn(
        &output,
        &header,
        &data,
        &nifti::WriteOptions {
            n_threads,
            ..Default::default()
        },
    )
    .map_err(|e| e.to_string())?;
    log(format!("Output warped image: {output}"));
    Ok(())
}

/// ANTs' output cast (`CastImageFilter`, a `static_cast`): floats round to nearest, integers
/// truncate toward zero. Out of range, the C++ cast is undefined; x86-64 builds (where ANTs
/// and fMRIPrep usually run) convert through a 32-bit integer (`cvttsd2si`, which gives
/// `i32::MIN` for NaN and values beyond its range) and keep the low bits, so `-u uchar` wraps
/// 400.7 to 144. This reproduces that, on every platform.
pub fn cast_output(values: Vec<f64>, shape: &[usize], ty: OutputType) -> DynArray {
    fn array<T>(v: Vec<T>, shape: &[usize]) -> ArrayD<T> {
        ArrayD::from_shape_vec(IxDyn(shape).f(), v).expect("shape matches the data")
    }
    /// x86-64's `cvttsd2si` to a 32-bit integer.
    fn to_i32(v: f64) -> i32 {
        if v.is_nan() || v >= 2_147_483_648.0 || v <= -2_147_483_649.0 {
            i32::MIN
        } else {
            v as i32
        }
    }
    match ty {
        OutputType::Double => DynArray::F64(array(values, shape)),
        OutputType::Float => {
            DynArray::F32(array(values.iter().map(|&v| v as f32).collect(), shape))
        }
        OutputType::Int => DynArray::I32(array(values.iter().map(|&v| to_i32(v)).collect(), shape)),
        OutputType::Short => DynArray::I16(array(
            values.iter().map(|&v| to_i32(v) as i16).collect(),
            shape,
        )),
        OutputType::UChar => DynArray::U8(array(
            values.iter().map(|&v| to_i32(v) as u8).collect(),
            shape,
        )),
        OutputType::Char => DynArray::I8(array(
            values.iter().map(|&v| to_i32(v) as i8).collect(),
            shape,
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integer_casts_follow_x86_64() {
        let values = vec![400.7, -3.7, 255.9, 1e12, f64::NAN, -128.5];
        let DynArray::U8(u) = cast_output(values.clone(), &[6], OutputType::UChar) else {
            panic!()
        };
        assert_eq!(
            u.as_slice_memory_order().unwrap(),
            [144, 253, 255, 0, 0, 128]
        );
        let DynArray::I32(i) = cast_output(values, &[6], OutputType::Int) else {
            panic!()
        };
        assert_eq!(
            i.as_slice_memory_order().unwrap(),
            [400, -3, 255, i32::MIN, i32::MIN, -128]
        );
    }
}
