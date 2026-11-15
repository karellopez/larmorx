// SPDX-License-Identifier: Apache-2.0
//! MINC 1 volume output: volume_io's volume set up and written as FreeSurfer 8.2.0's mincWrite
//! does it (`create_volume`, `set_volume_sizes`, `alloc_volume_data`, `set_volume_separations`,
//! `set_volume_direction_cosine`, `set_volume_translation`, `set_volume_voxel_value`,
//! `output_volume`), with every argument drawn by the probe, by the two originals
//! (`shim_minc.c`'s `fm_minc_write`) and by the port (`larmorx_io::minc`). The written files are
//! compared byte for byte, with the dumps of each step, standard output, standard error and the
//! exit status.
//!
//! A case is either mincWrite's shape (the names `xspace, yspace, zspace[, time]`, its four
//! types, the same type and range for the file as for the volume, an empty history), compared
//! with both originals (FreeSurfer's copy as measured: `FreeSurferDialect`), or any other call,
//! compared with MINC 1.5.1 only.

use std::ffi::CString;
use std::path::Path;

use larmorx_core::cfmt::Streams;
use larmorx_core::defects::{Notices, UpstreamDefects};
use larmorx_io::minc::{self, Ctx, Dialect, Exit, Session, Volume};

use crate::Rng;
use crate::ffi::{MincLib, Outcome, WriteCaseC};
use crate::minc::FreeSurferDialect;

/// A write case.
#[derive(Clone, Debug)]
pub struct Case {
    pub n_dims: i32,
    /// `None`: `create_volume`'s names are `NULL`; an entry `None` is a `NULL` name.
    pub names: Option<Vec<Option<Vec<u8>>>>,
    pub nc_type: i32,
    pub signed: bool,
    pub vmin: f64,
    pub vmax: f64,
    pub real: Option<(f64, f64)>,
    pub sizes: [i32; 5],
    pub alloc: bool,
    pub sep: [f64; 5],
    pub cos_unit: bool,
    pub cos: Vec<(i32, [f64; 3])>,
    pub translation: Option<([f64; 5], [f64; 3])>,
    pub values: Vec<f64>,
    pub fname: Vec<u8>,
    pub out_type: i32,
    pub out_signed: bool,
    pub out_min: f64,
    pub out_max: f64,
    pub history: Option<Vec<u8>>,
    pub space: Option<Vec<u8>>,
    /// mincWrite's shape: compared with FreeSurfer's copy too.
    pub fs_shaped: bool,
}

/// What a run gave.
#[derive(Clone, Debug, PartialEq)]
pub struct Run {
    pub dump: String,
    /// `None`: the sequence returned; else the exit status.
    pub exit: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    /// The file's bytes after the run (`None`: no file).
    pub file: Option<Vec<u8>>,
    pub notices: Vec<String>,
}

#[derive(Default)]
struct Cap {
    out: Vec<u8>,
    err: Vec<u8>,
}

impl Streams for Cap {
    fn out(&mut self, t: &str) {
        self.out.extend_from_slice(t.as_bytes());
    }
    fn err(&mut self, t: &str) {
        self.err.extend_from_slice(t.as_bytes());
    }
    fn out_bytes(&mut self, b: &[u8]) {
        self.out.extend_from_slice(b);
    }
    fn err_bytes(&mut self, b: &[u8]) {
        self.err.extend_from_slice(b);
    }
}

fn read_file(name: &[u8]) -> Option<Vec<u8>> {
    let p = Path::new(std::str::from_utf8(name).ok()?);
    if p.is_file() {
        std::fs::read(p).ok()
    } else {
        None
    }
}

/// The case with the port.
pub fn port(case: &Case, mode: UpstreamDefects, dialect: &dyn Dialect) -> Run {
    let mut cap = Cap::default();
    let mut notices = Notices::new(mode);
    let mut s = Session::new();
    let mut dump = String::new();
    let r = {
        let mut c = Ctx {
            streams: &mut cap,
            notices: &mut notices,
            dialect,
        };
        run_port(&mut s, &mut c, case, &mut dump)
    };
    drop(s);
    Run {
        dump,
        exit: r.err().map(|e| e.status),
        stdout: cap.out,
        stderr: cap.err,
        file: read_file(&case.fname),
        notices: notices
            .as_slice()
            .iter()
            .map(|n| n.defect.id.to_string())
            .collect(),
    }
}

