// SPDX-License-Identifier: Apache-2.0
//! Golden cases of fs.core's clean room (spec §20.6, CLAUDE.md rule 10): one case per path,
//! inputs from a seeded generator and plain arithmetic, the SHA-256 of every result's bytes
//! (NaN bits included) recorded on Linux x86-64 and checked on every platform. No file system,
//! clock or environment beyond what each case sets (the zone cases read a zone file built in
//! memory, and POSIX strings under a `TZDIR` that does not exist).
//!
//! Record: `FS_CORE_GOLDEN_RECORD=1 cargo test -p larmorx-fs-core --test golden -- --nocapture`
//! prints `<case> <sha256>` for each case instead of checking it. The cases also run under the
//! AArch64 NaN emulation (`validation/investigations/freesurfer-cleanroom-core/README.md`).

use std::io::Write;
use std::sync::{Arc, Mutex};

use larmorx_core::defects::UpstreamDefects;
use larmorx_fs_core::clib::time::{Zone, strftime_c};
use larmorx_fs_core::numbers::{self, ListStats};
use larmorx_fs_core::{Environment, Process, Stop, atan, cmdargs, romp, strings, swap};
use sha2::{Digest, Sha256};

const GOLDEN: &[(&str, &str)] = &[
    (
        "swaps.fix",
        "4aaf43ad928b3761f86aba164efd895b0d7ba7314418593c79f5df74e382f9ed",
    ),
    (
        "swaps.reproduce",
        "d7bd4619d8152504389a368d76895f8590eaa1e2db92624cc3db9fdf1b0e06b4",
    ),
    (
        "strings",
        "99512f9b6a6b659162dbd05f08cdb6a33c6a4ceef8840ca7b32692a0149c3548",
    ),
    (
        "cmdargs",
        "51caba9105d8421a7f1bd9a1cf746ae65ac28dfae722973d105174cc0f970f89",
    ),
    (
        "numbers.fix",
        "da2ecc68f10090a6732c9b1ca732a65ced031f35666549d65d8cae728446c5ed",
    ),
    (
        "numbers.reproduce",
        "791d9137c887da86e94642df0c2b36c77b9463afcd63de6e4f7dbfc82cfa7bb8",
    ),
    (
        "lists.fix",
        "b8813e1bbf037edfe108485287d75c5cc965394e3f933f51e16a8b4fe2339f95",
    ),
    (
        "select",
        "22a175414d382f3f9d20fe3f0ea8e5e5ae4231ce9defdd70386f0dc8434642c3",
    ),
    (
        "atan2",
        "466706aaef6203693c865e220dd169ce552dc023de86f16e5c43da3a73ff9b6f",
    ),
    (
        "romp",
        "104db911e9b25033d9de13554ff088ab57ebee86a7c4922924bf842ee2f73b39",
    ),
    (
        "random",
        "41e72222ad03a2a96b9fe8778b3c0f72e229a2e30c4a86b8f14b5442840cabe7",
    ),
    (
        "version.fix",
        "05020c961cc72032ed2a14896257e27b1e5a8eb2b3ec88c2482bf2d758ef4ce0",
    ),
    (
        "messages.fix",
        "bfb539dc83ddd13e07a2302e3efe17fab5f700b61f90317cface46718793aa48",
    ),
    (
        "messages.reproduce",
        "b1e81c15de3c93ed32c0f59fad620dfc0418872d99b5999f3276826c1a3d8bbb",
    ),
    (
        "time.posix",
        "e8496cff1c6aa64e6c7b9622a84d87658274a71b042872d585afdaa30ee81e8c",
    ),
    (
        "time.tzif",
        "7f530229ea0b3f9085bcd26a4be5792ad3223fa57c79bf6ad2b0415553850c40",
    ),
    (
        "diag",
        "649ae109f6938355bae454eafa112afc1098bfad1a6e00d7c0bd7502a90b0155",
    ),
    (
        "logic",
        "2827b2e301864e0c636c8dd5095fbc8f4e30b7adea2c7f166821a9e82484f778",
    ),
    (
        "xdbg",
        "f065a63808f10feaff0ab485bfa0e9be2bd636575c7a432485ff5ae1f93a3a35",
    ),
];

