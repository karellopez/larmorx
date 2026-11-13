// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The fs.io probe's cases (`validation/probes/fs-io/`): a seeded generator of case lines, and the
//! port's answer to each, printed as `fsio` prints FreeSurfer's (`<result> | <text>`, the text
//! FreeSurfer's code wrote C-escaped; `EXIT <status> | <text>` when it ended the program).
//!
//! A `?` in the port's result stands for one hexadecimal digit of a byte FreeSurfer reads from
//! memory contents (FS-49, FS-167): it matches any digit. The pseudo-IDs `SKIP` (FreeSurfer's line
//! is not comparable: it crashes, loops for ever, or the port refuses what it does) and `VARY`
//! (FreeSurfer's result depends on its memory: compared, counted apart) follow the notices.
#![allow(dead_code)]

use std::cell::RefCell;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::fio::Tsv;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_io::bfileio;
use larmorx_freesurfer_io::cfile::CFile;
use larmorx_freesurfer_io::fio;
use larmorx_freesurfer_io::mriio_old;
use larmorx_io::znz::ZnzFile;

#[path = "piece10.rs"]
mod piece10;
#[path = "piece11.rs"]
mod piece11;
#[path = "piece2.rs"]
mod piece2;
#[path = "piece3.rs"]
mod piece3;
#[path = "piece4.rs"]
mod piece4;
#[path = "piece5.rs"]
mod piece5;
#[path = "piece6.rs"]
mod piece6;
#[path = "piece7.rs"]
mod piece7;
#[path = "piece8.rs"]
mod piece8;
#[path = "piece9.rs"]
mod piece9;

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

fn hex(b: &[u8]) -> String {
    if b.is_empty() {
        return "-".into();
    }
    b.iter().map(|c| format!("{c:02x}")).collect()
}

fn unhex(s: &str) -> Vec<u8> {
    if s == "-" {
        return Vec::new();
    }
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap())
        .collect()
}

/// `N` or `x<hex>`.
fn enc(s: Option<&[u8]>) -> String {
    match s {
        None => "N".into(),
        Some([]) => "x".into(),
        Some(b) => format!("x{}", hex(b)),
    }
}

fn dec(t: &str) -> Option<Vec<u8>> {
    if t.starts_with('N') {
        None
    } else if t.len() == 1 {
        Some(Vec::new())
    } else {
        Some(unhex(&t[1..]))
    }
}

fn bytes(g: &mut Gen, max: i64) -> Vec<u8> {
    let n = g.int(0, max) as usize;
    (0..n)
        .map(|_| {
            if g.chance(4) {
                g.pick(&[0u8, 0xff, 0x7f, 0x80, 0x3f, 0x7f, 0xc0])
            } else {
                g.below(256) as u8
            }
        })
        .collect()
}

fn path_name(g: &mut Gen) -> Vec<u8> {
    let n = g.int(0, 7);
    let mut s = Vec::new();
    for _ in 0..n {
        let c = g.pick(&[
            b"a".as_slice(),
            b"b",
            b"/",
            b"/",
            b".",
            b"x",
            b"\xc3\xa9",
            b"mgz",
        ]);
        s.extend_from_slice(c);
    }
    s
}

const FIXTURE_NAMES: [&str; 16] = [
    "f",
    "d",
    "d/",
    "f/",
    "d/i",
    "nowhere",
    "d/nowhere",
    ".",
    "./d",
    "",
    "nr",
    "ro",
    "ln",
    "dln",
    "bad",
    "ro/x",
];

/// The fixture names that behave alike on every platform (no permissions, no links).
const PORTABLE_NAMES: usize = 10;

fn tsv_number(g: &mut Gen) -> String {
    match g.below(12) {
        0 => g.int(-1000, 1000).to_string(),
        1 => format!("{}.{}", g.int(-99, 99), g.int(0, 999)),
        2 => format!("{}e{}", g.int(1, 9), g.int(-320, 320)),
        3 => g
            .pick(&[
                "inf", "-inf", "nan", "-nan", "NaN", "Infinity", "-0", "+.5", "5.",
            ])
            .into(),
        4 => g
            .pick(&["0x1p3", "0x1.8", "0x.p1", "1e", "1e+", "100ergs"])
            .into(),
        5 => g
            .pick(&["1e400", "1e-400", "4.9e-324", "2.2250738585072009e-308"])
            .into(),
        6 if g.chance(6) => g
            .pick(&["x", "-x", "-.", "abc", "-.-.5", "+-1", "in", "infin", "nax"])
            .into(),
        _ => format!("{}", g.int(-50_000, 50_000) as f64 / 64.0),
    }
}