fn run_port(s: &mut Session, c: &mut Ctx, w: &Case, dump: &mut String) -> Result<(), Exit> {
    let names: Option<Vec<Vec<u8>>> = w
        .names
        .as_ref()
        .map(|v| v.iter().map(|n| n.clone().unwrap_or_default()).collect());
    let vol = Volume::create_volume(
        c,
        w.n_dims,
        names.as_deref(),
        w.nc_type,
        w.signed,
        w.vmin,
        w.vmax,
    )?;
    dump.push_str(&format!("created {}\n", i32::from(vol.is_some())));
    let Some(mut vol) = vol else {
        return Ok(());
    };
    if let Some(sp) = &w.space {
        vol.set_space_type(sp);
    }
    if let Some((a, b)) = w.real {
        vol.set_real_range(a, b);
    }
    vol.set_sizes(&w.sizes);
    if w.alloc {
        vol.alloc_volume_data(c);
    }
    dump.push_str("allocated\n");
    vol.set_separations(&w.sep);
    for (axis, d) in &w.cos {
        if w.cos_unit {
            vol.set_direction_unit_cosine(*axis, d, c.dialect, c.streams);
        } else {
            vol.set_direction_cosine(*axis, d, c.dialect, c.streams)?;
        }
    }
    dump.push_str("cosines\n");
    if let Some((v, wd)) = &w.translation {
        vol.set_translation(v, wd, c.dialect, c.streams);
    }
    dump.push_str("translated\n");
    if w.alloc && !w.values.is_empty() {
        let n = (w.n_dims.clamp(0, 5)) as usize;
        let total: i64 = w.sizes[..n].iter().map(|&x| x as i64).product();
        if w.n_dims >= 1 && total > 0 {
            let mut idx = [0usize; 5];
            let mut k = 0usize;
            loop {
                vol.set_voxel_value(&idx, w.values[k % w.values.len()]);
                k += 1;
                let mut i = n as i64 - 1;
                while i >= 0 {
                    let iu = i as usize;
                    idx[iu] += 1;
                    if (idx[iu] as i64) < w.sizes[iu] as i64 {
                        break;
                    }
                    idx[iu] = 0;
                    i -= 1;
                }
                if i < 0 {
                    break;
                }
            }
        }
    }
    dump.push_str("set\n");
    let status = minc::output_volume(
        s,
        c,
        &w.fname,
        w.out_type,
        w.out_signed,
        w.out_min,
        w.out_max,
        &mut vol,
        w.history.as_deref(),
        None,
    )?;
    dump.push_str(&format!("status {}\n", status as i32));
    Ok(())
}

/// The case with an original.
pub fn original(case: &Case, lib: &MincLib) -> (Outcome, Run) {
    let names_c: Vec<Option<CString>> = (0..5)
        .map(|i| {
            case.names
                .as_ref()
                .and_then(|v| v.get(i).cloned().flatten())
                .map(|n| CString::new(n).unwrap())
        })
        .collect();
    let fname = CString::new(case.fname.clone()).unwrap();
    let hist = case.history.clone().map(|h| CString::new(h).unwrap());
    let space = case.space.clone().map(|h| CString::new(h).unwrap());
    let mut w = WriteCaseC {
        n_dims: case.n_dims,
        names_null: i32::from(case.names.is_none()),
        names: [std::ptr::null(); 5],
        nc_type: case.nc_type,
        signed_flag: i32::from(case.signed),
        vmin: case.vmin,
        vmax: case.vmax,
        set_real: i32::from(case.real.is_some()),
        rmin: case.real.map_or(0.0, |r| r.0),
        rmax: case.real.map_or(0.0, |r| r.1),
        sizes: case.sizes,
        do_alloc: i32::from(case.alloc),
        sep: case.sep,
        n_cos: case.cos.len() as i32,
        cos_unit: i32::from(case.cos_unit),
        cos_axis: [0; 5],
        cos: [[0.0; 3]; 5],
        do_translation: i32::from(case.translation.is_some()),
        tvoxel: case.translation.map_or([0.0; 5], |t| t.0),
        tworld: case.translation.map_or([0.0; 3], |t| t.1),
        n_values: case.values.len() as std::ffi::c_long,
        values: case.values.as_ptr(),
        fname: fname.as_ptr(),
        out_type: case.out_type,
        out_signed: i32::from(case.out_signed),
        out_min: case.out_min,
        out_max: case.out_max,
        history: hist.as_ref().map_or(std::ptr::null(), |h| h.as_ptr()),
        set_space: i32::from(case.space.is_some()),
        space: space.as_ref().map_or(std::ptr::null(), |h| h.as_ptr()),
    };
    for (i, n) in names_c.iter().enumerate() {
        w.names[i] = n.as_ref().map_or(std::ptr::null(), |c| c.as_ptr());
    }
    for (i, (a, d)) in case.cos.iter().enumerate().take(5) {
        w.cos_axis[i] = *a;
        w.cos[i] = *d;
    }
    let (oc, dump, out, err) = lib.run_write(&w, 20000);
    let exit = match &oc {
        Outcome::Returned(_) => None,
        Outcome::Exit(s) => Some(*s),
        Outcome::Signal(s) => Some(1000 + *s),
        Outcome::Hang => Some(2000),
    };
    let run = Run {
        dump,
        exit,
        stdout: out,
        stderr: err,
        file: read_file(&case.fname),
        notices: Vec::new(),
    };
    (oc, run)
}

