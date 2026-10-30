// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Karel Lopez Vilaret. Written for larmorx-gpl and relicensed to Apache-2.0 by its
// author on 2026-10-29, when the AFNI replicas moved into larmorx.
//! Golden tests of the AFNI replicas: every platform reproduces the output they give on Linux
//! x86_64, bit for bit (CLAUDE.md rule 10, `docs/validation/golden.md`). Until 2026-10-29 these
//! were the `larmorx-gpl` program's (`crates-gpl/larmorx-gpl-cli/tests/golden.rs`); the
//! checksums did not change with the move.
//!
//! The inputs are generated here from seeded integers (SplitMix64, the same generator as the
//! Python golden tests) and plain integer arithmetic. Each case runs the `larmorx` binary's
//! built-in replica (`LARMORX_IMPLEMENTATION=replica`) on them and hashes (SHA-256) the header and the data of the NIfTI file it writes; the
//! hashes are compared with `tests/afni_replica_golden.tsv`. One case per distinct code path: each
//! interpolation method, each stored type, the restore and detrend options, and the paths of
//! AFNI's defects (D13): each corrected (the default, `LARMORX_UPSTREAM_DEFECTS=fix`) and,
//! in the `*.reproduce` cases, reproduced.
//!
//! Recording, on Linux x86_64 only, from a committed tree whose replica parity passes:
//! `LARMORX_GOLDEN_RECORD=1 cargo test --locked -p larmorx-cli --test afni_replica_golden`
//! (`LARMORX_GOLDEN_ALLOW_DIRTY=1` to record from uncommitted changes). On a mismatch the
//! outputs are saved in `$LARMORX_GOLDEN_MISMATCHES` (CI uploads it), else in
//! `target/tmp/golden-mismatches`.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use larmorx_core::affine::Affine;
use larmorx_core::element::{DataType, Element};
use larmorx_core::ndarray::{ArrayViewD, IxDyn, ShapeBuilder};
use larmorx_io::nifti::{self, NiftiHeader, NiftiVersion, WriteOptions};
use sha2::{Digest, Sha256};

const EXE: &str = env!("CARGO_BIN_EXE_larmorx");
const GOLDEN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/afni_replica_golden.tsv");
/// Values kept per data output, at fixed positions, to tell how large a mismatch is.
const N_SAMPLES: usize = 8;

/// The 3dTshift series: 8 × 6 × 5 voxels, 10 time points (AFNI needs at least 5).
const SHAPE: [usize; 4] = [8, 6, 5, 10];

#[derive(Clone, Copy)]
enum Input {
    Float32,
    /// int16 with `scl_slope` 0.25.
    Int16,
    Uint8,
    /// int16 with `scl_slope` -0.25 (AFNI-01).
    Int16Negative,
    /// float32 with `scl_slope` 2 and `scl_inter` 10 (AFNI-02), an oblique sform in metres
    /// with an origin (AFNI-07), and header slice code 2 (AFNI 25.2.09's AFNI-03, which AFNI
    /// 26.3.05 fixed: both modes read the slice times as NIfTI defines them).
    Float32Defects,
    /// float32 with header slice times 0.5 ms apart beyond a 1 ms TR: a copy of the input
    /// that keeps them (AFNI-08).
    Float32TinyOffsets,
    /// float32 with an sform 1e-4 rad (0.006°) from plumb, which AFNI writes plumb (AFNI-16).
    Float32NearlyPlumb,
    /// int16 with `scl_slope` 0.25 and `scl_inter` 100, whose intercept `AFNI_NIFTI_SCALE=NO`
    /// drops (AFNI-23).
    Int16Intercept,
    /// A T1-like head of 24 × 24 × 16 voxels, int16 ([`head_values`]).
    Head,
    /// The T2-like contrast of the same head, float32.
    HeadT2,
    /// 4 volumes of the T2-like head, int16, 4 mm voxels (3dUnifize -EPI; 3dAutomask's average).
    Head4d,
    /// The same 4 volumes stored negated with `scl_slope` -1 (AFNI-01).
    Head4dNegative,
    /// One axial slice of the head, 24 × 24 × 1, float32 (the 2D automask).
    Slice,
}

struct Case {
    id: &'static str,
    /// `<family> <tool>`, as `larmorx --help` lists it.
    tool: &'static str,
    input: Input,
    args: &'static [&'static str],
    /// `LARMORX_UPSTREAM_DEFECTS`: `fix` (the default) or `reproduce`.
    mode: &'static str,
    /// The upstream defects the run must name on standard error, and no other.
    named: &'static [&'static str],
    /// AFNI's environment variables set for the run.
    env: &'static [(&'static str, &'static str)],
    /// Whether a file of the output's name exists before the run.
    existing: bool,
    /// More outputs: `(option, name)`; the option is given `<id>.<name>.nii`, and the file's
    /// fingerprints are recorded as `<name>.header` and `<name>.data`.
    outputs: &'static [(&'static str, &'static str)],
}

