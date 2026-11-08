// SPDX-License-Identifier: GPL-3.0-only
// Part of larmorx-gpl (GNU GPL version 3; see crates-gpl/LICENSE). Copyright 2026 Karel Lopez
// Vilaret.
//! Runs t2smap's workflow up to the fit and writes its stages as `.npy` files, for
//! `validation/investigations/t2smap/replica/check_stages.py`, which compares them with tedana's
//! own arrays (bit for bit).
//!
//! `t2smap_stages <dir> <t2smap arguments...>`: writes `<dir>/voxels.npy` (the mask's voxels,
//! Fortran-order indices, int64), `adaptive.npy` (int64), `denoise.npy` (uint8), `t2s.npy`,
//! `s0.npy` (float64) and, for curvefit, `failures.npy` (uint8), `t2s_var.npy`, `s0_var.npy`,
//! `covar.npy`. The environment is the program's (`LARMORX_UPSTREAM_DEFECTS`,
//! `LARMORX_TEDANA_LIBRARIES`).
use std::io::Write;

use larmorx_core::defects::{Notices, UpstreamDefects};
use larmorx_gpl_tedana::decay::Libraries;
use larmorx_gpl_tedana::log::Log;
use larmorx_gpl_tedana::parser;
use larmorx_gpl_tedana::workflow::{Env, prepare_and_fit};

fn npy(path: &std::path::Path, descr: &str, shape: &[usize], bytes: &[u8]) {
    let dims = match shape.len() {
        1 => format!("({},)", shape[0]),
        _ => format!(
            "({})",
            shape
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ),
    };
    let mut h = format!("{{'descr': '{descr}', 'fortran_order': False, 'shape': {dims}, }}");
    while (10 + h.len() + 1) % 64 != 0 {
        h.push(' ');
    }
    h.push('\n');
    let mut f = std::fs::File::create(path).expect("create");
    f.write_all(b"\x93NUMPY\x01\x00").unwrap();
    f.write_all(&(h.len() as u16).to_le_bytes()).unwrap();
    f.write_all(h.as_bytes()).unwrap();
    f.write_all(bytes).unwrap();
}

fn f64s(v: &[f64]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = std::path::PathBuf::from(&args[0]);
    std::fs::create_dir_all(&dir).expect("output folder");
    let rest = &args[1..];
    let mode = UpstreamDefects::from_env().expect("LARMORX_UPSTREAM_DEFECTS");
    let libraries = match std::env::var("LARMORX_TEDANA_LIBRARIES").as_deref() {
        Ok("shared") => Libraries::Shared,
        _ => Libraries::Newest,
    };
    let env = Env {
        columns: None,
        terminal_columns: None,
        mode,
        libraries,
    };
    let settings = match parser::parse(rest, 78) {
        Ok(s) => s,
        Err(e) => {
            eprint!("{}", e.stderr);
            std::process::exit(e.status);
        }
    };
    let mut err = std::io::stderr();
    let mut log = Log::new(&mut err);
    let mut notices = Notices::new(mode);
    let (setup, st) = match prepare_and_fit(settings, rest, &env, &mut log, &mut notices) {
        Ok(s) => s,
        Err(e) => {
            eprint!("{}", e.traceback());
            std::process::exit(1);
        }
    };
    for n in notices.into_vec() {
        eprintln!("larmorx-gpl: {}", n.line());
    }
    let md = st.fit.t2s.len();
    let shape: Vec<usize> = if setup.settings.fit_ts {
        let t = md / st.denoise.iter().filter(|&&b| b).count().max(1);
        vec![md / t.max(1), t]
    } else {
        vec![md]
    };
    let vox: Vec<u8> = st
        .masked
        .voxels
        .iter()
        .flat_map(|&v| (v as i64).to_le_bytes())
        .collect();
    npy(
        &dir.join("voxels.npy"),
        "<i8",
        &[st.masked.voxels.len()],
        &vox,
    );
    let am: Vec<u8> = st.adaptive.iter().flat_map(|a| a.to_le_bytes()).collect();
    npy(&dir.join("adaptive.npy"), "<i8", &[st.adaptive.len()], &am);
    let dn: Vec<u8> = st.denoise.iter().map(|&b| u8::from(b)).collect();
    npy(&dir.join("denoise.npy"), "|u1", &[dn.len()], &dn);
    npy(&dir.join("t2s.npy"), "<f8", &shape, &f64s(&st.fit.t2s));
    npy(&dir.join("s0.npy"), "<f8", &shape, &f64s(&st.fit.s0));
    if let Some(f) = &st.fit.failures {
        let b: Vec<u8> = f.iter().map(|&x| u8::from(x)).collect();
        npy(&dir.join("failures.npy"), "|u1", &shape, &b);
        npy(
            &dir.join("t2s_var.npy"),
            "<f8",
            &shape,
            &f64s(st.fit.t2s_var.as_ref().unwrap()),
        );
        npy(
            &dir.join("s0_var.npy"),
            "<f8",
            &shape,
            &f64s(st.fit.s0_var.as_ref().unwrap()),
        );
        npy(
            &dir.join("covar.npy"),
            "<f8",
            &shape,
            &f64s(st.fit.covar.as_ref().unwrap()),
        );
    }
}
