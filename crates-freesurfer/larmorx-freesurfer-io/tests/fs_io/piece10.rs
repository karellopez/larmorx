// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The fs.io probe's piece-10 cases (relay 19): fs.labels' text colour tables and lookups
//! (`colortab.cpp`). `cra` (`CTABreadASCII2` of a generated table file, `ctabDuplicates`), `crt`
//! (`CTABreadASCIIttHeader`), `ctd` (`CTABreadDefault` with `FREESURFER_HOME`, `SUBJECTS_DIR` and
//! `FS_DESIRED_BVEC_SPACE` set or not), `cal` (`CTABalloc` after `setRandomSeed`), `cun`
//! (`CTABunique`), `ccr` (`CTABcountRepeats`), `cq` (the counts, tests and lookups), `cpw`
//! (`CTABwriteFileASCII` and the file's bytes), `cps` (`CTABprintASCII` to standard output), `cts`
//! (`TissueTypeSchema` of a table or of the default one), `cme` (`CTABmerge`). The table files are
//! built from the format's rules: entry lines of five numbers and a name (and a tissue type) with
//! every separator, sign, overflow and stray character, comments (whole lines and after entries),
//! blank lines, `#ctTType` lines whole and broken, lines of other white space, CR LF ends, lines
//! longer than the readers' buffer, names with `#`, case variants and bytes beyond ASCII; missing
//! files and folders.

use std::io::Write;

use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::random;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_io::colortab;
use larmorx_freesurfer_mri::ctab::{ColorTable, ColorTableEntry};

use super::piece4::{file_entry, setup};
use super::{Gen, Out, Work, dec, enc, hex};

/// The piece-10 commands.
pub const KINDS: [&str; 11] = [
    "cra", "crt", "ctd", "cal", "cun", "ccr", "cq", "cpw", "cps", "cts", "cme",
];

// ---------------------------------------------------------------- generation

const NAMES: [&[u8]; 22] = [
    b"Unknown",
    b"Left-Cerebral-Cortex",
    b"left-cerebral-cortex",
    b"LEFT-CEREBRAL-CORTEX",
    b"Right-Hippocampus",
    b"ctx-lh-bankssts",
    b"WM-hypointensities",
    b"wm",
    b"WM",
    b"Brain-Stem",
    b"a#b",
    b"x#",
    b"caf\xc3\xa9",
    b"\xff\x01n",
    b"cluster3",
    b"unknown",
    b"Left-Lateral-Ventricle",
    b"CSF",
    b"head",
    b"x",
    b"Optic-Chiasm",
    b"N",
];

fn name(g: &mut Gen) -> Vec<u8> {
    if g.chance(60) {
        return vec![b'L'; g.int(25, 40) as usize];
    }
    g.pick(&NAMES).to_vec()
}

fn sep(g: &mut Gen) -> &'static [u8] {
    g.pick(&[b" ".as_slice(), b"  ", b"\t", b" \t ", b"   "])
}

fn number(g: &mut Gen, lo: i64, hi: i64) -> Vec<u8> {
    if g.chance(12) {
        return g
            .pick(&[
                b"-1".as_slice(),
                b"256",
                b"2147483647",
                b"-2147483648",
                b"99999999999",
                b"+7",
                b"0x1f",
                b"1e3",
                b"abc",
                b"007",
                b"-0",
                b"4294967297",
            ])
            .to_vec();
    }
    g.int(lo, hi).to_string().into_bytes()
}

fn structure(g: &mut Gen, used: &[i64]) -> Vec<u8> {
    match g.below(40) {
        0 => b"-3".to_vec(),
        1 => b"16777300".to_vec(),
        2..=5 if !used.is_empty() => g.pick(used).to_string().into_bytes(),
        6..=8 => g.int(100, 3000).to_string().into_bytes(),
        9 => g
            .pick(&[
                1000i64, 1035, 2000, 3201, 4207, 5001, 11100, 12300, 914, 258,
            ])
            .to_string()
            .into_bytes(),
        10 => b"0".to_vec(),
        _ => g.int(1, 60).to_string().into_bytes(),
    }
}

fn line_end(g: &mut Gen) -> &'static [u8] {
    g.pick(&[
        b"\n".as_slice(),
        b"\n",
        b"\n",
        b"\r\n",
        b"  # a comment\n",
        b"#c\n",
        b" \n",
    ])
}

