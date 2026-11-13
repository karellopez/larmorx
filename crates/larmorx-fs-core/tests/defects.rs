// SPDX-License-Identifier: Apache-2.0
//! The cases the probe cannot show (spec §20.1): both modes defined by larmorx, or a crash in
//! FreeSurfer. FS-03's exit status, FS-04's seed, FS-05's error line, FS-06's values on NaN,
//! infinite and overflowing pairs, FS-11 on strings of 1,024 bytes and more, FS-263's error
//! value; each checked against the spec's text, with its notice.

use std::io::Write;
use std::sync::{Arc, Mutex};

use larmorx_core::defects::UpstreamDefects;
use larmorx_core::vnl_random::VnlRandom;
use larmorx_fs_core::atan::{self, Lookup};
use larmorx_fs_core::{Environment, Process, Stop};

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
    fn text(&self) -> Vec<u8> {
        self.0.lock().unwrap().clone()
    }
}

fn run(env: Environment, mode: UpstreamDefects) -> (Process, Buf, Buf) {
    let (o, e) = (Buf::default(), Buf::default());
    let p = Process::new(env, mode, Box::new(o.clone()), Box::new(e.clone()));
    (p, o, e)
}

fn noted(p: &Process, id: &str) -> bool {
    let mut b = Vec::new();
    p.notices().print(&mut b);
    String::from_utf8_lossy(&b).contains(id)
}

#[test]
fn fs03_failed_assertion() {
    for (mode, status) in [(UpstreamDefects::Fix, 1), (UpstreamDefects::Reproduce, 139)] {
        let (mut p, o, e) = run(Environment::new(), mode);
        let s = p.assert_failed(b"f.cpp", 12, b"a < b");
        assert_eq!(s, Stop::Exit(status));
        let line = b"ASSERTION FAILED: f.cpp:12 a < b\n";
        assert!(o.text().starts_with(line));
        assert!(o.text().ends_with(b"/utils/diag.cpp:85\n"));
        assert_eq!(e.text(), line);
        assert!(noted(&p, "FS-03"));
    }
}

#[test]
fn fs04_unseeded_draw() {
    // fix: the current seed -1, the first number of a generator seeded with -1, count 1
    let (mut p, _, e) = run(Environment::new(), UpstreamDefects::Fix);
    let v = p.random_number(0.0, 1.0).unwrap();
    assert_eq!(p.random_seed(), -1);
    assert_eq!(p.random_calls(), 1);
    assert!(
        e.text()
            .ends_with(b" supposed to be reproducible but seed not set\n")
    );
    assert!(noted(&p, "FS-04"));
    // no draw discarded on this path (explicit seeding discards one)
    let mut g = VnlRandom::new(0);
    g.reseed((-1i64) as u64);
    assert_eq!(v.to_bits(), f64::from(g.drand64(0.0, 1.0) as f32).to_bits());
    // the line once per run
    p.set_random_seed(0);
    p.random_number(0.0, 1.0).unwrap();
    assert_eq!(e.text().iter().filter(|&&c| c == b'\n').count(), 1);
    // reproduce: -|t|, t the clock's seconds truncated to 32 bits
    let (mut r, _, _) = run(Environment::new(), UpstreamDefects::Reproduce);
    r.clock = Some(0x1_2345_6789);
    r.random_number(0.0, 1.0).unwrap();
    assert_eq!(r.random_seed(), -(0x2345_6789i64));
}

#[test]
fn fs05_seed_not_a_long() {
    for value in ["abc", "+", "99999999999999999999"] {
        let env = Environment::from_pairs([("FREESURFER_SEED", value)]);
        let (mut p, o, e) = run(env.clone(), UpstreamDefects::Fix);
        assert_eq!(p.random_number(0.0, 1.0), Err(Stop::Exit(1)));
        let want = format!(
            "error: FREESURFER_SEED must be an integer in the range of a 64-bit long, not '{value}'\n"
        );
        assert_eq!(e.text(), want.as_bytes());
        let set = format!("Setting seed for random number genererator to {value}\n");
        assert_eq!(o.text(), set.as_bytes());
        assert!(noted(&p, "FS-05"));
        let (mut r, _, e2) = run(env, UpstreamDefects::Reproduce);
        assert_eq!(r.random_number(0.0, 1.0), Err(Stop::Exit(134)));
        let kind = if value.len() > 19 {
            "std::out_of_range"
        } else {
            "std::invalid_argument"
        };
        let want =
            format!("terminate called after throwing an instance of '{kind}'\n  what():  stol\n");
        assert_eq!(e2.text(), want.as_bytes());
    }
}

