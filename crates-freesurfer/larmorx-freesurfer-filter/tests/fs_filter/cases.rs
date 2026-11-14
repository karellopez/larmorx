// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The fs.filter probe's cases (`validation/probes/fs-filter/`): a seeded generator of case lines,
//! and the port's answer to each, printed as `fsfilter` prints FreeSurfer's (`<result> | <text>`,
//! the text FreeSurfer's code wrote C-escaped; `EXIT <status> | <text>` when it ended the program).
#![allow(dead_code)]

use std::cell::RefCell;
use std::io::Write;
use std::rc::Rc;

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_filter::fastmarch as fmm;
use larmorx_freesurfer_io::identify::MRI_VOLUME_TYPE_UNKNOWN;
use larmorx_freesurfer_io::mriio::mri_read;
use larmorx_freesurfer_mri::mri::{
    MRI_FLOAT, MRI_INT, MRI_LONG, MRI_SHORT, MRI_UCHAR, MRI_USHRT, Mri, mri_alloc_sequence,
};

#[path = "conv.rs"]
mod conv;

/// The cases' random numbers.
pub struct Gen(u64);

impl Gen {
    pub fn new(seed: u64) -> Gen {
        Gen(seed)
    }
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn int(&mut self, lo: i64, hi: i64) -> i64 {
        lo + self.below((hi - lo + 1) as u64) as i64
    }
    fn pick<T: Copy>(&mut self, xs: &[T]) -> T {
        xs[self.below(xs.len() as u64) as usize]
    }
    fn chance(&mut self, k: u64) -> bool {
        self.below(k) == 0
    }
}

fn fx(v: f32) -> String {
    format!("{:08x}", v.to_bits())
}

/// Run-length tokens (`v*k`) of a voxel list.
fn rle(vals: &[String]) -> Vec<String> {
    let mut t = Vec::new();
    let mut i = 0;
    while i < vals.len() {
        let mut k = 1;
        while i + k < vals.len() && vals[i + k] == vals[i] {
            k += 1;
        }
        t.push(if k > 1 {
            format!("{}*{k}", vals[i])
        } else {
            vals[i].clone()
        });
        i += k;
    }
    t
}

/// A voxel's token for type `ty`, from an integer value (the integer types) or a float.
fn token(ty: i32, v: f64) -> String {
    match ty {
        MRI_FLOAT => fx(v as f32),
        MRI_UCHAR => ((v as i64).clamp(0, 255)).to_string(),
        MRI_SHORT => ((v as i64).clamp(-32768, 32767)).to_string(),
        MRI_USHRT => ((v as i64).clamp(0, 65535)).to_string(),
        _ => (v as i64).to_string(),
    }
}

fn extent(g: &mut Gen) -> [i32; 3] {
    if g.chance(40) {
        return [
            g.int(20, 32) as i32,
            g.int(20, 32) as i32,
            g.int(18, 32) as i32,
        ];
    }
    if g.chance(10) {
        return [
            g.int(10, 18) as i32,
            g.int(9, 16) as i32,
            g.int(8, 14) as i32,
        ];
    }
    let mut e = [0; 3];
    for x in &mut e {
        *x = if g.chance(9) { 1 } else { g.int(2, 8) as i32 };
    }
    e
}

/// A label volume: boxes and balls of `label` on a background, with scattered other values.
#[allow(clippy::too_many_arguments)]
fn label_vol(g: &mut Gen, t: &mut Vec<String>, e: [i32; 3], ty: i32, nf: i32, label: i64, xs: f32) {
    let [w, h, d] = e;
    t.extend([w, h, d, ty, nf].map(|v| v.to_string()));
    for _ in 0..3 {
        t.push(fx(xs));
    }
    let n = (w * h * d) as usize;
    let back = g.pick(&[0i64, 0, 0, 2, 255, -1]);
    let mut v = vec![back as f64; n];
    for _ in 0..g.int(0, 3) {
        let c = [
            g.int(0, i64::from(w) - 1),
            g.int(0, i64::from(h) - 1),
            g.int(0, i64::from(d) - 1),
        ];
        let r = g.int(0, i64::from(w.min(h).min(d)).max(4) / 2 + 2);
        let ball = g.chance(2);
        for z in 0..d {
            for y in 0..h {
                for x in 0..w {
                    let dd = [
                        i64::from(x) - c[0],
                        i64::from(y) - c[1],
                        i64::from(z) - c[2],
                    ];
                    let inside = if ball {
                        dd.iter().map(|q| q * q).sum::<i64>() <= r * r
                    } else {
                        dd.iter().all(|q| q.abs() <= r)
                    };
                    if inside {
                        v[((z * h + y) * w + x) as usize] = label as f64;
                    }
                }
            }
        }
    }
    for _ in 0..g.int(0, 4) {
        let i = g.below(n as u64) as usize;
        v[i] = g.pick(&[0.0, 1.0, 3.0, 128.0, label as f64]);
    }
    let mut vals: Vec<String> = v.iter().map(|&x| token(ty, x)).collect();
    if ty == MRI_FLOAT {
        // values that round to the label or past it, and those with no integer
        for _ in 0..g.int(0, 5) {
            let i = g.below(n as u64) as usize;
            let l = label as f32;
            let f = g.pick(&[
                l + 0.4,
                l - 0.4,
                l + 0.5,
                l - 0.5,
                l + 0.49999997,
                f32::NAN,
                f32::INFINITY,
                f32::NEG_INFINITY,
                3e9,
                -3e9,
                -0.0,
                2147483520.0,
            ]);
            vals[i] = fx(f);
        }
    }
    let frames: Vec<String> = (0..nf)
        .flat_map(|f| {
            if f == 0 {
                vals.clone()
            } else {
                vec![token(ty, 7.0); n]
            }
        })
        .collect();
    t.extend(rle(&frames));
}