fn pick<'a, T>(r: &mut Rng, v: &'a [T]) -> &'a T {
    &v[r.below(v.len() as u64) as usize]
}

fn chance(r: &mut Rng, p: u64) -> bool {
    r.below(100) < p
}

fn rand_f(r: &mut Rng, lo: f64, hi: f64) -> f64 {
    lo + (hi - lo) * (r.below(1 << 30) as f64 / (1u64 << 30) as f64)
}

const NC_BYTE: i32 = 1;
const NC_SHORT: i32 = 3;
const NC_INT: i32 = 4;
const NC_FLOAT: i32 = 5;
const NC_DOUBLE: i32 = 6;

fn odd_value(r: &mut Rng) -> f64 {
    *pick(
        r,
        &[
            f64::NAN,
            -f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            0.0,
            -0.0,
            1e300,
            -1e300,
            3.4e38,
            4e38,
            -4e38,
            2_147_483_648.0,
            -2_147_483_649.0,
            4_294_967_296.0,
            65536.5,
            -32768.5,
            255.5,
            256.0,
            -1.0,
            -128.5,
            0.49999999999999994,
            1e-310,
            f64::from_bits(0x7ff0_0000_0000_0001),
            f64::from_bits(0x7ff8_dead_beef_0000),
        ],
    )
}

/// A voxel value for a volume of `ty` with the range `lo..hi`.
fn value(r: &mut Rng, ty: i32, lo: f64, hi: f64) -> f64 {
    match r.below(10) {
        0 => odd_value(r),
        1 => lo,
        2 => hi,
        3..=5 if ty == NC_FLOAT || ty == NC_DOUBLE => {
            let (a, b) = if lo < hi && (hi - lo).is_finite() {
                (lo, hi)
            } else {
                (-1000.0, 1000.0)
            };
            let v = rand_f(r, a, b);
            if ty == NC_FLOAT { v as f32 as f64 } else { v }
        }
        _ => {
            let (a, b) = if lo < hi && (hi - lo).is_finite() {
                (lo, hi)
            } else {
                (0.0, 255.0)
            };
            let v = rand_f(r, a, b);
            if chance(r, 70) { v.round() } else { v }
        }
    }
}

fn range(r: &mut Rng, ty: i32) -> (f64, f64) {
    let base: (f64, f64) = match ty {
        NC_BYTE => (0.0, 255.0),
        NC_SHORT => (-32768.0, 32767.0),
        NC_INT => (-2_147_483_648.0, 2_147_483_647.0),
        _ => (-1000.0, 1000.0),
    };
    match r.below(12) {
        0 => (base.0, base.0),
        1 => (base.1, base.0),
        2 => (f64::NAN, base.1),
        3 => (base.0, f64::NAN),
        4 => (odd_value(r), odd_value(r)),
        5 => (0.0, 0.0),
        6 => {
            let a = rand_f(r, -50.0, 50.0).round();
            (a, a)
        }
        _ => {
            let a = rand_f(r, base.0.max(-5000.0), base.1.min(5000.0)).round();
            let b = rand_f(r, a, base.1.min(a + 20000.0)).round();
            if ty == NC_FLOAT {
                (a + 0.25, b + 0.5)
            } else {
                (a, b)
            }
        }
    }
}

fn sep(r: &mut Rng) -> f64 {
    *pick(
        r,
        &[
            1.0,
            1.0,
            0.5,
            2.0,
            -1.0,
            -1.5,
            0.9375,
            3.0,
            0.0,
            f64::NAN,
            f64::INFINITY,
            1e-300,
            1e300,
            1.2,
        ],
    )
}

