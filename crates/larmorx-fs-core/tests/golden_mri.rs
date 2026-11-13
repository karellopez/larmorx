// SPDX-License-Identifier: Apache-2.0
//! Golden cases of fs.mri's clean room (`specs/freesurfer-mri.md` §23.4, CLAUDE.md rule 10):
//! every command of the black-box probe's grammar (`tests/mri_cmd/`, the comparison's candidate
//! commands) on seeded arguments, in both defect modes, one SHA-256 per group of the spec's
//! sections over every result line (the printed values, both streams and the notices; NaN bits
//! included), recorded on Linux x86-64 and checked on every platform. The arguments come from a
//! seeded generator and plain arithmetic (special values written as their bits); no file system,
//! no clock; volumes of at most 3 x 3 x 3 voxels and 2 frames.
//!
//! Record: `FS_CORE_GOLDEN_RECORD=1 cargo test -p larmorx-fs-core --test golden_mri --
//! --nocapture` prints `GOLDEN <case> <sha256>` for each case instead of checking it
//! (`FS_CORE_GOLDEN_DUMP=<dir>` also writes each case's lines). The cases also run under the
//! AArch64 NaN emulation (`validation/investigations/freesurfer-cleanroom-mri/README.md`).

mod mri_cmd;

use larmorx_core::defects::UpstreamDefects;
use larmorx_fs_core::Environment;
use sha2::{Digest, Sha256};

