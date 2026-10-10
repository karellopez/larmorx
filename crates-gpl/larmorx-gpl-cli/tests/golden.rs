// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Karel Lopez Vilaret.
//! Golden tests of the replicas: every platform reproduces the output `larmorx-gpl` gives on
//! Linux x86_64, bit for bit (CLAUDE.md rule 10, `docs/validation/golden.md`).
//!
//! The inputs are generated here from seeded integers (SplitMix64, the same generator as the
//! Python golden tests) and plain integer arithmetic. Each case runs the `larmorx-gpl` binary
//! on them and hashes (SHA-256) the header and the data of the NIfTI file it writes; the
//! hashes are compared with `tests/golden.tsv`. One case per distinct code path: each
//! interpolation method, each stored type, the restore and detrend options.
//!
//! Recording, on Linux x86_64 only, from a committed tree whose replica parity passes:
//! `LARMORX_GOLDEN_RECORD=1 cargo test --locked -p larmorx-gpl-cli --test golden`
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

const EXE: &str = env!("CARGO_BIN_EXE_larmorx-gpl");
const GOLDEN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden.tsv");
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
}

struct Case {
    id: &'static str,
    /// `<family> <tool>`, as `larmorx-gpl --help` lists it.
    tool: &'static str,
    input: Input,
    args: &'static [&'static str],
}

const CASES: &[Case] = &[
    Case {
        id: "3dTshift.fourier.int16",
        tool: "afni 3dTshift",
        input: Input::Int16,
        args: &["-tpattern", "alt+z2"],
    },
    Case {
        id: "3dTshift.fourier.float32.ignore",
        tool: "afni 3dTshift",
        input: Input::Float32,
        args: &["-tpattern", "alt+z", "-ignore", "2"],
    },
    Case {
        id: "3dTshift.linear.float32",
        tool: "afni 3dTshift",
        input: Input::Float32,
        args: &["-tpattern", "seq+z", "-linear"],
    },
    Case {
        id: "3dTshift.cubic.int16.rlt+",
        tool: "afni 3dTshift",
        input: Input::Int16,
        args: &["-tpattern", "alt-z", "-cubic", "-rlt+"],
    },
    Case {
        id: "3dTshift.quintic.float32.tzero",
        tool: "afni 3dTshift",
        input: Input::Float32,
        args: &["-tpattern", "alt+z2", "-quintic", "-tzero", "0"],
    },
    Case {
        id: "3dTshift.heptic.uint8.no_detrend",
        tool: "afni 3dTshift",
        input: Input::Uint8,
        args: &["-tpattern", "seq-z", "-heptic", "-no_detrend"],
    },
    Case {
        id: "3dTshift.wsinc5.float32.rlt",
        tool: "afni 3dTshift",
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
        input: Input::Int16,
        args: &["-tpattern", "alt+z", "-wsinc9", "-slice", "3"],
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

fn write_input<T: Element>(path: &Path, data: &[T], data_type: DataType, slope: f64) {
    let mut h = NiftiHeader::new(NiftiVersion::V1, &SHAPE, data_type).unwrap();
    let affine = Affine::from_zooms([2.0, 2.0, 3.0], [-7.0, -5.0, -6.0]);
    h.set_sform(&affine, 1);
    h.set_qform(&affine, 1).unwrap();
    h.pixdim[4] = 2.0;
    h.xyzt_units = 2 | 8;
    h.scl_slope = slope;
    h.scl_inter = 0.0;
    let view = ArrayViewD::from_shape(IxDyn(&SHAPE).f(), data).unwrap();
    nifti::write(path, &h, view, &WriteOptions::default()).unwrap();
}

fn write_inputs(dir: &Path) -> BTreeMap<&'static str, PathBuf> {
    let values = series_values();
    let paths = BTreeMap::from([
        ("float32", dir.join("float32.nii")),
        ("int16", dir.join("int16.nii")),
        ("uint8", dir.join("uint8.nii")),
    ]);
    let f32s: Vec<f32> = values.iter().map(|&v| v as f32 / 4.0).collect();
    write_input(&paths["float32"], &f32s, DataType::F32, 0.0);
    let i16s: Vec<i16> = values.iter().map(|&v| v as i16).collect();
    write_input(&paths["int16"], &i16s, DataType::I16, 0.25);
    let u8s: Vec<u8> = values.iter().map(|&v| (v / 8 - 80) as u8).collect();
    write_input(&paths["uint8"], &u8s, DataType::U8, 0.0);
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

/// `header` (the bytes before the data) and `data` fingerprints of a NIfTI-1 file.
fn nifti_fingerprints(path: &Path) -> BTreeMap<&'static str, Fingerprint> {
    let raw = std::fs::read(path).unwrap();
    assert_eq!(
        &raw[0..4],
        &348i32.to_le_bytes(),
        "{}: not little-endian NIfTI-1",
        path.display()
    );
    let datatype = i16::from_le_bytes([raw[70], raw[71]]);
    let offset = f32::from_le_bytes(raw[108..112].try_into().unwrap()) as usize;
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
        "# SPDX-License-Identifier: GPL-3.0-or-later\n\
         # Golden checksums of the larmorx-gpl replicas (tests/golden.rs): SHA-256 of the NIfTI\n\
         # header and data that larmorx-gpl writes on Linux x86_64, with sampled data values.\n\
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
        .join("larmorx-gpl")
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
        };
        let (family, tool) = case.tool.split_once(' ').unwrap();
        let mut args: Vec<String> = vec![family.into(), tool.into()];
        args.extend(case.args.iter().map(|a| (*a).to_owned()));
        args.extend(["-prefix".into(), output.display().to_string()]);
        args.push(input.display().to_string());
        let out = Command::new(EXE).args(&args).output().unwrap();
        assert!(out.status.success(), "{}: {out:?}", case.id);
        for (name, fp) in nifti_fingerprints(&output) {
            actual.insert((case.id.to_owned(), name.to_owned()), fp);
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
            "larmorx-gpl differs from its Linux x86_64 output (recorded from {source}):\n  {}\n\
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
