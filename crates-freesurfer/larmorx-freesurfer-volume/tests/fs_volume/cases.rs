// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The fs.volume probe's cases (`validation/probes/fs-volume/`): a seeded generator of case lines,
//! and the port's answer to each, printed as `fsvolume` prints FreeSurfer's (`<result> | <text>`,
//! the text FreeSurfer's code wrote C-escaped; `EXIT <status> | <text>` when it ended the program).
#![allow(dead_code)]

use std::cell::RefCell;
use std::io::Write;
use std::rc::Rc;

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::cname::CName;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::matrix::Matrix;
use larmorx_freesurfer_io::cfile::CFile;
use larmorx_freesurfer_mri::vol_geom::VolGeom;
use larmorx_freesurfer_volume::vgeom;

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

const SPECIAL: [f32; 6] = [
    f32::NAN,
    f32::INFINITY,
    f32::NEG_INFINITY,
    1e30,
    -0.0,
    1e-40,
];

fn fx(v: f32) -> String {
    format!("{:08x}", v.to_bits())
}
fn dx(v: f64) -> String {
    format!("{:016x}", v.to_bits())
}
fn hex(b: &[u8]) -> String {
    if b.is_empty() {
        return "-".into();
    }
    b.iter().map(|c| format!("{c:02x}")).collect()
}

fn num(g: &mut Gen, lo: i64, hi: i64, d: i64) -> f32 {
    if g.chance(40) {
        return g.pick(&SPECIAL);
    }
    g.int(lo * d, hi * d) as f32 / d as f32
}

fn cosine(g: &mut Gen) -> f32 {
    if g.chance(60) {
        return g.pick(&SPECIAL);
    }
    g.pick(&[-1.0, 0.0, 1.0, 0.6, 0.8, -0.6, -0.8, 0.70710677, 0.1, 0.995])
}

fn size(g: &mut Gen) -> f32 {
    if g.chance(40) {
        return g.pick(&SPECIAL);
    }
    g.pick(&[1.0, 0.5, 2.0, 1.25, 0.9375, 3.0, 0.0, -1.0, 1.1, 0.8])
}

const NAMES: [&str; 6] = [
    "-",
    "vol.mgz",
    "orig/001.mgz",
    "/data/sub-01/mri/orig.mgz",
    "unknown",
    "a",
];

/// `set_geom`'s tokens.
fn geom(g: &mut Gen, t: &mut Vec<String>) {
    t.push(g.pick(&[0, 1, 1, 1, 2, -1]).to_string());
    for _ in 0..3 {
        t.push(g.pick(&[1, 2, 3, 4, 6, 256, 0, -2]).to_string());
    }
    for _ in 0..3 {
        t.push(fx(size(g)));
    }
    t.push(g.int(0, 1).to_string());
    for _ in 0..9 {
        t.push(fx(cosine(g)));
    }
    for _ in 0..3 {
        t.push(fx(num(g, -120, 120, 8)));
    }
    for _ in 0..3 {
        let s = if g.chance(3) {
            g.pick(&[0.1, 1e-6, 2e-5, -0.3, 0.5])
        } else {
            0.0
        };
        t.push(fx(s));
    }
    t.push(g.pick(&NAMES).to_owned());
}

/// A number's text in a volume-geometry line.
fn numtext(g: &mut Gen, int: bool) -> String {
    if g.chance(6) {
        return g
            .pick(&[
                "nan",
                "-nan",
                "inf",
                "-inf",
                "infinity",
                "INF",
                "1e40",
                "-1e-50",
                "0x1.8p3",
                "1e",
                "abc",
                "+5",
                "1.5e+00x",
                "2147483648",
                "99999999999",
                "-0",
                ".5",
                "5.",
                "1,5",
                "nan(1)",
                "0x",
                "--1",
                "3e-39",
                "1e-46",
            ])
            .to_owned();
    }
    if int {
        return g.pick(&[1, 2, 64, 128, 256, 0, -1, 311]).to_string();
    }
    let v = num(g, -200, 200, 64);
    match g.below(3) {
        0 => larmorx_core::cfmt::e(f64::from(v), 15),
        1 => format!("{v}"),
        _ => larmorx_core::cfmt::f(f64::from(v), 6),
    }
}

