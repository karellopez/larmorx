// SPDX-License-Identifier: Apache-2.0
//! MINC 1 volumes: generated netCDF classic files (well-formed MINC files of every type, sign,
//! range, slice scaling, geometry and dimension order; broken headers; the special cases) read by
//! the port and by the two originals with FreeSurfer 8.2.0's mincRead sequence (`shim_minc.c`).
//!
//! The port runs with MINC 1.5.1's own dialect for `libminc_up.so` and with FreeSurfer's dialect as
//! measured here (its package errors end the program with its source file and line, it cannot
//! decompress, it refuses unknown types and irregular spacings) for `libminc_fs.so`.

use std::fmt::Write as _;
use std::path::Path;

use larmorx_core::cfmt::Streams;
use larmorx_core::defects::{Notices, UpstreamDefects};
use larmorx_io::minc::{self, Ctx, Dialect, Exit, PkgSite, Session, Status};

use crate::Rng;
use crate::ffi::{MincLib, Outcome};

const NC_BYTE: i32 = 1;
const NC_CHAR: i32 = 2;
const NC_SHORT: i32 = 3;
const NC_INT: i32 = 4;
const NC_FLOAT: i32 = 5;
const NC_DOUBLE: i32 = 6;

/// An attribute's or a variable's values as written (big-endian external bytes per type).
#[derive(Clone, Debug)]
pub enum Val {
    Typed(i32, Vec<f64>),
    Text(Vec<u8>),
    /// A type netCDF does not know: its count and bytes as given.
    Raw(i32, u32, Vec<u8>),
}

#[derive(Clone, Debug)]
pub struct Att {
    pub name: Vec<u8>,
    pub val: Val,
}

#[derive(Clone, Debug)]
pub struct VarDef {
    pub name: Vec<u8>,
    pub dimids: Vec<i32>,
    pub atts: Vec<Att>,
    pub ty: i32,
    pub data: Val,
}

#[derive(Clone, Debug, Default)]
pub struct Spec {
    pub version: u8,
    pub numrecs: u32,
    pub dims: Vec<(Vec<u8>, u32)>,
    pub gatts: Vec<Att>,
    pub vars: Vec<VarDef>,
}

fn tsize(t: i32) -> usize {
    match t {
        NC_BYTE | NC_CHAR => 1,
        NC_SHORT => 2,
        NC_INT | NC_FLOAT => 4,
        NC_DOUBLE => 8,
        _ => 0,
    }
}

fn pad4(v: &mut Vec<u8>) {
    while v.len() % 4 != 0 {
        v.push(0);
    }
}

fn put_name(o: &mut Vec<u8>, n: &[u8]) {
    o.extend_from_slice(&(n.len() as u32).to_be_bytes());
    o.extend_from_slice(n);
    pad4(o);
}

fn encode(t: i32, vals: &[f64]) -> Vec<u8> {
    let mut o = Vec::new();
    for &v in vals {
        match t {
            NC_BYTE => o.push(v as i64 as i8 as u8),
            NC_CHAR => o.push(v as u8),
            NC_SHORT => o.extend_from_slice(&(v as i64 as i16).to_be_bytes()),
            NC_INT => o.extend_from_slice(&(v as i64 as i32).to_be_bytes()),
            NC_FLOAT => o.extend_from_slice(&(v as f32).to_bits().to_be_bytes()),
            NC_DOUBLE => o.extend_from_slice(&v.to_bits().to_be_bytes()),
            _ => {}
        }
    }
    o
}

fn val_bytes(v: &Val) -> (i32, u32, Vec<u8>) {
    match v {
        Val::Typed(t, x) => (*t, x.len() as u32, encode(*t, x)),
        Val::Text(b) => (NC_CHAR, b.len() as u32, b.clone()),
        Val::Raw(t, n, b) => (*t, *n, b.clone()),
    }
}

fn put_atts(o: &mut Vec<u8>, atts: &[Att]) {
    if atts.is_empty() {
        o.extend_from_slice(&[0; 8]);
        return;
    }
    o.extend_from_slice(&0x0Cu32.to_be_bytes());
    o.extend_from_slice(&(atts.len() as u32).to_be_bytes());
    for a in atts {
        put_name(o, &a.name);
        let (t, n, mut b) = val_bytes(&a.val);
        o.extend_from_slice(&(t as u32).to_be_bytes());
        o.extend_from_slice(&n.to_be_bytes());
        pad4(&mut b);
        o.extend_from_slice(&b);
    }
}

impl Spec {
    fn is_rec(&self, v: &VarDef) -> bool {
        v.dimids
            .first()
            .and_then(|&d| self.dims.get(d as usize))
            .is_some_and(|d| d.1 == 0)
    }
    /// Values per record (or all values of a non-record variable).
    fn per(&self, v: &VarDef) -> usize {
        let mut n = 1usize;
        for (i, &d) in v.dimids.iter().enumerate() {
            if i == 0 && self.is_rec(v) {
                continue;
            }
            n *= self.dims.get(d as usize).map_or(1, |x| x.1 as usize);
        }
        n
    }
    fn vsize(&self, v: &VarDef) -> usize {
        let n = self.per(v) * tsize(v.ty);
        n.div_ceil(4) * 4
    }

