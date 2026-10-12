// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Karel Lopez Vilaret.
//! Runs the `larmorx-gpl` binary as a separate process on generated files.

mod common;

const EXE: &str = env!("CARGO_BIN_EXE_larmorx-gpl");

#[test]
fn version() {
    let out = common::run(EXE, &["--version"]);
    assert!(out.status.success(), "{out:?}");
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        format!("larmorx-gpl {}\n", env!("CARGO_PKG_VERSION"))
    );
    let out = common::run(EXE, &["no-such-family"]);
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn tshift_writes_afni_output() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("in.nii.gz");
    common::int16_scaled(&input);
    for (i, method) in ["-Fourier", "-cubic", "-heptic", "-wsinc9"]
        .iter()
        .enumerate()
    {
        let output = dir.path().join(format!("out{i}.nii"));
        let out = common::run(
            EXE,
            &[
                "afni",
                "3dTshift",
                "-tpattern",
                "alt+z",
                "-ignore",
                "2",
                method,
                "-prefix",
                output.to_str().unwrap(),
                input.to_str().unwrap(),
            ],
        );
        assert!(out.status.success(), "{method}: {out:?}");
        let stdout = String::from_utf8(out.stdout).unwrap();
        // alt+z over 4 slices with TR 2: times 0, 1, 0.5, 1.5; tzero = their mean.
        assert_eq!(stdout, "++ updating time offset to 0.75\n", "{method}");
        let (bytes, h) = common::read(&output);
        let (input_bytes, _) = common::read(&input);
        assert_eq!(h.datatype, 4, "{method}: int16 stays int16");
        assert_eq!((h.scl_slope, h.scl_inter), (0.25, 0.0));
        assert_eq!((h.pixdim[4], h.toffset), (2.0, 0.75));
        assert_eq!((h.xyzt_units, h.dim_info, h.slice_end), (10, 48, 3));
        assert_eq!(bytes.len(), input_bytes.len());
        assert_ne!(bytes, input_bytes, "{method}: the data were shifted");
        // The two ignored time points are unchanged.
        let vol = 5 * 3 * 4 * 2;
        assert_eq!(bytes[..2 * vol], input_bytes[..2 * vol], "{method}");
    }
}

#[test]
fn tshift_copies_and_errors() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("in.nii");
    common::float32(&input);
    let output = dir.path().join("copy.nii");
    // No slice timing anywhere: a copy of the input.
    let out = common::run(
        EXE,
        &[
            "afni",
            "3dTshift",
            "-prefix",
            output.to_str().unwrap(),
            input.to_str().unwrap(),
        ],
    );
    assert!(out.status.success(), "{out:?}");
    let err = String::from_utf8(out.stderr).unwrap();
    assert!(err.contains("just a copy of input"), "{err}");
    assert_eq!(common::read(&output).0, common::read(&input).0);

    // -verbose names the implementation first.
    let verbose = dir.path().join("verbose.nii");
    let out = common::run(
        EXE,
        &[
            "afni",
            "3dTshift",
            "-verbose",
            "-prefix",
            verbose.to_str().unwrap(),
            input.to_str().unwrap(),
        ],
    );
    assert!(out.status.success(), "{out:?}");
    let err = String::from_utf8(out.stderr).unwrap();
    assert!(
        err.starts_with("++ implementation: replica (larmorx-gpl "),
        "{err}"
    );

    for (args, message) in [
        (
            vec!["-tpattern", "ALT+Z"],
            "** ERROR: Unknown tpattern = ALT+Z",
        ),
        (
            vec!["-tpattern", "alt+z", "-ignore", "20"],
            "-ignore value 20 is too large",
        ),
        (
            vec!["-tpattern", "alt+z", "-slice", "4"],
            "-slice value is too large (4 >= 4)",
        ),
        (
            vec!["-tpattern", "@1D: 0 1 2.5 1"],
            "Illegal value 2.5 in tpattern file",
        ),
        (vec!["-tpattern", "alt+z"], "conflicts with existing file"),
    ] {
        let mut all = vec!["afni", "3dTshift"];
        all.extend(args);
        all.extend(["-prefix", output.to_str().unwrap(), input.to_str().unwrap()]);
        let out = common::run(EXE, &all);
        assert_eq!(out.status.code(), Some(1), "{message}");
        let err = String::from_utf8(out.stderr).unwrap();
        assert!(err.contains(message), "{message}: {err}");
    }
}

