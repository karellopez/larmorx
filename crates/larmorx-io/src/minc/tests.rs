// SPDX-License-Identifier: Apache-2.0
//! Unit tests of the MINC volume reader: hand-checked values of MINC 1.5.1's reads, and the
//! dumps (every field, every double's bits, every voxel) of 120 seeded files whose originals'
//! dumps were recorded (`validation/probes/fs-nrrd-minc`: `LARMORX_MINC_EXPORT=<dir>` writes the
//! files, the probe's `--files <dir>` prints MINC 1.5.1's dumps of them).

use std::fmt::Write as _;

use larmorx_core::cfmt::Streams;
use larmorx_core::defects::{Notices, UpstreamDefects};
use sha2::{Digest, Sha256};

use super::*;

const BYTE: i32 = 1;
const CHAR: i32 = 2;
const SHORT: i32 = 3;
const INT: i32 = 4;
const FLOAT: i32 = 5;
const DOUBLE: i32 = 6;

#[derive(Clone)]
enum V {
    N(i32, Vec<f64>),
    T(Vec<u8>),
}

fn enc(t: i32, x: &[f64]) -> Vec<u8> {
    let mut o = Vec::new();
    for &v in x {
        match t {
            BYTE => o.push(v as i64 as i8 as u8),
            SHORT => o.extend_from_slice(&(v as i64 as i16).to_be_bytes()),
            INT => o.extend_from_slice(&(v as i64 as i32).to_be_bytes()),
            FLOAT => o.extend_from_slice(&(v as f32).to_bits().to_be_bytes()),
            DOUBLE => o.extend_from_slice(&v.to_bits().to_be_bytes()),
            _ => {}
        }
    }
    o
}

fn pad(o: &mut Vec<u8>) {
    while !o.len().is_multiple_of(4) {
        o.push(0);
    }
}

fn name(o: &mut Vec<u8>, n: &[u8]) {
    o.extend_from_slice(&(n.len() as u32).to_be_bytes());
    o.extend_from_slice(n);
    pad(o);
}

fn atts(o: &mut Vec<u8>, a: &[(&[u8], V)]) {
    if a.is_empty() {
        o.extend_from_slice(&[0; 8]);
        return;
    }
    o.extend_from_slice(&12u32.to_be_bytes());
    o.extend_from_slice(&(a.len() as u32).to_be_bytes());
    for (n, v) in a {
        name(o, n);
        let (t, cnt, mut b) = match v {
            V::N(t, x) => (*t, x.len(), enc(*t, x)),
            V::T(s) => (CHAR, s.len(), s.clone()),
        };
        o.extend_from_slice(&(t as u32).to_be_bytes());
        o.extend_from_slice(&(cnt as u32).to_be_bytes());
        pad(&mut b);
        o.extend_from_slice(&b);
    }
}