/// A `#ctTType` line: whole mostly, else broken.
fn tt_line(g: &mut Gen) -> Vec<u8> {
    let mut l = b"#ctTType".to_vec();
    if g.chance(10) {
        l.extend_from_slice(g.pick(&[b"\n".as_slice(), b"  \n", b"\t\r\n"]));
        return l;
    }
    let k = if g.chance(5) { g.int(1, 5) } else { 6 };
    let fields: [Vec<u8>; 6] = [
        if g.chance(8) {
            g.pick(&[b"abc".as_slice(), b"-1", b"x9"]).to_vec()
        } else {
            g.int(0, 6).to_string().into_bytes()
        },
        g.pick(&[
            b"unknown".as_slice(),
            b"cortex",
            b"subcort_gm",
            b"wm",
            b"csf",
            b"head",
        ])
        .to_vec(),
        number(g, 0, 255),
        number(g, 0, 255),
        number(g, 0, 255),
        number(g, 0, 255),
    ];
    for f in &fields[..k as usize] {
        l.extend_from_slice(sep(g));
        l.extend_from_slice(f);
    }
    l.extend_from_slice(line_end(g));
    l
}

/// A text colour table.
fn lut(g: &mut Gen) -> Vec<u8> {
    let mut s = Vec::new();
    let mut used: Vec<i64> = Vec::new();
    let n = g.int(0, 14);
    for _ in 0..n {
        match g.below(24) {
            0 => s.extend_from_slice(g.pick(&[
                b"# FreeSurfer colour table\n".as_slice(),
                b"#No. Label Name:  R   G   B   A\n",
                b"   # indented comment\n",
                b"#\n",
            ])),
            1 => s.extend_from_slice(g.pick(&[b"\n".as_slice(), b"   \t\n", b"\r\n"])),
            2 | 3 => s.extend_from_slice(&tt_line(g)),
            4 if g.chance(4) => {
                s.extend_from_slice(g.pick(&[b"\x0b\n".as_slice(), b" \x0c \n", b"\x0b\x0c\n"]))
            }
            5 if g.chance(3) => {
                // longer than the readers' 1,023-byte buffer
                let mut l = b"7 ".to_vec();
                l.extend(std::iter::repeat_n(b'q', g.int(1000, 1100) as usize));
                l.extend_from_slice(b" 1 2 3 0 9 8 7 6 5\n");
                s.extend_from_slice(&l);
            }
            6 if g.chance(6) => s.extend_from_slice(b"12 nul\x00name 1 2 3 0\n"),
            _ => {
                let st = structure(g, &used);
                if let Ok(v) = std::str::from_utf8(&st).unwrap_or("x").parse::<i64>() {
                    used.push(v);
                }
                let mut l = Vec::new();
                if g.chance(8) {
                    l.extend_from_slice(b"  ");
                }
                l.extend_from_slice(&st);
                l.extend_from_slice(sep(g));
                l.extend_from_slice(&name(g));
                let k = if g.chance(10) { g.int(2, 3) } else { 4 };
                for _ in 0..k {
                    l.extend_from_slice(sep(g));
                    l.extend_from_slice(&number(g, 0, 255));
                }
                if g.chance(3) {
                    l.extend_from_slice(sep(g));
                    l.extend_from_slice(&number(g, -1, 5));
                }
                l.extend_from_slice(line_end(g));
                s.extend_from_slice(&l);
            }
        }
    }
    if g.chance(5) && s.last() == Some(&b'\n') {
        s.pop();
    }
    s
}

