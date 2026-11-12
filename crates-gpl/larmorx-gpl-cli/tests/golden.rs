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
//! (`dataset_description.json` without its `Node` block, the host's name and system, and with
//! larmorx-gpl's version written `<version>`: a release changes no checksum), the call script;
//! and the exit status and the upstream defects named on standard error. One case per
//! distinct code path: each fit type and mode, each combination, both naming conventions, the
//! curve fit's variance maps, no mask (nilearn's mask), both adaptive-mask methods, dummy scans
//! and excluded volumes, float32 and int16 inputs, a z-concatenated file, echo times in
//! milliseconds, both library environments, D13's default and `reproduce`, and a failure; and
//! since relay RS2 of the clean-room part (2026-11-12) TED-46's saturated voxels (the
//! log-linear fit, the curve fit in both modes), TED-31's refusal, an ANALYZE 7.5 pair (SPM2's
//! big-endian scaled int16, its origin) and an MGH image (its affine; nilearn's mask).
//!
//! Recording, on Linux x86_64 only, from a committed tree whose replica parity passes:
//! `LARMORX_GOLDEN_RECORD=1 cargo test --locked -p larmorx-gpl-cli --test golden`
//! (`LARMORX_GOLDEN_ALLOW_DIRTY=1` to record from uncommitted changes). On a mismatch the
//! outputs are saved in `$LARMORX_GOLDEN_MISMATCHES` (CI uploads it), else in
//! `target/tmp/golden-mismatches`.
//!
//! The installed program: `LARMORX_GOLDEN_PROGRAM=<path or name> cargo test --locked -p
//! larmorx-gpl-cli --test golden` runs that `larmorx-gpl` instead of cargo's build (e.g. the
//! program a wheel installs, `<venv>/bin/larmorx-gpl`; `Scripts\larmorx-gpl.exe` on Windows),
//! after checking that its `--version` is this checkout's.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use sha2::{Digest, Sha256};

const EXE: &str = env!("CARGO_BIN_EXE_larmorx-gpl");
const GOLDEN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden.tsv");
/// Values kept per image, at fixed positions, to tell how large a mismatch is.
const N_SAMPLES: usize = 8;

/// This crate's version in PEP 440's form, as `larmorx-gpl --version` prints it (build.rs).
const VERSION: &str = env!("LARMORX_GPL_PEP440_VERSION");

/// The program the cases run: cargo's build of `larmorx-gpl` (the test's own), or the one
/// `LARMORX_GOLDEN_PROGRAM` names, e.g. the program the larmorx-gpl wheel installs: a path (a
/// relative one from this crate's directory, where cargo runs the tests) or a name looked up on
/// `PATH` (`.exe` added on Windows). Its `--version` must be this crate's.
fn program() -> &'static Path {
    static PROGRAM: OnceLock<PathBuf> = OnceLock::new();
    PROGRAM.get_or_init(|| match std::env::var_os("LARMORX_GOLDEN_PROGRAM") {
        Some(given) if !given.is_empty() => installed(Path::new(&given)),
        _ => PathBuf::from(EXE),
    })
}

