// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The GIFTI writers' and the GIFTI volumes' corrections (relay 22, FS-397 to FS-400 and FS-521
//! to FS-524): what the default gives, written and read back, against what `reproduce` gives (the
//! probe's `gwr` compares `reproduce` with FreeSurfer's own build; these tests show the defaults).

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{ERROR_BADFILE, ERROR_BADPARM, Fatal, NO_ERROR};
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_mri::ctab::{ColorTable, ColorTableEntry};
use larmorx_freesurfer_mri::mri::{MRI_FLOAT, Mri, mri_alloc_sequence};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};
use larmorx_freesurfer_surf::aaa::SurfGlobals;
use larmorx_freesurfer_surf::coords::Io;
use larmorx_freesurfer_surf::gifti::{FS_MRISURFOVERLAY_STATS, GIFTI_FILE, OverlayInfo};
use larmorx_freesurfer_surf::gifti_vol::{mri_write_gifti, mris_read_gifti_as_mri};
use larmorx_freesurfer_surf::gifti_write::{
    get_shape_stat_intent_count, mris_write_gifti, mris_write_gifti_combined, mris_write_gifti_mri,
};
use larmorx_freesurfer_surf::mris::Mris;
use larmorx_freesurfer_surf::{alloc, gifti};

const LABEL: i32 = 1002;
const SHAPE: i32 = 2005;
/// `NIFTI_INTENT_NORMAL`: a statistics intent the GIFTI volume reader reads.
const NORMAL: i32 = 11;

struct Run {
    g: Globals,
    sg: SurfGlobals,
    env: Environment,
    out: Vec<u8>,
    err: Vec<u8>,
    dir: tempfile::TempDir,
}

impl Run {
    fn new(mode: UpstreamDefects) -> Run {
        Run {
            g: Globals::new(mode),
            sg: SurfGlobals::default(),
            env: Environment::from_pairs(vec![(
                "FREESURFER_REPLACEMENT_FOR_CREATION_TIME_STRING".to_owned(),
                "now".to_owned(),
            )]),
            out: Vec::new(),
            err: Vec::new(),
            dir: tempfile::tempdir().expect("a folder"),
        }
    }

    /// A surface of `nv` vertices (at least 3) and one face: coordinates, curvature and stat from
    /// the vertex number, annotations from `annots`.
    fn surface(&mut self, nv: i32, annots: &[i32]) -> Mris {
        let mut m = alloc::alloc(&mut self.g, &mut self.out, &mut self.err, &self.sg, nv, 1)
            .expect("a surface");
        m.face_mut(0).v = [0, 1, 2];
        for vno in 0..nv as usize {
            let v = m.vertex_mut(vno);
            (v.x, v.y, v.z) = (vno as f32, 1.0, 2.0);
            v.curv = vno as f32 / 2.0;
            v.stat = -(vno as f32);
            v.annotation = annots.get(vno).copied().unwrap_or(0);
        }
        m
    }

    fn volume(&mut self, w: i32, h: i32, d: i32, frames: i32) -> Mri {
        let mut v = mri_alloc_sequence(
            &mut self.g,
            &mut self.out,
            &mut self.err,
            w,
            h,
            d,
            MRI_FLOAT,
            frames,
        )
        .expect("a volume");
        let mut k = 0.0f32;
        for f in 0..frames {
            for s in 0..d {
                for r in 0..h {
                    for c in 0..w {
                        k += 1.0;
                        mri_set_vox_val(&mut self.g, &mut self.err, &mut v, c, r, s, f, k, 0)
                            .expect("a voxel");
                    }
                }
            }
        }
        v
    }

    fn io(&mut self) -> (Io<'_>, &mut Globals, &mut SurfGlobals, &std::path::Path) {
        (
            Io {
                out: &mut self.out,
                err: &mut self.err,
                env: &self.env,
            },
            &mut self.g,
            &mut self.sg,
            self.dir.path(),
        )
    }

    fn read(&mut self, name: &str) -> Option<Mri> {
        let (mut io, g, sg, dir) = self.io();
        mris_read_gifti_as_mri(g, sg, &mut io, Some(dir), name, true).expect("a read")
    }