    /// The file's bytes.
    pub fn write(&self) -> Vec<u8> {
        let header = |begins: &[u64]| {
            let mut o = Vec::new();
            o.extend_from_slice(b"CDF");
            o.push(self.version);
            o.extend_from_slice(&self.numrecs.to_be_bytes());
            if self.dims.is_empty() {
                o.extend_from_slice(&[0; 8]);
            } else {
                o.extend_from_slice(&0x0Au32.to_be_bytes());
                o.extend_from_slice(&(self.dims.len() as u32).to_be_bytes());
                for (n, s) in &self.dims {
                    put_name(&mut o, n);
                    o.extend_from_slice(&s.to_be_bytes());
                }
            }
            put_atts(&mut o, &self.gatts);
            if self.vars.is_empty() {
                o.extend_from_slice(&[0; 8]);
            } else {
                o.extend_from_slice(&0x0Bu32.to_be_bytes());
                o.extend_from_slice(&(self.vars.len() as u32).to_be_bytes());
                for (v, &b) in self.vars.iter().zip(begins) {
                    put_name(&mut o, &v.name);
                    o.extend_from_slice(&(v.dimids.len() as u32).to_be_bytes());
                    for d in &v.dimids {
                        o.extend_from_slice(&(*d as u32).to_be_bytes());
                    }
                    put_atts(&mut o, &v.atts);
                    o.extend_from_slice(&(v.ty as u32).to_be_bytes());
                    o.extend_from_slice(&(self.vsize(v) as u32).to_be_bytes());
                    if self.version == 1 {
                        o.extend_from_slice(&(b as u32).to_be_bytes());
                    } else {
                        o.extend_from_slice(&b.to_be_bytes());
                    }
                }
            }
            o
        };
        let zero = vec![0u64; self.vars.len()];
        let hlen = header(&zero).len() as u64;
        let mut begins = vec![0u64; self.vars.len()];
        let mut off = hlen;
        for (i, v) in self.vars.iter().enumerate() {
            if !self.is_rec(v) {
                begins[i] = off;
                off += self.vsize(v) as u64;
            }
        }
        let recs: Vec<usize> = (0..self.vars.len())
            .filter(|&i| self.is_rec(&self.vars[i]))
            .collect();
        let single = recs.len() == 1;
        let mut o = off;
        for &i in &recs {
            begins[i] = o;
            if !single {
                o += self.vsize(&self.vars[i]) as u64;
            }
        }
        let mut out = header(&begins);
        for v in &self.vars {
            if self.is_rec(v) {
                continue;
            }
            let (_, _, mut b) = val_bytes(&v.data);
            b.resize(self.per(v) * tsize(v.ty), 0);
            pad4(&mut b);
            out.extend_from_slice(&b);
        }
        for r in 0..self.numrecs as usize {
            for &i in &recs {
                let v = &self.vars[i];
                let per = self.per(v);
                let (_, _, b) = val_bytes(&v.data);
                let sz = tsize(v.ty);
                let mut piece = vec![0u8; per * sz];
                let a = (r * per * sz).min(b.len());
                let e = ((r + 1) * per * sz).min(b.len());
                piece[..e - a].copy_from_slice(&b[a..e]);
                if !single {
                    pad4(&mut piece);
                }
                out.extend_from_slice(&piece);
            }
        }
        out
    }
}

fn hex(b: &[u8]) -> String {
    let mut s = String::with_capacity(b.len() * 2);
    for c in b {
        let _ = write!(s, "{c:02x}");
    }
    s
}

fn cs(b: &[u8]) -> String {
    let b = &b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())];
    format!(" s{}", hex(b))
}

fn dbl(v: f64) -> String {
    format!(" {:016x}", v.to_bits())
}

/// FreeSurfer 8.2.0's copy of MINC, as the probe measures it: a package error prints the source
/// file and line of its copy and ends the program (exit status 1); no entry-point line; no
/// decompression (FS-788); unknown types and irregular spacings end the program.
pub struct FreeSurferDialect {
    /// The directory of its copy's sources as its messages print it (read from the library).
    pub prefix: String,
}

impl FreeSurferDialect {
    fn fatal(&self, at: &str, msg: &str, streams: &mut dyn Streams) -> Exit {
        streams.err(&format!("{}{at} {msg}\n", self.prefix));
        Exit { status: 1 }
    }
}

impl Dialect for FreeSurferDialect {
    fn pkg_error(
        &self,
        site: PkgSite,
        _code: i32,
        message: &str,
        streams: &mut dyn Streams,
    ) -> Option<Exit> {
        let at = match site {
            PkgSite::Uncompress => "minc_netcdf_convenience.c:402",
            PkgSite::AttNonNumeric => "minc_netcdf_convenience.c:786",
            PkgSite::AttNonScalar => "minc_netcdf_convenience.c:899",
            PkgSite::AttNonChar => "minc_netcdf_convenience.c:940",
            PkgSite::VarNonNumeric => "minc_value_conversion.c:229",
            PkgSite::IcvTooFewDims => "minc_image_conversion.c:2599",
            PkgSite::VarLoopAlloc => "minc_value_conversion.c:490",
            PkgSite::AttGetAlloc => "minc_netcdf_convenience.c:801",
            PkgSite::IcvNotAttached => "minc_image_conversion.c:1328",
            _ => "minc_image_conversion.c:?",
        };
        Some(self.fatal(at, message, streams))
    }
    fn logs_entry_point(&self) -> bool {
        false
    }
    fn cache_threshold(&self) -> Option<i64> {
        None
    }
    fn decompresses(&self, notices: &mut Notices) -> bool {
        if notices.reproduce() {
            false
        } else {
            notices.add(
                &larmorx_core::defects::FS_788,
                "a compressed MINC file decompressed (FreeSurfer's copy cannot)",
            );
            true
        }
    }
    fn default_range(
        &self,
        nc_type: i32,
        notices: &mut Notices,
        streams: &mut dyn Streams,
    ) -> Option<Exit> {
        match nc_type {
            NC_BYTE | NC_SHORT | NC_INT | NC_FLOAT => None,
            NC_DOUBLE if notices.fix() => {
                notices.add(
                    &larmorx_core::defects::FS_775,
                    "a MINC image of type double read (FreeSurfer's copy ends the program)",
                );
                None
            }
            _ => Some(self.fatal("minc_convenience.c:568", "bad data type", streams)),
        }
    }
    fn xfm(&self) -> &dyn larmorx_io::xfm::Dialect {
        &FsInverse
    }
    fn bad_data_type(&self, _nc_type: i32, streams: &mut dyn Streams) -> Option<Exit> {
        Some(self.fatal("minc_volume_io.c:2289", "bad nc_data_type", streams))
    }
    fn irregular_spacing(&self, widths: bool, streams: &mut dyn Streams) -> Option<Exit> {
        Some(if widths {
            self.fatal(
                "minc_volume_io.c:?",
                "set_volume_irregular_widths NYI",
                streams,
            )
        } else {
            self.fatal(
                "minc_volume_io.c:2035",
                "set_volume_irregular_starts NYI",
                streams,
            )
        })
    }
}

