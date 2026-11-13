// SPDX-License-Identifier: Apache-2.0
//! Tests of the `qsort` clean room: the probe's cases (`specs/glibc-qsort.md` §10: its
//! generator, elements, comparator kinds and hashes) against the reference lines glibc 2.35 gave
//! (§9), the spec's example of §4, the fallback's error, the choice of §3, `qsort_r`'s context,
//! the typed functions against the byte ones, and golden checksums of the clean room's own
//! outputs (one per comparator kind and order, and the path through positions).

use super::*;
use crate::ranlux::sha256::Sha256;

/// splitmix64 (§10).
struct SplitMix(u64);

impl SplitMix {
    fn mix(mut z: u64) -> u64 {
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        SplitMix::mix(self.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Kind {
    ThreeWay,
    TiePlus,
    TieMinus,
    Hash,
    F3,
    F2,
    D3,
    D2,
}

impl Kind {
    fn parse(s: &str) -> Kind {
        match s {
            "3way" => Kind::ThreeWay,
            "tie+" => Kind::TiePlus,
            "tie-" => Kind::TieMinus,
            "hash" => Kind::Hash,
            "f3" => Kind::F3,
            "f2" => Kind::F2,
            "d3" => Kind::D3,
            "d2" => Kind::D2,
            _ => panic!("kind {s}"),
        }
    }
}

const FVALS: [u32; 12] = [
    0x7fc0_0000,
    0x8000_0000,
    0x0000_0000,
    0x3f80_0000,
    0xbf80_0000,
    0x7f80_0000,
    0xff80_0000,
    0x7fc0_0001,
    0xffc0_0000,
    0x4000_0000,
    0x0000_0001,
    0x3f80_0000,
];
const DVALS: [u64; 12] = [
    0x7ff8_0000_0000_0000,
    0x8000_0000_0000_0000,
    0,
    0x3ff0_0000_0000_0000,
    0xbff0_0000_0000_0000,
    0x7ff0_0000_0000_0000,
    0xfff0_0000_0000_0000,
    0x7ff8_0000_0000_0001,
    0xfff8_0000_0000_0000,
    0x4000_0000_0000_0000,
    1,
    0x3ff0_0000_0000_0000,
];

/// The probe's elements (§10).
fn elements(s: usize, n: usize, kind: Kind, seed: u64, keyrange: u64) -> Vec<u8> {
    let mut g = SplitMix(seed);
    let mut data = vec![0u8; n * s];
    for i in 0..n {
        let p = &mut data[i * s..(i + 1) * s];
        let k = g.next() % keyrange;
        match kind {
            Kind::F3 | Kind::F2 => p.copy_from_slice(&FVALS[(k % 12) as usize].to_le_bytes()),
            Kind::D3 | Kind::D2 => p.copy_from_slice(&DVALS[(k % 12) as usize].to_le_bytes()),
            _ if s == 1 => p[0] = k as u8,
            _ if s == 2 => p.copy_from_slice(&(k as u16).to_le_bytes()),
            _ => {
                p[..4].copy_from_slice(&(k as u32).to_le_bytes());
                if s >= 8 {
                    p[4..8].copy_from_slice(&(i as u32).to_le_bytes());
                    let mut off = 8;
                    while off < s {
                        let r = g.next().to_le_bytes();
                        let m = (s - off).min(8);
                        p[off..off + m].copy_from_slice(&r[..m]);
                        off += 8;
                    }
                }
            }
        }
    }
    data
}

fn key(e: &[u8]) -> u32 {
    match e.len() {
        1 => u32::from(e[0]),
        2 => u32::from(u16::from_le_bytes([e[0], e[1]])),
        _ => u32::from_le_bytes([e[0], e[1], e[2], e[3]]),
    }
}

/// The probe's comparator kinds (§10).
fn compare(kind: Kind, hseed: u64, a: &[u8], b: &[u8]) -> i32 {
    let three = |lt: bool, gt: bool| {
        if lt {
            -1
        } else if gt {
            1
        } else {
            0
        }
    };
    match kind {
        Kind::ThreeWay => three(key(a) < key(b), key(a) > key(b)),
        Kind::TiePlus => {
            if key(a) < key(b) {
                -1
            } else {
                1
            }
        }
        Kind::TieMinus => {
            if key(a) > key(b) {
                1
            } else {
                -1
            }
        }
        Kind::Hash => {
            let ta = u64::from(u32::from_le_bytes([a[4], a[5], a[6], a[7]]));
            let tb = u64::from(u32::from_le_bytes([b[4], b[5], b[6], b[7]]));
            let z = hseed
                ^ ta.wrapping_mul(0x9E37_79B9_7F4A_7C15)
                ^ tb.wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
            (SplitMix::mix(z) % 3) as i32 - 1
        }
        Kind::F3 | Kind::F2 => {
            let x = f32::from_le_bytes([a[0], a[1], a[2], a[3]]);
            let y = f32::from_le_bytes([b[0], b[1], b[2], b[3]]);
            if kind == Kind::F3 {
                three(x < y, x > y)
            } else {
                three(x < y, x != y)
            }
        }
        Kind::D3 | Kind::D2 => {
            let x = f64::from_le_bytes(a[..8].try_into().unwrap());
            let y = f64::from_le_bytes(b[..8].try_into().unwrap());
            if kind == Kind::D3 {
                three(x < y, x > y)
            } else {
                three(x < y, x != y)
            }
        }
    }
}

const FNV0: u64 = 14_695_981_039_346_656_037;
const FNVP: u64 = 1_099_511_628_211;

fn fnv(mut h: u64, bytes: &[u8]) -> u64 {
    for &b in bytes {
        h = (h ^ u64::from(b)).wrapping_mul(FNVP);
    }
    h
}

/// The probe's counted and hashed comparator.
struct Recorder {
    kind: Kind,
    hseed: u64,
    calls: u64,
    hash: u64,
}

impl Recorder {
    fn new(kind: Kind, seed: u64) -> Recorder {
        Recorder {
            kind,
            hseed: seed ^ 0x5_DEEC_E66D,
            calls: 0,
            hash: FNV0,
        }
    }
    fn call(&mut self, a: &[u8], b: &[u8]) -> i32 {
        let m = a.len().min(8);
        self.calls += 1;
        self.hash = fnv(fnv(self.hash, &a[..m]), &b[..m]);
        compare(self.kind, self.hseed, a, b)
    }
}

#[derive(Clone, Copy)]
enum Via {
    Merge,
    Quick,
    Glibc { fail: bool },
}

/// One probe line's output, `calls hash sorted requests errno` or `calls hash outside`.
fn run(s: usize, n: usize, kind: Kind, seed: u64, keyrange: u64, via: Via) -> String {
    let mut data = elements(s, n, kind, seed, keyrange);
    let mut rec = Recorder::new(kind, seed);
    let mut cmp = |a: &[u8], b: &[u8]| rec.call(a, b);
    let (result, requests) = match via {
        Via::Merge => {
            merge_order_bytes(&mut data, s, &mut cmp);
            (Ok(()), 0)
        }
        Via::Quick => (quick_order_bytes(&mut data, s, &mut cmp), 0),
        Via::Glibc { fail } => {
            let machine = Machine {
                allocation_succeeds: !fail,
                ..Machine::REFERENCE
            };
            match qsort_as_glibc_bytes(&mut data, s, &mut cmp, &machine) {
                Ok(choice) => (Ok(()), choice.requests()),
                Err(e) => (Err(e), 0),
            }
        }
    };
    match result {
        Ok(()) => format!(
            "{} {:016x} {:016x} {requests} 1",
            rec.calls,
            rec.hash,
            fnv(FNV0, &data)
        ),
        Err(_) => format!("{} {:016x} outside", rec.calls, rec.hash),
    }
}

fn run_line(line: &str) -> String {
    let w: Vec<&str> = line.split_whitespace().collect();
    run(
        w[0].parse().unwrap(),
        w[1].parse().unwrap(),
        Kind::parse(w[2]),
        w[3].parse().unwrap(),
        w[4].parse().unwrap(),
        Via::Glibc { fail: w[6] == "1" },
    )
}

/// The spec's reference lines (§9: glibc 2.35-0ubuntu3.15 through the probe).
#[test]
fn the_spec_reference_lines() {
    let lines = [
        (
            "4 1000 3way 1 10 qsort 0",
            "8504 22b3708447d84162 985da63e885ae5b7 1 1",
        ),
        (
            "4 1000 3way 1 10 qsort 1",
            "9540 1b768952e1c84670 985da63e885ae5b7 1 1",
        ),
        (
            "12 1000 3way 2 10 qsort 0",
            "8465 cd3c5a72a7733a84 1331cd34ebdbf954 1 1",
        ),
        (
            "12 1000 tie+ 2 10 qsort 0",
            "8494 4bcd1b66e61a1fa1 3c92d2e1fd018aac 1 1",
        ),
        (
            "16 1000 tie- 3 10 qsort 0",
            "8544 c4bcc9a6525d3b2e 31c543dbc89ca695 1 1",
        ),
        (
            "40 1000 hash 4 3 qsort 0",
            "6916 f672409666b84a2b 25ec899552189d77 1 1",
        ),
        (
            "40 1000 hash 4 3 qsort_r 0",
            "6916 f672409666b84a2b 25ec899552189d77 1 1",
        ),
        (
            "40 1000 hash 4 3 qsort 1",
            "9621 2ea5d1118facf2b7 2b1e1211fa2aad7f 1 1",
        ),
        (
            "4 1000 f3 5 12 qsort 0",
            "5997 a8107a0175eee3f5 563fcc1dcef7a859 1 1",
        ),
        (
            "4 1000 f2 5 12 qsort 0",
            "6127 5669dc3121f908a8 62a8788fd4a06369 1 1",
        ),
        (
            "8 1000 d3 6 12 qsort 0",
            "5879 8cc18a31990e2485 edf4496b7e528b61 1 1",
        ),
        (
            "8 1000 d2 6 12 qsort 1",
            "9262 c90bb93c4a7bba71 d7f7923ba5e9ce51 1 1",
        ),
        (
            "104 100 3way 7 5 qsort 0",
            "531 0461360aad912fdb 84af1b838617c27b 1 1",
        ),
        (
            "1 255 3way 8 4 qsort 0",
            "1612 5923cd99459aacc0 d1fe3ddfe7376da5 0 1",
        ),
        (
            "4 100000 3way 9 1000 qsort 0",
            "1536229 8bdb84d427f725aa cd915035a4583feb 1 1",
        ),
    ];
    for (input, want) in lines {
        assert_eq!(run_line(input), want, "{input}");
    }
}

/// §4's example: `M(0, 5)` = `M(0, 2)`, then `M(2, 3)` = `M(2, 1)` and `M(3, 2)`, then the merges.
#[test]
fn the_merge_example() {
    // a b c d e with keys 4 3 2 1 0: every merge takes the right part's element
    let mut v = [(4, 'a'), (3, 'b'), (2, 'c'), (1, 'd'), (0, 'e')];
    let mut calls = Vec::new();
    merge_order(&mut v, |x: &(i32, char), y: &(i32, char)| {
        calls.push((x.1, y.1));
        x.0.cmp(&y.0)
    });
    assert_eq!(
        calls,
        [
            ('a', 'b'),
            ('d', 'e'),
            ('c', 'e'),
            ('c', 'd'),
            ('b', 'e'),
            ('b', 'd'),
            ('b', 'c')
        ]
    );
    assert_eq!(v.map(|e| e.1), ['e', 'd', 'c', 'b', 'a']);
    // a tie takes the left part's element: stable
    let mut w = [(1, 'a'), (0, 'b'), (1, 'c'), (0, 'd'), (1, 'e')];
    merge_order(&mut w, |x: &(i32, char), y: &(i32, char)| x.0 - y.0);
    assert_eq!(w.map(|e| e.1), ['b', 'd', 'a', 'c', 'e']);
}

/// The fallback leaves the array with "ties → −1": the insertion of an element equal to the
/// first minimum scans past position 0 (§5); the error comes at that call.
#[test]
fn the_fallback_error() {
    let mut v = [7u32, 7, 7];
    let mut calls = 0;
    let e = quick_order(&mut v, |x: &u32, y: &u32| {
        calls += 1;
        if x > y { 1 } else { -1 }
    })
    .unwrap_err();
    // step 1: (1, 0) and (2, 1); step 2 for p = 2: (2, 1), (2, 0), then position −1
    assert_eq!(calls, 4);
    assert_eq!(
        e,
        QuickOrderError::Outside {
            position: -1,
            len: 3
        }
    );
    assert!(e.to_string().contains("position -1"));
    // a consistent comparator never leaves the array
    let mut w: Vec<u32> = (0..200).map(|i| (i * 7919) % 31).collect();
    quick_order(&mut w, |x: &u32, y: &u32| x.cmp(y)).unwrap();
    assert!(w.is_sorted());
}

/// §3: the stack limit, the size rule and a refused allocation.
#[test]
fn the_choice() {
    let r = Machine::REFERENCE;
    assert_eq!(temporary_bytes(255, 4), 1020);
    assert_eq!(temporary_bytes(10, 33), 193);
    assert_eq!(temporary_bytes(0, 1040), 1040);
    assert_eq!(choose(255, 4, &r), Choice::Stack);
    assert_eq!(choose(256, 4, &r), Choice::Allocated);
    assert_eq!(choose(31, 33, &r), Choice::Stack);
    assert_eq!(choose(62, 33, &r), Choice::Allocated);
    assert_eq!(choose(0, 1040, &r), Choice::Allocated);
    let refused = Machine {
        allocation_succeeds: false,
        ..r
    };
    assert_eq!(choose(256, 4, &refused), Choice::AllocationRefused);
    assert_eq!(choose(255, 4, &refused), Choice::Stack);
    // a quarter of 16 pages of 4096 bytes: 4 pages; 5 pages' worth is too large
    let small = Machine {
        page_size: 4096,
        phys_pages: Some(16),
        allocation_succeeds: true,
    };
    assert_eq!(choose(4 * 4096 / 8, 8, &small), Choice::Allocated);
    assert_eq!(choose(5 * 4096 / 8, 8, &small), Choice::TooLarge);
    assert_eq!(choose(5 * 4096 / 8 - 1, 8, &small), Choice::Allocated);
    let unknown = Machine {
        phys_pages: None,
        ..small
    };
    assert_eq!(choose(1 << 30, 8, &unknown), Choice::Allocated);
    assert_eq!(Choice::TooLarge.order(), Order::Quick);
    assert_eq!(Choice::TooLarge.requests(), 0);
    assert_eq!(Choice::AllocationRefused.requests(), 1);
}

/// `qsort_r` passes the context in every call and gives `qsort`'s result.
#[test]
fn qsort_r_context() {
    let data = elements(8, 300, Kind::D3, 6, 12);
    let to_f64 = |d: &[u8]| -> Vec<f64> {
        d.chunks(8)
            .map(|c| f64::from_le_bytes(c.try_into().unwrap()))
            .collect()
    };
    let three = |x: &f64, y: &f64| -> i32 {
        if x < y {
            -1
        } else if x > y {
            1
        } else {
            0
        }
    };
    let mut a = to_f64(&data);
    let mut b = a.clone();
    let mut seen = 0u64;
    qsort(&mut a, three);
    qsort_r(&mut b, &mut seen, |x, y, ctx: &mut u64| {
        *ctx += 1;
        three(x, y)
    });
    assert_eq!(
        a.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
        b.iter().map(|x| x.to_bits()).collect::<Vec<_>>()
    );
    assert!(seen > 0);
}

/// The typed functions make the byte functions' calls and leave their arrangement, the direct
/// path and the one through positions (more than 32 bytes).
#[test]
fn typed_equals_bytes() {
    fn check<const S: usize>(kind: Kind, n: usize, seed: u64, keyrange: u64) {
        let data = elements(S, n, kind, seed, keyrange);
        let typed: Vec<[u8; S]> = data
            .chunks(S)
            .map(|c| <[u8; S]>::try_from(c).unwrap())
            .collect();
        for quick in [false, true] {
            let mut bytes = data.clone();
            let mut rb = Recorder::new(kind, seed);
            let mut rt = Recorder::new(kind, seed);
            let mut t = typed.clone();
            if quick {
                let eb = quick_order_bytes(&mut bytes, S, |a: &[u8], b: &[u8]| rb.call(a, b));
                let et = quick_order(&mut t, |a: &[u8; S], b: &[u8; S]| rt.call(a, b));
                assert_eq!(eb, et);
            } else {
                merge_order_bytes(&mut bytes, S, |a: &[u8], b: &[u8]| rb.call(a, b));
                merge_order(&mut t, |a: &[u8; S], b: &[u8; S]| rt.call(a, b));
            }
            assert_eq!(
                (rb.calls, rb.hash),
                (rt.calls, rt.hash),
                "S={S} quick={quick}"
            );
            assert_eq!(bytes, t.concat(), "S={S} quick={quick}");
        }
    }
    check::<1>(Kind::ThreeWay, 300, 1, 4);
    check::<4>(Kind::F3, 300, 2, 12);
    check::<8>(Kind::Hash, 300, 3, 3);
    check::<12>(Kind::TiePlus, 300, 4, 5);
    check::<32>(Kind::TieMinus, 300, 5, 5);
    check::<33>(Kind::ThreeWay, 300, 6, 5);
    check::<104>(Kind::Hash, 300, 7, 3);
}

/// Every `n` from 0 to 64 with each order: no panic, and a consistent comparator sorts.
#[test]
fn small_arrays() {
    for n in 0..=64 {
        for kind in [Kind::ThreeWay, Kind::TiePlus, Kind::TieMinus, Kind::Hash] {
            let _ = run(8, n, kind, n as u64, 3, Via::Merge);
            let _ = run(8, n, kind, n as u64, 3, Via::Quick);
        }
        let mut v: Vec<u32> = (0..n as u32).map(|i| (i * 37) % 11).collect();
        let mut w = v.clone();
        merge_order(&mut v, |x: &u32, y: &u32| x.cmp(y));
        quick_order(&mut w, |x: &u32, y: &u32| x.cmp(y)).unwrap();
        assert!(v.is_sorted() && w.is_sorted());
    }
}

fn golden_cases() -> Vec<(String, String)> {
    let kinds = [
        ("3way", Kind::ThreeWay, 4, 5),
        ("tie+", Kind::TiePlus, 4, 5),
        ("tie-", Kind::TieMinus, 4, 5),
        ("hash", Kind::Hash, 8, 3),
        ("f3", Kind::F3, 4, 12),
        ("f2", Kind::F2, 4, 12),
        ("d3", Kind::D3, 8, 12),
        ("d2", Kind::D2, 8, 12),
        ("3way-40", Kind::ThreeWay, 40, 5),
    ];
    let mut cases = Vec::new();
    for (name, kind, s, keyrange) in kinds {
        for (order, via) in [("merge", Via::Merge), ("quick", Via::Quick)] {
            cases.push((
                format!("{name} {order}"),
                run(s, 37, kind, 2026, keyrange, via),
            ));
        }
    }
    cases
}

/// The golden checksums, recorded on Linux x86-64 (`cargo test -p larmorx-core glibc_qsort`
/// prints a changed or missing one). Equal pairs are expected: the merge order uses only whether
/// a result is positive, the quicksort order only whether it is negative, so `3way` and `tie-`
/// agree in the first, `3way` and `tie+` (and `f3`/`f2`, `d3`/`d2`) in the second.
const GOLDEN: &[(&str, &str)] = &[
    (
        "3way merge",
        "102b71ec3dbd3eeae7ca8c2d33f1d95b44496f8041a0edd60dded80f1889ab3e",
    ),
    (
        "3way quick",
        "d3951a472817102136855e57b4b58f6ff8e333a47ef741603e21a99b2343e56b",
    ),
    (
        "tie+ merge",
        "77b9ef558eb894eca85f327e67cc3dbb4a1ffd277308636df3ac1a32e328ddb1",
    ),
    (
        "tie+ quick",
        "d3951a472817102136855e57b4b58f6ff8e333a47ef741603e21a99b2343e56b",
    ),
    (
        "tie- merge",
        "102b71ec3dbd3eeae7ca8c2d33f1d95b44496f8041a0edd60dded80f1889ab3e",
    ),
    (
        "tie- quick",
        "6e9236015074c1bd6528bf2d67b715818b69d1cb88dc56303d6cbada951c9a63",
    ),
    (
        "hash merge",
        "92567a7faa085746eff8b11e8888ca21d4f4b3e9629a61c187806909aaa84496",
    ),
    (
        "hash quick",
        "4ba0dab6dafd17cf2add49aca67af2f3d6a3461cff8b939cc315d6ef90fef02d",
    ),
    (
        "f3 merge",
        "5e6234d12d3d5734ae5b5c6b6ca4287080be1714b311044452e13821d5f09282",
    ),
    (
        "f3 quick",
        "a06a44b015d52e25d0deef25c8e5380a4f80eeb5b926e2d1ddcd611bfb7fb570",
    ),
    (
        "f2 merge",
        "b65686ce083ccfc0f1010974560943380472f15bdac9a1f6f72e83e82592c24e",
    ),
    (
        "f2 quick",
        "a06a44b015d52e25d0deef25c8e5380a4f80eeb5b926e2d1ddcd611bfb7fb570",
    ),
    (
        "d3 merge",
        "c0770d399b82239d4a2dc280676760ec035153ed19221dd13d687b1109bbd705",
    ),
    (
        "d3 quick",
        "1c6354e9628a1888d661bc2968e6903f0a2e8f10674e2e9d742399967aef6fe0",
    ),
    (
        "d2 merge",
        "a2cdbd2506321ec75b69712d466c18ff65b0b2c831e2415745dff6d835ae8273",
    ),
    (
        "d2 quick",
        "1c6354e9628a1888d661bc2968e6903f0a2e8f10674e2e9d742399967aef6fe0",
    ),
    (
        "3way-40 merge",
        "046350b5a040d93ac52d1bc453ba1c968c29c19e558cee3bfecccbed56386e89",
    ),
    (
        "3way-40 quick",
        "49d8650811fc4b3086aacf3cbb2611dddd917bf47646b4edcc2190f56f0c60de",
    ),
];

#[test]
fn golden() {
    let mut bad = Vec::new();
    for (name, text) in golden_cases() {
        let mut h = Sha256::new();
        h.update(text.as_bytes());
        let got = h.hex();
        match GOLDEN.iter().find(|(n, _)| *n == name) {
            Some((_, want)) if *want == got => {}
            _ => bad.push(format!("    (\"{name}\", \"{got}\"),")),
        }
    }
    assert!(
        bad.is_empty() && GOLDEN.len() == golden_cases().len(),
        "golden cases:\n{}",
        bad.join("\n")
    );
}
