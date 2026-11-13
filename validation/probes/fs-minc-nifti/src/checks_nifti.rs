// SPDX-License-Identifier: Apache-2.0
//! nifticlib 2.0.0's file names and znzlib (ported in `larmorx_io::nifti::niftilib` and
//! `larmorx_io::znz`), against nifticlib 2.0.0 and FreeSurfer 8.2.0's copy, both over the
//! machine's zlib 1.2.11.

use std::io::Write as _;
use std::path::Path;

use larmorx_core::cfmt::{Captured, Stream};
use larmorx_io::nifti::niftilib::{self, NiftiOptions};
use larmorx_core::defects::UpstreamDefects;
use larmorx_io::znz::{Whence, ZnzFile};

use crate::generate::Gen;
use crate::{Capture, Lib, Tally, ffi::cstr};

const BASES: &[&str] = &["a", "sub/b", "", "x.y", ".", "c.d.e", "UP"];
const EXTS: &[&str] = &[
    ".nii", ".nii.gz", ".hdr", ".hdr.gz", ".img", ".img.gz", ".nia", ".nia.gz", ".NII", ".NII.GZ",
    ".Nii", ".nii.GZ", ".HDR", ".HDR.GZ", ".IMG", ".IMG.GZ", ".gz", ".GZ", ".Gz", "", ".txt", "nii",
    ".ni", ".hdr.Gz",
];

fn gz(data: &[u8], level: u32) -> Vec<u8> {
    let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::new(level));
    e.write_all(data).unwrap();
    e.finish().unwrap()
}

/// A file's content: a header of one kind (NIfTI-1 single file, pair, ANALYZE either byte order,
/// nothing valid), maybe short, maybe gzipped.
fn header_file(g: &mut Gen) -> Vec<u8> {
    let mut h = vec![0u8; 352];
    for b in h.iter_mut() {
        *b = (g.rng.next_u64() & 0xff) as u8;
    }
    match g.below(6) {
        0 => h[344..348].copy_from_slice(b"n+1\0"),
        1 => h[344..348].copy_from_slice(b"ni1\0"),
        2 => h[0..4].copy_from_slice(&348i32.to_le_bytes()),
        3 => h[0..4].copy_from_slice(&348i32.to_be_bytes()),
        4 => h[344..348].copy_from_slice(b"n+0\0"),
        _ => {}
    }
    if g.below(6) == 0 {
        h.truncate(g.below(352) as usize);
    }
    h
}

pub fn check_names(up: &Lib, fs: &Lib, work: &Path, scale: u64, cap: &mut Capture) -> Vec<Tally> {
    let mut t = [Tally::new("names vs nifti"), Tally::new("names vs FS")];
    let mut g = Gen::new(0x0f0e_0011);
    let dir = work.join("names");
    let opts = NiftiOptions::default();
    for k in 0..6_000 * scale {
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        // a few files among the candidate names
        let mut files = Vec::new();
        for _ in 0..g.below(6) {
            let name = format!("{}{}", g.pick(BASES), g.pick(EXTS));
            if name.is_empty() || name == "." || name == "sub" {
                continue;
            }
            let mut content = header_file(&mut g);
            let lower = name.to_ascii_lowercase();
            if lower.ends_with(".gz") && g.below(3) > 0 {
                content = gz(&content, 6);
            }
            if std::fs::write(dir.join(&name), &content).is_ok() {
                files.push(name);
            }
        }
        let query = if !files.is_empty() && g.below(3) == 0 {
            files[g.below(files.len() as u64) as usize].clone()
        } else {
            format!("{}{}", g.pick(BASES), g.pick(EXTS))
        };
        let full = format!("{}/{}", dir.display(), query);
        let qb: Vec<u8> = if query.is_empty() { Vec::new() } else { full.clone().into_bytes() };
        for (i, lib) in [up, fs].into_iter().enumerate() {
        // FreeSurfer's copy, as its behaviour shows it: no upper-case extensions, `.gz` only in
        // lower case, `.nia.gz` a compressed extension.
        let opts = if i == 0 { opts } else { NiftiOptions { allow_upper_fext: false, upper_case_gz: false, nia_gz: true, ..opts } };
        // the port
        let mut c = Captured::default();
        let hdr = niftilib::findhdrname(&qb, &opts, &mut c);
        let base = niftilib::makebasename(&qb, &opts, &mut c);
        let ext = niftilib::find_file_extension(&qb, &opts, &mut c);
        let text = format!(
            "hdr={}|base={}|ext={}|gz={}|valid={}|exists={}|complete={}",
            hdr.as_deref().map_or("(NULL)".into(), String::from_utf8_lossy),
            String::from_utf8_lossy(&base),
            ext.map_or(-1, |e| e as i64),
            niftilib::is_gzfile_with(&qb, &opts) as i32,
            niftilib::validfilename(&qb, &opts, &mut c) as i32,
            niftilib::fileexists(&qb) as i32,
            niftilib::is_complete_filename(&qb, &opts, &mut c) as i32,
        );
        let names_err = c.text(Stream::Err);
        let mut c2 = Captured::default();
        let isn = niftilib::is_nifti_file(&qb, &opts, &mut c2);
        let isn_err = c2.text(Stream::Err);
        {
            let q = cstr(&qb);
            let mut out = vec![0u8; 8192];
            // SAFETY: a NUL-terminated name and a buffer of the stated size.
            let (n, _, err) = cap.run(|| unsafe { (lib.nifti_names.unwrap())(q.as_ptr(), out.as_mut_ptr().cast(), out.len() as i32) });
            out.truncate(n as usize);
            t[i].cases += 1;
            let shown = || format!("case {k} {query:?} files {files:?}");
            t[i].eq(&|| format!("{} names", shown()), &text, &String::from_utf8_lossy(&out).into_owned());
            t[i].eq(&|| format!("{} names stderr", shown()), &names_err, &err);
            // SAFETY: a NUL-terminated name.
            let (r, _, err2) = cap.run(|| unsafe { (lib.is_nifti_file.unwrap())(q.as_ptr()) });
            t[i].eq(&|| format!("{} is_nifti_file", shown()), (isn, &isn_err), (r, &err2));
        }
        }
    }
    t.into()
}

