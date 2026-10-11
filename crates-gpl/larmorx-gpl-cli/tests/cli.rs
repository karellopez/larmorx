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
