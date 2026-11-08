// SPDX-License-Identifier: GPL-3.0-or-later
// Part of larmorx-gpl (GNU GPL; see crates-gpl/LICENSE). Copyright 2026 Karel Lopez Vilaret.
//! Golden tests of the GPL family's replicas (tedana's t2smap): every platform reproduces the
//! output `larmorx-gpl` gives on Linux x86_64, bit for bit (CLAUDE.md rule 10,
//! `docs/validation/golden.md`).
//!
//! The inputs are generated here from seeded integers (SplitMix64, the same generator as the
//! other golden tests) and plain integer arithmetic: a multi-echo series of 16 x 14 x 8 voxels
//! and 4 volumes whose echoes decay by integer ratios (a few voxels rise with the echo time, a
//! few lose their last echo), written as NIfTI-1 files by the test itself. Each case runs
//! `larmorx-gpl tedana t2smap` with relative file names in its own directory and hashes (SHA-256)
//! every file it writes: the images' header and data (decompressed), the table, the JSON files
//! (`dataset_description.json` without its `Node` block, the host's name and system), the call
//! script; and the exit status and the upstream defects named on standard error. One case per
//! distinct code path: each fit type and mode, each combination, both naming conventions, the
//! curve fit's variance maps, no mask (nilearn's mask), both adaptive-mask methods, dummy scans
//! and excluded volumes, float32 and int16 inputs, a z-concatenated file, echo times in
//! milliseconds, both library environments, D13's default and `reproduce`, and a failure.
//!
//! Recording, on Linux x86_64 only, from a committed tree whose replica parity passes:
//! `LARMORX_GOLDEN_RECORD=1 cargo test --locked -p larmorx-gpl-cli --test golden`
//! (`LARMORX_GOLDEN_ALLOW_DIRTY=1` to record from uncommitted changes). On a mismatch the
//! outputs are saved in `$LARMORX_GOLDEN_MISMATCHES` (CI uploads it), else in
//! `target/tmp/golden-mismatches`.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use sha2::{Digest, Sha256};

const EXE: &str = env!("CARGO_BIN_EXE_larmorx-gpl");
const GOLDEN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden.tsv");
/// Values kept per image, at fixed positions, to tell how large a mismatch is.
const N_SAMPLES: usize = 8;

/// The series: 16 x 14 x 8 voxels of 3 x 3 x 3.5 mm, 4 volumes, TR 2 s.
const SHAPE: [usize; 3] = [16, 14, 8];
const VOLUMES: usize = 4;
const ECHOES: usize = 3;
const PIXDIM: [f32; 3] = [3.0, 3.0, 3.5];

struct Case {
    id: &'static str,
    /// `<family> <tool>`, as `larmorx-gpl --help` lists it.
    tool: &'static str,
    /// t2smap's arguments, separated by blanks; `{name}` is the generated input `name` (`e1`,
    /// `e2`, `e3`: int16 echoes; `f1`, `f2`, `f3`: float32 echoes; `zcat`: the three int16
    /// echoes along z; `mask`: uint8), relative to the case's directory.
    args: &'static str,
    /// `LARMORX_UPSTREAM_DEFECTS`: `fix` (the default) or `reproduce`.
    mode: &'static str,
    /// `LARMORX_TEDANA_LIBRARIES`: `newest` (the default) or `shared`.
    libraries: &'static str,
    /// The exit status expected.
    status: i32,
}

