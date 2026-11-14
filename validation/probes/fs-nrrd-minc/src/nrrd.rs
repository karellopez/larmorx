// SPDX-License-Identifier: Apache-2.0
//! NRRD: generated files (headers of every field in valid and broken forms, attached and detached
//! data, the three encodings NrrdIO has and the two it lacks) read by the port and by NrrdIO.

use std::fmt::Write as _;
use std::path::Path;

use larmorx_core::defects::{Notices, UpstreamDefects};
use larmorx_io::nrrd::{self, Biff, LoadError, NRRD, Nrrd};

use crate::Rng;
use crate::ffi::{Lib, Outcome};

fn hex(b: &[u8]) -> String {
    let mut s = String::with_capacity(b.len() * 2);
    for c in b {
        let _ = write!(s, "{c:02x}");
    }
    s
}

fn cs(s: &Option<Vec<u8>>) -> String {
    match s {
        None => "-".into(),
        Some(v) => format!("s{}", hex(v)),
    }
}

fn dbl(v: f64) -> String {
    format!(" {:016x}", v.to_bits())
}

/// The shim's dump of a result (`shim_nrrd.c`'s `dump`).
pub fn dump(ret: i32, n: Option<&Nrrd>, err: Option<&[u8]>) -> String {
    let mut s = format!("ret {ret}\nbiff {}\n", cs(&err.map(<[u8]>::to_vec)));
    let Some(n) = n else { return s };
    if ret != 0 {
        return s;
    }
    let _ = writeln!(
        s,
        "dim {} type {} blockSize {} space {} spaceDim {}",
        n.dim, n.type_, n.block_size, n.space, n.space_dim
    );
    let _ = write!(
        s,
        "content {}\nsampleUnits {}\nspaceUnits",
        cs(&n.content),
        cs(&n.sample_units)
    );
    for u in &n.space_units {
        let _ = write!(s, " {}", cs(u));
    }
    s.push_str("\nspaceOrigin");
    for &v in &n.space_origin {
        s.push_str(&dbl(v));
    }
    s.push_str("\nmf");
    for row in &n.measurement_frame {
        for &v in row {
            s.push_str(&dbl(v));
        }
    }
    s.push_str("\nold");
    s.push_str(&dbl(n.old_min));
    s.push_str(&dbl(n.old_max));
    s.push('\n');
    for (i, a) in n.axis.iter().enumerate() {
        let _ = write!(s, "axis {i} {}", a.size);
        for v in [a.spacing, a.thickness, a.min, a.max] {
            s.push_str(&dbl(v));
        }
        for &v in &a.space_direction {
            s.push_str(&dbl(v));
        }
        let _ = writeln!(
            s,
            " {} {} {} {}",
            a.center,
            a.kind,
            cs(&a.label),
            cs(&a.units)
        );
    }
    let _ = writeln!(s, "cmt {}", n.cmt.len());
    for c in &n.cmt {
        let _ = writeln!(s, "{}", cs(&Some(c.clone())));
    }
    let _ = writeln!(s, "kvp {}", n.kvp.len());
    for (k, v) in &n.kvp {
        let _ = writeln!(s, "{} {}", cs(&Some(k.clone())), cs(&Some(v.clone())));
    }
    match &n.data {
        Some(d) => {
            let _ = writeln!(s, "data {}", hex(d));
        }
        None => s.push_str("data -\n"),
    }
    s
}