const CASES: &[Case] = &[
    Case {
        id: "3dTshift.fourier.int16",
        tool: "afni 3dTshift",
        env: &[],
        existing: false,
        outputs: &[],
        named: &[],
        mode: "fix",
        input: Input::Int16,
        args: &["-tpattern", "alt+z2"],
    },
    Case {
        id: "3dTshift.fourier.float32.ignore",
        tool: "afni 3dTshift",
        env: &[],
        existing: false,
        outputs: &[],
        named: &[],
        mode: "fix",
        input: Input::Float32,
        args: &["-tpattern", "alt+z", "-ignore", "2"],
    },
    Case {
        id: "3dTshift.linear.float32",
        tool: "afni 3dTshift",
        env: &[],
        existing: false,
        outputs: &[],
        named: &[],
        mode: "fix",
        input: Input::Float32,
        args: &["-tpattern", "seq+z", "-linear"],
    },
    Case {
        id: "3dTshift.cubic.int16.rlt+",
        tool: "afni 3dTshift",
        env: &[],
        existing: false,
        outputs: &[],
        named: &[],
        mode: "fix",
        input: Input::Int16,
        args: &["-tpattern", "alt-z", "-cubic", "-rlt+"],
    },
    Case {
        id: "3dTshift.quintic.float32.tzero",
        tool: "afni 3dTshift",
        env: &[],
        existing: false,
        outputs: &[],
        named: &[],
        mode: "fix",
        input: Input::Float32,
        args: &["-tpattern", "alt+z2", "-quintic", "-tzero", "0"],
    },
    Case {
        id: "3dTshift.heptic.uint8.no_detrend",
        tool: "afni 3dTshift",
        env: &[],
        existing: false,
        outputs: &[],
        named: &[],
        mode: "fix",
        input: Input::Uint8,
        args: &["-tpattern", "seq-z", "-heptic", "-no_detrend"],
    },
    Case {
        id: "3dTshift.wsinc5.float32.rlt",
        tool: "afni 3dTshift",
        env: &[],
        existing: false,
        outputs: &[],
        named: &[],
        mode: "fix",
        input: Input::Float32,
        args: &[
            "-tpattern",
            "@1D: 0 1.5 0.5 1 0.25",
            "-TR",
            "2000ms",
            "-tzero",
            "0.3",
            "-wsinc5",
            "-rlt",
        ],
    },
    Case {
        id: "3dTshift.wsinc9.int16.slice",
        tool: "afni 3dTshift",
        env: &[],
        existing: false,
        outputs: &[],
        named: &[],
        mode: "fix",
        input: Input::Int16,
        args: &["-tpattern", "alt+z", "-wsinc9", "-slice", "3"],
    },
    Case {
        id: "3dTshift.linear.int16-negative-slope",
        tool: "afni 3dTshift",
        env: &[],
        existing: false,
        outputs: &[],
        named: &["AFNI-01"],
        mode: "fix",
        input: Input::Int16Negative,
        args: &["-tpattern", "alt+z", "-linear"],
    },
    Case {
        id: "3dTshift.cubic.float32-defects",
        tool: "afni 3dTshift",
        env: &[],
        existing: false,
        outputs: &[],
        named: &["AFNI-02", "AFNI-07"],
        mode: "fix",
        input: Input::Float32Defects,
        args: &["-cubic"],
    },
    Case {
        id: "3dTshift.copy.float32-tiny-offsets",
        tool: "afni 3dTshift",
        env: &[],
        existing: false,
        outputs: &[],
        named: &["AFNI-08"],
        mode: "fix",
        input: Input::Float32TinyOffsets,
        args: &[],
    },
    Case {
        id: "3dTshift.linear.float32-nearly-plumb",
        tool: "afni 3dTshift",
        env: &[],
        existing: false,
        outputs: &[],
        named: &["AFNI-16"],
        mode: "fix",
        input: Input::Float32NearlyPlumb,
        args: &["-tpattern", "seq+z", "-linear"],
    },
    Case {
        id: "3dTshift.linear.int16-negative-slope.reproduce",
        tool: "afni 3dTshift",
        env: &[],
        existing: false,
        outputs: &[],
        named: &["AFNI-01"],
        mode: "reproduce",
        input: Input::Int16Negative,
        args: &["-tpattern", "alt+z", "-linear"],
    },
    Case {
        id: "3dTshift.cubic.float32-defects.reproduce",
        tool: "afni 3dTshift",
        env: &[],
        existing: false,
        outputs: &[],
        named: &["AFNI-02", "AFNI-07"],
        mode: "reproduce",
        input: Input::Float32Defects,
        args: &["-cubic"],
    },
    Case {
        id: "3dTshift.copy.float32-tiny-offsets.reproduce",
        tool: "afni 3dTshift",
        env: &[],
        existing: false,
        outputs: &[],
        named: &["AFNI-08"],
        mode: "reproduce",
        input: Input::Float32TinyOffsets,
        args: &[],
    },
    Case {
        id: "3dTshift.linear.float32-nearly-plumb.reproduce",
        tool: "afni 3dTshift",
        env: &[],
        existing: false,
        outputs: &[],
        named: &["AFNI-16"],
        mode: "reproduce",
        input: Input::Float32NearlyPlumb,
        args: &["-tpattern", "seq+z", "-linear"],
    },
    Case {
        id: "3dTshift.linear.float32.overwrite",
        tool: "afni 3dTshift",
        env: &[],
        existing: true,
        outputs: &[],
        named: &[],
        mode: "fix",
        input: Input::Float32,
        args: &["-tpattern", "seq+z", "-overwrite", "-linear"],
    },
    Case {
        id: "3dTshift.linear.float32.nifti2",
        tool: "afni 3dTshift",
        env: &[],
        existing: false,
        outputs: &[],
        named: &[],
        mode: "fix",
        input: Input::Float32,
        args: &["-DAFNI_NIFTI_WRITE_TYPE=2", "-tpattern", "seq+z", "-linear"],
    },
    Case {
        id: "3dTshift.linear.float32.nifti2-environment",
        tool: "afni 3dTshift",
        env: &[("AFNI_NIFTI_WRITE_TYPE", "2")],
        existing: false,
        outputs: &[],
        named: &[],
        mode: "fix",
        input: Input::Float32,
        args: &["-tpattern", "seq+z", "-linear"],
    },
    Case {
        id: "3dTshift.linear.float32.msec",
        tool: "afni 3dTshift",
        env: &[],
        existing: false,
        outputs: &[],
        named: &["AFNI-19"],
        mode: "fix",
        input: Input::Float32,
        args: &[
            "-TR",
            "2000ms",
            "-tpattern",
            "seq+z",
            "-linear",
            "-DAFNI_ALLOW_MILLISECONDS=YES",
        ],
    },
    Case {
        id: "3dTshift.linear.int16-intercept.nifti-scale-no",
        tool: "afni 3dTshift",
        env: &[],
        existing: false,
        outputs: &[],
        named: &["AFNI-23"],
        mode: "fix",
        input: Input::Int16Intercept,
        args: &["-DAFNI_NIFTI_SCALE=NO", "-tpattern", "seq+z", "-linear"],
    },
    Case {
        id: "3dTshift.linear.int16-intercept.nifti-scale-no.reproduce",
        tool: "afni 3dTshift",
        env: &[],
        existing: false,
        outputs: &[],
        named: &["AFNI-23"],
        mode: "reproduce",
        input: Input::Int16Intercept,
        args: &["-DAFNI_NIFTI_SCALE=NO", "-tpattern", "seq+z", "-linear"],
    },
    Case {
        id: "3dTshift.linear.int16-intercept.nifti-scale-no.rlt-ignore",
        tool: "afni 3dTshift",
        env: &[],
        existing: false,
        outputs: &[],
        named: &["AFNI-23"],
        mode: "fix",
        input: Input::Int16Intercept,
        args: &[
            "-DAFNI_NIFTI_SCALE=NO",
            "-tpattern",
            "seq+z",
            "-linear",
            "-rlt",
            "-ignore",
            "2",
        ],
    },
    Case {
        id: "3dTshift.linear.float32.msec.reproduce",
        tool: "afni 3dTshift",
        env: &[],
        existing: false,
        outputs: &[],
        named: &["AFNI-19"],
        mode: "reproduce",
        input: Input::Float32,
        args: &[
            "-TR",
            "2000ms",
            "-tpattern",
            "seq+z",
            "-linear",
            "-DAFNI_ALLOW_MILLISECONDS=YES",
        ],
    },
    Case {
        id: "3dAutomask.fmriprep.int16",
        tool: "afni 3dAutomask",
        env: &[],
        existing: false,
        outputs: &[("-apply_prefix", "apply"), ("-depth", "depth")],
        named: &[],
        mode: "fix",
        input: Input::Head,
        args: &["-dilate", "1"],
    },
    Case {
        id: "3dAutomask.eclip.nograd.NN1.erode",
        tool: "afni 3dAutomask",
        env: &[],
        existing: false,
        outputs: &[],
        named: &[],
        mode: "fix",
        input: Input::HeadT2,
        args: &[
            "-q", "-eclip", "-nograd", "-clfrac", "0.3", "-NN1", "-erode", "1", "-peels", "2",
        ],
    },
    Case {
        id: "3dAutomask.4d.si",
        tool: "afni 3dAutomask",
        env: &[],
        existing: false,
        outputs: &[("-apply_prefix", "apply")],
        named: &[],
        mode: "fix",
        input: Input::Head4d,
        args: &["-SI", "30", "-NN3", "-dilate", "2"],
    },
    Case {
        id: "3dAutomask.2d",
        tool: "afni 3dAutomask",
        env: &[],
        existing: false,
        outputs: &[],
        named: &[],
        mode: "fix",
        input: Input::Slice,
        args: &["-q"],
    },
    Case {
        id: "3dAutomask.peels0",
        tool: "afni 3dAutomask",
        env: &[],
        existing: false,
        outputs: &[],
        named: &["AFNI-43"],
        mode: "fix",
        input: Input::Head,
        args: &["-q", "-peels", "0"],
    },
    Case {
        id: "3dAutomask.peels0.reproduce",
        tool: "afni 3dAutomask",
        env: &[],
        existing: false,
        outputs: &[],
        named: &["AFNI-43"],
        mode: "reproduce",
        input: Input::Head,
        args: &["-q", "-peels", "0"],
    },
    Case {
        id: "3dUnifize.default.int16",
        tool: "afni 3dUnifize",
        env: &[],
        existing: false,
        outputs: &[("-ssave", "scale"), ("-amsave", "automasked")],
        named: &[],
        mode: "fix",
        input: Input::Head,
        args: &["-Urad", "5"],
    },
    Case {
        id: "3dUnifize.fmriprep.T2",
        tool: "afni 3dUnifize",
        env: &[],
        existing: false,
        outputs: &[],
        named: &[],
        mode: "fix",
        input: Input::HeadT2,
        args: &["-clfrac", "0.2", "-rbt", "6", "65.0", "90.0", "-T2"],
    },
    Case {
        id: "3dUnifize.GM.nosquash",
        tool: "afni 3dUnifize",
        env: &[],
        existing: false,
        outputs: &[],
        named: &[],
        mode: "fix",
        input: Input::Head,
        args: &["-quiet", "-GM", "-nosquash", "-Urad", "5"],
    },
    Case {
        id: "3dUnifize.T2T2",
        tool: "afni 3dUnifize",
        env: &[],
        existing: false,
        outputs: &[],
        named: &[],
        mode: "fix",
        input: Input::HeadT2,
        args: &["-T2", "-T2", "-Urad", "5"],
    },
    Case {
        id: "3dUnifize.EPI",
        tool: "afni 3dUnifize",
        env: &[],
        existing: false,
        outputs: &[],
        named: &[],
        mode: "fix",
        input: Input::Head4d,
        args: &["-EPI"],
    },
    Case {
        id: "3dUnifize.EPI.negative",
        tool: "afni 3dUnifize",
        env: &[],
        existing: false,
        outputs: &[],
        named: &["AFNI-01"],
        mode: "fix",
        input: Input::Head4dNegative,
        args: &["-EPI", "-quiet"],
    },
    Case {
        id: "3dUnifize.EPI.negative.reproduce",
        tool: "afni 3dUnifize",
        env: &[],
        existing: false,
        outputs: &[],
        named: &["AFNI-01"],
        mode: "reproduce",
        input: Input::Head4dNegative,
        args: &["-EPI", "-quiet"],
    },
    Case {
        id: "3dUnifize.T2up100",
        tool: "afni 3dUnifize",
        env: &[],
        existing: false,
        outputs: &[],
        named: &["AFNI-45"],
        mode: "fix",
        input: Input::HeadT2,
        args: &["-T2", "-T2up", "100", "-Urad", "5"],
    },
    Case {
        id: "3dUnifize.T2up100.reproduce",
        tool: "afni 3dUnifize",
        env: &[],
        existing: false,
        outputs: &[],
        named: &["AFNI-45"],
        mode: "reproduce",
        input: Input::HeadT2,
        args: &["-T2", "-T2up", "100", "-Urad", "5"],
    },
    Case {
        id: "3dUnifize.2d",
        tool: "afni 3dUnifize",
        env: &[],
        existing: false,
        outputs: &[],
        named: &[],
        mode: "fix",
        input: Input::Slice,
        args: &["-quiet", "-Urad", "5"],
    },
];