/// FreeSurfer's 4 x 4 inverse as the probe measures it: a matrix with a row (checked first) or a
/// column of zeros fails with its message on standard output; otherwise volume_io's inverse (track A's port of FreeSurfer's
/// own is the reference: `larmorx_freesurfer_io::minc::FsVolumeIo`).
pub struct FsInverse;

thread_local! {
    /// Set when the port's run reached the shim's limit of 100,000 slabs.
    static LIMIT_HIT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// Set when [`FsInverse`] met a matrix with an entry that is not finite or beyond 1e300: the
    /// probe does not model FreeSurfer's inverse there.
    static NOT_MODELLED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

impl larmorx_io::xfm::Dialect for FsInverse {
    fn invert(
        &self,
        t: &larmorx_io::xfm::Transform,
        streams: &mut dyn Streams,
    ) -> Option<larmorx_io::xfm::Transform> {
        if t.iter()
            .flatten()
            .any(|v| !v.is_finite() || v.abs() > 1e300)
        {
            NOT_MODELLED.with(|f| f.set(true));
        }
        if (0..4).any(|r| (0..4).all(|c| t[r][c] == 0.0)) {
            streams.out("invert_4x4_matrix failed, Some row is all zeros\n");
            return None;
        }
        if (0..4).any(|c| (0..4).all(|r| t[r][c] == 0.0)) {
            streams.out("invert_4x4_matrix failed, Some col is all zeros\n");
            return None;
        }
        larmorx_io::xfm::compute_transform_inverse(t)
    }
}

/// MINC 1.5.1's dialect with the cache threshold the shim set in the original's child.
pub struct MniCache(pub i64);

impl Dialect for MniCache {
    fn cache_threshold(&self) -> Option<i64> {
        Some(self.0)
    }
}

/// What the port did with a case, in the shim's terms.
#[derive(Debug, Clone)]
pub struct PortRun {
    pub dump: Option<String>,
    pub exit: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
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

/// The shim's sequence with the port.
pub fn port(name: &[u8], mode: UpstreamDefects, dialect: &dyn Dialect) -> PortRun {
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
        run_port(&mut s, &mut c, name, &mut dump)
    };
    PortRun {
        dump: if r.is_ok() { Some(dump) } else { None },
        exit: r.err().map(|e| e.status),
        stdout: cap.out,
        stderr: cap.err,
        notices: notices
            .as_slice()
            .iter()
            .map(|n| n.defect.id.to_string())
            .collect(),
    }
}

fn run_port(s: &mut Session, c: &mut Ctx, name: &[u8], o: &mut String) -> Result<(), Exit> {
    let names: Vec<Vec<u8>> = vec![
        b"xspace".to_vec(),
        b"yspace".to_vec(),
        b"zspace".to_vec(),
        b"time".to_vec(),
    ];
    let mut vol = None;
    let (status, input) = minc::start_volume_input(
        s,
        c,
        name,
        0,
        Some(&names),
        0,
        false,
        0.0,
        0.0,
        true,
        &mut vol,
        None,
    )?;
    let _ = writeln!(o, "status {}", status as i32);
    if status != Status::Ok {
        return Ok(());
    }
    let mut input = input.expect("an input with status OK");
    let v = vol.as_mut().expect("a volume with status OK");
    let n = v.n_dimensions();
    let _ = writeln!(
        o,
        "ndims {n} type {} signed {} cached 0 rgba {}",
        v.nc_data_type, v.signed_flag as i32, v.is_rgba_data as i32
    );
    o.push_str("names");
    for k in 0..n {
        o.push_str(&cs(&v.dimension_names[k]));
    }
    o.push_str("\nsizes");
    for k in 0..n {
        let _ = write!(o, " {}", v.sizes[k]);
    }
    o.push_str("\nsep");
    for k in 0..n {
        o.push_str(&dbl(v.separations[k]));
    }
    o.push_str("\nstarts");
    for k in 0..n {
        o.push_str(&dbl(v.starts[k]));
    }
    o.push_str("\ndircos");
    for k in 0..n {
        for j in 0..3 {
            o.push_str(&dbl(v.direction_cosines[k][j]));
        }
    }
    let _ = writeln!(
        o,
        "\nspatial {} {} {}",
        v.spatial_axes[0], v.spatial_axes[1], v.spatial_axes[2]
    );
    let (a, b) = v.voxel_range();
    let (ra, rb) = v.real_range();
    let _ = write!(
        o,
        "vrange{}{}\nrrange{}{}\nscale{}{}\nspace{}",
        dbl(a),
        dbl(b),
        dbl(ra),
        dbl(rb),
        dbl(v.real_value_scale),
        dbl(v.real_value_translation),
        cs(&v.coordinate_system_name)
    );
    let m = v.voxel_to_world_matrix(c.dialect, c.streams);
    o.push_str("\nxform 0");
    for r in 0..4 {
        for k in 0..4 {
            o.push_str(&dbl(m[r][k]));
        }
    }
    let mut voxel = [0.0f64; 5];
    if n >= 3 {
        voxel[0] = v.sizes[0] as f64 / 2.0;
        voxel[1] = v.sizes[1] as f64 / 2.0;
        voxel[2] = v.sizes[2] as f64 / 2.0;
    }
    let w = v.convert_voxel_to_world(&voxel, c.dialect, c.streams);
    let _ = write!(o, "\nworld_c{}{}{}", dbl(w[0]), dbl(w[1]), dbl(w[2]));
    let w = v.convert_voxel_to_world(&[0.0; 5], c.dialect, c.streams);
    let _ = write!(o, "\nworld_0{}{}{}", dbl(w[0]), dbl(w[1]), dbl(w[2]));
    o.push_str("\nmore");
    let mut f;
    let mut k = 0;
    loop {
        let (more, frac) = minc::input_more_of_volume(s, c, v, &mut input)?;
        f = frac;
        if more && k >= 100000 {
            // The shim stops reading after 100,000 slabs: the rest of the volume is never
            // written (memory in the original's process); the case is not compared.
            LIMIT_HIT.with(|l| l.set(true));
        }
        if !(more && k < 100000) {
            break;
        }
        o.push_str(&dbl(f));
        k += 1;
    }
    o.push_str(&dbl(f));
    let _ = write!(o, "\ncached_after {}\ndata", v.is_cached_volume as i32);
    let total: i64 = v.sizes.iter().map(|&x| x as i64).product();
    if n >= 1 && total > 0 && total <= 1 << 22 {
        let mut idx = vec![0usize; n];
        loop {
            o.push_str(&dbl(v.voxel_value(&idx)));
            let mut i = n as i64 - 1;
            while i >= 0 {
                idx[i as usize] += 1;
                if idx[i as usize] < v.sizes[i as usize] as usize {
                    break;
                }
                idx[i as usize] = 0;
                i -= 1;
            }
            if i < 0 {
                break;
            }
        }
    }
    o.push('\n');
    minc::delete_volume_input(s, c, input)?;
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// The generator.

fn pick<'a, T>(r: &mut Rng, v: &'a [T]) -> &'a T {
    &v[r.below(v.len() as u64) as usize]
}