/// The port on `name`: what the original should do, and the notices.
pub struct Port {
    pub expect: Expect,
    pub notices: Vec<String>,
    pub stderr: Vec<u8>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Expect {
    Dump(String),
    Exit(i32, Vec<u8>),
}

struct ErrSink(Vec<u8>);
impl larmorx_core::cfmt::Streams for ErrSink {
    fn out(&mut self, _: &str) {}
    fn err(&mut self, t: &str) {
        self.0.extend_from_slice(t.as_bytes());
    }
}

pub fn port(name: &[u8], mode: UpstreamDefects) -> Port {
    let mut biff = Biff::new();
    let mut n = Notices::new(mode);
    let mut e = ErrSink(Vec::new());
    let r = nrrd::load_with(name, &mut biff, &mut n, &mut e);
    let notices = n
        .as_slice()
        .iter()
        .map(|x| x.defect.id.to_owned())
        .collect();
    let expect = match r {
        Ok(nr) => Expect::Dump(dump(0, Some(&nr), None)),
        Err(LoadError::Failed { code }) => {
            let msg = biff.get_done(NRRD);
            Expect::Dump(dump(code, None, msg.as_deref()))
        }
        Err(LoadError::Exit { status, message }) => Expect::Exit(status, message),
    };
    Port {
        expect,
        notices,
        stderr: e.0,
    }
}

// ------------------------------------------------------------------------------------------
// the generator
// ------------------------------------------------------------------------------------------

const TYPES: &[(&str, usize)] = &[
    ("signed char", 1),
    ("int8", 1),
    ("uchar", 1),
    ("unsigned char", 1),
    ("uint8_t", 1),
    ("short", 2),
    ("Short Int", 2),
    ("ushort", 2),
    ("uint16", 2),
    ("int", 4),
    ("int32_t", 4),
    ("uint", 4),
    ("unsigned int", 4),
    ("longlong", 8),
    ("int64", 8),
    ("unsigned long long int", 8),
    ("uint64", 8),
    ("float", 4),
    ("FLOAT", 4),
    ("double", 8),
    ("block", 3),
    ("char", 1),
    ("long", 8),
    ("float32", 4),
    ("", 1),
];

const SPACES: &[&str] = &[
    "RAS",
    "LAS",
    "LPS",
    "right-anterior-superior",
    "left-posterior-superior",
    "Left Anterior Superior",
    "RAST",
    "scanner-xyz",
    "scanner-xyzt",
    "3D-right-handed",
    "3D-left-handed",
    "3Drighthanded",
    "3Drighthanded3D-left-handed",
    "3D-right-handed-time",
    "3D-left-handed-time",
    "3Dlefthandedtime",
    "(unknown_space)",
    "ras ",
    "xyz",
];

const KINDS: &[&str] = &[
    "domain",
    "space",
    "time",
    "list",
    "vector",
    "covariant-vector",
    "stub",
    "scalar",
    "complex",
    "RGB",
    "rgba-color",
    "3-vector",
    "3-gradient",
    "3-normal",
    "quaternion",
    "2D-sym-tensor",
    "3D-matrix",
    "3D-masked-symmetric-matrix",
    "none",
    "???",
    "kind",
];

const ENCODINGS: &[&str] = &[
    "raw",
    "raw",
    "raw",
    "RAW",
    "ascii",
    "text",
    "txt",
    "hex",
    "gzip",
    "gz",
    "bz2",
    "bzip2",
    "(unknown_encoding)",
    "zip",
];

fn num_int(r: &mut Rng, good: i64) -> String {
    match r.below(14) {
        0 => format!("+{good}"),
        1 => format!("0{good}"),
        2 => format!("{good}x"),
        3 => "-1".into(),
        4 => "4294967297".into(),
        5 => "abc".into(),
        6 => format!(" {good}"),
        7 => format!("{good}.5"),
        _ => format!("{good}"),
    }
}

fn num_dbl(r: &mut Rng) -> String {
    match r.below(24) {
        0 => "nan".into(),
        1 => "NaN".into(),
        2 => "-inf".into(),
        3 => "inf".into(),
        4 => "1e400".into(),
        5 => "0x1p3".into(),
        6 => "1e-320".into(),
        7 => "banana".into(),
        8 => ".5".into(),
        9 => "5.".into(),
        10 => "-0".into(),
        11 => "abc".into(),
        12 => "0".into(),
        13 => "information".into(),
        _ => {
            let v = (r.below(20001) as f64 - 10000.0) / 64.0;
            format!("{v}")
        }
    }
}

fn vector(r: &mut Rng, sd: usize) -> String {
    match r.below(16) {
        0 => "none".into(),
        1 => "nonex".into(),
        2 => "1,0,0".into(),
        3 => "(1,0".into(),
        _ => {
            let k = if r.below(10) == 0 {
                sd + 1
            } else if r.below(15) == 0 {
                sd.saturating_sub(1)
            } else {
                sd
            };
            let vals: Vec<String> = (0..k)
                .map(|_| {
                    if r.below(8) == 0 {
                        num_dbl(r)
                    } else {
                        format!("{}", r.below(7) as f64 - 3.0)
                    }
                })
                .collect();
            let sep = if r.below(6) == 0 { ", " } else { "," };
            format!("({})", vals.join(sep))
        }
    }
}

fn quoted(r: &mut Rng) -> String {
    match r.below(12) {
        0 => "\"a\\\"b\"".into(),
        1 => "\"unterminated".into(),
        2 => "noquote".into(),
        3 => "\"\"".into(),
        _ => format!(
            "\"{}\"",
            ["mm", "s", "x y", "Hz", "um"][r.below(5) as usize]
        ),
    }
}

fn pick<'a>(r: &mut Rng, v: &[&'a str]) -> &'a str {
    v[r.below(v.len() as u64) as usize]
}