/// A float volume of `nf` frames filled with one value (a destination).
fn fill_vol(g: &mut Gen, t: &mut Vec<String>, e: [i32; 3], ty: i32, nf: i32) {
    t.extend(e.map(|v| v.to_string()));
    t.push(ty.to_string());
    t.push(nf.to_string());
    for _ in 0..3 {
        t.push(fx(1.0));
    }
    let n = (e[0] * e[1] * e[2] * nf) as usize;
    let v = token(ty, g.pick(&[0.0, 9.0, -3.5, 100.0]));
    t.push(format!("{v}*{n}"));
}

fn max_distance(g: &mut Gen) -> f32 {
    if g.chance(12) {
        return g.pick(&[f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 1e30, 1e-3, -0.0]);
    }
    g.pick(&[
        -1.0, -1.0, 0.0, 3.0, 5.0, 10.0, 100.0, 2.5, 1.0, 0.5, 7.25, 4.0,
    ])
}

fn dist_case(g: &mut Gen, t: &mut Vec<String>) {
    let edm = g.chance(2);
    t.push(if edm { "edm" } else { "dtr" }.into());
    let e = extent(g);
    let ty = if g.chance(3) {
        MRI_FLOAT
    } else {
        g.pick(&[
            MRI_UCHAR, MRI_UCHAR, MRI_UCHAR, MRI_SHORT, MRI_INT, MRI_USHRT, MRI_LONG,
        ])
    };
    let nf = if g.chance(8) { 2 } else { 1 };
    let label = g.pick(&[1i64, 1, 1, 3, 128, 255, 2]);
    let xs = if g.chance(10) {
        g.pick(&[f32::NAN, -1.0, 0.0, f32::INFINITY])
    } else {
        g.pick(&[1.0, 1.0, 0.5, 2.0, 1.25, 0.9375])
    };
    label_vol(g, t, e, ty, nf, label, xs);
    // the destination
    match g.below(20) {
        0..=13 => t.push("0".into()),
        14..=17 => {
            t.push("1".into());
            let nf = if g.chance(3) { 2 } else { 1 };
            fill_vol(g, t, e, MRI_FLOAT, nf);
        }
        _ => {
            t.push("1".into());
            let mut e2 = e;
            let ty2 = if g.chance(2) {
                MRI_UCHAR
            } else {
                let k = g.below(3) as usize;
                e2[k] += 1;
                MRI_FLOAT
            };
            fill_vol(g, t, e2, ty2, 1);
        }
    }
    let l = if g.chance(8) {
        g.pick(&[0i64, -1, 300, 7, -2147483648])
    } else {
        label
    };
    t.push(l.to_string());
    t.push(fx(max_distance(g)));
    let mode = if g.chance(15) {
        g.pick(&[0, 5, -2])
    } else {
        g.int(1, 4) as i32
    };
    t.push(mode.to_string());
    // the mask
    if g.chance(5) {
        t.push("1".into());
        let mut e2 = e;
        if g.chance(6) {
            let k = g.below(3) as usize;
            e2[k] = (e2[k] + g.pick(&[-1, 1, 2])).max(1);
        }
        let mty = if g.chance(4) { MRI_FLOAT } else { MRI_UCHAR };
        t.extend(e2.map(|v| v.to_string()));
        t.push(mty.to_string());
        t.push("1".into());
        for _ in 0..3 {
            t.push(fx(1.0));
        }
        let n = (e2[0] * e2[1] * e2[2]) as usize;
        let vals: Vec<String> = (0..n)
            .map(|_| {
                if mty == MRI_FLOAT && g.chance(6) {
                    fx(g.pick(&[f32::NAN, 0.5, -0.5, 0.99, 3e9, -1.0]))
                } else {
                    token(mty, if g.chance(4) { 0.0 } else { 1.0 })
                }
            })
            .collect();
        t.extend(rle(&vals));
    } else {
        t.push("0".into());
    }
}