fn cosine(r: &mut Rng, axis: usize) -> [f64; 3] {
    match r.below(10) {
        0 => [0.0, 0.0, 0.0],
        1 => [f64::NAN, 0.0, 1.0],
        2 => [
            rand_f(r, -2.0, 2.0),
            rand_f(r, -2.0, 2.0),
            rand_f(r, -2.0, 2.0),
        ],
        3 => {
            let mut d = [0.0; 3];
            d[axis] = -1.0;
            d
        }
        4 => {
            let mut d = [0.0; 3];
            d[(axis + 1) % 3] = 1.0;
            d
        }
        5 => [1e300, 1e300, 0.0],
        6 => [0.6, 0.8, 0.0],
        _ => {
            let mut d = [0.0; 3];
            d[axis] = 1.0;
            let k = (axis + 1 + r.below(2) as usize) % 3;
            d[k] = rand_f(r, -0.3, 0.3);
            d
        }
    }
}

fn fname(r: &mut Rng, dir: &Path) -> Vec<u8> {
    let d = dir.to_string_lossy().into_owned();
    let n = match r.below(30) {
        0 => format!("{d}/missing/x.mnc"),
        1 => {
            std::fs::create_dir_all(dir.join("adir")).unwrap();
            format!("{d}/adir")
        }
        2 => {
            std::fs::write(dir.join("afile"), b"x").unwrap();
            format!("{d}/afile/x.mnc")
        }
        3 => String::new(),
        4 => format!("{d}/w.mnc.gz"),
        5 => "$LXPROBE_W/w.mnc".to_string(),
        6 => {
            std::fs::write(
                dir.join("w.mnc"),
                b"an older file that is longer than the header",
            )
            .unwrap();
            format!("{d}/w.mnc")
        }
        7 => format!("{d}/name with blanks.mnc"),
        _ => format!("{d}/w.mnc"),
    };
    n.into_bytes()
}

fn sizes(r: &mut Rng, n: usize) -> [i32; 5] {
    let mut s = [1i32; 5];
    let big = r.below(12);
    for x in s.iter_mut().take(n) {
        *x = match big {
            0 => 1 + r.below(60) as i32,
            1 => 20 + r.below(30) as i32,
            _ => 1 + r.below(6) as i32,
        };
    }
    if chance(r, 3) {
        s[r.below(n as u64) as usize] = 0;
    }
    // At most about 300,000 voxels.
    while s[..n].iter().map(|&x| x.max(1) as i64).product::<i64>() > 300_000 {
        let i = r.below(n as u64) as usize;
        s[i] = (s[i] / 2).max(1);
    }
    s
}