// ------------------------------------------------------------------------------------------------
// Inputs

/// `n` pseudo-random 64-bit integers: SplitMix64 started at `seed`.
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

/// Integer series with a trend and a slice- and voxel-dependent oscillation, in Fortran order
/// (the Python golden tests' `_series_values`).
fn series_values() -> Vec<i64> {
    let nvox = SHAPE[0] * SHAPE[1] * SHAPE[2];
    let n = nvox * SHAPE[3];
    let noise = splitmix64(11, n);
    (0..n)
        .map(|f| {
            let (v, t) = ((f % nvox) as i64, (f / nvox) as i64);
            800 + 3 * v + 5 * ((7 * t + 3 * v) % 11) + 2 * t + (noise[f] % 7) as i64
        })
        .collect()
}

/// A header for the inputs: plumb, 2 × 2 × 3 mm, TR 2 s, `scl_slope` = `slope`.
fn input_header(data_type: DataType, slope: f64) -> NiftiHeader {
    let mut h = NiftiHeader::new(NiftiVersion::V1, &SHAPE, data_type).unwrap();
    let affine = Affine::from_zooms([2.0, 2.0, 3.0], [-7.0, -5.0, -6.0]);
    h.set_sform(&affine, 1);
    h.set_qform(&affine, 1).unwrap();
    h.pixdim[4] = 2.0;
    h.xyzt_units = 2 | 8;
    h.scl_slope = slope;
    h.scl_inter = 0.0;
    h
}

