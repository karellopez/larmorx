// SPDX-License-Identifier: Apache-2.0
//! The families: `scan`, `xml`, `read` (with the writer).

use crate::Tally;
use crate::dialect::ProbeFs;
use crate::dump::{self, D, Events};
use crate::ffi::{self, Child, End, Lib, Work};
use crate::xmlgen::Gen;
use larmorx_core::cfmt::{Captured, Stream};
use larmorx_core::defects::{self as d, Notices, UpstreamDefects};
use larmorx_io::gifti::{Dialect, Gifti, Gifticlib};
use larmorx_io::xml;
use std::ffi::c_int;

// ------------------------------------------------------------------ scan

pub fn scan(up: &Lib, work: &Work, n: u64) -> Tally {
    let mut t = Tally::new("scan");
    let mut g = Gen::new(0x5ca7_f32);
    let mut cases: Vec<Vec<u8>> = Vec::new();
    let words = [
        "1.0",
        "1",
        "1.00",
        "2.0",
        "abc",
        "",
        " 1.0",
        "1.0x",
        "nan",
        "-nan",
        "nan(123)",
        "nan(0x7f)",
        "inf",
        "-Infinity",
        "0x1p-149",
        "0x1.fffffep127",
        "0x1.ffffffp127",
        "3.4028235e38",
        "3.4028236e38",
        "1e-46",
        "1.4e-45",
        "7e-46",
        "0x",
        "0x.",
        ".5",
        "-.",
        "1e",
        "1e+",
        "+",
        "1.17549435e-38",
        "1.1754942e-38",
        "16777217",
        "0x1.000001p0",
        "0x1.0000011p0",
        "1.00000017881393432617187499",
        "1.000000178813934326171875",
        "1.00000017881393432617187501",
    ];
    for w in words {
        cases.push(w.as_bytes().to_vec());
    }
    while (cases.len() as u64) < n {
        let s = match g.below(4) {
            0 => {
                let v = f32::from_bits(g.rng.next_u64() as u32);
                larmorx_core::cfmt::g(v as f64, 1 + g.below(12) as usize)
            }
            1 => {
                let v = f64::from_bits(g.rng.next_u64());
                larmorx_core::cfmt::e(v, g.below(25) as usize)
            }
            2 => {
                let b = g.rng.next_u64() as u32;
                format!(
                    "0x{:x}.{:x}p{}",
                    b >> 16,
                    b & 0xffff,
                    g.below(320) as i64 - 170
                )
            }
            _ => {
                // halfway cases: a float's midpoint, written exactly, nudged
                let b = (g.rng.next_u64() as u32) & 0x7f7f_ffff;
                let lo = f32::from_bits(b) as f64;
                let hi = f32::from_bits(b + 1) as f64;
                let mid = (lo + hi) / 2.0;
                format!("{mid:.60e}")
            }
        };
        cases.push(s.into_bytes());
    }
    // one child for the whole batch: the C library's answers, 5 bytes each
    let child = work.run(600_000, || {
        let mut out = Vec::new();
        for c in &cases {
            let s = ffi::cstr(c);
            let mut bits = 0u32;
            // SAFETY: the shim's sscanf wrapper with a NUL-terminated string.
            let k = unsafe { (up.scanf_f32)(s.as_ptr(), &mut bits) };
            out.push(k as u8);
            out.extend_from_slice(&bits.to_le_bytes());
        }
        out
    });
    assert_eq!(child.end, End::Exit(0), "scan child");
    for (k, c) in cases.iter().enumerate() {
        t.cases += 1;
        t.values += 1;
        let r = &child.dump[5 * k..5 * k + 5];
        let theirs = (r[0] == 1).then(|| u32::from_le_bytes(r[1..5].try_into().unwrap()));
        let ours = larmorx_core::cscan::sscanf_f32(c).map(f32::to_bits);
        if ours != theirs {
            t.diff(&|| {
                format!(
                    "sscanf %f {:?}: port {ours:08x?} C {theirs:08x?}",
                    String::from_utf8_lossy(c)
                )
            });
        }
    }
    t
}

// ------------------------------------------------------------------ xml