/// `LARMORX_GOLDEN_PROGRAM`'s program as an absolute path, once its `--version` is checked.
fn installed(given: &Path) -> PathBuf {
    let with_exe = |p: PathBuf| {
        let exe = p.with_extension(std::env::consts::EXE_EXTENSION);
        [p, exe]
    };
    let bare = given.parent().is_some_and(|p| p.as_os_str().is_empty());
    let found = if bare {
        std::env::var_os("PATH").and_then(|path| {
            std::env::split_paths(&path)
                .flat_map(|dir| with_exe(dir.join(given)))
                .find(|p| p.is_file())
        })
    } else {
        with_exe(given.to_path_buf())
            .into_iter()
            .find(|p| p.is_file())
    };
    let found = found.unwrap_or_else(|| {
        panic!(
            "LARMORX_GOLDEN_PROGRAM={}: no such program",
            given.display()
        )
    });
    let path = std::path::absolute(&found).unwrap_or_else(|e| panic!("{}: {e}", found.display()));
    let out = Command::new(&path)
        .arg("--version")
        .output()
        .unwrap_or_else(|e| panic!("LARMORX_GOLDEN_PROGRAM={}: {e}", path.display()));
    let reported = String::from_utf8_lossy(&out.stdout).trim_end().to_owned();
    assert!(
        out.status.success() && reported == format!("larmorx-gpl {VERSION}"),
        "LARMORX_GOLDEN_PROGRAM={}: --version printed {reported:?} (exit status {:?}), not \
         this checkout's larmorx-gpl {VERSION}",
        path.display(),
        out.status.code()
    );
    eprintln!(
        "golden: running {} ({reported}), not cargo's build",
        path.display()
    );
    path
}

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
    /// echoes along z; `mask`: uint8; `s1` to `s3`: uint8 echoes with a block saturated at 255;
    /// `d1`: the first echo negated; `a1` to `a3`: the int16 echoes as big-endian ANALYZE 7.5
    /// pairs scaled by SPM2's factor and intercept; `g1` to `g3`: the int16 echoes as MGH
    /// images), relative to the case's directory.
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
    // TED-46 (relay RS2): voxels saturated at 255 in every echo and volume take the exact
    // slope 0 in the log-linear fit, and the curve fit's exact solution (S0 255, no decay)
    Case {
        id: "ted46.loglin",
        tool: "tedana t2smap",
        args: "-d {s1} {s2} {s3} -e 0.012 0.028 0.044 --mask {mask} --out-dir out",
        mode: "fix",
        libraries: "newest",
        status: 0,
    },
    Case {
        id: "ted46.curvefit",
        tool: "tedana t2smap",
        args: "-d {s1} {s2} {s3} -e 0.012 0.028 0.044 --mask {mask} --fittype curvefit --verbose --out-dir out",
        mode: "fix",
        libraries: "newest",
        status: 0,
    },
    // tedana's rounding noise there (numpy's batched least squares, exactly)
    Case {
        id: "ted46.curvefit.reproduce",
        tool: "tedana t2smap",
        args: "-d {s1} {s2} {s3} -e 0.012 0.028 0.044 --mask {mask} --fittype curvefit --verbose --out-dir out",
        mode: "reproduce",
        libraries: "newest",
        status: 0,
    },
    // TED-31: the adaptive mask leaves no voxel for the fit (a negative first echo)
    Case {
        id: "ted31",
        tool: "tedana t2smap",
        args: "-d {d1} {e2} {e3} -e 0.012 0.028 0.044 --mask {mask} --masktype none --out-dir out",
        mode: "fix",
        libraries: "newest",
        status: 1,
    },
    // ANALYZE 7.5 (SPM2's big-endian int16 with a scale factor, an intercept and an origin)
    Case {
        id: "analyze.scaled",
        tool: "tedana t2smap",
        args: "-d {a1} {a2} {a3} -e 0.012 0.028 0.044 --mask {mask} --out-dir out",
        mode: "fix",
        libraries: "newest",
        status: 0,
    },
    // MGH (an affine of direction cosines and a centre; nilearn's mask of the first echo)
    Case {
        id: "mgh.nomask",
        tool: "tedana t2smap",
        args: "-d {g1} {g2} {g3} -e 12 28 44 --out-dir out",
        mode: "fix",
        libraries: "newest",
        status: 0,
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

/// An ANALYZE 7.5 pair of big-endian int16 data (`.hdr` 348 bytes, `.img`) with SPM2's scale
/// factor (`funused1`) and intercept (`funused2`) and SPM's origin (1-based voxel (9, 8, 5), so
/// that its affine is the mask's).
fn analyze_pair(hdr: &Path, dims: &[usize], values: impl Iterator<Item = i64>) {
    let mut h = vec![0u8; 348];
    let put_i32 = |h: &mut Vec<u8>, o: usize, v: i32| h[o..o + 4].copy_from_slice(&v.to_be_bytes());
    let put_i16 = |h: &mut Vec<u8>, o: usize, v: i16| h[o..o + 2].copy_from_slice(&v.to_be_bytes());
    let put_f32 = |h: &mut Vec<u8>, o: usize, v: f32| h[o..o + 4].copy_from_slice(&v.to_be_bytes());
    put_i32(&mut h, 0, 348);
    h[38] = b'r';
    put_i16(&mut h, 40, dims.len() as i16);
    for (k, d) in dims.iter().enumerate() {
        put_i16(&mut h, 42 + 2 * k, *d as i16);
    }
    for k in dims.len()..7 {
        put_i16(&mut h, 42 + 2 * k, 1);
    }
    put_i16(&mut h, 70, 4);
    put_i16(&mut h, 72, 16);
    for (k, d) in PIXDIM.iter().enumerate() {
        put_f32(&mut h, 80 + 4 * k, *d);
    }
    put_f32(&mut h, 92, 2.0);
    put_f32(&mut h, 112, 2.0);
    put_f32(&mut h, 116, 0.5);
    for (k, o) in [9i16, 8, 5].iter().enumerate() {
        put_i16(&mut h, 253 + 2 * k, *o);
    }
    std::fs::write(hdr, h).unwrap();
    let data: Vec<u8> = values
        .flat_map(|x| i16::try_from(x).unwrap().to_be_bytes())
        .collect();
    std::fs::write(hdr.with_extension("img"), data).unwrap();
}

/// An MGH image (`.mgh`) of int16 data: big-endian, voxel sizes 3 x 3 x 3.5 mm, direction
/// cosines of an LIA orientation, a centre off the grid's, the TR (2000 ms) in its footer.
fn mgh_file(path: &Path, dims: &[usize], values: impl Iterator<Item = i64>) {
    let mut h = vec![0u8; 284];
    let put_i32 = |h: &mut Vec<u8>, o: usize, v: i32| h[o..o + 4].copy_from_slice(&v.to_be_bytes());
    let put_f32 = |h: &mut Vec<u8>, o: usize, v: f32| h[o..o + 4].copy_from_slice(&v.to_be_bytes());
    put_i32(&mut h, 0, 1);
    for (k, d) in dims.iter().enumerate() {
        put_i32(&mut h, 4 + 4 * k, *d as i32);
    }
    put_i32(&mut h, 20, 4);
    h[28..30].copy_from_slice(&1i16.to_be_bytes());
    for (k, d) in PIXDIM.iter().enumerate() {
        put_f32(&mut h, 30 + 4 * k, *d);
    }
    let mdc = [-1.0f32, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 1.0, 0.0];
    for (k, v) in mdc.iter().enumerate() {
        put_f32(&mut h, 42 + 4 * k, *v);
    }
    for (k, c) in [1.25f32, -2.5, 3.375].iter().enumerate() {
        put_f32(&mut h, 78 + 4 * k, *c);
    }
    let mut bytes = h;
    bytes.extend(values.flat_map(|x| i16::try_from(x).unwrap().to_be_bytes()));
    bytes.extend_from_slice(&2000.0f32.to_be_bytes());
    bytes.extend_from_slice(&[0u8; 16]);
    std::fs::write(path, bytes).unwrap();
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
    // relay RS2: uint8 echoes (a fifth of the int16 ones) with a block saturated at 255 in
    // every echo and volume (TED-46); the first echo negated (TED-31); the int16 echoes as
    // ANALYZE pairs (stored (x - 0.5) / 2 floored, scaled back by 2 and 0.5) and MGH images
    for (e, name) in ["s1", "s2", "s3"].into_iter().enumerate() {
        let mut bytes = Vec::with_capacity(nvox * VOLUMES);
        for vol in &echoes[e] {
            for k in 0..nz {
                for j in 0..ny {
                    for i in 0..nx {
                        let x = vol[i + nx * (j + ny * k)];
                        let block =
                            (6..=8).contains(&i) && (6..=8).contains(&j) && (3..=4).contains(&k);
                        bytes.push(if block {
                            255
                        } else {
                            (x / 5).clamp(0, 254) as u8
                        });
                    }
                }
            }
        }
        let path = dir.join(format!("{name}.nii"));
        nifti_file(&path, &dims, 2, 8, &bytes);
        paths.insert(name, path);
    }
    let path = dir.join("d1.nii");
    nifti_file(
        &path,
        &dims,
        4,
        16,
        &int16_bytes(echoes[0].iter().flatten().map(|&x| -x)),
    );
    paths.insert("d1", path);
    for (e, (a, g)) in [("a1", "g1"), ("a2", "g2"), ("a3", "g3")]
        .into_iter()
        .enumerate()
    {
        let path = dir.join(format!("{a}.hdr"));
        analyze_pair(
            &path,
            &dims,
            echoes[e]
                .iter()
                .flatten()
                .map(|&x| (2 * x - 1).div_euclid(4)),
        );
        paths.insert(a, path);
        let path = dir.join(format!("{g}.mgh"));
        mgh_file(&path, &dims, echoes[e].iter().flatten().copied());
        paths.insert(g, path);
    }
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

/// `text` with this crate's version written `<version>` where tedana's description names the
/// program (`"Python": "larmorx-gpl <version> (tedana 26.0.3's t2smap ...)"`), so that a release
/// changes no checksum (as the Python golden tests' HDF5 `/OSVersion`). Only this version is
/// replaced, in Cargo's form (`0.1.0-rc.1`) or PEP 440's (`0.1.0rc1`): any other still differs.
fn without_version(text: &str) -> String {
    [env!("CARGO_PKG_VERSION"), VERSION]
        .iter()
        .fold(text.to_owned(), |t, v| {
            t.replace(
                &format!("\"Python\": \"larmorx-gpl {v} "),
                "\"Python\": \"larmorx-gpl <version> ",
            )
        })
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
            out.push((
                name,
                text_fingerprint(&without_version(&without_node(&text))),
            ));
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
        let out = Command::new(program())
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
            "{} golden mismatches of {} (recorded from {source}); outputs saved in {}:\n{}",
            problems.len(),
            program().display(),
            save.display(),
            problems.join("\n")
        );
    }
}

#[test]
fn the_version_is_written_as_a_placeholder() {
    let line = |v: &str| format!("    \"Python\": \"larmorx-gpl {v} (tedana 26.0.3's t2smap)\",\n");
    let normalised = line("<version>");
    assert_eq!(
        without_version(&line(env!("CARGO_PKG_VERSION"))),
        normalised
    );
    assert_eq!(without_version(&line(VERSION)), normalised);
    // another version (an older program's) is kept, so its checksum differs
    assert_eq!(without_version(&line("0.0.1")), line("0.0.1"));
}

#[test]
fn every_case_has_its_own_id() {
    for (i, a) in CASES.iter().enumerate() {
        assert!(CASES[..i].iter().all(|b| b.id != a.id), "{}", a.id);
    }
}