#[test]
fn fs06_outside_the_table() {
    let quiet = |b: u32| f32::from_bits(b | 0x0040_0000);
    // a NaN argument: atan2f's value, the NaN quieted; of two, x's (glibc's atan2f as a black
    // box: the probe's `single atan2f` with the slow-atan2f variable set)
    for (y, x, want) in [
        (f32::from_bits(0x7fa0_0001), 1.0, quiet(0x7fa0_0001)),
        (
            1.0,
            f32::from_bits(0xffc0_1234),
            f32::from_bits(0xffc0_1234),
        ),
        (
            f32::from_bits(0x7fc0_0005),
            f32::from_bits(0xffc0_0007),
            f32::from_bits(0xffc0_0007),
        ),
        (
            f32::from_bits(0xffa0_0003),
            f32::from_bits(0x7fa0_0009),
            quiet(0x7fa0_0009),
        ),
    ] {
        let (v, l) = atan::fast_atan2f(y, x);
        assert_eq!(l, Lookup::NanOrInfinities);
        assert_eq!(v.to_bits(), want.to_bits(), "atan2f({y}, {x})");
    }
    // two infinite ones: atan2f's value, ±π/4 and ±3π/4 in float
    let q = std::f64::consts::FRAC_PI_4;
    for (y, x, want) in [
        (1.0f32, 1.0f32, q),
        (1.0, -1.0, 3.0 * q),
        (-1.0, 1.0, -q),
        (-1.0, -1.0, -3.0 * q),
    ] {
        let (v, _) = atan::fast_atan2f(y * f32::INFINITY, x * f32::INFINITY);
        assert_eq!(v, want as f32);
    }
    // 100000·ay beyond the float range: computed on both magnitudes scaled by 2^-64
    let (v, l) = atan::fast_atan2f(f32::from_bits(0x7f00_0000), f32::MAX);
    assert_eq!(l, Lookup::Overflow);
    let ratio = f64::from(f32::from_bits(0x7f00_0000)) / f64::from(f32::MAX);
    let exact = (100_000.0 * ratio) as usize;
    assert_eq!(v, atan::table()[exact]);
    let (w, l) = atan::fast_atan2f(-f32::MAX, f32::from_bits(0x7e80_0000));
    assert_eq!(l, Lookup::Overflow);
    let k = (100_000.0 * (f64::from(f32::from_bits(0x7e80_0000)) / f64::from(f32::MAX))) as usize;
    assert_eq!(w, -(std::f32::consts::FRAC_PI_2 - atan::table()[k]));
    // one infinite argument with a finite other is no defect
    assert_eq!(atan::fast_atan2f(1.0, f32::INFINITY), (0.0, Lookup::Table));
    // the notice, in both modes
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let (mut p, _, _) = run(Environment::new(), mode);
        p.fast_approx_atan2f(f32::NAN, 1.0);
        assert!(noted(&p, "FS-06"));
    }
}

#[test]
fn fs11_long_strings() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let (mut p, _, _) = run(Environment::new(), mode);
        let mut a = vec![b'a'; 2000];
        let mut b = a.clone();
        b[1500] = b'c';
        assert_eq!(p.stricmp(&a, &b), i32::from(b'A') - i32::from(b'C'));
        a[1500] = b'C';
        assert_eq!(p.stricmp(&a, &b), 0);
        let mut name = vec![b'n'; 1500];
        name.extend_from_slice(b".tar.mgz");
        assert_eq!(p.file_name_extension(&name), b"mgz");
        assert!(noted(&p, "FS-11"));
        let (mut q, _, _) = run(Environment::new(), mode);
        assert_eq!(q.file_name_extension(b"short.mgz"), b"mgz");
        assert!(!noted(&q, "FS-11"));
    }
}

#[test]
fn fs263_fatal_with_exceptions_on() {
    let (mut p, _, e) = run(Environment::new(), UpstreamDefects::Fix);
    p.set_throw_exceptions(true);
    assert_eq!(p.fatal(7, b"f7"), Err(Stop::Thrown(7)));
    assert_eq!(e.text(), b"error: f7\n");
    assert!(noted(&p, "FS-263"));
    assert_eq!(p.fatal(0, b"zero"), Ok(()));
    let (mut r, _, e2) = run(Environment::new(), UpstreamDefects::Reproduce);
    r.set_throw_exceptions(true);
    assert_eq!(r.error_exit(-5, b"bad"), Err(Stop::Exit(134)));
    assert!(
        e2.text()
            .ends_with(b"terminate called after throwing an instance of 'int'\n")
    );
    assert!(noted(&r, "FS-263"));
}