fn port_events(data: &[u8], pieces: &[i32]) -> D {
    let mut d = D::default();
    let mut p = xml::Parser::new();
    let mut off = 0usize;
    for (k, &n) in pieces.iter().enumerate() {
        let n = n as usize;
        let r = p.parse(
            &data[off..off + n],
            k + 1 == pieces.len(),
            &mut Events(&mut d),
        );
        if r.is_err() {
            d.c(b'R');
            d.i(p.error_code());
            d.l(p.current_line_number() as i64);
            break;
        }
        off += n;
        d.c(b'K');
    }
    d
}

fn c_events(lib: &Lib, work: &Work, data: &[u8], pieces: &[i32]) -> Child {
    work.run(20_000, || {
        let cap = 1 << 20 | data.len() * 64;
        let mut out = vec![0u8; cap];
        // SAFETY: the shim's parser driver over buffers that live for the call.
        let n = unsafe {
            (lib.xml)(
                data.as_ptr(),
                data.len() as _,
                pieces.as_ptr(),
                pieces.len() as c_int,
                out.as_mut_ptr(),
                cap as _,
            )
        } as usize;
        assert!(n <= cap, "dump overflow");
        out.truncate(n);
        out
    })
}

pub fn xml(up: &Lib, fs: &Lib, work: &Work, n: u64) -> Tally {
    let mut t = Tally::new("xml");
    let mut g = Gen::new(0x0e8a_7201);
    for case in 0..n {
        let data = g.bytes();
        let pieces = g.pieces(data.len());
        t.cases += 1;
        let ours = match std::panic::catch_unwind(|| port_events(&data, &pieces)) {
            Ok(d) => d,
            Err(_) => {
                t.diff(&|| {
                    format!(
                        "case {case}: the port panicked on {:?}",
                        String::from_utf8_lossy(&data)
                    )
                });
                continue;
            }
        };
        for lib in [up, fs] {
            let c = c_events(lib, work, &data, &pieces);
            t.values += 1;
            if c.end != End::Exit(0) {
                t.diff(&|| format!("case {case}: {} ended {:?}", lib.name, c.end));
                continue;
            }
            if !ours.matches(&c.dump) {
                let a = dump::events_text(&ours.b);
                let b = dump::events_text(&c.dump);
                t.diff(&|| {
                    format!(
                        "case {case} vs {} (pieces {:?}):\n    doc {:?}\n  {}",
                        lib.name,
                        &pieces[..pieces.len().min(12)],
                        String::from_utf8_lossy(&data[..data.len().min(600)]),
                        dump::first_difference(&a, &b)
                    )
                });
                if std::env::var("LXPROBE_KEEP").is_ok() {
                    let _ = std::fs::write(work.dir.join(format!("xml-diff-{case}.xml")), &data);
                }
            }
        }
    }
    t
}

// ------------------------------------------------------------------ read (and write back)