/// Writes one case into `dir`: the file to load.
pub fn generate(r: &mut Rng, dir: &Path) -> Vec<u8> {
    if r.below(12) == 0 {
        return special(r, dir);
    }
    if r.below(5) < 2 {
        return valid(r, dir);
    }
    let magic = match r.below(30) {
        0 => "NRRD0006",
        1 => "nrrd0004",
        2 => "NRRD0004 ",
        3 => "",
        4 => "NRRD00.01",
        5 => "NRRD0001",
        6 => "NRRD0002",
        7 => "NRRD0003",
        8 => "NRRD0005",
        _ => "NRRD0004",
    };
    let nl = if r.below(10) == 0 { "\r\n" } else { "\n" };
    let dim: usize = 1 + r.below(4) as usize;
    let sizes: Vec<usize> = (0..dim).map(|_| 1 + r.below(4) as usize).collect();
    let (tname, mut esize) = TYPES[r.below(TYPES.len() as u64) as usize];
    let block = tname == "block";
    if block {
        esize = 1 + r.below(4) as usize;
    }
    let sd: usize = if r.below(3) == 0 { 4 } else { 3 };
    let mut lines: Vec<String> = Vec::new();
    // the fields, most in their usual form, each sometimes broken
    let fname = |r: &mut Rng, name: &str, alt: &[&str]| -> String {
        if !alt.is_empty() && r.below(5) == 0 {
            alt[r.below(alt.len() as u64) as usize].to_owned()
        } else if r.below(25) == 0 {
            name.to_uppercase()
        } else {
            name.to_owned()
        }
    };
    let sep = |r: &mut Rng| -> &'static str {
        match r.below(30) {
            0 => ":",
            1 => ":\t",
            2 => ":  ",
            _ => ": ",
        }
    };
    let mut fields: Vec<String> = Vec::new();
    if r.below(30) != 0 {
        fields.push(format!("{}{}{}", fname(r, "type", &[]), sep(r), tname));
    }
    if r.below(30) != 0 {
        let d = if r.below(25) == 0 {
            num_int(r, dim as i64)
        } else if r.below(40) == 0 {
            "17".into()
        } else {
            dim.to_string()
        };
        fields.push(format!("dimension{}{}", sep(r), d));
    }
    if r.below(30) != 0 {
        let mut v: Vec<String> = sizes.iter().map(|s| s.to_string()).collect();
        match r.below(20) {
            0 => {
                v.push("2".into());
            }
            1 => {
                v.pop();
            }
            2 => v[0] = "0".into(),
            3 => v[0] = num_int(r, sizes[0] as i64),
            _ => {}
        }
        fields.push(format!(
            "sizes{}{}",
            sep(r),
            v.join(if r.below(8) == 0 { "\t" } else { " " })
        ));
    }
    if block && r.below(8) != 0 {
        fields.push(format!(
            "{}: {}",
            fname(r, "block size", &["blocksize"]),
            esize
        ));
    } else if r.below(30) == 0 {
        fields.push("block size: 2".into());
    }
    let has_space = r.below(2) == 0;
    if has_space {
        if r.below(4) == 0 {
            fields.push(format!(
                "{}: {}",
                fname(r, "space dimension", &["spacedimension"]),
                if r.below(10) == 0 {
                    "9".to_string()
                } else {
                    sd.to_string()
                }
            ));
        } else {
            fields.push(format!("space: {}", pick(r, SPACES)));
        }
        if r.below(3) != 0 {
            let v: Vec<String> = (0..dim + usize::from(r.below(15) == 0))
                .map(|_| vector(r, sd))
                .collect();
            fields.push(format!(
                "{}: {}",
                fname(r, "space directions", &["spacedirections"]),
                v.join(" ")
            ));
        }
        if r.below(2) == 0 {
            fields.push(format!(
                "{}: {}",
                fname(r, "space origin", &["spaceorigin"]),
                vector(r, sd)
            ));
        }
        if r.below(5) == 0 {
            let v: Vec<String> = (0..sd).map(|_| quoted(r)).collect();
            fields.push(format!("space units: {}", v.join(" ")));
        }
        if r.below(6) == 0 {
            let v: Vec<String> = (0..sd).map(|_| vector(r, sd)).collect();
            fields.push(format!(
                "{}: {}",
                fname(r, "measurement frame", &["measurementframe"]),
                v.join(" ")
            ));
        }
    }
    for (name, alt) in [
        ("spacings", &[][..]),
        ("thicknesses", &[][..]),
        ("axis mins", &["axismins"][..]),
        ("axis maxs", &["axismaxs"][..]),
    ] {
        if r.below(6) == 0 {
            let k = if r.below(10) == 0 { dim + 1 } else { dim };
            let v: Vec<String> = (0..k).map(|_| num_dbl(r)).collect();
            fields.push(format!("{}: {}", fname(r, name, alt), v.join(" ")));
        }
    }
    if r.below(5) == 0 {
        let v: Vec<String> = (0..dim)
            .map(|_| pick(r, &["cell", "node", "???", "none", "Cell", "edge"]).to_string())
            .collect();
        fields.push(format!(
            "{}: {}",
            fname(r, "centers", &["centerings"]),
            v.join(" ")
        ));
    }
    if r.below(4) == 0 {
        let v: Vec<String> = (0..dim + usize::from(r.below(12) == 0))
            .map(|_| pick(r, KINDS).to_string())
            .collect();
        fields.push(format!("kinds: {}", v.join(" ")));
    }
    for name in ["labels", "units"] {
        if r.below(7) == 0 {
            let v: Vec<String> = (0..dim + usize::from(r.below(12) == 0))
                .map(|_| quoted(r))
                .collect();
            fields.push(format!("{name}: {}", v.join(" ")));
        }
    }
    for (name, alt) in [
        ("old min", &["oldmin"][..]),
        ("old max", &["oldmax"][..]),
        ("min", &[][..]),
        ("max", &[][..]),
        ("number", &[][..]),
    ] {
        if r.below(10) == 0 {
            fields.push(format!("{}: {}", fname(r, name, alt), num_dbl(r)));
        }
    }
    let enc = pick(r, ENCODINGS);
    if r.below(30) != 0 {
        fields.push(format!("encoding: {enc}"));
    }
    let endian = match r.below(6) {
        0 => None,
        1 => Some("big"),
        2 => Some(pick(r, &["Big", "middle", "LITTLE"])),
        _ => Some("little"),
    };
    if let Some(e) = endian {
        fields.push(format!("endian: {e}"));
    }
    let line_skip = if r.below(10) == 0 { r.below(3) } else { 0 };
    if line_skip > 0 || r.below(30) == 0 {
        fields.push(format!(
            "{}: {}",
            fname(r, "line skip", &["lineskip"]),
            line_skip
        ));
    }
    let byte_skip: i64 = match r.below(12) {
        0 => r.below(4) as i64,
        1 => -1,
        2 => -2,
        _ => 0,
    };
    if byte_skip != 0 {
        fields.push(format!(
            "{}: {}",
            fname(r, "byte skip", &["byteskip"]),
            byte_skip
        ));
    }
    if r.below(6) == 0 {
        fields.push(format!("content: {}", pick(r, &["a b  c", "", "x"])));
    }
    if r.below(8) == 0 {
        fields.push(format!("sample units: {}", pick(r, &["mm", "", "a  b"])));
    }
    for _ in 0..r.below(3) {
        let c = match r.below(12) {
            0 => "#".to_string(),
            1 => "# Complete NRRD file format specification at:".to_string(),
            2 => "## a\tb   c ".to_string(),
            3 => "# caf\u{e9}".to_string(),
            4 => "#\x01ctl".to_string(),
            _ => format!("# comment {}", r.below(100)),
        };
        fields.push(c);
    }
    for _ in 0..r.below(3) {
        let kv = match r.below(10) {
            0 => "key:=".to_string(),
            1 => ":=value".to_string(),
            2 => "a\\\\b:=c\\nd\\".to_string(),
            3 => "dup:=one".to_string(),
            4 => "dup:=two".to_string(),
            5 => "notakey: x".to_string(),
            6 => "noseparator".to_string(),
            7 => "k:=v: with colon".to_string(),
            _ => format!("k{}:=v{}", r.below(5), r.below(5)),
        };
        fields.push(kv);
    }
    if r.below(40) == 0 {
        fields.push("type: float".into());
    }
    // shuffle, keeping a few orders that matter (space before space directions)
    let n = fields.len();
    for i in 0..n {
        if r.below(4) == 0 {
            let j = r.below(n as u64) as usize;
            fields.swap(i, j);
        }
    }
    lines.push(magic.to_owned());
    lines.extend(fields);
    // the data
    let elems: usize = sizes.iter().product();
    let mut bytes: Vec<u8> = (0..elems * esize).map(|_| r.below(256) as u8).collect();
    let encoded: Vec<u8> = match enc {
        "ascii" | "text" | "txt" => {
            let vals: Vec<String> = (0..elems + usize::from(r.below(10) == 0))
                .map(|_| match r.below(10) {
                    0 => num_dbl(r),
                    1 => "300".into(),
                    2 => "-5".into(),
                    3 => "nan".into(),
                    _ => format!("{}", r.below(200) as i64 - 50),
                })
                .collect();
            let joined = vals.join(pick(r, &[" ", "\n", "  \t", ","]));
            let mut v = joined.into_bytes();
            if r.below(10) == 0 {
                v.truncate(v.len() / 2);
            }
            v.push(b'\n');
            v
        }
        "hex" => {
            let mut s = String::new();
            for (i, b) in bytes.iter().enumerate() {
                let _ = write!(
                    s,
                    "{}",
                    if r.below(2) == 0 {
                        format!("{b:02x}")
                    } else {
                        format!("{b:02X}")
                    }
                );
                if i % 3 == 2 {
                    s.push_str(pick(r, &[" ", "\n", "\t", ""]));
                }
            }
            let mut v = s.into_bytes();
            if r.below(10) == 0 && !v.is_empty() {
                let at = r.below(v.len() as u64) as usize;
                v[at] = pick(r, &["g", "\u{1}", "z"]).as_bytes()[0];
            }
            if r.below(12) == 0 && !v.is_empty() {
                let at = r.below(v.len() as u64) as usize;
                v[at] = [0xb1u8, 0xa0, 0xc1, 0xff][r.below(4) as usize];
            }
            if r.below(10) == 0 {
                v.truncate(v.len() / 2);
            }
            v
        }
        _ => {
            match r.below(12) {
                0 => bytes.truncate(bytes.len() / 2),
                1 => bytes.extend_from_slice(&[1, 2, 3]),
                _ => {}
            }
            bytes.clone()
        }
    };
    let mut skipped = Vec::new();
    for _ in 0..line_skip {
        skipped.extend_from_slice(b"skip me\n");
    }
    if byte_skip > 0 {
        skipped.extend((0..byte_skip).map(|_| b'#'));
    }
    let mut payload = skipped;
    payload.extend_from_slice(&encoded);
    // attached or detached
    let detached = r.below(3) == 0;
    let mut file_bytes;
    if detached {
        let how = r.below(6);
        let mut parts: Vec<Vec<u8>> = Vec::new();
        let df = match how {
            0 if dim >= 2 => {
                let last = sizes[dim - 1];
                let fmt = pick(
                    r,
                    &[
                        "d%d.raw",
                        "d%03d.raw",
                        "d%%x%d.raw",
                        "d%d%d.raw",
                        "d%2d.raw",
                    ],
                );
                for k in 0..last {
                    parts.push(payload.clone());
                    let name = c_sprintf(fmt, k as i64);
                    if let Some(nm) = name {
                        std::fs::write(dir.join(nm), &payload).unwrap();
                    }
                }
                let _ = parts;
                format!(
                    "data file: {fmt} 0 {} 1{}",
                    last as i64 - 1,
                    if r.below(5) == 0 {
                        format!(" {}", dim - 1)
                    } else {
                        String::new()
                    }
                )
            }
            1 => {
                let k = if dim >= 2 { sizes[dim - 1] } else { 1 };
                let mut l = "data file: LIST".to_string();
                if r.below(4) == 0 {
                    l.push_str(&format!(" {}", dim.saturating_sub(1).max(1)));
                }
                let mut s = l;
                for i in 0..k {
                    let name = format!("p{i}.raw");
                    std::fs::write(dir.join(&name), &payload).unwrap();
                    s.push_str(nl);
                    s.push_str(&name);
                }
                s
            }
            2 => "data file: missing.raw".to_string(),
            _ => {
                std::fs::write(dir.join("one.raw"), &payload).unwrap();
                if r.below(8) == 0 {
                    format!("data file: {}", dir.join("one.raw").display())
                } else {
                    "data file: one.raw".to_string()
                }
            }
        };
        if r.below(20) == 0 {
            lines.insert(1, df);
        } else {
            lines.push(df);
        }
        let mut s = lines.join(nl);
        if r.below(6) != 0 {
            s.push_str(nl);
        }
        file_bytes = s.into_bytes();
    } else {
        let mut s = lines.join(nl);
        s.push_str(nl);
        if r.below(25) != 0 {
            s.push_str(nl);
        }
        file_bytes = s.into_bytes();
        file_bytes.extend_from_slice(&payload);
    }
    if r.below(40) == 0 && file_bytes.len() > 20 {
        let at = 9 + r.below((file_bytes.len() - 9) as u64) as usize;
        file_bytes[at] = 0;
    }
    let name = if detached { "h.nhdr" } else { "a.nrrd" };
    std::fs::write(dir.join(name), &file_bytes).unwrap();
    dir.join(name).to_string_lossy().into_owned().into_bytes()
}