const GOLDEN: &[(&str, &str)] = &[
    (
        "alloc.fix",
        "29f478948ea44c88395ea3d91bf2041aa5183ae6533cec8818b87cba75a6e3c3",
    ),
    (
        "alloc.reproduce",
        "03147d88b084b13335ec2bd5b8472206710591ad7f1e6b724998ff926f4eba56",
    ),
    (
        "bbox.fix",
        "0f761a3ca209236423517124ead9a19a808fb6e4c2dc053cee0b9f4d3d8231a1",
    ),
    (
        "bbox.reproduce",
        "f72cd01f10f4597a594fd661127c56b2bf340d6dca6defd052aa33c6f08b4f46",
    ),
    (
        "coords.fix",
        "8de2c029353de0bd207bf5c2a4ea8f4403e0df4950788bee8985a8ea55721e26",
    ),
    (
        "coords.reproduce",
        "e43dc4552306eb51740d1035bc3815274da17d8ac376e19d8d9400a02a6a09f1",
    ),
    (
        "copies.fix",
        "d7c86adf831a44f48170eb335bc8710f326cacb0fdef7ae5ce6a207e8fafed46",
    ),
    (
        "copies.reproduce",
        "2d079a8087e65dbc5cce212e606bb3ff7d298c1b7b71771b68090b5aa0894789",
    ),
    (
        "geometry.fix",
        "3a03d25922995e33f84f24e536ce0fdf61090e5bc6a7395391234a987a706784",
    ),
    (
        "geometry.reproduce",
        "640ebe7415c1af8fe2f3351c3d190d22ed8ba234f284bd89dff5017c3ee900d5",
    ),
    (
        "headers.fix",
        "f21e2b224aa225fa4cd5ffa28ee5f6a8a1c6faab7ebe666ec634163ec19265f5",
    ),
    (
        "headers.reproduce",
        "d501844f2ca9dd09b5c3aafa7b4c6dcb085489e86013ec3be7730fa11087f313",
    ),
    (
        "lta.fix",
        "a3a69b4bafe35438eaa672d2d554c2b17102f9845471c36816d23ddaef6cf5ba",
    ),
    (
        "lta.reproduce",
        "84e38f972f127c597c4991ba45236be82d10a36086a4d509dad5a5c88c9936d8",
    ),
    (
        "matrices.fix",
        "fb7de5c32fd4e54421c123418a242733dec533201653144fe484ffc1b0ca0802",
    ),
    (
        "matrices.reproduce",
        "df3d3e0b008fb34e59a4df11e4eb58c98d799e9ded8307a0428fa1537a12679e",
    ),
    (
        "names.fix",
        "523981053503f0f4125282dcfbc9c14f4dd7f28d8cc2cd5a9472c710ff6276bf",
    ),
    (
        "names.reproduce",
        "523981053503f0f4125282dcfbc9c14f4dd7f28d8cc2cd5a9472c710ff6276bf",
    ),
    (
        "printing.fix",
        "bc64df7c28fbdbe6fe623bc7c413114c4ceba73f9f543e9509fc94ebae5cb43c",
    ),
    (
        "printing.reproduce",
        "bc64df7c28fbdbe6fe623bc7c413114c4ceba73f9f543e9509fc94ebae5cb43c",
    ),
    (
        "registrations.fix",
        "3729e7cace24a69b42f7fe40df0856fd7ffc39c4dd1f12451fc9e94e54835317",
    ),
    (
        "registrations.reproduce",
        "bdb9ed7da0b0320488f1d74a47ad68deb8645b3f860debc1f1f36432ecaddecb",
    ),
    (
        "tkreg.fix",
        "bfb80585a6507598c51bec1dc4c85355a18b69399c4994575712acfed334ad48",
    ),
    (
        "tkreg.reproduce",
        "fea0fc8c8aca3073e5e6a0d19fa4e811a8a0c89f78f2712bd32402e8fd3106b1",
    ),
    (
        "voxels.fix",
        "3a72836db0f31c24ecd35638cf20bd921a814369d286a0263f5e4566d9d0d07a",
    ),
    (
        "voxels.reproduce",
        "207ccf6b6b54542ca711ecad38a27de1dc01bb9fa91138ba2a15db4875217639",
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
            const S: [u32; 8] = [
                0x0000_0000,
                0x8000_0000,
                0x7f80_0000,
                0xff80_0000,
                0x7fc0_0000,
                0xffc1_2345,
                0x7f7f_ffff,
                0x0000_0001,
            ];
            return S[self.below(8) as usize];
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
            const S: [u64; 4] = [
                0x8000_0000_0000_0000,
                0x7ff0_0000_0000_0000,
                0xfff8_0000_0000_0000,
                0x41e0_0000_0000_0000,
            ];
            return format!("{:016x}", S[self.below(4) as usize]);
        }
        let v = f64::from(f32::from_bits(self.fbits(false)));
        format!("{:016x}", v.to_bits())
    }

    /// A voxel size: positive mostly, sometimes 0 or a special value.
    fn size(&mut self, special: bool) -> String {
        if special && self.one_in(10) {
            return self.pick(&["00000000", "7fc00000", "bf800000"]).to_owned();
        }
        self.pick(&["3f800000", "3f000000", "40000000", "3fc00000", "40400000", "3f700000"])
            .to_owned()
    }

    /// A string: empty, a name, or one byte.
    fn x(&mut self) -> String {
        self.pick(&["x", "x6f7269672e6d677a", "x612062", "x41"]).to_owned()
    }

    /// V: a volume (`types` its voxel types, `hdr` 0 voxels / 1 header / 2 either).
    fn v(&mut self, types: &[i32], hdr: u64, odd: bool) -> String {
        let e = |g: &mut G| {
            if odd && g.one_in(8) {
                g.int(-1, 0)
            } else {
                g.int(1, 3)
            }
        };
        let (w, h, d) = (e(self), e(self), e(self));
        let t = types[self.below(types.len() as u64) as usize];
        let nf = self.int(1, 2);
        let h0 = if hdr == 2 { self.below(2) } else { hdr };
        format!("{w} {h} {d} {t} {nf} {h0}")
    }

    /// The three direction cosines: a signed permutation of the axes, sometimes a rotation about
    /// one axis by 45 degrees, sometimes (with `special`) arbitrary floats.
    fn cosines(&mut self, special: bool) -> String {
        if special && self.one_in(6) {
            let mut c = String::new();
            for k in 0..9 {
                if k > 0 {
                    c.push(' ');
                }
                c.push_str(&self.f(true));
            }
            return c;
        }
        const H: f32 = std::f32::consts::FRAC_1_SQRT_2;
        let mut m = [[0.0f32; 3]; 3];
        if self.one_in(5) {
            m = [[H, H, 0.0], [-H, H, 0.0], [0.0, 0.0, 1.0]];
        } else {
            let perms = [[0, 1, 2], [0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]];
            let p = perms[self.below(6) as usize];
            for (j, &axis) in p.iter().enumerate() {
                m[j][axis] = if self.one_in(2) { 1.0 } else { -1.0 };
            }
        }
        m.iter()
            .flatten()
            .map(|v| format!("{:08x}", v.to_bits()))
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// G: a geometry (with `valid`).
    fn g(&mut self, special: bool) -> String {
        let special = special && self.one_in(4);
        let valid = self.int(0, 1);
        let e = |g: &mut G| {
            if special && g.one_in(10) {
                g.int(-1, 0)
            } else {
                g.int(1, 6)
            }
        };
        let (w, h, d) = (e(self), e(self), e(self));
        let sizes = format!("{} {} {}", self.size(special), self.size(special), self.size(special));
        let ras = self.int(0, 1);
        let cos = self.cosines(special);
        let c = format!("{} {} {}", self.f(special), self.f(special), self.f(special));
        let s = if self.one_in(3) {
            format!("{} {} {}", self.pick(&["00000000", "3c23d70a", "3e4ccccd", "bdcccccd"]), self.pick(&["00000000", "3e4ccccd"]), self.pick(&["00000000", "358637bd", "3c23d70a"]))
        } else {
            "00000000 00000000 00000000".to_owned()
        };
        let x = self.x();
        format!("{valid} {w} {h} {d} {sizes} {ras} {cos} {c} {s} {x}")
    }

    /// M: a matrix of `rows` x `cols` (`null` sometimes when `maybe_null`).
    fn m(&mut self, rows: i64, cols: i64, special: bool, maybe_null: bool) -> String {
        if maybe_null && self.one_in(5) {
            return "null".to_owned();
        }
        let mut s = format!("{rows} {cols}");
        for _ in 0..rows * cols {
            s.push(' ');
            s.push_str(&self.f(special));
        }
        s
    }

    /// M of 4 x 4, or another size with `odd`.
    fn m4(&mut self, special: bool, maybe_null: bool, odd: bool) -> String {
        if odd && self.one_in(3) {
            let (r, c) = [(3, 3), (4, 5), (5, 5), (5, 4), (4, 3), (4, 1), (3, 4)]
                [self.below(7) as usize];
            return self.m(r, c, special, false);
        }
        self.m(4, 4, special, maybe_null)
    }

    /// Gf: a geometry, its cache flag and registration matrix.
    fn gf(&mut self) -> String {
        let g = self.g(true);
        let cache = self.int(0, 1);
        let reg = self.m4(true, true, false);
        format!("{g} {cache} {reg}")
    }

    /// CT: a colour table (`inner`: its own tissue table none).
    fn ct(&mut self, inner: bool) -> String {
        if self.one_in(4) {
            return "null".to_owned();
        }
        let n = self.int(0, 3);
        let mut s = format!("{n} {} {} {} {}", self.x(), self.int(0, 3), self.int(0, 5), self.x());
        for _ in 0..n {
            if self.one_in(4) {
                s.push_str(" 0");
                continue;
            }
            s.push_str(&format!(
                " 1 {} {} {} {} {} {} {} {} {} {} {}",
                self.x(),
                self.int(0, 255),
                self.int(0, 255),
                self.int(0, 255),
                self.int(0, 255),
                self.f(false),
                self.f(false),
                self.f(false),
                self.f(false),
                self.int(0, 4),
                self.int(0, 500)
            ));
        }
        let t = if inner { "null".to_owned() } else { self.ct(true) };
        s.push(' ');
        s.push_str(&t);
        s
    }

    /// H: the header fields of the copies.
    fn h(&mut self) -> String {
        let mut s = format!(
            "{} {} {} {} {} {} {} {} {} {}",
            self.int(0, 3),
            self.int(0, 5),
            self.int(0, 9),
            self.d(true),
            self.size(false),
            self.size(false),
            self.size(false),
            self.int(0, 3),
            self.int(0, 3),
            self.int(0, 3)
        );
        for _ in 0..10 {
            s.push(' ');
            s.push_str(&self.f(true));
        }
        s.push_str(&format!(" {} {} {} {} {}", self.d(true), self.f(false), self.f(false), self.f(false), self.x()));
        s.push(' ');
        s.push_str(&self.cosines(false));
        for _ in 0..3 {
            s.push(' ');
            s.push_str(&self.f(true));
        }
        s.push_str(&format!(" {} {}", self.int(0, 1), self.int(0, 2)));
        for _ in 0..5 {
            s.push(' ');
            s.push_str(&self.x());
        }
        let n = self.int(0, 2);
        s.push_str(&format!(" {n}"));
        for _ in 0..n {
            s.push(' ');
            s.push_str(&self.x());
        }
        for _ in 0..3 {
            s.push(' ');
            s.push_str(&self.m4(false, true, false));
        }
        s.push(' ');
        s.push_str(&self.ct(false));
        s.push_str(&format!(
            " {} {} {} {} {} {} {} {}",
            self.int(0, 1),
            self.f(false),
            self.f(false),
            self.f(false),
            self.int(0, 2),
            self.int(0, 1),
            self.d(true),
            self.f(false)
        ));
        s
    }

    /// X: a MINC transform.
    fn xfm(&mut self) -> String {
        if self.one_in(3) {
            return "none".to_owned();
        }
        let kind = self.pick(&["0", "0", "0", "1", "7"]);
        let mut s = format!("{kind} {}", self.int(0, 1));
        for _ in 0..32 {
            s.push(' ');
            s.push_str(&self.d(true));
        }
        s
    }

    /// B: a box's volume.
    fn b(&mut self) -> String {
        let v = self.v(&[0, 1, 3, 4, 10, 9], 0, false);
        let seed = if self.one_in(3) { -1 } else { self.int(0, 255) };
        let n = self.int(0, 3);
        let mut s = format!("{v} {seed} {n}");
        for _ in 0..n {
            s.push_str(&format!(
                " {} {} {} 0 {}",
                self.int(0, 2),
                self.int(0, 2),
                self.int(0, 2),
                self.pick(&["42b40000", "428c0000", "7fc00000", "bf800000", "4f000000"])
            ));
        }
        s
    }

    /// LT: a linear transform.
    fn lt(&mut self) -> String {
        format!(
            "{} {} {} {} {} {} {} {} {} {} {}",
            self.f(false),
            self.f(false),
            self.f(false),
            self.f(false),
            self.m4(false, true, true),
            self.m4(false, true, false),
            self.m4(false, true, false),
            self.int(-1, 2),
            self.int(0, 9),
            self.gf(),
            self.gf()
        )
    }

    /// LTA: an array of `n` transforms.
    fn lta(&mut self, n: i64) -> String {
        let mut s = format!("{n} {} {} {}", self.int(0, 2), self.f(false), self.x());
        for _ in 0..2 * n {
            s.push(' ');
            s.push_str(&self.lt());
        }
        s
    }
}

