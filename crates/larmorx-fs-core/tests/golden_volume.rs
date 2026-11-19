// SPDX-License-Identifier: Apache-2.0
//! Golden cases of fs.volume's clean room (`specs/freesurfer-volume.md` §69.5, CLAUDE.md rule
//! 10): every command of the black-box probe's grammar (`tests/volume_cmd/`, the comparison's
//! candidate commands) on seeded arguments, in both defect modes, one SHA-256 per group of the
//! spec's parts 1 to 7a and of fs.bspline (`specs/freesurfer-bspline.md`) over every result line (the printed values, both streams and the notices;
//! NaN bits included), recorded on Linux x86-64 and checked on every platform. The arguments
//! come from a seeded generator and plain arithmetic (special values written as their bits); no
//! file system (the transform reader reads an in-memory file, `ltamem`), no clock; volumes of at
//! most 3 x 3 x 3 voxels and 2 frames, but the head's box (24 voxels a side at most).
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
        "deriv.fix",
        "a99b44b74602adbc03238a12d81d5afce4e98b525c0aebcd610ec9f23117f9a6",
    ),
    (
        "deriv.reproduce",
        "bf1cf71578e53b728a50500e96e21e1a31fdd796dd388e83215c6905007b7797",
    ),
    (
        "frames.fix",
        "ed14ad568160972ccbaffda334ba1e919f2ec2bf858b09597e53b84d855c882e",
    ),
    (
        "frames.reproduce",
        "b9c485a414a6e65737f9f462ce759ea094858d4f2f3322bad3d2811507087ecf",
    ),
    (
        "linear.fix",
        "701a77a3c4c0d18f7d393c7c6caf880f70969824df3c040ca4c5619088e20aba",
    ),
    (
        "linear.reproduce",
        "358a58730f2df6d34979e10e86eb9f1a6b3c1688d15ed086ce3fb70873d9c177",
    ),
    (
        "regrid.fix",
        "60e8e3db564123d07a3706284455a17b56ce1a1cb00d4e6a7229062b65e2d735",
    ),
    (
        "regrid.reproduce",
        "75fb32a32bf8c70a1696b167bcf38bff80e40a69d9b54edd654edfc11c1b8385",
    ),
    (
        "block.fix",
        "35aadda73ae86414bdcfb64f401ea0c0ad1dc69f237b5775780e4a9c801e2900",
    ),
    (
        "block.reproduce",
        "6c2d904579863f989c9428fee242f48a44e910d790c93f77d892323dc01d7ab0",
    ),
    (
        "cma.fix",
        "8b10060df6129811291bb4b3e9bcab672ed671239ee000504bb2de2d1304daa2",
    ),
    (
        "cma.reproduce",
        "5683e823ad6a3a7ad46e925cab65adf8ad60111af82286e0ff571d4c41533398",
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
        "ltaread.fix",
        "ff61b9869057a88f31c13d45e7523918a44c82d9afc3814091d46aef13fd4c87",
    ),
    (
        "ltaread.reproduce",
        "a2616d5e69633d1aeaa4ddf282dc50c0b74d8ed9af1f175ef2cf44dec5cdc2b6",
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
        "move.fix",
        "80457d5bbbc21515c2cf58f51654dae60269bd9d478fa1bdb818aa7770bcb94d",
    ),
    (
        "move.reproduce",
        "ae3849bcc8e1c35773bde009745adc2361dac1e42b2a438aed656936b5b9f16f",
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
        "res.fix",
        "f4542843b50dab60cdfa52d3bc62b13cf2106aa47ced29aa4d09ddb3043c291d",
    ),
    (
        "res.reproduce",
        "c91238ecbb64485373e1f49a2ecccdfb528fd6b44e71afbe2c50f87aef3085c2",
    ),
    (
        "sample.fix",
        "646f2067c3589ba8fc2b39e328bc405b56d222e8cfd5cb7bd7ee79fecbbc6380",
    ),
    (
        "sample.reproduce",
        "db299f895480701cb0e4ffbf4e28d9bc0ec2b2df076e5f34d1c72f0d253d144f",
    ),
    (
        "scale.fix",
        "e1f06d371b6767026d91f38f6dc581e483fc618bd7e96b90f912474e99a78fbc",
    ),
    (
        "scale.reproduce",
        "63721f5a5c7fe99cc1effc87c1cf206ea27be735b53d9e04495b2f31a7412278",
    ),
    (
        "skull.fix",
        "8506eb23d287baef3bff5788cc511816760fff645c16ecd778d40faa0a234080",
    ),
    (
        "skull.reproduce",
        "133c3862398bc90e0a67fec0af190af948a53345e816c7779bfb44e7ff713f5a",
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
    (
        "bspline.fix",
        "95378741724e25165a8edb8e8dc4da3b6df5cf5b35d6e4a61660d4bbd01dccaf",
    ),
    (
        "bspline.reproduce",
        "547f266687ddf96763c8170ff9e8e126fb43a477696002fa1b0f2763498756ae",
    ),
    (
        "bsvol.fix",
        "7661c9e083d2fb0916c1a97211458cb336f60c9b6cb4e6a7a04ab46563ade7a2",
    ),
    (
        "bsvol.reproduce",
        "6e5203f12add523a75f1503dcd19fd625c5b4c1f5b70e827fcd92e39b77fc8d5",
    ),
    (
        "info.fix",
        "d65d8cc2f282a4b212edac697402b1c7d064e6ab49505ef635709104f6682022",
    ),
    (
        "info.reproduce",
        "817b8b62fe02a52fdb072204841ff3646f11b24b8e301e2d0416e236496b640e",
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

impl G {
    /// A sampling coordinate (§18): whole, fractional, within `FLT_EPSILON` of whole, past the
    /// edges, special, beyond the `int` range.
    fn coord(&mut self) -> String {
        let v: f64 = match self.below(6) {
            0 => self.int(-1, 4) as f64,
            1 => self.int(-2, 8) as f64 / 2.0 + 0.25,
            2 => {
                const E: [f64; 4] = [1e-8, -1e-8, 1.192_092_895_507_812_5e-7, 2e-7];
                self.int(0, 3) as f64 + E[self.below(4) as usize]
            }
            3 => self.int(-16, 48) as f64 / 7.0,
            4 => return self.d(true),
            _ => [1e10, -2_147_483_648.0, 2_147_483_647.6, -0.5][self.below(4) as usize],
        };
        format!("{:016x}", v.to_bits())
    }

    /// A 3 x 3 matrix (M) of seeded ratios, a permutation with signs, or a singular one.
    fn mat3(&mut self) -> String {
        match self.below(4) {
            0 => {
                let c = self.cosines(false);
                format!("3 3 {}", c.join(" "))
            }
            1 => "3 3 3f800000 40000000 40400000 40000000 40800000 40c00000 00000000 00000000 3f800000"
                .to_owned(),
            _ => {
                let v: Vec<String> = (0..9)
                    .map(|_| {
                        let special = self.one_in(3);
                        self.f(special)
                    })
                    .collect();
                format!("3 3 {}", v.join(" "))
            }
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

/// `samp <k> W <outside> n (x y z frame aux)×n` (§18).
fn samp(g: &mut G, k: &str) -> String {
    let types = if g.one_in(6) { ALL } else { NUMERIC };
    let a = g.w(types);
    let n = g.int(1, 3);
    let mut pts = Vec::new();
    for _ in 0..n {
        let aux = match k {
            "typ" | "fty" => g.pick(&["0", "1", "1", "2", "3", "5", "-1", "7"]),
            "sinc" | "sincf" => g.pick(&["0", "1", "2", "3", "5", "-1", "70"]),
            _ => "0",
        };
        let (x, y, z) = (g.coord(), g.coord(), g.coord());
        pts.push(format!("{x} {y} {z} {} {aux}", g.int(0, 3)));
    }
    format!("{k} {a} {} {n} {}", g.d(true), pts.join(" "))
}

const SAMPLE: &[(&str, Make)] = &[
    ("samp", |g| samp(g, "tri")),
    ("samp", |g| samp(g, "typ")),
    ("samp", |g| samp(g, "frm")),
    ("samp", |g| samp(g, "fty")),
    ("samp", |g| samp(g, "cub")),
    ("samp", |g| samp(g, "cubf")),
    ("samp", |g| samp(g, "sinc")),
    ("samp", |g| samp(g, "sincf")),
    ("coef", |g| {
        let n = g.int(1, 4);
        let v: Vec<String> = (0..n)
            .map(|_| format!("{} {}", g.d(true), g.int(-1, 4)))
            .collect();
        format!("{n} {}", v.join(" "))
    }),
    ("hsinc", |g| {
        let n = g.int(1, 4);
        let v: Vec<String> = (0..n)
            .map(|_| {
                let fw = g.pick(&["4024000000000000", "4000000000000000", "0000000000000000"]);
                format!("{} {fw}", g.d(true))
            })
            .collect();
        format!("{n} {}", v.join(" "))
    }),
];

const MOVE: &[(&str, Make)] = &[
    ("trans", |g| {
        let a = g.w(&[0, 1, 3, 4, 6, 9, 10]);
        let d = g.dst(&a, false);
        format!("{a} {d} {} {} {}", g.d(true), g.d(false), g.d(false))
    }),
    ("rot", |g| {
        let a = g.w(&[0, 1, 3, 4, 6, 9, 10]);
        let d = g.dst(&a, false);
        let r = match g.below(10) {
            0 => "2 2 3f800000 00000000 00000000 3f800000".to_owned(),
            1 => "4 4 3f800000 0 0 0 0 3f800000 0 0 0 0 3f800000 0 0 0 0 3f800000".to_owned(),
            _ => g.mat3(),
        };
        let o = match g.below(6) {
            0..=2 => "null".to_owned(),
            3 => format!("3 2 {} 0 {} 0 {} 0", g.f(false), g.f(false), g.f(false)),
            4 => format!("4 1 {} {} {} 3f800000", g.f(false), g.f(false), g.f(false)),
            _ => format!("3 1 {} {} {}", g.f(false), g.f(false), g.f(false)),
        };
        format!("{a} {d} {r} {o}")
    }),
    ("rotx", |g| {
        let a = g.w(&[0, 3, 4]);
        let d = g.dst(&a, false);
        format!(
            "{a} {d} {}",
            g.pick(&["3fc90fdb", "3e99999a", "00000000", "7fc00000", "bf800000"])
        )
    }),
    ("roty", |g| {
        let a = g.w(&[0, 3, 4]);
        let d = g.dst(&a, false);
        format!(
            "{a} {d} {}",
            g.pick(&["3fc90fdb", "3e99999a", "7f800000", "40000000"])
        )
    }),
    ("rotz", |g| {
        let a = g.w(&[0, 3, 4]);
        let d = g.dst(&a, false);
        format!("{a} {d} {}", g.f(true))
    }),
];

/// `skull W thresh n (x0 y0 z0 x1 y1 z1 value)×n` (§20) on a plain volume of 4 to 24 voxels a
/// side: a bright head with dark margins, a dark inside, painted boxes.
fn skull(g: &mut G) -> String {
    let n = if g.one_in(4) {
        g.int(4, 11)
    } else {
        g.int(12, 24)
    };
    let dims = [n, if g.one_in(3) { g.int(4, 24) } else { n }, n];
    let t = g.pick(&["0", "3", "4"]);
    let fill = g.pick(&["-1", "0", "7"]);
    let one = "3f800000";
    let zero = "00000000";
    let geo = [
        one, one, one, "1", one, zero, zero, zero, one, zero, zero, zero, one,
    ];
    let mut w = format!(
        "{} {} {} {t} 1 0 {fill} {}",
        dims[0],
        dims[1],
        dims[2],
        geo.join(" ")
    );
    w.push_str(&format!(" {zero} {zero} {zero} {zero} {zero} {zero}"));
    w.push_str(&format!(" 1 {}", dims[2]));
    for _ in 0..11 {
        w.push_str(&format!(" {zero}"));
    }
    w.push_str(" 0000000000000000 0 00000000 00000000 0 1 null null");
    let mut boxes = Vec::new();
    if dims.iter().all(|&x| x >= 16) {
        let m: Vec<i64> = dims.iter().map(|&x| g.int(3, x / 4)).collect();
        let hi: Vec<i64> = dims.iter().map(|&x| x - 1 - g.int(3, x / 4)).collect();
        boxes.push(format!(
            "{} {} {} {} {} {} {}",
            m[0],
            m[1],
            m[2],
            hi[0],
            hi[1],
            hi[2],
            g.pick(&["42f00000", "43480000"])
        ));
        if g.one_in(2) {
            let c: Vec<i64> = (0..3).map(|k| (m[k] + hi[k]) / 2).collect();
            boxes.push(format!(
                "{} {} {} {} {} {} {zero}",
                c[0] - 1,
                c[1] - 2,
                c[2] - 1,
                c[0] + 1,
                c[1] + 2,
                c[2] + 1
            ));
        }
    }
    for _ in 0..g.int(0, 2) {
        let lo: Vec<i64> = dims.iter().map(|&x| g.int(0, x - 1)).collect();
        let hi: Vec<i64> = (0..3)
            .map(|k| (lo[k] + g.int(0, dims[k])).min(dims[k] - 1))
            .collect();
        boxes.push(format!(
            "{} {} {} {} {} {} {}",
            lo[0],
            lo[1],
            lo[2],
            hi[0],
            hi[1],
            hi[2],
            g.pick(&[
                "00000000", "41f00000", "42f00000", "437f0000", "c1200000", "7fc00000"
            ])
        ));
    }
    let th = g.pick(&["-5", "0", "1", "50", "100", "100", "150", "255", "300"]);
    format!("{w} {th} {} {}", boxes.len(), boxes.join(" "))
}

const SKULL: &[(&str, Make)] = &[("skull", skull)];

/// Bytes as the probe's string argument.
fn xs(b: &[u8]) -> String {
    let mut s = "x".to_owned();
    for c in b {
        s.push_str(&format!("{c:02x}"));
    }
    s
}

/// A transform file's text (§21): an `.lta`, an `.xfm`, an FSL matrix or a `register.dat`,
/// sometimes cut.
fn transform_text(g: &mut G, kind: u64) -> String {
    let num = |g: &mut G| format!("{}", g.int(-2000, 2000) as f64 / 1000.0);
    let row = |g: &mut G| (0..4).map(|_| num(g)).collect::<Vec<_>>().join(" ");
    let mut t = match kind {
        0 => {
            let n = g.pick(&["1", "1", "2", "0", "-1"]);
            let head = g.pick(&["type      = 1", "type = 21 # x", "typ = 1", "type\t= 14"]);
            let mut t = format!("# lta\n{head}\nnxforms   = {n}\n");
            let k: i64 = n.parse().unwrap_or(0);
            for _ in 0..k.max(0) {
                t.push_str(&format!("mean      = {} {} {}\n", num(g), num(g), num(g)));
                t.push_str(&format!("sigma     = {}\n", g.pick(&["1", "1e3", "nan"])));
                let (r, c) = if g.one_in(5) { (4, 5) } else { (4, 4) };
                t.push_str(&format!("1 {r} {c}\n"));
                for _ in 0..r {
                    let v: Vec<String> = (0..c).map(|_| num(g)).collect();
                    t.push_str(&format!("{}\n", v.join(" ")));
                }
            }
            for _ in 0..k.max(0) {
                if g.one_in(2) {
                    t.push_str("src volume info\nvalid = 1\nfilename = a.mgz\nvolume = 2 3 4\nvoxelsize = 1 1 2\n");
                    t.push_str("xras   = 1 0 0\nyras   = 0 1 0\nzras   = 0 0 1\ncras   = 1 2 3\n");
                    t.push_str("dst volume info\nvalid = 0\nfilename = b.mgz\nvolume = 5 5 5\n");
                }
            }
            if g.one_in(2) {
                t.push_str(&format!("subject {}\n", g.pick(&["bert", "x"])));
            }
            if g.one_in(2) {
                t.push_str(&format!("fscale {}\n", g.pick(&["0.1", "x", "-2"])));
            }
            t
        }
        1 => {
            let info = g.pick(&[
                "%Generated by mritotal src s.mnc dst average_305.mnc",
                "%Generated by mritotal src there.mnc dst b",
                "%Generated by x src",
                "%Generated by x src a.mnc dst",
                "% x %Generated by srcdst a.mnc b.mnc",
                "%x minctracc in.mnc there.mnc",
                "%x minctracc one.mnc",
                "%src.mnc there.mnc",
                "Transform_Type = Linear;",
            ]);
            let info2 = g.pick(&["%a.mnc src2.mnc\n", "%a\n%b\n", "", "%c\n%d\n#e\n"]);
            let first = g.pick(&["MNI Transform File", "Not MNI", " MNI Transform File"]);
            let short = if g.one_in(3) { "1 2" } else { "0 1 0 0 # c" };
            format!(
                "{first}\n{info}\n{info2}\nTransform_Type = Linear;\nLinear_Transform =\n{}\n{short}\n{};\n",
                row(g),
                row(g)
            )
        }
        2 => (0..4).map(|_| format!("{}\n", row(g))).collect(),
        _ => format!(
            "bert\n{}\n1\n0.15\n{}\n{}\n{}\n{}\n{}",
            g.pick(&["1", "0.9375"]),
            row(g),
            row(g),
            row(g),
            row(g),
            g.pick(&["", "round\n", "tkreg\n", "bad\n"])
        ),
    };
    if g.one_in(8) {
        let k = g.below(t.len() as u64 + 1) as usize;
        t.truncate(k);
    }
    t
}

const LTAREAD: &[(&str, Make)] = &[
    ("tftype", |g| {
        let names = [
            "a.lta",
            "b.XfM",
            "c.m3z",
            "d.nii.gz",
            "nii.gz",
            "x.fslmat",
            "y.dat",
            "noext",
            "a@lta",
            "a.lta@xfm#y",
            "a#b.xfm",
            "a.",
            "@",
            "",
            "dir.d/file",
            "a.LtA#.xfm",
            "a#b@xfm",
            "a#b@XFM",
            "d/e.f/g",
            "x.nii.gz@lta",
            "a.xfm@",
            "#",
            "M3Z",
            "a.lta.",
        ];
        let n = g.int(1, 4);
        let v: Vec<String> = (0..n)
            .map(|_| xs(names[g.below(names.len() as u64) as usize].as_bytes()))
            .collect();
        format!("{n} {}", v.join(" "))
    }),
    ("ltamem", |g| {
        let kind = g.below(5);
        let (name, text) = match kind {
            0 => (g.pick(&["t.lta", "t", "t.m3z"]), Some(transform_text(g, 0))),
            1 => ("t.xfm", Some(transform_text(g, 1))),
            2 => ("t.fslmat", Some(transform_text(g, 2))),
            3 => (g.pick(&["t.dat", "t.reg"]), Some(transform_text(g, 3))),
            _ => (
                g.pick(&[
                    "identity.nofile",
                    "d/identity.nofile",
                    "none.lta",
                    "none.dat",
                ]),
                None,
            ),
        };
        let text = text.map_or("nofile".to_owned(), |t| xs(t.as_bytes()));
        let vol = if g.one_in(2) {
            format!("{} {}", xs(b"there.mnc"), g.geom(false))
        } else {
            "none".to_owned()
        };
        format!("{} {text} {} {vol}", xs(name.as_bytes()), g.int(0, 2))
    }),
    // verbose diagnostics with DIAG_SHOW: the volumes' INFO lines (§21, step 5)
    ("+gdiag=10000040 ltamem", |g| {
        let text = transform_text(g, 1);
        format!(
            "{} {} {} none",
            xs(b"t.xfm"),
            xs(text.as_bytes()),
            g.int(0, 2)
        )
    }),
    // several .xfm files in one process: minctracc's count (§21, step 5)
    ("ltamems", |g| {
        let n = g.int(2, 3);
        let mut v = Vec::new();
        for k in 0..n {
            let info = g.pick(&[
                "%x minctracc one.mnc",
                "%x minctracc a.mnc b.mnc",
                "%Generated by x src s.mnc",
                "%x",
            ]);
            let t = format!(
                "MNI Transform File\n{info}\nTransform_Type = Linear;\nLinear_Transform =\n1 0 0 0\n0 1 0 0\n0 0 1 0;\n"
            );
            let name = format!("t{k}.xfm");
            if g.one_in(6) {
                v.push(format!("{} nofile", xs(name.as_bytes())));
            } else {
                v.push(format!("{} {}", xs(name.as_bytes()), xs(t.as_bytes())));
            }
        }
        format!("{n} {} {} none", v.join(" "), g.int(0, 2))
    }),
];

impl G {
    /// A label value (§23): the colour table's numbers, their neighbours, fractions and special
    /// values, as a float's bits.
    fn label(&mut self) -> String {
        const L: [i32; 28] = [
            0, 2, 3, 4, 5, 10, 17, 18, 28, 30, 31, 41, 42, 43, 49, 62, 77, 87, 251, 255, 8, 47, 12,
            1000, 2035, 5001, -1, 3,
        ];
        match self.below(10) {
            0 => self
                .pick(&["7fc00000", "4f32d05e", "cf32d05e", "3fc00000", "c0200000"])
                .to_owned(),
            _ => format!(
                "{:08x}",
                (L[self.below(L.len() as u64) as usize] as f32).to_bits()
            ),
        }
    }

    /// P, a painting of a volume of the shape `shape`: n voxels of frame 0 set to labels.
    fn paint(&mut self, shape: &str) -> String {
        let e: Vec<i64> = shape
            .split(' ')
            .take(3)
            .map(|x| x.parse().unwrap_or(1))
            .collect();
        let n = self.int(0, 6);
        let mut v = vec![n.to_string()];
        for _ in 0..n {
            let (x, y, z) = (
                self.int(0, e[0] - 1),
                self.int(0, e[1] - 1),
                self.int(0, e[2] - 1),
            );
            let l = self.label();
            v.push(format!("{x} {y} {z} {l}"));
        }
        v.join(" ")
    }

    /// A segmentation W and its painting, with the shape's six tokens.
    fn seg(&mut self) -> (String, String) {
        let shape = self.shape(NUMERIC);
        let w = self.w_of(&shape);
        let p = self.paint(&shape);
        (shape, format!("{w} {p}"))
    }

    /// A second volume of the same shape (a ribbon) with its painting.
    fn like(&mut self, shape: &str) -> String {
        let w = self.w_of(shape);
        let p = self.paint(shape);
        format!("{w} {p}")
    }

    /// A label list (M) of `rows` rows: `n x 1`, `1 x n`, `n x 2` or `n x 0`.
    fn list(&mut self, rows: i64) -> String {
        let cols = match self.below(6) {
            0 => 2,
            1 => 0,
            _ => 1,
        };
        let v: Vec<String> = (0..rows * cols).map(|_| self.label()).collect();
        format!("{rows} {cols} {}", v.join(" "))
    }
}

/// A cache's text (§23): the 16 lines written from values, then lines dropped, doubled or
/// changed: other number forms, a negative value, carriage returns and white space at the end.
fn cache_text(g: &mut G) -> String {
    let stats: Vec<f64> = (0..16).map(|_| g.int(-5, 3000) as f64 / 4.0).collect();
    let text = String::from_utf8(volume_cmd::cache_text(&stats)).expect("text");
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    for _ in 0..g.int(0, 3) {
        let k = g.below(lines.len() as u64) as usize;
        match g.below(7) {
            0 => {
                lines.remove(k);
            }
            1 => {
                let l = lines[k].clone();
                lines.push(l);
            }
            2 => lines[k].push('\r'),
            3 => lines[k] = format!("{} ", lines[k].replace(", mm^3", ", x,")),
            4 => {
                let w = g.pick(&[
                    "12abc,", "0x10,", "1e,", "1e400,", "inf,", "-0,", "+.5e1,", "7",
                ]);
                lines[k] = format!(
                    "# Measure {}, {w} mm^3",
                    lines[k].split(", ").nth(1).unwrap_or("")
                );
            }
            5 => lines[k] = lines[k].replace(", mm^3", ", -2.5, mm^3"),
            _ => lines[k] = lines[k].replace(", mm^3", " ,\t2.5e2, mm^3"),
        }
        if lines.is_empty() {
            break;
        }
    }
    let mut t = lines.join("\n");
    if g.one_in(3) {
        t.push('\n');
    }
    t
}

const CMA: &[(&str, Make)] = &[
    ("cmalab", |g| {
        let n = g.int(1, 4);
        let v: Vec<String> = (0..n)
            .map(|_| {
                let l = g.label();
                let i = f32::from_bits(u32::from_str_radix(&l, 16).unwrap_or(0));
                let i = if i.is_finite() && i.abs() < 1e6 {
                    i as i32
                } else {
                    270
                };
                let f = if g.one_in(3) {
                    format!("{:08x}", (i as f32 + 0.5).to_bits())
                } else {
                    l
                };
                format!("{i} {f}")
            })
            .collect();
        format!("{n} {}", v.join(" "))
    }),
    ("cmavol", |g| {
        let (shape, a) = g.seg();
        let r = g.like(&shape.split(' ').take(6).collect::<Vec<_>>().join(" "));
        format!("{a} {r} {}", g.int(0, 3))
    }),
    ("lrswap", |g| g.seg().1),
    ("fixrib", |g| {
        let (shape, a) = g.seg();
        let s6 = shape.split(' ').take(6).collect::<Vec<_>>().join(" ");
        let r = g.like(&s6);
        let d = match g.below(5) {
            0 => "new".to_owned(),
            1 => "self".to_owned(),
            2 => "other".to_owned(),
            _ => g.w_of(&s6),
        };
        format!("{a} {r} {d}")
    }),
    ("lrseg", |g| {
        let a = g.seg().1;
        let n = g.int(1, 4);
        let m = if g.one_in(6) { g.int(1, 4) } else { n };
        format!("{a} {} {}", g.list(n), g.list(m))
    }),
    ("ohe", |g| {
        let a = g.seg().1;
        let n = g.int(0, 4);
        let v: Vec<String> = (0..n)
            .map(|_| g.pick(&["0", "2", "3", "41", "42", "0", "17"]).to_owned())
            .collect();
        format!("{a} {n} {} {}", v.join(" "), g.int(0, 2))
    }),
    ("bvtext", |g| {
        let v: Vec<String> = (0..16).map(|_| g.d(true)).collect();
        v.join(" ")
    }),
    ("bvparse", |g| xs(cache_text(g).as_bytes())),
];

/// A resampling's destination (§25): none, the source, one of the result's extent, of the
/// source's shape, or any volume.
fn rdst(g: &mut G, shape: &str, ext: Option<(i64, i64, i64)>) -> String {
    let s: Vec<&str> = shape.split(' ').take(6).collect();
    match g.below(6) {
        0 | 1 => "new".to_owned(),
        2 => "self".to_owned(),
        3 => match ext {
            Some((w, h, d)) if w > 0 && h > 0 && d > 0 => {
                g.w_of(&format!("{w} {h} {d} {} {} 0", s[3], s[4]))
            }
            _ => g.w_of(&s.join(" ")),
        },
        4 => g.w_of(&s.join(" ")),
        _ => g.w(ALL),
    }
}

/// The extent of a shape.
fn ext3(shape: &str) -> (i64, i64, i64) {
    let v: Vec<i64> = shape
        .split(' ')
        .take(3)
        .map(|x| x.parse().unwrap_or(1))
        .collect();
    (v[0], v[1], v[2])
}

const FACTORS: &[&str] = &[
    "3f800000", "40000000", "40400000", "3fc00000", "3f000000", "40800000", "40200000", "3f400000",
];

const FRACTIONS: &[&str] = &[
    "00000000", "3c23d70a", "3dcccccd", "3f000000", "3f7fbe77", "3f800000", "bf000000", "7fc00000",
];

const RES: &[(&str, Make)] = &[
    ("setres", |g| {
        let a = g.w(ALL);
        format!("{a} {} {} {}", g.f(true), g.f(true), g.f(true))
    }),
    ("vfill", |g| {
        let a = g.w(ALL);
        let v = if g.one_in(2) {
            g.pick(&[
                "437f8000", "43800000", "477fff00", "4f000000", "501502f9", "7f800001", "ffc12345",
                "bf000000",
            ])
            .to_owned()
        } else {
            g.f(true)
        };
        format!("{a} {v}")
    }),
    ("fillfg", |g| {
        let s = g.shape(&[0, 0, 0, 1, 3, 4, 10]);
        let a = g.w_of(&s);
        let d = rdst(g, &s, None);
        let (w, h, dd) = ext3(&s);
        let seed = [g.int(-1, w), g.int(-1, h), g.int(-1, dd)];
        let th = g.pick(&["0", "1", "64", "128", "255", "256", "-1"]);
        let fv = g.pick(&["1", "128", "255", "0", "256", "300", "-1"]);
        format!("{a} {d} {} {} {} {th} {fv}", seed[0], seed[1], seed[2])
    }),
    ("explane", |g| {
        let s = g.shape(ALL);
        let a = g.w_of(&s);
        let d = rdst(g, &s, None);
        let o = g.pick(&["0", "1", "2", "3", "-1"]);
        format!("{a} {d} {o} {}", g.int(-1, 4))
    }),
    ("fillplane", |g| {
        let m = g.w(ALL);
        let d = g.w(ALL);
        let o = g.pick(&["0", "1", "2", "3"]);
        let fv = g.pick(&["0", "1", "7", "255", "256", "-3", "70000"]);
        format!("{m} {d} {o} {} {fv}", g.int(-1, 4))
    }),
    ("ups", |g| {
        let s = g.shape(NUMERIC);
        let a = g.w_of(&s);
        let k = g.int(0, 2);
        let n = g.pick(&["1", "2", "2", "3", "0", "-1"]);
        let m: i64 = if k == 1 { 2 } else { n.parse().unwrap_or(1) };
        let (w, h, d) = ext3(&s);
        let dv = rdst(g, &s, Some((m * w, m * h, m * d)));
        format!("{k} {a} {dv} {n}")
    }),
    ("dns2", |g| {
        let s = g.shape(ALL);
        let a = g.w_of(&s);
        let (w, h, d) = ext3(&s);
        let dv = rdst(g, &s, Some((w / 2, h / 2, d / 2)));
        format!("{a} {dv}")
    }),
    ("dnsn", |g| {
        let s = g.shape(ALL);
        let a = g.w_of(&s);
        let f = [g.pick(FACTORS), g.pick(FACTORS), g.pick(FACTORS)];
        let dv = rdst(g, &s, None);
        format!("{a} {dv} {} {} {} {}", f[0], f[1], f[2], g.int(0, 1))
    }),
    ("mft", |g| {
        let a = g.w(ALL);
        format!("{a} {} {}", g.int(-1, 3), g.f(true))
    }),
    ("lim", |g| g.w(ALL)),
    ("chtype", |g| {
        let k = g.int(0, 1);
        let a = g.w(ALL);
        let t = ALL[g.below(ALL.len() as u64) as usize];
        format!(
            "{k} {a} {t} {} {} {} {}",
            g.pick(FRACTIONS),
            g.pick(FRACTIONS),
            g.int(0, 2),
            g.int(0, 1)
        )
    }),
];

impl G {
    /// A 4 x 4 matrix (M): the identity, a scaling, a shift, a permutation with a shift, seeded
    /// ratios with special values, a singular one; or a 3 x 3 one, or none.
    fn mat4(&mut self) -> String {
        let one = "3f800000";
        let z = "00000000";
        match self.below(9) {
            0 => format!(
                "4 4 {one} {z} {z} {z} {z} {one} {z} {z} {z} {z} {one} {z} {z} {z} {z} {one}"
            ),
            1 => {
                let d = [self.size(false), self.size(false), self.size(false)];
                format!(
                    "4 4 {} {z} {z} {z} {z} {} {z} {z} {z} {z} {} {z} {z} {z} {z} {one}",
                    d[0], d[1], d[2]
                )
            }
            2 => {
                let t = [self.f(false), self.f(false), self.f(false)];
                format!(
                    "4 4 {one} {z} {z} {} {z} {one} {z} {} {z} {z} {one} {} {z} {z} {z} {one}",
                    t[0], t[1], t[2]
                )
            }
            3 => {
                let c = self.cosines(false);
                let t = [self.f(false), self.f(false), self.f(false)];
                format!(
                    "4 4 {} {} {} {} {} {} {} {} {} {} {} {} {z} {z} {z} {one}",
                    c[0], c[1], c[2], t[0], c[3], c[4], c[5], t[1], c[6], c[7], c[8], t[2]
                )
            }
            4 | 5 => {
                let v: Vec<String> = (0..12)
                    .map(|_| {
                        let special = self.one_in(4);
                        self.f(special)
                    })
                    .collect();
                format!("4 4 {} {z} {z} {z} {one}", v.join(" "))
            }
            6 => {
                format!("4 4 {one} {z} {z} {z} {z} {z} {z} {z} {z} {z} {one} {z} {z} {z} {z} {one}")
            }
            7 => self.mat3(),
            _ => "null".to_owned(),
        }
    }

    /// A small working volume of the sampled types (bytes to floats, sometimes any type).
    fn ws(&mut self) -> String {
        let types = if self.one_in(6) { ALL } else { NUMERIC };
        self.w(types)
    }
}

/// `sseq W d n (x y z first last code)×n` and `sxyz W d n (x y z begin end k)×n` (§27).
fn frame_range(g: &mut G, seq: bool) -> String {
    let a = g.ws();
    let n = g.int(1, 3);
    let mut pts = Vec::new();
    for _ in 0..n {
        if seq {
            let (x, y, z) = (g.coord(), g.coord(), g.coord());
            let code = if g.one_in(12) {
                "5"
            } else {
                g.pick(&["-1", "-1", "0", "1", "1", "2", "3", "4", "6", "7"])
            };
            pts.push(format!(
                "{x} {y} {z} {} {} {code}",
                g.int(-1, 2),
                g.int(-1, 3)
            ));
        } else {
            pts.push(format!(
                "{} {} {} {} {} {}",
                g.int(-2, 4),
                g.int(-2, 4),
                g.int(-2, 4),
                g.int(-1, 2),
                g.int(-1, 3),
                g.int(0, 1)
            ));
        }
    }
    format!("{a} {} {n} {}", g.d(true), pts.join(" "))
}

const FRAMES: &[(&str, Make)] = &[
    ("splat", |g| {
        let a = g.ws();
        let n = g.int(1, 3);
        let v: Vec<String> = (0..n)
            .map(|_| {
                let val = match g.below(4) {
                    0 => g
                        .pick(&["4202a05f20000000", "40f1170000000000", "c0f1170000000000"])
                        .to_owned(),
                    _ => g.d(true),
                };
                format!(
                    "{} {} {} {} {} {val}",
                    g.int(0, 1),
                    g.coord(),
                    g.coord(),
                    g.coord(),
                    g.int(0, 2)
                )
            })
            .collect();
        format!("{a} {n} {}", v.join(" "))
    }),
    ("sseq", |g| frame_range(g, true)),
    ("sxyz", |g| frame_range(g, false)),
    ("tkern", |g| {
        let a = g.w(ALL);
        let n = g.int(1, 3);
        let v: Vec<String> = (0..n)
            .map(|_| format!("{} {} {} {}", g.coord(), g.coord(), g.coord(), g.int(0, 1)))
            .collect();
        format!("{a} {n} {}", v.join(" "))
    }),
];

fn sigma(g: &mut G) -> String {
    let v: f64 = match g.below(10) {
        0 => 0.0,
        1 => 1e-9,
        2 => -1.0,
        3 => 1e300,
        4 => f64::NAN,
        5 => {
            if g.one_in(3) {
                return g.pick(&["7fefffffffffffff", "7ff0000000000000"]).to_owned();
            }
            0.5
        }
        _ => g.int(1, 12) as f64 / 4.0,
    };
    format!("{:016x}", v.to_bits())
}

const DERIV: &[(&str, Make)] = &[
    ("deriv", |g| {
        let a = g.ws();
        let s = g.pick(&[
            "3ff0000000000000",
            "bff0000000000000",
            "3fe0000000000000",
            "7ff8000000000000",
        ]);
        let n = g.int(1, 3);
        let v: Vec<String> = (0..n)
            .map(|_| {
                format!(
                    "{} {} {} {} {} {} {} {}",
                    g.int(0, 1),
                    g.coord(),
                    g.coord(),
                    g.coord(),
                    g.d(true),
                    g.d(true),
                    g.d(true),
                    sigma(g)
                )
            })
            .collect();
        format!("{a} {s} {n} {}", v.join(" "))
    }),
    ("grad", |g| {
        let a = g.ws();
        let n = g.int(1, 3);
        let v: Vec<String> = (0..n)
            .map(|_| {
                format!(
                    "{} {} {} {} {}",
                    g.int(0, 1),
                    g.coord(),
                    g.coord(),
                    g.coord(),
                    g.int(0, 2)
                )
            })
            .collect();
        format!("{a} {n} {}", v.join(" "))
    }),
    ("nbrs", |g| {
        let a = g.w(ALL);
        let n = g.int(1, 4);
        let v: Vec<String> = (0..n)
            .map(|_| {
                let k = g.int(0, 7);
                let x = if g.one_in(30) { 600 } else { g.int(-3, 5) };
                let pq = match k {
                    2 => format!("{} {}", g.int(-3, 7), g.int(0, 255)),
                    4 => format!("{} 0", g.int(0, 2)),
                    _ => format!("{} 0", g.int(-1, 255)),
                };
                format!(
                    "{k} {x} {} {} {pq} {} {}",
                    g.int(-3, 5),
                    g.int(-3, 5),
                    g.f(true),
                    g.f(true)
                )
            })
            .collect();
        format!("{a} {n} {}", v.join(" "))
    }),
];

const LINEAR: &[(&str, Make)] = &[
    ("setvals", |g| {
        let a = g.w(ALL);
        format!("{a} {}", g.f(true))
    }),
    ("rasvox", |g| {
        let a = g.ws();
        let b = if g.one_in(3) {
            "null".to_owned()
        } else {
            g.ws()
        };
        format!("{a} {b} {}", g.mat4())
    }),
    ("lxf", |g| {
        let k = g.int(0, 5);
        let s = g.shape(NUMERIC);
        let a = g.w_of(&s);
        let ov = g.pick(&[
            "0000000000000000",
            "4014000000000000",
            "3e112e0be826d695",
            "c004000000000000",
            "7ff8000000000000",
        ]);
        let d = rdst(g, &s, None);
        let interp = if g.one_in(14) {
            "5"
        } else {
            g.pick(&["0", "1", "1", "2", "3", "4", "7"])
        };
        format!("{k} {a} {ov} {d} {} {interp}", g.mat4())
    }),
    ("rsmat", |g| {
        let a = g.ws();
        let b = g.ws();
        format!("{a} {b}")
    }),
    ("+gdiag=10000040 rsmat", |g| {
        let a = g.ws();
        let b = g.ws();
        format!("{a} {b}")
    }),
];

/// A template of a source's shape: its extent divided or kept, the voxel sizes scaled.
fn template_of(g: &mut G, s: &str) -> String {
    if g.one_in(3) {
        return g.ws();
    }
    let (w, h, d) = ext3(s);
    let t: Vec<&str> = s.split(' ').collect();
    let k = g.int(1, 2);
    let shape = format!(
        "{} {} {} {} {} 0",
        (w / k).max(1),
        (h / k).max(1),
        (d / k).max(1),
        t[3],
        g.int(1, 2)
    );
    g.w_of(&shape)
}

const REGRID: &[(&str, Make)] = &[
    ("rsfill", |g| {
        let s = g.shape(&[0, 0, 1, 2, 3, 4]);
        let a = g.w_of(&s);
        let b = template_of(g, &s);
        let t = if g.one_in(16) {
            "5"
        } else {
            g.pick(&["0", "1", "1", "2", "3", "4", "6", "6", "7"])
        };
        format!("{a} {b} {t} {} {}", g.f(true), i64::from(g.one_in(5)))
    }),
    ("+gdiag=10000040 rsfill", |g| {
        let s = g.shape(&[0, 3]);
        let a = g.w_of(&s);
        let b = template_of(g, &s);
        format!("{a} {b} {} {} 0", g.pick(&["0", "1", "6"]), g.f(false))
    }),
    ("repval", |g| {
        let s = g.shape(&[0, 0, 0, 1, 3, 4, 6]);
        let a = g.w_of(&s);
        let d = rdst(g, &s, None);
        let rest = if g.one_in(2) {
            format!("1 {} {}", g.int(-1, 256), g.int(-1, 256))
        } else {
            format!("0 {} {}", g.f(true), g.f(true))
        };
        format!("{a} {d} {rest}")
    }),
];

/// An orientation string (§33): 0 to 5 characters of the axes' letters in both cases, an invalid
/// letter, a NUL and a byte past ASCII, as `x<hex>`.
fn ostring(g: &mut G) -> String {
    const C: &[u8] = b"LRPAISlrpaisX0\x00\x80";
    if g.one_in(2) {
        let mut axes = [
            b"LR"[g.below(2) as usize],
            b"PA"[g.below(2) as usize],
            b"IS"[g.below(2) as usize],
        ];
        let k = g.below(3) as usize;
        axes.swap(0, k);
        return xs(&axes);
    }
    let n = g.int(0, 5) as usize;
    let b: Vec<u8> = (0..n)
        .map(|_| C[g.below(C.len() as u64) as usize])
        .collect();
    xs(&b)
}

/// A matrix of `rows x cols` seeded floats (some special).
fn matrix(g: &mut G, rows: i64, cols: i64) -> String {
    let v: Vec<String> = (0..rows * cols).map(|_| g.f(true)).collect();
    format!("{rows} {cols} {}", v.join(" "))
}

const INFO: &[(&str, Make)] = &[
    ("stats", |g| g.w(ALL)),
    ("pstats", |g| g.w(ALL)),
    ("type2s", |_| {
        "15 -2 -1 0 1 2 3 4 5 6 7 8 9 10 11 12".to_owned()
    }),
    ("cropbb", |g| {
        let a = g.w(NUMERIC);
        let mut v = Vec::new();
        for _ in 0..3 {
            v.push(
                g.pick(&["-3", "0", "1", "2", "5", "2147483647", "-2147483648"])
                    .to_owned(),
            );
        }
        for _ in 0..3 {
            v.push(g.pick(&["-1", "0", "1", "3", "7", "2147483647"]).to_owned());
        }
        format!("{a} {}", v.join(" "))
    }),
    ("crass", |g| {
        let a = g.ws();
        let b = g.ws();
        format!("{a} {b}")
    }),
    ("+gdiag=10000040 crass", |g| {
        let a = g.ws();
        let b = g.ws();
        format!("{a} {b}")
    }),
    ("chkostr", ostring),
    ("o2dc", |g| {
        let a = g.ws();
        format!("{a} {}", ostring(g))
    }),
    ("s44", |g| {
        let k = g.int(0, 3);
        let size = if k == 2 {
            format!(" {} {}", g.int(3, 5), g.int(3, 5))
        } else {
            String::new()
        };
        let v: Vec<String> = (0..16).map(|_| g.f(true)).collect();
        format!("{k}{size} {}", v.join(" "))
    }),
    ("vx2rx", |g| {
        let a = g.ws();
        let b = if g.one_in(3) {
            "null".to_owned()
        } else {
            g.ws()
        };
        let mv = g.mat4();
        let mr = if g.one_in(2) {
            "null".to_owned()
        } else {
            g.mat4()
        };
        format!("{a} {b} {mv} {mr}")
    }),
    ("v2v", |g| {
        let a = g.ws();
        let b = g.ws();
        format!("{a} {b}")
    }),
    ("setv2r", |g| {
        let a = g.ws();
        let m = match g.below(4) {
            0 | 1 => g.mat4(),
            2 => matrix(g, 3, 4),
            _ => matrix(g, 2, 2),
        };
        let e = g.pick(&["unset", "unset", "x302e35", "x616263", "x", "x3130"]);
        format!("{a} {m} {e}")
    }),
];

/// A B-spline sampling point (§4.1): `x y z frame`.
fn bpoint(g: &mut G) -> String {
    format!("{} {} {} {}", g.coord(), g.coord(), g.coord(), g.int(-1, 2))
}

/// An LT of a transform array: the matrix, the type, the label and two geometries (Gf).
fn bslt(g: &mut G) -> String {
    let mut m = g.mat4();
    while m == "null" {
        m = g.mat4();
    }
    let gs = format!("{} {} null", g.geom(false), g.int(0, 1));
    let gd = format!("{} {} null", g.geom(false), g.int(0, 1));
    format!(
        "{} {} {} {} {m} null null {} 0 {gs} {gd}",
        g.f(false),
        g.f(false),
        g.f(false),
        g.f(false),
        g.int(0, 2)
    )
}

const BSPLINE: &[(&str, Make)] = &[
    ("bsp", |g| {
        let k = g.int(0, 3);
        let s = g.shape(NUMERIC);
        let a = g.w_of(&s);
        let deg = g.pick(&["2", "3", "3", "4", "5", "6", "7", "8", "9", "0", "10", "-1"]);
        let extra = if k == 2 {
            if g.one_in(2) {
                let t: Vec<&str> = s.split(' ').collect();
                format!(
                    " {}",
                    g.w_of(&format!("{} {} {} 3 {} 0", t[0], t[1], t[2], t[4]))
                )
            } else {
                format!(" {}", g.w(NUMERIC))
            }
        } else {
            String::new()
        };
        format!("{k} {a} {deg}{extra}")
    }),
    ("bsamp", |g| {
        let deg = g.int(2, 9);
        let a = g.w(NUMERIC);
        let n = g.int(1, 3);
        let p: Vec<String> = (0..n).map(|_| bpoint(g)).collect();
        format!("{deg} {a} {} {n} {}", g.d(true), p.join(" "))
    }),
    ("bsraw", |g| {
        let deg = g.int(-2, 12);
        let a = g.w(&[3]);
        let n = g.int(1, 3);
        let p: Vec<String> = (0..n).map(|_| bpoint(g)).collect();
        format!(
            "{deg} {} {a} {} {n} {}",
            g.int(0, 1),
            g.d(true),
            p.join(" ")
        )
    }),
    ("bsseq", |g| {
        let deg = g.int(2, 9);
        let a = g.w(NUMERIC);
        let n = g.int(1, 3);
        let p: Vec<String> = (0..n)
            .map(|_| {
                let first = g.int(0, 1);
                let last = first + g.int(-1, 1);
                format!("{} {} {} {first} {last}", g.coord(), g.coord(), g.coord())
            })
            .collect();
        format!("{deg} {a} {} {n} {}", g.d(true), p.join(" "))
    }),
    ("bslxf", |g| {
        let s = g.shape(NUMERIC);
        let a = g.w_of(&s);
        let d = rdst(g, &s, None);
        format!("{} {a} {d} {}", g.int(2, 5), g.mat4())
    }),
    ("bsras", |g| {
        let s = g.shape(NUMERIC);
        let a = g.w_of(&s);
        let d = rdst(g, &s, None);
        format!("{} {a} {d} {}", g.int(2, 5), g.mat4())
    }),
    ("bslta", |g| {
        let s = g.shape(NUMERIC);
        let a = g.w_of(&s);
        let d = if g.one_in(2) {
            "new".to_owned()
        } else {
            rdst(g, &s, None)
        };
        let n = if g.one_in(12) { 2 } else { 1 };
        let ty = g.pick(&["0", "0", "1", "1", "1", "2", "14"]);
        let lts: Vec<String> = (0..2 * n).map(|_| bslt(g)).collect();
        format!(
            "{} {a} {d} {n} {ty} 3f800000 x {} {}",
            g.int(2, 4),
            lts.join(" "),
            g.int(0, 2)
        )
    }),
    ("bslta", |g| {
        // a clean source and geometries: the transform-array paths that succeed
        let z = "00000000";
        let one = "3f800000";
        let t = g.pick(&["0", "3"]);
        let c = [g.f(false), g.f(false), g.f(false)];
        let a = format!(
            "3 2 2 {t} 1 0 {} {one} 3fc00000 {one} 1 {one} {z} {z} {z} {one} {z} {z} {z} {one} {} {} {} {z} {z} {z} \
             0 1 {z} {z} {z} {z} {z} {z} {one} {one} {z} {z} {z} 0000000000000000 0 {z} {z} 0 1 null null",
            g.int(0, 255),
            c[0],
            c[1],
            c[2]
        );
        let geo = |g: &mut G| {
            let valid = g.pick(&["1", "1", "0"]);
            format!(
                "{valid} 3 2 2 {one} 3fc00000 {one} 1 {one} {z} {z} {z} {one} {z} {z} {z} {one} {z} {z} {z} {z} {z} {z} x 1 null"
            )
        };
        let rot = format!(
            "4 4 3f7ae148 be4ba5e3 {z} {one} 3e4ba5e3 3f7ae148 {z} bf000000 {z} {z} {one} 3e800000 {z} {z} {z} {one}"
        );
        let lt = |g: &mut G| {
            let (gs, gd) = (geo(g), geo(g));
            format!("{z} {z} {z} {z} {rot} null null 0 0 {gs} {gd}")
        };
        let (l1, l2) = (lt(g), lt(g));
        format!(
            "3 {a} new 1 {} {one} x {l1} {l2} {}",
            g.pick(&["0", "1", "1"]),
            g.int(0, 2)
        )
    }),
    ("bsdown", |g| {
        let s = g.shape(NUMERIC);
        let a = g.w_of(&s);
        let (w, h, d) = ext3(&s);
        let e = ((w / 2).max(1), (h / 2).max(1), (d / 2).max(1));
        let dd = rdst(g, &s, Some(e));
        format!("{a} {dd}")
    }),
    ("bsup", |g| {
        let s = g.shape(NUMERIC);
        let a = g.w_of(&s);
        let (w, h, d) = ext3(&s);
        let up = |e: i64| if e > 1 { 2 * e } else { 1 };
        let dd = rdst(g, &s, Some((up(w), up(h), up(d))));
        format!("{a} {dd}")
    }),
    ("bsfree", |g| g.int(0, 2).to_string()),
];

const BSVOL: &[(&str, Make)] = &[
    ("lxf", |g| {
        let k = g.pick(&["0", "3"]);
        let s = g.shape(NUMERIC);
        let a = g.w_of(&s);
        let ov = g.pick(&["0000000000000000", "4014000000000000", "7ff8000000000000"]);
        let d = rdst(g, &s, None);
        format!("{k} {a} {ov} {d} {} 5", g.mat4())
    }),
    ("rsfill", |g| {
        let s = g.shape(&[0, 0, 1, 2, 3, 4]);
        let a = g.w_of(&s);
        let b = template_of(g, &s);
        format!("{a} {b} 5 {} {}", g.f(true), i64::from(g.one_in(5)))
    }),
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

#[test]
fn samplers() {
    group("sample", SAMPLE, 8);
}

#[test]
fn moves() {
    group("move", MOVE, 6);
}

#[test]
fn head_box() {
    group("skull", SKULL, 24);
}

#[test]
fn cma_labels() {
    group("cma", CMA, 8);
}

#[test]
fn transform_reader() {
    group("ltaread", LTAREAD, 24);
}

#[test]
fn resampling() {
    group("res", RES, 8);
}

#[test]
fn frame_samplers_and_splats() {
    group("frames", FRAMES, 8);
}

#[test]
fn derivatives_and_neighbours() {
    group("deriv", DERIV, 8);
}

#[test]
fn linear_transforms() {
    group("linear", LINEAR, 8);
}

#[test]
fn template_resampling() {
    group("regrid", REGRID, 8);
}

#[test]
fn statistics_and_orientation() {
    group("info", INFO, 8);
}

#[test]
fn bsplines() {
    group("bspline", BSPLINE, 8);
}

#[test]
fn bspline_paths() {
    group("bsvol", BSVOL, 8);
}
