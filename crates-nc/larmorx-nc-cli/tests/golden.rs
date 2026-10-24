// SPDX-License-Identifier: LicenseRef-FSL
// Part of larmorx-nc (FSL Licence terms; see LICENSE). Copyright 2026 Karel Lopez Vilaret.
//! Golden tests of the FSL replicas: every platform reproduces the output `larmorx-nc` gives on
//! Linux x86_64, bit for bit (CLAUDE.md rule 10, `docs/validation/golden.md`).
//!
//! The inputs are generated here from seeded integers (SplitMix64, the same generator as the
//! Python golden tests) and plain integer arithmetic, and written as NIfTI-1 files by the test
//! itself. Each case runs the `larmorx-nc` binary on them and hashes (SHA-256) every file it
//! writes: the images' header and data, the `.mat` matrices, the `.par` and `.rms` texts; the
//! hashes are compared with `tests/golden.tsv`. One case per distinct code path: each cost
//! function, final interpolation, stage count, reference choice, input type and output type,
//! the in-plane mode, `-init`, `-stats`, and the paths of mcflirt's defects (D13): each
//! corrected (the default, `LARMORX_UPSTREAM_DEFECTS=fix`) and, in the `*.reproduce` cases,
//! reproduced.
//!
//! Recording, on Linux x86_64 only, from a committed tree whose replica parity passes:
//! `LARMORX_GOLDEN_RECORD=1 cargo test --locked -p larmorx-nc-cli --test golden`
//! (`LARMORX_GOLDEN_ALLOW_DIRTY=1` to record from uncommitted changes). On a mismatch the
//! outputs are saved in `$LARMORX_GOLDEN_MISMATCHES` (CI uploads it), else in
//! `target/tmp/golden-mismatches`.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use sha2::{Digest, Sha256};

const EXE: &str = env!("CARGO_BIN_EXE_larmorx-nc");
const GOLDEN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden.tsv");
/// Values kept per image, at fixed positions, to tell how large a mismatch is.
const N_SAMPLES: usize = 8;

/// The series: 16 × 14 × 8 voxels of 3 × 3 × 3.5 mm, 4 volumes.
const SHAPE: [usize; 4] = [16, 14, 8, 4];
const PIXDIM: [f32; 3] = [3.0, 3.0, 3.5];

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Input {
    /// The moving head as int16 (the series' default path: the output is int16).
    Int16,
    /// The same as float32.
    Float32,
    /// As uint8 (scaled to 0..250).
    Uint8,
    /// As int8 (centred: negative values; FSL-09).
    Int8,
    /// float32 with one NaN voxel in volume 2 and an infinite one in volume 3 (FSL-14).
    Float32NonFinite,
    /// float32, big-endian, with a positive-determinant sform (the neurological order).
    Float32BigEndianNeurological,
    /// int16 with a TR of 2000 in milliseconds (FSL-10).
    Int16Milliseconds,
    /// A 3-slice slab (the in-plane mode by thickness), float32.
    Slab,
}

struct Case {
    id: &'static str,
    /// `<family> <tool>`, as `larmorx-nc --help` lists it.
    tool: &'static str,
    input: Input,
    /// mcflirt's arguments after `-in <input> -out <id>` (`@ref` is the reference volume
    /// file, `@init` an initial matrix file).
    args: &'static [&'static str],
    /// `LARMORX_UPSTREAM_DEFECTS`: `fix` (the default) or `reproduce`.
    mode: &'static str,
    /// The upstream defects the run must name on standard error, and no other.
    named: &'static [&'static str],
    /// `FSLOUTPUTTYPE`.
    output_type: &'static str,
}

macro_rules! args {
    ($($a:expr),* $(,)?) => { &[$($a),*] };
}