type Make = fn(&mut G) -> String;

const SIZED: &[i32] = &[0, 1, 2, 3, 4, 6, 7, 9, 10];
const ALL_TYPES: &[i32] = &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];

const ALLOC: &[(&str, Make)] = &[
    ("alloc", |g| g.v(ALL_TYPES, 2, true)),
    ("alloc", |g| format!("{} {} {} {} {} 1", g.int(1, 2), g.int(1, 2), g.int(1, 2), g.int(0, 10), g.int(-1, 2))),
    ("allocvg", |g| format!("{} {} 1 {}", g.g(true), g.pick(&["0", "3", "4"]), g.int(0, 1))),
    ("fromvg", |g| format!("{} {} 1 {}", g.g(true), g.pick(&["0", "3", "4"]), g.int(0, 1))),
    ("free2", |g| g.v(SIZED, 2, false)),
    ("sizeof", |g| format!("{}", g.int(-1, 12))),
    ("intent", |g| format!("{} {}", g.int(-1, 6), g.pick(&["x4d5249", "x5348415045", "x57415250", "x"]))),
];

const MATRICES: &[(&str, Make)] = &[
    ("xfm", |g| format!("{} {} {} {} {}", g.v(SIZED, 1, false), g.g(true), g.int(0, 1), g.int(0, 1), g.m4(true, true, true))),
    ("+gdiag=10000 xfm", |g| format!("{} {} {} 1 {}", g.v(SIZED, 1, false), g.g(true), g.int(0, 1), g.m4(false, true, false))),
    ("setv2r", |g| format!("{} {} {}", g.v(SIZED, 1, false), g.g(true), g.m4(true, false, true))),
    ("p0", |g| format!("{} {} {} {} {}", g.v(SIZED, 1, false), g.g(true), g.d(true), g.d(true), g.d(true))),
    ("reinit", |g| format!("{} {}", g.v(SIZED, 1, false), g.g(true))),
];

