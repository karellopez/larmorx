// SPDX-License-Identifier: Apache-2.0
//! The JSON probe (README.md): generated documents parsed (strict and as `operator>>`), written
//! back (`dump` with indents -1, 0, 2, 4, a tab, ASCII-only or not; `operator<<` with a width),
//! accessed (keys, indices, conversions, sizes, range-for), and random doubles printed, by
//! nlohmann/json 3.2.0 (libup.so) and FreeSurfer 8.2.0's copy (libfs.so) and by
//! larmorx_core::json (reproduce); every record compared byte for byte.
//! Usage: fs-json <out-dir> [<scale>] (LXPROBE_ONLY_CASE=<n> shows one case).

use larmorx_core::defects::{Notices, UpstreamDefects};
use larmorx_core::json::{self, Dialect, Error, Nlohmann, Value};
use std::ffi::{CString, c_char, c_int, c_long, c_void};
use std::path::Path;

unsafe extern "C" {
    fn dlopen(filename: *const c_char, flag: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
}

type ParseFn = unsafe extern "C" fn(*const u8, c_long, c_int, *mut u8, c_long) -> c_long;
type DumpFn = unsafe extern "C" fn(*const u8, c_long, c_int, c_int, c_int, *mut u8, c_long) -> c_long;
type StreamFn = unsafe extern "C" fn(*const u8, c_long, c_int, c_int, *mut u8, c_long) -> c_long;
type FloatFn = unsafe extern "C" fn(u64, *mut u8, c_long) -> c_long;
type AccessFn =
    unsafe extern "C" fn(*const u8, c_long, *const c_long, c_int, *const u8, *mut u8, c_long) -> c_long;

struct Lib {
    name: &'static str,
    parse: ParseFn,
    dump: DumpFn,
    stream: StreamFn,
    float: FloatFn,
    access: AccessFn,
}

fn load(path: &Path, name: &'static str) -> Lib {
    let c = CString::new(path.to_string_lossy().as_bytes()).unwrap();
    // SAFETY: the dynamic loader with a NUL-terminated name (RTLD_NOW | RTLD_LOCAL).
    let h = unsafe { dlopen(c.as_ptr(), 2) };
    assert!(!h.is_null(), "dlopen({})", path.display());
    let sym = |s: &str| {
        let c = CString::new(s).unwrap();
        // SAFETY: a symbol lookup in the library opened above.
        let p = unsafe { dlsym(h, c.as_ptr()) };
        assert!(!p.is_null(), "{s}");
        p
    };
    // SAFETY: each symbol is the shim function with this signature.
    unsafe {
        Lib {
            name,
            parse: std::mem::transmute::<*mut c_void, ParseFn>(sym("fj_parse")),
            dump: std::mem::transmute::<*mut c_void, DumpFn>(sym("fj_dump")),
            stream: std::mem::transmute::<*mut c_void, StreamFn>(sym("fj_stream")),
            float: std::mem::transmute::<*mut c_void, FloatFn>(sym("fj_float")),
            access: std::mem::transmute::<*mut c_void, AccessFn>(sym("fj_access")),
        }
    }
}

fn call(f: impl FnOnce(*mut u8, c_long) -> c_long) -> Vec<u8> {
    let mut buf = vec![0u8; 1 << 22];
    let n = f(buf.as_mut_ptr(), buf.len() as c_long);
    buf.truncate((n.max(0) as usize).min(buf.len()));
    buf
}

// ---- the port's records, in the shim's format
fn put_bytes(o: &mut Vec<u8>, b: &[u8]) {
    o.extend_from_slice(&(b.len() as u32).to_le_bytes());
    o.extend_from_slice(b);
}
fn tree(o: &mut Vec<u8>, v: &Value) {
    match v {
        Value::Null => o.push(b'n'),
        Value::Boolean(b) => {
            o.push(b'b');
            o.push(*b as u8);
        }
        Value::Integer(i) => {
            o.push(b'i');
            o.extend_from_slice(&(*i as u64).to_le_bytes());
        }
        Value::Unsigned(u) => {
            o.push(b'u');
            o.extend_from_slice(&u.to_le_bytes());
        }
        Value::Float(f) => {
            o.push(b'f');
            o.extend_from_slice(&f.to_bits().to_le_bytes());
        }
        Value::String(s) => {
            o.push(b's');
            put_bytes(o, s);
        }
        Value::Array(a) => {
            o.push(b'a');
            o.extend_from_slice(&(a.len() as u32).to_le_bytes());
            for e in a {
                tree(o, e);
            }
        }
        Value::Object(m) => {
            o.push(b'o');
            o.extend_from_slice(&(m.len() as u32).to_le_bytes());
            for (k, e) in m {
                put_bytes(o, k);
                tree(o, e);
            }
        }
    }
}
fn err(o: &mut Vec<u8>, e: &Error) {
    o.push(b'E');
    o.extend_from_slice(&e.id.to_le_bytes());
    put_bytes(o, &e.message);
}

struct FsDialect;
impl Dialect for FsDialect {
    fn operator_index_message(&self, numeric: bool, type_name: &str) -> String {
        let a = if numeric { "a numeric" } else { "a string" };
        format!("cannot use operator[] with {a} argument with {type_name}")
    }
}

fn port_parse(t: &[u8], strict: bool) -> Vec<u8> {
    let mut o = Vec::new();
    match json::parse(t, strict) {
        Ok((v, used)) => {
            o.push(b'V');
            tree(&mut o, &v);
            if !strict {
                o.push(b'C');
                o.extend_from_slice(&(used as u64).to_le_bytes());
            }
        }
        Err(e) => err(&mut o, &e),
    }
    o
}

fn port_dump(t: &[u8], indent: i32, ichar: u8, ascii: bool) -> (Vec<u8>, Vec<&'static str>) {
    let mut o = Vec::new();
    let mut n = Notices::new(UpstreamDefects::Reproduce);
    match json::parse(t, true) {
        Ok((v, _)) => match v.dump(indent, ichar, ascii, &mut n) {
            Ok(s) => {
                o.push(b'D');
                put_bytes(&mut o, &s);
            }
            Err(e) => err(&mut o, &e),
        },
        Err(e) => err(&mut o, &e),
    }
    (o, n.as_slice().iter().map(|x| x.defect.id).collect())
}

fn port_stream(t: &[u8], width: i64, fill: u8) -> Vec<u8> {
    let mut o = Vec::new();
    let mut n = Notices::new(UpstreamDefects::Reproduce);
    match json::parse(t, true) {
        Ok((v, _)) => {
            let (bytes, r) = v.write_stream(width, fill, &mut n);
            match r {
                Ok(()) => {
                    o.push(b'W');
                    put_bytes(&mut o, &bytes);
                }
                Err(e) => {
                    o.push(b'P');
                    put_bytes(&mut o, &bytes);
                    err(&mut o, &e);
                }
            }
        }
        Err(e) => err(&mut o, &e),
    }
    o
}

fn port_access(t: &[u8], ops: &[i64], keys: &[Vec<u8>], d: &dyn Dialect) -> Vec<u8> {
    let mut o = Vec::new();
    let mut root = match json::parse(t, true) {
        Ok((v, _)) => v,
        Err(e) => {
            err(&mut o, &e);
            return o;
        }
    };
    let mut path: Vec<(i64, i64, usize)> = Vec::new();
    let mut ki = 0;
    let r = (|| -> Result<(), Error> {
        for p in ops.chunks_exact(2) {
            let (op, arg) = (p[0], p[1]);
            // walk the path from the root each time (the C++ keeps a pointer)
            let mut cur: &mut Value = &mut root;
            for &(c, a, k) in &path {
                cur = if c == 1 { cur.index_key(&keys[k], d)? } else { cur.index(a as usize, d)? };
            }
            match op {
                1 => {
                    cur.index_key(&keys[ki], d)?;
                    path.push((1, 0, ki));
                    ki += 1;
                    o.push(b'K');
                }
                2 => {
                    cur.index(arg as usize, d)?;
                    path.push((2, arg, 0));
                    o.push(b'I');
                }
                3 => {
                    let v = cur.get_f64()?;
                    o.push(b'd');
                    o.extend_from_slice(&v.to_bits().to_le_bytes());
                }
                4 => {
                    let v = cur.get_f32()?;
                    o.push(b'f');
                    o.extend_from_slice(&v.to_bits().to_le_bytes());
                }
                // the shim writes these tags before the conversion (which may throw)
                5 => {
                    o.push(b'i');
                    let v = cur.get_i32()?;
                    o.extend_from_slice(&(v as u32).to_le_bytes());
                }
                6 => {
                    o.push(b'l');
                    let v = cur.get_i64()?;
                    o.extend_from_slice(&(v as u64).to_le_bytes());
                }
                7 => {
                    o.push(b's');
                    let v = cur.get_string()?.to_vec();
                    put_bytes(&mut o, &v);
                }
                8 => {
                    o.push(b'z');
                    o.extend_from_slice(&(cur.size() as u64).to_le_bytes());
                }
                9 => {
                    let items: Vec<Value> = cur.iter().cloned().collect();
                    o.push(b'r');
                    o.extend_from_slice(&(items.len() as u32).to_le_bytes());
                    for e in &items {
                        tree(&mut o, e);
                    }
                }
                _ => {}
            }
        }
        Ok(())
    })();
    match r {
        Ok(()) => {
            o.push(b'T');
            tree(&mut o, &root);
        }
        Err(e) => err(&mut o, &e),
    }
    o
}

// ---- generated documents
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: u64) -> u64 {
        if n == 0 { 0 } else { self.next() % n }
    }
    fn chance(&mut self, p: f64) -> bool {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64 <= p
    }
}