/// A case: mincWrite's shape (`fs`) or any call.
pub fn generate(r: &mut Rng, dir: &Path) -> Case {
    let fs = chance(r, 60);
    let (n_dims, names, nc_type, signed): (i32, Option<Vec<Option<Vec<u8>>>>, i32, bool) = if fs {
        let n = if chance(r, 70) { 3 } else { 4 };
        let mut v: Vec<Option<Vec<u8>>> = vec![
            Some(b"xspace".to_vec()),
            Some(b"yspace".to_vec()),
            Some(b"zspace".to_vec()),
        ];
        if n == 4 {
            v.push(Some(b"time".to_vec()));
        }
        let (t, sg) = *pick(
            r,
            &[
                (NC_BYTE, false),
                (NC_SHORT, true),
                (NC_INT, true),
                (NC_FLOAT, true),
            ],
        );
        (n, Some(v), t, sg)
    } else {
        let n = *pick(r, &[1, 2, 3, 3, 3, 4, 4, 5, 0, 6]);
        let pool: &[&[u8]] = &[
            b"xspace",
            b"yspace",
            b"zspace",
            b"time",
            b"vector_dimension",
            b"foo",
            b"xspace",
            b"",
        ];
        let names = if chance(r, 10) {
            None
        } else {
            let mut v: Vec<Option<Vec<u8>>> = Vec::new();
            let order = r.below(4);
            for i in 0..n.clamp(0, 5) as usize {
                let nm: Option<Vec<u8>> = match order {
                    0 => Some(
                        [
                            b"zspace".as_slice(),
                            b"yspace",
                            b"xspace",
                            b"time",
                            b"vector_dimension",
                        ][i]
                            .to_vec(),
                    ),
                    1 => Some(
                        [
                            b"xspace".as_slice(),
                            b"yspace",
                            b"zspace",
                            b"time",
                            b"vector_dimension",
                        ][i]
                            .to_vec(),
                    ),
                    2 => Some(
                        [
                            b"time".as_slice(),
                            b"zspace",
                            b"yspace",
                            b"xspace",
                            b"vector_dimension",
                        ][i]
                            .to_vec(),
                    ),
                    _ => {
                        if chance(r, 5) {
                            None
                        } else {
                            Some(pick(r, pool).to_vec())
                        }
                    }
                };
                v.push(nm);
            }
            Some(v)
        };
        let t = *pick(
            r,
            &[
                NC_BYTE, NC_SHORT, NC_INT, NC_FLOAT, NC_DOUBLE, NC_BYTE, NC_SHORT, 0, 2,
            ],
        );
        (n, names, t, chance(r, 50))
    };
    let (vmin, vmax) = range(r, nc_type);
    let nn = n_dims.clamp(0, 5) as usize;
    let sz = sizes(r, nn.max(1));
    let mut sepv = [1.0f64; 5];
    for x in sepv.iter_mut().take(nn) {
        *x = sep(r);
    }
    if fs {
        sepv[3] = 1.0;
    }
    let mut cos = Vec::new();
    let ncos = if fs { 3 } else { r.below(5) as usize };
    for k in 0..ncos {
        let axis = if fs || chance(r, 80) {
            (k % 3) as i32
        } else {
            *pick(r, &[-1, 3, 4, 5, 0])
        };
        cos.push((axis, cosine(r, axis.clamp(0, 2) as usize)));
    }
    let translation = if fs || chance(r, 80) {
        let mut v = [0.0; 5];
        for (i, x) in v.iter_mut().enumerate().take(3) {
            *x = sz[i] as f64 / 2.0;
        }
        if !fs && chance(r, 30) {
            v[r.below(5) as usize] = rand_f(r, -3.0, 9.0);
        }
        let mut wd = [0.0; 3];
        for x in wd.iter_mut() {
            *x = if chance(r, 5) {
                odd_value(r)
            } else {
                (rand_f(r, -128.0, 128.0) as f32) as f64
            };
        }
        Some((v, wd))
    } else {
        None
    };
    let nvals = 1 + r.below(40) as usize;
    let values: Vec<f64> = (0..nvals).map(|_| value(r, nc_type, vmin, vmax)).collect();
    let (out_type, out_signed, out_min, out_max) = if fs || chance(r, 50) {
        (nc_type, signed, vmin, vmax)
    } else {
        let t = *pick(r, &[NC_BYTE, NC_SHORT, NC_INT, NC_FLOAT, NC_DOUBLE, 0]);
        let (a, b) = range(r, t.max(1));
        (t, chance(r, 50), a, b)
    };
    let history = if fs {
        Some(Vec::new())
    } else {
        match r.below(4) {
            0 => None,
            1 => Some(b"lx probe history\n".to_vec()),
            _ => Some(Vec::new()),
        }
    };
    let space = if !fs && chance(r, 20) {
        Some(
            pick(
                r,
                &[b"talairach_".as_slice(), b"native____", b"unknown___", b""],
            )
            .to_vec(),
        )
    } else {
        None
    };
    let real = if !fs && chance(r, 20) {
        Some(range(r, NC_DOUBLE))
    } else {
        None
    };
    Case {
        n_dims,
        names,
        nc_type,
        signed,
        vmin,
        vmax,
        real,
        sizes: sz,
        alloc: fs || chance(r, 90),
        sep: sepv,
        cos_unit: !fs && chance(r, 20),
        cos,
        translation,
        values,
        fname: fname(r, dir),
        out_type,
        out_signed,
        out_min,
        out_max,
        history,
        space,
        fs_shaped: fs,
    }
}

/// Defects of undefined behaviour: a case where the port notes one is counted, not compared.
const UNDEFINED: &[&str] = &["FS-781", "FS-789", "FS-798", "FS-799", "FS-800", "FS-801"];

#[derive(Default)]
pub struct Tally {
    pub cases: u64,
    pub same: u64,
    pub written: u64,
    pub differ: u64,
    pub skipped: u64,
    pub shown: u32,
    pub outcomes: std::collections::BTreeMap<String, u64>,
    pub notices: std::collections::BTreeMap<String, u64>,
}

fn reset(dir: &Path, case: &Case) {
    // Every run starts from the case's own setting: the written file removed, the setting's
    // other files rebuilt.
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir).unwrap();
    let n = String::from_utf8_lossy(&case.fname).into_owned();
    if n.ends_with("/adir") {
        std::fs::create_dir_all(dir.join("adir")).unwrap();
    } else if n.contains("/afile/") {
        std::fs::write(dir.join("afile"), b"x").unwrap();
    }
}