    fn count(&mut self, name: &str) -> i32 {
        let (mut io, g, sg, dir) = self.io();
        let (mut a, mut b) = (0, 0);
        get_shape_stat_intent_count(g, sg, &mut io, Some(dir), name, &mut a, &mut b)
            .expect("a count")
    }

    fn values(&mut self, m: &Mri) -> Vec<Vec<f32>> {
        let mut sink = Vec::new();
        (0..m.nframes)
            .map(|f| {
                (0..m.width)
                    .map(|c| {
                        mri_get_vox_val(&mut self.g, &mut sink, m, c, 0, 0, f, 0).expect("a voxel")
                    })
                    .collect()
            })
            .collect()
    }

    fn ids(&self) -> Vec<&'static str> {
        self.g
            .notices
            .as_slice()
            .iter()
            .map(|n| n.defect.id)
            .collect()
    }
}

fn entry(name: &str, r: i32, g: i32, b: i32) -> Option<Box<ColorTableEntry>> {
    Some(Box::new(ColorTableEntry {
        name: name.as_bytes().to_vec(),
        ri: r,
        gi: g,
        bi: b,
        ai: 0,
        rf: r as f32 / 255.0,
        gf: g as f32 / 255.0,
        bf: b as f32 / 255.0,
        af: 0.0,
        tissue_type: 0,
        count: 0,
    }))
}

/// FS-397: a colour table with an unnamed entry. The default writes the named entries; FreeSurfer
/// (`reproduce`) refuses its own table as invalid and writes nothing.
#[test]
fn fs397_unnamed_colour_entries() {
    for (mode, rc) in [
        (UpstreamDefects::Fix, NO_ERROR),
        (UpstreamDefects::Reproduce, ERROR_BADFILE),
    ] {
        let mut r = Run::new(mode);
        let mut m = r.surface(3, &[10, 10 | (20 << 8), 0]);
        m.ct = Some(ColorTable {
            entries: vec![
                entry("unknown", 0, 0, 0),
                entry("", 5, 5, 5),
                entry("a", 10, 20, 0),
            ],
            ..ColorTable::default()
        });
        let (mut io, g, sg, dir) = r.io();
        let e = mris_write_gifti(g, sg, &mut io, Some(dir), &mut m, LABEL, "l.gii", None)
            .expect("a write");
        assert_eq!(e, rc, "{mode:?}");
        assert!(r.ids().contains(&"FS-397"));
        if rc == NO_ERROR {
            let v = r.read("l.gii").expect("the labels read back");
            // the keys are the entries' places: the second vertex has entry 2's colours
            assert_eq!(r.values(&v), vec![vec![0.0, 2.0, 0.0]]);
            let ct = v.ct.as_ref().expect("the label table");
            assert_eq!(ct.entries.len(), 3);
            assert!(ct.entries[1].is_none(), "the unnamed entry is not written");
        }
    }
}

/// FS-398 and FS-523: labels without a colour table, an overlay without its volume. The default
/// refuses them; FreeSurfer crashes.
#[test]
fn fs398_fs523_refused_by_default() {
    let mut r = Run::new(UpstreamDefects::Fix);
    let mut m = r.surface(3, &[]);
    let (mut io, g, sg, dir) = r.io();
    let e = mris_write_gifti(g, sg, &mut io, Some(dir), &mut m, LABEL, "l.gii", None).unwrap();
    assert_eq!(e, ERROR_BADPARM);
    let ov = OverlayInfo {
        foverlay: None,
        otype: 100,
        gifti_intent: SHAPE,
        shapedatatype: Vec::new(),
        format: GIFTI_FILE,
        overlaymri: None,
        n_vals_per_vertex: 1,
    };
    let e = mris_write_gifti_combined(g, sg, &mut io, Some(dir), &mut m, &[ov], "c.gii").unwrap();
    assert_eq!(e, ERROR_BADPARM);
    let mut r = Run::new(UpstreamDefects::Reproduce);
    let mut m = r.surface(3, &[]);
    let (mut io, g, sg, dir) = r.io();
    let e = mris_write_gifti(g, sg, &mut io, Some(dir), &mut m, LABEL, "l.gii", None);
    assert!(matches!(e, Err(Fatal::Exit(139))));
}

