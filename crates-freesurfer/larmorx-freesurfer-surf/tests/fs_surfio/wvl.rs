// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The probe's command of relay 37 of track D: `wvl`, the values written (`valfile.rs`'s
//! `MRISwriteValues`, `MRISwriteAreaErrorToValFile`; `surfmri.rs`'s `MRIcopyMRIS`): a surface read
//! by `MRISread` (`d/lh.s0`), some vertices ripped, `Gdiag`'s show and verbose bits now and then,
//! the vertices' values given or not; then 1 to 4 operations: `MRISwriteValues` (`wv`) and
//! `MRISwriteAreaErrorToValFile` (`ae`) to w-file and volume names (some in a folder that does not
//! exist), `MRISstoreMetricProperties` (`sm`: the faces' original areas for `ae`), `MRIcopyMRIS` (`cm`) of a field (an unknown one now and then) into a new volume or a
//! given one (of the vertex count or not, 1 or 2 frames) at frame 0 to 2; then the values and every
//! file in the folder: the generator and the port's answers, as `fssurfio.cpp`'s.

use super::fli::{fv, surface};
use super::mpd::{grid, ico};
use super::sfile::list_files;
use super::srd::mesh;
use super::*;

use larmorx_freesurfer_core::diag::{DIAG_SHOW, DIAG_VERBOSE};
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_mri::mri::{MRI_FLOAT, Mri, mri_alloc_sequence};
use larmorx_freesurfer_mri::voxel::mri_get_vox_val;
use larmorx_freesurfer_surf::{readwrite, surfmri, valfile};

const NAMES: [&str; 9] = [
    "v.w",
    "v",
    "v.wq",
    "d/v",
    "v.mgh",
    "v.nii",
    "nodir/v",
    "d/v.w",
    "nodir/v.mgh",
];
const FIELDS: [&str; 19] = [
    "val",
    "curv",
    "border",
    "marked",
    "stat",
    "d",
    "undefval",
    "vnum",
    "ripflag",
    "annotation",
    "area",
    "x",
    "nx",
    "tdx",
    "mean",
    "bogus",
    "marked3",
    "fieldsign",
    "k1",
];

pub(super) fn wvl_tokens(t: &mut Vec<String>, g: &mut Gen) {
    let heavy = g.chance(5);
    let m = match g.below(10) {
        0 | 1 => {
            let (w, h) = (g.int(2, 4), g.int(2, 4));
            grid(g, w, h)
        }
        2 => mesh(g, heavy),
        _ => ico(g, 1, 10.0),
    };
    surface(t, &m);
    let nv = m.v.len() as i64;
    let nrip = if g.chance(4) && nv > 0 {
        g.int(1, 3)
    } else {
        0
    };
    ix(t, nrip);
    for _ in 0..nrip {
        ix(t, g.int(0, nv - 1));
    }
    ix(t, if g.chance(4) { g.int(1, 3) } else { 0 });
    let vals = !g.chance(5);
    ix(t, i64::from(vals));
    if vals {
        for _ in 0..nv {
            let x = if g.chance(3) {
                0.0
            } else if heavy && g.chance(8) {
                f32::from_bits(smallish(g, true))
            } else {
                g.int(-40, 40) as f32 / 8.0
            };
            fv(t, x);
        }
    }
    let nops = g.int(1, 4);
    ix(t, nops);
    for _ in 0..nops {
        match g.below(7) {
            6 => t.push("sm".into()),
            0 | 1 => {
                t.push("wv".into());
                hx(t, g.pick(&NAMES).as_bytes());
            }
            2 => {
                t.push("ae".into());
                hx(t, g.pick(&NAMES).as_bytes());
            }
            _ => {
                t.push("cm".into());
                ix(t, g.pick(&[0, 0, 0, 1, 2]));
                hx(t, g.pick(&FIELDS).as_bytes());
                let given = g.chance(3);
                ix(t, i64::from(given));
                if given {
                    let (w, h, d) = if g.chance(4) {
                        (nv + 1, 1, 1)
                    } else if nv % 2 == 0 && g.chance(2) {
                        (nv / 2, 2, 1)
                    } else {
                        (nv, 1, 1)
                    };
                    for x in [w, h, d, g.pick(&[1, 2])] {
                        ix(t, x);
                    }
                }
            }
        }
    }
}