const CASES: &[Case] = &[
    Case {
        id: "mcflirt.default.int16",
        tool: "fsl mcflirt",
        input: Input::Int16,
        args: args!["-mats", "-plots", "-rmsrel", "-rmsabs"],
        mode: "fix",
        named: &["FSL-01", "FSL-09"],
        output_type: "NIFTI",
    },
    Case {
        id: "mcflirt.default.int16.reproduce",
        tool: "fsl mcflirt",
        input: Input::Int16,
        args: args!["-mats", "-plots", "-rmsrel", "-rmsabs"],
        mode: "reproduce",
        named: &["FSL-01", "FSL-09"],
        output_type: "NIFTI",
    },
    Case {
        id: "mcflirt.reffile.sinc.stages4.float32",
        tool: "fsl mcflirt",
        input: Input::Float32,
        args: args![
            "-reffile",
            "@ref",
            "-stages",
            "4",
            "-sinc_final",
            "-mats",
            "-plots",
            "-rmsabs"
        ],
        mode: "fix",
        named: &[],
        output_type: "NIFTI",
    },
    Case {
        id: "mcflirt.meanvol.spline.dof12",
        tool: "fsl mcflirt",
        input: Input::Float32,
        args: args!["-meanvol", "-spline_final", "-dof", "12", "-mats", "-plots"],
        mode: "fix",
        named: &["FSL-08"],
        output_type: "NIFTI",
    },
    Case {
        id: "mcflirt.mutualinfo.nn.bins64",
        tool: "fsl mcflirt",
        input: Input::Float32,
        args: args![
            "-cost",
            "mutualinfo",
            "-bins",
            "64",
            "-nn_final",
            "-refvol",
            "0",
            "-mats",
            "-plots"
        ],
        mode: "fix",
        named: &[],
        output_type: "NIFTI",
    },
    Case {
        id: "mcflirt.normmi.stages1",
        tool: "fsl mcflirt",
        input: Input::Float32,
        args: args!["-cost", "normmi", "-stages", "1", "-mats"],
        mode: "fix",
        named: &[],
        output_type: "NIFTI",
    },
    Case {
        id: "mcflirt.corratio.stages2",
        tool: "fsl mcflirt",
        input: Input::Float32,
        args: args!["-cost", "corratio", "-stages", "2", "-mats"],
        mode: "fix",
        named: &[],
        output_type: "NIFTI",
    },
    Case {
        id: "mcflirt.woods.stages1",
        tool: "fsl mcflirt",
        input: Input::Float32,
        args: args!["-cost", "woods", "-stages", "1", "-mats"],
        mode: "fix",
        named: &[],
        output_type: "NIFTI",
    },
    Case {
        id: "mcflirt.leastsquares.smooth0",
        tool: "fsl mcflirt",
        input: Input::Float32,
        args: args!["-cost", "leastsquares", "-smooth", "0", "-mats"],
        mode: "fix",
        named: &[],
        output_type: "NIFTI",
    },
    Case {
        id: "mcflirt.init.fudge.rotation2",
        tool: "fsl mcflirt",
        input: Input::Float32,
        args: args![
            "-init",
            "@init",
            "-fudge",
            "-rotation",
            "2",
            "-mats",
            "-plots"
        ],
        mode: "fix",
        named: &[],
        output_type: "NIFTI",
    },
    Case {
        id: "mcflirt.2d.slab",
        tool: "fsl mcflirt",
        input: Input::Slab,
        args: args!["-mats", "-plots"],
        mode: "fix",
        named: &["FSL-01", "FSL-03"],
        output_type: "NIFTI",
    },
    Case {
        id: "mcflirt.2d.slab.reproduce",
        tool: "fsl mcflirt",
        input: Input::Slab,
        args: args!["-2d", "-mats", "-plots"],
        mode: "reproduce",
        named: &["FSL-01", "FSL-03"],
        output_type: "NIFTI",
    },
    Case {
        id: "mcflirt.stats",
        tool: "fsl mcflirt",
        input: Input::Float32,
        args: args!["-stats", "-mats"],
        mode: "fix",
        named: &["FSL-01", "FSL-08"],
        output_type: "NIFTI",
    },
    Case {
        id: "mcflirt.stats.reproduce",
        tool: "fsl mcflirt",
        input: Input::Float32,
        args: args!["-stats", "-mats"],
        mode: "reproduce",
        named: &["FSL-01", "FSL-08"],
        output_type: "NIFTI",
    },
    Case {
        id: "mcflirt.gdt.reproduce",
        tool: "fsl mcflirt",
        input: Input::Float32,
        args: args!["-gdt", "-mats"],
        mode: "reproduce",
        named: &["FSL-01"],
        output_type: "NIFTI",
    },
    Case {
        id: "mcflirt.uint8",
        tool: "fsl mcflirt",
        input: Input::Uint8,
        args: args!["-mats"],
        mode: "fix",
        named: &["FSL-01", "FSL-09"],
        output_type: "NIFTI",
    },
    Case {
        id: "mcflirt.int8",
        tool: "fsl mcflirt",
        input: Input::Int8,
        args: args!["-mats"],
        mode: "fix",
        named: &["FSL-01", "FSL-09"],
        output_type: "NIFTI",
    },
    Case {
        id: "mcflirt.int8.reproduce",
        tool: "fsl mcflirt",
        input: Input::Int8,
        args: args!["-mats"],
        mode: "reproduce",
        named: &["FSL-01", "FSL-09"],
        output_type: "NIFTI",
    },
    Case {
        id: "mcflirt.nonfinite",
        tool: "fsl mcflirt",
        input: Input::Float32NonFinite,
        args: args!["-mats", "-plots", "-rmsabs"],
        mode: "fix",
        named: &["FSL-01", "FSL-14"],
        output_type: "NIFTI",
    },
    Case {
        id: "mcflirt.nonfinite.reproduce",
        tool: "fsl mcflirt",
        input: Input::Float32NonFinite,
        args: args!["-mats", "-plots", "-rmsabs"],
        mode: "reproduce",
        named: &["FSL-01", "FSL-14"],
        output_type: "NIFTI",
    },
    Case {
        id: "mcflirt.bigendian.neurological.nifti2",
        tool: "fsl mcflirt",
        input: Input::Float32BigEndianNeurological,
        args: args!["-mats", "-plots"],
        mode: "fix",
        named: &["FSL-01"],
        output_type: "NIFTI2",
    },
    Case {
        id: "mcflirt.milliseconds",
        tool: "fsl mcflirt",
        input: Input::Int16Milliseconds,
        args: args!["-stages", "1", "-mats"],
        mode: "fix",
        named: &["FSL-01", "FSL-09", "FSL-10"],
        output_type: "NIFTI",
    },
    Case {
        id: "mcflirt.milliseconds.reproduce",
        tool: "fsl mcflirt",
        input: Input::Int16Milliseconds,
        args: args!["-stages", "1", "-mats"],
        mode: "reproduce",
        named: &["FSL-01", "FSL-09", "FSL-10"],
        output_type: "NIFTI",
    },
];

