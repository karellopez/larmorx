// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The selective-average headers (`utils/selxavgio.cpp`, item 3.1-30): the `.dat` file beside a
//! volume made by FS-FAST's `selxavg` (`is_sxa_volume`, `ld_sxadat`, `ld_sxadat_from_stem`,
//! `sxa_framepower`, `sv_sxadat`, `sv_sxadat_by_stem`). FreeSurfer 8.2.0's programs reach them only
//! through a branch that is never taken (`if (0 && ...)` in the surface sampling of volumes), so
//! they are ported for the library's completeness.
//!
//! Defects (`larmorx_core::defects`; mechanisms in `docs/findings/freesurfer-io.md`, Part 32):
//! - **FS-846** (memory): a volume name that with `.dat` takes 1,000 bytes or more overflows
//!   FreeSurfer's buffer: both modes use the whole name, noted.
//! - **FS-847** (out of bounds): a header with more than 10 gamma functions (`GammaFit`) writes
//!   their parameters over the header's other fields: by default every value kept; with
//!   `reproduce` FreeSurfer's fields for up to 20 (more are refused: written past the header).
//! - **FS-848** (out of bounds, crash): counts whose products pass 2^31 (the matrices of
//!   estimates, the frames of `sxa_framepower`): FreeSurfer's sizes wrap around in 32 bits and
//!   its numbers are written outside its arrays. Refused by default; with `reproduce` FreeSurfer's
//!   reading while it stays inside its arrays (refused where it would not), `sxa_framepower` ends
//!   as the crash (139).
//! - **FS-849** (out of bounds): writing a header whose arrays hold fewer numbers than its counts
//!   (FreeSurfer reads past them): refused in both modes.

use std::io::Write;
use std::path::Path;

use larmorx_core::cfmt::f as cf;
use larmorx_core::defects::{FS_846, FS_847, FS_848, FS_849};
use larmorx_freesurfer_core::cscan::Scanf;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{Fatal, strerror};

use crate::cfile::CFile;
use crate::identify::id_stem_from_name;

/// FreeSurfer's name buffer (`char tmpstr[1000]`).
const TMPSTR: usize = 1000;
/// `gfDelta[10]`, `gfTau[10]`.
pub const NGAMMA: usize = 10;

/// `SXADAT`: a selective average's header.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SxaDat {
    pub version: i32,
    pub tr: f32,
    pub ter: f32,
    pub time_window: f32,
    pub t_pre_stim: f32,
    /// `Nc`: the conditions, fixation included.
    pub nc: i32,
    /// `Nnnc`: `Nc - 1`.
    pub nnnc: i32,
    /// `Nh`: the estimates per condition.
    pub nh: i32,
    pub dof: f32,
    /// `npercond` (`None`: `NULL`).
    pub npercond: Option<Vec<i32>>,
    pub nruns: i32,
    pub ntp: i32,
    pub nrows: i32,
    pub ncols: i32,
    pub nskip: i32,
    pub dt_order: i32,
    pub rescale_factor: f32,
    pub hanning_radius: f32,
    pub brain_air_seg: i32,
    pub gamma_fit: i32,
    /// `gfDelta`: 10 places in FreeSurfer (more by default for FS-847).
    pub gf_delta: Vec<f32>,
    pub gf_tau: Vec<f32>,
    pub null_cond_id: i32,
    /// `SumXtX` (`None`: `NULL`).
    pub sum_xtx: Option<Vec<f32>>,
    pub h_cov_mtx: Option<Vec<f32>>,
    pub n_noise_ac: i32,
    pub cond_id_map: Option<Vec<i32>>,
}

impl SxaDat {
    /// A header of zeros (`calloc`): the gamma arrays' 10 places each.
    pub fn zeroed() -> SxaDat {
        SxaDat {
            gf_delta: vec![0.0; NGAMMA],
            gf_tau: vec![0.0; NGAMMA],
            ..SxaDat::default()
        }
    }
}