/// D13: `LARMORX_UPSTREAM_DEFECTS` chooses between the corrections and AFNI's behaviour; the
/// command line's own defects are an existing output (AFNI-11) and refused names (AFNI-12).
/// Outputs are named relative to the temporary directory, so that its own path (which may hold
/// a blank) is not part of the `-prefix`.
#[test]
fn tshift_upstream_defects_switch() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("in.nii");
    common::float32(&input);
    // AFNI's labels without its colour codes, which `reproduce` mode prints otherwise.
    let run = |prefix: &str, mode: Option<&str>| {
        let mut command = std::process::Command::new(EXE);
        command
            .args(["afni", "3dTshift", "-tpattern", "alt+z", "-prefix", prefix])
            .arg(&input)
            .current_dir(dir.path())
            .env("AFNI_MESSAGE_COLORIZE", "NO")
            .env_remove("LARMORX_UPSTREAM_DEFECTS");
        if let Some(mode) = mode {
            command.env("LARMORX_UPSTREAM_DEFECTS", mode);
        }
        let out = command.output().unwrap();
        (out.status.code(), String::from_utf8(out.stderr).unwrap())
    };

    // An invalid mode is a usage error.
    let (code, err) = run("fine.nii", Some("faithful"));
    assert_eq!(code, Some(2), "{err}");
    assert!(err.contains("must be 'fix' or 'reproduce'"), "{err}");

    // AFNI-11: an existing output.
    let taken = dir.path().join("taken.nii");
    std::fs::write(&taken, "taken").unwrap();
    let (code, err) = run("taken.nii", None);
    assert_eq!(code, Some(1), "{err}");
    assert!(
        err.contains("upstream defect AFNI-11 (3dTshift) corrected"),
        "{err}"
    );
    let (code, err) = run("taken.nii", Some("reproduce"));
    assert_eq!(code, Some(0), "{err}");
    assert!(
        err.contains("** ERROR: output dataset name 'taken.nii' conflicts with existing file"),
        "{err}"
    );
    assert!(
        err.contains("** ERROR: dataset NOT written to disk!"),
        "{err}"
    );
    assert_eq!(std::fs::read(&taken).unwrap(), b"taken");

    // AFNI-12: a -prefix with a blank.
    let (code, err) = run("with blank.nii", Some("reproduce"));
    assert_eq!(code, Some(1), "{err}");
    assert!(
        err.contains("illegal value 'with blank.nii' after -prefix"),
        "{err}"
    );
    assert!(!dir.path().join("with blank.nii").exists());
    let (code, err) = run("with blank.nii", None);
    assert_eq!(code, Some(0), "{err}");
    assert!(
        err.contains("upstream defect AFNI-12 (3dTshift) corrected"),
        "{err}"
    );
    assert!(dir.path().join("with blank.nii").exists());

    // AFNI-12 when AFNI writes: a -prefix that passes AFNI's option scan through its "3dcalc"
    // exception is refused by its NIfTI writer as "./3dcalc a b.nii" (nothing written, exit 0).
    let (code, err) = run("3dcalc a b.nii", Some("reproduce"));
    assert_eq!(code, Some(0), "{err}");
    assert!(
        err.contains("** ERROR: Illegal filename for NIfTI output: ./3dcalc a b.nii"),
        "{err}"
    );
    assert!(err.contains("upstream defect AFNI-12 (3dTshift) reproduced"));
    assert!(!dir.path().join("3dcalc a b.nii").exists());
    let (code, err) = run("3dcalc a b.nii", None);
    assert_eq!(code, Some(0), "{err}");
    assert!(err.contains("upstream defect AFNI-12 (3dTshift) corrected"));
    assert!(dir.path().join("3dcalc a b.nii").exists());

    // A plain run names nothing, in either mode.
    for mode in [None, Some("fix"), Some("reproduce"), Some("")] {
        let name = format!("plain-{}.nii", mode.unwrap_or("unset"));
        let (code, err) = run(&name, mode);
        assert_eq!(code, Some(0), "{err}");
        assert!(!err.contains("upstream defect"), "{err}");
    }
}

/// Runs `larmorx-gpl afni 3dTshift args` in `dir` in the default mode, with AFNI's variables
/// `env` set (and plain messages); returns the exit status and standard error.
fn tshift_in(dir: &std::path::Path, args: &[&str], env: &[(&str, &str)]) -> (Option<i32>, String) {
    let mut command = std::process::Command::new(EXE);
    command
        .args(["afni", "3dTshift"])
        .args(args)
        .current_dir(dir)
        .env_remove("LARMORX_UPSTREAM_DEFECTS")
        .env("AFNI_MESSAGE_COLORIZE", "NO");
    for (name, value) in env {
        command.env(name, value);
    }
    let out = command.output().unwrap();
    (out.status.code(), String::from_utf8(out.stderr).unwrap())
}