fn chance(r: &mut Rng, p: u64) -> bool {
    r.below(100) < p
}

fn rand_f(r: &mut Rng, lo: f64, hi: f64) -> f64 {
    lo + (hi - lo) * (r.below(1 << 30) as f64 / (1u64 << 30) as f64)
}

fn type_range(t: i32, signed: bool) -> (f64, f64) {
    match (t, signed) {
        (NC_BYTE, true) => (-128.0, 127.0),
        (NC_BYTE, false) => (0.0, 255.0),
        (NC_SHORT, true) => (-32768.0, 32767.0),
        (NC_SHORT, false) => (0.0, 65535.0),
        (NC_INT, true) => (-2147483648.0, 2147483647.0),
        (NC_INT, false) => (0.0, 4294967295.0),
        _ => (-1e6, 1e6),
    }
}

/// A value stored as the type stores it (the writer narrows).
fn rand_value(r: &mut Rng, t: i32, signed: bool) -> f64 {
    let (lo, hi) = type_range(t, signed);
    let x = match r.below(8) {
        0 => lo,
        1 => hi,
        2 => 0.0,
        _ => rand_f(r, lo, hi).round(),
    };
    match t {
        NC_FLOAT | NC_DOUBLE => match r.below(40) {
            0 => f64::NAN,
            1 => f64::INFINITY,
            2 => -f64::INFINITY,
            3 => 3.4e38,
            _ => rand_f(r, -1000.0, 1000.0),
        },
        // The writer stores the low bits: a signed view of an unsigned value.
        NC_BYTE => (x as i64 as u8 as i8) as f64,
        NC_SHORT => (x as i64 as u16 as i16) as f64,
        NC_INT => (x as i64 as u32 as i32) as f64,
        _ => x,
    }
}

fn dim_var(r: &mut Rng, name: &[u8], spatial: bool) -> VarDef {
    let mut atts = Vec::new();
    if chance(r, 85) {
        let step = match r.below(12) {
            0 => -rand_f(r, 0.5, 3.0),
            1 => 0.0,
            _ => rand_f(r, 0.5, 3.0),
        };
        match r.below(30) {
            0 => atts.push(Att {
                name: b"step".to_vec(),
                val: Val::Typed(NC_DOUBLE, vec![step, 1.0]),
            }),
            1 => atts.push(Att {
                name: b"step".to_vec(),
                val: Val::Text(b"abc".to_vec()),
            }),
            2 => atts.push(Att {
                name: b"step".to_vec(),
                val: Val::Typed(NC_FLOAT, vec![step]),
            }),
            3 => atts.push(Att {
                name: b"step".to_vec(),
                val: Val::Typed(NC_SHORT, vec![step.round()]),
            }),
            _ => atts.push(Att {
                name: b"step".to_vec(),
                val: Val::Typed(NC_DOUBLE, vec![step]),
            }),
        }
    }
    if chance(r, 80) {
        atts.push(Att {
            name: b"start".to_vec(),
            val: Val::Typed(NC_DOUBLE, vec![rand_f(r, -150.0, 150.0)]),
        });
    }
    if spatial && chance(r, 70) {
        let n = if chance(r, 5) { 2 } else { 3 };
        let mut d: Vec<f64> = (0..n).map(|_| rand_f(r, -1.0, 1.0)).collect();
        if chance(r, 5) {
            d = vec![0.0; n];
        }
        atts.push(Att {
            name: b"direction_cosines".to_vec(),
            val: Val::Typed(NC_DOUBLE, d),
        });
    }
    if spatial && chance(r, 50) {
        let st: &[u8] = pick(
            r,
            &[
                &b"talairach_"[..],
                b"native____",
                b"world",
                b"talairach_",
                b"native____",
                b"world",
                b"talairach_",
                b"",
            ],
        );
        atts.push(Att {
            name: b"spacetype".to_vec(),
            val: Val::Text(st.to_vec()),
        });
    }
    if !spatial && chance(r, 60) {
        let sp: &[u8] = pick(
            r,
            &[
                &b"regular__"[..],
                b"irregular",
                b"regular__\0",
                b"regular__",
                b"irregular",
                b"regular__",
                b"",
            ],
        );
        atts.push(Att {
            name: b"spacing".to_vec(),
            val: Val::Text(sp.to_vec()),
        });
    }
    VarDef {
        name: name.to_vec(),
        dimids: Vec::new(),
        atts,
        ty: NC_INT,
        data: Val::Typed(NC_INT, vec![0.0]),
    }
}

