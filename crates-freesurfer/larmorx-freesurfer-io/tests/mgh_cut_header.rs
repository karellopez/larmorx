// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! An MGH volume whose file ends inside its header (relay 59 of track A, from relay 58's finding):
//! by default a file that ends before the header's voxel type is a read error (FS-167, rule B:
//! FreeSurfer goes on with sizes from memory contents or the earlier number's bytes); a short
//! text named `.mgz` (13 bytes) is then refused at once, where the old default (missing bytes
//! read as 0) filled index tables of about 14 GB. With `reproduce` the text's frame count from
//! memory is refused as FreeSurfer refuses it (FS-760, status 254). A header cut after the type
//! is read as before. Every case runs in a few milliseconds and allocates no large table.

use std::path::Path;

use larmorx_core::defects::{FS_167, FS_760, FS_1342, UpstreamDefects};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_io::mriio;
use larmorx_freesurfer_mri::mri::index_tables_fail;

const MODES: [UpstreamDefects; 2] = [UpstreamDefects::Fix, UpstreamDefects::Reproduce];

fn dir() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("fs mgh cut ~")
        .tempdir()
        .unwrap()
}

/// `MRIread(name)` of a file holding `bytes`: whether a volume came back, the exit status of a
/// fatal error (0 without), the text printed and the notices' IDs.
fn read(
    mode: UpstreamDefects,
    d: &Path,
    name: &str,
    bytes: &[u8],
    volume: bool,
) -> (bool, i32, String, Vec<&'static str>) {
    std::fs::write(d.join(name), bytes).unwrap();
    let mut g = Globals::new(mode);
    let env = Environment::default();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let r = mriio::mri_read(
        &mut g,
        &mut out,
        &mut err,
        &env,
        Some(d),
        name,
        -1,
        volume,
        -1,
        -1,
    );
    let (got, status) = match r {
        Ok(v) => (v.is_some(), 0),
        Err(f) => (false, i32::from(f.status())),
    };
    out.extend_from_slice(&err);
    let ids = g.notices.as_slice().iter().map(|n| n.defect.id).collect();
    (got, status, String::from_utf8_lossy(&out).into_owned(), ids)
}

/// A big-endian MGH header: version 1, then `ints` (width, height, depth, frames, type, dof),
/// the RAS flag 0 and the unused space, cut to `keep` bytes.
fn header(ints: [i32; 6], keep: usize) -> Vec<u8> {
    let mut b = 1i32.to_be_bytes().to_vec();
    for v in ints {
        b.extend_from_slice(&v.to_be_bytes());
    }
    b.extend_from_slice(&0i16.to_be_bytes());
    b.resize(b.len() + 254, 0);
    b.truncate(keep);
    b
}

#[test]
fn a_short_text_named_mgz_is_refused_at_once() {
    let d = dir();
    for name in ["t.mgz", "t.mgh"] {
        let t = std::time::Instant::now();
        let (got, status, text, ids) = read(
            UpstreamDefects::Fix,
            d.path(),
            name,
            b"not a volume\n",
            true,
        );
        assert!(!got && status == 0, "{status} {text}");
        assert!(
            text.contains(&format!(
                "mghRead({}, -1): read error",
                d.path().join(name).display()
            )) || text.contains(&format!("mghRead({name}, -1): read error")),
            "{text}"
        );
        // FS-01: the error message's NUL byte
        assert_eq!(ids, vec![FS_167.id, "FS-01"], "{text}");
        assert!(t.elapsed().as_secs() < 5, "refused at once");
        let (got, status, text, ids) = read(
            UpstreamDefects::Reproduce,
            d.path(),
            name,
            b"not a volume\n",
            true,
        );
        assert!(!got && status == 254, "{status} {text}");
        assert!(text.contains("MRIalloc: could not allocate"), "{text}");
        assert!(ids.contains(&FS_760.id), "{ids:?}");
    }
}

#[test]
fn a_header_cut_before_the_type_is_a_read_error_by_default() {
    let d = dir();
    let ints = [4, 3, 2, 1, 0, 1];
    // cut in each of the sizes and in the type: a read error by default (reproduce: FreeSurfer's
    // sizes from the earlier number's bytes, a volume or another failure, never this message)
    for keep in [4, 5, 8, 11, 16, 20, 23] {
        for volume in [false, true] {
            let (got, _, text, ids) = read(
                UpstreamDefects::Fix,
                d.path(),
                "c.mgh",
                &header(ints, keep),
                volume,
            );
            assert!(!got, "{keep}: {text}");
            assert!(text.contains("read error"), "{keep}: {text}");
            assert!(ids.contains(&FS_167.id), "{keep}: {ids:?}");
            let (_, _, text, _) = read(
                UpstreamDefects::Reproduce,
                d.path(),
                "c.mgh",
                &header(ints, keep),
                volume,
            );
            assert!(!text.contains("read error"), "{keep}: {text}");
        }
    }
    // cut after the type (in dof, the flag, the unused space): read as before, in both modes
    for mode in MODES {
        for keep in [24, 26, 28, 29, 100] {
            let (got, _, text, _) = read(mode, d.path(), "c.mgh", &header(ints, keep), false);
            assert!(got, "{keep}: {text}");
            assert!(!text.contains("read error"), "{keep}: {text}");
        }
    }
    // the whole header and voxels: read
    let mut full = header(ints, 284);
    full.extend_from_slice(&[7u8; 24]);
    for mode in MODES {
        let (got, status, text, ids) = read(mode, d.path(), "f.mgh", &full, true);
        assert!(got && status == 0 && ids.is_empty(), "{text} {ids:?}");
    }
}

#[test]
fn index_tables_past_16_gib_are_refused_in_both_modes() {
    for mode in MODES {
        let mut g = Globals::new(mode);
        let mut filled = 1288;
        // two tables of 2,147,483,647 entries: 16 GiB - 8 bytes with one frame's record
        assert!(!index_tables_fail(&mut g, &mut filled, 1, 0, i32::MAX));
        assert!(!index_tables_fail(
            &mut g,
            &mut filled,
            1,
            1,
            i32::MAX - 322
        ));
        assert!(g.notices.is_empty());
        assert!(index_tables_fail(&mut g, &mut filled, 1, 2, 1000));
        assert!(g.notices.contains(&FS_1342));
        let mut g = Globals::new(mode);
        let mut filled = 0;
        assert!(!index_tables_fail(&mut g, &mut filled, 0, 0, 1_629_518_423));
        assert!(!index_tables_fail(&mut g, &mut filled, 0, 1, 1_819_636_045));
        assert!(!index_tables_fail(&mut g, &mut filled, 0, 2, 1000));
        assert!(g.notices.is_empty(), "13.8 GB: below the limit");
    }
}