/// A table in the probe's form: `<version> <nentries> <idbase> <fname> <k>`, then `k` entries
/// `<index> <name> <r> <g> <b> <a> <tissue type> <count>`.
fn table(g: &mut Gen, holes: bool) -> Vec<String> {
    let nentries = if g.chance(10) {
        g.int(0, 3)
    } else {
        g.int(1, 24)
    };
    let mut t = vec![
        g.pick(&[2i64, 1, -2]).to_string(),
        nentries.to_string(),
        if g.chance(4) {
            g.pick(&[1000i64, -1, 2147483647]).to_string()
        } else {
            "0".into()
        },
        enc(Some(g.pick(&[
            b"none".as_slice(),
            b"w/lut.txt",
            b"",
            b"a b",
        ]))),
    ];
    let mut entries = Vec::new();
    for i in 0..nentries {
        if holes && g.chance(3) {
            continue;
        }
        let nm = if g.chance(6) {
            let mut v = g.pick(&NAMES).to_vec();
            v.extend_from_slice(g.pick(&[b" ".as_slice(), b"\t", b" x "]));
            v.extend_from_slice(b"y");
            v
        } else {
            name(g)
        };
        let col = |g: &mut Gen| -> i64 {
            if g.chance(10) {
                g.pick(&[-1i64, 256, 1000, -2147483648, 2147483647])
            } else if g.chance(3) {
                g.pick(&[0i64, 255, 7])
            } else {
                g.int(0, 255)
            }
        };
        let (r, gg, b) = (col(g), col(g), col(g));
        entries.push(vec![
            i.to_string(),
            enc(Some(&nm)),
            r.to_string(),
            gg.to_string(),
            b.to_string(),
            g.pick(&[255i64, 0, 128]).to_string(),
            g.int(-1, 5).to_string(),
            g.pick(&[0i64, 0, 17]).to_string(),
        ]);
    }
    t.push(entries.len().to_string());
    for e in entries {
        t.extend(e);
    }
    t
}

fn env3(g: &mut Gen) -> Vec<String> {
    vec![
        g.pick(&["x77", "x77", "x77", "N", "x772f6e6f6e65"]).into(),
        g.pick(&["N", "x77"]).into(),
        g.pick(&["N", "N", "N", "N", "N", "x31", "x32", "x33", "x78"])
            .into(),
    ]
}

/// A well-formed table: the default table's first entries, a tissue-type header sometimes.
fn good_lut(g: &mut Gen) -> Vec<u8> {
    let mut s = Vec::new();
    if g.chance(3) {
        s.extend_from_slice(b"# TissueTypeSchema default-jan-2014\n#ctTType   0  unknown   0 0 0  0\n#ctTType   1  cortex  205 62 78  0\n#ctTType   3  wm  0 255 0  0\n");
    }
    s.extend_from_slice(b"#$Id: FreeSurferColorLUT.txt\n\n#No. Label Name:                            R   G   B   A\n\n");
    let rows: [&[u8]; 10] = [
        b"0   Unknown                                 0   0   0   0",
        b"2   Left-Cerebral-White-Matter            245 245 245  0",
        b"3   Left-Cerebral-Cortex                  205  62  78  0",
        b"4   Left-Lateral-Ventricle                120  18 134  0",
        b"17  Left-Hippocampus                      220 216  20  0",
        b"24  CSF                                    60  60  60  0",
        b"42  Right-Cerebral-Cortex                 205  62  78  0",
        b"165 Skull                                 120 120  20  0",
        b"258 Head-ExtraCerebral                    150 150 200  0",
        b"1035 ctx-lh-insula                        255 192  32  0",
    ];
    for r in rows {
        if g.chance(5) {
            continue;
        }
        s.extend_from_slice(r);
        if g.chance(3) {
            s.extend_from_slice(g.pick(&[b"  1".as_slice(), b" 3", b" -1"]));
        }
        s.push(b'\n');
    }
    s
}

fn lut_setup(g: &mut Gen, file: &str) -> String {
    if file.ends_with("FreeSurferColorLUT.txt") && !g.chance(4) {
        return file_entry(file, &good_lut(g));
    }
    if g.chance(15) {
        return format!("D{}", hex(file.as_bytes()));
    }
    if g.chance(20) {
        return "-".into();
    }
    file_entry(file, &lut(g))
}