/// A file to read: plain bytes, gzip (one or several members, trailing bytes, truncated,
/// damaged), empty.
fn read_file(g: &mut Gen) -> (Vec<u8>, &'static str) {
    let n = match g.below(4) {
        0 => g.below(100) as usize,
        1 => g.below(20_000) as usize,
        _ => g.below(200_000) as usize,
    };
    let mut data = vec![0u8; n];
    let kind = g.below(3);
    for (i, b) in data.iter_mut().enumerate() {
        *b = match kind {
            0 => (g.rng.next_u64() & 0xff) as u8,
            1 => (i % 7) as u8,
            _ => ((i / 300) % 256) as u8,
        };
    }
    match g.below(12) {
        0 => (data, "plain"),
        1 => (Vec::new(), "empty"),
        2 => {
            let mut a = gz(&data[..n / 2], 6);
            a.extend(gz(&data[n / 2..], 1));
            (a, "two members")
        }
        3 => {
            let mut a = gz(&data, 6);
            a.extend_from_slice(b"trailing bytes");
            (a, "trailing")
        }
        4 => {
            let mut a = gz(&data, 6);
            let cut = g.below(a.len() as u64 + 1) as usize;
            a.truncate(cut);
            (a, "truncated")
        }
        5 | 6 => {
            let mut a = gz(&data, 6);
            if a.len() > 12 {
                let at = 10 + g.below(a.len() as u64 - 10) as usize;
                a[at] ^= 1 << g.below(8);
            }
            (a, "damaged")
        }
        _ => (gz(&data, [1, 6, 9][g.below(3) as usize]), "gzip"),
    }
}