fn long_word(g: &mut Gen, n: usize) -> String {
    let c = g.pick(&["a", "x", "valid", "Z9"]);
    let mut s = String::new();
    while s.len() < n {
        s += c;
    }
    s.truncate(n);
    s
}

/// The line of key `k` (0 valid .. 7 cras), as writeVolGeom writes it or perturbed.
fn key_line(g: &mut Gen, k: usize) -> String {
    const KEYS: [&str; 8] = [
        "valid",
        "filename",
        "volume",
        "voxelsize",
        "xras  ",
        "yras  ",
        "zras  ",
        "cras  ",
    ];
    let mut key = KEYS[k].to_owned();
    let mut eq = "=".to_owned();
    if g.chance(12) {
        eq = g
            .pick(&["==", "=1", ":", "is", "", "=valid", "="])
            .to_owned();
    }
    if g.chance(15) {
        key = g
            .pick(&["Valid", "valid=", "xras=", "VOLUME", "cras:", "filenames"])
            .to_owned();
    }
    let body = match k {
        0 => {
            let v = numtext(g, true);
            let tail = g.pick(&[
                "  # volume info valid",
                "  # volume info invalid",
                "",
                " extra",
            ]);
            format!("{v}{tail}")
        }
        1 => {
            if g.chance(10) {
                String::new()
            } else if g.chance(12) {
                let n = g.pick(&[1022, 1023, 1024, 1030]);
                long_word(g, n)
            } else {
                let n = g.pick(&NAMES[1..]);
                let extra = if g.chance(6) { " second word" } else { "" };
                format!("{n}{extra}")
            }
        }
        2 => {
            let n = if g.chance(8) { g.int(0, 4) } else { 3 };
            (0..n)
                .map(|_| numtext(g, true))
                .collect::<Vec<_>>()
                .join(" ")
        }
        _ => {
            let n = if g.chance(8) { g.int(0, 4) } else { 3 };
            let sep = if g.chance(10) { "\t" } else { " " };
            (0..n)
                .map(|_| numtext(g, false))
                .collect::<Vec<_>>()
                .join(sep)
        }
    };
    let lead = if g.chance(15) { "  " } else { "" };
    let sp = if g.chance(15) { "\t" } else { " " };
    let end = if g.chance(12) { "\r\n" } else { "\n" };
    if eq.is_empty() {
        format!("{lead}{key}{sp}{body}{end}")
    } else {
        format!("{lead}{key}{sp}{eq} {body}{end}")
    }
}

/// An odd line between the block's lines.
fn odd_line(g: &mut Gen) -> Vec<u8> {
    match g.below(11) {
        0 => b"\n".to_vec(),
        1 => b"   \t\n".to_vec(),
        2 => b"dst volume info\n".to_vec(),
        3 => b"type      = 1 # LINEAR_RAS_TO_RAS\n".to_vec(),
        4 => {
            let n = g.pick(&[63, 64, 65, 100, 1030]);
            format!("{} = 1\n", long_word(g, n)).into_bytes()
        }
        5 => b"valid\n".to_vec(),
        6 => b"\0valid = 1\n".to_vec(),
        7 => b"filename =\n".to_vec(),
        8 => format!("cras = 1 2 {}\n", long_word(g, 1100)).into_bytes(),
        9 => b"# comment\n".to_vec(),
        _ => b"xras = 1 \0 2 3\n".to_vec(),
    }
}

/// The bytes of a volume-geometry block, well formed or perturbed, with what follows it.
fn vg_file(g: &mut Gen) -> Vec<u8> {
    let mut f = Vec::new();
    if g.chance(12) {
        f.extend_from_slice(g.pick(&[&b"\n"[..], b"  \n", b"\0\n"]));
    }
    if g.chance(30) {
        return f;
    }
    let mut order: Vec<usize> = (0..8).collect();
    if g.chance(6) {
        for i in (1..order.len()).rev() {
            let j = g.below(i as u64 + 1) as usize;
            order.swap(i, j);
        }
    }
    let perturb = g.chance(2);
    for &k in &order {
        if perturb && g.chance(14) {
            continue;
        }
        if perturb && g.chance(10) {
            f.extend(odd_line(g));
        }
        f.extend(key_line(g, k).into_bytes());
        if perturb && g.chance(20) {
            f.extend(key_line(g, k).into_bytes());
        }
    }
    match g.below(6) {
        0 => {}
        1 => f.extend_from_slice(b"dst volume info\nvalid = 1  # volume info valid\n"),
        2 => f.extend_from_slice(b"\x00\x00\x00\x14\x00\x00\x00\x01"),
        3 => f.extend_from_slice(b"subject bert\nfscale 0.1\n"),
        4 => {
            // no final newline
            f.pop();
        }
        _ => f.extend(odd_line(g)),
    }
    f
}

