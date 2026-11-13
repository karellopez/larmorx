// SPDX-License-Identifier: Apache-2.0
//! The 3.1-18 probe (README.md): deflate streams (valid, cut, bit-flipped, random, and the
//! undefined symbols, patterns and distances of FS-281..284) inflated four ways by miniz 1.15
//! r4 (libup.so), FreeSurfer 8.2.0's copy (libfs.so) and larmorx_core::miniz (reproduce and
//! fix); FNV-1a's 64-bit accumulator against FreeSurfer's FNV headers; libtiff's handlers'
//! messages; NrrdIO's and Cephes' constants.
//! Usage: fs-misc <out-dir> [<scale>] (LXPROBE_ONLY_CASE=<n> shows one case).

use larmorx_core::cfmt::{Captured, Stream};
use larmorx_core::defects::UpstreamDefects::{self, Fix, Reproduce};
use larmorx_core::fnv;
use larmorx_core::libtiff::TiffMessages;
use larmorx_core::miniz::streams::{self, Bits, Rng, Tok};
use larmorx_core::miniz::{self, InflateStream};
use larmorx_core::nrrdio;
use std::ffi::{CString, c_char, c_int, c_long, c_void};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

unsafe extern "C" {
    fn dlopen(filename: *const c_char, flag: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
}

type InflateFn = unsafe extern "C" fn(
    *const u8,
    c_long,
    c_int,
    c_int,
    c_long,
    c_long,
    *mut u8,
    *mut c_long,
    *mut u64,
    *mut u64,
) -> c_int;
type UncompressFn = unsafe extern "C" fn(*const u8, c_long, *mut u8, c_long, *mut c_long) -> c_int;

struct Lib {
    inflate: InflateFn,
    uncompress: UncompressFn,
}

fn load(path: &Path) -> Lib {
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
    // SAFETY: the shim's functions have these signatures (shim.c).
    unsafe {
        Lib {
            inflate: std::mem::transmute::<*mut c_void, InflateFn>(sym("probe_inflate")),
            uncompress: std::mem::transmute::<*mut c_void, UncompressFn>(sym("probe_uncompress")),
        }
    }
}

/// One run's observables: code, output, total_in, adler, notices.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Obs {
    ret: i32,
    out: Vec<u8>,
    total_in: u64,
    adler: u64,
}

fn c_run(lib: &Lib, input: &[u8], wb: i32, how: i32, room: usize, piece: usize) -> Obs {
    let mut out = vec![0u8; room + 16];
    let (mut n, mut ti, mut ad) = (0 as c_long, 0u64, 0u64);
    // SAFETY: buffers of the lengths passed (16 bytes of slack past `room`).
    let ret = unsafe {
        (lib.inflate)(
            input.as_ptr(),
            input.len() as c_long,
            wb,
            how,
            room as c_long,
            piece as c_long,
            out.as_mut_ptr(),
            &mut n,
            &mut ti,
            &mut ad,
        )
    };
    out.truncate(n as usize);
    Obs { ret, out, total_in: ti, adler: ad }
}

