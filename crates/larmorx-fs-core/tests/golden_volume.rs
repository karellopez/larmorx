// SPDX-License-Identifier: Apache-2.0
//! Golden cases of fs.volume's clean room (`specs/freesurfer-volume.md` §69.5, CLAUDE.md rule
//! 10): every command of the black-box probe's grammar (`tests/volume_cmd/`, the comparison's
//! candidate commands) on seeded arguments, in both defect modes, one SHA-256 per group of the
//! spec's parts 1 to 3 over every result line (the printed values, both streams and the notices;
//! NaN bits included), recorded on Linux x86-64 and checked on every platform. The arguments
//! come from a seeded generator and plain arithmetic (special values written as their bits); no
//! file system, no clock; volumes of at most 3 x 3 x 3 voxels and 2 frames.
//!
//! Record: `FS_CORE_GOLDEN_RECORD=1 cargo test -p larmorx-fs-core --test golden_volume --
//! --nocapture` prints `GOLDEN <case> <sha256>` for each case instead of checking it
//! (`FS_CORE_GOLDEN_DUMP=<dir>` also writes each case's lines). The cases also run under the
//! AArch64 NaN emulation (`validation/investigations/freesurfer-cleanroom-volume/README.md`).

mod volume_cmd;

use larmorx_core::defects::UpstreamDefects;
use larmorx_fs_core::Environment;
use sha2::{Digest, Sha256};

const GOLDEN: &[(&str, &str)] = &[
    (
        "block.fix",
        "35aadda73ae86414bdcfb64f401ea0c0ad1dc69f237b5775780e4a9c801e2900",
    ),
    (
        "block.reproduce",
        "6c2d904579863f989c9428fee242f48a44e910d790c93f77d892323dc01d7ab0",
    ),
    (
        "coords.fix",
        "28205a5a4461d446ecf3d7b0883565aa8df60dc994c3d5d44cce8dabfb8a87f1",
    ),
    (
        "coords.reproduce",
        "54d3f536d4c5f46ebb7dc7b20853ad1d2950a9607e9fd6ce91585100ab7aa1aa",
    ),
    (
        "extract.fix",
        "a884109340d1a171c9132a4f37063c5753ad07d1501f05158cf7a61bc8b82a74",
    ),
    (
        "extract.reproduce",
        "83e5c14ee3b42e6cb829edf6f9bcc4438c43af3da562e94dd870a8c31e25df8d",
    ),
    (
        "header.fix",
        "3ad0f620b75f9e0aea948bcb705fa32d30b349c0c369f8df38c3edde8e1f1cc2",
    ),
    (
        "header.reproduce",
        "eff5c60c6afce0c9de814c3eff622b7c813d358bc5fe1ac826fcaff498b29a27",
    ),
    (
        "make.fix",
        "519f882b37c16080e5093bf34215636cb699ff378f32132fbc0a7e9dcefc1a6d",
    ),
    (
        "make.reproduce",
        "062a41326d5cd141784149f7ec16a87a6c7af9ccd170f2cf30ddb6a1bd89124a",
    ),
    (
        "range.fix",
        "4a91354714005a7724833f94b7c54367f3450951a45fc0ef6c3a98309109dd1a",
    ),
    (
        "range.reproduce",
        "c31f9b9c01131e00264322bfb1cd240669e0937061a95d873c28e9efe49edc81",
    ),
    (
        "scale.fix",
        "e1f06d371b6767026d91f38f6dc581e483fc618bd7e96b90f912474e99a78fbc",
    ),
    (
        "scale.reproduce",
        "d21bf3e8b13dda0669abfda0a6b8f9913ac33a2e32abfae343838e97d471c67e",
    ),
    (
        "stats.fix",
        "982996ae411e7d2aa66ab0a943a5254de92c1daf3361ffc59a70e28c651cfcc8",
    ),
    (
        "stats.reproduce",
        "1356dd06a64140496062c84e42cfebee003f413084f140e449d3258523a79574",
    ),
    (
        "two.fix",
        "83b17f2eaf9eccf758fadd7d83e539286614900439569b1d84b20dac3ba7e554",
    ),
    (
        "two.reproduce",
        "0e9b798b8a1c4102528fc5ccf830a9506d1f33dc80057ce8eb745c2ce28f864d",
    ),
    (
        "vgcopy.fix",
        "1477d2565a910423a35da424178294939e54346c0de01c3f2ad2dac283cec951",
    ),
    (
        "vgcopy.reproduce",
        "1477d2565a910423a35da424178294939e54346c0de01c3f2ad2dac283cec951",
    ),
];