/// A well-formed MINC file (mostly; some attributes in odd forms).
fn well_formed(r: &mut Rng) -> Spec {
    let mut spatial: Vec<&[u8]> = vec![b"zspace", b"yspace", b"xspace"];
    if chance(r, 25) {
        for i in (1..3).rev() {
            let j = r.below(i as u64 + 1) as usize;
            spatial.swap(i, j);
        }
    }
    if chance(r, 8) {
        spatial.truncate(1 + r.below(2) as usize);
    }
    let mut names: Vec<Vec<u8>> = spatial.iter().map(|s| s.to_vec()).collect();
    let with_time = chance(r, 35);
    if with_time {
        let at = if chance(r, 80) {
            0
        } else {
            r.below(names.len() as u64 + 1) as usize
        };
        names.insert(at, b"time".to_vec());
    }
    if chance(r, 4) {
        names.push(b"vector_dimension".to_vec());
    }
    if chance(r, 3) {
        names.insert(0, b"foo".to_vec());
    }
    let big = chance(r, 6);
    let mut dims: Vec<(Vec<u8>, u32)> = names
        .iter()
        .map(|n| {
            let s = if big {
                5 + r.below(30) as u32
            } else {
                1 + r.below(5) as u32
            };
            (n.clone(), s)
        })
        .collect();
    let mut numrecs = 0;
    if with_time && dims[0].0 == b"time" && chance(r, 40) {
        numrecs = dims[0].1;
        if chance(r, 5) {
            numrecs = 0;
        }
        dims[0].1 = 0;
    }
    let ty = *pick(
        r,
        &[NC_BYTE, NC_SHORT, NC_SHORT, NC_INT, NC_FLOAT, NC_DOUBLE],
    );
    let mut iatts = Vec::new();
    let signed_choice = r.below(6);
    let signed = match signed_choice {
        0 => {
            iatts.push(Att {
                name: b"signtype".to_vec(),
                val: Val::Text(b"signed__".to_vec()),
            });
            true
        }
        1 => {
            iatts.push(Att {
                name: b"signtype".to_vec(),
                val: Val::Text(b"unsigned".to_vec()),
            });
            false
        }
        2 if chance(r, 30) => {
            let v: &[u8] = pick(
                r,
                &[&b"foo"[..], b"signed__\0", b"unsigned extra", b"foo", b""],
            );
            iatts.push(Att {
                name: b"signtype".to_vec(),
                val: Val::Text(v.to_vec()),
            });
            ty != NC_BYTE
        }
        3 if chance(r, 10) => {
            iatts.push(Att {
                name: b"signtype".to_vec(),
                val: Val::Typed(NC_INT, vec![1.0]),
            });
            ty != NC_BYTE
        }
        _ => ty != NC_BYTE,
    };
    let (lo, hi) = type_range(ty, signed);
    match r.below(8) {
        0 | 1 => {
            let a = rand_value(r, ty, signed);
            let b = rand_value(r, ty, signed);
            let (a, b) = if chance(r, 85) && a > b {
                (b, a)
            } else {
                (a, b)
            };
            let t = *pick(
                r,
                &[NC_DOUBLE, NC_DOUBLE, NC_DOUBLE, NC_FLOAT, NC_SHORT, NC_INT],
            );
            iatts.push(Att {
                name: b"valid_range".to_vec(),
                val: Val::Typed(t, vec![a, b]),
            });
        }
        2 => {
            iatts.push(Att {
                name: b"valid_max".to_vec(),
                val: Val::Typed(NC_DOUBLE, vec![rand_value(r, ty, signed)]),
            });
            if chance(r, 60) {
                iatts.push(Att {
                    name: b"valid_min".to_vec(),
                    val: Val::Typed(NC_DOUBLE, vec![rand_value(r, ty, signed)]),
                });
            }
        }
        3 if chance(r, 30) => {
            let n = *pick(r, &[1usize, 3]);
            iatts.push(Att {
                name: b"valid_range".to_vec(),
                val: Val::Typed(
                    NC_DOUBLE,
                    (0..n).map(|_| rand_value(r, ty, signed)).collect(),
                ),
            });
        }
        4 if chance(r, 20) => {
            iatts.push(Att {
                name: b"valid_range".to_vec(),
                val: Val::Typed(NC_DOUBLE, vec![lo - 1000.0, hi + 1000.0]),
            });
        }
        _ => {}
    }
    let ndim = dims.len();
    let mut vars = Vec::new();
    for (i, (n, _)) in dims.iter().enumerate() {
        if n == b"vector_dimension" || n == b"foo" {
            continue;
        }
        if chance(r, 88) {
            let sp = n != b"time";
            let mut v = dim_var(r, n, sp);
            if !sp
                && let Some(a) = v.atts.iter().find(|a| a.name == b"spacing")
                && matches!(&a.val, Val::Text(t) if t == b"irregular")
            {
                // The time values.
                let size = if dims[i].1 == 0 { numrecs } else { dims[i].1 };
                v.dimids = vec![i as i32];
                v.ty = NC_DOUBLE;
                v.data = Val::Typed(NC_DOUBLE, (0..size).map(|k| k as f64 * 2.5).collect());
            }
            vars.push(v);
        }
    }
    // image-max / image-min.
    let mm = r.below(10);
    let mm_dims: Vec<i32> = match mm {
        0..=2 => Vec::new(),
        3..=5 => vec![0],
        6 if ndim >= 2 => vec![0, 1],
        7 if ndim >= 2 => vec![1],
        8 => vec![(ndim - 1) as i32],
        _ => Vec::new(),
    };
    let with_mm = mm != 9 && (mm >= 3 || chance(r, 50));
    if with_mm {
        let count: usize = mm_dims
            .iter()
            .map(|&d| {
                let s = dims[d as usize].1;
                if s == 0 { numrecs as usize } else { s as usize }
            })
            .product();
        let mut maxs = Vec::new();
        let mut mins = Vec::new();
        for _ in 0..count.max(1) {
            let a = match r.below(30) {
                0 => f64::NAN,
                1 => 0.0,
                _ => rand_f(r, -500.0, 2000.0),
            };
            let b = match r.below(25) {
                0 => a,
                1 => a + 1.0,
                _ => a - rand_f(r, 0.0, 1000.0),
            };
            maxs.push(a);
            mins.push(b);
        }
        let t = *pick(r, &[NC_DOUBLE, NC_DOUBLE, NC_DOUBLE, NC_FLOAT, NC_SHORT]);
        let only_one = chance(r, 5);
        vars.push(VarDef {
            name: b"image-max".to_vec(),
            dimids: mm_dims.clone(),
            atts: Vec::new(),
            ty: t,
            data: Val::Typed(t, maxs),
        });
        if !only_one {
            vars.push(VarDef {
                name: b"image-min".to_vec(),
                dimids: mm_dims,
                atts: Vec::new(),
                ty: t,
                data: Val::Typed(t, mins),
            });
        }
    }
    let total: usize = dims
        .iter()
        .map(|d| {
            if d.1 == 0 {
                numrecs as usize
            } else {
                d.1 as usize
            }
        })
        .product();
    let data: Vec<f64> = (0..total).map(|_| rand_value(r, ty, signed)).collect();
    let image = VarDef {
        name: b"image".to_vec(),
        dimids: (0..ndim as i32).collect(),
        atts: iatts,
        ty,
        data: Val::Typed(ty, data),
    };
    if chance(r, 20) {
        vars.insert(0, image);
    } else {
        vars.push(image);
    }
    let mut gatts = Vec::new();
    if chance(r, 50) {
        gatts.push(Att {
            name: b"history".to_vec(),
            val: Val::Text(b"made by the probe\n".to_vec()),
        });
    }
    Spec {
        version: if chance(r, 15) { 2 } else { 1 },
        numrecs,
        dims,
        gatts,
        vars,
    }
}