fn vox_ops(g: &mut G, n: i64) -> String {
    let mut s = String::new();
    for _ in 0..n {
        let at = |g: &mut G| {
            if g.one_in(5) {
                format!("{} {} {} {}", g.int(-1, 4), g.int(-1, 4), g.int(-1, 4), g.int(-1, 2))
            } else {
                format!("{} {} {} 0", g.int(0, 2), g.int(0, 2), g.int(0, 2))
            }
        };
        match g.below(5) {
            0 | 1 => s.push_str(&format!(" get {} {}", at(g), g.pick(&["0", "0", "0", "1", "1", "2"]))),
            2 | 3 => s.push_str(&format!(
                " set {} {} {}",
                at(g),
                g.pick(&["0", "0", "1"]),
                g.pick(&["437f8000", "c3000000", "4f000000", "cf000000", "7fc00000", "3fc00000", "bfc00000", "5f000000", "477fff00"])
            )),
            _ => s.push_str(&format!(" {} {}", g.pick(&["dx", "dy", "dz"]), at(g))),
        }
    }
    s
}

const VOXELS: &[(&str, Make)] = &[("vox", |g| {
    if g.one_in(12) {
        let v = g.v(&[0, 3], 1, false);
        return format!("{v} -1 0000000000000000 1 get 0 0 0 0 0");
    }
    let v = g.v(SIZED, 0, false);
    let seed = g.int(0, 255);
    let o = g.d(true);
    let n = g.int(1, 8);
    let ops = vox_ops(g, n);
    format!("{v} {seed} {o} {n}{ops}")
})];