// ------------------------------------------------------------------------------------------------
// Inputs

fn splitmix64(seed: u64, n: usize) -> Vec<u64> {
    (1..=n as u64)
        .map(|i| {
            let mut z = seed.wrapping_add(i.wrapping_mul(0x9E37_79B9_7F4A_7C15));
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^ (z >> 31)
        })
        .collect()
}

/// A smooth head-like volume (an ellipsoid with an inner structure, integer arithmetic only)
/// displaced by `shift` voxels (x, y, z in quarter voxels, rounded down), with noise.
fn head_volume(shape: [usize; 3], shift: [i64; 3], seed: u64) -> Vec<i64> {
    let [nx, ny, nz] = shape;
    let noise = splitmix64(seed, nx * ny * nz);
    let mut out = Vec::with_capacity(nx * ny * nz);
    for z in 0..nz as i64 {
        for y in 0..ny as i64 {
            for x in 0..nx as i64 {
                // Coordinates in quarter voxels from the centre, minus the shift.
                let cx = 4 * x - 2 * (nx as i64 - 1) - shift[0];
                let cy = 4 * y - 2 * (ny as i64 - 1) - shift[1];
                let cz = 4 * z - 2 * (nz as i64 - 1) - shift[2];
                // The ellipsoid's squared radius, scaled to 0..10000 at its edge.
                let r = 10_000 * cx * cx / (24 * 24)
                    + 10_000 * cy * cy / (22 * 22)
                    + 10_000 * cz * cz / (12 * 12);
                let mut v = if r < 10_000 {
                    600 + (10_000 - r) / 25
                } else {
                    20
                };
                // A bright off-centre structure, so that rotations are visible.
                let (sx, sy, sz) = (cx - 8, cy + 6, cz);
                if sx * sx + sy * sy + 2 * sz * sz < 64 {
                    v += 300;
                }
                let n = (noise[out.len()] % 41) as i64 - 20;
                out.push(v + n);
            }
        }
    }
    out
}