/// Pointers as gifticlib prints them (`%p`) and the port's placeholders, made one token.
fn normalize(s: &str) -> String {
    let mut s = s.to_string();
    for p in [
        "<gxml_data>",
        "<children>",
        "<set>",
        "<buf>",
        "<da>",
        "<blen>",
        "<cdata>",
        "<gim>",
    ] {
        s = s.replace(p, "<ptr>");
    }
    // the compression ratio is the compressed stream's (zlib's own deflate is not reproduced)
    let s: String = s
        .lines()
        .map(|l| {
            if l.starts_with("-- compress buffer (") {
                format!(
                    "-- compress buffer (<ratio>{}",
                    &l[l.find("% of").unwrap_or(0)..]
                )
            } else {
                l.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + if s.ends_with('\n') { "\n" } else { "" };
    let b = s.as_bytes();
    let mut out = String::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'0' && i + 1 < b.len() && b[i + 1] == b'x' {
            let mut j = i + 2;
            while j < b.len() && b[j].is_ascii_hexdigit() {
                j += 1;
            }
            if j - i - 2 >= 6 {
                out.push_str("<ptr>");
                i = j;
                continue;
            }
        }
        let ch = s[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

/// A written file with every GZipBase64Binary `Data` replaced by its decompressed bytes in hex
/// (zlib's own deflate stream is not reproduced: the files are compared by content, as larmorx
/// compares `.h5` files), and whether any was replaced.
fn gz_content(f: &[u8]) -> (Vec<u8>, bool) {
    let s = String::from_utf8_lossy(f).into_owned();
    if !s.contains("Encoding=\"GZipBase64Binary\"") {
        return (f.to_vec(), false);
    }
    let mut out = String::new();
    let mut rest = s.as_str();
    let mut any = false;
    while let Some(da) = rest.find("<DataArray") {
        let (head, tail) = rest.split_at(da);
        out.push_str(head);
        let end = tail
            .find("</DataArray>")
            .map_or(tail.len(), |e| e + "</DataArray>".len());
        let (block, after) = tail.split_at(end);
        rest = after;
        if !block.contains("Encoding=\"GZipBase64Binary\"") {
            out.push_str(block);
            continue;
        }
        if let (Some(a), Some(b)) = (block.find("<Data>"), block.find("</Data>")) {
            let a = a + "<Data>".len();
            let text: String = block[a..b].chars().filter(|c| !c.is_whitespace()).collect();
            let z = b64_decode(&text);
            let mut plain = Vec::new();
            use std::io::Read;
            let ok = flate2::read::ZlibDecoder::new(&z[..])
                .read_to_end(&mut plain)
                .is_ok();
            out.push_str(&block[..a]);
            out.push_str(&format!("[inflated ok={ok} {:02x?}]", plain));
            out.push_str(&block[b..]);
            any = true;
        } else {
            out.push_str(block);
        }
    }
    out.push_str(rest);
    (out.into_bytes(), any)
}

fn b64_decode(s: &str) -> Vec<u8> {
    let val = |c: u8| -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        } as u32)
    };
    let mut out = Vec::new();
    let (mut acc, mut bits) = (0u32, 0);
    for c in s.bytes() {
        let Some(v) = val(c) else { continue };
        acc = acc << 6 | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    out
}

struct PortRead {
    dump: D,
    err: String,
    out: String,
    notices: Notices,
    written: Option<Vec<u8>>,
}

fn port_read(
    case: &crate::gifti_gen::Case,
    dialect: &dyn Dialect,
    mode: UpstreamDefects,
    wenc: i32,
    wdata: bool,
    wname: &str,
) -> PortRead {
    let mut cap = Captured::default();
    let mut notices = Notices::new(mode);
    let mut d = D::default();
    let mut g = Gifti::default();
    let s = case.set;
    g.set_verb(s[0]);
    g.set_b64_check(s[1]);
    g.set_xml_buf_size(s[2]);
    g.set_update_ok(s[3]);
    g.set_zlevel(s[4], &mut cap);
    g.set_indent(s[5]);
    let path = std::path::Path::new("case.gii");
    let mut written = None;
    for f in 0..case.nfiles {
        let gim = if !case.dalist.is_empty() {
            g.read_da_list(
                path,
                case.read_data,
                &case.dalist,
                dialect,
                &mut cap,
                &mut notices,
            )
        } else {
            g.read_image(path, case.read_data, dialect, &mut cap, &mut notices)
        };
        match gim {
            None => d.c(b'N'),
            Some(mut gim) => {
                dump::gim(&mut d, &gim, notices.contains(&d::FS_80));
                let fix = notices.fix();
                let v = g.valid_gifti_image(Some(&gim), true, dialect, fix, &mut cap, &mut notices);
                d.i(v as i32);
                if case.disp {
                    g.disp_gifti_image(Some("probe"), &gim, true, &mut cap, &mut notices);
                }
                if f == case.nfiles - 1 && wenc >= 0 {
                    if wenc > 0 {
                        for da in gim.darray.iter_mut().flatten() {
                            da.encoding = wenc;
                            if wenc == larmorx_io::gifti::GIFTI_ENCODING_EXTBIN {
                                da.ext_fname = Some(b"w.ext".to_vec());
                                da.ext_offset = 0;
                            }
                        }
                    }
                    d.c(b'W');
                    let _ = std::fs::remove_file("w.ext");
                    let _ = std::fs::remove_file("w.gii");
                    let _ = std::fs::remove_file(format!("{wname}.ext"));
                    let st = g.write_image(
                        &mut gim,
                        std::path::Path::new("w.gii"),
                        wdata,
                        &mut cap,
                        &mut notices,
                    );
                    d.i(st);
                    written = std::fs::read("w.gii").ok();
                    let _ = std::fs::rename("w.gii", wname);
                    let _ = std::fs::rename("w.ext", format!("{wname}.ext"));
                }
                g.free_image(gim, &mut cap);
            }
        }
    }
    PortRead {
        dump: d,
        err: cap.text(Stream::Err),
        out: cap.text(Stream::Out),
        notices,
        written,
    }
}

fn c_read(
    lib: &Lib,
    work: &Work,
    case: &crate::gifti_gen::Case,
    wenc: i32,
    wdata: bool,
    wname: &str,
) -> (Child, Option<Vec<u8>>) {
    let names: Vec<u8> = (0..case.nfiles)
        .flat_map(|_| b"case.gii\0".iter().copied())
        .collect();
    let wn = ffi::cstr(b"w.gii");
    let we = ffi::cstr(b"w.ext");
    let _ = std::fs::remove_file("w.gii");
    let _ = std::fs::remove_file("w.ext");
    let _ = std::fs::remove_file(format!("{wname}.ext"));
    let child = work.run(20_000, || {
        let cap = 64 << 20;
        let mut out = vec![0u8; cap];
        // SAFETY: the shim's reader over buffers that live for the call.
        let n = unsafe {
            (lib.read)(
                names.as_ptr() as *const _,
                case.nfiles,
                case.read_data as c_int,
                case.set.as_ptr(),
                case.dalist.as_ptr(),
                case.dalist.len() as c_int,
                case.disp as c_int,
                if wenc >= 0 {
                    wn.as_ptr()
                } else {
                    std::ptr::null()
                },
                wdata as c_int,
                wenc.max(0),
                we.as_ptr(),
                out.as_mut_ptr(),
                cap as _,
            )
        } as usize;
        assert!(n <= cap, "dump overflow");
        out.truncate(n);
        out
    });
    let w = std::fs::read("w.gii").ok();
    let _ = std::fs::rename("w.gii", wname);
    let _ = std::fs::rename("w.ext", format!("{wname}.ext"));
    (child, w)
}

pub fn read(up: &Lib, fs: &Lib, work: &Work, n: u64) -> Tally {
    let mut t = Tally::new("read");
    let mut g = Gen::new(0x9_1f71);
    let (mut gz_files, mut gz_same) = (0u64, 0u64);
    for case_no in 0..n {
        let case = g.gifti_case();
        for (name, bytes) in &case.ext {
            std::fs::write(work.dir.join(name), bytes).unwrap();
        }
        std::fs::write(work.dir.join("case.gii"), &case.file).unwrap();
        let wenc = [-1, 0, 0, 0, 1, 2, 3, 4][g.below(8) as usize];
        let wdata = !g.chance(6);
        if let Some(only) = std::env::var("LXPROBE_ONLY_CASE")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            && only != case_no
        {
            continue;
        }
        for (lib, dialect) in [
            (up, &Gifticlib as &dyn Dialect),
            (fs, &ProbeFs as &dyn Dialect),
        ] {
            t.cases += 1;
            let wname_c = format!("w-{}.gii", lib.name);
            let (c, cw) = c_read(lib, work, &case, wenc, wdata, &wname_c);
            let wname_p = format!("w-port-{}.gii", lib.name);
            let p = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                port_read(
                    &case,
                    dialect,
                    UpstreamDefects::Reproduce,
                    wenc,
                    wdata,
                    &wname_p,
                )
            })) {
                Ok(p) => p,
                Err(_) => {
                    t.diff(&|| format!("case {case_no}: the port panicked (reproduce)"));
                    continue;
                }
            };
            if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                port_read(
                    &case,
                    dialect,
                    UpstreamDefects::Fix,
                    wenc,
                    wdata,
                    "w-fix.gii",
                )
            }))
            .is_err()
            {
                t.diff(&|| format!("case {case_no}: the port panicked (fix)"));
            }
            let crashy = p.notices.contains(&d::FS_84)
                || p.notices.contains(&d::FS_85)
                || p.notices.contains(&d::FS_86);
            let garbage = crashy || p.notices.contains(&d::FS_80) || p.notices.contains(&d::FS_87);
            match c.end {
                End::Exit(0) => {}
                e => {
                    if crashy {
                        t.skipped += 1;
                    } else {
                        t.diff(&|| {
                            format!(
                                "case {case_no}: {} ended {e:?} (no crash notice in the port; set {:?} read_data {} dalist {:?} nfiles {} disp {} wenc {wenc} wdata {wdata})",
                                lib.name, case.set, case.read_data, case.dalist, case.nfiles, case.disp
                            )
                        });
                        keep(work, case_no, &case);
                    }
                    continue;
                }
            }
            if crashy {
                // the original read memory it never set (and did not crash this time)
                t.skipped += 1;
                continue;
            }
            t.values += 1;
            if std::env::var("LXPROBE_ONLY_CASE").is_ok() {
                let _ = std::fs::write(
                    work.dir.join(format!("dump-port-{}.bin", lib.name)),
                    &p.dump.b,
                );
                let _ = std::fs::write(work.dir.join(format!("dump-c-{}.bin", lib.name)), &c.dump);
            }
            if !p.dump.matches(&c.dump) && std::env::var("LXPROBE_IGNORE_VALUES").is_err() {
                let a = dump::read_text(&p.dump.filled(&c.dump));
                let b = dump::read_text(&c.dump);
                let at = (0..p.dump.b.len().max(c.dump.len()))
                    .find(|&k| {
                        k >= p.dump.b.len()
                            || k >= c.dump.len()
                            || (!p.dump.any[k] && p.dump.b[k] != c.dump[k])
                    })
                    .unwrap_or(0);
                t.diff(&|| {
                    format!(
                        "case {case_no} vs {} (set {:?}): values (first byte {at} of {} / {})\n  {}",
                        lib.name,
                        case.set,
                        p.dump.b.len(),
                        c.dump.len(),
                        dump::first_difference(&a, &b)
                    )
                });
                keep(work, case_no, &case);
                continue;
            }
            if garbage {
                t.skipped += 1;
                continue;
            }
            t.values += 1;
            let ce = normalize(&String::from_utf8_lossy(&c.err));
            let pe = normalize(&p.err);
            if ce != pe {
                let a: Vec<String> = pe.lines().map(String::from).collect();
                let b: Vec<String> = ce.lines().map(String::from).collect();
                t.diff(&|| {
                    format!(
                        "case {case_no} vs {} (set {:?}): stderr\n  {}",
                        lib.name,
                        case.set,
                        dump::first_difference(&a, &b)
                    )
                });
                keep(work, case_no, &case);
                continue;
            }
            t.values += 1;
            if String::from_utf8_lossy(&c.out) != p.out {
                t.diff(&|| format!("case {case_no} vs {}: stdout differs", lib.name));
                continue;
            }
            if wenc >= 0 {
                t.values += 1;
                let (a, gz) = gz_content(p.written.as_deref().unwrap_or(b"(none)"));
                let (b, _) = gz_content(cw.as_deref().unwrap_or(b"(none)"));
                if gz {
                    gz_files += 1;
                    gz_same += (p.written == cw) as u64;
                }
                if a != b {
                    let x: Vec<String> = String::from_utf8_lossy(&a)
                        .lines()
                        .map(String::from)
                        .collect();
                    let y: Vec<String> = String::from_utf8_lossy(&b)
                        .lines()
                        .map(String::from)
                        .collect();
                    t.diff(&|| {
                        format!(
                            "case {case_no} vs {}: written file (enc {wenc})\n  {}",
                            lib.name,
                            dump::first_difference(&x, &y)
                        )
                    });
                    keep(work, case_no, &case);
                }
                if wenc == 4 {
                    t.values += 1;
                    let a = std::fs::read(format!("{wname_p}.ext")).ok();
                    let b = std::fs::read(format!("{wname_c}.ext")).ok();
                    if a != b {
                        t.diff(&|| {
                            format!(
                                "case {case_no} vs {}: external data written differ: port {:?} bytes, original {:?} bytes",
                                lib.name,
                                a.as_ref().map(|v| v.len()),
                                b.as_ref().map(|v| v.len())
                            )
                        });
                    }
                    let _ = std::fs::remove_file(format!("{wname_p}.ext"));
                    let _ = std::fs::remove_file(format!("{wname_c}.ext"));
                }
            }
        }
        for (name, _) in &case.ext {
            let _ = std::fs::remove_file(work.dir.join(name));
        }
    }
    println!(
        "  read: GZipBase64Binary files written {gz_files}, byte-identical to zlib's {gz_same}"
    );
    t
}

fn keep(work: &Work, case_no: u64, case: &crate::gifti_gen::Case) {
    if std::env::var("LXPROBE_KEEP").is_ok() {
        let _ = std::fs::write(
            work.dir.join(format!("read-diff-{case_no}.gii")),
            &case.file,
        );
    }
}