/// A variable: name, dimension ids, attributes, type, values.
type Var<'a> = (&'a [u8], Vec<u32>, Vec<(&'a [u8], V)>, i32, Vec<f64>);

fn tsize(t: i32) -> usize {
    match t {
        BYTE | CHAR => 1,
        SHORT => 2,
        INT | FLOAT => 4,
        _ => 8,
    }
}

/// A netCDF classic file without record variables.
fn nc(dims: &[(&[u8], u32)], vars: &[Var]) -> Vec<u8> {
    let vsize = |v: &Var| {
        let n: usize = v.1.iter().map(|&d| dims[d as usize].1 as usize).product();
        (n * tsize(v.3)).div_ceil(4) * 4
    };
    let header = |begins: &[u32]| {
        let mut o = b"CDF\x01".to_vec();
        o.extend_from_slice(&0u32.to_be_bytes());
        o.extend_from_slice(&10u32.to_be_bytes());
        o.extend_from_slice(&(dims.len() as u32).to_be_bytes());
        for (n, s) in dims {
            name(&mut o, n);
            o.extend_from_slice(&s.to_be_bytes());
        }
        o.extend_from_slice(&[0; 8]);
        o.extend_from_slice(&11u32.to_be_bytes());
        o.extend_from_slice(&(vars.len() as u32).to_be_bytes());
        for (v, b) in vars.iter().zip(begins) {
            name(&mut o, v.0);
            o.extend_from_slice(&(v.1.len() as u32).to_be_bytes());
            for d in &v.1 {
                o.extend_from_slice(&d.to_be_bytes());
            }
            atts(&mut o, &v.2);
            o.extend_from_slice(&(v.3 as u32).to_be_bytes());
            o.extend_from_slice(&(vsize(v) as u32).to_be_bytes());
            o.extend_from_slice(&b.to_be_bytes());
        }
        o
    };
    let h = header(&vec![0; vars.len()]).len() as u32;
    let mut begins = Vec::new();
    let mut at = h;
    for v in vars {
        begins.push(at);
        at += vsize(v) as u32;
    }
    let mut o = header(&begins);
    for v in vars {
        let mut b = enc(v.3, &v.4);
        b.resize(vsize(v), 0);
        o.extend_from_slice(&b);
    }
    o
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

fn dbl(v: f64) -> String {
    format!(" {:016x}", v.to_bits())
}

fn cs(b: &[u8]) -> String {
    let b = netcdf::cstr(b);
    let mut s = " s".to_string();
    for c in b {
        let _ = write!(s, "{c:02x}");
    }
    s
}

/// FreeSurfer 8.2.0's mincRead sequence with the port, dumped as the probe's shim dumps it.
fn dump(path: &std::path::Path, mode: UpstreamDefects) -> (String, Option<i32>, Vec<u8>, Notices) {
    let mut cap = Cap::default();
    let mut notices = Notices::new(mode);
    let mut s = Session::new();
    let mut o = String::new();
    let name = path.to_string_lossy().into_owned().into_bytes();
    let r = {
        let mut c = Ctx {
            streams: &mut cap,
            notices: &mut notices,
            dialect: &Mni,
        };
        (|| -> Result<(), Exit> {
            let names: Vec<Vec<u8>> = [&b"xspace"[..], b"yspace", b"zspace", b"time"]
                .iter()
                .map(|x| x.to_vec())
                .collect();
            let mut vol = None;
            let (st, input) = start_volume_input(
                &mut s,
                &mut c,
                &name,
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
            let _ = writeln!(o, "status {}", st as i32);
            if st != Status::Ok {
                return Ok(());
            }
            let mut input = input.unwrap();
            let v = vol.as_mut().unwrap();
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
            for (label, a) in [("\nsep", &v.separations), ("\nstarts", &v.starts)] {
                o.push_str(label);
                for k in 0..n {
                    o.push_str(&dbl(a[k]));
                }
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
            for row in m {
                for x in row {
                    o.push_str(&dbl(x));
                }
            }
            let mut voxel = [0.0f64; 5];
            if n >= 3 {
                for k in 0..3 {
                    voxel[k] = v.sizes[k] as f64 / 2.0;
                }
            }
            let w = v.convert_voxel_to_world(&voxel, c.dialect, c.streams);
            let _ = write!(o, "\nworld_c{}{}{}", dbl(w[0]), dbl(w[1]), dbl(w[2]));
            let w = v.convert_voxel_to_world(&[0.0; 5], c.dialect, c.streams);
            let _ = write!(o, "\nworld_0{}{}{}", dbl(w[0]), dbl(w[1]), dbl(w[2]));
            o.push_str("\nmore");
            let mut f;
            loop {
                let (more, frac) = input_more_of_volume(&mut s, &mut c, v, &mut input)?;
                f = frac;
                if !more {
                    break;
                }
                o.push_str(&dbl(f));
            }
            o.push_str(&dbl(f));
            let _ = write!(o, "\ncached_after {}\ndata", v.is_cached_volume as i32);
            let total: i64 = v.sizes.iter().map(|&x| x as i64).product();
            if n >= 1 && total > 0 {
                let mut idx = vec![0usize; n];
                'all: loop {
                    o.push_str(&dbl(v.voxel_value(&idx)));
                    for i in (0..n).rev() {
                        idx[i] += 1;
                        if idx[i] < v.sizes[i] as usize {
                            continue 'all;
                        }
                        idx[i] = 0;
                    }
                    break;
                }
            }
            o.push('\n');
            delete_volume_input(&mut s, &mut c, input)?;
            Ok(())
        })()
    };
    (o, r.err().map(|e| e.status), cap.err, notices)
}

fn write_tmp(dir: &std::path::Path, n: &str, bytes: &[u8]) -> std::path::PathBuf {
    let p = dir.join(n);
    std::fs::write(&p, bytes).unwrap();
    p
}

fn img3(t: i32, atts: Vec<(&'static [u8], V)>, vals: Vec<f64>) -> Vec<u8> {
    nc(
        &[(b"zspace", 2), (b"yspace", 3), (b"xspace", 4)],
        &[(b"image", vec![0, 1, 2], atts, t, vals)],
    )
}

/// MINC 1.5.1's read of a short volume with per-slice ranges and geometry (the values its
/// volume_io gives, from the probe's shim).
#[test]
fn per_slice_scaling_and_geometry() {
    let dir = tempfile::tempdir().unwrap();
    let data: Vec<f64> = (-12..12).map(|x| x as f64).collect();
    let bytes = nc(
        &[(b"zspace", 2), (b"yspace", 3), (b"xspace", 4)],
        &[
            (
                b"xspace",
                vec![],
                vec![
                    (b"step", V::N(DOUBLE, vec![1.5])),
                    (b"start", V::N(DOUBLE, vec![-10.0])),
                    (b"direction_cosines", V::N(DOUBLE, vec![1.0, 0.0, 0.0])),
                ],
                INT,
                vec![0.0],
            ),
            (
                b"yspace",
                vec![],
                vec![
                    (b"step", V::N(DOUBLE, vec![-2.0])),
                    (b"start", V::N(DOUBLE, vec![20.0])),
                ],
                INT,
                vec![0.0],
            ),
            (
                b"zspace",
                vec![],
                vec![
                    (b"step", V::N(DOUBLE, vec![3.0])),
                    (b"start", V::N(DOUBLE, vec![5.0])),
                    (b"direction_cosines", V::N(DOUBLE, vec![0.0, 0.1, 0.99])),
                ],
                INT,
                vec![0.0],
            ),
            (b"image-max", vec![0], vec![], DOUBLE, vec![100.0, 200.0]),
            (b"image-min", vec![0], vec![], DOUBLE, vec![-50.0, 0.0]),
            (
                b"image",
                vec![0, 1, 2],
                vec![
                    (b"signtype", V::T(b"signed__".to_vec())),
                    (b"valid_range", V::N(DOUBLE, vec![-12.0, 11.0])),
                ],
                SHORT,
                data,
            ),
        ],
    );
    let p = write_tmp(dir.path(), "a.mnc", &bytes);
    let (d, exit, err, _) = dump(&p, UpstreamDefects::Reproduce);
    assert_eq!(exit, None);
    assert!(err.is_empty(), "{}", String::from_utf8_lossy(&err));
    let line = |k: &str| {
        d.lines()
            .find(|l| l.starts_with(k))
            .unwrap()
            .split_whitespace()
            .skip(1)
            .map(|h| f64::from_bits(u64::from_str_radix(h, 16).unwrap()))
            .collect::<Vec<f64>>()
    };
    assert_eq!(line("rrange"), vec![-50.0, 200.0]);
    assert_eq!(line("world_c"), vec![-7.0, 17.8, 7.92]);
    assert_eq!(line("world_0"), vec![-10.0, 20.5, 4.95]);
    assert_eq!(
        line("data"),
        vec![
            -12.0, 2.0, -10.0, 5.0, -7.0, 9.0, -11.0, 3.0, -9.0, 6.0, -7.0, 9.0, -11.0, 4.0, -8.0,
            7.0, -6.0, 10.0, -10.0, 5.0, -8.0, 8.0, -5.0, 11.0
        ]
    );
}

/// The messages of a file that is not netCDF, and of dimension names that do not match.
#[test]
fn failure_messages() {
    let dir = tempfile::tempdir().unwrap();
    let p = write_tmp(dir.path(), "x.mnc", b"not netcdf");
    let (d, exit, err, _) = dump(&p, UpstreamDefects::Reproduce);
    assert_eq!((d.as_str(), exit), ("status 1\n", None));
    let n = p.to_string_lossy();
    assert_eq!(
        String::from_utf8_lossy(&err),
        format!(
            "ncopen: filename \"{n}\": Not a netCDF file\nmiopen: MINC package entry point\nError opening {n}\n"
        )
    );
    let bytes = nc(
        &[(b"time", 2), (b"yspace", 3), (b"xspace", 4)],
        &[(b"image", vec![0, 1, 2], vec![], SHORT, vec![1.0; 24])],
    );
    let p = write_tmp(dir.path(), "t.mnc", &bytes);
    let (_, _, err, _) = dump(&p, UpstreamDefects::Reproduce);
    assert_eq!(
        String::from_utf8_lossy(&err),
        "Error:  dimension names did not match: \n\nRequested:\n1: xspace\n2: yspace\n3: zspace\n\nIn File:\n1: time\n2: yspace\n3: xspace\n"
    );
}

/// A file shorter than its header: past its end netCDF's buffer gives an earlier block's bytes
/// (FS-793, `reproduce`), zeros otherwise.
#[test]
fn truncated_file_past_its_end() {
    let dir = tempfile::tempdir().unwrap();
    let n = 13 * 24 * 31;
    let vals: Vec<f64> = (0..n).map(|k| (k % 200) as f64 + 1.0).collect();
    let mut bytes = nc(
        &[(b"zspace", 31), (b"yspace", 24), (b"xspace", 13)],
        &[(b"image", vec![0, 1, 2], vec![], INT, vals)],
    );
    bytes.truncate(20000);
    let p = write_tmp(dir.path(), "t.mnc", &bytes);
    let (fix, _, _, nf) = dump(&p, UpstreamDefects::Fix);
    let (rep, _, _, nr) = dump(&p, UpstreamDefects::Reproduce);
    assert!(nf.contains(&larmorx_core::defects::FS_793));
    assert!(nr.contains(&larmorx_core::defects::FS_793));
    assert_ne!(fix, rep);
}

/// Seeded files of every type, sign, valid range and slice range, dumped: their SHA-256 equals
/// that of MINC 1.5.1's own dumps (recorded with the probe).
#[test]
fn seeded_files_as_minc() {
    let dir = tempfile::tempdir().unwrap();
    let export = std::env::var_os("LARMORX_MINC_EXPORT");
    let mut r = 0x5eed_u64;
    let mut next = || {
        r = r.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = r;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    };
    let mut h = Sha256::new();
    for case in 0..120 {
        let t = [BYTE, SHORT, INT, FLOAT, DOUBLE][(next() % 5) as usize];
        let signed = next() % 2 == 0;
        let (lo, hi) = match (t, signed) {
            (BYTE, true) => (-128.0, 127.0),
            (BYTE, false) => (0.0, 255.0),
            (SHORT, true) => (-32768.0, 32767.0),
            (SHORT, false) => (0.0, 65535.0),
            (INT, true) => (-2147483648.0, 2147483647.0),
            (INT, false) => (0.0, 4294967295.0),
            _ => (-1000.0, 1000.0),
        };
        let val = |u: u64| lo + (hi - lo) * ((u % 1000) as f64 / 999.0);
        let stored = |x: f64| match t {
            BYTE => (x.round() as i64 as u8 as i8) as f64,
            SHORT => (x.round() as i64 as u16 as i16) as f64,
            INT => (x.round() as i64 as u32 as i32) as f64,
            FLOAT => (x as f32) as f64,
            _ => x,
        };
        let mut a: Vec<(&'static [u8], V)> = vec![(
            b"signtype",
            V::T(if signed {
                b"signed__".to_vec()
            } else {
                b"unsigned".to_vec()
            }),
        )];
        match next() % 4 {
            0 => {
                let (x, y) = (val(next()), val(next()));
                a.push((b"valid_range", V::N(DOUBLE, vec![x.min(y), x.max(y)])));
            }
            1 => a.push((b"valid_max", V::N(DOUBLE, vec![val(next())]))),
            _ => {}
        }
        let vals: Vec<f64> = (0..24).map(|_| stored(val(next()))).collect();
        let mut vars: Vec<Var> = Vec::new();
        if next() % 3 != 0 {
            let mx: Vec<f64> = (0..2).map(|_| (next() % 2000) as f64 - 500.0).collect();
            let mn: Vec<f64> = mx.iter().map(|m| m - (next() % 700) as f64).collect();
            vars.push((b"image-max", vec![0], vec![], DOUBLE, mx));
            vars.push((b"image-min", vec![0], vec![], DOUBLE, mn));
        }
        vars.push((b"image", vec![0, 1, 2], a, t, vals));
        let bytes = nc(&[(b"zspace", 2), (b"yspace", 3), (b"xspace", 4)], &vars);
        let name = format!("s{case:03}.mnc");
        let p = write_tmp(dir.path(), &name, &bytes);
        if let Some(e) = &export {
            std::fs::create_dir_all(e).unwrap();
            std::fs::write(std::path::Path::new(e).join(&name), &bytes).unwrap();
        }
        let (d, exit, err, _) = dump(&p, UpstreamDefects::Reproduce);
        h.update(format!("{name} {exit:?} {}\n{d}", err.len()).as_bytes());
    }
    let got: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
    // MINC 1.5.1's dumps of the same 120 files (the probe's --files), hashed the same way.
    assert_eq!(got, SEEDED_SHA256, "the dumps' SHA-256");
}

/// The SHA-256 of MINC 1.5.1's dumps of [`seeded_files_as_minc`]'s files.
const SEEDED_SHA256: &str = "c96915507a07c679e24d29c32397d68ce1ddcc485b7997b6605062d72286de47";

/// A character image: refused by default (FS-781).
#[test]
fn char_image_refused() {
    let dir = tempfile::tempdir().unwrap();
    let mut bytes = img3(SHORT, vec![], vec![0.0; 24]);
    // The image's type field: the 4 bytes before its vsize and begin.
    let n = bytes.len();
    let at = n - 48 - 12;
    bytes[at..at + 4].copy_from_slice(&(CHAR as u32).to_be_bytes());
    let p = write_tmp(dir.path(), "c.mnc", &bytes);
    let (d, _, err, notices) = dump(&p, UpstreamDefects::Fix);
    assert_eq!(d, "status 1\n");
    assert!(notices.contains(&larmorx_core::defects::FS_781));
    assert!(String::from_utf8_lossy(&err).contains("is not a numeric type"));
}

/// One x slice of a `zspace, yspace, xspace` file read as x, y, z: volume_io reads each z slab as one
/// run into the volume (FS-794; FS-796 for the voxels no slab wrote, with `reproduce`).
#[test]
fn one_slice_slabs_in_their_place() {
    let dir = tempfile::tempdir().unwrap();
    let vals: Vec<f64> = (0..20000).map(|k| k as f64).collect();
    let bytes = nc(
        &[(b"zspace", 2), (b"yspace", 10000), (b"xspace", 1)],
        &[(
            b"image",
            vec![0, 1, 2],
            vec![
                (b"signtype", V::T(b"signed__".to_vec())),
                (b"valid_range", V::N(DOUBLE, vec![0.0, 30000.0])),
            ],
            SHORT,
            vals,
        )],
    );
    let p = write_tmp(dir.path(), "slice.mnc", &bytes);
    let read = |mode| {
        let mut cap = Cap::default();
        let mut notices = Notices::new(mode);
        let mut s = Session::new();
        let name = p.to_string_lossy().into_owned().into_bytes();
        let mut c = Ctx {
            streams: &mut cap,
            notices: &mut notices,
            dialect: &Mni,
        };
        let names: Vec<Vec<u8>> = [&b"xspace"[..], b"yspace", b"zspace"]
            .iter()
            .map(|x| x.to_vec())
            .collect();
        let mut vol = None;
        let (st, input) = start_volume_input(
            &mut s,
            &mut c,
            &name,
            0,
            Some(&names),
            0,
            false,
            0.0,
            0.0,
            true,
            &mut vol,
            None,
        )
        .unwrap();
        assert_eq!(st, Status::Ok);
        let mut input = input.unwrap();
        let v = vol.as_mut().unwrap();
        while input_more_of_volume(&mut s, &mut c, v, &mut input)
            .unwrap()
            .0
        {}
        let d = v.data.clone().unwrap();
        (d, notices)
    };
    let (fixed, nf) = read(UpstreamDefects::Fix);
    assert!(nf.contains(&larmorx_core::defects::FS_794));
    for (y, z) in [(0, 0), (1, 0), (9999, 1), (5000, 1)] {
        assert_eq!(fixed.get(y * 2 + z), (z * 10000 + y) as f64);
    }
    let (runs, nr) = read(UpstreamDefects::Reproduce);
    assert!(nr.contains(&larmorx_core::defects::FS_794));
    assert!(nr.contains(&larmorx_core::defects::FS_796));
    for p in [0usize, 1, 2, 5000, 10000, 10001, 19999] {
        let want = match p {
            0 => 0.0,
            1..=10000 => (10000 + p - 1) as f64,
            _ => 0.0,
        };
        assert_eq!(runs.get(p), want, "voxel {p}");
    }
}