/// The 4 volumes of a series of `shape` (moving: a few quarter-voxel shifts and a skew that
/// acts as a small rotation).
fn series(shape: [usize; 3]) -> Vec<i64> {
    let shifts = [[0, 0, 0], [1, -1, 0], [2, 1, 1], [-1, 2, -1]];
    let mut v = Vec::new();
    for (t, s) in shifts.iter().enumerate() {
        v.extend(head_volume(shape, *s, 0x5EED_0000 + t as u64));
    }
    v
}

/// A NIfTI-1 single file (`.nii`): `datatype`, `bitpix`, the data bytes (in the file's byte
/// order), an sform of `pixdim` (no qform) whose x axis runs right to left (a negative
/// determinant, FSL's radiological order) or, with `neurological`, left to right, `xyzt_units`
/// and the TR.
#[allow(clippy::too_many_arguments)]
fn nifti_file(
    path: &Path,
    dims: &[usize],
    datatype: i16,
    bitpix: i16,
    data: &[u8],
    big_endian: bool,
    neurological: bool,
    units: u8,
    tr: f32,
) {
    let mut h = vec![0u8; 352];
    let put_i32 = |h: &mut Vec<u8>, o: usize, v: i32| {
        let b = if big_endian {
            v.to_be_bytes()
        } else {
            v.to_le_bytes()
        };
        h[o..o + 4].copy_from_slice(&b);
    };
    let put_i16 = |h: &mut Vec<u8>, o: usize, v: i16| {
        let b = if big_endian {
            v.to_be_bytes()
        } else {
            v.to_le_bytes()
        };
        h[o..o + 2].copy_from_slice(&b);
    };
    let put_f32 = |h: &mut Vec<u8>, o: usize, v: f32| {
        let b = if big_endian {
            v.to_be_bytes()
        } else {
            v.to_le_bytes()
        };
        h[o..o + 4].copy_from_slice(&b);
    };
    put_i32(&mut h, 0, 348);
    h[38] = b'r';
    put_i16(&mut h, 40, dims.len() as i16);
    for (k, d) in dims.iter().enumerate() {
        put_i16(&mut h, 42 + 2 * k, *d as i16);
    }
    for k in dims.len()..7 {
        put_i16(&mut h, 42 + 2 * k, 1);
    }
    put_i16(&mut h, 70, datatype);
    put_i16(&mut h, 72, bitpix);
    put_f32(&mut h, 76, 1.0);
    for k in 0..3 {
        put_f32(&mut h, 80 + 4 * k, PIXDIM[k]);
    }
    put_f32(&mut h, 92, tr);
    put_f32(&mut h, 108, 352.0);
    put_f32(&mut h, 112, 1.0);
    h[123] = units;
    // sform_code 1 (scanner), no qform.
    put_i16(&mut h, 254, 1);
    let xsign = if neurological { 1.0 } else { -1.0 };
    let offsets = [xsign * -24.0, -21.0, -14.0];
    let srow = [
        [xsign * PIXDIM[0], 0.0, 0.0, offsets[0]],
        [0.0, PIXDIM[1], 0.0, offsets[1]],
        [0.0, 0.0, PIXDIM[2], offsets[2]],
    ];
    for (r, row) in srow.iter().enumerate() {
        for (c, v) in row.iter().enumerate() {
            put_f32(&mut h, 280 + 16 * r + 4 * c, *v);
        }
    }
    h[344..348].copy_from_slice(b"n+1\0");
    let mut bytes = h;
    bytes.extend_from_slice(data);
    std::fs::write(path, bytes).unwrap();
}