/// One script of operations (the shim's encoding) and the port's results in the same encoding.
fn script(g: &mut Gen, write: bool, total: usize, comp: bool) -> Vec<u8> {
    let mut ops = Vec::new();
    for _ in 0..1 + g.below(14) {
        let c = if write {
            *[b'w', b'w', b'w', b's', b't', b'p', b'f', b'e'].get(g.below(8) as usize).unwrap()
        } else {
            *[b'r', b'r', b'r', b'r', b's', b's', b't', b'e', b'g', b'R'].get(g.below(10) as usize).unwrap()
        };
        ops.push(c);
        match c {
            b'r' | b'w' => {
                // a member size of 0 on a compressed file divides by zero in nifticlib (FS-76)
                let size: u32 = [1, 1, 1, 2, 4, 8, if comp { 3 } else { 0 }][g.below(7) as usize];
                let nmemb: u32 = match g.below(4) {
                    0 => g.below(10) as u32,
                    1 => g.below(20_000) as u32,
                    2 => (total as u64 / size.max(1) as u64 + g.below(3)) as u32,
                    _ => g.below(70_000) as u32,
                };
                let nmemb = nmemb.min(200_000 / size.max(1));
                ops.extend_from_slice(&size.to_le_bytes());
                ops.extend_from_slice(&nmemb.to_le_bytes());
                if c == b'w' {
                    for i in 0..(size * nmemb) as usize {
                        ops.push(((i * 31 + g.below(3) as usize) % 251) as u8);
                    }
                }
            }
            b's' => {
                let off: i64 = match g.below(4) {
                    0 => g.below(100) as i64 - 50,
                    1 => g.below(total as u64 + 100) as i64,
                    _ => g.below(70_000) as i64 - 10_000,
                };
                ops.extend_from_slice(&off.to_le_bytes());
                ops.push([0u8, 1, 2][g.below(3) as usize]);
            }
            b'p' => ops.push((g.rng.next_u64() & 0xff) as u8),
            _ => {}
        }
    }
    ops
}

/// Which znzlib: nifticlib 2.0.0's (`larmorx_io::znz::ZnzFile`), or FreeSurfer 8.2.0's as its
/// behaviour shows it (never its source): on a compressed file one `gzread` or `gzwrite` of
/// `size * nmemb` bytes whose count is divided by `size` (so zlib's -1 becomes `SIZE_MAX / size`),
/// no "short" warning, and `gzrewind` for `znzrewind`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Flavour {
    Nifticlib,
    FreeSurfer,
}

fn port_script(path: &[u8], mode: &[u8], comp: bool, ops: &[u8], defects: UpstreamDefects) -> (Vec<u8>, String) {
    port_script_as(path, mode, comp, ops, defects, Flavour::Nifticlib).0
}

fn port_script_as(
    path: &[u8],
    mode: &[u8],
    comp: bool,
    ops: &[u8],
    defects: UpstreamDefects,
    flavour: Flavour,
) -> ((Vec<u8>, String), bool) {
    let mut out = Vec::new();
    let mut c = Captured::default();
    let put = |o: &mut Vec<u8>, v: i64| o.extend_from_slice(&v.to_le_bytes());
    let Some(f) = ZnzFile::open(path, mode, comp) else {
        put(&mut out, 0);
        return ((out, String::new()), false);
    };
    let mut f = f.with_upstream_defects(defects);
    let fsgz = flavour == Flavour::FreeSurfer && f.compressed();
    put(&mut out, 1);
    let mut i = 0;
    let u32at = |at: usize| u32::from_le_bytes(ops[at..at + 4].try_into().unwrap()) as usize;
    while i < ops.len() {
        let op = ops[i];
        i += 1;
        match op {
            b'r' => {
                let (a, b) = (u32at(i), u32at(i + 4));
                i += 8;
                let mut buf = vec![0u8; a * b];
                let got = if fsgz {
                    let n = f.gz().unwrap().gzread(&mut buf);
                    if n < 0 { (u64::MAX / a as u64) as i64 } else { (n as usize / a) as i64 }
                } else {
                    f.read(&mut buf, a, b, &mut c)
                };
                put(&mut out, got);
                out.extend_from_slice(&buf);
            }
            b'w' => {
                let (a, b) = (u32at(i), u32at(i + 4));
                i += 8;
                let got = if fsgz {
                    let n = f.gz().unwrap().gzwrite(&ops[i..i + a * b]);
                    if n < 0 { (u64::MAX / a as u64) as i64 } else { (n as usize / a) as i64 }
                } else {
                    f.write(&ops[i..i + a * b], a, b, &mut c)
                };
                put(&mut out, got);
                i += a * b;
            }
            b's' => {
                let off = i64::from_le_bytes(ops[i..i + 8].try_into().unwrap());
                let w = [Whence::Set, Whence::Cur, Whence::End][ops[i + 8] as usize];
                i += 9;
                put(&mut out, f.seek(off, w));
            }
            b't' => put(&mut out, f.tell()),
            b'e' => put(&mut out, f.eof() as i64),
            b'g' => put(&mut out, f.getc() as i64),
            b'p' => {
                put(&mut out, f.putc(ops[i] as i32) as i64);
                i += 1;
            }
            b'f' => put(&mut out, f.flush() as i64),
            b'R' => {
                let r = if fsgz { if f.gz().unwrap().gzrewind().is_ok() { 0 } else { -1 } } else { f.rewind() as i64 };
                put(&mut out, r);
            }
            _ => {}
        }
    }
    let (r, notices) = f.close_with_notices();
    put(&mut out, r as i64);
    ((out, c.text(Stream::Err)), notices.contains(&larmorx_core::defects::FS_78))
}