fn tsv_content(g: &mut Gen) -> String {
    match g.below(20) {
        0 => return "N".into(),
        1 => return "-".into(),
        _ => {}
    }
    let mut t: Vec<u8> = Vec::new();
    if g.chance(12) {
        t.push(b'\n');
    }
    if g.chance(10) {
        t.push(b'\t');
    }
    let ncols = g.int(1, 4);
    for c in 0..ncols {
        let name = g.pick(&["a", "bb", "x y", "AIF", "Time", "c\r", ""]);
        t.extend_from_slice(name.as_bytes());
        if c + 1 < ncols {
            t.push(b'\t');
            if g.chance(8) {
                t.push(b'\t');
            }
        }
    }
    if g.chance(40) {
        t.extend(std::iter::repeat_n(b'h', 5100));
    }
    match g.below(6) {
        0 => {}
        1 => t.extend_from_slice(b"\r\n"),
        2 => t.extend_from_slice(b"\t\n"),
        _ => t.push(b'\n'),
    }
    let rows = g.int(0, 4);
    for r in 0..rows {
        let n = if g.chance(8) {
            g.int(0, ncols + 1)
        } else {
            ncols
        };
        for c in 0..n {
            t.extend_from_slice(tsv_number(g).as_bytes());
            if c + 1 < n {
                t.push(g.pick(b"\t\t \n"));
            }
        }
        if r + 1 < rows || g.chance(2) {
            t.extend_from_slice(g.pick(&[b"\n".as_slice(), b"\r\n", b" \n", b"\n\n"]));
        }
    }
    hex(&t)
}

const FORMATS: [&str; 20] = [
    "%g", "%f", "%.3e", "%10.4f", "%+g", "%-8.2f|", "%#g", "%G", "x", "%lf", "%5.0E", "%%f%g",
    "% .2f", "%012.3e", "%.0f", "%d", "%f%f", "%s", "%Lf", "%a",
];

/// The indices of seed 1's 4,800 portable cases that take more than 0.25 s of CPU each in a debug
/// build (`tests/fs_io_heavy.rs`; re-measured whenever the generators change: 2026-11-13, track A
/// relay 19, with piece 10's sixteen commands: .mat files whose corrupted headers make the readers
/// allocate and hash millions of numbers, 28.9 of the 29.8 s). The recorded test (`fs_io_official.rs`) leaves them to
/// the probe's runs (`check.py --portable` without `--light`, every seed's full runs), which keep
/// them.
pub const HEAVY_SEED1: [usize; 3] = [2248, 3687, 4361];

/// The recorded test's cases: seed 1's 4,800 portable cases without [`HEAVY_SEED1`].
#[allow(dead_code)]
pub fn recorded_cases() -> Vec<String> {
    generate(1, 4800, true)
        .into_iter()
        .enumerate()
        .filter(|(i, _)| !HEAVY_SEED1.contains(i))
        .map(|(_, l)| l)
        .collect()
}

