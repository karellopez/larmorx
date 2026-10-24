// SPDX-License-Identifier: LicenseRef-FSL
// Copyright 2026 Karel Lopez Vilaret.
//! Golden tests of the BET replicas: every platform reproduces the files `larmorx-nc-bet`
//! writes on Linux x86_64, byte for byte (CLAUDE.md rule 10, `docs/validation/golden.md`).
//!
//! The inputs are generated here from seeded integers (SplitMix64, the generator of the other
//! golden tests) and integer arithmetic: a head of 32 x 32 x 24 voxels of 2 mm (brain, CSF,
//! skull, scalp), its T2-like contrast, 4 volumes of it, and a slab of 10 slices. Each case runs
//! the program on them and hashes (SHA-256) every file it writes (NIfTI decompressed, VTK
//! meshes as text); the hashes are compared with `tests/golden.tsv`. One case per distinct code
//! path: bet2's outputs and options, bet's variations (-R, -F, -Z), betsurf with and without the
//! T2, and the paths of FSL's defects (D13): corrected (the default) and, in the `*.reproduce`
//! cases, reproduced.
//!
//! Recording, on Linux x86_64 only, from a committed tree whose parity passes:
//! `LARMORX_GOLDEN_RECORD=1 cargo test --locked -p larmorx-nc-bet --test golden`
//! (`LARMORX_GOLDEN_ALLOW_DIRTY=1` to record from uncommitted changes). On a mismatch the
//! outputs are saved in `$LARMORX_GOLDEN_MISMATCHES` (CI uploads it), else in
//! `target/tmp/golden-mismatches`.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use sha2::{Digest, Sha256};

const EXE: &str = env!("CARGO_BIN_EXE_larmorx-nc-bet");
const GOLDEN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden.tsv");

/// The head's grid.
const HEAD: [i64; 3] = [32, 32, 24];

struct Case {
    id: &'static str,
    /// `<family> <tool>`, as `larmorx-nc-bet --help` lists it.
    tool: &'static str,
    /// The arguments; `{t1}`, `{t2}`, `{4d}`, `{slab}` are the inputs, `{mesh}` and `{mat}` the
    /// bet2 mesh and the matrix betsurf reads.
    args: &'static [&'static str],
    /// `LARMORX_UPSTREAM_DEFECTS`.
    mode: &'static str,
    /// The upstream defects the run must name on standard error, and no other.
    named: &'static [&'static str],
    /// `FSLOUTPUTTYPE`.
    output_type: &'static str,
}

const CASES: &[Case] = &[
    Case {
        id: "bet2.outputs",
        tool: "fsl bet2",
        args: &["{t1}", "out", "-o", "-m", "-s", "-e", "-f", "0.4"],
        mode: "fix",
        named: &[],
        output_type: "NIFTI",
    },
    Case {
        id: "bet2.threshold-gradient-centre",
        tool: "fsl bet2",
        args: &[
            "{t1}", "out", "-n", "-m", "-t", "-g", "0.2", "-w", "1.5", "-r", "50", "-c", "16",
            "15", "12",
        ],
        mode: "fix",
        named: &[],
        output_type: "NIFTI_GZ",
    },
    Case {
        id: "bet2.ct.pair",
        tool: "fsl bet2",
        args: &["{t1}", "out", "--ct", "-m"],
        mode: "fix",
        named: &[],
        output_type: "NIFTI_PAIR",
    },
    Case {
        id: "bet2.hdr-name.reproduce",
        tool: "fsl bet2",
        args: &["{t1}", "out.hdrx", "-m"],
        mode: "reproduce",
        named: &["FSL-40"],
        output_type: "NIFTI",
    },
    Case {
        id: "bet.robust",
        tool: "fsl bet",
        args: &["{t1}", "out", "-R", "-m"],
        mode: "fix",
        named: &[],
        output_type: "NIFTI",
    },
    Case {
        id: "bet.fmri",
        tool: "fsl bet",
        args: &["{4d}", "out", "-F"],
        mode: "fix",
        named: &[],
        output_type: "NIFTI",
    },
    Case {
        id: "bet.small-fov",
        tool: "fsl bet",
        args: &["{slab}", "out", "-Z", "-m"],
        mode: "fix",
        named: &["FSL-45"],
        output_type: "NIFTI",
    },
    Case {
        id: "bet.small-fov.reproduce",
        tool: "fsl bet",
        args: &["{slab}", "out", "-Z", "-m"],
        mode: "reproduce",
        named: &["FSL-45"],
        output_type: "NIFTI",
    },
    Case {
        id: "betsurf.t1only",
        tool: "fsl betsurf",
        args: &[
            "--t1only", "-o", "-m", "-s", "{t1}", "{mesh}", "{mat}", "out",
        ],
        mode: "fix",
        named: &[],
        output_type: "NIFTI",
    },
    Case {
        id: "betsurf.t2.precision",
        tool: "fsl betsurf",
        args: &[
            "-m", "-s", "-p", "1", "{t1}", "{t2}", "{mesh}", "{mat}", "out",
        ],
        mode: "fix",
        named: &[],
        output_type: "NIFTI",
    },
];