fn write_with<T: Element>(path: &Path, h: &NiftiHeader, data: &[T]) {
    let view = ArrayViewD::from_shape(IxDyn(&SHAPE).f(), data).unwrap();
    nifti::write(path, h, view, &WriteOptions::default()).unwrap();
}

fn write_input<T: Element>(path: &Path, data: &[T], data_type: DataType, slope: f64) {
    write_with(path, &input_header(data_type, slope), data);
}

/// The float32 input of AFNI's defects (D13): stored values with `scl_slope` 2 and
/// `scl_inter` 10; an oblique sform in metres (a rotation about y with cosine 0.8 and sine 0.6;
/// steps 2, 2 and 3 mm) with an origin; header slice code 2 over all 5 slices, 0.4 s apart.
fn defects_header() -> NiftiHeader {
    let mut h = input_header(DataType::F32, 2.0);
    h.scl_inter = 10.0;
    let affine = Affine::from_linear(
        [
            [0.0016, 0.0, -0.0018],
            [0.0, 0.002, 0.0],
            [0.0012, 0.0, 0.0024],
        ],
        [-0.007, -0.005, -0.006],
    );
    h.set_sform(&affine, 1);
    h.set_qform(&affine, 1).unwrap();
    h.xyzt_units = 1 | 8;
    h.dim_info = 3 << 4;
    h.slice_code = 2;
    (h.slice_start, h.slice_end) = (0, 4);
    h.slice_duration = 0.4;
    h
}

