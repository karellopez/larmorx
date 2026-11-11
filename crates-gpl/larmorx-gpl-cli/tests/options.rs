// SPDX-License-Identifier: GPL-3.0-or-later
// Part of larmorx-gpl (GNU GPL; see crates-gpl/LICENSE). Copyright 2026 Karel Lopez Vilaret.
//! larmorx's own options before the family (the user's decision of 2026-11-11): `larmorx-gpl
//! [--n-threads N] [--upstream-defects MODE] [--implementation replica] tedana t2smap ...`
//! gives the bytes of the variables the options replace (`OMP_NUM_THREADS`,
//! `LARMORX_UPSTREAM_DEFECTS`), each option winning over its variable; without t2smap's own
//! `--n-threads` (after the tool) the run's thread count does the work, with it t2smap's: the
//! thread pools started show which (`larmorx_core::parallel::pool_sizes`), and no thread count
//! changes a byte. The inputs are a small multi-echo series of seeded integers, written here.

use std::path::Path;
use std::process::Command;

use larmorx_core::defects::UpstreamDefects;
use larmorx_gpl_tedana::decay::Libraries;
use larmorx_gpl_tedana::workflow::Env;

const EXE: &str = env!("CARGO_BIN_EXE_larmorx-gpl");

/// The variables the options replace (and larmorx's choice of implementation), removed from
/// every run unless a case sets them.
const VARIABLES: [&str; 4] = [
    "OMP_NUM_THREADS",
    "LARMORX_UPSTREAM_DEFECTS",
    "LARMORX_IMPLEMENTATION",
    "COLUMNS",
];

const SHAPE: [usize; 3] = [8, 8, 6];
const VOLUMES: usize = 3;

/// A little-endian NIfTI-1 int16 file (`.nii`), 2 mm voxels, sform code 1.
fn nifti_int16(path: &Path, dims: &[usize], values: &[i16]) {
    let mut h = vec![0u8; 352];
    h[0..4].copy_from_slice(&348i32.to_le_bytes());
    h[40..42].copy_from_slice(&(dims.len() as i16).to_le_bytes());
    for k in 0..7 {
        let d = dims.get(k).copied().unwrap_or(1) as i16;
        h[42 + 2 * k..44 + 2 * k].copy_from_slice(&d.to_le_bytes());
    }
    h[70..72].copy_from_slice(&4i16.to_le_bytes());
    h[72..74].copy_from_slice(&16i16.to_le_bytes());
    for (k, p) in [1.0f32, 2.0, 2.0, 2.0, 2.0].iter().enumerate() {
        h[76 + 4 * k..80 + 4 * k].copy_from_slice(&p.to_le_bytes());
    }
    h[108..112].copy_from_slice(&352f32.to_le_bytes());
    h[112..116].copy_from_slice(&1f32.to_le_bytes());
    h[123] = 2 | 8;
    h[254..256].copy_from_slice(&1i16.to_le_bytes());
    let srow = [
        [-2.0f32, 0.0, 0.0, 8.0],
        [0.0, 2.0, 0.0, -8.0],
        [0.0, 0.0, 2.0, -6.0],
    ];
    for (r, row) in srow.iter().enumerate() {
        for (c, v) in row.iter().enumerate() {
            let o = 280 + 16 * r + 4 * c;
            h[o..o + 4].copy_from_slice(&v.to_le_bytes());
        }
    }
    h[344..348].copy_from_slice(b"n+1\0");
    for v in values {
        h.extend_from_slice(&v.to_le_bytes());
    }
    std::fs::write(path, h).unwrap();
}