/// `sprintf(tmpstr, "%s.dat", stem or volid)`: FS-846's check.
fn dat_name(g: &mut Globals, who: &str, volid: &str) -> String {
    let base = id_stem_from_name(volid).unwrap_or_else(|| volid.to_owned());
    let name = format!("{base}.dat");
    if name.len() >= TMPSTR {
        g.notices.add(
            &FS_846,
            format!(
                "{who}: a name of {} bytes used whole (FreeSurfer's buffer holds 999)",
                name.len()
            ),
        );
    }
    name
}

/// `is_sxa_volume(volid)`: 1 when `<stem>.dat` opens for reading (the stem `IDstemFromName`
/// gives, or the whole name; a folder opens, as on Linux), 0 otherwise or for `None`.
pub fn is_sxa_volume(g: &mut Globals, dir: Option<&Path>, volid: Option<&str>) -> i32 {
    let Some(volid) = volid else {
        return 0;
    };
    let name = dat_name(g, "is_sxa_volume", volid);
    match CFile::open(dir, &name, "r") {
        Ok(f) => {
            f.close();
            1
        }
        Err(e) => {
            g.errno = e;
            0
        }
    }
}

/// `ld_sxadat_from_stem(volid)`: `None` when [`is_sxa_volume`] is 0, else [`ld_sxadat`] of the
/// same `<stem>.dat`.
pub fn ld_sxadat_from_stem(
    g: &mut Globals,
    err: &mut dyn Write,
    dir: Option<&Path>,
    volid: &str,
) -> Result<Option<SxaDat>, Fatal> {
    if is_sxa_volume(g, dir, Some(volid)) == 0 {
        return Ok(None);
    }
    let name = dat_name(g, "ld_sxadat_from_stem", volid);
    ld_sxadat(g, err, dir, &name)
}

/// One `fscanf(fp, "%*s %f")` / `"%*s %d"` / `"%*s"` / `"%f"` / `"%d"` call.
enum Item {
    WordF,
    WordD,
    Word,
    F,
    D,
}

/// What a call read: the `%f` or `%d` value (when assigned), and the call's return value.
struct Got {
    f: Option<f32>,
    d: Option<i32>,
    n: i32,
}

fn scan(g: &mut Globals, fp: &mut CFile, item: Item) -> Got {
    let mut errno = g.errno;
    let mut sc = Scanf::new(fp);
    let (mut f, mut d) = (None, None);
    match item {
        Item::WordF | Item::WordD | Item::Word => {
            sc.s(&mut errno, usize::MAX, false);
            if !matches!(item, Item::Word) {
                sc.text(&mut errno, b" ");
                if matches!(item, Item::WordF) {
                    f = sc.f(&mut errno, true);
                } else {
                    d = sc.d(&mut errno, true);
                }
            }
        }
        Item::F => f = sc.f(&mut errno, true),
        Item::D => d = sc.d(&mut errno, true),
    }
    let n = sc.finish(&mut errno);
    g.errno = errno;
    Got { f, d, n }
}

fn read_f(g: &mut Globals, fp: &mut CFile, into: &mut f32) {
    if let Some(v) = scan(g, fp, Item::WordF).f {
        *into = v;
    }
}

fn read_d(g: &mut Globals, fp: &mut CFile, into: &mut i32) {
    if let Some(v) = scan(g, fp, Item::WordD).d {
        *into = v;
    }
}

/// `calloc(n, sizeof(int or float))` of a count FreeSurfer computes in `int`: `None` for a
/// negative count (its `size_t` is past the address space), zeros otherwise.
fn calloc_n<T: Clone + Default>(n: i32) -> Option<Vec<T>> {
    if n < 0 {
        return None;
    }
    larmorx_freesurfer_core::alloc::zeroed::<T>(n as usize)
}