/// `n` case lines of seed `seed`; `portable`: only the cases whose results are the same on every
/// platform (no permissions, links, overlong names or huge allocations).
pub fn generate(seed: u64, n: usize, portable: bool) -> Vec<String> {
    let mut g = Gen::new(seed);
    let mut lines = Vec::with_capacity(n);
    let mut kinds: Vec<&str> = if portable {
        vec![
            "rd", "zr", "wr", "zw", "dirname", "basename", "ext", "tsv", "swap", "calloc",
        ]
    } else {
        vec![
            "rd", "zr", "wr", "zw", "dirname", "basename", "ext", "fsx", "tsv", "swap", "calloc",
        ]
    };
    kinds.extend(piece2::KINDS);
    kinds.extend(piece3::KINDS);
    kinds.extend(piece4::KINDS);
    kinds.extend(piece5::KINDS);
    kinds.extend(piece6::KINDS);
    kinds.extend(piece7::KINDS);
    kinds.extend(piece8::KINDS);
    kinds.extend(piece9::KINDS);
    kinds.extend(piece10::KINDS);
    kinds.extend(piece11::KINDS);
    for i in 0..n {
        let kind = kinds[i % kinds.len()];
        if piece2::KINDS.contains(&kind) {
            lines.push(piece2::generate(&mut g, kind).join(" "));
            continue;
        }
        if piece3::KINDS.contains(&kind) {
            lines.push(piece3::generate(&mut g, kind, portable).join(" "));
            continue;
        }
        if piece4::KINDS.contains(&kind) {
            lines.push(piece4::generate(&mut g, kind, portable).join(" "));
            continue;
        }
        if piece5::KINDS.contains(&kind) {
            lines.push(piece5::generate(&mut g, kind, portable).join(" "));
            continue;
        }
        if piece6::KINDS.contains(&kind) {
            lines.push(piece6::generate(&mut g, kind, portable).join(" "));
            continue;
        }
        if piece7::KINDS.contains(&kind) {
            lines.push(piece7::generate(&mut g, kind, portable).join(" "));
            continue;
        }
        if piece8::KINDS.contains(&kind) {
            lines.push(piece8::generate(&mut g, kind, portable).join(" "));
            continue;
        }
        if piece9::KINDS.contains(&kind) {
            lines.push(piece9::generate(&mut g, kind, portable).join(" "));
            continue;
        }
        if piece10::KINDS.contains(&kind) {
            lines.push(piece10::generate(&mut g, kind, portable).join(" "));
            continue;
        }
        if piece11::KINDS.contains(&kind) {
            lines.push(piece11::generate(&mut g, kind, portable).join(" "));
            continue;
        }
        let mut t: Vec<String> = vec![kind.into()];
        match kind {
            "rd" => {
                let f = g.pick(&[
                    "r1", "r2", "r3", "rd", "ri", "rl", "rix", "rf", "r4", "gf", "rs", "rfx",
                    "rdx", "rsx",
                ]);
                let size = match f {
                    "r1" => 1,
                    "r2" | "rs" | "rsx" => 2,
                    "r3" => 3,
                    "rd" | "rl" | "rdx" => 8,
                    _ => 4,
                };
                let count = g.int(1, 4);
                t.push(f.into());
                t.push(g.pick(&["-1", "0"]).into());
                t.push(count.to_string());
                t.push(format!("{:08x}", g.next() as u32));
                t.push(hex(&bytes(&mut g, size * count + 2)));
            }
            "zr" => {
                let f = g.pick(&[
                    "z1", "zs", "zi", "zl", "zf", "zd", "zix", "zfx", "z2", "z3", "z4", "zdx",
                    "zsx",
                ]);
                let size = match f {
                    "z1" => 1,
                    "zs" | "z2" | "zsx" => 2,
                    "z3" => 3,
                    "zl" | "zd" | "zdx" => 8,
                    _ => 4,
                };
                let count = g.int(1, 4);
                t.push(f.into());
                t.push(g.pick(&["-1", "0"]).into());
                t.push(g.int(0, 2).to_string());
                t.push(count.to_string());
                t.push(format!("{:08x}", g.next() as u32));
                t.push(hex(&bytes(&mut g, size * count + 2)));
            }
            "wr" => {
                let f = g.pick(&["w2", "w3", "wi", "wl", "wf", "w1", "w4", "wsh", "pf", "wd"]);
                t.push(f.into());
                let v = match f {
                    "wf" | "pf" => format!("{:08x}", g.next() as u32),
                    "wd" => format!("{:016x}", g.next()),
                    "wl" => {
                        if g.chance(2) {
                            (g.next() as i64).to_string()
                        } else {
                            g.pick(&[0i64, -1, i64::MIN, i64::MAX, 1 << 40]).to_string()
                        }
                    }
                    _ => {
                        if g.chance(2) {
                            (g.next() as i32).to_string()
                        } else {
                            g.pick(&[
                                0i64,
                                -1,
                                1,
                                32767,
                                32768,
                                -32767,
                                -32768,
                                -32769,
                                40000,
                                1 << 23,
                                1 << 24,
                                -(1 << 24),
                                i64::from(i32::MIN),
                                i64::from(i32::MAX),
                            ])
                            .to_string()
                        }
                    }
                };
                t.push(v);
            }
            "zw" => {
                let f = g.pick(&[
                    "zws", "zwus", "zwi", "zwl", "zwf", "zwd", "zw1", "zw2", "zw3", "zw4",
                ]);
                t.push(f.into());
                t.push(g.int(0, 1).to_string());
                let v = match f {
                    "zwf" => format!("{:08x}", g.next() as u32),
                    "zwd" => format!("{:016x}", g.next()),
                    "zwl" => (g.next() as i64).to_string(),
                    _ if g.chance(3) => (g.next() as i32).to_string(),
                    _ => g
                        .pick(&[
                            0i64,
                            -1,
                            1,
                            255,
                            256,
                            32767,
                            32768,
                            -32767,
                            -32768,
                            65535,
                            65536,
                            1 << 24,
                            0x1234_5678,
                            i64::from(i32::MIN),
                        ])
                        .to_string(),
                };
                t.push(v);
            }
            "dirname" | "ext" => {
                let s = if g.chance(15) {
                    None
                } else {
                    Some(path_name(&mut g))
                };
                t.push(enc(s.as_deref()));
            }
            "basename" => {
                let s = if g.chance(15) {
                    None
                } else {
                    Some(path_name(&mut g))
                };
                t.push(enc(s.as_deref()));
                let e: Option<Vec<u8>> = match g.below(6) {
                    0 => None,
                    1 => Some(b".mgz".to_vec()),
                    2 => Some(Vec::new()),
                    3 => Some(b"b".to_vec()),
                    4 => Some(b"/".to_vec()),
                    _ => Some(path_name(&mut g)),
                };
                t.push(enc(e.as_deref()));
            }
            "fsx" => {
                let f = g.pick(&["exists", "isdir", "dirw0", "dirw1"]);
                t.push(f.into());
                let name: Vec<u8> = if g.chance(10) {
                    let k = g.pick(&[1960usize, 1975, 1980, 1981, 1982, 1983, 2000, 4100]);
                    let mut v = b"d/".to_vec();
                    v.extend(std::iter::repeat_n(b'a', k));
                    v
                } else {
                    g.pick(&FIXTURE_NAMES).as_bytes().to_vec()
                };
                t.push(enc(Some(&name)));
            }
            "tsv" => {
                t.push(g.int(0, 1).to_string());
                let fmt = if g.chance(2) {
                    None
                } else if g.chance(30) {
                    let mut f = "%g".to_string();
                    f.push_str(&"y".repeat(g.pick(&[995usize, 996, 997, 1000])));
                    Some(f)
                } else {
                    Some(g.pick(&FORMATS).to_string())
                };
                t.push(enc(fmt.as_deref().map(str::as_bytes)));
                t.push(tsv_content(&mut g));
            }
            "swap" => {
                let f = g.pick(&["s2", "s4", "s8"]);
                let buflen = g.int(0, 20);
                let count = match g.below(6) {
                    0 => -1,
                    1 => 0,
                    2 => buflen,
                    3 => (buflen - 1).max(0),
                    4 => buflen + 1,
                    _ => g.int(0, 24),
                };
                t.push(f.into());
                t.push(count.to_string());
                t.push(buflen.to_string());
                let b: Vec<u8> = (0..buflen).map(|_| g.below(256) as u8).collect();
                t.push(hex(&b));
            }
            _ => {
                let (a, b) = if portable {
                    g.pick(&[
                        (0u64, 1u64),
                        (1, 0),
                        (3, 1),
                        (100, 4),
                        (4096, 1),
                        (u64::MAX, 2),
                        (1 << 62, 4),
                        (1 << 63, 1),
                    ])
                } else {
                    g.pick(&[
                        (0u64, 1u64),
                        (3, 1),
                        (100, 4),
                        (u64::MAX, 2),
                        (1 << 63, 1),
                        (1 << 42, 1),
                        (1 << 40, 64),
                    ])
                };
                t.push(a.to_string());
                t.push(b.to_string());
            }
        }
        lines.push(t.join(" "));
    }
    lines
}