/// Three echoes (`e1.nii` .. `e3.nii`) decaying by integer ratios, and `mask.nii`, in `dir`.
fn inputs(dir: &Path) {
    let [nx, ny, nz] = SHAPE;
    for (e, ratio) in [(1usize, 10i64), (2, 6), (3, 4)] {
        let mut v = Vec::with_capacity(nx * ny * nz * VOLUMES);
        for t in 0..VOLUMES {
            for k in 0..nz {
                for j in 0..ny {
                    for i in 0..nx {
                        let inside =
                            (2..6).contains(&i) && (2..6).contains(&j) && (1..5).contains(&k);
                        let base = if inside { 900 } else { 60 };
                        let s = base + ((i * 7 + j * 3 + k * 5 + t * 11) % 37) as i64;
                        v.push(i16::try_from(s * ratio / 10).unwrap());
                    }
                }
            }
        }
        nifti_int16(&dir.join(format!("e{e}.nii")), &[nx, ny, nz, VOLUMES], &v);
    }
    let mut m = Vec::with_capacity(nx * ny * nz);
    for k in 0..nz {
        for j in 0..ny {
            for i in 0..nx {
                let inside = (1..7).contains(&i) && (1..7).contains(&j) && (1..5).contains(&k);
                m.push(i16::from(inside));
            }
        }
    }
    nifti_int16(&dir.join("mask.nii"), &SHAPE, &m);
}

/// t2smap's arguments (relative names, so that no output names the folder).
const T2SMAP: [&str; 10] = [
    "-d", "e1.nii", "e2.nii", "e3.nii", "-e", "14", "30", "46", "--mask", "mask.nii",
];

/// Every file under `dir` but the inputs, by relative name, with its bytes.
fn outputs(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            let rel = p
                .strip_prefix(dir)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            if p.is_dir() {
                stack.push(p);
            } else if !matches!(rel.as_str(), "e1.nii" | "e2.nii" | "e3.nii" | "mask.nii") {
                out.push((rel, std::fs::read(&p).unwrap()));
            }
        }
    }
    out.sort();
    out
}

type Run = (i32, String, String, Vec<(String, Vec<u8>)>);

/// The folder every run of [`t2smap`] works in, emptied before each one: the same name for
/// all, since t2smap writes its output folder's absolute name into its call script.
fn work() -> &'static Path {
    static WORK: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    WORK.get_or_init(|| tempfile::tempdir().unwrap()).path()
}

/// `larmorx-gpl <options> tedana t2smap <T2SMAP> <extra>` with the variables `env` in the
/// emptied [`work`] folder: the exit status, standard output and error, the files written.
fn t2smap(options: &[&str], extra: &[&str], env: &[(&str, &str)]) -> Run {
    let dir = work().join("run");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).unwrap();
    }
    std::fs::create_dir(&dir).unwrap();
    inputs(&dir);
    let mut c = Command::new(EXE);
    c.args(options)
        .args(["tedana", "t2smap"])
        .args(T2SMAP)
        .args(extra)
        .current_dir(&dir)
        .env("TQDM_DISABLE", "1");
    for v in VARIABLES {
        c.env_remove(v);
    }
    for (k, v) in env {
        c.env(k, v);
    }
    let o = c.output().unwrap();
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&o.stdout).into_owned(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
        outputs(&dir),
    )
}