fn ws(r: &mut Rng, o: &mut Vec<u8>) {
    if r.chance(0.3) {
        for _ in 0..r.below(3) {
            o.push(*[b' ', b'\t', b'\n', b'\r'].get(r.below(4) as usize).unwrap());
        }
    }
}

fn number(r: &mut Rng, o: &mut Vec<u8>) {
    let s = match r.below(12) {
        0 => format!("{}", r.next() as i64),
        1 => format!("{}", r.next()),
        2 => format!("-{}", r.next()),
        3 => format!("{}{}", r.next(), r.below(1000)),
        4 => format!("{}.{}", r.below(1000), r.below(100000)),
        5 => format!("{}e{}", r.below(100), r.below(700) as i64 - 350),
        6 => format!("-{}.{}E+{}", r.below(10), r.below(1000), r.below(330)),
        7 => format!("{:e}", f64::from_bits(r.next())),
        8 => format!("{}", f64::from_bits(r.next() & 0x7fef_ffff_ffff_ffff)),
        9 => ["0", "-0", "0.0", "-0.0", "1e400", "-1e400", "1e-400", "18446744073709551615", "18446744073709551616", "-9223372036854775808", "-9223372036854775809", "4.9406564584124654e-324"][r.below(12) as usize].to_string(),
        10 => ["01", "-", "1.", ".5", "1e", "1e+", "--1", "+1", "1.5e3.2", "0x10"][r.below(10) as usize].to_string(),
        _ => format!("{}", r.below(100)),
    };
    o.extend_from_slice(s.as_bytes());
}