/// The float32 input of AFNI-08: header slice code 1, slices 0.5 ms apart, a TR of 1 ms.
fn tiny_offsets_header() -> NiftiHeader {
    let mut h = input_header(DataType::F32, 0.0);
    h.pixdim[4] = 0.001;
    h.dim_info = 3 << 4;
    h.slice_code = 1;
    (h.slice_start, h.slice_end) = (0, 4);
    h.slice_duration = 0.0005;
    h
}

/// The float32 input of AFNI-16: the 2 × 2 × 3 mm grid turned by 1e-4 rad about y (the
/// entries of the rotation rounded to 4 decimals: plain arithmetic), which AFNI's test calls
/// plumb.
fn nearly_plumb_header() -> NiftiHeader {
    let mut h = input_header(DataType::F32, 0.0);
    let affine = Affine::from_linear(
        [[2.0, 0.0, -3.0e-4], [0.0, 2.0, 0.0], [2.0e-4, 0.0, 3.0]],
        [-7.0, -5.0, -6.0],
    );
    h.set_sform(&affine, 1);
    h.set_qform(&affine, 1).unwrap();
    h
}

/// The grid of the head inputs.
const HEAD: [usize; 3] = [24, 24, 16];

/// A synthetic head on [`HEAD`], from integers only: in an ellipsoid (doubled coordinates
/// `x = 2i - 23`, `y = 2j - 23`, `z = 2k - 15`; `r = x^2 + y^2 + 2z^2`), white matter
/// (`r < 150`), grey matter (`r < 300`), CSF (`r < 360`), a bright skull (`r < 440`) and
/// background, with the tissue values given; shaded along x (`(80 + 2i) / 100`) and with noise
/// (SplitMix64, `seed`, uniform in [-20, 20]).
fn head_values(levels: [i64; 5], seed: u64) -> Vec<i64> {
    let [nx, ny, nz] = HEAD;
    let noise = splitmix64(seed, nx * ny * nz);
    let mut v = Vec::with_capacity(nx * ny * nz);
    for k in 0..nz as i64 {
        for j in 0..ny as i64 {
            for i in 0..nx as i64 {
                let (x, y, z) = (2 * i - 23, 2 * j - 23, 2 * k - 15);
                let r = x * x + y * y + 2 * z * z;
                let level = match r {
                    0..150 => levels[0],
                    150..300 => levels[1],
                    300..360 => levels[2],
                    360..440 => levels[3],
                    _ => levels[4],
                };
                let n = (noise[v.len()] % 41) as i64 - 20;
                v.push(level * (80 + 2 * i) / 100 + n);
            }
        }
    }
    v
}