const HEADERS: &[(&str, Make)] = &[
    ("copyhdr", |g| {
        let s = g.v(SIZED, 2, false);
        let seed = g.int(0, 255);
        let h = g.h();
        let d = if g.one_in(3) {
            "none".to_owned()
        } else {
            format!("{} {} {}", g.v(SIZED, 2, false), g.int(0, 255), g.h())
        };
        format!("{s} {seed} {h} {d}")
    }),
    ("+gdiag=10000040 copyhdr", |g| {
        format!("{} {} {} none", g.v(SIZED, 1, false), g.int(0, 255), g.h())
    }),
    ("copyxfm", |g| {
        let a = g.xfm();
        let b = if g.one_in(3) { "new".to_owned() } else { g.xfm() };
        format!("{a} {b}")
    }),
    ("pulse", |g| format!("{} {} {} {}", g.d(true), g.f(true), g.f(true), g.f(true))),
    ("ctab", |g| format!("{} {}", g.ct(false), g.int(0, 1))),
    ("ctabnull", |_| String::new()),
];

const GEOMETRY: &[(&str, Make)] = &[
    ("vgcopy", |g| g.gf()),
    ("vgassign", |g| format!("{} {}", g.gf(), g.gf())),
    ("usevg", |g| format!("{} {} {}", g.g(true), g.v(SIZED, 1, false), g.g(true))),
    ("vgtomri", |g| format!("{} {} {}", g.g(true), g.v(SIZED, 1, false), g.g(true))),
    ("vgfrommri", |g| format!("{} {} {}", g.v(SIZED, 1, false), g.g(true), g.g(true))),
    ("sphinx", |g| g.g(true)),
    ("tomat", |g| format!("{} {}", g.g(true), g.m4(true, true, true))),
];

