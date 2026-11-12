// SPDX-License-Identifier: Apache-2.0
//! `larmorx_io::xfm` against MINC 1.5.1's volume_io and FreeSurfer 8.2.0's copy.

use std::cell::RefCell;
use std::path::Path;

use larmorx_core::cfmt::{Captured, Stream};
use larmorx_core::defects::{self as d, Notices};
use larmorx_io::xfm::{self, Dialect, GeneralTransform, Transform, TransformData};

use crate::dialect::{ProbeFs, ProbeMni};
use crate::generate::Gen;
use crate::{Capture, Lib, Tally, ffi::cstr};

/// The shims' dump of a transform (`shim_common.h`), made from the port's.
pub fn dump(out: &mut Vec<u8>, t: &GeneralTransform) {
    let put_i = |o: &mut Vec<u8>, v: i32| o.extend_from_slice(&v.to_le_bytes());
    let put_t = |o: &mut Vec<u8>, m: &Transform| {
        for row in m {
            for v in row {
                o.extend_from_slice(&v.to_le_bytes());
            }
        }
    };
    put_i(out, t.transform_type() as i32);
    put_i(out, t.inverse_flag as i32);
    match &t.data {
        TransformData::Linear { forward, inverse } => {
            put_t(out, forward);
            put_t(out, inverse);
        }
        TransformData::ThinPlateSpline { n_dimensions, points, displacements } => {
            put_i(out, *n_dimensions as i32);
            put_i(out, points.len() as i32);
            for r in points.iter().chain(displacements) {
                for v in r {
                    out.extend_from_slice(&v.to_le_bytes());
                }
            }
        }
        TransformData::Concatenated(v) => {
            put_i(out, v.len() as i32);
            for s in v {
                dump(out, s);
            }
        }
        TransformData::Grid { .. } => {}
    }
}

/// What a C call gave: status, dump, stdout, stderr.
type Got = (i32, Vec<u8>, String, String);

fn c_read(lib: &Lib, cap: &mut Capture, path: &Path) -> Got {
    let f = lib.input_transform_file.expect("fm_input_transform_file");
    let mut buf = vec![0u8; 1 << 20];
    let mut len = 0;
    let p = cstr(path.as_os_str().as_encoded_bytes());
    // SAFETY: a NUL-terminated name and a buffer of the stated size.
    let (st, out, err) = cap.run(|| unsafe { f(p.as_ptr(), buf.as_mut_ptr(), buf.len() as i32, &mut len) });
    buf.truncate(if st == 0 { len as usize } else { 0 });
    (st, buf, out, err)
}

fn port_read(path: &Path, dialect: &dyn Dialect) -> (Got, Notices) {
    let mut c = Captured::default();
    let mut n = Notices::default();
    let r = xfm::input_transform_file(path.as_os_str().as_encoded_bytes(), dialect, &mut c, &mut n);
    let mut buf = Vec::new();
    let st = match &r {
        Ok(t) => {
            dump(&mut buf, t);
            0
        }
        Err(()) => 1,
    };
    ((st, buf, c.text(Stream::Out), c.text(Stream::Err)), n)
}

/// The defects whose original crashes or never returns: the probe does not call it there.
fn original_dies(n: &Notices) -> bool {
    n.contains(&d::FS_72) || n.contains(&d::FS_73) || n.contains(&d::FS_74)
}

fn compare(t: &mut Tally, what: &dyn Fn() -> String, ours: &Got, theirs: &Got) {
    t.eq(&|| format!("{} status", what()), ours.0, theirs.0);
    t.eq(&|| format!("{} transform", what()), &ours.1, &theirs.1);
    t.eq(&|| format!("{} stdout", what()), &ours.2, &theirs.2);
    t.eq(&|| format!("{} stderr", what()), &ours.3, &theirs.3);
}