/// The values of one loop of `fscanf(fp, "%d" or "%f", &a[i])` for `i` from 0 while below
/// `count` (an `i64`: FreeSurfer's loops of `Nch x Nch` run that many calls), stored where they
/// fall. A call that fails leaves the stream as it was (a matching failure reads nothing more
/// than white space, already read by the first), so every later call fails the same way: the
/// loop stops there. `Err(i)` when a value is read for a place `i` outside `a` (FreeSurfer writes
/// outside its array).
fn read_list<T: Copy>(
    g: &mut Globals,
    fp: &mut CFile,
    a: &mut Option<Vec<T>>,
    count: i64,
    read: fn(&mut Globals, &mut CFile) -> Option<T>,
) -> Result<(), i64> {
    let mut i = 0i64;
    while i < count {
        let Some(v) = read(g, fp) else {
            return Ok(());
        };
        match a.as_mut().and_then(|a| a.get_mut(i as usize)) {
            Some(p) => *p = v,
            None => return Err(i),
        }
        i += 1;
    }
    Ok(())
}

fn one_d(g: &mut Globals, fp: &mut CFile) -> Option<i32> {
    scan(g, fp, Item::D).d
}

fn one_f(g: &mut Globals, fp: &mut CFile) -> Option<f32> {
    scan(g, fp, Item::F).f
}

/// FS-848's refusal (`None` after the message).
fn too_large(g: &mut Globals, err: &mut dyn Write, what: &str) -> Option<SxaDat> {
    g.notices.add(
        &FS_848,
        format!(
            "ld_sxadat: {what}: a count past 2^31 (FreeSurfer's size wraps around and its numbers are written outside its arrays): refused"
        ),
    );
    let _ = writeln!(
        err,
        "ld_sxadat(): {what} past 2^31 numbers: the header is refused"
    );
    None
}

/// The gamma functions' parameters (FS-847): `G` places of `gfDelta` then of `gfTau` read with
/// `%f`, each list after a word. By default every value is kept (the lists hold `max(10, G)`).
/// With `reproduce`, FreeSurfer's stores in its header: `gfDelta[k]` for `k` from 10 is
/// `gfTau[k - 10]`, and past `gfTau` come `NullCondId`, padding, the `SumXtX` and `hCovMtx`
/// pointers, `nNoiseAC`, padding and the `CondIdMap` pointer (places 20 to 29 after `gfDelta`);
/// `Err` past the header (`G` above 20) or when a pointer the reading does not set again is
/// written (`version` 1: `hCovMtx` and `CondIdMap`).
fn read_gamma(
    g: &mut Globals,
    fp: &mut CFile,
    sxa: &mut SxaDat,
    version_after: &mut dyn FnMut(&mut Globals, &mut CFile, &mut SxaDat) -> GammaTail,
) -> Result<(), &'static str> {
    let gf = sxa.gamma_fit;
    let n = gf.max(0) as usize;
    if gf > NGAMMA as i32 {
        g.notices.add(
            &FS_847,
            format!(
                "ld_sxadat: {gf} gamma functions (FreeSurfer's header holds 10: their parameters are written over its other fields)"
            ),
        );
    }
    if gf <= NGAMMA as i32 || !g.notices.reproduce() {
        sxa.gf_delta.resize(n.max(NGAMMA), 0.0);
        sxa.gf_tau.resize(n.max(NGAMMA), 0.0);
        scan(g, fp, Item::Word);
        for k in 0..n {
            let Some(v) = one_f(g, fp) else { break };
            sxa.gf_delta[k] = v;
        }
        scan(g, fp, Item::Word);
        for k in 0..n {
            let Some(v) = one_f(g, fp) else { break };
            sxa.gf_tau[k] = v;
        }
        version_after(g, fp, sxa);
        return Ok(());
    }
    if n > 20 {
        return Err("more than 20 gamma functions are written past FreeSurfer's header");
    }
    // the 30 places of 4 bytes from gfDelta to the end of the header, as written
    let mut slot: [Option<u32>; 30] = [None; 30];
    scan(g, fp, Item::Word);
    for k in 0..n {
        let Some(v) = one_f(g, fp) else { break };
        slot[k] = Some(v.to_bits());
    }
    scan(g, fp, Item::Word);
    for k in 0..n {
        let Some(v) = one_f(g, fp) else { break };
        slot[10 + k] = Some(v.to_bits());
    }
    for k in 0..NGAMMA {
        sxa.gf_delta[k] = f32::from_bits(slot[k].unwrap_or(0));
        sxa.gf_tau[k] = f32::from_bits(slot[10 + k].unwrap_or(0));
    }
    if let Some(b) = slot[20] {
        sxa.null_cond_id = b as i32;
    }
    if let Some(b) = slot[26] {
        sxa.n_noise_ac = b as i32;
    }
    let tail = version_after(g, fp, sxa);
    if tail.version_one && (24..30).filter(|&k| k != 26 && k != 27).any(|k| slot[k].is_some()) {
        return Err("a pointer of the header overwritten and never set again");
    }
    Ok(())
}