/// FS-399 and FS-400: a statistics volume of 2 x 2 voxels and 2 frames for a surface of 4
/// vertices. The default writes one array per frame, every voxel at its place; FreeSurfer one
/// array (the last frame) with each voxel at its column.
#[test]
fn fs399_fs400_volume_overlays() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut r = Run::new(mode);
        let mut m = r.surface(4, &[]);
        let vol = r.volume(2, 2, 1, 2);
        let (mut io, g, sg, dir) = r.io();
        let e = mris_write_gifti_mri(
            g,
            sg,
            &mut io,
            Some(dir),
            &mut m,
            &vol,
            NORMAL,
            "s.gii",
            None,
            None,
        )
        .expect("a write");
        assert_eq!(e, NO_ERROR);
        assert_eq!(r.count("s.gii"), if mode.reproduce() { 1 } else { 2 });
        let back = r.read("s.gii").expect("read back");
        let v = r.values(&back);
        if mode.reproduce() {
            // the last frame (voxels 5..8), the second row over the first, columns only
            assert_eq!(v, vec![vec![7.0, 8.0, 0.0, 0.0]]);
        } else {
            assert_eq!(v, vec![vec![1.0, 2.0, 3.0, 4.0], vec![5.0, 6.0, 7.0, 8.0]]);
        }
        assert!(r.ids().contains(&"FS-399") && r.ids().contains(&"FS-400"));
    }
}

/// FS-522: a volume of 3 x 2 voxels written as GIFTI. The default writes its 6 voxels; FreeSurfer
/// its first row.
#[test]
fn fs522_volume_rows() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut r = Run::new(mode);
        let vol = r.volume(3, 2, 1, 1);
        let (mut io, g, sg, dir) = r.io();
        assert_eq!(
            mri_write_gifti(g, sg, &mut io, Some(dir), &vol, "v.gii").unwrap(),
            NO_ERROR
        );
        let back = r.read("v.gii").expect("read back");
        let want: Vec<f32> = if mode.reproduce() {
            vec![1.0, 2.0, 3.0]
        } else {
            vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]
        };
        assert_eq!(r.values(&back), vec![want]);
    }
}

/// FS-521 and FS-524: a surface with a shape overlay and a statistics overlay of the generic type,
/// written together and read as a volume. The default writes the statistics overlay (with its own
/// intent) and reads the first intent found (the shape); FreeSurfer drops the overlay and reads an
/// intent from memory it never wrote (here: none of the file's, every value 0).
#[test]
fn fs521_fs524_combined_and_mixed_intents() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut r = Run::new(mode);
        let mut m = r.surface(3, &[]);
        let shape = r.volume(3, 1, 1, 1);
        let stats = r.volume(3, 1, 1, 1);
        let ov = vec![
            OverlayInfo {
                foverlay: Some(b"lh.thickness".to_vec()),
                otype: 100,
                gifti_intent: SHAPE,
                shapedatatype: b"Thickness".to_vec(),
                format: GIFTI_FILE,
                overlaymri: Some(shape),
                n_vals_per_vertex: 1,
            },
            OverlayInfo {
                foverlay: Some(b"t.gii".to_vec()),
                otype: FS_MRISURFOVERLAY_STATS,
                gifti_intent: NORMAL,
                shapedatatype: Vec::new(),
                format: GIFTI_FILE,
                overlaymri: Some(stats),
                n_vals_per_vertex: 1,
            },
        ];
        let (mut io, g, sg, dir) = r.io();
        let e = mris_write_gifti_combined(g, sg, &mut io, Some(dir), &mut m, &ov, "c.gii").unwrap();
        assert_eq!(e, NO_ERROR);
        assert_eq!(r.count("c.gii"), if mode.reproduce() { 1 } else { 2 });
        let back = r.read("c.gii").expect("read back");
        let v = r.values(&back);
        if mode.reproduce() {
            assert_eq!(v, vec![vec![0.0, 0.0, 0.0]]);
        } else {
            assert_eq!(v, vec![vec![1.0, 2.0, 3.0]]);
        }
        assert!(r.ids().contains(&"FS-521") && r.ids().contains(&"FS-524"));
        let _ = gifti::NIFTI_INTENT_SHAPE;
    }
}