fn rust_run(
    input: &[u8],
    wb: i32,
    how: i32,
    room: usize,
    piece: usize,
    mode: UpstreamDefects,
) -> (Obs, Vec<&'static str>) {
    let mut s = InflateStream::new(mode);
    assert_eq!(s.init2(wb), miniz::MZ_OK);
    let mut out = vec![0u8; room];
    let (mut op, mut ip) = (0usize, 0usize);
    let mut ret;
    match how {
        0 => {
            let r = s.inflate(input, &mut out, miniz::MZ_FINISH);
            op = r.produced;
            ret = r.ret;
        }
        1 => loop {
            let end = (op + piece).min(room);
            let r = s.inflate(&input[ip..], &mut out[op..end], miniz::MZ_SYNC_FLUSH);
            ip += r.consumed;
            op += r.produced;
            ret = r.ret;
            if ret != miniz::MZ_OK || (r.consumed == 0 && r.produced == 0) {
                break;
            }
        },
        _ => loop {
            let (inp, flush) = if ip < input.len() {
                (&input[ip..ip + 1], miniz::MZ_NO_FLUSH)
            } else {
                (&input[ip..], miniz::MZ_FINISH)
            };
            let r = s.inflate(inp, &mut out[op..], flush);
            ip += r.consumed;
            op += r.produced;
            ret = r.ret;
            if ret != miniz::MZ_OK || flush == miniz::MZ_FINISH {
                break;
            }
        },
    }
    out.truncate(op);
    let ids = s.notices.as_slice().iter().map(|n| n.defect.id).collect();
    (Obs { ret, out, total_in: s.total_in, adler: s.adler }, ids)
}