/// What the reading after the gamma functions found (for FS-847's pointers).
struct GammaTail {
    version_one: bool,
}

/// `ld_sxadat(sxadatfile)`: the header read with `fscanf`: each field after a word (`TR`,
/// `TimeWindow`, `TPreStim` as `%f`, `nCond`, `Nh` as `%d`; `Nnnc = Nc - 1`); a missing
/// `version` value (the word read, the number not) returns the header at once with `version` 0;
/// otherwise `TER`, `DOF`, a word, `Nc` numbers of `NPerCond`, `nruns`, `ntp`, `nrows`,
/// `ncols`, `nskip`, `DTOrder`, `RescaleFactor`, `HanningRadius`, `nNoiseAC`, `BrainAirSeg`,
/// `GammaFit` (when not 0: a word and `GammaFit` deltas, a word and as many taus), `NullCondId`;
/// a word and the `Nch x Nch` numbers of `SumXtX` (`Nch = Nh x Nnnc`); for a version other than
/// 1 a word and as many of `hCovMtx`, a word and `Nc` numbers of `CondIdMap`. A value that is not
/// there leaves its field 0 (an array's places 0); an array of a negative count is `NULL`. A file
/// that does not open prints `ldsxdat():: <strerror>` and `Could not open <name>` on standard
/// error and gives `None`. FS-847, FS-848.
pub fn ld_sxadat(
    g: &mut Globals,
    err: &mut dyn Write,
    dir: Option<&Path>,
    sxadatfile: &str,
) -> Result<Option<SxaDat>, Fatal> {
    let mut fp = match CFile::open(dir, sxadatfile, "r") {
        Ok(f) => f,
        Err(e) => {
            g.errno = e;
            let _ = writeln!(err, "ldsxdat():: {}", strerror(e));
            let _ = writeln!(err, "Could not open {sxadatfile}");
            return Ok(None);
        }
    };
    let mut sxa = SxaDat::zeroed();
    read_f(g, &mut fp, &mut sxa.tr);
    read_f(g, &mut fp, &mut sxa.time_window);
    read_f(g, &mut fp, &mut sxa.t_pre_stim);
    read_d(g, &mut fp, &mut sxa.nc);
    read_d(g, &mut fp, &mut sxa.nh);
    sxa.nnnc = sxa.nc.wrapping_sub(1);
    let got = scan(g, &mut fp, Item::WordD);
    if let Some(v) = got.d {
        sxa.version = v;
    }
    if got.n == 0 {
        sxa.version = 0;
        return Ok(Some(sxa));
    }
    read_f(g, &mut fp, &mut sxa.ter);
    read_f(g, &mut fp, &mut sxa.dof);
    scan(g, &mut fp, Item::Word);
    sxa.npercond = calloc_n(sxa.nc);
    let nc = i64::from(sxa.nc);
    // a negative count reads nothing; a count of 0 and up has its array
    let _ = read_list(g, &mut fp, &mut sxa.npercond, nc, one_d);
    read_d(g, &mut fp, &mut sxa.nruns);
    read_d(g, &mut fp, &mut sxa.ntp);
    read_d(g, &mut fp, &mut sxa.nrows);
    read_d(g, &mut fp, &mut sxa.ncols);
    read_d(g, &mut fp, &mut sxa.nskip);
    read_d(g, &mut fp, &mut sxa.dt_order);
    read_f(g, &mut fp, &mut sxa.rescale_factor);
    read_f(g, &mut fp, &mut sxa.hanning_radius);
    read_d(g, &mut fp, &mut sxa.n_noise_ac);
    read_d(g, &mut fp, &mut sxa.brain_air_seg);
    read_d(g, &mut fp, &mut sxa.gamma_fit);
    let mut rest = |g: &mut Globals, fp: &mut CFile, sxa: &mut SxaDat| -> GammaTail {
        read_d(g, fp, &mut sxa.null_cond_id);
        GammaTail {
            version_one: sxa.version == 1,
        }
    };
    if sxa.gamma_fit != 0 {
        if let Err(why) = read_gamma(g, &mut fp, &mut sxa, &mut rest) {
            return Ok(too_large_gamma(g, err, why));
        }
    } else {
        rest(g, &mut fp, &mut sxa);
    }
    let nch = sxa.nh.wrapping_mul(sxa.nnnc);
    let exact = i64::from(nch) * i64::from(nch) * i64::from(nch > 0);
    let wrapped = nch.wrapping_mul(nch);
    let overflow = exact != i64::from(wrapped) && nch > 0;
    if overflow && !g.notices.reproduce() {
        return Ok(too_large(g, err, "SumXtX"));
    }
    scan(g, &mut fp, Item::Word);
    sxa.sum_xtx = calloc_n(wrapped);
    if read_list(g, &mut fp, &mut sxa.sum_xtx, exact, one_f).is_err() {
        return Ok(too_large(g, err, "SumXtX"));
    }
    if sxa.version == 1 {
        return Ok(Some(sxa));
    }
    scan(g, &mut fp, Item::Word);
    sxa.h_cov_mtx = calloc_n(wrapped);
    if read_list(g, &mut fp, &mut sxa.h_cov_mtx, exact, one_f).is_err() {
        return Ok(too_large(g, err, "hCovMtx"));
    }
    scan(g, &mut fp, Item::Word);
    sxa.cond_id_map = calloc_n(sxa.nc);
    let _ = read_list(g, &mut fp, &mut sxa.cond_id_map, nc, one_d);
    fp.close();
    Ok(Some(sxa))
}