fn c_script(lib: &Lib, cap: &mut Capture, path: &[u8], mode: &[u8], comp: bool, ops: &[u8]) -> (Vec<u8>, String) {
    let mut out = vec![0u8; 8 << 20];
    let mut len = 0;
    let (p, m) = (cstr(path), cstr(mode));
    // SAFETY: NUL-terminated strings, the ops and a buffer of the stated sizes.
    let (_, _, err) = cap.run(|| unsafe {
        (lib.znz_script.unwrap())(p.as_ptr(), m.as_ptr(), comp as i32, ops.as_ptr(), ops.len() as i32, out.as_mut_ptr(), out.len() as i32, &mut len)
    });
    out.truncate(len as usize);
    (out, err)
}

fn gunzip_all(data: &[u8]) -> Option<Vec<u8>> {
    use std::io::Read;
    let mut out = Vec::new();
    flate2::read::MultiGzDecoder::new(data).read_to_end(&mut out).ok().map(|_| out)
}

pub fn check_znz(up: &Lib, fs: &Lib, work: &Path, scale: u64, cap: &mut Capture) -> Vec<Tally> {
    let mut tr = [Tally::new("znz read nifti"), Tally::new("znz read FS")];
    let mut tw = [Tally::new("znz write nifti"), Tally::new("znz write FS")];
    let mut g = Gen::new(0x0f0e_0012);
    let path = work.join("znz.bin");
    let pb = path.as_os_str().as_encoded_bytes().to_vec();
    let mut kinds = std::collections::BTreeMap::<&str, u64>::new();
    for k in 0..3_000 * scale {
        let (content, kind) = read_file(&mut g);
        *kinds.entry(kind).or_default() += 1;
        std::fs::write(&path, &content).unwrap();
        let comp = g.below(5) > 0;
        let mode: &[u8] = [&b"rb"[..], b"r", b"rb", b"r+b", b"rT"][g.below(5) as usize];
        let ops = script(&mut g, false, content.len().max(1) * 3, comp);
        for (i, lib) in [up, fs].into_iter().enumerate() {
            // nifticlib 2.0.0 counts a partly read member (FS-77, reproduced); FreeSurfer's copy
            // counts the complete ones (larmorx's correction).
            let mode_d = if i == 0 { UpstreamDefects::Reproduce } else { UpstreamDefects::Fix };
            let flavour = if i == 0 { Flavour::Nifticlib } else { Flavour::FreeSurfer };
            let (ours, _) = port_script_as(&pb, mode, comp, &ops, mode_d, flavour);
            let theirs = c_script(lib, cap, &pb, mode, comp, &ops);
            if ours != theirs && std::env::var_os("LXPROBE_KEEP").is_some() {
                let keep = work.join(format!("fail-{k}.bin"));
                let _ = std::fs::write(&keep, &content);
                println!("  KEPT {} mode {:?} comp {comp} ops {}", keep.display(), String::from_utf8_lossy(mode), zt_ops(&ops));
            }
            tr[i].cases += 1;
            tr[i].eq(&|| format!("case {k} {kind} {} bytes mode {:?} comp {comp} ops {:?}", content.len(), String::from_utf8_lossy(mode), &ops[..ops.len().min(80)]), &ours, &theirs);
        }
    }
    tr[0].notes.push(format!("files: {kinds:?}"));
    let mut same_bytes = [0u64; 2];
    let mut gaps = [0u64; 2];
    let mut compressed = [0u64; 2];
    for k in 0..2_000 * scale {
        let comp = g.below(4) > 0;
        let mode: &[u8] = [&b"wb"[..], b"wb6", b"wb1", b"wb9", b"ab", b"w", b"wT"][g.below(7) as usize];
        let ops = script(&mut g, true, 50_000, comp);
        let prior = if g.below(4) == 0 { gz(b"prior member", 6) } else { Vec::new() };
        for (i, lib) in [up, fs].into_iter().enumerate() {
            let mode_d = if i == 0 { UpstreamDefects::Reproduce } else { UpstreamDefects::Fix };
            let flavour = if i == 0 { Flavour::Nifticlib } else { Flavour::FreeSurfer };
            std::fs::write(&path, &prior).unwrap();
            let (ours, zero_gap) = port_script_as(&pb, mode, comp, &ops, mode_d, flavour);
            let ours_file = std::fs::read(&path).unwrap_or_default();
            std::fs::write(&path, &prior).unwrap();
            let theirs = c_script(lib, cap, &pb, mode, comp, &ops);
            let theirs_file = std::fs::read(&path).unwrap_or_default();
            tw[i].cases += 1;
            let what = || format!("case {k} mode {:?} comp {comp} ops {:?}", String::from_utf8_lossy(mode), &ops[..ops.len().min(60)]);
            tw[i].eq(&|| format!("{} results", what()), &ours, &theirs);
            let direct = !comp || mode.contains(&b'T');
            if zero_gap {
                // FS-78: zlib 1.2.11 wrote memory it never set where larmorx writes zeros.
                gaps[i] += 1;
            } else if direct {
                tw[i].eq(&|| format!("{} file", what()), &ours_file, &theirs_file);
            } else {
                // The decompressed content must agree; the compressed bytes are deflate's.
                tw[i].eq(&|| format!("{} content", what()), gunzip_all(&ours_file), gunzip_all(&theirs_file));
                compressed[i] += 1;
                if ours_file == theirs_file {
                    same_bytes[i] += 1;
                }
            }
        }
    }
    tw[0].notes.push(format!("compressed files byte-identical to zlib 1.2.11's: {} of {}; files not compared (FS-78, a gap before the first write): {}", same_bytes[0], compressed[0], gaps[0]));
    tw[1].notes.push(format!("compressed files byte-identical to zlib 1.2.11's: {} of {}; files not compared (FS-78): {}", same_bytes[1], compressed[1], gaps[1]));
    let [a, b] = tr;
    let [c, d] = tw;
    vec![a, b, c, d]
}