fn show_run(tag: &str, x: &Run) {
    println!(
        "    {tag}: exit {:?} notices {:?}\n      stdout {:?}\n      stderr {:?}\n      dump {:?}\n      file {}",
        x.exit,
        x.notices,
        String::from_utf8_lossy(&x.stdout),
        String::from_utf8_lossy(&x.stderr),
        x.dump,
        match &x.file {
            None => "none".to_string(),
            Some(b) => format!("{} bytes", b.len()),
        }
    );
}

pub fn run(seed: u64, count: u64, work: &Path, up: &MincLib, fs: &MincLib, t: &mut Tally) {
    let mut r = Rng::new(seed ^ 0x7772_6974);
    let show: u32 = std::env::var("LXPROBE_SHOW")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10);
    let fsd = FreeSurferDialect {
        prefix: crate::minc::fs_prefix(fs, work),
    };
    for case_no in 0..count {
        let dir = work.join(format!("w{case_no}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // SAFETY: the probe is single-threaded here; the children inherit the variable.
        unsafe { std::env::set_var("LXPROBE_W", &dir) };
        let case = generate(&mut r, &dir);
        let older = std::fs::read(dir.join("w.mnc")).ok();
        let setup = |dir: &Path| {
            reset(dir, &case);
            if let Some(o) = &older {
                std::fs::write(dir.join("w.mnc"), o).unwrap();
            }
        };
        t.cases += 1;
        let mut keep = false;
        let sides: Vec<(&MincLib, &str, &dyn Dialect)> = if case.fs_shaped {
            vec![(up, "up", &minc::Mni), (fs, "fs", &fsd)]
        } else {
            vec![(up, "up", &minc::Mni)]
        };
        for (lib, side, dialect) in sides {
            setup(&dir);
            let pf = port(&case, UpstreamDefects::Fix, dialect);
            for id in &pf.notices {
                *t.notices.entry(format!("{side} {id}")).or_default() += 1;
            }
            setup(&dir);
            crate::minc::take_not_modelled();
            let p = port(&case, UpstreamDefects::Reproduce, dialect);
            let not_modelled = crate::minc::take_not_modelled();
            setup(&dir);
            let (oc, o) = original(&case, lib);
            let label = match &oc {
                Outcome::Returned(_) => "returned".to_string(),
                Outcome::Exit(s) => format!("exit {s}"),
                Outcome::Signal(s) => format!("signal {s}"),
                Outcome::Hang => "hang".to_string(),
            };
            let undefined: Vec<String> = p
                .notices
                .iter()
                .filter(|n| UNDEFINED.contains(&n.as_str()))
                .cloned()
                .collect();
            if !undefined.is_empty() {
                *t.outcomes
                    .entry(format!("{side} {} -> {label}", undefined.join("+")))
                    .or_default() += 1;
                t.skipped += 1;
                continue;
            }
            if not_modelled {
                *t.outcomes
                    .entry(format!("{side} a non-finite voxel-to-world matrix (FreeSurfer's inverse not modelled by the probe) -> {label}"))
                    .or_default() += 1;
                t.skipped += 1;
                continue;
            }
            let same = p.dump == o.dump
                && p.exit == o.exit
                && p.stdout == o.stdout
                && p.stderr == o.stderr
                && p.file == o.file;
            if same {
                t.same += 1;
                if p.dump.ends_with("status 0\n") {
                    t.written += 1;
                }
            } else {
                t.differ += 1;
                if t.shown < show {
                    t.shown += 1;
                    keep = true;
                    println!("  DIFF minc-write seed {seed} case {case_no} ({side}): {case:?}");
                    show_run("port    ", &p);
                    show_run("original", &o);
                    if let (Some(a), Some(b)) = (&p.file, &o.file) {
                        let first = a.iter().zip(b).position(|(x, y)| x != y);
                        println!(
                            "      files: port {} bytes, original {} bytes, first difference at {first:?}",
                            a.len(),
                            b.len()
                        );
                        let k = work.join(format!("keepw-{seed}-{case_no}"));
                        let _ = std::fs::create_dir_all(&k);
                        let _ = std::fs::write(k.join(format!("{side}-port.mnc")), a);
                        let _ = std::fs::write(k.join(format!("{side}-orig.mnc")), b);
                    }
                }
            }
        }
        let _ = keep;
        let _ = std::fs::remove_dir_all(&dir);
    }
}