/// A case: the file (or what stands for it) and the name the read is given.
fn generate(r: &mut Rng, dir: &Path) -> Vec<u8> {
    let base = dir.join("case.mnc");
    let name = |p: &Path| p.to_string_lossy().into_owned().into_bytes();
    let kind = r.below(100);
    if kind < 62 {
        let spec = well_formed(r);
        let mut bytes = spec.write();
        if chance(r, 6) {
            let hdr = 32 + r.below(bytes.len() as u64) as usize;
            bytes.truncate(hdr.min(bytes.len()));
        }
        std::fs::write(&base, bytes).unwrap();
        return name(&base);
    }
    if kind < 82 {
        // A broken header: bytes of a well-formed file changed.
        let spec = well_formed(r);
        let mut bytes = spec.write();
        let n = 1 + r.below(4);
        for _ in 0..n {
            let at = r.below(bytes.len().min(400) as u64) as usize;
            bytes[at] = match r.below(4) {
                0 => 0,
                1 => 0xff,
                2 => bytes[at].wrapping_add(1),
                _ => r.below(256) as u8,
            };
        }
        if chance(r, 15) {
            let at = r.below(bytes.len() as u64) as usize;
            bytes.truncate(at);
        }
        std::fs::write(&base, bytes).unwrap();
        return name(&base);
    }
    match r.below(12) {
        0 => {
            std::fs::write(&base, b"").unwrap();
            name(&base)
        }
        1 => {
            std::fs::write(&base, b"this is not a netCDF file at all\n").unwrap();
            name(&base)
        }
        2 => {
            let mut b = b"\x89HDF\r\n\x1a\n".to_vec();
            b.resize(512, 0);
            std::fs::write(&base, b).unwrap();
            name(&base)
        }
        3 => {
            let spec = well_formed(r);
            let bytes = spec.write();
            let gz = dir.join("case.mnc.gz");
            let mut e = flate2_gz(&bytes);
            if chance(r, 10) {
                e.truncate(e.len() / 2);
            }
            std::fs::write(&gz, e).unwrap();
            name(&gz)
        }
        4 => {
            let d = dir.join("adir.mnc");
            std::fs::create_dir_all(&d).unwrap();
            name(&d)
        }
        5 => name(&dir.join("missing.mnc")),
        6 => {
            // A character or unknown-typed image.
            let mut spec = well_formed(r);
            if let Some(v) = spec.vars.iter_mut().find(|v| v.name == b"image") {
                let n = match &v.data {
                    Val::Typed(_, x) => x.len(),
                    _ => 0,
                };
                if chance(r, 60) {
                    v.ty = NC_CHAR;
                    v.data = Val::Text(vec![b'a'; n]);
                } else {
                    v.ty = 7 + r.below(3) as i32;
                    v.data = Val::Raw(v.ty, n as u32, vec![0; n]);
                }
            }
            std::fs::write(&base, spec.write()).unwrap();
            name(&base)
        }
        7 => {
            // Long or empty dimension names.
            let mut spec = well_formed(r);
            let i = r.below(spec.dims.len() as u64) as usize;
            spec.dims[i].0 = if chance(r, 50) {
                vec![b'q'; 65 + r.below(80) as usize]
            } else {
                Vec::new()
            };
            std::fs::write(&base, spec.write()).unwrap();
            name(&base)
        }
        8 => {
            // An attribute of a type netCDF does not know, or a scalar image-max of NaN.
            let mut spec = well_formed(r);
            if let Some(v) = spec.vars.iter_mut().find(|v| v.name == b"image") {
                v.atts.push(Att {
                    name: pick(r, &[&b"valid_range"[..], b"signtype", b"other"]).to_vec(),
                    val: Val::Raw(7 + r.below(4) as i32, r.below(3) as u32, vec![0; 8]),
                });
            }
            std::fs::write(&base, spec.write()).unwrap();
            name(&base)
        }
        9 => {
            // Global attributes named as the dimension attributes (read when a dimension has no
            // variable).
            let mut spec = well_formed(r);
            spec.vars
                .retain(|v| !matches!(v.name.as_slice(), b"xspace" | b"yspace" | b"zspace"));
            spec.gatts.push(Att {
                name: b"step".to_vec(),
                val: if chance(r, 50) {
                    Val::Text(b"x".to_vec())
                } else {
                    Val::Typed(NC_DOUBLE, vec![2.0])
                },
            });
            std::fs::write(&base, spec.write()).unwrap();
            name(&base)
        }
        10 => {
            // image-max over a dimension the image does not have.
            let mut spec = well_formed(r);
            spec.dims.push((b"other".to_vec(), 1 + r.below(2) as u32));
            let od = (spec.dims.len() - 1) as i32;
            spec.vars
                .retain(|v| v.name != b"image-max" && v.name != b"image-min");
            let n = spec.dims[od as usize].1 as usize;
            spec.vars.insert(
                0,
                VarDef {
                    name: b"image-max".to_vec(),
                    dimids: vec![od],
                    atts: Vec::new(),
                    ty: NC_DOUBLE,
                    data: Val::Typed(NC_DOUBLE, (0..n).map(|k| 100.0 + k as f64).collect()),
                },
            );
            spec.vars.insert(
                0,
                VarDef {
                    name: b"image-min".to_vec(),
                    dimids: vec![od],
                    atts: Vec::new(),
                    ty: NC_DOUBLE,
                    data: Val::Typed(NC_DOUBLE, vec![0.0; n]),
                },
            );
            std::fs::write(&base, spec.write()).unwrap();
            name(&base)
        }
        _ => {
            // A numeric signtype or spacetype, a character image-max.
            let mut spec = well_formed(r);
            match r.below(3) {
                0 => {
                    if let Some(v) = spec.vars.iter_mut().find(|v| v.name == b"xspace") {
                        v.atts.push(Att {
                            name: b"spacetype".to_vec(),
                            val: Val::Typed(NC_INT, vec![3.0]),
                        });
                    }
                }
                1 => {
                    if let Some(v) = spec.vars.iter_mut().find(|v| v.name == b"image-max") {
                        let n = match &v.data {
                            Val::Typed(_, x) => x.len(),
                            _ => 1,
                        };
                        v.ty = NC_CHAR;
                        v.data = Val::Text(vec![b'1'; n]);
                    }
                }
                _ => {
                    if let Some(v) = spec.vars.iter_mut().find(|v| v.name == b"image") {
                        v.atts.retain(|a| a.name != b"signtype");
                        v.atts.push(Att {
                            name: b"signtype".to_vec(),
                            val: Val::Typed(NC_DOUBLE, vec![0.0]),
                        });
                    }
                }
            }
            std::fs::write(&base, spec.write()).unwrap();
            name(&base)
        }
    }
}