/// One piece-10 case.
pub fn generate(g: &mut Gen, kind: &str, portable: bool) -> Vec<String> {
    let mut t = vec![kind.to_string()];
    match kind {
        "cra" | "crt" => {
            if kind == "cra" {
                t.push(g.pick(&["0", "1", "1"]).into());
            }
            let file = g.pick(&["w/lut.txt", "w/lut.txt", "w/a b.txt"]);
            t.push(lut_setup(g, file));
            let asked = if g.chance(20) {
                g.pick(&["w/missing.txt", "", "d", "w"])
            } else {
                file
            };
            t.push(enc(Some(asked.as_bytes())));
        }
        "ctd" => {
            t.extend(env3(g));
            t.push(lut_setup(g, "w/FreeSurferColorLUT.txt"));
        }
        "cal" => {
            t.push(g.int(-1000, 100000).to_string());
            t.push(
                if g.chance(15) {
                    g.pick(&[0i64, -1, 1])
                } else {
                    g.int(2, 60)
                }
                .to_string(),
            );
        }
        "cun" => {
            t.push(g.int(-1000, 100000).to_string());
            t.push(g.pick(&[0i64, 1, 3, 10, 100]).to_string());
            let holes = g.chance(6);
            t.extend(table(g, holes));
        }
        "ccr" => {
            t.push(g.pick(&["0", "1"]).into());
            let holes = g.chance(6);
            t.extend(table(g, holes));
        }
        "cq" => {
            t.extend(table(g, true));
            let n = g.int(1, 8);
            t.push(n.to_string());
            for _ in 0..n {
                let op = g.pick(&[
                    "v", "t", "e", "h", "r", "n", "a", "x", "y", "i", "f", "F", "g", "m", "d",
                ]);
                t.push(op.into());
                let idx = |g: &mut Gen| g.pick(&[-1i64, 0, 1, 2, 3, 5, 9, 23, 24, 100]).to_string();
                let comp = |g: &mut Gen| {
                    if g.chance(5) {
                        g.pick(&[-1i64, 256, 65536, -2147483648]).to_string()
                    } else {
                        g.int(0, 255).to_string()
                    }
                };
                match op {
                    "e" | "h" | "r" | "i" => t.push(idx(g)),
                    "n" => {
                        t.push(idx(g));
                        t.push(g.pick(&[0i64, 1, 3, 8, 20, 1024]).to_string());
                    }
                    "a" | "g" => {
                        for _ in 0..3 {
                            t.push(comp(g));
                        }
                    }
                    "x" | "y" | "m" => {
                        let nm = if g.chance(40) {
                            vec![b'n'; 1100]
                        } else {
                            name(g)
                        };
                        t.push(enc(Some(&nm)));
                    }
                    "f" | "F" => t.push(
                        if g.chance(3) {
                            g.int(-2147483648, 2147483647)
                        } else {
                            g.int(0, 255) + (g.int(0, 255) << 8) + (g.int(0, 255) << 16)
                        }
                        .to_string(),
                    ),
                    _ => {}
                }
            }
        }
        "cpw" => {
            t.push(g.pick(&[0i64, 0, 65536]).to_string());
            // a folder or a file in place of a folder: Linux's errors (not portable)
            let out = if portable {
                g.pick(&["w/out.txt", "w/out.txt", "w/no/x"])
            } else {
                g.pick(&["w/out.txt", "w/out.txt", "w/o", "w/no/x", "nr/x"])
            };
            t.push(if out == "w/o" && g.chance(2) {
                format!("D{}", hex(b"w/o"))
            } else {
                "-".into()
            });
            t.extend(table(g, true));
            t.push(enc(Some(out.as_bytes())));
        }
        "cps" => {
            t.push(g.pick(&[0i64, 65536]).to_string());
            t.extend(table(g, true));
        }
        "cts" => {
            let schema: &[u8] = g.pick(&[
                b"default-jan-2014".as_slice(),
                b"default-jan-2014+head",
                b"default-apr-2019+head",
                b"default",
            ]);
            t.push(enc(Some(schema)));
            t.push(g.int(-1000, 100000).to_string());
            t.extend(env3(g));
            t.push(lut_setup(g, "w/FreeSurferColorLUT.txt"));
            if g.chance(3) {
                t.push("0".into());
            } else {
                t.push("1".into());
                let mut tb = table(g, true);
                // a few of the schemas' structure numbers
                if g.chance(2) {
                    tb[1] = "1000".into();
                    let k: usize = tb[4].parse().unwrap();
                    let extra = [3i64, 17, 75, 172, 258, 165, 914, 34, 120];
                    let m = g.int(1, 5) as usize;
                    let mut add = Vec::new();
                    let mut seen: Vec<i64> = (0..k)
                        .map(|i| tb[5 + 8 * i].parse::<i64>().unwrap())
                        .collect();
                    for _ in 0..m {
                        let s = g.pick(&extra);
                        if seen.contains(&s) {
                            continue;
                        }
                        seen.push(s);
                        add.extend([
                            s.to_string(),
                            enc(Some(b"s")),
                            "1".into(),
                            "2".into(),
                            "3".into(),
                            "255".into(),
                            "0".into(),
                            "0".into(),
                        ]);
                    }
                    tb[4] = (k + add.len() / 8).to_string();
                    tb.extend(add);
                }
                t.extend(tb);
            }
        }
        _ => {
            let a = table(g, true);
            let mut b = table(g, true);
            // mostly a merge table no larger than the table (FS-418 beyond)
            let (na, nb): (i64, i64) = (a[1].parse().unwrap(), b[1].parse().unwrap());
            if nb > na && !g.chance(6) {
                let k: usize = b[4].parse().unwrap();
                let mut keep = Vec::new();
                for i in 0..k {
                    let e = &b[5 + 8 * i..13 + 8 * i];
                    if e[0].parse::<i64>().unwrap() < na {
                        keep.extend_from_slice(e);
                    }
                }
                b.truncate(4);
                b[1] = na.to_string();
                b.push((keep.len() / 8).to_string());
                b.extend(keep);
            }
            t.extend(a);
            t.extend(b);
        }
    }
    t
}