/// FS-847's refusal with `reproduce` (`None` after the message).
fn too_large_gamma(g: &mut Globals, err: &mut dyn Write, why: &str) -> Option<SxaDat> {
    g.notices.add(
        &FS_847,
        format!("ld_sxadat: {why} (memory contents in FreeSurfer): refused"),
    );
    let _ = writeln!(
        err,
        "ld_sxadat(): too many gamma functions for FreeSurfer's header: the header is refused"
    );
    None
}

/// `sxa_framepower(sxa, &nframes)`: `nframes = 2 Nc Nh` (in `int`) and the frames' powers, for
/// each condition `Nh` places of 1 (the averages) then `Nh` of 2 (the deviations); `None`
/// (`NULL`) for a negative count. FS-848: a count past 2^31 for positive `Nc` and `Nh` (FreeSurfer
/// writes outside its array and crashes) is refused by default (`None`, with an error line);
/// with `reproduce` the program ends as FreeSurfer's (139).
pub fn sxa_framepower(
    g: &mut Globals,
    err: &mut dyn Write,
    sxa: &SxaDat,
) -> Result<(Option<Vec<f32>>, i32), Fatal> {
    let nframes = 2i32.wrapping_mul(sxa.nc).wrapping_mul(sxa.nh);
    let exact = 2 * i64::from(sxa.nc.max(0)) * i64::from(sxa.nh.max(0));
    if exact > i64::from(i32::MAX) || (exact > 0 && i64::from(nframes) != exact) {
        g.notices.add(
            &FS_848,
            format!(
                "sxa_framepower: 2 x {} x {} frames past 2^31 (FreeSurfer writes outside its array and crashes)",
                sxa.nc, sxa.nh
            ),
        );
        if g.notices.reproduce() {
            return Err(Fatal::Exit(139));
        }
        let _ = writeln!(
            err,
            "sxa_framepower(): {} conditions of {} estimates are past 2^31 frames",
            sxa.nc, sxa.nh
        );
        return Ok((None, nframes));
    }
    let mut fp: Option<Vec<f32>> = calloc_n(nframes);
    if let Some(p) = fp.as_mut() {
        let mut frame = 0usize;
        for _ in 0..sxa.nc.max(0) {
            for statid in 0..2 {
                for _ in 0..sxa.nh.max(0) {
                    p[frame] = if statid == 0 { 1.0 } else { 2.0 };
                    frame += 1;
                }
            }
        }
    }
    Ok((fp, nframes))
}