const CASES: &[Case] = &[
    // tedana's defaults: the log-linear fit over all volumes, the dropout mask, t2s weights
    Case {
        id: "loglin",
        tool: "tedana t2smap",
        args: "-d {e1} {e2} {e3} -e 0.012 0.028 0.044 --mask {mask} --out-dir out",
        mode: "fix",
        libraries: "newest",
        status: 0,
    },
    Case {
        id: "loglin.reproduce",
        tool: "tedana t2smap",
        args: "-d {e1} {e2} {e3} -e 0.012 0.028 0.044 --mask {mask} --out-dir out",
        mode: "reproduce",
        libraries: "newest",
        status: 0,
    },
    // fMRIPrep's fit (the curve fit) with its variance maps, tedana's original names, a prefix
    Case {
        id: "curvefit.verbose.orig",
        tool: "tedana t2smap",
        args: "-d {e1} {e2} {e3} -e 0.012 0.028 0.044 --mask {mask} --fittype curvefit --verbose --convention orig --prefix sub-01 --n-threads 2 --out-dir out",
        mode: "fix",
        libraries: "newest",
        status: 0,
    },
    Case {
        id: "curvefit.verbose.orig.reproduce",
        tool: "tedana t2smap",
        args: "-d {e1} {e2} {e3} -e 0.012 0.028 0.044 --mask {mask} --fittype curvefit --verbose --convention orig --prefix sub-01 --n-threads 2 --out-dir out",
        mode: "reproduce",
        libraries: "newest",
        status: 0,
    },
    // the shared environment's SciPy (1.15.3) and numpy (2.3.5): another curve fit
    Case {
        id: "curvefit.shared.reproduce",
        tool: "tedana t2smap",
        args: "-d {e1} {e2} {e3} -e 0.012 0.028 0.044 --mask {mask} --fittype curvefit --out-dir out",
        mode: "reproduce",
        libraries: "shared",
        status: 0,
    },
    // one fit per volume (4D maps) and PAID's weights
    Case {
        id: "curvefit.ts.paid.reproduce",
        tool: "tedana t2smap",
        args: "-d {e1} {e2} {e3} -e 0.012 0.028 0.044 --mask {mask} --fittype curvefit --fitmode ts --combmode paid --out-dir out",
        mode: "reproduce",
        libraries: "newest",
        status: 0,
    },
    // no mask (nilearn's compute_epi_mask of the first echo), both adaptive-mask methods,
    // a dummy scan and an excluded volume, float32 echoes, echo times in milliseconds
    Case {
        id: "nomask.float32.ms",
        tool: "tedana t2smap",
        args: "-d {f1} {f2} {f3} -e 12 28 44 --masktype dropout decay --dummy-scans 1 --exclude 0 --debug --out-dir out",
        mode: "fix",
        libraries: "newest",
        status: 0,
    },
    Case {
        id: "nomask.float32.ms.reproduce",
        tool: "tedana t2smap",
        args: "-d {f1} {f2} {f3} -e 12 28 44 --masktype dropout decay --dummy-scans 1 --exclude 0 --debug --out-dir out",
        mode: "reproduce",
        libraries: "newest",
        status: 0,
    },
    // a z-concatenated file (tedana's deprecated single input), the adaptive mask off
    Case {
        id: "zcat.masktype-none",
        tool: "tedana t2smap",
        args: "-d {zcat} -e 0.012 0.028 0.044 --mask {mask} --masktype none --out-dir out",
        mode: "fix",
        libraries: "newest",
        status: 0,
    },
    // TED-02: an excluded index equal to the number of volumes, refused with a clear error
    Case {
        id: "exclude.ted02",
        tool: "tedana t2smap",
        args: "-d {e1} {e2} {e3} -e 0.012 0.028 0.044 --mask {mask} --exclude 4 --out-dir out",
        mode: "fix",
        libraries: "newest",
        status: 1,
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

/// Whether voxel `(i, j, k)` is inside the ellipsoid of half-axes 7, 6 and 3.5 voxels
/// (`shrink` voxels less), integer arithmetic only (doubled coordinates).
fn inside(i: usize, j: usize, k: usize, shrink: i64) -> bool {
    let (dx, dy, dz) = (2 * i as i64 - 15, 2 * j as i64 - 13, 2 * k as i64 - 7);
    let (a, b, c) = (14 - 2 * shrink, 12 - 2 * shrink, 7 - 2 * shrink);
    let (a2, b2, c2) = (a * a, b * b, c * c);
    dx * dx * b2 * c2 + dy * dy * a2 * c2 + dz * dz * a2 * b2 <= a2 * b2 * c2
}

/// The echoes' values, `[echo][volume][voxel]` with x fastest: inside the head a signal of
/// 800-1199 decaying by a ratio of 55-85 % per echo (every 17th voxel rising by 5 %, every
/// 23rd losing its last echo), outside 20-49; noise of +-10 per volume.
fn echo_values() -> Vec<Vec<Vec<i64>>> {
    let [nx, ny, nz] = SHAPE;
    let nvox = nx * ny * nz;
    let base = splitmix64(11, nvox);
    let ratio = splitmix64(12, nvox);
    let noise = splitmix64(13, nvox * VOLUMES * ECHOES);
    let mut out = vec![vec![vec![0i64; nvox]; VOLUMES]; ECHOES];
    for k in 0..nz {
        for j in 0..ny {
            for i in 0..nx {
                let v = i + nx * (j + ny * k);
                let head = inside(i, j, k, 0);
                let s0 = if head {
                    800 + (base[v] % 400) as i64
                } else {
                    20 + (base[v] % 30) as i64
                };
                let r = if head && (i + j + k) % 17 == 0 {
                    105
                } else {
                    55 + (ratio[v] % 31) as i64
                };
                for (e, echo) in out.iter_mut().enumerate() {
                    let mut s = s0;
                    for _ in 0..e {
                        s = s * r / 100;
                    }
                    for (t, vol) in echo.iter_mut().enumerate() {
                        let n = noise[(e * VOLUMES + t) * nvox + v] % 21;
                        let mut x = s + n as i64 - 10;
                        if head && e == ECHOES - 1 && v % 23 == 0 {
                            x = 0;
                        }
                        vol[v] = x.max(0);
                    }
                }
            }
        }
    }
    out
}

/// A little-endian NIfTI-1 file (`.nii`): sform code 1 (scanner), no qform, millimetres and
/// seconds, the TR 2 s.
fn nifti_file(path: &Path, dims: &[usize], datatype: i16, bitpix: i16, data: &[u8]) {
    let mut h = vec![0u8; 352];
    let put_i32 = |h: &mut Vec<u8>, o: usize, v: i32| h[o..o + 4].copy_from_slice(&v.to_le_bytes());
    let put_i16 = |h: &mut Vec<u8>, o: usize, v: i16| h[o..o + 2].copy_from_slice(&v.to_le_bytes());
    let put_f32 = |h: &mut Vec<u8>, o: usize, v: f32| h[o..o + 4].copy_from_slice(&v.to_le_bytes());
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
    for (k, d) in PIXDIM.iter().enumerate() {
        put_f32(&mut h, 80 + 4 * k, *d);
    }
    put_f32(&mut h, 92, 2.0);
    put_f32(&mut h, 108, 352.0);
    put_f32(&mut h, 112, 1.0);
    h[123] = 2 | 8;
    put_i16(&mut h, 254, 1);
    let offsets = [24.0, -21.0, -14.0];
    let srow = [
        [-PIXDIM[0], 0.0, 0.0, offsets[0]],
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

fn int16_bytes(values: impl Iterator<Item = i64>) -> Vec<u8> {
    values
        .flat_map(|x| i16::try_from(x).unwrap().to_le_bytes())
        .collect()
}

/// The generated inputs in `dir/inputs`, by name.
fn write_inputs(dir: &Path) -> BTreeMap<&'static str, PathBuf> {
    let dir = dir.join("inputs");
    std::fs::create_dir(&dir).unwrap();
    let [nx, ny, nz] = SHAPE;
    let nvox = nx * ny * nz;
    let echoes = echo_values();
    let frac = splitmix64(14, nvox * VOLUMES * ECHOES);
    let mut paths = BTreeMap::new();
    let dims = [nx, ny, nz, VOLUMES];
    for (e, (int_name, float_name)) in [("e1", "f1"), ("e2", "f2"), ("e3", "f3")]
        .into_iter()
        .enumerate()
    {
        let path = dir.join(format!("{int_name}.nii"));
        nifti_file(
            &path,
            &dims,
            4,
            16,
            &int16_bytes(echoes[e].iter().flatten().copied()),
        );
        paths.insert(int_name, path);
        // float32: the same values plus quarters (exact in float32)
        let floats: Vec<u8> = echoes[e]
            .iter()
            .flatten()
            .enumerate()
            .flat_map(|(n, &x)| {
                let q = (frac[e * nvox * VOLUMES + n] % 4) as f32 * 0.25;
                (x as f32 + q).to_le_bytes()
            })
            .collect();
        let path = dir.join(format!("{float_name}.nii"));
        nifti_file(&path, &dims, 16, 32, &floats);
        paths.insert(float_name, path);
    }
    // the echoes stacked along z (tedana's z-concatenated file)
    let mut zcat = Vec::new();
    for t in 0..VOLUMES {
        for echo in &echoes {
            zcat.extend(int16_bytes(echo[t].iter().copied()));
        }
    }
    // reorder to x fastest, then y, then z (echoes' slabs), then t
    let slab = nvox;
    let mut ordered = vec![0u8; zcat.len()];
    for t in 0..VOLUMES {
        for e in 0..ECHOES {
            let from = (t * ECHOES + e) * slab * 2;
            let to = (t * ECHOES * slab + e * slab) * 2;
            ordered[to..to + slab * 2].copy_from_slice(&zcat[from..from + slab * 2]);
        }
    }
    let path = dir.join("zcat.nii");
    nifti_file(&path, &[nx, ny, nz * ECHOES, VOLUMES], 4, 16, &ordered);
    paths.insert("zcat", path);
    // the mask: the head shrunk by one voxel, uint8
    let mut mask = Vec::with_capacity(nvox);
    for k in 0..nz {
        for j in 0..ny {
            for i in 0..nx {
                mask.push(u8::from(inside(i, j, k, 1)));
            }
        }
    }
    let path = dir.join("mask.nii");
    nifti_file(&path, &SHAPE, 2, 8, &mask);
    paths.insert("mask", path);
    paths
}

// ------------------------------------------------------------------------------------------------
// Fingerprints

struct Fingerprint {
    sha256: String,
    /// The NIfTI data type of an image's data (`-` for a header or another file).
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

fn text_fingerprint(text: &str) -> Fingerprint {
    Fingerprint {
        sha256: sha256(text.as_bytes()),
        dtype: "-".into(),
        sample: "-".into(),
    }
}

fn dtype_name(code: i16) -> (&'static str, usize) {
    match code {
        2 => ("uint8", 1),
        4 => ("int16", 2),
        8 => ("int32", 4),
        16 => ("float32", 4),
        64 => ("float64", 8),
        other => panic!("unexpected NIfTI data type {other}"),
    }
}

/// `header` (the bytes before the data) and `data` fingerprints of a little-endian NIfTI-1
/// file, gzip-compressed or not.
fn nifti_fingerprints(path: &Path) -> Vec<(String, Fingerprint)> {
    let mut raw = std::fs::read(path).unwrap();
    if raw.starts_with(&[0x1f, 0x8b]) {
        let mut out = Vec::new();
        flate2::read::MultiGzDecoder::new(&raw[..])
            .read_to_end(&mut out)
            .unwrap();
        raw = out;
    }
    assert_eq!(
        i32::from_le_bytes(raw[0..4].try_into().unwrap()),
        348,
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

/// `dataset_description.json` without its `Node` block (the host's name, system and release,
/// which tedana records from the machine it runs on).
fn without_node(text: &str) -> String {
    let mut out = String::new();
    let mut depth = None;
    for line in text.lines() {
        if let Some(d) = depth {
            if line.trim_start().starts_with('}') && line.len() - line.trim_start().len() == d {
                depth = None;
            }
            continue;
        }
        if line.trim_start().starts_with("\"Node\": {") {
            depth = Some(line.len() - line.trim_start().len());
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// Every file a run wrote below `dir`, by its path there: images by header and data, the
/// others by their bytes.
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
        if name.ends_with(".nii") || name.ends_with(".nii.gz") {
            for (part, fp) in nifti_fingerprints(&p) {
                out.push((format!("{name}.{part}"), fp));
            }
        } else if name.ends_with("dataset_description.json") {
            let text = std::fs::read_to_string(&p).unwrap();
            out.push((name, text_fingerprint(&without_node(&text))));
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
    // Text outputs, the status and the named defects have no sampled values.
    let size = match expected.dtype.as_str() {
        "uint8" => 1,
        "int16" => 2,
        "float32" | "int32" => 4,
        "float64" => 8,
        _ => return String::new(),
    };
    let (e, a) = (unhex(&expected.sample), unhex(&actual.sample));
    let differ = e
        .chunks(size)
        .zip(a.chunks(size))
        .filter(|(x, y)| x != y)
        .count();
    format!("{differ} of {} sampled values differ", e.len() / size)
}

/// The upstream defects a run named on standard error (`larmorx-gpl: upstream defect <ID>
/// (<tool>) corrected: ...`), as `<ID> <verb>` lines, sorted.
fn named(stderr: &[u8]) -> String {
    let stderr = String::from_utf8_lossy(stderr);
    let mut found: Vec<String> = stderr
        .lines()
        .filter_map(|l| l.strip_prefix("larmorx-gpl: upstream defect "))
        .map(|rest| {
            let id = rest.split(' ').next().unwrap_or("");
            let verb = if rest.contains(") reproduced: ") {
                "reproduced"
            } else {
                "corrected"
            };
            format!("{id} {verb}")
        })
        .collect();
    found.sort_unstable();
    found.dedup();
    found.iter().map(|l| format!("{l}\n")).collect()
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
         # Golden checksums of the larmorx-gpl replicas (tests/golden.rs): SHA-256 of the files\n\
         # larmorx-gpl writes on Linux x86_64 (images by header and data, with sampled values),\n\
         # its exit status and the upstream defects it names. Every platform must reproduce them\n\
         # (docs/validation/golden.md). Do not edit by hand.\n",
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

/// `arg` with `{name}` replaced by the generated input's path relative to a case's directory
/// (`../inputs/<file>`, with `/` on every platform), so no temporary path enters an output.
fn fill(arg: &str, inputs: &BTreeMap<&'static str, PathBuf>) -> String {
    match arg.strip_prefix('{').and_then(|a| a.strip_suffix('}')) {
        Some(name) => format!(
            "../inputs/{}",
            inputs[name].file_name().unwrap().to_string_lossy()
        ),
        None => arg.to_owned(),
    }
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
        args.extend(case.args.split_whitespace().map(|a| fill(a, &inputs)));
        let out = Command::new(EXE)
            .args(&args)
            .current_dir(&run)
            .env_remove("COLUMNS")
            .env("LARMORX_UPSTREAM_DEFECTS", case.mode)
            .env("LARMORX_TEDANA_LIBRARIES", case.libraries)
            .output()
            .unwrap();
        assert_eq!(
            out.status.code(),
            Some(case.status),
            "{}: {}",
            case.id,
            String::from_utf8_lossy(&out.stderr)
        );
        let fps = case_fingerprints(&run);
        assert!(!fps.is_empty(), "{}: no outputs", case.id);
        for (name, fp) in fps {
            actual.insert((case.id.to_owned(), name), fp);
        }
        let status = format!("{}\n", case.status);
        actual.insert(
            (case.id.to_owned(), "status".to_owned()),
            Fingerprint {
                sha256: sha256(status.as_bytes()),
                dtype: "-".into(),
                sample: case.status.to_string(),
            },
        );
        actual.insert(
            (case.id.to_owned(), "named defects".to_owned()),
            text_fingerprint(&named(&out.stderr)),
        );
        std::fs::write(run.join("stderr.txt"), &out.stderr).unwrap();
        runs.insert(case.id, run);
    }

    if std::env::var_os("LARMORX_GOLDEN_RECORD").is_some() {
        record(&actual);
        return;
    }
    let (source, expected) = read_golden();
    let mut problems = Vec::new();
    for (key, fp) in &actual {
        match expected.get(key) {
            None => problems.push(format!("{} {}: not recorded", key.0, key.1)),
            Some(e) if e.sha256 != fp.sha256 => {
                problems.push(format!("{} {}: {}", key.0, key.1, describe(e, fp)));
            }
            Some(_) => {}
        }
    }
    for key in expected.keys() {
        if !actual.contains_key(key) {
            problems.push(format!("{} {}: recorded, not written", key.0, key.1));
        }
    }
    if !problems.is_empty() {
        let save = mismatch_dir();
        for case in problems
            .iter()
            .filter_map(|p| p.split(' ').next())
            .collect::<std::collections::BTreeSet<_>>()
        {
            if let Some(run) = runs.get(case) {
                let to = save.join(case);
                let _ = std::fs::create_dir_all(&to);
                let mut stack = vec![run.clone()];
                while let Some(d) = stack.pop() {
                    for e in std::fs::read_dir(&d).unwrap() {
                        let p = e.unwrap().path();
                        let rel = p.strip_prefix(run).unwrap();
                        if p.is_dir() {
                            let _ = std::fs::create_dir_all(to.join(rel));
                            stack.push(p);
                        } else {
                            let _ = std::fs::copy(&p, to.join(rel));
                        }
                    }
                }
            }
        }
        panic!(
            "{} golden mismatches (recorded from {source}); outputs saved in {}:\n{}",
            problems.len(),
            save.display(),
            problems.join("\n")
        );
    }
}

#[test]
fn every_case_has_its_own_id() {
    for (i, a) in CASES.iter().enumerate() {
        assert!(CASES[..i].iter().all(|b| b.id != a.id), "{}", a.id);
    }
}