pub fn generate(seed: u64, n: usize) -> Vec<String> {
    let mut g = Gen::new(seed);
    let mut lines = Vec::with_capacity(n);
    let cmds = COMMANDS;
    for i in 0..n {
        let cmd = cmds[i % cmds.len()];
        let mut t: Vec<String> = vec![cmd.to_owned()];
        match cmd {
            "vgwrite" | "vgprint0" | "vgi2r" | "vgr2i" | "tkrv2r" => geom(&mut g, &mut t),
            "vgread" => {
                geom(&mut g, &mut t);
                t.push(hex(&vg_file(&mut g)));
            }
            "getvg" | "copyvg" => {
                geom(&mut g, &mut t);
                geom(&mut g, &mut t);
            }
            "vgeq0" => {
                geom(&mut g, &mut t);
                let a = t[1..].to_vec();
                if g.chance(2) {
                    // the second geometry: the first, perturbed
                    let mut b = a.clone();
                    for _ in 0..g.int(0, 3) {
                        let k = g.below(b.len() as u64 - 1) as usize;
                        if (4..=6).contains(&k) || (8..=22).contains(&k) {
                            let v = f32::from_bits(u32::from_str_radix(&b[k], 16).unwrap());
                            let d = g.pick(&[1e-8f32, 1e-6, 1e-3, 0.5]);
                            b[k] = fx(v + d);
                        } else if k < 4 {
                            b[k] = g.int(0, 4).to_string();
                        }
                    }
                    t.extend(b);
                } else {
                    geom(&mut g, &mut t);
                }
                t.push(dx(g.pick(&[f64::from(f32::EPSILON), 1e-6, 1e-3, 0.0, 1.0])));
                t.push(dx(g.pick(&[f64::from(f32::EPSILON), 1e-4, 0.0])));
            }
            c => panic!("no generator for {c}"),
        }
        lines.push(t.join(" "));
    }
    lines
}

/// The probe's commands, in the order the generator cycles through them.
pub const COMMANDS: [&str; 10] = [
    "vgwrite", "vgread", "vgread", "vgprint0", "getvg", "copyvg", "vgi2r", "vgr2i", "tkrv2r",
    "vgeq0",
];

/// One buffer both streams write into, in order (FreeSurfer's standard output and error go to
/// one file in `fsvolume`).
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
        f32::from_bits(u32::from_str_radix(self.next(), 16).expect("a float"))
    }
    fn d(&mut self) -> f64 {
        f64::from_bits(u64::from_str_radix(self.next(), 16).expect("a double"))
    }
    fn s(&mut self) -> String {
        let s = self.next();
        if s == "-" {
            String::new()
        } else {
            s.to_owned()
        }
    }
    fn hex(&mut self) -> Vec<u8> {
        let s = self.next();
        if s == "-" {
            return Vec::new();
        }
        (0..s.len() / 2)
            .map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).expect("hex"))
            .collect()
    }
}

#[derive(Default)]
struct Out(String);

impl Out {
    fn i(&mut self, v: i64) {
        self.0 += &format!("{v} ");
    }
    fn f(&mut self, v: f32) {
        self.0 += &format!("{:08x} ", v.to_bits());
    }
    fn d(&mut self, v: f64) {
        self.0 += &format!("{:016x} ", v.to_bits());
    }
    fn hex(&mut self, b: &[u8]) {
        self.0 += &hex(b);
        self.0 += " ";
    }
    fn m(&mut self, m: Option<&Matrix>) {
        let Some(m) = m else {
            self.0 += "NULL ";
            return;
        };
        self.i(i64::from(m.rows));
        self.i(i64::from(m.cols));
        for r in 1..=m.rows {
            for c in 1..=m.cols {
                self.f(m.relt(r, c));
            }
        }
    }
    fn geom(&mut self, v: &VolGeom) {
        self.i(i64::from(v.valid));
        for x in [v.width, v.height, v.depth] {
            self.i(i64::from(x));
        }
        for x in [v.xsize, v.ysize, v.zsize] {
            self.f(x);
        }
        self.i(i64::from(v.ras_good_flag));
        for x in [
            v.x_r, v.x_a, v.x_s, v.y_r, v.y_a, v.y_s, v.z_r, v.z_a, v.z_s, v.c_r, v.c_a, v.c_s,
            v.s_r, v.s_a, v.s_s,
        ] {
            self.f(x);
        }
        self.hex(v.fname.c_bytes());
    }
}