// ------------------------------------------------------------------------------------------------
// Inputs

/// SplitMix64: `n` values from `seed`.
fn splitmix64(seed: u64, n: usize) -> Vec<u64> {
    let mut state = seed;
    (0..n)
        .map(|_| {
            state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
            let mut z = state;
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            z ^ (z >> 31)
        })
        .collect()
}

/// A head: brain, CSF, skull, scalp and air by (twice) the squared distance from the centre,
/// with an intensity ramp along x and noise.
fn head_values(levels: [i64; 5], seed: u64) -> Vec<i64> {
    let [nx, ny, nz] = HEAD;
    let noise = splitmix64(seed, (nx * ny * nz) as usize);
    let mut v = Vec::with_capacity(noise.len());
    for k in 0..nz {
        for j in 0..ny {
            for i in 0..nx {
                let (x, y, z) = (2 * i - 31, 2 * j - 31, 2 * k - 23);
                let r = x * x + y * y + 2 * z * z;
                let level = match r {
                    0..324 => levels[0],
                    324..400 => levels[1],
                    400..484 => levels[2],
                    484..676 => levels[3],
                    _ => levels[4],
                };
                let n = (noise[v.len()] % 41) as i64 - 20;
                v.push(level * (90 + i) / 100 + n);
            }
        }
    }
    v
}

const T1_LEVELS: [i64; 5] = [700, 150, 60, 800, 10];
const T2_LEVELS: [i64; 5] = [400, 1500, 40, 300, 10];

/// A NIfTI-1 single file: 2 mm voxels, an sform and a qform of `diag(-2, 2, 2)` (radiological
/// storage, as most scanners' files), the data type and values given.
fn nifti(dims: &[i64], datatype: i16, bitpix: i16, data: &[u8]) -> Vec<u8> {
    let mut h = vec![0u8; 352];
    h[0..4].copy_from_slice(&348i32.to_le_bytes());
    let mut dim = [1i16; 8];
    dim[0] = dims.len() as i16;
    for (k, &d) in dims.iter().enumerate() {
        dim[k + 1] = d as i16;
    }
    for (k, d) in dim.iter().enumerate() {
        h[40 + 2 * k..42 + 2 * k].copy_from_slice(&d.to_le_bytes());
    }
    h[70..72].copy_from_slice(&datatype.to_le_bytes());
    h[72..74].copy_from_slice(&bitpix.to_le_bytes());
    let pixdim: [f32; 8] = [-1.0, 2.0, 2.0, 2.0, 1.5, 1.0, 1.0, 1.0];
    for (k, p) in pixdim.iter().enumerate() {
        h[76 + 4 * k..80 + 4 * k].copy_from_slice(&p.to_le_bytes());
    }
    h[108..112].copy_from_slice(&352f32.to_le_bytes());
    h[112..116].copy_from_slice(&1f32.to_le_bytes()); // scl_slope
    h[123] = 10; // xyzt_units: mm, s
    h[252..254].copy_from_slice(&1i16.to_le_bytes()); // qform_code
    h[254..256].copy_from_slice(&1i16.to_le_bytes()); // sform_code
    // quaternion (0, 0, 0): identity rotation, qfac -1 flips z; offsets
    let (ox, oy, oz) = (31.0f32, -31.0f32, -23.0f32);
    for (k, v) in [0f32, 0.0, 0.0, ox, oy, oz].iter().enumerate() {
        h[256 + 4 * k..260 + 4 * k].copy_from_slice(&v.to_le_bytes());
    }
    let rows: [[f32; 4]; 3] = [
        [-2.0, 0.0, 0.0, ox],
        [0.0, 2.0, 0.0, oy],
        [0.0, 0.0, 2.0, oz],
    ];
    for (r, row) in rows.iter().enumerate() {
        for (c, v) in row.iter().enumerate() {
            let at = 280 + 16 * r + 4 * c;
            h[at..at + 4].copy_from_slice(&v.to_le_bytes());
        }
    }
    h[344..348].copy_from_slice(b"n+1\0");
    h.extend_from_slice(data);
    h
}