/// `n` case lines of `seed`: the distance maps' cases and each command of [`conv`] in turn.
pub fn generate(seed: u64, n: usize) -> Vec<String> {
    let mut g = Gen::new(seed);
    let round = conv::COMMANDS.len() + 1;
    (0..n)
        .map(|k| {
            let mut t = Vec::new();
            match k % round {
                0 => dist_case(&mut g, &mut t),
                i => {
                    let cmd = conv::COMMANDS[i - 1];
                    t.push(cmd.to_owned());
                    conv::gen_case(&mut g, &mut t, cmd);
                }
            }
            t.join(" ")
        })
        .collect()
}

#[derive(Clone, Default)]
struct Shared(Rc<RefCell<Vec<u8>>>);

impl Write for Shared {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

struct Toks<'a> {
    t: Vec<&'a str>,
    i: usize,
}

impl<'a> Toks<'a> {
    fn next(&mut self) -> &'a str {
        let s = self.t[self.i];
        self.i += 1;
        s
    }
    fn i(&mut self) -> i32 {
        self.next().parse().expect("an int")
    }
    fn f(&mut self) -> f32 {
        f32::from_bits(u32::from_str_radix(self.next(), 16).expect("hex"))
    }
}

struct Ctx {
    g: Globals,
    a: Shared,
    b: Shared,
}

/// `get_vol`: the case's volume.
fn get_vol(cx: &mut Ctx, t: &mut Toks) -> Result<Mri, Fatal> {
    let (w, h, d, ty, nf) = (t.i(), t.i(), t.i(), t.i(), t.i());
    let mut m = mri_alloc_sequence(&mut cx.g, &mut cx.a, &mut cx.b, w, h, d, ty, nf)?;
    m.vg.xsize = t.f();
    m.vg.ysize = t.f();
    m.vg.zsize = t.f();
    let n = (w * h * d * nf) as usize;
    let size = m.bytes_per_vox;
    let c = m.chunk.as_mut().expect("voxels");
    let mut i = 0;
    while i < n {
        let tok = t.next();
        let (v, k) = match tok.split_once('*') {
            Some((v, k)) => (v, k.parse::<usize>().expect("count")),
            None => (tok, 1),
        };
        let bytes: Vec<u8> = match ty {
            MRI_UCHAR => vec![v.parse::<i64>().expect("byte") as u8],
            MRI_SHORT => (v.parse::<i64>().expect("short") as i16)
                .to_le_bytes()
                .to_vec(),
            MRI_USHRT => (v.parse::<i64>().expect("ushort") as u16)
                .to_le_bytes()
                .to_vec(),
            MRI_INT => (v.parse::<i64>().expect("int") as i32)
                .to_le_bytes()
                .to_vec(),
            MRI_LONG => v.parse::<i64>().expect("long").to_le_bytes().to_vec(),
            MRI_FLOAT => u32::from_str_radix(v, 16)
                .expect("hex")
                .to_le_bytes()
                .to_vec(),
            _ => panic!("type {ty}"),
        };
        for _ in 0..k {
            if i < n {
                c[i * size..(i + 1) * size].copy_from_slice(&bytes);
                i += 1;
            }
        }
    }
    Ok(m)
}

fn put_vol(o: &mut String, m: &Mri) {
    *o += &format!(
        "{} {} {} {} {} {:016x} {:08x} ",
        m.vg.width,
        m.vg.height,
        m.vg.depth,
        m.mtype,
        m.nframes,
        m.outside_val.to_bits(),
        m.vg.xsize.to_bits()
    );
    let n = (m.vg.width * m.vg.height * m.vg.depth * m.nframes) as usize;
    let c = m.chunk.as_ref().expect("voxels");
    let vals: Vec<String> = (0..n)
        .map(|i| match m.mtype {
            MRI_UCHAR => c[i].to_string(),
            MRI_SHORT => i16::from_le_bytes([c[2 * i], c[2 * i + 1]]).to_string(),
            MRI_USHRT => u16::from_le_bytes([c[2 * i], c[2 * i + 1]]).to_string(),
            MRI_INT => i32::from_le_bytes(c[4 * i..4 * i + 4].try_into().unwrap()).to_string(),
            MRI_LONG => i64::from_le_bytes(c[8 * i..8 * i + 8].try_into().unwrap()).to_string(),
            MRI_FLOAT => format!(
                "{:08x}",
                u32::from_le_bytes(c[4 * i..4 * i + 4].try_into().unwrap())
            ),
            t => panic!("type {t}"),
        })
        .collect();
    for v in rle(&vals) {
        *o += &v;
        *o += " ";
    }
}