fn write_inputs(dir: &Path) -> BTreeMap<Input, PathBuf> {
    let [nx, ny, nz, nt] = SHAPE;
    let values = series([nx, ny, nz]);
    let mut paths = BTreeMap::new();
    let dims = [nx, ny, nz, nt];
    let as_bytes_i16 =
        |v: &[i64]| -> Vec<u8> { v.iter().flat_map(|&x| (x as i16).to_le_bytes()).collect() };
    let as_bytes_f32 = |v: &[i64], be: bool| -> Vec<u8> {
        v.iter()
            .flat_map(|&x| {
                let f = x as f32 * 0.5;
                if be { f.to_be_bytes() } else { f.to_le_bytes() }
            })
            .collect()
    };
    let p = dir.join("int16.nii");
    nifti_file(
        &p,
        &dims,
        4,
        16,
        &as_bytes_i16(&values),
        false,
        false,
        10,
        2.0,
    );
    paths.insert(Input::Int16, p);
    let p = dir.join("float32.nii");
    nifti_file(
        &p,
        &dims,
        16,
        32,
        &as_bytes_f32(&values, false),
        false,
        false,
        10,
        2.0,
    );
    paths.insert(Input::Float32, p);
    let max = *values.iter().max().unwrap();
    let u8s: Vec<u8> = values
        .iter()
        .map(|&x| (x.max(0) * 250 / max) as u8)
        .collect();
    let p = dir.join("uint8.nii");
    nifti_file(&p, &dims, 2, 8, &u8s, false, false, 10, 2.0);
    paths.insert(Input::Uint8, p);
    let i8s: Vec<u8> = values
        .iter()
        .map(|&x| ((x.max(0) * 250 / max) - 125) as i8 as u8)
        .collect();
    let p = dir.join("int8.nii");
    nifti_file(&p, &dims, 256, 8, &i8s, false, false, 10, 2.0);
    paths.insert(Input::Int8, p);
    let mut nonfinite = as_bytes_f32(&values, false);
    let per = nx * ny * nz;
    let put =
        |b: &mut Vec<u8>, i: usize, v: f32| b[4 * i..4 * i + 4].copy_from_slice(&v.to_le_bytes());
    put(&mut nonfinite, 2 * per + per / 2 + 3, f32::NAN);
    put(&mut nonfinite, 3 * per + 7, f32::INFINITY);
    let p = dir.join("float32-nonfinite.nii");
    nifti_file(&p, &dims, 16, 32, &nonfinite, false, false, 10, 2.0);
    paths.insert(Input::Float32NonFinite, p);
    let p = dir.join("float32-be-neuro.nii");
    nifti_file(
        &p,
        &dims,
        16,
        32,
        &as_bytes_f32(&values, true),
        true,
        true,
        10,
        2.0,
    );
    paths.insert(Input::Float32BigEndianNeurological, p);
    let p = dir.join("int16-ms.nii");
    nifti_file(
        &p,
        &dims,
        4,
        16,
        &as_bytes_i16(&values),
        false,
        false,
        2 | 16,
        2000.0,
    );
    paths.insert(Input::Int16Milliseconds, p);
    // A 3-slice slab of the same head (the in-plane mode: thinner than -fov's 20 mm).
    let slab_shape = [nx, ny, 3];
    let slab = series(slab_shape);
    let p = dir.join("slab.nii");
    nifti_file(
        &p,
        &[nx, ny, 3, nt],
        16,
        32,
        &as_bytes_f32(&slab, false),
        false,
        false,
        10,
        2.0,
    );
    paths.insert(Input::Slab, p);
    // The reference: the first volume of the float32 series, alone.
    let reference = as_bytes_f32(&values[..per], false);
    nifti_file(
        &dir.join("reference.nii"),
        &[nx, ny, nz],
        16,
        32,
        &reference,
        false,
        false,
        10,
        0.0,
    );
    std::fs::write(
        dir.join("init.mat"),
        "1 0 0 0.75\n0 1 0 -0.5\n0 0 1 0.25\n0 0 0 1\n",
    )
    .unwrap();
    paths
}

// ------------------------------------------------------------------------------------------------
// Fingerprints