fn check(name: &str, bytes: &[u8]) {
    if let Some(dir) = std::env::var_os("FS_CORE_GOLDEN_DUMP") {
        let _ = std::fs::write(
            std::path::Path::new(&dir).join(format!("{name}.txt")),
            bytes,
        );
    }
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

/// splitmix64: the cases' seeded arguments.
struct G(u64);

impl G {
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

    /// An integer in `lo..=hi`.
    fn int(&mut self, lo: i64, hi: i64) -> i64 {
        lo + self.below((hi - lo + 1) as u64) as i64
    }

    fn one_in(&mut self, n: u64) -> bool {
        self.below(n) == 0
    }

    fn pick<'a>(&mut self, a: &[&'a str]) -> &'a str {
        a[self.below(a.len() as u64) as usize]
    }

    /// A float's bits: special values (with `special`), small integers, ratios.
    fn fbits(&mut self, special: bool) -> u32 {
        if special && self.one_in(8) {
            const S: [u32; 9] = [
                0x0000_0000,
                0x8000_0000,
                0x7f80_0000,
                0xff80_0000,
                0x7fc0_0000,
                0xffc1_2345,
                0x7f80_0001,
                0x7f7f_ffff,
                0x0000_0001,
            ];
            return S[self.below(9) as usize];
        }
        let v = match self.below(4) {
            0 => self.int(-8, 8) as f32,
            1 => self.int(-64, 64) as f32 / 8.0,
            2 => self.int(-1000, 1000) as f32 / 7.0,
            _ => self.int(0, 300) as f32,
        };
        v.to_bits()
    }

    fn f(&mut self, special: bool) -> String {
        format!("{:08x}", self.fbits(special))
    }

    /// A double's bits (a float's value widened, or a special value).
    fn d(&mut self, special: bool) -> String {
        if special && self.one_in(8) {
            const S: [u64; 5] = [
                0x8000_0000_0000_0000,
                0x7ff0_0000_0000_0000,
                0xfff8_0000_0000_0000,
                0x41e0_0000_0000_0000,
                0x3fe0_0000_0000_0000,
            ];
            return format!("{:016x}", S[self.below(5) as usize]);
        }
        let v = f64::from(f32::from_bits(self.fbits(false)));
        format!("{:016x}", v.to_bits())
    }

    /// A voxel size: positive mostly, sometimes 0 or a special value.
    fn size(&mut self, special: bool) -> String {
        if special && self.one_in(10) {
            return self.pick(&["00000000", "7fc00000", "bf800000"]).to_owned();
        }
        self.pick(&[
            "3f800000", "3f000000", "40000000", "3fc00000", "40400000", "3f700000",
        ])
        .to_owned()
    }

    /// A string: empty, a name, or one byte.
    fn x(&mut self) -> String {
        self.pick(&["x", "x6f7269672e6d677a", "x612062", "x41"])
            .to_owned()
    }

    /// The direction cosines: a permutation with signs, or seeded ratios.
    fn cosines(&mut self, special: bool) -> Vec<String> {
        if self.one_in(2) {
            let perm = [
                [0, 1, 2],
                [1, 0, 2],
                [2, 0, 1],
                [0, 2, 1],
                [1, 2, 0],
                [2, 1, 0],
            ][self.below(6) as usize];
            let mut v = vec!["00000000".to_owned(); 9];
            for (j, &k) in perm.iter().enumerate() {
                v[3 * j + k] = self.pick(&["3f800000", "bf800000"]).to_owned();
            }
            v
        } else {
            (0..9).map(|_| self.f(special)).collect()
        }
    }

    /// The geometry's fields but valid, the extent and the name.
    fn gfields(&mut self, special: bool) -> String {
        let mut v = vec![self.size(special), self.size(special), self.size(special)];
        v.push(self.int(0, 1).to_string());
        v.extend(self.cosines(special));
        v.extend((0..3).map(|_| self.f(special)));
        let s = if self.one_in(5) {
            [self.f(false), self.f(false), self.f(false)]
        } else {
            [
                "00000000".to_owned(),
                "00000000".to_owned(),
                "00000000".to_owned(),
            ]
        };
        v.extend(s);
        v.join(" ")
    }

    /// G: valid, extent, the fields, a name.
    fn geom(&mut self, special: bool) -> String {
        let e: Vec<String> = (0..3).map(|_| self.int(1, 4).to_string()).collect();
        format!(
            "{} {} {} {}",
            self.int(0, 1),
            e.join(" "),
            self.gfields(special),
            self.x()
        )
    }

    /// V: extent, type, frames, header-only.
    fn shape(&mut self, types: &[i32]) -> String {
        let t = types[self.below(types.len() as u64) as usize];
        let sized = !matches!(t, 5 | 8);
        let hdr = if sized && !self.one_in(12) { 0 } else { 1 };
        format!(
            "{} {} {} {t} {} {hdr}",
            self.int(1, 3),
            self.int(1, 3),
            self.int(1, 3),
            self.int(0, 2)
        )
    }

    /// W of a given shape: V, a fill, the geometry's fields, the header fields, the cache, the
    /// registration matrix, the colour table.
    fn w_of(&mut self, shape: &str) -> String {
        let t: i32 = shape
            .split(' ')
            .nth(3)
            .and_then(|x| x.parse().ok())
            .unwrap_or(0);
        let fill = if t == 3 && !self.one_in(4) {
            format!("t{}", self.int(0, 40))
        } else if self.one_in(5) {
            "-1".to_owned()
        } else {
            self.int(0, 255).to_string()
        };
        let mut h = vec![self.int(-1, 3).to_string(), self.int(-1, 4).to_string()];
        h.extend((0..11).map(|_| self.f(true)));
        h.push(self.d(true));
        h.push(self.int(0, 5).to_string());
        h.push(self.f(false));
        h.push(self.f(true));
        h.push(self.int(0, 2).to_string());
        let cache = self.pick(&["0", "1", "1", "2"]);
        let reg = if self.one_in(6) {
            let v: Vec<String> = (0..16).map(|_| self.f(false)).collect();
            format!("4 4 {}", v.join(" "))
        } else {
            "null".to_owned()
        };
        let ct = if self.one_in(6) {
            format!(
                "1 x 1 0 x 1 x41 {} {} {} {} 3f000000 3f000000 3f000000 3f800000 1 {} null",
                self.int(0, 255),
                self.int(0, 255),
                self.int(0, 255),
                self.int(0, 255),
                self.int(0, 9)
            )
        } else {
            "null".to_owned()
        };
        format!(
            "{shape} {fill} {} {} {cache} {reg} {ct}",
            self.gfields(true),
            h.join(" ")
        )
    }

    fn w(&mut self, types: &[i32]) -> String {
        let s = self.shape(types);
        self.w_of(&s)
    }

    /// D: new, self, other (two-volume commands), a W of the same shape or another.
    fn dst(&mut self, shape_of: &str, two: bool) -> String {
        let shape: Vec<&str> = shape_of.split(' ').take(6).collect();
        match self.below(if two { 5 } else { 4 }) {
            0 => "new".to_owned(),
            1 => "self".to_owned(),
            2 => {
                let s = shape.join(" ");
                self.w_of(&s)
            }
            3 => self.w(ALL),
            _ => "other".to_owned(),
        }
    }
}