pub fn check_read(up: &Lib, fs: &Lib, work: &Path, scale: u64, cap: &mut Capture) -> Vec<Tally> {
    let mut t_up = Tally::new("xfm vs MINC");
    let mut t_fs = Tally::new("xfm vs FS");
    let mut g = Gen::new(0x0f0e_0001);
    let path = work.join("case.xfm");
    let mut dies = 0u64;
    let mut kinds = std::collections::BTreeMap::<String, u64>::new();
    for k in 0..20_000 * scale {
        let bytes = g.file();
        std::fs::write(&path, &bytes).unwrap();
        let shown = || format!("case {k} {:?}", String::from_utf8_lossy(&bytes));
        let (ours, notices) = port_read(&path, &ProbeMni);
        *kinds.entry(format!("status {}", ours.0)).or_default() += 1;
        t_up.cases += 1;
        if original_dies(&notices) {
            dies += 1;
        } else {
            let theirs = c_read(up, cap, &path);
            compare(&mut t_up, &shown, &ours, &theirs);
        }
        let probe_fs = ProbeFs { lib: fs, cap: RefCell::new(&mut *cap) };
        let (ours_fs, _) = port_read(&path, &probe_fs);
        drop(probe_fs);
        let theirs_fs = c_read(fs, cap, &path);
        t_fs.cases += 1;
        compare(&mut t_fs, &shown, &ours_fs, &theirs_fs);
    }
    t_up.notes.push(format!("{dies} cases where MINC 1.5.1 crashes or never returns (FS-72..74), not run there"));
    t_up.notes.push(format!("{kinds:?}"));
    vec![t_up, t_fs]
}

/// Files for the operations: every name a file of `Gen::file`'s that reads.
fn readable(g: &mut Gen, work: &Path, name: &str, linear_only: bool) -> std::path::PathBuf {
    let p = work.join(name);
    loop {
        let b = g.file();
        let text = String::from_utf8_lossy(&b);
        if linear_only && !text.contains("Linear") {
            continue;
        }
        let mut c = Captured::default();
        let mut n = Notices::default();
        if let Ok(t) = xfm::input_transform_from_bytes(&b, None, &ProbeMni, &mut c, &mut n) {
            if linear_only && t.transform_type() != xfm::TransformType::Linear {
                continue;
            }
            if matches!(t.data, TransformData::Grid { .. }) || original_dies(&n) {
                continue;
            }
            std::fs::write(&p, &b).unwrap();
            return p;
        }
    }
}