/// The first `n` values of an array, or FS-849's refusal.
fn values<'a, T>(a: &'a Option<Vec<T>>, n: i64, what: &str) -> Result<&'a [T], String> {
    let n = n.max(0) as usize;
    match a {
        Some(v) if v.len() >= n => Ok(&v[..n]),
        _ => Err(format!(
            "{what} holds fewer numbers than the header's count ({n})"
        )),
    }
}

/// `sv_sxadat(sxa, sxadatfile)`: the header written as `ld_sxadat` reads it: `TR         %f`,
/// `TimeWindow %f`, `TPreStim   %f`, `nCond      %d`, `Nh         %d` (version 0 ends there),
/// `version %d`, `TER %f`, `DOF %f`, `NPerCond ` and each `%d `, `nruns %d`, `ntp %d`, `nrows
/// %d`, `ncols %d`, `nskip %d`, `DTOrder %d`, `RescaleFactor %f`, `HanningRadius %f`, `nNoiseAC
/// %d`, `BrainAirSeg %d`, `GammaFit %d` (when not 0: `gfDelta ` and each `%f `, `gfTau ` and
/// each `%f `, each list on its line), `NullCondId %d`, `SumXtX` and each of its `Nch x Nch`
/// numbers on its line as `%f ` (version 1 ends there), `hCovMtx` likewise, `CondIdMap ` and each
/// `%d `; 0. A file that does not open prints `sv_sxdat():: <strerror>` and `Could not open
/// <name>` on standard error and returns 1. FreeSurfer leaves a version-1 file open, so its
/// bytes reach the file when the program ends (here when the function returns). FS-849: a header
/// whose arrays hold fewer numbers than its counts is refused (1, an error line; FreeSurfer reads
/// past them), as are more than 10 gamma functions held in 10 places.
pub fn sv_sxadat(
    g: &mut Globals,
    err: &mut dyn Write,
    dir: Option<&Path>,
    sxa: &SxaDat,
    sxadatfile: &str,
) -> i32 {
    let nch = i64::from(sxa.nh.wrapping_mul(sxa.nnnc));
    let nn = nch * nch * i64::from(nch > 0);
    let gf = i64::from(sxa.gamma_fit);
    let gamma = |a: &Vec<f32>, what: &str| {
        if a.len() as i64 >= gf {
            Ok(())
        } else {
            Err(format!(
                "{what} holds fewer numbers than GammaFit ({gf})"
            ))
        }
    };
    let check = (|| -> Result<(), String> {
        if sxa.version == 0 {
            return Ok(());
        }
        values(&sxa.npercond, i64::from(sxa.nc), "NPerCond")?;
        if sxa.gamma_fit != 0 {
            gamma(&sxa.gf_delta, "gfDelta")?;
            gamma(&sxa.gf_tau, "gfTau")?;
        }
        values(&sxa.sum_xtx, nn, "SumXtX")?;
        if sxa.version != 1 {
            values(&sxa.h_cov_mtx, nn, "hCovMtx")?;
            values(&sxa.cond_id_map, i64::from(sxa.nc), "CondIdMap")?;
        }
        Ok(())
    })();
    if let Err(why) = check {
        g.notices.add(
            &FS_849,
            format!("sv_sxadat: {why} (FreeSurfer reads past it): refused"),
        );
        let _ = writeln!(err, "sv_sxadat(): {why}: not written");
        return 1;
    }
    let mut fp = match CFile::open(dir, sxadatfile, "w") {
        Ok(f) => f,
        Err(e) => {
            g.errno = e;
            let _ = writeln!(err, "sv_sxdat():: {}", strerror(e));
            let _ = writeln!(err, "Could not open {sxadatfile}");
            return 1;
        }
    };
    let f = |v: f32| cf(f64::from(v), 6);
    let mut t = String::new();
    t += &format!("TR         {}\n", f(sxa.tr));
    t += &format!("TimeWindow {}\n", f(sxa.time_window));
    t += &format!("TPreStim   {}\n", f(sxa.t_pre_stim));
    t += &format!("nCond      {}\n", sxa.nc);
    t += &format!("Nh         {}\n", sxa.nh);
    if sxa.version == 0 {
        fp.put_bytes(t.as_bytes());
        fp.close();
        return 0;
    }
    t += &format!("version {}\n", sxa.version);
    t += &format!("TER {}\n", f(sxa.ter));
    t += &format!("DOF {}\n", f(sxa.dof));
    t += "NPerCond ";
    for v in values(&sxa.npercond, i64::from(sxa.nc), "").unwrap_or(&[]) {
        t += &format!("{v} ");
    }
    t += "\n";
    t += &format!("nruns {}\n", sxa.nruns);
    t += &format!("ntp {}\n", sxa.ntp);
    t += &format!("nrows {}\n", sxa.nrows);
    t += &format!("ncols {}\n", sxa.ncols);
    t += &format!("nskip {}\n", sxa.nskip);
    t += &format!("DTOrder {}\n", sxa.dt_order);
    t += &format!("RescaleFactor {}\n", f(sxa.rescale_factor));
    t += &format!("HanningRadius {}\n", f(sxa.hanning_radius));
    t += &format!("nNoiseAC {}\n", sxa.n_noise_ac);
    t += &format!("BrainAirSeg {}\n", sxa.brain_air_seg);
    t += &format!("GammaFit {}\n", sxa.gamma_fit);
    if sxa.gamma_fit != 0 {
        let n = gf.max(0) as usize;
        t += "gfDelta ";
        for v in &sxa.gf_delta[..n] {
            t += &format!("{} ", f(*v));
        }
        t += "\n";
        t += "gfTau ";
        for v in &sxa.gf_tau[..n] {
            t += &format!("{} ", f(*v));
        }
        t += "\n";
    }
    t += &format!("NullCondId {}\n", sxa.null_cond_id);
    t += "SumXtX\n";
    fp.put_bytes(t.as_bytes());
    t.clear();
    for v in values(&sxa.sum_xtx, nn, "").unwrap_or(&[]) {
        t += &format!("{} \n", f(*v));
    }
    if sxa.version == 1 {
        fp.put_bytes(t.as_bytes());
        drop(fp);
        return 0;
    }
    t += "hCovMtx\n";
    for v in values(&sxa.h_cov_mtx, nn, "").unwrap_or(&[]) {
        t += &format!("{} \n", f(*v));
    }
    t += "CondIdMap ";
    for v in values(&sxa.cond_id_map, i64::from(sxa.nc), "").unwrap_or(&[]) {
        t += &format!("{v} ");
    }
    t += "\n";
    fp.put_bytes(t.as_bytes());
    fp.close();
    0
}