/// `b` with every `from` replaced by `to`.
fn replace_all(b: &[u8], from: &[u8], to: &[u8]) -> Vec<u8> {
    let mut r = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if !from.is_empty() && b[i..].starts_with(from) {
            r.extend_from_slice(to);
            i += from.len();
        } else {
            r.push(b[i]);
            i += 1;
        }
    }
    r
}

/// The C escaping of `fsio`'s text.
fn escape(b: &[u8]) -> String {
    let mut r = String::new();
    for &c in b {
        if !(0x20..=0x7e).contains(&c) || c == b'\\' {
            r.push_str(&format!("\\x{c:02x}"));
        } else {
            r.push(c as char);
        }
    }
    r
}

/// The program's standard output and error, in the order written.
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

#[derive(Default)]
struct Out(String);

impl Out {
    fn i(&mut self, v: i64) {
        self.0.push_str(&format!("{v} "));
    }
    /// A value's bytes, most significant first, with `?` for the `unknown` last bytes of its
    /// memory (`n` bytes in all; memory order is the reverse of printing order).
    fn x(&mut self, be: &[u8], unknown_from: usize) {
        for (k, b) in be.iter().enumerate() {
            if k >= unknown_from {
                self.0.push_str("??");
            } else {
                self.0.push_str(&format!("{b:02x}"));
            }
        }
        self.0.push(' ');
    }
    fn bytes(&mut self, b: &[u8]) {
        self.0.push_str(&hex(b));
        self.0.push(' ');
    }
    fn s(&mut self, s: Option<&str>) {
        self.0.push_str(&enc(s.map(str::as_bytes)));
        self.0.push(' ');
    }
}

/// The fixture `fsio` makes (as far as the platform allows: no permissions or links elsewhere).
pub fn make_fixture(fix: &Path) {
    let _ = std::fs::remove_dir_all(fix);
    std::fs::create_dir_all(fix.join("d")).unwrap();
    std::fs::write(fix.join("f"), b"abc\n").unwrap();
    std::fs::write(fix.join("d/i"), b"x").unwrap();
    std::fs::write(fix.join("nr"), b"secret").unwrap();
    std::fs::create_dir(fix.join("ro")).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(fix.join("nr"), std::fs::Permissions::from_mode(0o000)).unwrap();
        std::fs::set_permissions(fix.join("ro"), std::fs::Permissions::from_mode(0o500)).unwrap();
        std::os::unix::fs::symlink("f", fix.join("ln")).unwrap();
        std::os::unix::fs::symlink("d", fix.join("dln")).unwrap();
        std::os::unix::fs::symlink("nowhere", fix.join("bad")).unwrap();
    }
}

/// Removes the fixture (permissions given back first).
pub fn remove_fixture(fix: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(fix.join("nr"), std::fs::Permissions::from_mode(0o600));
        let _ = std::fs::set_permissions(fix.join("ro"), std::fs::Permissions::from_mode(0o700));
    }
    let _ = std::fs::remove_dir_all(fix);
}

/// Where a case's files live: `fix` (the fixture, the program's directory) and `dat` (the data
/// file, outside it).
pub struct Work {
    pub fix: PathBuf,
    pub dat: PathBuf,
}

/// Where the unknown digits of an `n`-byte big-endian number begin when the file gave `got` of its
/// bytes: the number's bytes in printing order are its memory's, so the unknown ones come last.
fn mem_of(n: usize, got: usize) -> usize {
    got.min(n)
}