// ---------------------------------------------------------------- the port's answers

struct Toks<'a> {
    t: &'a [&'a str],
    i: usize,
}

impl<'a> Toks<'a> {
    fn next(&mut self) -> &'a str {
        let s = self.t[self.i];
        self.i += 1;
        s
    }
    fn int(&mut self) -> i64 {
        self.next().parse().unwrap()
    }
    fn bytes(&mut self) -> Vec<u8> {
        dec(self.next()).unwrap_or_default()
    }
    fn string(&mut self) -> String {
        String::from_utf8(self.bytes()).unwrap()
    }
}

fn unit(i: i32) -> f32 {
    (f64::from(i as f32) / 255.0) as f32
}

fn read_table(tk: &mut Toks) -> ColorTable {
    let version = tk.int() as i32;
    let nentries = tk.int() as i32;
    let idbase = tk.int() as i32;
    let fname = tk.bytes();
    let mut ct = ColorTable {
        entries: vec![None; nentries.max(0) as usize],
        fname,
        version,
        idbase,
        ..ColorTable::default()
    };
    let k = tk.int();
    for _ in 0..k {
        let idx = tk.int() as usize;
        let name = tk.bytes();
        let (ri, gi, bi, ai) = (
            tk.int() as i32,
            tk.int() as i32,
            tk.int() as i32,
            tk.int() as i32,
        );
        let tissue_type = tk.int() as i32;
        let count = tk.int() as i32;
        ct.entries[idx] = Some(Box::new(ColorTableEntry {
            name,
            ri,
            gi,
            bi,
            ai,
            rf: unit(ri),
            gf: unit(gi),
            bf: unit(bi),
            af: unit(ai),
            tissue_type,
            count,
        }));
    }
    ct
}

fn of(o: &mut Out, f: f32) {
    o.x(&f.to_bits().to_be_bytes(), 4);
}

fn ostr(o: &mut Out, s: &[u8]) {
    let s = &s[..s.iter().position(|&c| c == 0).unwrap_or(s.len())];
    if s.is_empty() {
        o.0.push_str("x ");
    } else {
        o.0.push('x');
        o.bytes(s);
    }
}

fn print_table(o: &mut Out, ct: Option<&ColorTable>, alloc: bool, sub_alloc: bool) {
    let Some(ct) = ct else {
        o.0.push_str("N ");
        return;
    };
    o.i(i64::from(ct.nentries()));
    o.i(i64::from(ct.version));
    o.i(i64::from(ct.idbase));
    ostr(o, &ct.fname);
    ostr(o, ct.tissue_type_schema.as_bytes());
    for (i, e) in ct.entries.iter().enumerate() {
        let Some(e) = e else { continue };
        o.0.push_str(&format!("@{i} "));
        ostr(o, &e.name);
        o.i(i64::from(e.ri));
        o.i(i64::from(e.gi));
        o.i(i64::from(e.bi));
        o.i(i64::from(e.ai));
        of(o, e.rf);
        of(o, e.gf);
        of(o, e.bf);
        of(o, e.af);
        if !alloc {
            o.i(i64::from(e.tissue_type));
            o.i(i64::from(e.count));
        }
    }
    o.0.push_str("T ");
    print_table(o, ct.ctab_tissue_type.as_deref(), sub_alloc, false);
}