pub fn check_ops(up: &Lib, fs: &Lib, work: &Path, scale: u64, cap: &mut Capture) -> Vec<Tally> {
    let mut t_inv = Tally::new("inverse");
    let mut t_fsinv = Tally::new("inverse FS");
    let mut t_mul = Tally::new("concat 4x4");
    let mut t_pt = Tally::new("points");
    let mut t_cat = Tally::new("concat files");
    let mut t_exp = Tally::new("expand name");
    let mut g = Gen::new(0x0f0e_0002);
    let mut fs_same = [0u64; 4];
    let mut fs_kind = [0u64; 4];
    let mut fs_singular = (0u64, 0u64);
    for k in 0..50_000 * scale {
        let mut m = [[0.0f64; 4]; 4];
        let kind = g.below(4);
        for i in 0..4 {
            for j in 0..4 {
                m[i][j] = match (kind, i) {
                    (0, 3) => [0.0, 0.0, 0.0, 1.0][j],
                    (1, _) => g.below(5) as f64 - 2.0,
                    _ => g.value(),
                };
            }
        }
        if kind == 2 && g.below(4) == 0 {
            m[2] = m[1];
        }
        let flat: Vec<f64> = m.iter().flatten().copied().collect();
        let ours = xfm::compute_transform_inverse(&m);
        for (lib, t) in [(up, &mut t_inv)] {
            let f = lib.compute_transform_inverse.unwrap();
            let mut inv = [0.0f64; 16];
            // SAFETY: two 16-double arrays.
            let (ok, _, _) = cap.run(|| unsafe { f(flat.as_ptr(), inv.as_mut_ptr()) });
            t.cases += 1;
            let ours_flat: Option<Vec<u64>> = ours.map(|o| o.iter().flatten().map(|v| v.to_bits()).collect());
            let theirs_flat: Option<Vec<u64>> = (ok != 0).then(|| inv.iter().map(|v| v.to_bits()).collect());
            t.eq(&|| format!("case {k} {m:?}"), ours_flat, theirs_flat);
        }
        // FreeSurfer's own 4 x 4 inverse: how often it gives MINC's bits (a measurement).
        {
            let f = fs.compute_transform_inverse.unwrap();
            let mut inv = [0.0f64; 16];
            // SAFETY: two 16-double arrays.
            let (ok, _, _) = cap.run(|| unsafe { f(flat.as_ptr(), inv.as_mut_ptr()) });
            t_fsinv.cases += 1;
            match (ours, ok != 0) {
                (Some(o), true) => {
                    fs_kind[kind as usize] += 1;
                    if o.iter().flatten().zip(inv.iter()).all(|(a, b)| a.to_bits() == b.to_bits()) {
                        fs_same[kind as usize] += 1;
                    }
                }
                (None, false) => fs_singular.0 += 1,
                _ => fs_singular.1 += 1,
            }
        }
        // concat_transforms and transform_point, both libraries
        let mut b = [[0.0f64; 4]; 4];
        for row in b.iter_mut() {
            for v in row.iter_mut() {
                *v = g.value();
            }
        }
        let bf: Vec<f64> = b.iter().flatten().copied().collect();
        let ours_c = xfm::concat_transforms(&m, &b);
        let (x, y, z) = (g.value(), g.value(), g.value());
        let ours_p = xfm::transform_point(&m, x, y, z);
        for lib in [up, fs] {
            let mut r = [0.0f64; 16];
            // SAFETY: three 16-double arrays.
            unsafe { (lib.concat_transforms.unwrap())(flat.as_ptr(), bf.as_ptr(), r.as_mut_ptr()) };
            t_mul.cases += 1;
            t_mul.eq(&|| format!("case {k}"), ours_c.iter().flatten().map(|v| v.to_bits()).collect::<Vec<_>>(), r.iter().map(|v| v.to_bits()).collect());
            let mut p = [0.0f64; 3];
            // SAFETY: a 16-double array and 3 doubles out.
            unsafe { (lib.transform_point.unwrap())(flat.as_ptr(), x, y, z, p.as_mut_ptr()) };
            t_pt.cases += 1;
            t_pt.eq(&|| format!("case {k}"), ours_p.map(f64::to_bits), p.map(f64::to_bits));
        }
    }
    t_fsinv.notes.push(format!(
        "FreeSurfer's inverse has MINC's bits (all 16) in {}/{} affine (last row 0 0 0 1), {}/{} small-integer, {}/{} and {}/{} general matrices; both singular {}, one singular {}",
        fs_same[0], fs_kind[0], fs_same[1], fs_kind[1], fs_same[2], fs_kind[2], fs_same[3], fs_kind[3], fs_singular.0, fs_singular.1
    ));
    // concat_general_transforms of two files (MINC: with inverse flags; FreeSurfer: linear only)
    for k in 0..4_000 * scale {
        let a = readable(&mut g, work, "a.xfm", false);
        let b = readable(&mut g, work, "b.xfm", false);
        let (ia, ib) = (g.below(2) as i32, g.below(2) as i32);
        let ours = {
            let mut c = Captured::default();
            let mut n = Notices::default();
            let mut ta = xfm::input_transform_file(a.as_os_str().as_encoded_bytes(), &ProbeMni, &mut c, &mut n).unwrap();
            let mut tb = xfm::input_transform_file(b.as_os_str().as_encoded_bytes(), &ProbeMni, &mut c, &mut n).unwrap();
            if ia == 1 {
                ta.invert_in_place();
            }
            if ib == 1 {
                tb.invert_in_place();
            }
            let mut out = Vec::new();
            dump(&mut out, &GeneralTransform::concat(&ta, &tb));
            out
        };
        let f = up.concat_files.unwrap();
        let mut buf = vec![0u8; 1 << 20];
        let mut len = 0;
        let (pa, pb) = (cstr(a.as_os_str().as_encoded_bytes()), cstr(b.as_os_str().as_encoded_bytes()));
        // SAFETY: NUL-terminated names and a buffer of the stated size.
        let (st, _, _) = cap.run(|| unsafe { f(pa.as_ptr(), pb.as_ptr(), ia, ib, buf.as_mut_ptr(), buf.len() as i32, &mut len) });
        buf.truncate(len as usize);
        t_cat.cases += 1;
        t_cat.eq(&|| format!("case {k} invert {ia} {ib}"), (0, ours), (st, buf));
    }
    // expand_filename, both libraries
    // SAFETY: setting variables before any thread reads the environment (single-threaded probe).
    unsafe {
        std::env::set_var("LXPROBE_A", "/dir/a");
        std::env::set_var("LXPROBE_B", "b$LXPROBE_A\\x");
        std::env::set_var("LXPROBE_E", "");
    }
    let mut exp_crashes = 0u64;
    const PARTS: &[&str] = &["~", "~root", "~nosuchuser_x", "$LXPROBE_A", "$LXPROBE_B", "$LXPROBE_E", "$NOPE", "$", "\\", "\\$", "\\\\", "/", ".", "x", "$LXPROBE_A.", "a.b", "~/"];
    for k in 0..20_000 * scale {
        let n = 1 + g.below(5);
        let mut name = String::new();
        for _ in 0..n {
            name.push_str(g.pick(PARTS));
        }
        let mut crashes = 0;
        for (lib, is_fs) in [(up, false), (fs, true)] {
            let mut n = Notices::default();
            let ours = if is_fs {
                let probe_fs = ProbeFs { lib: fs, cap: RefCell::new(&mut *cap) };
                xfm::expand_filename_in(name.as_bytes(), &probe_fs, &mut n)
            } else {
                xfm::expand_filename_in(name.as_bytes(), &ProbeMni, &mut n)
            };
            if !is_fs && n.contains(&d::FS_75) {
                // MINC 1.5.1 writes through a null pointer here.
                crashes += 1;
                continue;
            }
            let mut out = vec![0u8; 4096];
            let c = cstr(name.as_bytes());
            // SAFETY: a NUL-terminated name and an output buffer of the stated size.
            let len = unsafe { (lib.expand_filename.unwrap())(c.as_ptr(), out.as_mut_ptr().cast(), out.len() as i32) };
            t_exp.cases += 1;
            if len < 0 {
                t_exp.eq(&|| format!("case {k} {name:?}"), Some(&ours), None);
                continue;
            }
            out.truncate(len as usize);
            t_exp.eq(&|| format!("case {k} {name:?}"), &ours, &out);
        }
        exp_crashes += crashes;
    }
    t_exp.notes.push(format!("{exp_crashes} names where MINC 1.5.1 crashes (FS-75), not run there"));
    vec![t_inv, t_fsinv, t_mul, t_pt, t_cat, t_exp]
}