const ALL: &[i32] = &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
const NUMERIC: &[i32] = &[0, 1, 2, 3, 4, 10];

type Make = fn(&mut G) -> String;

/// A block of the volume-geometry text, perturbed.
fn block(g: &mut G) -> String {
    let mut lines = vec![
        format!("valid = {}  # volume info valid", g.int(-1, 2)),
        format!("filename = {}", g.pick(&["orig.mgz", "", "a b"])),
        format!(
            "volume = {} {} {}",
            g.int(1, 300),
            g.int(1, 300),
            g.int(1, 300)
        ),
        "voxelsize = 1.000000000000000e+00 5.000000000000000e-01 1.250000000000000e+00".to_owned(),
        "xras   = -1.000000000000000e+00 0.000000000000000e+00 0.000000000000000e+00".to_owned(),
        "yras   = 0.000000000000000e+00 0.000000000000000e+00 -1.000000000000000e+00".to_owned(),
        "zras   = 0.000000000000000e+00 1.000000000000000e+00 0.000000000000000e+00".to_owned(),
        "cras   = 1.5e1 -2.25e0 nan".to_owned(),
    ];
    for _ in 0..g.int(0, 3) {
        let k = g.below(lines.len() as u64 + 1) as usize;
        match g.below(8) {
            0 if !lines.is_empty() => {
                lines.remove(k.min(lines.len() - 1));
            }
            1 => lines.insert(k, g.pick(&["", "   ", "\t"]).to_owned()),
            2 => lines.insert(
                k,
                g.pick(&["foo = 1", "valid", "cras 1e40 2 3", "volume ="])
                    .to_owned(),
            ),
            3 if !lines.is_empty() => {
                let j = k.min(lines.len() - 1);
                lines[j] = lines[j].replacen(" = ", " == ", 1);
            }
            4 if !lines.is_empty() => {
                let j = k.min(lines.len() - 1);
                lines[j] = format!("{} {}", "k".repeat(70), lines[j]);
            }
            5 if !lines.is_empty() => {
                let j = k.min(lines.len() - 1);
                lines[j].push_str(&" y".repeat(600));
            }
            6 => lines.insert(k, "xras = 0x1p3 -inf 1e-50".to_owned()),
            _ => lines.insert(k, "valid = 1".to_owned()),
        }
    }
    let end = g.pick(&["\n", "\r\n", "\n"]);
    let mut text = lines.join(end);
    if !g.one_in(6) {
        text.push_str(end);
    }
    if g.one_in(3) {
        text.push_str("trailing\n");
    }
    let hex: String = text.bytes().map(|b| format!("{b:02x}")).collect();
    format!("x{hex} {}", g.geom(false))
}