fn env3_of(tk: &mut Toks) -> Environment {
    let mut pairs = Vec::new();
    for nm in ["FREESURFER_HOME", "SUBJECTS_DIR", "FS_DESIRED_BVEC_SPACE"] {
        if let Some(v) = dec(tk.next()) {
            pairs.push((nm.to_string(), String::from_utf8(v).unwrap()));
        }
    }
    Environment::from_pairs(pairs)
}

fn ok_or(o: &mut Out, r: Result<i32, i32>) {
    match r {
        Ok(v) => {
            o.i(0);
            o.i(i64::from(v));
        }
        Err(e) => {
            o.i(i64::from(e));
            o.i(-7);
        }
    }
}

/// The port's answer to a piece-10 case (`Err`: the program's exit status).
pub fn run(
    tk: &[&str],
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    o: &mut Out,
    w: &Work,
    extra: &mut Vec<&'static str>,
) -> Result<(), u8> {
    let r = answer(tk, g, out, err, o, w);
    for n in g.notices.as_slice() {
        let id = n.defect.id;
        // FreeSurfer crashes, writes outside its tables or depends on the machine's memory
        let crash = matches!(
            id,
            "FS-413" | "FS-416" | "FS-417" | "FS-418" | "FS-420" | "FS-11"
        ) || (id == "FS-414"
            && (n.detail.contains("crashes") || n.detail.contains("outside")));
        if crash {
            extra.push("SKIP");
        }
        // FreeSurfer's values from memory never written
        if id == "FS-414" && n.detail.contains("never written") {
            extra.push("VARY");
        }
    }
    r
}