fn flate2_gz(b: &[u8]) -> Vec<u8> {
    // A gzip member with stored blocks (no compression library in the probe).
    let mut o = vec![0x1f, 0x8b, 8, 0, 0, 0, 0, 0, 0, 255];
    let mut i = 0;
    loop {
        let n = (b.len() - i).min(65535);
        let last = i + n >= b.len();
        o.push(last as u8);
        o.extend_from_slice(&(n as u16).to_le_bytes());
        o.extend_from_slice(&(!(n as u16)).to_le_bytes());
        o.extend_from_slice(&b[i..i + n]);
        i += n;
        if last {
            break;
        }
    }
    o.extend_from_slice(&crc32(b).to_le_bytes());
    o.extend_from_slice(&(b.len() as u32).to_le_bytes());
    o
}

fn crc32(b: &[u8]) -> u32 {
    let mut c = 0xffff_ffffu32;
    for &x in b {
        c ^= x as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 {
                0xedb8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
        }
    }
    !c
}

// ---------------------------------------------------------------------------------------------
// The comparison.

/// The defects whose original behaviour depends on memory or crashes: such cases are counted,
/// not compared.
const UNDEFINED: &[&str] = &[
    "FS-774", "FS-776", "FS-777", "FS-778", "FS-779", "FS-782", "FS-783", "FS-784", "FS-785",
    "FS-789", "FS-790", "FS-792", "FS-795", "FS-796",
];
/// ... and those undefined in MINC 1.5.1 only (FreeSurfer's copy ends the program first).
const UNDEFINED_UP: &[&str] = &["FS-781"];

#[derive(Default)]
pub struct Tally {
    pub cases: u64,
    pub same: u64,
    pub read_ok: u64,
    pub differ: u64,
    pub skipped: u64,
    pub too_big: u64,
    pub shown: u32,
    pub outcomes: std::collections::BTreeMap<String, u64>,
    pub notices: std::collections::BTreeMap<String, u64>,
}

/// The random part of MINC's temporary names (`/tmp/minc-XXXXXX`).
fn mask_temp(b: &[u8]) -> Vec<u8> {
    let mut o = b.to_vec();
    let pat = b"/minc-";
    let mut i = 0;
    while i + pat.len() + 6 <= o.len() {
        if &o[i..i + pat.len()] == pat {
            for k in 0..6 {
                o[i + pat.len() + k] = b'X';
            }
            i += pat.len() + 6;
        } else {
            i += 1;
        }
    }
    o
}

/// The directory prefix of FreeSurfer's copy as its messages print it: read from a compressed
/// file's message.
pub fn fs_prefix(fs: &MincLib, work: &Path) -> String {
    let p = work.join("probe-prefix.mnc.gz");
    std::fs::write(&p, flate2_gz(b"CDF\x01")).unwrap();
    let (_, _, err) = fs.run(p.to_string_lossy().as_bytes(), 5000, -1);
    let t = String::from_utf8_lossy(&err).into_owned();
    match t.find("minc_netcdf_convenience.c:") {
        Some(i) => t[..i].to_string(),
        None => panic!("no prefix in FreeSurfer's message {t:?}"),
    }
}

/// The files of `dir` (sorted by name) through MINC 1.5.1 and the port: the SHA-256 of the
/// original's dumps hashed as larmorx-io's `seeded_files_as_minc` test hashes the port's, and
/// whether the port's dumps are the same.
pub fn files(dir: &Path, up: &MincLib) {
    use sha2::{Digest, Sha256};
    let mut names: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    let mut h = Sha256::new();
    let mut same = 0;
    for n in &names {
        let path = dir.join(n);
        let pb = path.to_string_lossy().into_owned().into_bytes();
        let (oc, _out, err) = up.run(&pb, 5000, -1);
        let (d, exit) = match &oc {
            Outcome::Returned(d) => (d.clone(), None),
            Outcome::Exit(s) => (String::new(), Some(*s)),
            x => panic!("{n}: {x:?}"),
        };
        h.update(format!("{n} {exit:?} {}\n{d}", err.len()).as_bytes());
        let p = port(&pb, UpstreamDefects::Reproduce, &minc::Mni);
        if p.dump.as_deref() == Some(d.as_str()) && p.exit == exit && p.stderr == err {
            same += 1;
        } else {
            println!("  differs: {n}");
        }
    }
    let hex: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
    println!(
        "files: {} files, port identical on {same}; MINC 1.5.1's dumps sha256 {hex}",
        names.len()
    );
}