/// The ops in the debugging tool's spelling (`r:size:n`, `s:off:whence`, ...).
fn zt_ops(ops: &[u8]) -> String {
    let mut i = 0;
    let mut v = Vec::new();
    let u32at = |at: usize| u32::from_le_bytes(ops[at..at + 4].try_into().unwrap());
    while i < ops.len() {
        let op = ops[i];
        i += 1;
        match op {
            b'r' => {
                v.push(format!("r:{}:{}", u32at(i), u32at(i + 4)));
                i += 8;
            }
            b's' => {
                v.push(format!("s:{}:{}", i64::from_le_bytes(ops[i..i + 8].try_into().unwrap()), ops[i + 8]));
                i += 9;
            }
            c => v.push((c as char).to_string()),
        }
    }
    v.join(" ")
}

/// `--replay`: runs one script on the port and prints its results as the debugging tool does.
pub fn replay(args: &[String]) {
    let path = args[0].as_bytes();
    let mode = args[1].as_bytes();
    let comp = args[2] == "1";
    let defects = if args[3] == "1" { UpstreamDefects::Reproduce } else { UpstreamDefects::Fix };
    let mut ops = Vec::new();
    for a in &args[4..] {
        let c = a.as_bytes()[0];
        ops.push(c);
        let parts: Vec<&str> = a.split(':').collect();
        match c {
            b'r' => {
                ops.extend_from_slice(&parts[1].parse::<u32>().unwrap().to_le_bytes());
                ops.extend_from_slice(&parts[2].parse::<u32>().unwrap().to_le_bytes());
            }
            b's' => {
                ops.extend_from_slice(&parts[1].parse::<i64>().unwrap().to_le_bytes());
                ops.push(parts[2].parse::<u8>().unwrap());
            }
            _ => {}
        }
    }
    let (out, _) = port_script(path, mode, comp, &ops, defects);
    if let Some(d) = std::env::var_os("LXPROBE_DUMP") {
        std::fs::write(d, &out).unwrap();
    }
    let v = |at: usize| i64::from_le_bytes(out[at..at + 8].try_into().unwrap());
    let mut p = 8;
    let mut line = format!("open={}", v(0));
    for a in &args[4..] {
        line.push_str(&format!(" {}={}", &a[..1], v(p)));
        p += 8;
        if a.starts_with('r') {
            let parts: Vec<usize> = a.split(':').skip(1).map(|x| x.parse().unwrap()).collect();
            p += parts[0] * parts[1];
        }
    }
    line.push_str(&format!(" close={}", v(p)));
    println!("{line}");
}