fn string(r: &mut Rng, o: &mut Vec<u8>) {
    o.push(b'"');
    for _ in 0..r.below(12) {
        match r.below(20) {
            0 => o.extend_from_slice(b"\\n"),
            1 => o.extend_from_slice(b"\\\""),
            2 => o.extend_from_slice(format!("\\u{:04x}", r.below(0x10000)).as_bytes()),
            3 => o.extend_from_slice(format!("\\ud83d\\u{:04x}", 0xdc00 + r.below(0x400)).as_bytes()),
            4 => o.extend_from_slice("é€😀".as_bytes()),
            5 => o.push(0x80 + r.below(0x80) as u8),
            6 => o.extend_from_slice(&[0xe0, 0x80 + r.below(0x40) as u8, 0x80]),
            7 => o.push(r.below(0x20) as u8),
            8 => o.extend_from_slice(["\\x", "\\u12", "\\ud800x", "\\udc00", "\\/", "\\b\\f\\r\\t\\\\"][r.below(6) as usize].as_bytes()),
            9 => o.extend_from_slice(&[0xf0, 0x9f, 0x98]),
            10 => o.push(0x7f),
            _ => o.push(b'a' + r.below(26) as u8),
        }
    }
    if !r.chance(0.02) {
        o.push(b'"');
    }
}