/// AFNI's global options on every platform (`docs/api/afni-tshift.md`): `-overwrite` replaces
/// an existing output whole, wherever it stands, also under a name with a blank and non-ASCII
/// characters; `-Dname=value` and the environment set AFNI's variables alike, `-D` winning.
#[test]
fn tshift_global_options() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("in.nii");
    common::float32(&input);
    let input = input.to_str().unwrap();
    let ok = |(code, err): (Option<i32>, String)| assert_eq!(code, Some(0), "{err}");

    // The reference: the same run without an existing output.
    ok(tshift_in(
        dir.path(),
        &["-tpattern", "alt+z", "-prefix", "ref.nii", input],
        &[],
    ));
    let reference = std::fs::read(dir.path().join("ref.nii")).unwrap();

    // An existing output is an error without -overwrite, and is left as it was.
    let target = dir.path().join("été 1.nii");
    std::fs::write(&target, "taken").unwrap();
    let args = ["-tpattern", "alt+z", "-prefix", "été 1.nii", input];
    let (code, err) = tshift_in(dir.path(), &args, &[]);
    assert_eq!(code, Some(1), "{err}");
    assert_eq!(std::fs::read(&target).unwrap(), b"taken");
    // -overwrite, after the dataset, replaces it.
    let mut with = args.to_vec();
    with.push("-overwrite");
    ok(tshift_in(dir.path(), &with, &[]));
    assert_eq!(std::fs::read(&target).unwrap(), reference);
    // -overwrite in the place of -tpattern's value: taken out before the option scan.
    std::fs::write(&target, "taken").unwrap();
    ok(tshift_in(
        dir.path(),
        &[
            "-tpattern",
            "-overwrite",
            "alt+z",
            "-prefix",
            "été 1.nii",
            input,
        ],
        &[],
    ));
    assert_eq!(std::fs::read(&target).unwrap(), reference);
    // Nothing else is left in the directory (no temporary file).
    let mut names: Vec<String> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, ["in.nii", "ref.nii", "été 1.nii"]);

    // AFNI_DECONFLICT=YES, from -D or from the environment: the output goes to <name>_AA1.nii.
    std::fs::write(dir.path().join("out.nii"), "taken").unwrap();
    let rename = ["-tpattern", "alt+z", "-prefix", "out.nii", input];
    let mut by_d = vec!["-DAFNI_DECONFLICT=YES"];
    by_d.extend(rename);
    ok(tshift_in(dir.path(), &by_d, &[]));
    assert_eq!(
        std::fs::read(dir.path().join("out_AA1.nii")).unwrap(),
        reference
    );
    ok(tshift_in(
        dir.path(),
        &rename,
        &[("AFNI_DECONFLICT", "YES")],
    ));
    assert_eq!(
        std::fs::read(dir.path().join("out_AA2.nii")).unwrap(),
        reference
    );
    // -D wins over the environment: NO refuses before the work.
    let mut no = vec!["-DAFNI_DECONFLICT=NO"];
    no.extend(rename);
    let (code, err) = tshift_in(dir.path(), &no, &[("AFNI_DECONFLICT", "YES")]);
    assert_eq!(code, Some(1), "{err}");
    assert!(
        err.contains("output dataset already exists './out.nii'"),
        "{err}"
    );
    assert!(!dir.path().join("out_AA3.nii").exists());

    // AFNI_NIFTI_WRITE_TYPE=2: a NIfTI-2 file, from -D and from the environment alike.
    let sizeof_hdr = |name: &str| {
        let raw = std::fs::read(dir.path().join(name)).unwrap();
        i32::from_le_bytes(raw[..4].try_into().unwrap())
    };
    ok(tshift_in(
        dir.path(),
        &[
            "-tpattern",
            "alt+z",
            "-DAFNI_NIFTI_WRITE_TYPE=2",
            "-prefix",
            "d2.nii",
            input,
        ],
        &[],
    ));
    ok(tshift_in(
        dir.path(),
        &["-tpattern", "alt+z", "-prefix", "e2.nii", input],
        &[("AFNI_NIFTI_WRITE_TYPE", "2")],
    ));
    assert_eq!(
        (
            sizeof_hdr("d2.nii"),
            sizeof_hdr("e2.nii"),
            sizeof_hdr("ref.nii")
        ),
        (540, 540, 348)
    );
    assert_eq!(
        std::fs::read(dir.path().join("d2.nii")).unwrap(),
        std::fs::read(dir.path().join("e2.nii")).unwrap()
    );
}