/// The port's answer to one case line.
pub fn run_line(line: &str, mode: UpstreamDefects, w: &Work) -> (String, Vec<&'static str>) {
    let shared = Shared::default();
    let mut io = shared.clone();
    let mut g = Globals::new(mode);
    let t: Vec<&str> = line.split_whitespace().collect();
    let mut o = Out::default();
    let mut extra: Vec<&'static str> = Vec::new();
    let res: Result<(), u8> = (|| {
        if piece2::KINDS.contains(&t[0]) {
            let mut o2 = shared.clone();
            let mut e2 = shared.clone();
            return piece2::run(&t, &mut g, &mut o2, &mut e2, &mut o, w, &mut extra);
        }
        if piece3::KINDS.contains(&t[0]) {
            let mut o2 = shared.clone();
            let mut e2 = shared.clone();
            return piece3::run(&t, &mut g, &mut o2, &mut e2, &mut o, w, &mut extra);
        }
        if piece4::KINDS.contains(&t[0]) {
            let mut o2 = shared.clone();
            let mut e2 = shared.clone();
            return piece4::run(&t, &mut g, &mut o2, &mut e2, &mut o, w, &mut extra);
        }
        if piece5::KINDS.contains(&t[0]) {
            let mut o2 = shared.clone();
            let mut e2 = shared.clone();
            return piece5::run(&t, &mut g, &mut o2, &mut e2, &mut o, w, &mut extra);
        }
        if piece6::KINDS.contains(&t[0]) {
            let mut o2 = shared.clone();
            let mut e2 = shared.clone();
            return piece6::run(&t, &mut g, &mut o2, &mut e2, &mut o, w, &mut extra);
        }
        if piece7::KINDS.contains(&t[0]) {
            let mut o2 = shared.clone();
            let mut e2 = shared.clone();
            return piece7::run(&t, &mut g, &mut o2, &mut e2, &mut o, w, &mut extra);
        }
        if piece8::KINDS.contains(&t[0]) {
            let mut o2 = shared.clone();
            let mut e2 = shared.clone();
            return piece8::run(&t, &mut g, &mut o2, &mut e2, &mut o, w, &mut extra);
        }
        if piece9::KINDS.contains(&t[0]) {
            let mut o2 = shared.clone();
            let mut e2 = shared.clone();
            return piece9::run(&t, &mut g, &mut o2, &mut e2, &mut o, w, &mut extra);
        }
        if piece10::KINDS.contains(&t[0]) {
            let mut o2 = shared.clone();
            let mut e2 = shared.clone();
            return piece10::run(&t, &mut g, &mut o2, &mut e2, &mut o, w, &mut extra);
        }
        if piece11::KINDS.contains(&t[0]) {
            let mut o2 = shared.clone();
            let mut e2 = shared.clone();
            return piece11::run(&t, &mut g, &mut o2, &mut e2, &mut o, w, &mut extra);
        }
        match t[0] {
            "rd" => {
                g.gdiag_no = t[2].parse().unwrap();
                let count: usize = t[3].parse().unwrap();
                let init = u32::from_str_radix(t[4], 16).unwrap() as i32;
                let data = unhex(t[5]);
                std::fs::write(&w.dat, &data).unwrap();
                let mut fp = CFile::open(None, w.dat.to_str().unwrap(), "rb").unwrap();
                let mut rem = data.len();
                for _ in 0..count {
                    let mut take = |n: usize| {
                        let got = rem.min(n);
                        rem -= got;
                        got
                    };
                    match t[1] {
                        "r1" => {
                            let got = take(1);
                            let (r, v) = fio::fread1(&mut g, &mut fp);
                            o.i(i64::from(r));
                            o.x(&v.to_be_bytes(), if got < 1 { 3 } else { 4 });
                        }
                        "r2" => {
                            let got = take(2);
                            let (r, v) = fio::fread2(&mut g, &mut fp);
                            o.i(i64::from(r));
                            o.x(&v.to_be_bytes(), if got == 0 { 0 } else { 2 + got });
                        }
                        "r3" => {
                            take(3);
                            let (r, v) = fio::fread3(&mut fp);
                            o.i(i64::from(r));
                            o.x(&v.to_be_bytes(), 4);
                        }
                        "rd" => {
                            let got = take(8);
                            let v = fio::fread_double(&mut g, &mut io, &mut fp);
                            o.x(&v.to_be_bytes(), mem_of(8, got));
                        }
                        "ri" => {
                            let got = take(4);
                            let v = fio::fread_int(&mut g, &mut fp);
                            o.x(&v.to_be_bytes(), mem_of(4, got));
                        }
                        "rl" => {
                            let got = take(8);
                            let v = fio::fread_long(&mut g, &mut io, &mut fp);
                            // FS-176's rounding can carry from the unknown bytes into the others
                            o.x(&v.to_be_bytes(), if got < 8 { 0 } else { 8 });
                        }
                        "rix" => {
                            take(4);
                            let mut v = init;
                            let r = fio::fread_int_ex(&mut fp, &mut v);
                            o.i(i64::from(r));
                            o.x(&v.to_be_bytes(), 4);
                        }
                        "rf" => {
                            let got = take(4);
                            let v = fio::fread_float(&mut g, &mut io, &mut fp);
                            o.x(&v.to_be_bytes(), mem_of(4, got));
                        }
                        "r4" => {
                            let got = take(4);
                            let (r, v) = fio::fread4(&mut g, &mut fp);
                            o.i(i64::from(r));
                            o.x(&v.to_be_bytes(), mem_of(4, got));
                        }
                        "gf" => {
                            let got = take(4);
                            let v = fio::getf(&mut g, &mut io, &mut fp);
                            o.x(&v.to_be_bytes(), mem_of(4, got));
                        }
                        "rs" => {
                            let got = take(2);
                            let v = fio::fread_short(&mut g, &mut io, &mut fp);
                            o.x(&v.to_be_bytes(), mem_of(2, got));
                        }
                        "rfx" => {
                            take(4);
                            let mut v = f32::from_bits(init as u32);
                            let r = fio::fread_float_ex(&mut fp, &mut v);
                            o.i(i64::from(r));
                            o.x(&v.to_be_bytes(), 4);
                        }
                        "rdx" => {
                            take(8);
                            let u = init as u32 as u64;
                            let mut v = f64::from_bits((u << 32) | u);
                            let r = fio::fread_double_ex(&mut fp, &mut v);
                            o.i(i64::from(r));
                            o.x(&v.to_be_bytes(), 8);
                        }
                        _ => {
                            take(2);
                            let mut v = init as i16;
                            let r = fio::fread_short_ex(&mut fp, &mut v);
                            o.i(i64::from(r));
                            o.x(&v.to_be_bytes(), 2);
                        }
                    }
                }
                o.i(i64::from(fp.feof()));
                fp.close();
            }
            "zr" => {
                g.gdiag_no = t[2].parse().unwrap();
                let zmode: i32 = t[3].parse().unwrap();
                let count: usize = t[4].parse().unwrap();
                let init = u32::from_str_radix(t[5], 16).unwrap();
                let data = unhex(t[6]);
                let path = w.dat.to_str().unwrap().as_bytes().to_vec();
                if zmode == 1 {
                    let mut z = ZnzFile::open(&path, b"wb", true).unwrap();
                    if !data.is_empty() {
                        fio::fs_znzwrite(&mut z, &data, 1, data.len());
                    }
                    z.close();
                } else {
                    std::fs::write(&w.dat, &data).unwrap();
                }
                let mut z = ZnzFile::open(&path, b"rb", zmode != 0).unwrap();
                let mut rem = data.len();
                for _ in 0..count {
                    let mut take = |n: usize| {
                        let got = rem.min(n);
                        rem -= got;
                        got
                    };
                    match t[1] {
                        "z1" => {
                            let got = take(1);
                            let (r, v) = fio::znzread1(&mut g, &mut z);
                            o.i(i64::from(r));
                            o.x(&v.to_be_bytes(), if got < 1 { 3 } else { 4 });
                        }
                        "zs" => {
                            let got = take(2);
                            let v = fio::znz_read_short(&mut g, &mut io, &mut z);
                            o.x(&v.to_be_bytes(), mem_of(2, got));
                        }
                        "zi" => {
                            let got = take(4);
                            let v = fio::znz_read_int(&mut g, &mut io, &mut z);
                            o.x(&v.to_be_bytes(), mem_of(4, got));
                        }
                        "zl" => {
                            let got = take(8);
                            let v = fio::znz_read_long(&mut g, &mut io, &mut z);
                            o.x(&v.to_be_bytes(), if got < 8 { 0 } else { 8 });
                        }
                        "zf" => {
                            let got = take(4);
                            let v = fio::znz_read_float(&mut g, &mut io, &mut z);
                            o.x(&v.to_be_bytes(), mem_of(4, got));
                        }
                        "zd" => {
                            let got = take(8);
                            let v = fio::znz_read_double(&mut g, &mut io, &mut z);
                            o.x(&v.to_be_bytes(), mem_of(8, got));
                        }
                        "zix" => {
                            take(4);
                            let mut v = init as i32;
                            let r = fio::znz_read_int_ex(&mut z, &mut v);
                            o.i(i64::from(r));
                            o.x(&v.to_be_bytes(), 4);
                        }
                        "zfx" => {
                            take(4);
                            let mut v = f32::from_bits(init);
                            let r = fio::znz_read_float_ex(&mut z, &mut v);
                            o.i(i64::from(r));
                            o.x(&v.to_be_bytes(), 4);
                        }
                        "z2" => {
                            let got = take(2);
                            let (r, v) = fio::znzread2(&mut g, &mut z);
                            o.i(i64::from(r));
                            o.x(&v.to_be_bytes(), if got == 0 { 0 } else { 2 + got });
                        }
                        "z3" => {
                            take(3);
                            let (r, v) = fio::znzread3(&mut z);
                            o.i(i64::from(r));
                            o.x(&v.to_be_bytes(), 4);
                        }
                        "z4" => {
                            let got = take(4);
                            let (r, v) = fio::znzread4(&mut g, &mut z);
                            o.i(i64::from(r));
                            o.x(&v.to_be_bytes(), mem_of(4, got));
                        }
                        "zdx" => {
                            take(8);
                            let u = u64::from(init);
                            let mut v = f64::from_bits((u << 32) | u);
                            let r = fio::znz_read_double_ex(&mut z, &mut v);
                            o.i(i64::from(r));
                            o.x(&v.to_be_bytes(), 8);
                        }
                        _ => {
                            take(2);
                            let mut v = init as i16;
                            let r = fio::znz_read_short_ex(&mut z, &mut v);
                            o.i(i64::from(r));
                            o.x(&v.to_be_bytes(), 2);
                        }
                    }
                }
                o.i(i64::from(z.eof()));
                z.close();
            }
            "wr" => {
                let mut fp = CFile::open(None, w.dat.to_str().unwrap(), "wb").unwrap();
                let v = t[2];
                let r = match t[1] {
                    "w2" => fio::fwrite2(&mut fp, v.parse::<i64>().unwrap() as i32),
                    "w3" => fio::fwrite3(&mut fp, v.parse::<i64>().unwrap() as i32),
                    "wi" => fio::fwrite_int(&mut fp, v.parse::<i64>().unwrap() as i32),
                    "wl" => fio::fwrite_long(&mut g, &mut fp, v.parse::<i64>().unwrap()),
                    "w1" => fio::fwrite1(&mut fp, v.parse::<i64>().unwrap() as i32),
                    "w4" => fio::fwrite4(&mut fp, v.parse::<i64>().unwrap() as i32),
                    "wsh" => fio::fwrite_short(&mut fp, v.parse::<i64>().unwrap() as i16),
                    "pf" => fio::putf(&mut fp, f32::from_bits(u32::from_str_radix(v, 16).unwrap())),
                    "wd" => fio::fwrite_double(
                        &mut fp,
                        f64::from_bits(u64::from_str_radix(v, 16).unwrap()),
                    ),
                    _ => fio::fwrite_float(
                        &mut fp,
                        f32::from_bits(u32::from_str_radix(v, 16).unwrap()),
                    ),
                };
                fp.close();
                o.i(i64::from(r));
                o.bytes(&std::fs::read(&w.dat).unwrap());
            }
            "zw" => {
                let gz = t[2] == "1";
                let path = w.dat.to_str().unwrap().as_bytes().to_vec();
                let mut z = ZnzFile::open(&path, b"wb", gz).unwrap();
                let v = t[3];
                let r = match t[1] {
                    "zws" => fio::znz_write_short(&mut z, v.parse::<i64>().unwrap() as i16),
                    "zwus" => fio::znz_write_ushort(&mut z, v.parse::<i64>().unwrap() as u16),
                    "zwi" => fio::znz_write_int(&mut z, v.parse::<i64>().unwrap() as i32),
                    "zwl" => fio::znz_write_long(&mut g, &mut z, v.parse::<i64>().unwrap()),
                    "zwf" => fio::znz_write_float(
                        &mut z,
                        f32::from_bits(u32::from_str_radix(v, 16).unwrap()),
                    ),
                    "zw1" => fio::znzwrite1(&mut z, v.parse::<i64>().unwrap() as i32),
                    "zw2" => fio::znzwrite2(&mut z, v.parse::<i64>().unwrap() as i32),
                    "zw3" => fio::znzwrite3(&mut z, v.parse::<i64>().unwrap() as i32),
                    "zw4" => fio::znzwrite4(&mut z, v.parse::<i64>().unwrap() as i32),
                    _ => fio::znz_write_double(
                        &mut z,
                        f64::from_bits(u64::from_str_radix(v, 16).unwrap()),
                    ),
                };
                z.close();
                o.i(i64::from(r));
                let mut z = ZnzFile::open(&path, b"rb", gz).unwrap();
                let mut back = Vec::new();
                let mut c = [0u8; 1];
                while fio::fs_znzread(&mut z, &mut c, 1, 1) == 1 {
                    back.push(c[0]);
                }
                z.close();
                o.bytes(&back);
            }
            "dirname" => {
                let s = dec(t[1]).map(|b| String::from_utf8(b).unwrap());
                let r = fio::fio_dirname(&mut g, s.as_deref());
                o.s(r.as_deref());
            }
            "basename" => {
                let s = dec(t[1]).map(|b| String::from_utf8(b).unwrap());
                let e = dec(t[2]).map(|b| String::from_utf8(b).unwrap());
                let r = fio::fio_basename(s.as_deref(), e.as_deref());
                o.s(r.as_deref());
            }
            "ext" => {
                let s = dec(t[1]).map(|b| String::from_utf8(b).unwrap());
                let r = fio::fio_extension(s.as_deref());
                o.s(r.as_deref());
            }
            "fsx" => {
                let s = String::from_utf8(dec(t[2]).unwrap()).unwrap();
                let dir = Some(w.fix.as_path());
                match t[1] {
                    "exists" => o.i(i64::from(fio::fio_file_exists_readable(&mut g, dir, &s))),
                    "isdir" => o.i(i64::from(fio::fio_is_directory(&mut g, dir, &s))),
                    f => {
                        let fl = i32::from(f == "dirw1");
                        let r = fio::fio_dir_is_writable(&mut g, dir, &s, fl);
                        let junk = if fl != 0 {
                            format!("{s}.junk.54_-_sdfklj")
                        } else {
                            format!("{s}/.junk.54_-_sdfklj")
                        };
                        o.i(i64::from(r));
                        o.i(i64::from(w.fix.join(junk).exists()));
                    }
                }
                o.i(i64::from(g.errno));
            }
            "tsv" => {
                let debug: i32 = t[1].parse().unwrap();
                let fmt = dec(t[2]).map(|b| String::from_utf8(b).unwrap());
                let tf = w.fix.join("t.tsv");
                let _ = std::fs::remove_file(&tf);
                if t[3] != "N" {
                    std::fs::write(&tf, unhex(t[3])).unwrap();
                }
                let mut tsv = Tsv {
                    debug,
                    ..Tsv::default()
                };
                let dir = Some(w.fix.as_path());
                o.i(i64::from(fio::tsv_read(
                    &mut g, &mut io, dir, &mut tsv, "t.tsv",
                )));
                o.i(i64::from(tsv.ncols()));
                for h in &tsv.headers {
                    o.0.push_str(&enc(Some(h)));
                    o.0.push(' ');
                }
                o.i(i64::from(tsv.nrows()));
                for row in &tsv.data {
                    o.i(row.len() as i64);
                    for v in row {
                        o.x(&v.to_be_bytes(), 8);
                    }
                }
                o.i(i64::from(g.errno));
                if let Some(f) = fmt {
                    o.i(i64::from(fio::tsv_write(
                        &mut g, &mut io, dir, &tsv, "o.tsv", &f,
                    )));
                    let of = w.fix.join("o.tsv");
                    o.bytes(&std::fs::read(&of).unwrap_or_default());
                    let _ = std::fs::remove_file(of);
                }
                let _ = std::fs::remove_file(tf);
            }
            "swap" => {
                let count: i64 = t[2].parse().unwrap();
                let mut b = unhex(t[4]);
                match t[1] {
                    "s2" => bfileio::byteswapbufshort(&mut g, &mut b, count),
                    "s4" => bfileio::byteswapbuffloat(&mut g, &mut b, count),
                    _ => bfileio::byteswapbufdouble(&mut g, &mut b, count),
                };
                o.bytes(&b);
            }
            _ => {
                let nmemb: u64 = t[1].parse().unwrap();
                let size: u64 = t[2].parse().unwrap();
                match mriio_old::lcalloc::<u8>(&mut g, &mut io, nmemb as usize, size as usize) {
                    Ok(v) => {
                        o.0.push_str("ok ");
                        o.i(i64::from(v.iter().take(64).any(|&c| c != 0)));
                    }
                    Err(f) => return Err(f.status()),
                }
            }
        }
        Ok(())
    })();
    let refused_tsv = shared
        .0
        .borrow()
        .windows(32)
        .any(|w| w == b"has an entry that is not a numbe");
    let mut raw = shared.0.borrow().clone();
    if piece4::KINDS.contains(&t[0]) || piece6::KINDS.contains(&t[0]) {
        // CTN's messages name the file by the port's path into the fixture; fsio runs in the
        // fixture and names it from there
        let mut prefix = w.fix.to_string_lossy().into_owned().into_bytes();
        prefix.push(std::path::MAIN_SEPARATOR as u8);
        raw = replace_all(&raw, &prefix, b"");
    }
    // as fsio: messages beyond 1 MiB shown by their length
    let text = if raw.len() > 1 << 20 {
        format!("BIGTEXT {}", raw.len())
    } else {
        escape(&raw)
    };
    let result = match res {
        Ok(()) => o.0.trim_end().to_owned(),
        Err(status) => format!("EXIT {status}"),
    };
    let mut ids: Vec<&'static str> = g.notices.as_slice().iter().map(|n| n.defect.id).collect();
    for n in g.notices.as_slice() {
        let id = n.defect.id;
        // FS-172: where the port refuses the table (FreeSurfer reads for ever there) or where
        // FreeSurfer stores memory contents (always the first FS-172 of a read)
        let skip = matches!(
            id,
            "FS-169"
                | "FS-173"
                | "FS-175"
                | "FS-179"
                | "FS-181"
                | "FS-183"
                | "FS-185"
                | "FS-189"
                | "FS-191"
        ) || (id == "FS-172" && (refused_tsv || n.detail.contains("memory contents")))
            || (id == "FS-188" && n.detail.contains("past the entry"));
        if skip {
            extra.push("SKIP");
        }
        // piece 2: FreeSurfer's numbers or names from memory contents
        let memory = piece2::KINDS.contains(&t[0]) && matches!(id, "FS-49" | "FS-167");
        if memory || matches!(id, "FS-168" | "FS-170" | "FS-180" | "FS-187" | "FS-192") {
            extra.push("VARY");
        }
    }
    ids.extend(extra);
    ids.sort_unstable();
    ids.dedup();
    (format!("{result} | {text}"), ids)
}