/// A well-formed file: every field and encoding NrrdIO reads, with values that read.
fn valid(r: &mut Rng, dir: &Path) -> Vec<u8> {
    let nl = if r.below(8) == 0 { "\r\n" } else { "\n" };
    let magic = ["NRRD0001", "NRRD0002", "NRRD0003", "NRRD0004", "NRRD0005"][r.below(5) as usize];
    let dim = 1 + r.below(4) as usize;
    let sizes: Vec<usize> = (0..dim).map(|_| 1 + r.below(5) as usize).collect();
    let types: &[(&str, usize, bool)] = &[
        ("signed char", 1, true),
        ("uchar", 1, false),
        ("short", 2, true),
        ("ushort", 2, false),
        ("int", 4, true),
        ("uint", 4, false),
        ("int64", 8, true),
        ("uint64", 8, false),
        ("float", 4, true),
        ("double", 8, true),
        ("block", 0, false),
    ];
    let (tname, mut esize, signed) = types[r.below(types.len() as u64) as usize];
    let block = tname == "block";
    if block {
        esize = 1 + r.below(5) as usize;
    }
    let enc = if block {
        "raw"
    } else {
        ["raw", "raw", "ascii", "hex"][r.below(4) as usize]
    };
    let mut f: Vec<String> = vec![
        format!("type: {tname}"),
        format!("dimension: {dim}"),
        format!(
            "sizes: {}",
            sizes
                .iter()
                .map(|x| x.to_string())
                .collect::<Vec<_>>()
                .join(" ")
        ),
        format!("encoding: {enc}"),
    ];
    if block {
        f.push(format!("block size: {esize}"));
    }
    let big = r.below(3) == 0;
    if esize > 1 || r.below(3) == 0 {
        f.push(format!("endian: {}", if big { "big" } else { "little" }));
    }
    let space = r.below(3);
    if space > 0 {
        let sd = if space == 1 {
            f.push(format!(
                "space: {}",
                [
                    "RAS",
                    "LAS",
                    "LPS",
                    "left-posterior-superior",
                    "scanner-xyz",
                    "RAST"
                ][r.below(6) as usize]
            ));
            if f.last().unwrap().ends_with('T') {
                4
            } else {
                3
            }
        } else {
            let sd = 1 + r.below(4) as usize;
            f.push(format!("space dimension: {sd}"));
            sd
        };
        let dirs: Vec<String> = (0..dim)
            .map(|_| {
                if r.below(4) == 0 {
                    "none".to_string()
                } else {
                    let v: Vec<String> = (0..sd)
                        .map(|_| format!("{}", (r.below(2001) as f64 - 1000.0) / 8.0))
                        .collect();
                    format!("({})", v.join(","))
                }
            })
            .collect();
        f.push(format!("space directions: {}", dirs.join(" ")));
        if r.below(2) == 0 {
            let v: Vec<String> = (0..sd)
                .map(|_| format!("{:e}", (r.below(100001) as f64 - 50000.0) / 7.0))
                .collect();
            f.push(format!("space origin: ({})", v.join(",")));
        }
        if r.below(3) == 0 {
            let v: Vec<String> = (0..sd)
                .map(|_| format!("\"{}\"", ["mm", "m", "a \\\"q\\\" b"][r.below(3) as usize]))
                .collect();
            f.push(format!("space units: {}", v.join(" ")));
        }
        if r.below(3) == 0 {
            let v: Vec<String> = (0..sd)
                .map(|_| {
                    format!(
                        "({})",
                        (0..sd)
                            .map(|_| format!("{}", r.below(3) as f64 - 1.0))
                            .collect::<Vec<_>>()
                            .join(",")
                    )
                })
                .collect();
            f.push(format!("measurement frame: {}", v.join(" ")));
        }
    } else {
        for name in ["spacings", "axis mins", "axis maxs"] {
            if r.below(3) == 0 {
                let v: Vec<String> = (0..dim)
                    .map(|_| format!("{}", (r.below(1000) as f64 + 1.0) / 3.0))
                    .collect();
                f.push(format!("{name}: {}", v.join(" ")));
            }
        }
        if r.below(3) == 0 {
            f.push(format!(
                "units: {}",
                (0..dim).map(|_| "\"mm\"").collect::<Vec<_>>().join(" ")
            ));
        }
    }
    if r.below(3) == 0 {
        f.push(format!(
            "thicknesses: {}",
            (0..dim)
                .map(|_| format!("{}", r.below(5)))
                .collect::<Vec<_>>()
                .join(" ")
        ));
    }
    if r.below(3) == 0 {
        f.push(format!(
            "centers: {}",
            (0..dim)
                .map(|_| ["cell", "node", "???", "none"][r.below(4) as usize])
                .collect::<Vec<_>>()
                .join(" ")
        ));
    }
    if r.below(3) == 0 {
        let k: Vec<&str> = sizes
            .iter()
            .map(|&s| match s {
                1 => ["stub", "scalar", "domain"][r.below(3) as usize],
                2 => ["complex", "2-vector", "space"][r.below(3) as usize],
                3 => ["RGB-color", "3-vector", "3-normal", "3D-sym-matrix", "list"]
                    [r.below(5) as usize],
                4 => ["RGBA", "quaternion", "4-vector", "2D-matrix", "time"][r.below(5) as usize],
                _ => ["domain", "space", "list", "point", "vector", "???", "none"]
                    [r.below(7) as usize],
            })
            .collect();
        f.push(format!("kinds: {}", k.join(" ")));
    }
    if r.below(3) == 0 {
        f.push(format!(
            "labels: {}",
            (0..dim)
                .map(|i| format!("\"axis {i}\""))
                .collect::<Vec<_>>()
                .join(" ")
        ));
    }
    for name in [
        "old min",
        "old max",
        "content",
        "sample units",
        "number",
        "min",
        "max",
    ] {
        if r.below(4) == 0 {
            f.push(format!(
                "{name}: {}",
                ["1.5", "-2e3", "nan", "x y  z", "0x1p-3"][r.below(5) as usize]
            ));
        }
    }
    for i in 0..r.below(4) {
        f.push(
            [
                "# a comment",
                "#",
                "##  two   spaces  ",
                "# Complete NRRD file format specification at:",
                "#\ttab",
            ][r.below(5) as usize]
                .to_string(),
        );
        f.push(format!("key{}:=value {}", r.below(3), i));
    }
    for i in (1..f.len()).rev() {
        let j = 1 + r.below(i as u64) as usize;
        if r.below(2) == 0 && f[i].starts_with("space") == f[j].starts_with("space") {
            f.swap(i, j);
        }
    }
    // the samples
    let elems: usize = sizes.iter().product();
    let mut raw = Vec::new();
    let mut ascii = Vec::new();
    for _ in 0..elems {
        let v: i64 = if signed {
            r.below(4000) as i64 - 2000
        } else {
            r.below(4000) as i64
        };
        ascii.push(match tname {
            "float" | "double" => format!("{}", v as f64 / 16.0),
            _ => v.to_string(),
        });
        let le: Vec<u8> = match tname {
            "float" => (v as f32 / 16.0).to_le_bytes().to_vec(),
            "double" => (v as f64 / 16.0).to_le_bytes().to_vec(),
            _ if block => (0..esize).map(|_| r.below(256) as u8).collect(),
            _ => v.to_le_bytes()[..esize].to_vec(),
        };
        let mut e = le;
        if big && !block {
            e.reverse();
        }
        raw.extend_from_slice(&e);
    }
    let payload: Vec<u8> = match enc {
        "ascii" => {
            let mut v = ascii
                .join([" ", "\n", "\t "][r.below(3) as usize])
                .into_bytes();
            v.push(b'\n');
            v
        }
        "hex" => {
            let mut s = String::new();
            for (i, c) in raw.iter().enumerate() {
                s.push_str(&format!("{c:02x}"));
                if i % 4 == 3 {
                    s.push_str(["\n", " ", ""][r.below(3) as usize]);
                }
            }
            s.into_bytes()
        }
        _ => raw,
    };
    let skip = r.below(3);
    let mut pre = Vec::new();
    if skip > 0 {
        f.push(format!("line skip: {skip}"));
        for _ in 0..skip {
            pre.extend_from_slice(b"ignored line\n");
        }
    }
    let bskip = if enc != "ascii" && r.below(4) == 0 {
        r.below(3) as i64 + if r.below(3) == 0 { -1 } else { 0 }
    } else {
        0
    };
    if bskip != 0 {
        f.push(format!("byte skip: {bskip}"));
        if bskip > 0 {
            pre.extend((0..bskip).map(|_| b'Z'));
        }
    }
    let detached = r.below(2) == 0;
    let mut text = format!("{magic}{nl}{}{nl}", f.join(nl));
    let mut body = pre.clone();
    body.extend_from_slice(&payload);
    if bskip == -1 {
        body = pre;
        body.extend_from_slice(b"leading junk");
        body.extend_from_slice(&payload);
    }
    if detached {
        let last = sizes[dim - 1];
        let per = payload.len() / last.max(1);
        match r.below(3) {
            0 if dim >= 2 && enc == "raw" && bskip == 0 && skip == 0 => {
                for k in 0..last {
                    std::fs::write(
                        dir.join(format!("s{k:02}.raw")),
                        &payload[k * per..(k + 1) * per],
                    )
                    .unwrap();
                }
                text.push_str(&format!("data file: s%02d.raw 0 {} 1{nl}", last - 1));
            }
            1 if dim >= 2 && enc == "raw" && bskip == 0 && skip == 0 => {
                text.push_str(&format!("data file: LIST{nl}"));
                for k in 0..last {
                    std::fs::write(
                        dir.join(format!("p{k}.raw")),
                        &payload[k * per..(k + 1) * per],
                    )
                    .unwrap();
                    text.push_str(&format!("p{k}.raw{nl}"));
                }
            }
            _ => {
                std::fs::write(dir.join("d.raw"), &body).unwrap();
                text.push_str(&format!("data file: d.raw{nl}"));
            }
        }
        std::fs::write(dir.join("h.nhdr"), text.as_bytes()).unwrap();
        return dir
            .join("h.nhdr")
            .to_string_lossy()
            .into_owned()
            .into_bytes();
    }
    text.push_str(nl);
    let mut b = text.into_bytes();
    b.extend_from_slice(&body);
    std::fs::write(dir.join("a.nrrd"), &b).unwrap();
    dir.join("a.nrrd")
        .to_string_lossy()
        .into_owned()
        .into_bytes()
}