const TKREG: &[(&str, Make)] = &[
    ("tkreg", |g| format!("{} {} {} {}", g.v(SIZED, 1, false), g.g(true), g.int(0, 1), g.int(0, 1))),
    ("ras2s", |g| format!("{} {} {}", g.v(SIZED, 1, false), g.g(true), g.m4(true, true, true))),
    ("crs1", |g| g.m4(false, true, true)),
];

fn two_volumes(g: &mut G) -> String {
    format!(
        "{} {} {} {}",
        g.v(SIZED, 1, false),
        g.g(true),
        g.v(SIZED, 1, false),
        g.g(true)
    )
}

const REGISTRATIONS: &[(&str, Make)] = &[
    ("tk2nat", |g| {
        let v = two_volumes(g);
        let sp = g.one_in(6);
        format!("{v} {}", g.m4(sp, true, true))
    }),
    ("nat2tk", |g| {
        let v = two_volumes(g);
        let sp = g.one_in(6);
        format!("{v} {}", g.m4(sp, true, true))
    }),
    ("v2v", |g| {
        let v = two_volumes(g);
        let sp = g.one_in(6);
        format!("{v} {}", g.m4(sp, true, true))
    }),
    ("tkv2v", |g| {
        let v = two_volumes(g);
        let sp = g.one_in(6);
        format!("{v} {}", g.m4(sp, true, true))
    }),
    ("fixtk", |g| format!("{} {} {} {}", g.v(SIZED, 1, false), g.g(true), g.int(0, 99), g.m4(false, true, true))),
    ("fsl2tk", |g| {
        let t = g.pick(&["x4e49465449", "x4e494654495f4741", "x4e494654495f505a", "x414e414c595a45", "unset"]);
        let m = if g.one_in(8) { "null".to_owned() } else { g.m4(false, false, true) };
        format!("{t} {} {m}", two_volumes(g))
    }),
    ("tk2fsl", |g| {
        let t = g.pick(&["x4e49465449", "x4e494654495f4741", "x414e414c595a45", "unset"]);
        let m = if g.one_in(8) { "null".to_owned() } else { g.m4(false, false, true) };
        format!("{t} {} {m}", two_volumes(g))
    }),
];

const COORDS: &[(&str, Make)] = &[("v2w", |g| {
    let v = g.v(SIZED, 1, false);
    let geo = g.g(true);
    let cache = g.int(0, 2);
    let n = g.int(1, 4);
    let mut s = format!("{v} {geo} {cache} {n}");
    for _ in 0..n {
        s.push_str(&format!(" {} {} {} {}", g.d(true), g.d(true), g.d(true), g.int(0, 1)));
    }
    s
})];

const PRINTING: &[(&str, Make)] = &[
    ("vgprint", |g| format!("{} {} {}", g.g(true), g.int(0, 1), g.int(0, 1))),
    ("vgeq", |g| {
        let a = g.g(false);
        let b = if g.one_in(2) { a.clone() } else { g.g(false) };
        let t = g.pick(&["0000000000000000", "3e80000000000000", "3fb999999999999a"]);
        format!("{a} {b} {t} {} {}", g.int(0, 1), g.m4(false, true, false))
    }),
    ("diffv2r", |g| {
        format!(
            "{} {} {} {} {}",
            g.m(4, 4, true, false),
            g.m(4, 4, true, false),
            g.pick(&["0000000000000000", "3fb999999999999a"]),
            g.x(),
            g.int(0, 1)
        )
    }),
    ("shearless", |g| format!("{} {}", g.v(SIZED, 1, false), g.g(true))),
];