fn dump_vol(g: &mut Globals, err: &mut dyn Write, m: &Mri, o: &mut Out) {
    o.i(i64::from(m.nframes));
    for f in 0..m.nframes {
        for s in 0..m.vg.depth {
            for r in 0..m.vg.height {
                for c in 0..m.vg.width {
                    o.f(mri_get_vox_val(g, err, m, c, r, s, f, 0).unwrap_or(0.0));
                }
            }
        }
    }
}

pub(super) fn wvl_case(cx: &mut Ctx, t: &mut Toks, o: &mut Out) -> Result<(), Fatal> {
    let td = tempfile::tempdir().expect("a work folder");
    let dir = td.path();
    std::fs::create_dir(dir.join("d")).expect("the folder d");
    cx.g.progname = "fssurfio".into();
    let bytes = t.h();
    let (nv, nf) = (t.i(), t.i());
    let rips: Vec<i32> = (0..t.i()).map(|_| t.i()).collect();
    let gd = t.i();
    let env = Environment::from_pairs(vec![("SURFER_SIDEDOOR".to_owned(), "1".to_owned())]);
    std::fs::write(dir.join("d/lh.s0"), bytes).expect("the surface file");
    let d = Some(dir);
    let (mut so, mut se) = (cx.a.clone(), cx.b.clone());
    let mut io = coords::Io {
        out: &mut so,
        err: &mut se,
        env: &env,
    };
    let g = &mut cx.g;
    let sg = &mut cx.sg;
    g.errno = 0;
    let Some(mut s) = readwrite::read(g, sg, &mut io, d, "d/lh.s0", true)? else {
        o.s("NULL");
        return Ok(());
    };
    if nv != s.nvertices || nf != s.nfaces {
        o.i(i64::from(s.nvertices));
        o.i(i64::from(s.nfaces));
        o.s("COUNTS");
        return Ok(());
    }
    for vno in rips {
        s.vertex_mut(vno as usize).ripflag = 1;
    }
    if gd & 1 != 0 {
        g.gdiag |= DIAG_SHOW;
    }
    if gd & 2 != 0 {
        g.gdiag |= DIAG_VERBOSE;
    }
    if t.i() != 0 {
        for vno in 0..nv as usize {
            s.vertex_mut(vno).val = t.f();
        }
    }
    let nops = t.i();
    for _ in 0..nops {
        let op = t.next();
        o.s(op);
        match op {
            "wv" | "ae" => {
                let name = String::from_utf8(t.h()).expect("a name");
                let r = if op == "wv" {
                    valfile::write_values(g, &mut io, d, &s, &name)?
                } else {
                    valfile::write_area_error_to_val_file(g, &mut io, d, &mut s, &name)?
                };
                o.i(i64::from(r));
            }
            "sm" => o.i(i64::from(
                larmorx_freesurfer_surf::metric::store_metric_properties(&mut s),
            )),
            "cm" => {
                let frame = t.i();
                let field = String::from_utf8(t.h()).expect("a field");
                let mut given = if t.i() != 0 {
                    let (w, h, dd, nfr) = (t.i(), t.i(), t.i(), t.i());
                    Some(mri_alloc_sequence(
                        g, io.out, io.err, w, h, dd, MRI_FLOAT, nfr,
                    )?)
                } else {
                    None
                };
                match surfmri::copy_mris(g, &mut io, given.as_mut(), &s, frame, &field)? {
                    Ret::New(m) => dump_vol(g, io.err, &m, o),
                    Ret::Given => dump_vol(g, io.err, given.as_ref().expect("given"), o),
                    Ret::Null => o.s("NULL"),
                }
            }
            _ => panic!("wvl op {op}"),
        }
    }
    o.0 += "| ";
    for vno in 0..nv as usize {
        o.f(s.vertex(vno).val);
    }
    let mut files = Vec::new();
    list_files(dir, "", &mut files);
    files.sort();
    for n in files {
        o.h(Some(n.as_bytes()));
        match std::fs::read(dir.join(&n)) {
            Ok(b) => {
                let mut x = String::from("x");
                for c in b {
                    x += &format!("{c:02x}");
                }
                o.s(&x);
            }
            Err(_) => o.s("NOFILE"),
        }
    }
    Ok(())
}