/// Checks (or, with `FS_CORE_GOLDEN_RECORD` set, prints) a case's hash.
fn check(name: &str, bytes: &[u8]) {
    let hex: String = Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    if std::env::var_os("FS_CORE_GOLDEN_RECORD").is_some() {
        println!("GOLDEN {name} {hex}");
        return;
    }
    let want = GOLDEN
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, h)| *h)
        .unwrap_or_else(|| panic!("no golden hash for {name}"));
    assert_eq!(hex, want, "golden case {name}");
}

/// splitmix64: the cases' seeded inputs.
struct Gen(u64);

impl Gen {
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

    /// A float from a few special values, small integers, ratios and raw bit patterns.
    fn f32(&mut self) -> f32 {
        match self.below(8) {
            0 => [
                0.0,
                -0.0,
                f32::INFINITY,
                f32::NEG_INFINITY,
                f32::NAN,
                -f32::NAN,
                f32::MAX,
            ][self.below(7) as usize],
            1 | 2 => (self.below(41) as f32) - 20.0,
            3 | 4 => ((self.below(20001) as f32) - 10000.0) / 64.0,
            5 => f32::from_bits(self.next() as u32),
            _ => ((self.below(2001) as f32) - 1000.0) / 7.0,
        }
    }

    fn f64(&mut self) -> f64 {
        match self.below(8) {
            0 => [
                0.0,
                -0.0,
                f64::INFINITY,
                f64::NEG_INFINITY,
                f64::NAN,
                2147483647.5,
                -2147483648.5,
                1e300,
            ][self.below(8) as usize],
            1 | 2 => {
                (self.below(201) as f64) - 100.0 + [0.0, 0.5, 0.25, 0.75][self.below(4) as usize]
            }
            3 => f64::from_bits(self.next()),
            _ => ((self.below(2_000_001) as f64) - 1_000_000.0) / 1024.0,
        }
    }

    fn bytes(&mut self, max: u64) -> Vec<u8> {
        const ALPHA: &[u8] = b"ab.c/ #@\t\nXYZ-_~.\x80\xe9\xff09 ";
        (0..self.below(max + 1))
            .map(|_| ALPHA[self.below(ALPHA.len() as u64) as usize])
            .collect()
    }
}

/// A shared buffer for a run's output streams.
#[derive(Clone, Default)]
struct Buf(Arc<Mutex<Vec<u8>>>);