/// The T1-like levels (white, grey, CSF, skull, background) and the T2-like ones.
const T1_LEVELS: [i64; 5] = [1000, 700, 250, 900, 30];
const T2_LEVELS: [i64; 5] = [400, 700, 1500, 200, 30];

/// A header for a head input: plumb, `zoom` mm voxels, `scl_slope` = `slope`, TR 2 s for 4D.
fn head_header(shape: &[usize], data_type: DataType, zoom: f64, slope: f64) -> NiftiHeader {
    let mut h = NiftiHeader::new(NiftiVersion::V1, shape, data_type).unwrap();
    let affine = Affine::from_zooms([zoom, zoom, zoom], [-30.0, -40.0, -20.0]);
    h.set_sform(&affine, 1);
    h.set_qform(&affine, 1).unwrap();
    if shape.len() == 4 {
        h.pixdim[4] = 2.0;
        h.xyzt_units = 2 | 8;
    } else {
        h.xyzt_units = 2;
    }
    h.scl_slope = slope;
    h.scl_inter = 0.0;
    h
}

fn write_shaped<T: Element>(path: &Path, h: &NiftiHeader, shape: &[usize], data: &[T]) {
    let view = ArrayViewD::from_shape(IxDyn(shape).f(), data).unwrap();
    nifti::write(path, h, view, &WriteOptions::default()).unwrap();
}

/// The head inputs: `head`, `head-t2`, `head-4d`, `head-4d-negative` and `slice`.
fn write_head_inputs(dir: &Path, paths: &mut BTreeMap<&'static str, PathBuf>) {
    let [nx, ny, nz] = HEAD;
    let t1 = head_values(T1_LEVELS, 21);
    let t2 = head_values(T2_LEVELS, 22);
    let p = |name: &'static str, paths: &mut BTreeMap<&'static str, PathBuf>| {
        let path = dir.join(format!("{name}.nii"));
        paths.insert(name, path.clone());
        path
    };
    let i16s: Vec<i16> = t1.iter().map(|&v| v as i16).collect();
    write_shaped(
        &p("head", paths),
        &head_header(&HEAD, DataType::I16, 2.0, 0.0),
        &HEAD,
        &i16s,
    );
    let f32s: Vec<f32> = t2.iter().map(|&v| v as f32 / 2.0).collect();
    write_shaped(
        &p("head-t2", paths),
        &head_header(&HEAD, DataType::F32, 2.0, 0.0),
        &HEAD,
        &f32s,
    );
    let shape4 = [nx, ny, nz, 4];
    let noise = splitmix64(23, nx * ny * nz * 4);
    let series: Vec<i64> = (0..nx * ny * nz * 4)
        .map(|f| {
            let (v, t) = (f % (nx * ny * nz), (f / (nx * ny * nz)) as i64);
            t2[v] * (100 + 3 * t) / 100 + (noise[f] % 21) as i64 - 10
        })
        .collect();
    let i16s: Vec<i16> = series.iter().map(|&v| v as i16).collect();
    write_shaped(
        &p("head-4d", paths),
        &head_header(&shape4, DataType::I16, 4.0, 0.0),
        &shape4,
        &i16s,
    );
    let negated: Vec<i16> = series.iter().map(|&v| -v as i16).collect();
    write_shaped(
        &p("head-4d-negative", paths),
        &head_header(&shape4, DataType::I16, 4.0, -1.0),
        &shape4,
        &negated,
    );
    let plane = nx * ny * 8;
    let slice: Vec<f32> = t1[plane..plane + nx * ny]
        .iter()
        .map(|&v| v as f32)
        .collect();
    let shape2 = [nx, ny, 1];
    write_shaped(
        &p("slice", paths),
        &head_header(&shape2, DataType::F32, 2.0, 0.0),
        &shape2,
        &slice,
    );
}