const BLOCK: &[(&str, Make)] = &[("vgwrite", |g| g.geom(true)), ("vgread", block)];

const VGCOPY: &[(&str, Make)] = &[
    ("getvg", |g| format!("{} {}", g.geom(true), g.geom(true))),
    ("copyvg", |g| format!("{} {}", g.geom(true), g.geom(true))),
    ("vgi2r", |g| g.geom(true)),
    ("vgr2i", |g| g.geom(true)),
    ("tkrv2r", |g| g.geom(true)),
    ("vgprint", |g| g.geom(true)),
    ("vgeq", |g| {
        let a = g.geom(true);
        let b = if g.one_in(2) { a.clone() } else { g.geom(true) };
        format!("{a} {b} {}", g.d(true))
    }),
];

const MAKE: &[(&str, Make)] = &[
    ("clone", |g| {
        let a = g.w(ALL);
        let d = g.dst(&a, false);
        format!("{a} {d}")
    }),
    ("clonesp", |g| {
        format!("{} {} {}", g.w(ALL), g.int(-1, 3), g.int(-1, 2))
    }),
    ("clear", |g| g.w(ALL)),
    ("match", |g| format!("{} {}", g.w(ALL), g.w(ALL))),
    ("chsize", |g| {
        let b = if g.one_in(4) {
            "null".to_owned()
        } else {
            g.w(ALL)
        };
        format!(
            "{} {b} {} {} {}",
            g.w(ALL),
            g.int(0, 3),
            g.int(0, 3),
            g.int(0, 3)
        )
    }),
    ("cpframe", |g| {
        let a = g.w(ALL);
        let d = g.dst(&a, false);
        format!("{a} {d} {} {}", g.int(-1, 2), g.int(-1, 2))
    }),
    ("cpframes", |g| {
        let a = g.w(ALL);
        let d = g.dst(&a, false);
        format!("{a} {d} {} {} {}", g.int(0, 1), g.int(-1, 2), g.int(0, 1))
    }),
];