/// The cases aimed at NrrdIO's undefined paths and at its long-line reader.
fn special(r: &mut Rng, dir: &Path) -> Vec<u8> {
    let long = |n: usize| -> String { "x".repeat(n) };
    let base = "NRRD0004\ntype: uchar\ndimension: 1\nsizes: 2\nencoding: raw\n";
    let (name, bytes): (String, Vec<u8>) = match r.below(9) {
        // FS-762: a message longer than 256 bytes (by a few, or by far)
        0 => {
            let k = 200 + r.below(120) as usize;
            ("a.nrrd".into(), format!("NRRD0004\ntype: {}\n", long(k)).into_bytes())
        }
        1 => {
            let k = 200 + r.below(300) as usize;
            ("a.nrrd".into(), format!("NRRD0004\ndimension: 1\nsizes: {}\n", long(k)).into_bytes())
        }
        // a long file name of a missing file (nrrdLoad's fopen message)
        2 => {
            let k = 180 + r.below(80) as usize;
            let n = format!("{}.nrrd", long(k));
            return dir.join(n).to_string_lossy().into_owned().into_bytes();
        }
        // FS-765: 17 sizes for 16 dimensions
        3 => {
            let v = vec!["1"; 17 + r.below(2) as usize].join(" ");
            ("a.nrrd".into(), format!("NRRD0004\ntype: uchar\ndimension: 16\nsizes: {v}\nencoding: raw\n\n\x01").into_bytes())
        }
        // FS-766: templates
        4 => {
            let f = ["d%d%d.raw", "d%050d.raw", "d%s%d.raw", "d%d%%.raw", "d%5d.raw"][r.below(5) as usize];
            let _ = std::fs::write(dir.join("d    0.raw"), [1]);
            let _ = std::fs::write(dir.join("d%.raw"), [1]);
            ("h.nhdr".into(), format!("NRRD0004\ntype: uchar\ndimension: 2\nsizes: 1 1\nencoding: raw\ndata file: {f} 0 0 1\n").into_bytes())
        }
        // FS-767: a long ASCII value
        5 => {
            let k = 900 + r.below(300) as usize;
            ("a.nrrd".into(), format!("NRRD0004\ntype: int\ndimension: 1\nsizes: 1\nencoding: ascii\n\n{}\n", "7".repeat(k)).into_bytes())
        }
        // FS-768: a size whose bytes pass 2^64
        6 => ("a.nrrd".into(), b"NRRD0004\ntype: int\ndimension: 1\nsizes: 4611686018427387904\nendian: little\nencoding: raw\n\n\x01\x02".to_vec()),
        // long lines that fit (the line buffer's growth), comments and key/values
        7 => {
            let k = 20 + r.below(240) as usize;
            let mut s = format!("NRRD0004\n# {}\nk:={}\n", long(k), long(k / 2 + 1));
            s.push_str(&base[9..]);
            s.push('\n');
            let mut b = s.into_bytes();
            b.extend_from_slice(&[4, 5]);
            ("a.nrrd".into(), b)
        }
        // the end of the file inside a long last line
        _ => {
            let k = 10 + r.below(200) as usize;
            let _ = std::fs::write(dir.join(long(k)), [9, 8]);
            ("h.nhdr".into(), format!("{base}data file: {}", long(k)).into_bytes())
        }
    };
    std::fs::write(dir.join(&name), &bytes).unwrap();
    dir.join(&name).to_string_lossy().into_owned().into_bytes()
}