/// AFNI-23: `AFNI_NIFTI_SCALE=NO` keeps int16 data with a slope and an intercept as int16,
/// with the slope as their scale factor, and AFNI drops the intercept. By default the output
/// keeps it as `scl_inter`, so it reads back as the run without the variable does, to within
/// half a stored unit; with `-rlt` the trend takes the intercept away (nothing to correct);
/// with `-rlt -ignore` the data are read as without the variable.
#[test]
fn tshift_nifti_scale_no_keeps_the_intercept() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("in.nii");
    common::int16_slope_inter(&input);
    let input = input.to_str().unwrap();
    let reproduce = ("LARMORX_UPSTREAM_DEFECTS", "reproduce");
    let run = |extra: &[&str], out: &str, env: &[(&str, &str)]| {
        let mut args = vec!["-tpattern", "alt+z", "-linear"];
        args.extend(extra);
        args.extend(["-prefix", out, input]);
        let (code, err) = tshift_in(dir.path(), &args, env);
        assert_eq!(code, Some(0), "{err}");
        let (bytes, h) = common::read(&dir.path().join(out));
        (common::scaled(&bytes, &h), bytes, h, err)
    };
    let max_diff = |a: &[f64], b: &[f64]| {
        a.iter()
            .zip(b)
            .fold(0.0f64, |m, (x, y)| m.max((x - y).abs()))
    };
    let corrected = "upstream defect AFNI-23 (3dTshift) corrected";
    let reproduced = "upstream defect AFNI-23 (3dTshift) reproduced";

    // Without the variable: float32, slope * x + inter.
    let (plain, _, h, err) = run(&[], "plain.nii", &[]);
    assert_eq!((h.datatype, h.scl_slope), (16, 0.0));
    assert!(!err.contains("AFNI-23"), "{err}");

    // With it, from -D and from the environment alike.
    let (fix, fix_bytes, h, err) = run(&["-DAFNI_NIFTI_SCALE=NO"], "fix.nii", &[]);
    assert_eq!((h.datatype, h.scl_slope, h.scl_inter), (4, 0.25, 100.0));
    assert!(err.contains(corrected), "{err}");
    assert!(
        max_diff(&fix, &plain) <= 0.125,
        "{}",
        max_diff(&fix, &plain)
    );
    let (_, env_bytes, _, _) = run(&[], "env.nii", &[("AFNI_NIFTI_SCALE", "NO")]);
    assert_eq!(env_bytes, fix_bytes);
    let (rep, rep_bytes, h, err) = run(&["-DAFNI_NIFTI_SCALE=NO"], "rep.nii", &[reproduce]);
    assert_eq!((h.datatype, h.scl_slope, h.scl_inter), (4, 0.25, 0.0));
    assert!(err.contains(reproduced), "{err}");
    assert_eq!(rep_bytes, fix_bytes, "only the header differs");
    assert!((max_diff(&rep, &plain) - 100.0).abs() <= 0.125);

    // -rlt: the trend, intercept included, is removed; AFNI's output is right as it is.
    let (rlt_plain, ..) = run(&["-rlt"], "rlt-plain.nii", &[]);
    for env in [&[][..], &[reproduce][..]] {
        let (rlt, _, h, err) = run(&["-rlt", "-DAFNI_NIFTI_SCALE=NO"], "rlt.nii", env);
        assert_eq!((h.datatype, h.scl_inter), (4, 0.0));
        assert!(!err.contains("AFNI-23"), "{err}");
        assert!(max_diff(&rlt, &rlt_plain) <= 0.125);
    }

    // -rlt -ignore 2: the ignored volumes keep the intercept, the others lose it.
    let ignore = ["-rlt", "-ignore", "2", "-DAFNI_NIFTI_SCALE=NO"];
    let (_, plain_bytes, ..) = run(&ignore[..3], "ign-plain.nii", &[]);
    let (_, bytes, h, err) = run(&ignore, "ign-fix.nii", &[]);
    assert_eq!(h.datatype, 16);
    assert_eq!(bytes, plain_bytes, "read as without the variable");
    assert!(err.contains(corrected), "{err}");
    let (_, _, h, err) = run(&ignore, "ign-rep.nii", &[reproduce]);
    assert_eq!((h.datatype, h.scl_inter), (4, 0.0));
    assert!(err.contains(reproduced), "{err}");
}

/// On Windows a rename over a file another program holds open fails; the output is renamed
/// over it once it is released (here after 0.3 s), within the replica's retries.
#[cfg(windows)]
#[test]
fn tshift_overwrite_waits_for_a_held_output() {
    use std::os::windows::fs::OpenOptionsExt;
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("in.nii");
    common::float32(&input);
    let target = dir.path().join("held.nii");
    std::fs::write(&target, "taken").unwrap();
    // Shared for reading only (FILE_SHARE_READ): nothing may replace it meanwhile.
    let held = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(&target)
        .unwrap();
    let release = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(300));
        drop(held);
    });
    let (code, err) = tshift_in(
        dir.path(),
        &[
            "-overwrite",
            "-tpattern",
            "alt+z",
            "-prefix",
            "held.nii",
            input.to_str().unwrap(),
        ],
        &[],
    );
    release.join().unwrap();
    assert_eq!(code, Some(0), "{err}");
    assert_ne!(std::fs::read(&target).unwrap(), b"taken");
}