pub fn check_write(up: &Lib, work: &Path, scale: u64, cap: &mut Capture) -> Vec<Tally> {
    let mut t = Tally::new("write vs MINC");
    let mut t_pt = Tally::new("general point");
    let mut g = Gen::new(0x0f0e_0003);
    const COMMENTS: &[Option<&str>] = &[None, Some(""), Some("one line"), Some("two\nlines\n"), Some("a\n\nb"), Some("\n")];
    for k in 0..10_000 * scale {
        let a = readable(&mut g, work, "w-in.xfm", false);
        let comments = COMMENTS[g.below(COMMENTS.len() as u64) as usize];
        let out_name = if g.below(3) == 0 { work.join("w-out") } else { work.join("w-out.xfm") };
        let written = |p: &Path| std::fs::read(if p.extension().is_none() { p.with_extension("xfm") } else { p.to_path_buf() }).unwrap_or_default();
        let _ = std::fs::remove_file(work.join("w-out.xfm"));
        let ours = {
            let mut c = Captured::default();
            let mut n = Notices::default();
            let tr = xfm::input_transform_file(a.as_os_str().as_encoded_bytes(), &ProbeMni, &mut c, &mut n).unwrap();
            let st = xfm::output_transform_file(out_name.as_os_str().as_encoded_bytes(), comments.map(str::as_bytes), &tr, &ProbeMni, &mut c, &mut n);
            (st.is_err() as i32, written(&out_name), c.text(Stream::Err), n.contains(&d::FS_74))
        };
        let _ = std::fs::remove_file(work.join("w-out.xfm"));
        let f = up.rewrite.unwrap();
        let (pa, po) = (cstr(a.as_os_str().as_encoded_bytes()), cstr(out_name.as_os_str().as_encoded_bytes()));
        let pc = comments.map(|c| cstr(c.as_bytes()));
        // SAFETY: NUL-terminated names (comments NULL or NUL-terminated).
        let (st, _, err) = cap.run(|| unsafe { f(pa.as_ptr(), po.as_ptr(), pc.as_ref().map_or(std::ptr::null(), |c| c.as_ptr())) });
        let theirs = (st, written(&out_name), err);
        t.cases += 1;
        if ours.3 {
            // FS-74: an empty Points list, written with its ';' by larmorx, without by MINC.
            continue;
        }
        t.eq(&|| format!("case {k} {:?}", comments), (ours.0, ours.1, ours.2), theirs);
        // general_transform_point (and its inverse) of a linear or concatenated transform
        let (x, y, z) = (g.value(), g.value(), g.value());
        let inverse = g.below(2) as i32;
        let mut c = Captured::default();
        let mut n = Notices::default();
        let tr = xfm::input_transform_file(a.as_os_str().as_encoded_bytes(), &ProbeMni, &mut c, &mut n).unwrap();
        if let Ok(p) = xfm::general_transform_point(&tr, inverse == 1, x, y, z) {
            let mut q = [0.0f64; 3];
            // SAFETY: a NUL-terminated name and 3 doubles out.
            let (st, _, _) = cap.run(|| unsafe { (up.general_point.unwrap())(pa.as_ptr(), inverse, x, y, z, q.as_mut_ptr()) });
            t_pt.cases += 1;
            t_pt.eq(&|| format!("case {k}"), (0, p.map(f64::to_bits)), (st, q.map(f64::to_bits)));
        }
    }
    vec![t, t_pt]
}