/// One file through both originals and the port, every output printed (`LXPROBE_ONE`).
pub fn one(name: &[u8], up: &MincLib, fs: &MincLib, work: &Path) {
    let fsd = FreeSurferDialect {
        prefix: fs_prefix(fs, work),
    };
    for (lib, side) in [(up, "up"), (fs, "fs")] {
        let dialect: &dyn Dialect = if side == "up" { &minc::Mni } else { &fsd };
        let p = port(name, UpstreamDefects::Reproduce, dialect);
        let (oc, out, err) = lib.run(name, 5000, -1);
        println!(
            "== {side} original {oc:?}\n   stdout {:?}\n   stderr {:?}",
            String::from_utf8_lossy(&out),
            String::from_utf8_lossy(&err)
        );
        println!(
            "== {side} port exit {:?} notices {:?}\n   stdout {:?}\n   stderr {:?}\n{}",
            p.exit,
            p.notices,
            String::from_utf8_lossy(&p.stdout),
            String::from_utf8_lossy(&p.stderr),
            p.dump.unwrap_or_default()
        );
    }
}

pub fn run(seed: u64, count: u64, work: &Path, up: &MincLib, fs: &MincLib, t: &mut Tally) {
    let mut r = Rng::new(seed ^ 0x6d69_6e63);
    let show: u32 = std::env::var("LXPROBE_SHOW")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10);
    let fsd = FreeSurferDialect {
        prefix: fs_prefix(fs, work),
    };
    for case in 0..count {
        let dir = work.join(format!("m{case}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let name = generate(&mut r, &dir);
        t.cases += 1;
        // A header announcing a large image: the originals (and the port) would allocate and
        // read it whole; not run.
        if minc::header_image_size(&name).is_some_and(|n| n > 1 << 22) {
            t.too_big += 1;
            let _ = std::fs::remove_dir_all(&dir);
            continue;
        }
        let cache_variant = chance(&mut r, 10);
        let mut keep = false;
        for (lib, side) in [(up, "up"), (fs, "fs")] {
            let cache_on = MniCache(0);
            let dialect: &dyn Dialect = if side == "up" && cache_variant {
                &cache_on
            } else if side == "up" {
                &minc::Mni
            } else {
                &fsd
            };
            NOT_MODELLED.with(|f| f.set(false));
            LIMIT_HIT.with(|l| l.set(false));
            let p = port(&name, UpstreamDefects::Reproduce, dialect);
            let not_modelled = NOT_MODELLED.with(|f| f.get());
            let limit_hit = LIMIT_HIT.with(|l| l.get());
            let pf = port(&name, UpstreamDefects::Fix, dialect);
            for id in &pf.notices {
                *t.notices.entry(format!("{side} {id}")).or_default() += 1;
            }
            let cache = if side == "up" && cache_variant { 0 } else { -1 };
            let (oc, out, err) = lib.run(&name, 5000, cache);
            let label = match &oc {
                Outcome::Returned(_) => "returned".to_string(),
                Outcome::Exit(s) => format!("exit {s}"),
                Outcome::Signal(s) => format!("signal {s}"),
                Outcome::Hang => "hang".to_string(),
            };
            let undefined: Vec<String> = p
                .notices
                .iter()
                .filter(|n| {
                    UNDEFINED.contains(&n.as_str())
                        || (side == "up" && UNDEFINED_UP.contains(&n.as_str()))
                })
                .cloned()
                .collect();
            if cache_variant && side == "up" && p.notices.iter().any(|n| n == "FS-793") {
                // The cache reads a short file's blocks in the order the caller asks for voxels:
                // not followed by the port (it reads slabs).
                *t.outcomes
                    .entry(format!("{side} FS-793 with the cache (not modelled) -> {label}"))
                    .or_default() += 1;
                t.skipped += 1;
                continue;
            }
            if limit_hit {
                *t.outcomes
                    .entry(format!("{side} more than 100,000 slabs (the shim's limit: not read to the end) -> {label}"))
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
            if !undefined.is_empty() {
                *t.outcomes
                    .entry(format!("{side} {} -> {label}", undefined.join("+")))
                    .or_default() += 1;
                t.skipped += 1;
                continue;
            }
            let err_m = mask_temp(&err);
            let perr = mask_temp(&p.stderr);
            let ok = match (&p.dump, p.exit, &oc) {
                (Some(d), None, Outcome::Returned(o)) => {
                    *d == *o && err_m == perr && out == p.stdout
                }
                (None, Some(s), Outcome::Exit(so)) => s == *so && err_m == perr && out == p.stdout,
                _ => false,
            };
            if ok {
                t.same += 1;
                if let Some(d) = &p.dump
                    && d.starts_with("status 0\n")
                {
                    t.read_ok += 1;
                }
            } else {
                t.differ += 1;
                if t.shown < show {
                    t.shown += 1;
                    keep = true;
                    println!(
                        "  DIFF minc seed {seed} case {case} ({side}{}): kept in {}",
                        if cache != -1 { ", cache" } else { "" },
                        work.join(format!("keep-{seed}-{case}")).display()
                    );
                    println!(
                        "    port     exit {:?} stderr {:?} notices {:?}\n      dump {:?}",
                        p.exit,
                        String::from_utf8_lossy(&p.stderr),
                        p.notices,
                        p.dump.as_deref().map(|d| &d[..d.len().min(1500)])
                    );
                    let od = match &oc {
                        Outcome::Returned(o) => o[..o.len().min(1500)].to_string(),
                        x => format!("{x:?}"),
                    };
                    println!(
                        "    original {label} stdout {:?} stderr {:?}\n      dump {od:?}",
                        String::from_utf8_lossy(&out),
                        String::from_utf8_lossy(&err)
                    );
                }
            }
        }
        if keep {
            let k = work.join(format!("keep-{seed}-{case}"));
            let _ = std::fs::remove_dir_all(&k);
            let _ = std::fs::rename(&dir, &k);
        } else {
            let _ = std::fs::remove_dir_all(&dir);
        }
    }
}