fn write_inputs(dir: &Path) -> BTreeMap<&'static str, PathBuf> {
    let values = series_values();
    let paths = BTreeMap::from([
        ("float32", dir.join("float32.nii")),
        ("int16", dir.join("int16.nii")),
        ("uint8", dir.join("uint8.nii")),
        ("int16-negative", dir.join("int16-negative.nii")),
        ("float32-defects", dir.join("float32-defects.nii")),
        ("float32-tiny-offsets", dir.join("float32-tiny-offsets.nii")),
        ("float32-nearly-plumb", dir.join("float32-nearly-plumb.nii")),
        ("int16-intercept", dir.join("int16-intercept.nii")),
    ]);
    let f32s: Vec<f32> = values.iter().map(|&v| v as f32 / 4.0).collect();
    write_input(&paths["float32"], &f32s, DataType::F32, 0.0);
    let i16s: Vec<i16> = values.iter().map(|&v| v as i16).collect();
    write_input(&paths["int16"], &i16s, DataType::I16, 0.25);
    let u8s: Vec<u8> = values.iter().map(|&v| (v / 8 - 80) as u8).collect();
    write_input(&paths["uint8"], &u8s, DataType::U8, 0.0);
    write_input(&paths["int16-negative"], &i16s, DataType::I16, -0.25);
    write_with(&paths["float32-defects"], &defects_header(), &f32s);
    write_with(
        &paths["float32-tiny-offsets"],
        &tiny_offsets_header(),
        &f32s,
    );
    write_with(
        &paths["float32-nearly-plumb"],
        &nearly_plumb_header(),
        &f32s,
    );
    let mut intercept = input_header(DataType::I16, 0.25);
    intercept.scl_inter = 100.0;
    write_with(&paths["int16-intercept"], &intercept, &i16s);
    let mut paths = paths;
    write_head_inputs(dir, &mut paths);
    paths
}

// ------------------------------------------------------------------------------------------------
// Outputs

/// A recorded or computed fingerprint of one output.
#[derive(Clone, Debug, PartialEq)]
struct Fingerprint {
    sha256: String,
    /// The NIfTI data type of a data output (`-` for a header).
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
        16 => ("float32", 4),
        64 => ("float64", 8),
        other => panic!("unexpected NIfTI data type {other}"),
    }
}

/// `header` (the bytes before the data) and `data` fingerprints of a little-endian NIfTI-1 or
/// NIfTI-2 file.
fn nifti_fingerprints(path: &Path) -> BTreeMap<&'static str, Fingerprint> {
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
        let i = if N_SAMPLES > 1 {
            s * (n - 1) / (N_SAMPLES - 1)
        } else {
            0
        };
        sample.extend_from_slice(&data[i * size..(i + 1) * size]);
    }
    BTreeMap::from([
        (
            "header",
            Fingerprint {
                sha256: sha256(&raw[..offset]),
                dtype: "-".into(),
                sample: "-".into(),
            },
        ),
        (
            "data",
            Fingerprint {
                sha256: sha256(data),
                dtype: name.into(),
                sample: hex(&sample),
            },
        ),
    ])
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
        "uint8" => 1,
        "int16" => 2,
        "float32" => 4,
        "float64" => 8,
        _ => return String::new(),
    };
    let value = |b: &[u8]| -> f64 {
        match size {
            1 => f64::from(b[0]),
            2 => f64::from(i16::from_le_bytes([b[0], b[1]])),
            4 => f64::from(f32::from_le_bytes(b.try_into().unwrap())),
            _ => f64::from_le_bytes(b.try_into().unwrap()),
        }
    };
    let (mut differ, mut max_abs, mut max_ulp) = (0, 0.0f64, 0u64);
    for (x, y) in e.chunks(size).zip(a.chunks(size)) {
        if x != y {
            differ += 1;
            max_abs = max_abs.max((value(x) - value(y)).abs());
            if size == 4 {
                let ordered = |b: &[u8]| {
                    let bits = i64::from(i32::from_le_bytes(b.try_into().unwrap()));
                    if bits < 0 {
                        i64::from(i32::MIN) - bits
                    } else {
                        bits
                    }
                };
                max_ulp = max_ulp.max(ordered(x).abs_diff(ordered(y)));
            }
        }
    }
    let mut text = format!("{differ} of {} sampled values differ", e.len() / size);
    if differ > 0 {
        let _ = write!(text, ", max |difference| {max_abs}");
        if size == 4 {
            let _ = write!(text, ", max {max_ulp} ulp");
        }
    }
    text
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
        "# SPDX-License-Identifier: Apache-2.0\n\
         # Golden checksums of the AFNI replicas (tests/afni_replica_golden.rs): SHA-256 of the\n\
         # NIfTI header and data they write on Linux x86_64, with sampled data values.\n\
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
        .join("afni-replica")
}

// ------------------------------------------------------------------------------------------------
// Tests