fn i16_bytes(v: &[i64]) -> Vec<u8> {
    v.iter().flat_map(|&x| (x as i16).to_le_bytes()).collect()
}

fn f32_bytes(v: &[i64]) -> Vec<u8> {
    v.iter().flat_map(|&x| (x as f32).to_le_bytes()).collect()
}

fn write_inputs(dir: &Path) -> BTreeMap<&'static str, PathBuf> {
    let [nx, ny, nz] = HEAD;
    let t1 = head_values(T1_LEVELS, 1);
    let t2 = head_values(T2_LEVELS, 2);
    let mut paths = BTreeMap::new();
    let mut put = |name: &'static str, bytes: Vec<u8>| {
        let p = dir.join(format!("{name}.nii"));
        std::fs::write(&p, bytes).unwrap();
        paths.insert(name, p);
    };
    put("t1", nifti(&[nx, ny, nz], 4, 16, &i16_bytes(&t1)));
    put("t2", nifti(&[nx, ny, nz], 16, 32, &f32_bytes(&t2)));
    let mut series = Vec::new();
    for t in 0..4u64 {
        series.extend(head_values(T1_LEVELS, 10 + t));
    }
    put("4d", nifti(&[nx, ny, nz, 4], 4, 16, &i16_bytes(&series)));
    let plane = (nx * ny) as usize;
    let slab = &t1[7 * plane..17 * plane];
    put("slab", nifti(&[nx, ny, 10], 4, 16, &i16_bytes(slab)));
    // a matrix to MNI152 (1 mm scale, the head's centre on the template's): identity rows
    let mat = "0.5 0 0 72\n0 0.5 0 112\n0 0 0.5 62\n0 0 0 1\n";
    let p = dir.join("t1_to_std.mat");
    std::fs::write(&p, mat).unwrap();
    paths.insert("mat", p);
    paths
}

// ------------------------------------------------------------------------------------------------
// Fingerprints

/// `(case, file) -> sha256` of every output.
type Recorded = BTreeMap<(String, String), String>;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

/// The SHA-256 of a file's content (a gzip file's decompressed content).
fn fingerprint(path: &Path) -> String {
    let raw = std::fs::read(path).unwrap();
    let content = if raw.starts_with(&[0x1f, 0x8b]) {
        let mut out = Vec::new();
        flate2::read::MultiGzDecoder::new(&raw[..])
            .read_to_end(&mut out)
            .unwrap();
        out
    } else {
        raw
    };
    hex(&Sha256::digest(&content))
}

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
        assert_eq!(f.len(), 3, "{GOLDEN}: bad line {line:?}");
        recorded.insert((f[0].to_owned(), f[1].to_owned()), f[2].to_owned());
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
         # Golden checksums of the BET replicas (tests/golden.rs): SHA-256 of every file that\n\
         # larmorx-nc-bet writes on Linux x86_64 (NIfTI decompressed). Every platform must\n\
         # reproduce them (docs/validation/golden.md). Do not edit by hand.\n",
    );
    let _ = writeln!(
        text,
        "# Recorded from {commit} on {} (Linux x86_64)",
        today()
    );
    text.push_str("# case\tfile\tsha256\n");
    for ((case, file), sha) in actual {
        let _ = writeln!(text, "{case}\t{file}\t{sha}");
    }
    std::fs::write(GOLDEN, text).unwrap();
    eprintln!("recorded {} files in {GOLDEN}", actual.len());
}

fn mismatch_dir() -> PathBuf {
    std::env::var_os("LARMORX_GOLDEN_MISMATCHES")
        .map_or_else(
            || Path::new(env!("CARGO_TARGET_TMPDIR")).join("golden-mismatches"),
            PathBuf::from,
        )
        .join("larmorx-nc")
}