struct Fingerprint {
    sha256: String,
    /// The NIfTI data type of an image's data (`-` for a header or a text file).
    dtype: String,
    /// The values at [`N_SAMPLES`] fixed positions, as hex of their little-endian bytes.
    sample: String,
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

fn sha256(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

fn dtype_name(code: i16) -> (&'static str, usize) {
    match code {
        2 => ("uint8", 1),
        4 => ("int16", 2),
        8 => ("int32", 4),
        16 => ("float32", 4),
        64 => ("float64", 8),
        256 => ("int8", 1),
        other => panic!("unexpected NIfTI data type {other}"),
    }
}

/// `header` (the bytes before the data) and `data` fingerprints of a little-endian NIfTI-1 or
/// NIfTI-2 file.
fn nifti_fingerprints(path: &Path) -> Vec<(String, Fingerprint)> {
    let raw = std::fs::read(path).unwrap();
    let (datatype, offset) = match i32::from_le_bytes(raw[0..4].try_into().unwrap()) {
        348 => (
            i16::from_le_bytes([raw[70], raw[71]]),
            f32::from_le_bytes(raw[108..112].try_into().unwrap()) as usize,
        ),
        540 => (
            i16::from_le_bytes([raw[12], raw[13]]),
            i64::from_le_bytes(raw[168..176].try_into().unwrap()) as usize,
        ),
        _ => panic!("{}: not little-endian NIfTI-1 or NIfTI-2", path.display()),
    };
    let (name, size) = dtype_name(datatype);
    let data = &raw[offset..];
    let n = data.len() / size;
    let mut sample = Vec::new();
    for s in 0..N_SAMPLES.min(n) {
        let i = s * (n - 1) / (N_SAMPLES - 1);
        sample.extend_from_slice(&data[i * size..(i + 1) * size]);
    }
    vec![
        (
            "header".into(),
            Fingerprint {
                sha256: sha256(&raw[..offset]),
                dtype: "-".into(),
                sample: "-".into(),
            },
        ),
        (
            "data".into(),
            Fingerprint {
                sha256: sha256(data),
                dtype: name.into(),
                sample: hex(&sample),
            },
        ),
    ]
}

/// Every file a run wrote in its directory `dir`, by its path there: images by header and
/// data, the others by their bytes.
fn case_fingerprints(dir: &Path) -> Vec<(String, Fingerprint)> {
    let mut files = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else {
                files.push(p);
            }
        }
    }
    files.sort();
    let mut out = Vec::new();
    for p in files {
        let name = p
            .strip_prefix(dir)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        if name.ends_with(".nii") {
            for (part, fp) in nifti_fingerprints(&p) {
                out.push((format!("{name}.{part}"), fp));
            }
        } else {
            let bytes = std::fs::read(&p).unwrap();
            out.push((
                name,
                Fingerprint {
                    sha256: sha256(&bytes),
                    dtype: "-".into(),
                    sample: "-".into(),
                },
            ));
        }
    }
    out
}

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

/// How two data fingerprints differ at their sample positions.
fn describe(expected: &Fingerprint, actual: &Fingerprint) -> String {
    if expected.dtype != actual.dtype {
        return format!("type {} expected, {} found", expected.dtype, actual.dtype);
    }
    let (e, a) = (unhex(&expected.sample), unhex(&actual.sample));
    let size = match expected.dtype.as_str() {
        "uint8" | "int8" => 1,
        "int16" => 2,
        "float32" | "int32" => 4,
        "float64" => 8,
        _ => return String::new(),
    };
    let differ = e
        .chunks(size)
        .zip(a.chunks(size))
        .filter(|(x, y)| x != y)
        .count();
    format!("{differ} of {} sampled values differ", e.len() / size)
}

// ------------------------------------------------------------------------------------------------
// golden.tsv

type Recorded = BTreeMap<(String, String), Fingerprint>;