/// Each option gives the bytes of the variable it replaces, an option wins over its variable,
/// and no thread count (larmorx's, the variable's, t2smap's own) changes a byte.
#[test]
fn the_options_give_the_variables_bytes() {
    let plain = t2smap(&[], &[], &[]);
    assert_eq!(plain.0, 0, "{}", plain.2);
    assert!(
        plain.3.iter().any(|(f, _)| f.ends_with("T2starmap.nii.gz")),
        "{:?}",
        plain.3.iter().map(|(f, _)| f).collect::<Vec<_>>()
    );
    for (options, extra, env) in [
        (
            &["--n-threads", "3"][..],
            &[][..],
            &[("OMP_NUM_THREADS", "3")][..],
        ),
        (
            &["--n-threads=1"][..],
            &[][..],
            &[("OMP_NUM_THREADS", "1")][..],
        ),
        (&["--n-threads", "0"][..], &[][..], &[][..]),
        (&["--implementation", "replica"][..], &[][..], &[][..]),
        (
            &["--upstream-defects=fix"][..],
            &[][..],
            &[("LARMORX_UPSTREAM_DEFECTS", "fix")][..],
        ),
        // t2smap's own --n-threads, after the tool, beside larmorx's
        (
            &["--n-threads", "4"][..],
            &["--n-threads", "2"][..],
            &[("OMP_NUM_THREADS", "4")][..],
        ),
    ] {
        let a = t2smap(options, extra, &[]);
        let b = t2smap(&[], extra, env);
        assert!(
            a == b,
            "{options:?} {extra:?} and {env:?} differ:\n{}\n{}",
            a.2,
            b.2
        );
        // the same images and tables; t2smap's call script and dataset_description.json
        // record its own arguments (the command)
        let script = |r: &Run| -> Vec<(String, Vec<u8>)> {
            r.3.iter()
                .filter(|(f, _)| {
                    extra.is_empty()
                        || !(f.ends_with("_call.sh") || f.ends_with("dataset_description.json"))
                })
                .cloned()
                .collect()
        };
        assert!(
            script(&a) == script(&plain),
            "{options:?} {extra:?}: other files"
        );
        assert_eq!(a.3.len(), plain.3.len(), "{options:?} {extra:?}");
    }
    let reproduce = t2smap(&[], &[], &[("LARMORX_UPSTREAM_DEFECTS", "reproduce")]);
    assert_eq!(reproduce.0, 0, "{}", reproduce.2);
    let a = t2smap(&["--upstream-defects", "reproduce"], &[], &[]);
    assert!(a == reproduce, "{}", a.2);
    // the option wins: the variable is not read (a wrong value in it is no error)
    for env in [
        &[("LARMORX_UPSTREAM_DEFECTS", "reproduce")][..],
        &[("LARMORX_UPSTREAM_DEFECTS", "bogus")][..],
    ] {
        let a = t2smap(&["--upstream-defects", "fix"], &[], env);
        assert!(a == plain, "{env:?}: {}", a.2);
    }
    let a = t2smap(
        &["--upstream-defects", "reproduce"],
        &[],
        &[("LARMORX_UPSTREAM_DEFECTS", "fix")],
    );
    assert!(a == reproduce, "{}", a.2);
    let a = t2smap(&["--n-threads", "2"], &[], &[("OMP_NUM_THREADS", "x")]);
    assert!(a == plain, "{}", a.2);
    // larmorx's choice of implementation in the environment changes nothing here
    let a = t2smap(&[], &[], &[("LARMORX_IMPLEMENTATION", "clean-room")]);
    assert!(a == plain, "{}", a.2);
    // --implementation other than replica: a usage error, nothing run
    let (code, out, err, files) = t2smap(&["--implementation", "clean-room"], &[], &[]);
    assert_eq!((code, out.as_str()), (2, ""));
    assert!(files.is_empty(), "{files:?}");
    assert!(
        err.starts_with(
            "error: --implementation clean-room: this program runs the bit-exact replicas only"
        ),
        "{err}"
    );
}

/// The thread count reaches t2smap's work: the run's (`Env::threads`) without t2smap's own
/// `--n-threads`, t2smap's when it is given (in this process: the pools it starts).
#[test]
fn the_run_threads_unless_t2smap_n_threads_is_given() {
    use larmorx_core::parallel::pool_sizes;
    let env = |threads: usize| Env {
        columns: None,
        terminal_columns: None,
        mode: UpstreamDefects::Fix,
        libraries: Libraries::Newest,
        threads,
    };
    let run = |args: &[&str], env: &Env| {
        let d = tempfile::tempdir().unwrap();
        let dir = d.path();
        inputs(dir);
        let mut a: Vec<String> = T2SMAP
            .iter()
            .map(|s| {
                if s.ends_with(".nii") {
                    dir.join(s).to_string_lossy().into_owned()
                } else {
                    (*s).to_owned()
                }
            })
            .collect();
        a.extend(args.iter().map(|s| (*s).to_owned()));
        a.extend([
            "--out-dir".to_owned(),
            dir.join("out").to_string_lossy().into_owned(),
        ]);
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = larmorx_gpl_tedana::t2smap(&a, env, &mut out, &mut err);
        assert_eq!(code, 0, "{}", String::from_utf8_lossy(&err));
    };
    assert!(!pool_sizes().contains(&3), "{:?}", pool_sizes());
    run(&[], &env(3));
    assert!(pool_sizes().contains(&3), "{:?}", pool_sizes());
    assert!(!pool_sizes().contains(&5) && !pool_sizes().contains(&2));
    run(&["--n-threads", "2"], &env(5));
    assert!(pool_sizes().contains(&2), "{:?}", pool_sizes());
    assert!(!pool_sizes().contains(&5), "{:?}", pool_sizes());
}