const RANGE: &[(&str, Make)] = &[
    ("vrange", |g| g.w(ALL)),
    ("vrangef", |g| format!("{} {}", g.w(ALL), g.int(0, 2))),
    ("vrangereg", |g| {
        let r: Vec<String> = (0..6).map(|_| g.int(-1, 3).to_string()).collect();
        format!("{} {}", g.w(ALL), r.join(" "))
    }),
    ("lrange", |g| {
        let a = g.w(ALL);
        let shape: Vec<&str> = a.split(' ').take(6).collect();
        let l = g.w_of(&shape.join(" "));
        format!("{a} {l} {}", g.int(0, 255))
    }),
];

fn one(cmd: &'static str) -> Make {
    match cmd {
        "linscale" => |g| {
            let a = g.w(ALL);
            let d = g.dst(&a, false);
            format!("{a} {d} {} {} {}", g.f(true), g.f(true), g.int(0, 1))
        },
        "smulf" => |g| {
            let a = g.w(ALL);
            let d = g.dst(&a, false);
            format!("{a} {d} {} {}", g.f(true), g.int(0, 2))
        },
        "binz" => |g| {
            let a = g.w(ALL);
            let d = g.dst(&a, false);
            format!("{a} {d} {} {} {}", g.f(true), g.f(true), g.f(true))
        },
        "sign" => |g| {
            let a = g.w(ALL);
            let d = g.dst(&a, false);
            format!("{a} {d}")
        },
        _ => |g| {
            let a = g.w(ALL);
            let d = g.dst(&a, false);
            format!("{a} {d} {}", g.f(true))
        },
    }
}

fn scale_table() -> Vec<(&'static str, Make)> {
    vec![
        ("linscale", one("linscale")),
        ("smul", one("smul")),
        ("smulf", one("smulf")),
        ("thresh", one("thresh")),
        ("uthresh", one("uthresh")),
        ("invc", one("invc")),
        ("binz", one("binz")),
        ("abs", one("sign")),
        ("pos", one("sign")),
        ("neg", one("sign")),
    ]
}

fn two(g: &mut G) -> String {
    let a = g.w(&[0, 1, 2, 3, 4, 9, 10]);
    let shape: Vec<String> = a.split(' ').take(6).map(str::to_owned).collect();
    let b = if g.one_in(2) {
        g.w_of(&shape.join(" "))
    } else {
        let mut s = shape.clone();
        s[3] = NUMERIC[g.below(NUMERIC.len() as u64) as usize].to_string();
        g.w_of(&s.join(" "))
    };
    let d = g.dst(&a, true);
    format!("{a} {b} {d}")
}

const TWO: &[(&str, Make)] = &[
    ("mul2", two),
    ("sub2", two),
    ("add2", two),
    ("div2", two),
    ("sam", |g| {
        let a = g.w(&[0, 0, 0, 1]);
        let s: Vec<String> = a.split(' ').take(6).map(str::to_owned).collect();
        let b = g.w_of(&s.join(" "));
        let d = g.dst(&a, true);
        format!("{a} {} {b} {d}", g.f(true))
    }),
];

fn boxes(g: &mut G, into: bool) -> String {
    let mut b: Vec<String> = (0..3).map(|_| g.int(-1, 3).to_string()).collect();
    b.extend((0..3).map(|_| g.int(-1, 3).to_string()));
    if into {
        b.extend((0..3).map(|_| g.int(0, 1).to_string()));
    }
    b.join(" ")
}

const EXTRACT: &[(&str, Make)] = &[
    ("extract", |g| {
        let a = g.w(ALL);
        let d = g.dst(&a, false).replace("self", "new");
        format!("{a} {d} {}", boxes(g, false))
    }),
    ("extinto", |g| {
        let a = g.w(ALL);
        let d = g.dst(&a, false).replace("self", "new");
        format!("{a} {d} {}", boxes(g, true))
    }),
    ("extreg", |g| {
        let a = g.w(ALL);
        let d = g.dst(&a, false).replace("self", "new");
        let r = if g.one_in(4) {
            "null".to_owned()
        } else {
            boxes(g, false)
        };
        format!("{a} {d} {r}")
    }),
    ("cras", |g| {
        let c: Vec<String> = (0..6).map(|_| g.int(-1, 3).to_string()).collect();
        format!("{} {} {}", g.w(ALL), g.w(ALL), c.join(" "))
    }),
];