fn read_golden() -> (String, Recorded) {
    let text = std::fs::read_to_string(GOLDEN).unwrap_or_else(|e| panic!("{GOLDEN}: {e}"));
    let mut source = String::new();
    let mut recorded = Recorded::new();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("# Recorded from ") {
            source = rest.to_owned();
        }
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let f: Vec<&str> = line.split('\t').collect();
        assert_eq!(f.len(), 5, "{GOLDEN}: bad line {line:?}");
        recorded.insert(
            (f[0].to_owned(), f[1].to_owned()),
            Fingerprint {
                sha256: f[2].into(),
                dtype: f[3].into(),
                sample: f[4].into(),
            },
        );
    }
    (source, recorded)
}

fn git(args: &[&str]) -> String {
    let out = Command::new("git")
        .args(args)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("git");
    assert!(out.status.success(), "git {args:?}: {out:?}");
    String::from_utf8(out.stdout).unwrap().trim().to_owned()
}

/// Today's date (UTC) as `YYYY-MM-DD` (Howard Hinnant's `civil_from_days`).
fn today() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let z = (secs / 86_400) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

fn record(actual: &Recorded) {
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        panic!("golden checksums are recorded on Linux x86_64 only (CLAUDE.md rule 10)");
    }
    let dirty: Vec<String> = git(&["status", "--porcelain"])
        .lines()
        .filter(|l| !l.ends_with("golden.tsv"))
        .map(str::to_owned)
        .collect();
    let allow = std::env::var_os("LARMORX_GOLDEN_ALLOW_DIRTY").is_some();
    assert!(
        dirty.is_empty() || allow,
        "commit first (the checksums name their commit) or set LARMORX_GOLDEN_ALLOW_DIRTY=1:\n{}",
        dirty.join("\n")
    );
    let commit = git(&["rev-parse", "HEAD"]) + if dirty.is_empty() { "" } else { "+dirty" };
    let mut text = String::from(
        "# SPDX-License-Identifier: LicenseRef-FSL\n\
         # Golden checksums of the larmorx-nc replicas (tests/golden.rs): SHA-256 of the files\n\
         # larmorx-nc writes on Linux x86_64 (images by header and data, with sampled values).\n\
         # Every platform must reproduce them (docs/validation/golden.md). Do not edit by hand.\n",
    );
    let _ = writeln!(
        text,
        "# Recorded from {commit} on {} (Linux x86_64)",
        today()
    );
    text.push_str("# case\toutput\tsha256\ttype\tsample\n");
    for ((case, output), fp) in actual {
        let _ = writeln!(
            text,
            "{case}\t{output}\t{}\t{}\t{}",
            fp.sha256, fp.dtype, fp.sample
        );
    }
    std::fs::write(GOLDEN, text).unwrap();
    eprintln!("recorded {} outputs in {GOLDEN}", actual.len());
}

fn mismatch_dir() -> PathBuf {
    std::env::var_os("LARMORX_GOLDEN_MISMATCHES")
        .map_or_else(
            || Path::new(env!("CARGO_TARGET_TMPDIR")).join("golden-mismatches"),
            PathBuf::from,
        )
        .join("larmorx-nc")
}

// ------------------------------------------------------------------------------------------------
// Tests