/// Runs `larmorx-nc-bet <family> <tool> args` in `cwd`.
fn run(
    cwd: &Path,
    tool: &str,
    args: &[String],
    mode: &str,
    output_type: &str,
) -> std::process::Output {
    let (family, tool) = tool.split_once(' ').unwrap();
    Command::new(EXE)
        .arg(family)
        .arg(tool)
        .args(args)
        .current_dir(cwd)
        .env_remove("FSLDIR")
        .env_remove("FSL_LOAD_NIFTI_EXTENSIONS")
        .env("PATH", "")
        .env("FSLOUTPUTTYPE", output_type)
        .env("LARMORX_UPSTREAM_DEFECTS", mode)
        .env("OMP_NUM_THREADS", "2")
        .output()
        .unwrap()
}

// ------------------------------------------------------------------------------------------------
// Tests

#[test]
fn golden() {
    let dir = tempfile::tempdir().unwrap();
    let inputs = write_inputs(dir.path());
    // the brain surface betsurf starts from: bet2's mesh of the head
    let prep = dir.path().join("prep");
    std::fs::create_dir(&prep).unwrap();
    let t1 = inputs["t1"].display().to_string();
    let out = run(
        &prep,
        "fsl bet2",
        &[t1, "t1".into(), "-e".into(), "-n".into()],
        "fix",
        "NIFTI",
    );
    assert!(out.status.success(), "bet2 -e: {out:?}");
    let mesh = prep.join("t1_mesh.vtk");
    let mut actual = Recorded::new();
    let mut dirs = BTreeMap::new();
    for case in CASES {
        let cwd = dir.path().join(case.id);
        std::fs::create_dir(&cwd).unwrap();
        let args: Vec<String> = case
            .args
            .iter()
            .map(|a| match *a {
                "{t1}" => inputs["t1"].display().to_string(),
                "{t2}" => inputs["t2"].display().to_string(),
                "{4d}" => inputs["4d"].display().to_string(),
                "{slab}" => inputs["slab"].display().to_string(),
                "{mat}" => inputs["mat"].display().to_string(),
                "{mesh}" => mesh.display().to_string(),
                other => other.to_owned(),
            })
            .collect();
        let out = run(&cwd, case.tool, &args, case.mode, case.output_type);
        assert!(out.status.success(), "{}: {out:?}", case.id);
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
        let mut files: Vec<PathBuf> = std::fs::read_dir(&cwd)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        files.sort();
        assert!(!files.is_empty(), "{}: no output", case.id);
        for f in files {
            let name = f.file_name().unwrap().to_string_lossy().into_owned();
            actual.insert((case.id.to_owned(), name), fingerprint(&f));
        }
        dirs.insert(case.id, cwd);
    }

    if std::env::var_os("LARMORX_GOLDEN_RECORD").is_some() {
        record(&actual);
        return;
    }
    let (source, recorded) = read_golden();
    let mut problems = Vec::new();
    let mut bad = std::collections::BTreeSet::new();
    for key in recorded.keys().chain(actual.keys()) {
        let (case, file) = key;
        let line = match (recorded.get(key), actual.get(key)) {
            (Some(e), Some(a)) if e == a => continue,
            (Some(e), Some(a)) => format!(
                "{case} {file}: sha256 {}… expected, {}… found",
                &e[..16],
                &a[..16]
            ),
            (Some(_), None) => format!("{case} {file}: recorded but no longer produced"),
            (None, _) => format!("{case} {file}: not recorded (record on Linux x86_64)"),
        };
        if bad.insert(key.clone()) {
            problems.push(line);
        }
    }
    if !problems.is_empty() {
        let root = mismatch_dir();
        for (case, file) in &bad {
            if let Some(d) = dirs.get(case.as_str()) {
                let target = root.join(case);
                std::fs::create_dir_all(&target).unwrap();
                let _ = std::fs::copy(d.join(file), target.join(file));
            }
        }
        panic!(
            "larmorx-nc-bet differs from its Linux x86_64 output (recorded from {source}):\n  {}\n\
             The outputs are saved in {}.",
            problems.join("\n  "),
            root.display()
        );
    }
}

#[test]
fn every_tool_has_a_golden_case() {
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