const NAMES: &[(&str, Make)] = &[
    ("codes", |g| {
        format!(
            "{} {} {} {}",
            g.pick(&["x6e656172657374", "x54524931", "x746c69", "x53696e63", "x637562", "x6e65", "x"]),
            g.int(-1, 6),
            g.pick(&["x7563686172", "x53484f5254", "x7573687274", "x696e74", "x6c6f6e67", "x666c6f6174", "x666c"]),
            g.int(-1, 11)
        )
    }),
    ("shape", |g| {
        let mut s = String::new();
        for k in 0..8 {
            if k > 0 {
                s.push(' ');
            }
            s.push_str(&g.int(-1, 3).to_string());
        }
        s
    }),
];

const COPIES: &[(&str, Make)] = &[("copy", |g| {
    let (w, h, d, nf) = (g.int(1, 3), g.int(1, 3), g.int(1, 3), g.int(1, 2));
    let t = SIZED[g.below(SIZED.len() as u64) as usize];
    let hdr = if g.one_in(6) { 1 } else { 0 };
    let seed = g.int(0, 255);
    let h_fields = g.h();
    let dst = match g.below(8) {
        0 | 1 => "none".to_owned(),
        2 => format!("{} {} {}", g.v(SIZED, 2, false), g.int(-1, 255), g.int(0, 3)),
        _ => {
            let t2 = SIZED[g.below(SIZED.len() as u64) as usize];
            let dh = if g.one_in(8) { 1 } else { 0 };
            format!("{w} {h} {d} {t2} {nf} {dh} {} {}", g.int(-1, 255), g.int(0, 3))
        }
    };
    format!("{w} {h} {d} {t} {nf} {hdr} {seed} {h_fields} {dst}")
})];

const BBOX: &[(&str, Make)] = &[(
    "bbox",
    |g| format!("{} {}", g.b(), g.pick(&["70", "0", "-1", "255", "2147483647"])),
)];

const LTA: &[(&str, Make)] = &[
    ("ltaalloc", |g| {
        let n = g.int(-1, 2);
        let m = if g.one_in(2) { "none".to_owned() } else { g.b() };
        format!("{n} {m}")
    }),
    ("ltacopy", |g| {
        let n = g.int(0, 2);
        let a = g.lta(n);
        let d = match g.below(3) {
            0 => "none".to_owned(),
            1 => g.lta(n),
            _ => {
                let m = g.int(0, 2);
                g.lta(m)
            }
        };
        format!("{a} {d}")
    }),
    ("ltcopy", |g| {
        let a = g.lta(1);
        let d = if g.one_in(4) { "null".to_owned() } else { g.lta(1) };
        format!("{a} {d}")
    }),
    ("ltafreenull", |_| String::new()),
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
                let res = mri_cmd::run_line(&toks, mode, &env);
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
fn allocation() {
    group("alloc", ALLOC, 12);
}

#[test]
fn matrices() {
    group("matrices", MATRICES, 12);
}

#[test]
fn voxels() {
    group("voxels", VOXELS, 24);
}

#[test]
fn headers() {
    group("headers", HEADERS, 10);
}

#[test]
fn geometry() {
    group("geometry", GEOMETRY, 10);
}

#[test]
fn tkreg() {
    group("tkreg", TKREG, 12);
}

#[test]
fn registrations() {
    group("registrations", REGISTRATIONS, 10);
}

#[test]
fn coordinates() {
    group("coords", COORDS, 16);
}

#[test]
fn printing() {
    group("printing", PRINTING, 10);
}

#[test]
fn names_and_shapes() {
    group("names", NAMES, 16);
}

#[test]
fn copies() {
    group("copies", COPIES, 24);
}

#[test]
fn bounding_box() {
    group("bbox", BBOX, 16);
}

#[test]
fn transform_arrays() {
    group("lta", LTA, 10);
}