fn value(r: &mut Rng, o: &mut Vec<u8>, depth: u32) {
    ws(r, o);
    match r.below(if depth > 5 { 6 } else { 9 }) {
        0 => o.extend_from_slice(b"null"),
        1 => o.extend_from_slice(if r.chance(0.5) { b"true" } else { b"false" }),
        2 | 3 => number(r, o),
        4 | 5 => string(r, o),
        6 | 7 => {
            o.push(b'{');
            let n = r.below(5);
            for i in 0..n {
                if i > 0 {
                    o.push(b',');
                }
                ws(r, o);
                string(r, o);
                ws(r, o);
                o.push(b':');
                value(r, o, depth + 1);
            }
            ws(r, o);
            o.push(b'}');
        }
        _ => {
            o.push(b'[');
            let n = r.below(5);
            for i in 0..n {
                if i > 0 {
                    o.push(b',');
                }
                value(r, o, depth + 1);
            }
            ws(r, o);
            o.push(b']');
        }
    }
    ws(r, o);
}

fn mutate(r: &mut Rng, d: &mut Vec<u8>) {
    if d.is_empty() {
        return;
    }
    match r.below(6) {
        0 => {
            let n = r.below(d.len() as u64) as usize;
            d.truncate(n);
        }
        1 => {
            let i = r.below(d.len() as u64) as usize;
            d[i] = *b"{}[],:\"\\0aen- \x00\xff".get(r.below(16) as usize).unwrap();
        }
        2 => d.extend_from_slice([&b" x"[..], b"]", b"\x00garbage", b"{}", b"  \n"][r.below(5) as usize]),
        3 => {
            let mut e = vec![0xef, 0xbb, 0xbf];
            if r.chance(0.3) {
                e = vec![0xef, 0xbb];
            }
            e.extend_from_slice(d);
            *d = e;
        }
        4 => {
            let i = r.below(d.len() as u64) as usize;
            d.remove(i);
        }
        _ => {
            let i = r.below(d.len() as u64) as usize;
            d.insert(i, *b",:{[ \"".get(r.below(6) as usize).unwrap());
        }
    }
}