fn case(cx: &mut Ctx, t: &mut Toks, o: &mut String) -> Result<(), Fatal> {
    let cmd = t.next();
    if conv::run(cx, cmd, t, o)? {
        return Ok(());
    }
    match cmd {
        "edm" | "dtr" => {
            let src = get_vol(cx, t)?;
            let mut dst = if t.i() != 0 {
                Some(get_vol(cx, t)?)
            } else {
                None
            };
            let label = t.i();
            let maxd = t.f();
            let mode = t.i();
            let mask = if t.i() != 0 {
                Some(get_vol(cx, t)?)
            } else {
                None
            };
            let given = dst.is_some();
            let r = if cmd == "edm" {
                fmm::mri_extract_distance_map(
                    &mut cx.g,
                    &mut cx.a,
                    &mut cx.b,
                    &src,
                    dst.as_mut(),
                    label,
                    maxd,
                    mode,
                    mask.as_ref(),
                )?
            } else {
                fmm::mri_distance_transform(
                    &mut cx.g,
                    &mut cx.a,
                    &mut cx.b,
                    &Environment::default(),
                    &src,
                    dst.as_mut(),
                    label,
                    maxd,
                    mode,
                    mask.as_ref(),
                )?
            };
            match r {
                Ret::New(m) => {
                    *o += "0 ";
                    put_vol(o, &m);
                }
                Ret::Given => {
                    *o += "1 ";
                    put_vol(o, dst.as_ref().expect("given"));
                }
                Ret::Null => *o += if given { "0 NULL " } else { "1 NULL " },
            }
            Ok(())
        }
        "edmf" | "dtrf" => {
            let path = t.next();
            let src = mri_read(
                &mut cx.g,
                &mut cx.a,
                &mut cx.b,
                &Environment::default(),
                None,
                path,
                MRI_VOLUME_TYPE_UNKNOWN,
                true,
                -1,
                -1,
            )?
            .expect("the volume");
            let label = t.i();
            let maxd = t.f();
            let mode = t.i();
            let r = if cmd == "edmf" {
                fmm::mri_extract_distance_map(
                    &mut cx.g, &mut cx.a, &mut cx.b, &src, None, label, maxd, mode, None,
                )?
            } else {
                fmm::mri_distance_transform(
                    &mut cx.g,
                    &mut cx.a,
                    &mut cx.b,
                    &Environment::default(),
                    &src,
                    None,
                    label,
                    maxd,
                    mode,
                    None,
                )?
            };
            let Ret::New(m) = r else {
                panic!("a new volume")
            };
            *o += &format!(
                "{} {} {} {} {} {:016x} {:08x} ",
                m.vg.width,
                m.vg.height,
                m.vg.depth,
                m.mtype,
                m.nframes,
                m.outside_val.to_bits(),
                m.vg.xsize.to_bits()
            );
            let mut h: u64 = 0xcbf2_9ce4_8422_2325;
            for &b in m.chunk.as_ref().expect("voxels") {
                h = (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3);
            }
            *o += &format!("{h:016x} ");
            Ok(())
        }
        c => panic!("unknown command {c}"),
    }
}

fn escape(b: &[u8]) -> String {
    let mut s = String::new();
    for &c in b {
        if !(0x20..=0x7e).contains(&c) || c == b'\\' {
            s += &format!("\\x{c:02x}");
        } else {
            s.push(c as char);
        }
    }
    s
}

/// The port's line for a case line, and the defects it noted.
pub fn run_line(line: &str, mode: UpstreamDefects) -> (String, Vec<&'static str>) {
    let shared = Shared::default();
    let mut cx = Ctx {
        g: Globals::new(mode),
        a: shared.clone(),
        b: shared.clone(),
    };
    cx.g.progname = "fsfilter".to_owned();
    let mut t = Toks {
        t: line.split_whitespace().collect(),
        i: 0,
    };
    let mut o = String::new();
    let res = case(&mut cx, &mut t, &mut o);
    let text = escape(&shared.0.borrow());
    let result = match res {
        Ok(()) => o.trim_end().to_owned(),
        Err(f) => format!("EXIT {}", f.status()),
    };
    let mut ids: Vec<&'static str> =
        cx.g.notices
            .as_slice()
            .iter()
            .map(|n| n.defect.id)
            .collect();
    ids.sort_unstable();
    ids.dedup();
    (format!("{result} | {text}"), ids)
}