impl Write for Buf {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Buf {
    fn take(&self) -> Vec<u8> {
        std::mem::take(&mut *self.0.lock().unwrap())
    }
}

/// A run with captured streams (standard output, standard error).
fn run(env: Environment, mode: UpstreamDefects) -> (Process, Buf, Buf) {
    let (o, e) = (Buf::default(), Buf::default());
    let p = Process::new(env, mode, Box::new(o.clone()), Box::new(e.clone()));
    (p, o, e)
}

fn modes() -> [(UpstreamDefects, &'static str); 2] {
    [
        (UpstreamDefects::Fix, "fix"),
        (UpstreamDefects::Reproduce, "reproduce"),
    ]
}

fn put(out: &mut Vec<u8>, b: &[u8]) {
    out.extend_from_slice(&(b.len() as u32).to_le_bytes());
    out.extend_from_slice(b);
}

#[test]
fn swaps() {
    for (mode, name) in modes() {
        let mut g = Gen(1);
        let (mut p, _, _) = run(Environment::new(), mode);
        let mut out = Vec::new();
        for _ in 0..300 {
            let v = g.next();
            out.extend_from_slice(&swap::swap_i16(v as i16).to_le_bytes());
            out.extend_from_slice(&swap::swap_u16(v as u16).to_le_bytes());
            out.extend_from_slice(&swap::swap_i32(v as i32).to_le_bytes());
            out.extend_from_slice(&swap::swap_f32_bits(v as u32).to_le_bytes());
            out.extend_from_slice(&swap::swap_f64_bits(v).to_le_bytes());
            out.extend_from_slice(&p.swap_i64(v as i64).to_le_bytes());
        }
        for k in [1, 2, 3, 4, 8] {
            for n in [0i64, 1, 7, 8, 16, 33] {
                let mut b: Vec<u8> = (0..48).map(|_| g.next() as u8).collect();
                let r = p.byte_swap_buf(&mut b, n, k);
                out.extend_from_slice(&r.to_le_bytes());
                out.extend_from_slice(&b);
            }
        }
        check(&format!("swaps.{name}"), &out);
    }
}

#[test]
fn string_helpers() {
    let mut g = Gen(2);
    let (mut p, _, _) = run(Environment::new(), UpstreamDefects::Fix);
    let mut out = Vec::new();
    for _ in 0..300 {
        let s = g.bytes(24);
        let t = g.bytes(6);
        let mut u = s.clone();
        strings::str_upper(&mut u);
        put(&mut out, &u);
        let mut l = s.clone();
        strings::str_lower(&mut l);
        put(&mut out, &l);
        put(&mut out, &strings::file_name_only(&s));
        put(&mut out, &strings::file_name_path(&s));
        put(&mut out, &strings::file_name_remove_extension(&s));
        put(&mut out, &p.file_name_extension(&s));
        let mut f = s.clone();
        let at = strings::file_name_in_place(&mut f);
        put(&mut out, &f[at..]);
        out.extend_from_slice(&p.stricmp(&s, &t).to_le_bytes());
        out.extend_from_slice(&strings::items_in_string(&s).to_le_bytes());
        out.extend_from_slice(&strings::count_items_in_string(&s).to_le_bytes());
        for n in [-1, 0, 1, 2] {
            match p.nth_item(&s, n) {
                Some(v) => put(&mut out, &v),
                None => out.push(0xee),
            }
        }
        put(&mut out, &strings::deblank(&s));
        out.push(u8::from(strings::string_ends_with(&s, &t)));
        put(&mut out, &strings::argv_to_cmdline(&[&s, &t]));
        let mut r = std::io::Cursor::new(s.clone());
        while let Some(line) = strings::fgetl(&mut r, 2 + g.below(12) as usize) {
            put(&mut out, &line);
        }
    }
    check("strings", &out);
}

#[test]
fn command_line_helpers() {
    let mut g = Gen(3);
    let mut out = Vec::new();
    for _ in 0..200 {
        let a = g.bytes(5);
        let b = [b"-".as_slice(), b"--", b"-x", b"--long", b"5", b"-5", b""][g.below(7) as usize];
        out.push(u8::from(cmdargs::is_flag(&a)));
        out.push(u8::from(cmdargs::is_flag(b)));
        out.push(u8::from(cmdargs::is_single_dash(b)));
        out.push(u8::from(cmdargs::string_match(Some(&a), Some(b))));
        out.push(u8::from(cmdargs::string_match(None, Some(b))));
        let args: Vec<&[u8]> = vec![b, &a, b"x", b"-y"];
        out.push(u8::from(cmdargs::nth_is_arg(&args, g.below(6) as i64 - 1)));
    }
    check("cmdargs", &out);
}

#[test]
fn rounding_and_numbers() {
    for (mode, name) in modes() {
        let mut g = Gen(4);
        let (mut p, o, e) = run(Environment::new(), mode);
        let mut out = Vec::new();
        for _ in 0..400 {
            let (x, y) = (g.f64(), g.f64());
            out.extend_from_slice(&p.nint(x).to_le_bytes());
            out.extend_from_slice(&p.nint2(y).to_le_bytes());
            let n = g.below(9) as i32 - 2;
            out.extend_from_slice(&numbers::sum2stddev(x, y, n).to_bits().to_le_bytes());
            let (a, b) = (g.f32(), g.f32());
            out.extend_from_slice(&numbers::delta_angle(a, b).to_bits().to_le_bytes());
            out.extend_from_slice(&numbers::compare_floats(a, b).to_le_bytes());
            out.extend_from_slice(&numbers::compare_ints(x as i32, y as i32).to_le_bytes());
            out.extend_from_slice(&numbers::dev_isinf(a).to_le_bytes());
            out.push(u8::from(numbers::dev_isnan(a)));
            out.push(u8::from(numbers::dev_finite(b)));
            out.extend_from_slice(&numbers::squaref(a).to_bits().to_le_bytes());
            out.extend_from_slice(&numbers::squared(x).to_bits().to_le_bytes());
            out.extend_from_slice(&p.check_finite(b"place", x).to_le_bytes());
        }
        for len in [0usize, 1, 2, 5, 12] {
            let list: Vec<f64> = (0..len).map(|_| g.f64()).collect();
            match p.dlist_stats(Some(&list), len as i32) {
                ListStats::Stats(s) => s
                    .iter()
                    .for_each(|v| out.extend_from_slice(&v.to_bits().to_le_bytes())),
                ListStats::None => out.push(0xee),
            }
        }
        p.flush();
        put(&mut out, &o.take());
        put(&mut out, &e.take());
        check(&format!("numbers.{name}"), &out);
    }
}

#[test]
fn integer_lists() {
    let mut g = Gen(5);
    let (mut p, _, _) = run(Environment::new(), UpstreamDefects::Fix);
    let mut out = Vec::new();
    for len in 0..40usize {
        let base: Vec<i32> = (0..len).map(|_| g.below(9) as i32 - 4).collect();
        let mut a = base.clone();
        out.extend_from_slice(&p.nunique_int_list(&mut a).to_le_bytes());
        a.iter()
            .for_each(|v| out.extend_from_slice(&v.to_le_bytes()));
        let mut b = base.clone();
        for v in p.unique_int_list(&mut b) {
            out.extend_from_slice(&v.to_le_bytes());
        }
        let mut c = base.clone();
        let (m, n) = p.most_frequent_int_list(&mut c);
        out.extend_from_slice(&m.to_le_bytes());
        out.extend_from_slice(&n.to_le_bytes());
        out.extend_from_slice(&strings::find_first_match(&base, 2).to_le_bytes());
    }
    check("lists.fix", &out);
}

#[test]
fn selections_and_median() {
    let mut g = Gen(6);
    let (mut p, _, _) = run(Environment::new(), UpstreamDefects::Fix);
    let mut out = Vec::new();
    for len in 0..60usize {
        let base: Vec<f32> = (0..len).map(|_| g.f32()).collect();
        let k = if len == 0 {
            1
        } else {
            1 + g.below(len as u64) as usize
        };
        if len > 0 {
            let mut a = base.clone();
            out.extend_from_slice(&numbers::kth_smallest(&mut a, k).to_bits().to_le_bytes());
            a.iter()
                .for_each(|v| out.extend_from_slice(&v.to_bits().to_le_bytes()));
            let mut b = base.clone();
            out.extend_from_slice(&numbers::quick_select(&mut b, k).to_bits().to_le_bytes());
            b.iter()
                .for_each(|v| out.extend_from_slice(&v.to_bits().to_le_bytes()));
        }
        let mut c = base.clone();
        out.extend_from_slice(&p.median(&mut c).to_bits().to_le_bytes());
        c.iter()
            .for_each(|v| out.extend_from_slice(&v.to_bits().to_le_bytes()));
    }
    check("select", &out);
}

#[test]
fn fast_arctangent() {
    let mut g = Gen(7);
    let mut out = Vec::new();
    for v in atan::table() {
        out.extend_from_slice(&v.to_bits().to_le_bytes());
    }
    for (mode, _) in modes() {
        let (mut p, _, _) = run(Environment::new(), mode);
        for _ in 0..400 {
            let (y, x) = (g.f32(), g.f32());
            out.extend_from_slice(&p.fast_approx_atan2f(y, x).to_bits().to_le_bytes());
        }
        for (y, x) in [
            (0.0, 0.0),
            (-0.0, 0.0),
            (0.0, -0.0),
            (-0.0, -0.0),
            (1.0, 100000.0),
            (f32::MAX, 1e-30),
        ] {
            out.extend_from_slice(&p.fast_approx_atan2f(y, x).to_bits().to_le_bytes());
        }
    }
    check("atan2", &out);
}

#[test]
fn reproducible_sums() {
    let mut g = Gen(8);
    let mut out = Vec::new();
    for len in [0usize, 1, 2, 127, 128, 129, 300, 1000] {
        let terms: Vec<f64> = (0..len).map(|_| g.f64()).collect();
        let first = romp::reproducible_sum(0.5, &terms, 1);
        for n in [3, 8] {
            assert_eq!(
                romp::reproducible_sum(0.5, &terms, n).to_bits(),
                first.to_bits()
            );
        }
        out.extend_from_slice(&first.to_bits().to_le_bytes());
        for (lo, hi) in romp::pieces(0, len as i64) {
            out.extend_from_slice(&lo.to_le_bytes());
            out.extend_from_slice(&hi.to_le_bytes());
        }
    }
    check("romp", &out);
}

#[test]
fn seeded_draws() {
    let mut out = Vec::new();
    for seed in [1234i64, 7, -42] {
        let (mut p, _, _) = run(Environment::new(), UpstreamDefects::Fix);
        p.set_random_seed(seed);
        for k in 0..40 {
            let lo = f64::from(k % 5) - 2.0;
            let r = p
                .random_number(lo, lo + f64::from(k % 7) + 1.0)
                .expect("a draw");
            out.extend_from_slice(&r.to_bits().to_le_bytes());
        }
        for v in p.compute_permutation(23).expect("a permutation") {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out.extend_from_slice(&p.random_seed().to_le_bytes());
        out.extend_from_slice(&p.random_calls().to_le_bytes());
    }
    check("random", &out);
}

#[test]
fn version_plans() {
    let mut g = Gen(9);
    let vocab: [&[u8]; 10] = [
        b"--version",
        b"-version",
        b"--versionx",
        b"-versionfoo",
        b"--all-info",
        b"-all-info",
        b"--all-infox",
        b"x",
        b"--x",
        b"-v",
    ];
    let (mut p, o, _) = run(
        Environment::from_pairs([("TZ", "UTC0")]),
        UpstreamDefects::Fix,
    );
    p.clock = Some(1_700_000_000);
    let mut out = Vec::new();
    for _ in 0..60 {
        let mut argv = vec![b"prog".to_vec()];
        for _ in 0..g.below(6) {
            argv.push(vocab[g.below(10) as usize].to_vec());
        }
        let (n, left) = p.handle_version_option(&argv, b"prog");
        out.extend_from_slice(&(n as u32).to_le_bytes());
        left.iter().for_each(|a| put(&mut out, a));
        // the all-info lines name the user, the machine and the release: kept to their start
        for line in o.take().split(|&c| c == b'\n') {
            let end = line
                .windows(7)
                .position(|w| w == b"  User:")
                .unwrap_or(line.len());
            put(&mut out, &line[..end]);
        }
    }
    check("version.fix", &out);
}

#[test]
fn messages() {
    for (mode, name) in modes() {
        let mut out = Vec::new();
        let (mut p, o, e) = run(Environment::new(), mode);
        p.progname = b"prog".to_vec();
        p.errno = 2;
        p.error_printf(-1, b"mghRead(/nonexistent.mgz, -1): could not open file");
        p.errno = 0;
        p.error_printf(-4, b"a\n");
        p.error(b"plain");
        p.warning(b"careful");
        p.debug(b"hidden");
        p.gdiag = 0x1000_0000;
        p.debug(b"shown");
        for code in [0, 1, -1, -5, 256, -256] {
            let s = p.fatal(code, b"fatal").err().map_or(-1, Stop::status);
            out.extend_from_slice(&s.to_le_bytes());
        }
        let s = p.cmd_arg_n_err(b"--flag", 2).status();
        out.extend_from_slice(&s.to_le_bytes());
        let s = p.assert_failed(b"f.c", 12, b"x > 0").status();
        out.extend_from_slice(&s.to_le_bytes());
        out.extend_from_slice(&p.hips_unsupported().to_le_bytes());
        p.set_throw_exceptions(true);
        let s = p.fatal(3, b"thrown").err().map_or(-1, Stop::status);
        out.extend_from_slice(&s.to_le_bytes());
        p.flush();
        put(&mut out, &o.take());
        put(&mut out, &e.take());
        check(&format!("messages.{name}"), &out);
    }
}

/// A version 2 zone file: two types (standard, daylight) and their transitions, one leap second
/// and a footer rule.
fn tzif() -> Vec<u8> {
    let trans: [i64; 4] = [-1_000_000_000, 1_000_000_000, 1_015_000_000, 1_031_000_000];
    let idx: [u8; 4] = [0, 1, 0, 1];
    let types: [(i32, u8, u8); 3] = [(3600, 0, 0), (7200, 1, 4), (1800, 0, 9)];
    let chars = b"STD\0DST\0\0OLD\0";
    let leaps: [(i64, i32); 1] = [(1_500_000_000, 1)];
    let block = |w: usize| {
        let mut b = Vec::new();
        for t in trans {
            if w == 8 {
                b.extend_from_slice(&t.to_be_bytes());
            } else {
                b.extend_from_slice(&(t as i32).to_be_bytes());
            }
        }
        b.extend_from_slice(&idx);
        for (off, dst, ab) in types {
            b.extend_from_slice(&off.to_be_bytes());
            b.push(dst);
            b.push(ab);
        }
        b.extend_from_slice(chars);
        for (o, c) in leaps {
            if w == 8 {
                b.extend_from_slice(&o.to_be_bytes());
            } else {
                b.extend_from_slice(&(o as i32).to_be_bytes());
            }
            b.extend_from_slice(&c.to_be_bytes());
        }
        b.extend_from_slice(&[0, 0, 1]); // standard indicators
        b.extend_from_slice(&[0, 0, 0]); // UT indicators
        b
    };
    let header = |b: &mut Vec<u8>| {
        b.extend_from_slice(b"TZif2");
        b.extend_from_slice(&[0; 15]);
        for c in [3u32, 3, 1, 4, 3, chars.len() as u32] {
            b.extend_from_slice(&c.to_be_bytes());
        }
    };
    let mut b = Vec::new();
    header(&mut b);
    b.extend_from_slice(&block(4));
    header(&mut b);
    b.extend_from_slice(&block(8));
    b.extend_from_slice(b"\nSTD-1DST,M3.5.0,M10.5.0/3\n");
    b
}

fn times(g: &mut Gen) -> Vec<i64> {
    let mut t: Vec<i64> = vec![
        0,
        -1,
        2147483647,
        -2147483648,
        -2208988800,
        4102444800,
        1_500_000_000,
        1_499_999_999,
    ];
    t.extend((0..200).map(|_| g.below(6_311_433_600) as i64 - 2_208_988_800));
    t
}

fn put_tm(out: &mut Vec<u8>, tm: Option<larmorx_fs_core::clib::time::Tm>) {
    match tm {
        None => out.push(0xee),
        Some(m) => {
            for v in [
                m.year, m.mon, m.mday, m.hour, m.min, m.sec, m.wday, m.yday, m.isdst,
            ] {
                out.extend_from_slice(&v.to_le_bytes());
            }
            out.extend_from_slice(&m.gmtoff.to_le_bytes());
            put(out, &m.zone);
            put(out, &strftime_c(&m));
        }
    }
}

#[test]
fn time_posix() {
    let mut g = Gen(10);
    let ts = times(&mut g);
    let mut out = Vec::new();
    for tz in [
        &b""[..],
        b"UTC0",
        b"EST5EDT,M3.2.0,M11.1.0",
        b"AEST-10AEDT,M10.1.0,M4.1.0/3",
        b"<+0530>-5:30",
        b"No/Such_Zone",
        b"Foo",
        b"FOO+25",
        b"EST5EDT4,J60,J300",
        b"EST5EDT,59/0,58/0",
        b"EST5EDT,M3.2.0/-1,M11.1.0/26",
        b"ABC5DEF",
        b"XYZ3:30",
    ] {
        let z = Zone::from_tz(Some(tz), Some(b"/nonexistent-zone-folder"));
        for &t in &ts {
            put_tm(&mut out, z.localtime(t));
        }
    }
    let (mut p, _, _) = run(
        Environment::from_pairs([("TZ", "UTC0")]),
        UpstreamDefects::Fix,
    );
    for t in [0, 1_700_000_000, 4_102_444_800] {
        p.clock = Some(t);
        put(&mut out, &p.cur_time_stamp());
        put(&mut out, &p.current_date_time(true));
    }
    check("time.posix", &out);
}

#[test]
fn time_zone_file() {
    let mut g = Gen(11);
    let ts = times(&mut g);
    let z = Zone::from_tzif(&tzif()).expect("the zone file reads");
    let mut out = Vec::new();
    for &t in &ts {
        put_tm(&mut out, z.localtime(t));
        put_tm(&mut out, z.gmtime(t));
    }
    check("time.tzif", &out);
}

#[test]
fn diagnostics() {
    let mut out = Vec::new();
    let envs: [&[(&str, &str)]; 5] = [
        &[],
        &[("DIAG", "0x1000"), ("DIAG_NO", "7")],
        &[("PROFILE", "7.9"), ("IMAGE_SIZE", "64")],
        &[("PROFILE", "1e10"), ("DIAG_X", "3"), ("DIAG_Y", "-2")],
        &[("DIAG_VERBOSE", "1")],
    ];
    for env in envs {
        let (mut p, o, e) = run(
            Environment::from_pairs(env.iter().copied()),
            UpstreamDefects::Fix,
        );
        out.extend_from_slice(&p.diag_init().to_le_bytes());
        for v in [
            p.gdiag_no,
            p.image_size,
            p.gprofile,
            p.gvoxel[0],
            p.gvoxel[1],
            p.gvoxel[2],
        ] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        p.gdiag |= 0x4000;
        for f in [0.0f32, 0.25, 0.5, 1.0] {
            p.diag_heartbeat(f);
            p.diag_show_pct_done(f, 4);
        }
        out.extend_from_slice(&p.diag_printf(0x4000, b"note").to_le_bytes());
        p.flush();
        put(&mut out, &o.take());
        put(&mut out, &e.take());
    }
    check("diag", &out);
}

#[test]
fn logic_problems() {
    let mut out = Vec::new();
    let (mut p, o, _) = run(
        Environment::from_pairs([("FREESURFER_reportLogicErrors", "1"), ("FIXIT", "")]),
        UpstreamDefects::Fix,
    );
    for line in [0, 3, 999_999] {
        for _ in 0..300 {
            let r = p
                .spend_time_checking_for_logic_problem(b"f.c", line)
                .expect("a line below a million");
            out.push(u8::from(r));
        }
    }
    for fixer in [Some(&b"FIXIT"[..]), Some(b"UNSET"), None] {
        for _ in 0..40 {
            let (r, rep) = p.cope_with_logic_problem(fixer, b"msg", b"g.c", 9, b"fn");
            out.push(r as u8);
            out.push(u8::from(rep));
        }
    }
    p.flush();
    put(&mut out, &o.take());
    check("logic", &out);
}

#[test]
fn debugging_stack() {
    let mut out = Vec::new();
    let (mut p, o, e) = run(Environment::new(), UpstreamDefects::Fix);
    let long = vec![b'q'; 700];
    for k in 0..53u32 {
        p.debug_enter_function(format!("f{k}").as_bytes());
        if k % 7 == 0 {
            p.xdbg_set_current_note(&long);
        }
        put(&mut out, &p.xdbg_current_function());
        put(&mut out, p.xdbg_current_note());
    }
    p.xdbg_set_stack_desc(&long);
    for _ in 0..56 {
        p.debug_exit_function();
        put(&mut out, &p.xdbg_current_function());
        put(&mut out, p.xdbg_current_note());
    }
    p.xdbg_push(b"t", b"n");
    p.xdbg_pop();
    p.xdbg_pop();
    put(&mut out, p.xdbg_stack_desc());
    p.flush();
    put(&mut out, &o.take());
    put(&mut out, &e.take());
    check("xdbg", &out);
}