fn answer(
    tk: &[&str],
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    o: &mut Out,
    w: &Work,
) -> Result<(), u8> {
    let fatal = |f: larmorx_freesurfer_core::log::Fatal| f.status();
    let dir = Some(w.fix.as_path());
    let mut tk = Toks { t: tk, i: 1 };
    let empty = Environment::default();
    match tk.t[0] {
        "cra" => {
            let check = tk.int() != 0;
            setup(w, tk.next());
            let name = tk.string();
            let (ct, dups) = colortab::ctab_read_ascii2_counted(g, out, err, dir, &name, check);
            print_table(o, ct.as_ref(), false, false);
            o.i(i64::from(dups.unwrap_or(-7)));
        }
        "crt" => {
            setup(w, tk.next());
            let name = tk.string();
            let ct = colortab::ctab_read_ascii_tt_header(g, out, err, dir, &name);
            print_table(o, ct.as_ref(), false, false);
        }
        "ctd" => {
            let env = env3_of(&mut tk);
            setup(w, tk.next());
            let ct = colortab::ctab_read_default(g, &env, out, err, dir);
            print_table(o, ct.as_ref(), false, false);
        }
        "cal" => {
            random::set_random_seed(g, tk.int());
            let n = tk.int() as i32;
            let ct = colortab::ctab_alloc(g, &empty, out, err, n).map_err(fatal)?;
            print_table(o, ct.as_ref(), true, false);
        }
        "cun" => {
            random::set_random_seed(g, tk.int());
            let nmax = tk.int() as i32;
            let mut ct = read_table(&mut tk);
            let r = colortab::ctab_unique(g, &empty, out, err, &mut ct, nmax).map_err(fatal)?;
            o.i(i64::from(r));
            print_table(o, Some(&ct), false, false);
        }
        "ccr" => {
            let brk = tk.int() != 0;
            let ct = read_table(&mut tk);
            o.i(i64::from(colortab::ctab_count_repeats(g, &ct, brk)));
        }
        "cq" => {
            let ct = read_table(&mut tk);
            let nops = tk.int();
            for _ in 0..nops {
                match tk.next() {
                    "v" => {
                        o.i(0);
                        o.i(i64::from(colortab::ctab_get_number_of_valid_entries(&ct)));
                    }
                    "t" => {
                        o.i(0);
                        o.i(i64::from(colortab::ctab_get_number_of_total_entries(&ct)));
                    }
                    "e" => {
                        let i = tk.int() as i32;
                        ok_or(
                            o,
                            colortab::ctab_is_entry_valid(g, err, &ct, i).map(i32::from),
                        );
                    }
                    "h" => {
                        let i = tk.int() as i32;
                        o.i(i64::from(colortab::ctab_has_entry(Some(&ct), i)));
                    }
                    "r" => {
                        let i = tk.int() as i32;
                        match colortab::ctab_rgb_at_indexi(g, err, &ct, i) {
                            Ok((r, gg, b)) => {
                                for v in [0, r, gg, b] {
                                    o.i(i64::from(v));
                                }
                            }
                            Err(e) => {
                                for v in [e, -7, -7, -7] {
                                    o.i(i64::from(v));
                                }
                            }
                        }
                    }
                    "n" => {
                        let i = tk.int() as i32;
                        let len = tk.int() as usize;
                        match colortab::ctab_copy_name(g, err, &ct, i, len) {
                            Ok(nm) => {
                                o.i(0);
                                ostr(o, &nm);
                            }
                            Err(e) => {
                                o.i(i64::from(e));
                                // the buffer untouched (its 'Z' bytes up to the length)
                                ostr(o, &vec![b'Z'; len.min(4096)]);
                            }
                        }
                    }
                    "a" => {
                        let (r, gg, b) = (tk.int() as i32, tk.int() as i32, tk.int() as i32);
                        o.i(i64::from(colortab::ctab_rgb2_annotation(r, gg, b)));
                    }
                    "x" => {
                        let nm = tk.bytes();
                        o.i(i64::from(colortab::ctab_entry_name_to_index(g, &nm, &ct)));
                    }
                    "y" => {
                        let nm = tk.bytes();
                        o.i(i64::from(colortab::ctab_entry_name_to_annotation(
                            g, &nm, &ct,
                        )));
                    }
                    "i" => {
                        let i = tk.int() as i32;
                        ok_or(o, colortab::ctab_annotation_at_index(g, err, &ct, i));
                    }
                    "f" => {
                        let a = tk.int() as i32;
                        ok_or(o, Ok(colortab::ctab_find_annotation(&ct, a)));
                    }
                    "F" => {
                        let a = tk.int() as i32;
                        ok_or(o, Ok(colortab::ctab_find_index_from_annotation(&ct, a)));
                    }
                    "g" => {
                        let (r, gg, b) = (tk.int() as i32, tk.int() as i32, tk.int() as i32);
                        ok_or(o, colortab::ctab_find_rgbi(g, err, &ct, r, gg, b));
                    }
                    "m" => {
                        let nm = tk.bytes();
                        ok_or(o, Ok(colortab::ctab_find_name(g, &ct, &nm)));
                    }
                    _ => {
                        o.i(i64::from(colortab::ctab_find_duplicate_names(out, &ct)));
                    }
                }
            }
        }
        "cpw" => {
            g.gdiag = tk.int() as u64;
            setup(w, tk.next());
            let ct = read_table(&mut tk);
            let name = tk.string();
            let r = colortab::ctab_write_file_ascii(g, out, err, dir, &ct, &name);
            o.i(i64::from(r));
            let b = std::fs::read(w.fix.join(&name)).unwrap_or_default();
            o.bytes(&b);
        }
        "cps" => {
            g.gdiag = tk.int() as u64;
            let ct = read_table(&mut tk);
            o.i(i64::from(colortab::ctab_print_ascii_stdout(g, out, &ct)));
        }
        "cts" => {
            let schema = tk.string();
            random::set_random_seed(g, tk.int());
            let env = env3_of(&mut tk);
            setup(w, tk.next());
            let ct = if tk.int() != 0 {
                Some(read_table(&mut tk))
            } else {
                None
            };
            let r =
                colortab::tissue_type_schema(g, &env, out, err, dir, ct, &schema).map_err(fatal)?;
            print_table(o, r.as_ref(), false, true);
        }
        _ => {
            let mut ct = read_table(&mut tk);
            let m = read_table(&mut tk);
            o.i(i64::from(colortab::ctab_merge(g, &mut ct, &m)));
            print_table(o, Some(&ct), false, false);
        }
    }
    o.i(i64::from(g.gerror));
    let _ = hex;
    Ok(())
}