#[test]
fn golden() {
    let dir = tempfile::tempdir().unwrap();
    let inputs = write_inputs(dir.path());
    let mut actual = Recorded::new();
    let mut files = BTreeMap::new();
    for case in CASES {
        let output = dir.path().join(format!("{}.nii", case.id));
        let input = match case.input {
            Input::Float32 => &inputs["float32"],
            Input::Int16 => &inputs["int16"],
            Input::Uint8 => &inputs["uint8"],
            Input::Int16Negative => &inputs["int16-negative"],
            Input::Float32Defects => &inputs["float32-defects"],
            Input::Float32TinyOffsets => &inputs["float32-tiny-offsets"],
            Input::Float32NearlyPlumb => &inputs["float32-nearly-plumb"],
            Input::Int16Intercept => &inputs["int16-intercept"],
            Input::Head => &inputs["head"],
            Input::HeadT2 => &inputs["head-t2"],
            Input::Head4d => &inputs["head-4d"],
            Input::Head4dNegative => &inputs["head-4d-negative"],
            Input::Slice => &inputs["slice"],
        };
        let (family, tool) = case.tool.split_once(' ').unwrap();
        let mut args: Vec<String> = vec![family.into(), tool.into()];
        args.extend(case.args.iter().map(|a| (*a).to_owned()));
        // The output is named relative to the temporary directory, so that a blank in its path
        // cannot make the -prefix one AFNI refuses (AFNI-12).
        args.extend(["-prefix".into(), format!("{}.nii", case.id)]);
        for (option, name) in case.outputs {
            args.extend([(*option).to_owned(), format!("{}.{name}.nii", case.id)]);
        }
        args.push(input.display().to_string());
        if case.existing {
            std::fs::write(&output, "taken").unwrap();
        }
        let mut command = Command::new(EXE);
        command.env("LARMORX_IMPLEMENTATION", "replica");
        command
            .args(&args)
            .current_dir(dir.path())
            .env("LARMORX_UPSTREAM_DEFECTS", case.mode);
        for (name, value) in case.env {
            command.env(name, value);
        }
        let out = command.output().unwrap();
        assert!(out.status.success(), "{}: {out:?}", case.id);
        // The notices (`*+ WARNING: upstream defect <ID> (<tool>) corrected: ...`).
        let stderr = String::from_utf8_lossy(&out.stderr);
        let found: Vec<&str> = stderr
            .lines()
            .filter_map(|l| l.split("upstream defect ").nth(1))
            .filter_map(|rest| rest.split(' ').next())
            .collect();
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
        for (name, fp) in nifti_fingerprints(&output) {
            actual.insert((case.id.to_owned(), name.to_owned()), fp);
        }
        for (_, extra) in case.outputs {
            let path = dir.path().join(format!("{}.{extra}.nii", case.id));
            for (name, fp) in nifti_fingerprints(&path) {
                actual.insert((case.id.to_owned(), format!("{extra}.{name}")), fp);
            }
        }
        files.insert(case.id, output);
    }

    if std::env::var_os("LARMORX_GOLDEN_RECORD").is_some() {
        record(&actual);
        return;
    }
    let (source, recorded) = read_golden();
    let mut problems = Vec::new();
    let mut bad_cases = std::collections::BTreeSet::new();
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
                if output == "data" {
                    let _ = write!(l, "; {}", describe(e, a));
                }
                l
            }
            (Some(_), None) => format!("{case} {output}: recorded but no longer produced"),
            (None, _) => format!("{case} {output}: not recorded (record on Linux x86_64)"),
        };
        if bad_cases.insert((case.clone(), output.clone())) {
            problems.push(line);
        }
    }
    if !problems.is_empty() {
        let root = mismatch_dir();
        std::fs::create_dir_all(&root).unwrap();
        for (case, _) in &bad_cases {
            if let Some(path) = files.get(case.as_str()) {
                std::fs::copy(path, root.join(format!("{case}.nii"))).unwrap();
            }
        }
        panic!(
            "the AFNI replicas differ from their Linux x86_64 output (recorded from {source}):\n  {}\n\
             The outputs are saved in {}.",
            problems.join("\n  "),
            root.display()
        );
    }
}

#[test]
fn every_tool_has_a_golden_case() {
    // The AFNI tools as the binary lists them in its help (the other families' cases are in
    // tests/golden).
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
        .filter(|t| t.starts_with("afni "))
        .collect();
    assert!(!tools.is_empty(), "{help}");
    let missing: Vec<&String> = tools
        .iter()
        .filter(|t| !CASES.iter().any(|c| c.tool == t.as_str()))
        .collect();
    assert!(
        missing.is_empty(),
        "no golden case for {missing:?}: add one to CASES in tests/afni_replica_golden.rs and record it on \
         Linux x86_64 (docs/validation/golden.md)"
    );
}