/// A random stream of 1 to 4 blocks (stored, fixed, dynamic), sometimes with one undefined
/// symbol, an uncovered pattern or a distance too far back: (raw deflate, the bytes it holds
/// where it is valid).
fn random_stream(rng: &mut Rng, max_len: usize, odd: bool) -> (Vec<u8>, Vec<u8>) {
    let mut b = Bits::default();
    let mut data = Vec::new();
    let blocks = 1 + rng.below(4);
    for k in 0..blocks {
        let last = k + 1 == blocks;
        let n = rng.below(max_len as u64 + 1) as usize;
        let alphabet = [2, 4, 16, 256][rng.below(4) as usize];
        let kind = rng.below(3);
        if kind == 0 {
            let bytes: Vec<u8> = (0..n.min(65535)).map(|_| rng.below(256) as u8).collect();
            streams::stored(&mut b, &bytes, last);
            data.extend_from_slice(&bytes);
            continue;
        }
        let (mut toks, out) = streams::random_tokens(rng, &data, n, alphabet, 32768);
        if odd && rng.below(2) == 0 {
            let at = rng.below(toks.len() as u64 + 1) as usize;
            let t = match rng.below(5) {
                0 => Tok::Raw(286 + rng.below(2) as u32, 0, Some((rng.below(4) as u32, 0))),
                1 => Tok::Raw(257 + rng.below(20) as u32, 0, Some((30 + rng.below(2) as u32, 0))),
                2 => Tok::Match(3 + rng.below(20) as u32, 1 + rng.below(40_000) as u32),
                3 => Tok::Raw(257, 0, None),
                _ => Tok::Bits(rng.below(256) as u32, 1 + rng.below(8) as u32),
            };
            toks.insert(at, t);
        }
        if kind == 1 {
            streams::fixed(&mut b, &toks, last);
        } else {
            let (ll, mut dl) = streams::lengths_for(rng, &toks);
            if odd && rng.below(3) == 0 {
                dl = vec![1 + rng.below(15) as u32];
            }
            let rle = rng.below(2) == 1;
            streams::dynamic(&mut b, rng, &toks, &ll, &dl, rle, last);
        }
        data.extend_from_slice(&out);
    }
    (b.finish(), data)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let dir = Path::new(&args[1]);
    let scale: u64 = args.get(2).map_or(1, |s| s.parse().unwrap());
    let only: Option<u64> = std::env::var("LXPROBE_ONLY_CASE").ok().map(|s| s.parse().unwrap());
    let up = load(&dir.join("libup.so"));
    let fsl = load(&dir.join("libfs.so"));
    let mut fails = 0u64;
    let (mut runs, mut same_up_fs, mut repro_same, mut fix_same) = (0u64, 0u64, 0u64, 0u64);
    let mut explained: std::collections::BTreeMap<String, u64> = Default::default();

    // inflate
    let mut rng = Rng(0x3118);
    let cases = 6000 * scale;
    for case in 0..cases {
        let odd = case % 3 == 2;
        let (raw, data) = random_stream(&mut rng, if case % 50 == 0 { 70_000 } else { 700 }, odd);
        let zlib = case % 2 == 1;
        let mut input = if zlib { streams::zlib(&raw, &data) } else { raw };
        match case % 7 {
            1 => input.truncate(rng.below(input.len() as u64 + 1) as usize),
            2 => {
                for _ in 0..1 + rng.below(4) {
                    let i = rng.below(input.len() as u64) as usize;
                    input[i] ^= 1 << rng.below(8);
                }
            }
            3 => input = (0..rng.below(400)).map(|_| rng.below(256) as u8).collect(),
            4 => input.extend((0..1 + rng.below(9)).map(|_| rng.below(256) as u8)),
            _ => {}
        }
        if only.is_some_and(|o| o != case) {
            continue;
        }
        let wb = if zlib { 15 } else { -15 };
        for (how, piece) in [(0, 0), (1, 1), (1, 1 + rng.below(5000) as usize), (2, 0)] {
            let room = (data.len() + 2).saturating_sub(rng.below(5) as usize);
            runs += 1;
            let a = c_run(&up, &input, wb, how, room, piece);
            let f = c_run(&fsl, &input, wb, how, room, piece);
            if a == f {
                same_up_fs += 1;
            } else {
                fails += 1;
                eprintln!("case {case} how {how}/{piece}: miniz 1.15 and FreeSurfer's copy differ");
            }
            let (r, ids) = rust_run(&input, wb, how, room, piece, Reproduce);
            if r == a {
                repro_same += 1;
            } else if ids.contains(&"FS-282") && r.ret == miniz::MZ_DATA_ERROR {
                *explained.entry("reproduce: FS-282 past the buffer".into()).or_default() += 1;
            } else {
                fails += 1;
                eprintln!(
                    "case {case} how {how}/{piece} room {room}: reproduce differs {:?} vs C ret {} out {} ti {} ad {:#x}; rust ret {} out {} ti {} ad {:#x}",
                    ids, a.ret, a.out.len(), a.total_in, a.adler, r.ret, r.out.len(), r.total_in, r.adler
                );
            }
            let (x, ids) = rust_run(&input, wb, how, room, piece, Fix);
            if ids.is_empty() {
                if x == a {
                    fix_same += 1;
                } else {
                    fails += 1;
                    eprintln!("case {case} how {how}/{piece}: fix without a notice differs");
                }
            } else {
                *explained.entry(format!("fix: {}", ids.join("+"))).or_default() += 1;
            }
        }
    }
    // mz_uncompress
    let mut unc = (0u64, 0u64);
    for case in 0..1000 * scale {
        let (raw, data) = random_stream(&mut rng, 500, case % 4 == 3);
        let mut z = streams::zlib(&raw, &data);
        if case % 3 == 1 {
            z.truncate(rng.below(z.len() as u64) as usize);
        }
        let room = data.len() + 1 - rng.below(3).min(data.len() as u64 + 1) as usize;
        let mut co = vec![0u8; room + 16];
        let mut n: c_long = 0;
        // SAFETY: buffers of the lengths passed.
        let cr = unsafe { (up.uncompress)(z.as_ptr(), z.len() as c_long, co.as_mut_ptr(), room as c_long, &mut n) };
        let mut ro = vec![0u8; room];
        let mut len = room as u64;
        let (rr, notes) = miniz::uncompress(&mut ro, &mut len, &z, Reproduce);
        unc.0 += 1;
        let same = cr == rr && (cr != miniz::MZ_OK || (len == n as u64 && ro[..len as usize] == co[..len as usize]));
        if same {
            unc.1 += 1;
        } else if notes.contains(&larmorx_core::defects::FS_282) {
            *explained.entry("uncompress: FS-282 past the buffer".into()).or_default() += 1;
        } else {
            fails += 1;
            eprintln!("uncompress case {case}: C {cr} n {n}, rust {rr} len {len}");
        }
    }

    // FNV through FreeSurfer's headers
    let facts = dir.join("facts");
    let mut recs = Vec::new();
    let mut want = Vec::new();
    for k in 0..20_000 * scale {
        let n = if k < 300 { k as usize } else { rng.below(3000) as usize };
        let bytes: Vec<u8> = (0..n).map(|_| rng.below(256) as u8).collect();
        recs.extend_from_slice(&(n as u32).to_le_bytes());
        recs.extend_from_slice(&bytes);
        let w = fnv::fnv1a_32_wide(fnv::FNV32_OFFSET as u64, &bytes);
        want.push(format!("{w} {w}"));
    }
    let got = run_facts(&facts, "fnv", &recs).0;
    let got: Vec<&str> = got.lines().collect();
    let fnv_same = got.iter().zip(&want).filter(|(g, w)| *g == w).count();
    if got.len() != want.len() || fnv_same != want.len() {
        fails += 1;
        eprintln!("fnv: {} of {} records identical", fnv_same, want.len());
    }
    // libtiff's handlers
    let mut lines = String::new();
    let mut expect = Captured::default();
    let t = TiffMessages::default();
    for k in 0..2000 * scale {
        let module = match k % 3 {
            0 => None,
            _ => Some(format!("Mod{}", rng.below(1000))),
        };
        let msg: String = (0..rng.below(80)).map(|_| (b' ' + rng.below(95) as u8) as char).collect();
        lines.push_str(&format!("{}\t{}\n", module.as_deref().unwrap_or("-"), msg));
        t.error(module.as_deref(), &msg, &mut expect);
        t.warning(module.as_deref(), &msg, &mut expect);
    }
    let (_, tiff_err) = run_facts(&facts, "tiff", lines.as_bytes());
    let tiff_same = tiff_err == expect.text(Stream::Err);
    if !tiff_same {
        fails += 1;
        eprintln!("tiff: the messages differ");
    }
    // constants
    let (consts, _) = run_facts(&facts, "consts", b"");
    let want_consts = format!(
        "airFloatQNaN {:08x}\nairFloatSNaN {:08x}\nairFloatPosInf {:08x}\nairFloatNegInf {:08x}\ncephes PI {:016x}\n",
        nrrdio::AIR_FLOAT_QNAN,
        nrrdio::AIR_FLOAT_SNAN,
        nrrdio::AIR_FLOAT_POS_INF,
        nrrdio::AIR_FLOAT_NEG_INF,
        std::f64::consts::PI.to_bits()
    );
    let consts_same = consts == want_consts;
    if !consts_same {
        fails += 1;
        eprintln!("constants differ:\n{consts}");
    }

    println!("inflate: {runs} runs; miniz 1.15 = FreeSurfer's copy in {same_up_fs}; reproduce = miniz in {repro_same}; fix without a notice = miniz in {fix_same}");
    println!("uncompress: {} runs, reproduce = miniz in {}", unc.0, unc.1);
    for (k, v) in &explained {
        println!("  explained {k}: {v}");
    }
    println!("fnv: {} records, identical {fnv_same}", want.len());
    println!("tiff: {} messages, identical {tiff_same}", 2 * 2000 * scale);
    println!("constants identical {consts_same}");
    println!("{}", if fails == 0 { "VERIFY-PASS" } else { "VERIFY-FAIL" });
    std::process::exit(if fails == 0 { 0 } else { 1 });
}

fn run_facts(exe: &Path, mode: &str, stdin: &[u8]) -> (String, String) {
    let mut child = Command::new(exe)
        .arg(mode)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("facts");
    let mut input = child.stdin.take().unwrap();
    let data = stdin.to_vec();
    let w = std::thread::spawn(move || input.write_all(&data));
    let o = child.wait_with_output().unwrap();
    w.join().unwrap().unwrap();
    (String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}