/// `sv_sxadat_by_stem(sxadat, volid)`: [`sv_sxadat`] into `<stem>.dat` (the stem
/// `IDstemFromName` gives, or the whole name).
pub fn sv_sxadat_by_stem(
    g: &mut Globals,
    err: &mut dyn Write,
    dir: Option<&Path>,
    sxa: &SxaDat,
    volid: &str,
) -> i32 {
    let name = dat_name(g, "sv_sxadat_by_stem", volid);
    sv_sxadat(g, err, dir, sxa, &name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_core::defects::UpstreamDefects;

    const HEADER: &[u8] = b"TR 2.0\nTimeWindow 20\nTPreStim 4\nnCond 3\nNh 2\nversion 2\nTER 2\nDOF 37\nNPerCond 10 11 12\nnruns 2\nntp 100\nnrows 64\nncols 64\nnskip 4\nDTOrder 1\nRescaleFactor 1000\nHanningRadius 0\nnNoiseAC 0\nBrainAirSeg 0\nGammaFit 0\nNullCondId 0\nSumXtX\n1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16\nhCovMtx\n0.5 0 0 0 0 0.5 0 0 0 0 0.5 0 0 0 0 0.5\nCondIdMap 0 1 2\n";

    #[test]
    fn reads_and_writes_a_header() {
        let mut g = Globals::new(UpstreamDefects::Fix);
        let mut e = Vec::new();
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("h.dat"), HEADER).unwrap();
        assert_eq!(is_sxa_volume(&mut g, Some(dir.path()), Some("h.mgz")), 1);
        assert_eq!(is_sxa_volume(&mut g, Some(dir.path()), Some("q.mgz")), 0);
        let s = ld_sxadat_from_stem(&mut g, &mut e, Some(dir.path()), "h.mgz")
            .unwrap()
            .unwrap();
        assert_eq!((s.version, s.nc, s.nnnc, s.nh), (2, 3, 2, 2));
        assert_eq!(s.npercond, Some(vec![10, 11, 12]));
        assert_eq!(s.sum_xtx.as_ref().unwrap()[15], 16.0);
        assert_eq!(s.cond_id_map, Some(vec![0, 1, 2]));
        let (p, n) = sxa_framepower(&mut g, &mut e, &s).unwrap();
        assert_eq!(n, 12);
        assert_eq!(p.unwrap()[..4], [1.0, 1.0, 2.0, 2.0]);
        assert_eq!(sv_sxadat(&mut g, &mut e, Some(dir.path()), &s, "o.dat"), 0);
        let back = ld_sxadat(&mut g, &mut e, Some(dir.path()), "o.dat")
            .unwrap()
            .unwrap();
        assert_eq!(back, s);
    }

    #[test]
    fn gamma_functions_past_ten() {
        let mut text = b"TR 2\nTimeWindow 20\nTPreStim 4\nnCond 2\nNh 1\nversion 2\nTER 2\nDOF 3\nNPerCond 1 1\nnruns 1\nntp 1\nnrows 1\nncols 1\nnskip 0\nDTOrder 0\nRescaleFactor 1\nHanningRadius 0\nnNoiseAC 5\nBrainAirSeg 0\nGammaFit 12\ngfDelta".to_vec();
        for k in 0..12 {
            text.extend_from_slice(format!(" {k}").as_bytes());
        }
        text.extend_from_slice(b"\ngfTau");
        for k in 0..12 {
            text.extend_from_slice(format!(" {}", 100 + k).as_bytes());
        }
        text.extend_from_slice(b"\nNullCondId 7\nSumXtX 9\nhCovMtx 8\nCondIdMap 0 1\n");
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("g.dat"), &text).unwrap();
        let mut e = Vec::new();
        let mut g = Globals::new(UpstreamDefects::Fix);
        let s = ld_sxadat(&mut g, &mut e, Some(dir.path()), "g.dat")
            .unwrap()
            .unwrap();
        assert_eq!(s.gf_delta[11], 11.0);
        assert_eq!(s.gf_tau[11], 111.0);
        assert_eq!((s.null_cond_id, s.n_noise_ac), (7, 5));
        let mut g = Globals::new(UpstreamDefects::Reproduce);
        let s = ld_sxadat(&mut g, &mut e, Some(dir.path()), "g.dat")
            .unwrap()
            .unwrap();
        assert_eq!(s.gf_delta.len(), 10);
        // gfTau[10] and [11] landed on NullCondId (read again after) and the padding
        assert_eq!(s.gf_tau[0], 100.0);
        assert_eq!(s.null_cond_id, 7);
        assert!(g.notices.contains(&FS_847));
    }
}