fn deep(n: usize, open: &[u8], close: &[u8]) -> Vec<u8> {
    let mut o = open.repeat(n);
    o.extend_from_slice(b"1");
    o.extend_from_slice(&close.repeat(n));
    o
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out = std::path::PathBuf::from(args.get(1).expect("usage: fs-json <out-dir> [<scale>]"));
    let scale: usize = args.get(2).map(|s| s.parse().unwrap()).unwrap_or(1);
    let only: Option<usize> = std::env::var("LXPROBE_ONLY_CASE").ok().map(|s| s.parse().unwrap());
    let libs = [load(&out.join("libup.so"), "nlohmann-3.2.0"), load(&out.join("libfs.so"), "freesurfer")];
    let mut docs: Vec<Vec<u8>> = vec![
        b"".to_vec(),
        b" ".to_vec(),
        b"{\"a\":1,\"a\":2}".to_vec(),
        b"[1, tru]".to_vec(),
        b"1 2".to_vec(),
        b"{\"b\":{},\"a\":[],\"c\":\"\"}".to_vec(),
        b"[1.0, 1e15, 1e16, 0.0001, 0.00001, 123456789012345678, -0.0, 5e-324, 1.7976931348623157e308]".to_vec(),
        deep(130, b"[", b"]"),
        deep(130, b"{\"k\":", b"}"),
        deep(300, b"[", b"]"),
    ];
    let mut rng = Rng(20261113);
    for i in 0..3000 * scale {
        let mut d = Vec::new();
        value(&mut rng, &mut d, 0);
        if i % 3 == 0 {
            for _ in 0..1 + rng.below(2) {
                mutate(&mut rng, &mut d);
            }
        }
        docs.push(d);
    }
    let mut counts = std::collections::BTreeMap::<String, usize>::new();
    let mut failures = 0usize;
    let mut shown = 0;
    let mut check = |what: &str, lib: &str, c: &[u8], p: &[u8], explained: bool, n: usize| {
        let key = if c == p {
            format!("{what} identical")
        } else if explained {
            format!("{what} explained FS-99")
        } else {
            failures += 1;
            if shown < 15 || only.is_some() {
                shown += 1;
                println!("== case {n} {what} vs {lib}\n   C   : {:?}\n   port: {:?}", String::from_utf8_lossy(&c[..c.len().min(400)]), String::from_utf8_lossy(&p[..p.len().min(400)]));
            }
            format!("{what} DIFFERENT")
        };
        *counts.entry(format!("{lib}: {key}")).or_default() += 1;
    };
    for (n, d) in docs.iter().enumerate() {
        if only.is_some_and(|o| o != n) {
            continue;
        }
        for lib in &libs {
            for strict in [1, 0] {
                // SAFETY: the shim reads `d` and writes at most `cap` bytes.
                let c = call(|b, cap| unsafe { (lib.parse)(d.as_ptr(), d.len() as c_long, strict, b, cap) });
                check(if strict == 1 { "parse" } else { "stream-in" }, lib.name, &c, &port_parse(d, strict == 1), false, n);
            }
            for (indent, ichar, ascii) in [(-1, b' ', 0), (0, b' ', 0), (2, b' ', 1), (4, b' ', 0), (4, b'\t', 1)] {
                // SAFETY: as above.
                let c = call(|b, cap| unsafe { (lib.dump)(d.as_ptr(), d.len() as c_long, indent, ichar as c_int, ascii, b, cap) });
                let (p, notices) = port_dump(d, indent, ichar, ascii != 0);
                check("dump", lib.name, &c, &p, notices.contains(&"FS-99"), n);
            }
            for (w, f) in [(0i64, b' '), (4, b' '), (3, b'.')] {
                // SAFETY: as above.
                let c = call(|b, cap| unsafe { (lib.stream)(d.as_ptr(), d.len() as c_long, w as c_int, f as c_int, b, cap) });
                check("stream-out", lib.name, &c, &port_stream(d, w, f), false, n);
            }
            if n % 4 == 0 {
                let keys: Vec<Vec<u8>> = vec![b"a".to_vec(), b"points".to_vec(), b"x".to_vec(), b"".to_vec()];
                let ops: Vec<i64> = match n % 16 {
                    0 => vec![9, 0, 8, 0, 3, 0],
                    4 => vec![1, 0, 3, 0, 4, 0, 5, 0, 6, 0],
                    8 => vec![2, 3, 8, 0, 7, 0],
                    _ => vec![1, 0, 1, 0, 9, 0, 2, 1, 6, 0],
                };
                let mut kb = Vec::new();
                for k in &keys {
                    kb.extend_from_slice(k);
                    kb.push(0);
                }
                let fsd = FsDialect;
                let dialect: &dyn Dialect = if lib.name == "freesurfer" { &fsd } else { &Nlohmann };
                // SAFETY: as above.
                let c = call(|b, cap| unsafe { (lib.access)(d.as_ptr(), d.len() as c_long, ops.as_ptr() as *const c_long, (ops.len() / 2) as c_int, kb.as_ptr(), b, cap) });
                check("access", lib.name, &c, &port_access(d, &ops, &keys, dialect), false, n);
            }
        }
    }
    // doubles
    let mut r = Rng(7);
    for i in 0..200_000 * scale {
        let bits = match i % 4 {
            0 => r.next(),
            1 => (r.next() & 0x000f_ffff_ffff_ffff) | ((r.below(2046) + 1) << 52),
            2 => (r.below(100_000) as f64 * 10f64.powi(r.below(40) as i32 - 20)).to_bits(),
            _ => r.below(1 << 20),
        };
        if f64::from_bits(bits).is_nan() || f64::from_bits(bits).is_infinite() {
            continue;
        }
        let mut p = vec![b'D'];
        put_bytes(&mut p, json::to_chars(f64::from_bits(bits)).as_bytes());
        for lib in &libs {
            // SAFETY: as above.
            let c = call(|b, cap| unsafe { (lib.float)(bits, b, cap) });
            check("double", lib.name, &c, &p, false, i);
        }
    }
    for (k, v) in &counts {
        println!("{k}: {v}");
    }
    println!("{}", if failures == 0 { "VERIFY-PASS" } else { "VERIFY-FAIL" });
    std::process::exit(if failures == 0 { 0 } else { 1 });
}