/// C's `sprintf(fmt, k)` for the generator's templates (None: undefined).
fn c_sprintf(fmt: &str, k: i64) -> Option<String> {
    match fmt {
        "d%d.raw" => Some(format!("d{k}.raw")),
        "d%03d.raw" => Some(format!("d{k:03}.raw")),
        "d%%x%d.raw" => Some(format!("d%x{k}.raw")),
        "d%2d.raw" => Some(format!("d{k:2}.raw")),
        _ => None,
    }
}

/// The defects whose original behaviour is undefined, a crash or a hang: not compared, the
/// original's outcome counted.
const UNDEFINED: &[&str] = &[
    "FS-761", "FS-762", "FS-765", "FS-766", "FS-767", "FS-768", "FS-771",
];

#[derive(Default)]
pub struct Tally {
    pub cases: u64,
    pub same: u64,
    pub differ: u64,
    pub skipped: u64,
    pub read_ok: u64,
    pub outcomes: std::collections::BTreeMap<String, u64>,
    pub notices: std::collections::BTreeMap<String, u64>,
    shown: u32,
}

pub fn run(seed: u64, count: u64, work: &Path, up: &Lib, fs: &Lib, t: &mut Tally) {
    let mut r = Rng::new(seed);
    let show: u32 = std::env::var("LXPROBE_SHOW")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10);
    for case in 0..count {
        let dir = work.join(format!("n{case}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let name = generate(&mut r, &dir);
        let p = port(&name, UpstreamDefects::Reproduce);
        let pf = port(&name, UpstreamDefects::Fix);
        for id in &pf.notices {
            *t.notices.entry(id.clone()).or_default() += 1;
        }
        t.cases += 1;
        let undefined = p.notices.iter().any(|n| UNDEFINED.contains(&n.as_str()));
        let timeout = if p.notices.iter().any(|n| n == "FS-761") {
            300
        } else {
            5000
        };
        let mut keep = false;
        for (lib, side) in [(up, "up"), (fs, "fs")] {
            let (oc, err) = lib.run(&name, timeout);
            let label = match &oc {
                Outcome::Returned(_) => "returned".to_string(),
                Outcome::Exit(s) => format!("exit {s}"),
                Outcome::Signal(s) => format!("signal {s}"),
                Outcome::Hang => "hang".to_string(),
            };
            if undefined {
                let key = format!(
                    "{side} {} -> {label}",
                    p.notices
                        .iter()
                        .filter(|n| UNDEFINED.contains(&n.as_str()))
                        .cloned()
                        .collect::<Vec<_>>()
                        .join("+")
                );
                *t.outcomes.entry(key).or_default() += 1;
                t.skipped += 1;
                continue;
            }
            let ok = match (&p.expect, &oc) {
                (super::nrrd::Expect::Dump(d), Outcome::Returned(o)) => d == o && err == p.stderr,
                (super::nrrd::Expect::Exit(s, m), Outcome::Exit(so)) => s == so && &err == m,
                _ => false,
            };
            if ok {
                t.same += 1;
                if let Outcome::Returned(o) = &oc {
                    if o.starts_with("ret 0\n") {
                        t.read_ok += 1;
                    }
                }
            } else {
                t.differ += 1;
                if t.shown < show {
                    t.shown += 1;
                    keep = true;
                    println!(
                        "  DIFF nrrd seed {seed} case {case} ({side}): kept in {}",
                        work.join(format!("keep-{seed}-{case}")).display()
                    );
                    println!(
                        "    port     {:?} stderr {:?} notices {:?}",
                        p.expect,
                        String::from_utf8_lossy(&p.stderr),
                        p.notices
                    );
                    println!(
                        "    original {oc:?} stderr {:?}",
                        String::from_utf8_lossy(&err)
                    );
                }
            }
        }
        if keep {
            let k = work.join(format!("keep-{seed}-{case}"));
            let _ = std::fs::remove_dir_all(&k);
            let _ = std::fs::rename(&dir, &k);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