#[test]
fn golden() {
    let dir = tempfile::tempdir().unwrap();
    let inputs = write_inputs(dir.path());
    let mut actual = Recorded::new();
    let mut runs = BTreeMap::new();
    for case in CASES {
        // Each case in its own directory, so that its outputs are exactly the files there.
        let run = dir.path().join(case.id);
        std::fs::create_dir(&run).unwrap();
        let (family, tool) = case.tool.split_once(' ').unwrap();
        let mut args: Vec<String> = vec![family.into(), tool.into()];
        args.extend(["-in".into(), inputs[&case.input].display().to_string()]);
        args.extend(["-out".into(), case.id.to_owned()]);
        for a in case.args {
            args.push(match *a {
                "@ref" => dir.path().join("reference.nii").display().to_string(),
                "@init" => dir.path().join("init.mat").display().to_string(),
                other => other.to_owned(),
            });
        }
        let out = Command::new(EXE)
            .args(&args)
            .current_dir(&run)
            .env_remove("FSLDIR")
            .env("FSLOUTPUTTYPE", case.output_type)
            .env("OMP_NUM_THREADS", "2")
            .env("LARMORX_UPSTREAM_DEFECTS", case.mode)
            .output()
            .unwrap();
        assert!(out.status.success(), "{}: {out:?}", case.id);
        // The notices (`larmorx-nc: upstream defect <ID> (mcflirt) corrected: ...`).
        let stderr = String::from_utf8_lossy(&out.stderr);
        let mut found: Vec<&str> = stderr
            .lines()
            .filter_map(|l| l.strip_prefix("larmorx-nc: upstream defect "))
            .filter_map(|rest| rest.split(' ').next())
            .collect();
        found.sort_unstable();
        found.dedup();
        assert_eq!(found, case.named, "{}: {stderr}", case.id);
        let verb = if case.mode == "fix" {
            "corrected"
        } else {
            "reproduced"
        };
        for id in case.named {
            let named = stderr.lines().any(|l| {
                l.contains(&format!("upstream defect {id} (")) && l.contains(&format!(") {verb}: "))
            });
            assert!(named, "{}: {stderr}", case.id);
        }
        let fps = case_fingerprints(&run);
        assert!(!fps.is_empty(), "{}: no outputs", case.id);
        for (name, fp) in fps {
            actual.insert((case.id.to_owned(), name), fp);
        }
        runs.insert(case.id, run);
    }

    if std::env::var_os("LARMORX_GOLDEN_RECORD").is_some() {
        record(&actual);
        return;
    }
    let (source, recorded) = read_golden();
    let mut problems = Vec::new();
    let mut bad = std::collections::BTreeSet::new();
    for key in recorded.keys().chain(actual.keys()) {
        let (case, output) = key;
        let line = match (recorded.get(key), actual.get(key)) {
            (Some(e), Some(a)) if e.sha256 == a.sha256 => continue,
            (Some(e), Some(a)) => {
                let mut l = format!(
                    "{case} {output}: sha256 {}… expected, {}… found",
                    &e.sha256[..16],
                    &a.sha256[..16]
                );
                if output.ends_with(".data") {
                    let _ = write!(l, "; {}", describe(e, a));
                }
                l
            }
            (Some(_), None) => format!("{case} {output}: recorded but no longer produced"),
            (None, _) => format!("{case} {output}: not recorded (record on Linux x86_64)"),
        };
        if bad.insert((case.clone(), output.clone())) {
            problems.push(line);
        }
    }
    if !problems.is_empty() {
        let root = mismatch_dir();
        for (case, _) in &bad {
            if let Some(run) = runs.get(case.as_str()) {
                let dest = root.join(case);
                std::fs::create_dir_all(&dest).unwrap();
                for e in std::fs::read_dir(run).unwrap() {
                    let p = e.unwrap().path();
                    if p.is_file() {
                        std::fs::copy(&p, dest.join(p.file_name().unwrap())).unwrap();
                    }
                }
            }
        }
        panic!(
            "larmorx-nc differs from its Linux x86_64 output (recorded from {source}):\n  {}\n\
             The outputs are saved in {}.",
            problems.join("\n  "),
            root.display()
        );
    }
}

#[test]
fn every_tool_has_a_golden_case() {
    // The families and tools as the binary lists them in its help.
    let out = Command::new(EXE).arg("--help").output().unwrap();
    let help = String::from_utf8(out.stdout).unwrap();
    let tools: Vec<String> = help
        .split("Tools:\n")
        .nth(1)
        .expect("a Tools: section in --help")
        .lines()
        .take_while(|l| !l.trim().is_empty())
        .flat_map(|l| {
            let mut words = l.split_whitespace();
            let family = words.next().unwrap().to_owned();
            let rest: String = words.collect::<Vec<_>>().join(" ");
            rest.split(", ")
                .map(|t| format!("{family} {t}"))
                .collect::<Vec<_>>()
        })
        .collect();
    assert!(!tools.is_empty(), "{help}");
    let missing: Vec<&String> = tools
        .iter()
        .filter(|t| !CASES.iter().any(|c| c.tool == t.as_str()))
        .collect();
    assert!(
        missing.is_empty(),
        "no golden case for {missing:?}: add one to CASES in tests/golden.rs and record it on \
         Linux x86_64 (docs/validation/golden.md)"
    );
}