const COORDS: &[(&str, Make)] = &[
    ("sras", |g| format!("{} {}", g.w(ALL), g.int(0, 3))),
    ("spt", |g| {
        format!(
            "{} {} {} {} {}",
            g.w(ALL),
            g.int(0, 2),
            g.d(true),
            g.d(true),
            g.d(true)
        )
    }),
    ("ostr", |g| g.geom(true)),
    ("vtal", |g| {
        let x = if g.one_in(3) {
            "none".to_owned()
        } else {
            let m: Vec<String> = (0..32).map(|_| g.d(true)).collect();
            format!("0 {} {}", g.int(0, 1), m.join(" "))
        };
        format!("{} {x} {} {} {}", g.w(ALL), g.d(true), g.d(true), g.d(true))
    }),
];

const STATS: &[(&str, Make)] = &[
    ("inidx", |g| {
        format!("{} {} {} {}", g.w(ALL), g.d(true), g.d(true), g.d(true))
    }),
    ("snear", |g| {
        format!(
            "{} {} {} {} {}",
            g.w(ALL),
            g.d(true),
            g.d(true),
            g.d(true),
            g.d(false)
        )
    }),
    ("com", |g| {
        format!("{} {}", g.w(ALL), g.pick(&["0", "10", "100", "255"]))
    }),
    ("pca", |g| {
        format!("{} {}", g.w(&[0, 1, 3, 4, 10]), g.pick(&["0", "10", "100"]))
    }),
    ("avg", |g| {
        let a = g.w(ALL);
        let dof = g.pick(&["0", "1", "2", "-1", "2147483647"]);
        let d = g.dst(&a, false);
        format!("{a} {dof} {d}")
    }),
];

const HEADER: &[(&str, Make)] = &[
    ("cmph", |g| {
        let a = g.w(ALL);
        let b = match g.below(3) {
            0 => "null".to_owned(),
            1 => "self".to_owned(),
            _ => g.w(ALL),
        };
        format!("{a} {b}")
    }),
    ("vdet", |g| g.w(ALL)),
    ("dump", |g| g.w(ALL)),
    ("dsign", |g| g.d(true)),
];

fn group(name: &str, table: &[(&str, Make)], per: usize) {
    let seed = name.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3)
    });
    let env = Environment::new();
    for (mode, tag) in [
        (UpstreamDefects::Fix, "fix"),
        (UpstreamDefects::Reproduce, "reproduce"),
    ] {
        let mut g = G(seed);
        let mut all = Vec::new();
        for (cmd, make) in table {
            for _ in 0..per {
                let args = make(&mut g);
                let line = format!("{cmd} {args}");
                let toks: Vec<String> = line.split_whitespace().map(str::to_owned).collect();
                let res = volume_cmd::run_line(&toks, mode, &env);
                all.extend_from_slice(line.as_bytes());
                all.push(b'\n');
                all.extend_from_slice(res.as_bytes());
                all.push(b'\n');
            }
        }
        check(&format!("{name}.{tag}"), &all);
    }
}

#[test]
fn geometry_block() {
    group("block", BLOCK, 16);
}

#[test]
fn geometry_copies() {
    group("vgcopy", VGCOPY, 6);
}

#[test]
fn clones_and_frames() {
    group("make", MAKE, 8);
}

#[test]
fn value_ranges() {
    group("range", RANGE, 8);
}

#[test]
fn scaling() {
    group("scale", &scale_table(), 6);
}

#[test]
fn two_volumes() {
    group("two", TWO, 8);
}

#[test]
fn extracting() {
    group("extract", EXTRACT, 8);
}

#[test]
fn coordinates() {
    group("coords", COORDS, 8);
}

#[test]
fn statistics() {
    group("stats", STATS, 8);
}

#[test]
fn headers() {
    group("header", HEADER, 8);
}