struct Ctx {
    g: Globals,
    a: Shared,
    b: Shared,
}

fn set_geom(t: &mut Toks, v: &mut VolGeom) {
    v.valid = t.i();
    v.width = t.i();
    v.height = t.i();
    v.depth = t.i();
    v.xsize = t.f();
    v.ysize = t.f();
    v.zsize = t.f();
    v.ras_good_flag = t.i();
    v.x_r = t.f();
    v.x_a = t.f();
    v.x_s = t.f();
    v.y_r = t.f();
    v.y_a = t.f();
    v.y_s = t.f();
    v.z_r = t.f();
    v.z_a = t.f();
    v.z_s = t.f();
    v.c_r = t.f();
    v.c_a = t.f();
    v.c_s = t.f();
    v.s_r = t.f();
    v.s_a = t.f();
    v.s_s = t.f();
    v.fname = CName::new(t.s().as_bytes());
}

fn geom_of(t: &mut Toks) -> VolGeom {
    let mut v = VolGeom::default();
    set_geom(t, &mut v);
    v
}

fn case(cx: &mut Ctx, t: &mut Toks, o: &mut Out) -> Result<(), Fatal> {
    let cmd = t.next();
    let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
    match cmd {
        "vgwrite" => {
            let vg = geom_of(t);
            let mut fp = CFile::memory_writer();
            vgeom::write_vol_geom(&mut fp, &vg);
            o.hex(&fp.into_bytes());
        }
        "vgread" => {
            let mut vg = geom_of(t);
            let bytes = t.hex();
            let mut fp = CFile::from_bytes(bytes);
            g.errno = 0;
            vgeom::read_vol_geom(g, b, &mut fp, &mut vg);
            let e = g.errno;
            o.geom(&vg);
            o.i(fp.tell());
            o.i(i64::from(fp.feof()));
            o.i(i64::from(fp.ferror()));
            o.i(i64::from(e));
            let mut rest = Vec::new();
            while rest.len() < 64 {
                let c = fp.getc();
                if c < 0 {
                    break;
                }
                rest.push(c as u8);
            }
            o.hex(&rest);
        }
        "vgprint0" => {
            let vg = geom_of(t);
            vgeom::vg_print(b, &vg);
            o.i(0);
        }
        "getvg" | "copyvg" => {
            let src = geom_of(t);
            let mut dst = geom_of(t);
            if cmd == "getvg" {
                vgeom::get_vol_geom(&src, &mut dst);
            } else {
                vgeom::copy_vol_geom(&src, &mut dst);
            }
            o.geom(&dst);
        }
        "vgi2r" => {
            let vg = geom_of(t);
            let m = vgeom::vg_i_to_r(g, a, b, &vg)?;
            o.m(Some(&m));
        }
        "vgr2i" => {
            let vg = geom_of(t);
            let m = vgeom::vg_r_to_i(g, a, b, &vg)?;
            o.m(m.as_ref());
        }
        "tkrv2r" => {
            let vg = geom_of(t);
            let m = vgeom::tkr_vox2ras_from_vol_geom(g, a, b, &vg)?;
            o.m(Some(&m));
        }
        "vgeq0" => {
            let va = geom_of(t);
            let vb = geom_of(t);
            let thresh = t.d();
            g.vg_is_equal_threshold = t.d();
            o.i(i64::from(vgeom::vg_is_not_equal_thresh(&va, &vb, thresh)));
            o.i(i64::from(vgeom::vg_is_equal(g, &va, &vb)));
        }
        c => panic!("unknown command {c}"),
    }
    Ok(())
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
    let mut t = Toks {
        t: line.split_whitespace().collect(),
        i: 0,
    };
    let mut o = Out::default();
    let res = case(&mut cx, &mut t, &mut o);
    let text = escape(&shared.0.borrow());
    let result = match res {
        Ok(()) => o.0.trim_end().to_owned(),
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
