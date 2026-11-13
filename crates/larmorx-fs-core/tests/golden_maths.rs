// SPDX-License-Identifier: Apache-2.0
//! Golden cases of fs.maths' clean room (`specs/freesurfer-maths.md` §23.4, CLAUDE.md rule 10):
//! every command of the black-box probe's grammar (`tests/maths_cmd/`, the comparison's candidate
//! commands) on seeded arguments, in both defect modes, one SHA-256 per group of the spec's
//! sections over every result line (the printed values, both streams and the notices; NaN bits
//! included), recorded on Linux x86-64 and checked on every platform. The arguments come from a
//! seeded generator and plain arithmetic (special values written as their bits); no file system
//! (the probe's file commands are left to the comparison; the matrix files' readers and writers
//! run on memory streams below), no clock.
//!
//! Some decompositions get finite arguments only (below).
//!
//! Record: `FS_CORE_GOLDEN_RECORD=1 cargo test -p larmorx-fs-core --test golden_maths --
//! --nocapture` prints `GOLDEN <case> <sha256>` for each case instead of checking it. The cases
//! also run under the AArch64 NaN emulation
//! (`validation/investigations/freesurfer-cleanroom-maths/README.md`).

mod maths_cmd;

use larmorx_core::defects::UpstreamDefects;
use larmorx_fs_core::maths::mfiles::CStream;
use larmorx_fs_core::{Environment, Process};
use sha2::{Digest, Sha256};

const GOLDEN: &[(&str, &str)] = &[
    (
        "decomp.fix",
        "85c473d42c3c94a444cc6197b4c7e1a53f62581b681f4d4b2afea38d4db95d3f",
    ),
    (
        "decomp.reproduce",
        "cfcc4d109729124b282218e98f47f3ad47508146db506efec9cc63c15579d3be",
    ),
    (
        "dmatrix.fix",
        "65fb7eed66dbf1c44fc6091b49e8d5a3f73b11bb27e6c357b5a24a1a54271be3",
    ),
    (
        "dmatrix.reproduce",
        "231715dd83d261781237daaf3401b5c58e904082e982b9331618bc3d8fcdb268",
    ),
    (
        "histo.fix",
        "3804eed7611541c15b93b7c91c98e1b007a3e2d570d494203b1391dc2f5bce98",
    ),
    (
        "histo.reproduce",
        "b50a9f1e65eb417c9b8a8ec61fe4de1feae36f7d890dfffc9e33236f590c5f3e",
    ),
    (
        "matrix-old.fix",
        "4aa79716a27e9671f8e445c1a9594454beee82b48c539064b3c602fda3752973",
    ),
    (
        "matrix-old.reproduce",
        "15a6f20dfbb5e2ed9b67a49f53af9644d90b7badffdb93d8dcaadd9e73d7e05b",
    ),
    (
        "matrix.fix",
        "fe42ec3eee73032bd32c6e39d5ddc886ffb4daf41918995b854e00498a7ef6e3",
    ),
    (
        "matrix.reproduce",
        "6c74b2cefcde834a48540f40cdf06bd5fcf568043abc6749f685e91c0487964c",
    ),
    (
        "mfiles.fix",
        "8f69297b98f48496c00a7068c9d094d2fa72f99a722d0fe7d442ed4a48d1dc40",
    ),
    (
        "mfiles.reproduce",
        "179d3575838bdd149ff97aed98c5272853f86ae59c769d0a25849a9c42b92d29",
    ),
    (
        "minimizers.fix",
        "bc37efee40b936b0fafddd0c644933b21ba51447db219a8ec1e35fe349f2b5ab",
    ),
    (
        "minimizers.reproduce",
        "6b7b92a7126524bdaa84890c523e9297a4e42cf808442498b7e2063835906468",
    ),
    (
        "misc.fix",
        "dab7dfde8f3d9ec4bce5e2aa89b6924f306b81d1a3a523c34674b9c25460b265",
    ),
    (
        "misc.reproduce",
        "dab7dfde8f3d9ec4bce5e2aa89b6924f306b81d1a3a523c34674b9c25460b265",
    ),
];

/// Checks (or, with `FS_CORE_GOLDEN_RECORD` set, prints) a case's hash; with
/// `FS_CORE_GOLDEN_DUMP=<folder>` also writes the case's bytes there (`<case>.txt`).
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

    /// A float's bits: special values (with `special`), small integers, ratios.
    fn fbits(&mut self, special: bool) -> u32 {
        if special && self.one_in(6) {
            const S: [u32; 9] = [
                0x0000_0000,
                0x8000_0000,
                0x7f80_0000,
                0xff80_0000,
                0x7fc0_0000,
                0xffc0_0000,
                0x7fa0_0001,
                0x7f7f_ffff,
                0x0000_0001,
            ];
            return S[self.below(9) as usize];
        }
        let v = match self.below(4) {
            0 => self.int(-8, 8) as f32,
            1 => self.int(-64, 64) as f32 / 8.0,
            2 => self.int(-1000, 1000) as f32 / 7.0,
            _ => self.int(0, 20) as f32,
        };
        v.to_bits()
    }

    fn f(&mut self, special: bool) -> String {
        format!("{:08x}", self.fbits(special))
    }

    /// A double's bits, as [`G::fbits`].
    fn d(&mut self, special: bool) -> String {
        if special && self.one_in(6) {
            const S: [u64; 7] = [
                0,
                0x8000_0000_0000_0000,
                0x7ff0_0000_0000_0000,
                0xfff0_0000_0000_0000,
                0x7ff8_0000_0000_0000,
                0xfff8_0000_0000_0000,
                0x7fef_ffff_ffff_ffff,
            ];
            return format!("{:016x}", S[self.below(7) as usize]);
        }
        let v = match self.below(3) {
            0 => self.int(-8, 8) as f64,
            1 => self.int(-1000, 1000) as f64 / 7.0,
            _ => self.int(0, 400) as f64 / 4.0,
        };
        format!("{:016x}", v.to_bits())
    }

    fn fs(&mut self, n: usize, special: bool) -> String {
        (0..n)
            .map(|_| self.f(special))
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// A histogram argument: `<alloc> <nbins> <bin_size> <min> <max> <bins> <counts>`.
    fn h(&mut self, special: bool) -> String {
        let nbins = self.int(0, 9) as usize;
        let alloc = nbins as i64 + self.int(0, 2);
        let size = [1.0f32, 0.5, 2.0, 4.0][self.below(4) as usize];
        let min = self.int(-10, 10) as f32;
        let max = min + size * nbins as f32;
        let mut t = format!(
            "{alloc} {nbins} {:08x} {:08x} {:08x}",
            size.to_bits(),
            min.to_bits(),
            max.to_bits()
        );
        let sorted = !self.one_in(4);
        for i in 0..nbins {
            if sorted {
                t += &format!(" {:08x}", (min + size * i as f32).to_bits());
            } else {
                t += &format!(" {}", self.f(special));
            }
        }
        for _ in 0..nbins {
            if self.one_in(5) {
                t += &format!(" {}", self.f(special));
            } else {
                t += &format!(" {:08x}", (self.int(0, 30) as f32).to_bits());
            }
        }
        t
    }

    /// A histogram output argument.
    fn oh(&mut self, special: bool) -> String {
        match self.below(5) {
            0 => "new".into(),
            1 => "self".into(),
            2 => "other".into(),
            _ => self.h(special),
        }
    }

    /// A float matrix argument of the given type and size.
    fn mk(&mut self, kind: i64, rows: i64, cols: i64, special: bool) -> String {
        let n = (rows * cols * if kind == 2 { 2 } else { 1 }).max(0) as usize;
        let v = self.fs(n, special);
        format!("{kind} {rows} {cols}{}{v}", if n > 0 { " " } else { "" })
    }

    /// A float matrix argument: mostly real, 1 to 4 rows and columns.
    fn m(&mut self, special: bool) -> String {
        let kind = if self.one_in(5) { 2 } else { 1 };
        let (r, c) = (self.int(1, 4), self.int(1, 4));
        self.mk(kind, r, c, special)
    }

    /// A real square matrix of 1 to 4 rows.
    fn msq(&mut self, special: bool) -> String {
        let n = self.int(1, 4);
        self.mk(1, n, n, special)
    }

    /// A float matrix output argument.
    fn om(&mut self, special: bool) -> String {
        match self.below(5) {
            0 => "new".into(),
            1 => "self".into(),
            2 => "other".into(),
            _ => self.m(special),
        }
    }

    /// A double matrix argument.
    fn dm(&mut self, special: bool) -> String {
        let (r, c) = (self.int(1, 4), self.int(1, 4));
        let v = (0..r * c).map(|_| self.d(special)).collect::<Vec<_>>();
        format!("{r} {c} {}", v.join(" "))
    }

    fn od(&mut self, special: bool) -> String {
        match self.below(5) {
            0 => "new".into(),
            1 => "self".into(),
            2 => "other".into(),
            _ => self.dm(special),
        }
    }

    /// A double matrix output argument of a one-input command (no `other`).
    fn od1(&mut self, special: bool) -> String {
        match self.below(4) {
            0 => "new".into(),
            1 => "self".into(),
            _ => self.dm(special),
        }
    }

    fn stream(&mut self) -> &'static str {
        ["out", "err", "null"][self.below(3) as usize]
    }

    fn x(&mut self, s: &[u8]) -> String {
        maths_cmd::hexbytes(s)
    }

    /// A test cost: `<kind> <n> <f0> <b> <q> <a[1..n]> <c[1..n]>`, n from 1 to 4.
    fn cost(&mut self) -> (String, usize) {
        let kind = self.int(0, 3);
        let n = self.int(1, 4) as usize;
        let f0 = (self.int(-2, 2) as f32).to_bits();
        let b = [0.0f32, 0.25, -0.3][self.below(3) as usize].to_bits();
        let q = [4.0f32, 16.0, 64.0][self.below(3) as usize].to_bits();
        let a: Vec<String> = (0..n)
            .map(|_| format!("{:08x}", (self.int(1, 6) as f32 / 2.0).to_bits()))
            .collect();
        let c = self.fs(n, false);
        (
            format!("{kind} {n} {f0:08x} {b:08x} {q:08x} {} {c}", a.join(" ")),
            n,
        )
    }

    /// `n` directions: 1 and the identity (or a sheared one), or 0 and none.
    fn dirs(&mut self, n: usize) -> String {
        if self.one_in(5) {
            return "0".into();
        }
        let mut t = "1".to_string();
        for r in 0..n {
            for c in 0..n {
                let v: f32 = if r == c {
                    1.0
                } else if c == r + 1 && self.one_in(2) {
                    0.5
                } else {
                    0.0
                };
                t += &format!(" {:08x}", v.to_bits());
            }
        }
        t
    }
}

type Make = fn(&mut G) -> String;

/// The histograms (§5, §21).
const HISTO: &[(&str, Make)] = &[
    ("halloc", |g| format!("{}", g.int(0, 12))),
    ("hrealloc", |g| format!("{} {}", g.h(true), g.int(0, 12))),
    ("hfree", |g| g.h(true)),
    ("hcopy", |g| format!("{} {}", g.h(true), g.oh(true))),
    ("hclear", |g| format!("{} {}", g.h(true), g.oh(true))),
    ("hclearcounts", |g| format!("{} {}", g.h(true), g.oh(true))),
    ("hpdf", |g| format!("{} {}", g.h(true), g.oh(true))),
    ("habs", |g| format!("{} {}", g.h(true), g.oh(true))),
    ("hcdf", |g| format!("{} {}", g.h(true), g.oh(true))),
    ("hrcdf", |g| format!("{} {}", g.h(true), g.oh(true))),
    ("hcumsum", |g| format!("{} {}", g.h(true), g.oh(true))),
    ("hinit", |g| {
        format!(
            "{} {} {} {}",
            g.oh(false),
            g.int(-1, 10),
            g.d(true),
            g.d(true)
        )
    }),
    ("hbins", |g| {
        format!("{} {} {}", g.int(-1, 10), g.d(true), g.d(true))
    }),
    ("hadd", |g| {
        format!("{} {} {}", g.h(true), g.h(true), g.oh(true))
    }),
    ("hsub", |g| {
        format!("{} {} {}", g.h(true), g.h(true), g.oh(true))
    }),
    ("hscale", |g| {
        format!("{} {} {} {}", g.h(true), g.oh(true), g.f(true), g.f(true))
    }),
    ("hclearbins", |g| {
        format!(
            "{} {} {} {}",
            g.h(true),
            g.oh(true),
            g.int(-2, 10),
            g.int(-2, 10)
        )
    }),
    ("hsoap", |g| {
        format!("{} {} {}", g.h(false), g.oh(false), g.int(1, 30))
    }),
    ("hsmooth", |g| {
        format!("{} {} {}", g.h(true), g.oh(true), g.f(true))
    }),
    ("hsumnorm", |g| g.h(true)),
    ("hfill", |g| g.h(true)),
    ("hclearzero", |g| g.h(true)),
    ("hlastpeak", |g| {
        format!("{} {} {}", g.h(true), g.int(0, 4), g.f(true))
    }),
    ("hfirstpeak", |g| {
        format!("{} {} {}", g.h(true), g.int(0, 4), g.f(true))
    }),
    ("hvalley", |g| {
        format!(
            "{} {} {} {}",
            g.h(true),
            g.int(0, 4),
            g.int(-2, 10),
            g.int(-2, 10)
        )
    }),
    ("hlastpeakin", |g| {
        format!(
            "{} {} {} {} {}",
            g.h(true),
            g.int(0, 4),
            g.f(true),
            g.int(-2, 10),
            g.int(-2, 10)
        )
    }),
    ("hhighest", |g| {
        format!("{} {} {}", g.h(true), g.int(-2, 10), g.int(-2, 10))
    }),
    ("hprevvalley", |g| {
        format!("{} {}", g.h(true), g.int(-2, 10))
    }),
    ("hnextvalley", |g| {
        format!("{} {}", g.h(true), g.int(-2, 10))
    }),
    ("hnextpeak", |g| {
        format!("{} {} {}", g.h(true), g.int(-2, 10), g.int(0, 4))
    }),
    ("hprevpeak", |g| {
        format!("{} {} {}", g.h(true), g.int(-2, 10), g.int(0, 4))
    }),
    ("hstartpeak", |g| {
        format!("{} {} {}", g.h(true), g.int(0, 4), g.f(true))
    }),
    ("hendpeak", |g| {
        format!("{} {} {}", g.h(true), g.int(0, 4), g.f(true))
    }),
    ("hcurrentpeak", |g| {
        format!(
            "{} {} {} {}",
            g.h(true),
            g.int(-2, 10),
            g.int(0, 4),
            g.f(true)
        )
    }),
    ("hfindbin", |g| format!("{} {}", g.h(true), g.f(true))),
    ("hvaltobin", |g| format!("{} {}", g.h(true), g.d(true))),
    ("hcount", |g| {
        let n = g.int(0, 6) as usize;
        let v: Vec<String> = (0..n).map(|_| g.d(true)).collect();
        format!("{} {n} {}", g.h(true), v.join(" "))
    }),
    ("haddsample", |g| {
        let n = g.int(0, 6) as usize;
        format!(
            "{} {n} {} {} {}",
            g.h(true),
            g.fs(n, true),
            g.f(true),
            g.f(true)
        )
    }),
    ("htotal", |g| {
        format!("{} {} {}", g.h(true), g.int(-2, 10), g.int(-2, 10))
    }),
    ("hthresh", |g| {
        format!("{} {} {}", g.h(true), g.h(true), g.f(true))
    }),
    ("hmedian", |g| g.h(true)),
    ("hentropy", |g| g.h(true)),
    ("hcorrelate", |g| format!("{} {}", g.h(true), g.h(true))),
    ("hlinfit", |g| {
        // The grid search's ranges as the comparison's (a few hundred points).
        let (a0, a1): (f64, f64) = [(0.5, 2.0), (1.0, 1.0), (0.9, 1.1)][g.below(3) as usize];
        let (b0, b1): (f64, f64) = [(-3.0, 3.0), (0.0, 0.0), (-1.0, 1.0)][g.below(3) as usize];
        format!(
            "{} {} {:016x} {:016x} {:016x} {:016x}",
            g.h(true),
            g.h(true),
            a0.to_bits(),
            a1.to_bits(),
            b0.to_bits(),
            b1.to_bits()
        )
    }),
    ("hfwhm", |g| format!("{} {}", g.h(true), g.int(-1, 10))),
    ("hrobust", |g| {
        let q = [0.5f64, 0.9, 0.3][g.below(3) as usize];
        format!("{} {:016x}", g.h(false), q.to_bits())
    }),
    ("h2d", |g| {
        let (n1, n2) = (g.int(1, 4) as usize, g.int(1, 4) as usize);
        format!(
            "{n1} {n2} {} {} {} {} {} {} {} {} {}",
            g.f(true),
            g.f(true),
            g.f(true),
            g.f(true),
            g.fs(n1, true),
            g.fs(n2, true),
            g.fs(n1 * n2, true),
            g.f(true),
            g.f(true)
        )
    }),
];

/// The float and double matrices and the affine helpers (§2 to §4, §8.1 to §8.7, §12, §16).
const MATRIX: &[(&str, Make)] = &[
    ("malloc", |g| {
        format!("{} {} {}", g.int(-1, 4), g.int(-1, 4), g.int(0, 3))
    }),
    ("malloc2", |g| {
        format!("{} {} {}", g.int(0, 4), g.int(0, 4), g.int(1, 2))
    }),
    ("mfree", |g| g.m(true)),
    ("mcopy", |g| format!("{} {}", g.m(true), g.om(true))),
    ("mmuld", |g| {
        format!("{} {} {}", g.m(true), g.m(true), g.om(true))
    }),
    ("mmul", |g| {
        format!("{} {} {}", g.m(true), g.m(true), g.om(true))
    }),
    ("affine", |g| {
        let (a, b) = (g.mk(1, 4, 4, true), g.mk(1, 4, 4, true));
        format!("{a} {b} {}", g.fs(4, true))
    }),
    ("affineset", |g| g.mk(1, 4, 4, true)),
    ("affineget", |g| {
        format!("{} {}", g.mk(1, 4, 4, true), g.fs(16, true))
    }),
    ("midentity", |g| format!("{} {}", g.int(0, 4), g.om(false))),
    ("mtranspose", |g| format!("{} {}", g.m(true), g.om(true))),
    ("msquare", |g| format!("{} {}", g.m(true), g.om(true))),
    ("mdiag", |g| format!("{} {}", g.m(true), g.om(true))),
    ("mclone", |g| g.m(true)),
    ("madd", |g| {
        format!("{} {} {}", g.m(true), g.m(true), g.om(true))
    }),
    ("msub", |g| {
        format!("{} {} {}", g.m(true), g.m(true), g.om(true))
    }),
    ("mscale", |g| {
        format!("{} {} {}", g.m(true), g.om(true), g.f(true))
    }),
    ("mclear", |g| g.m(true)),
    ("msymmetric", |g| g.msq(true)),
    ("miszero", |g| g.m(true)),
    ("mcheck", |g| g.m(true)),
    ("mtrace", |g| g.m(true)),
    ("msum", |g| g.m(true)),
    ("mconst", |g| {
        format!(
            "{} {} {} {}",
            g.f(true),
            g.int(0, 4),
            g.int(0, 4),
            g.om(false)
        )
    }),
    ("mzero", |g| {
        format!("{} {} {}", g.int(0, 4), g.int(0, 4), g.om(false))
    }),
    ("mreshape", |g| {
        format!(
            "{} {} {} {}",
            g.m(true),
            g.om(true),
            g.int(0, 5),
            g.int(0, 5)
        )
    }),
    ("mcolscale", |g| format!("{} {}", g.m(true), g.om(true))),
    ("mnormcol", |g| {
        format!("{} {} {}", g.m(true), g.om(true), g.om(true))
    }),
    ("mregion", |g| {
        let r: Vec<String> = (0..6).map(|_| g.int(-1, 4).to_string()).collect();
        format!("{} {} {}", g.m(true), g.om(true), r.join(" "))
    }),
    ("mrealregion", |g| {
        let r: Vec<String> = (0..6).map(|_| g.int(-1, 4).to_string()).collect();
        format!("{} {} {}", g.m(true), g.om(true), r.join(" "))
    }),
    ("mimagregion", |g| {
        let r: Vec<String> = (0..6).map(|_| g.int(-1, 4).to_string()).collect();
        format!("{} {} {}", g.m(true), g.om(true), r.join(" "))
    }),
    ("mr2c", |g| {
        format!("{} {} {}", g.m(true), g.m(true), g.om(true))
    }),
    ("mrotation", |g| {
        format!(
            "{} {} {} {}",
            g.int(1, 4),
            g.f(true),
            g.int(0, 3),
            g.om(false)
        )
    }),
    ("vlen", |g| g.m(true)),
    ("vdot", |g| format!("{} {}", g.m(true), g.m(true))),
    ("vdist", |g| format!("{} {}", g.m(true), g.m(true))),
    ("vcross", |g| {
        format!("{} {} {}", g.m(true), g.m(true), g.om(true))
    }),
    ("vtriple", |g| {
        format!("{} {} {}", g.m(true), g.m(true), g.m(true))
    }),
    ("v3angle", |g| format!("{} {}", g.m(true), g.m(true))),
    ("xyznorm", |g| g.fs(3, true)),
    ("xyzangle", |g| g.fs(7, true)),
    ("mprint", |g| {
        let s = g.stream();
        format!("{s} {} {}", g.m(true), g.int(0, 8))
    }),
    ("mprintfmt", |g| {
        let f = [&b"%f "[..], b"%8.3e", b"%g|", b"", b"%+5.1f\t"][g.below(5) as usize];
        format!("{} {}", g.x(f), g.m(true))
    }),
    ("msimilarity", |g| {
        format!("{} {} {}", g.m(true), g.m(true), g.om(true))
    }),
    ("mgaussian", |g| {
        format!(
            "{} {} {} {}",
            g.int(0, 7),
            g.f(true),
            g.int(0, 1),
            g.om(false)
        )
    }),
    ("mgaussian2", |g| {
        format!(
            "{} {} {} {} {} {}",
            g.int(0, 7),
            g.f(true),
            g.f(true),
            g.f(true),
            g.int(0, 1),
            g.om(false)
        )
    }),
];

const DMATRIX: &[(&str, Make)] = &[
    ("dalloc", |g| {
        format!("{} {} {}", g.int(-1, 4), g.int(-1, 4), g.int(0, 2))
    }),
    ("dcheck", |g| {
        let (a, b) = (g.dm(true), g.dm(true));
        let k = g.int(0, 3);
        let s = g.stream();
        format!("{a} {b} {k} {s} {}", g.x(b"pre"))
    }),
    ("daddmul", |g| {
        format!(
            "{} {} {} {} {}",
            g.dm(true),
            g.dm(true),
            g.od(true),
            g.d(true),
            g.d(true)
        )
    }),
    ("dadd", |g| {
        format!("{} {} {}", g.dm(true), g.dm(true), g.od(true))
    }),
    ("dsub", |g| {
        format!("{} {} {}", g.dm(true), g.dm(true), g.od(true))
    }),
    ("dscale", |g| {
        format!("{} {} {}", g.dm(true), g.od1(true), g.d(true))
    }),
    ("dcopy", |g| format!("{} {}", g.dm(true), g.od1(true))),
    ("dtranspose", |g| format!("{} {}", g.dm(true), g.od1(true))),
    ("dprint", |g| {
        let f = [&b"%lf "[..], b"%7.2f", b"", b"%g,"][g.below(4) as usize];
        format!("{} {}", g.x(f), g.dm(true))
    }),
    ("dmul", |g| {
        format!("{} {} {}", g.dm(true), g.dm(true), g.od(true))
    }),
    ("ddot", |g| format!("{} {}", g.dm(true), g.dm(true))),
    ("dcmpf", |g| {
        let (r, c) = (g.int(1, 3), g.int(1, 3));
        let m = g.mk(1, r, c, true);
        let v: Vec<String> = (0..r * c).map(|_| g.d(true)).collect();
        format!("{m} {r} {c} {}", v.join(" "))
    }),
    ("dcopyf", |g| format!("{} {}", g.m(true), g.od(true))),
    ("dconst", |g| {
        format!(
            "{} {} {} {}",
            g.d(true),
            g.int(0, 4),
            g.int(0, 4),
            g.od(false)
        )
    }),
    ("dzero", |g| {
        format!("{} {} {}", g.int(0, 4), g.int(0, 4), g.od(false))
    }),
    ("dmaxabs", |g| g.dm(true)),
    ("dliststats", |g| {
        let n = g.int(0, 6) as usize;
        let v: Vec<String> = (0..n).map(|_| g.d(true)).collect();
        format!("{n} {}", v.join(" "))
    }),
];

/// The decompositions (§8.8, §17). The inverse, determinant and QR factors and the eigensystems
/// of `OpenEigenSystem` and `OpenNonSymmetricEigenSystem` (VNL's routines in the main package,
/// whose NaN results are not yet the official build's on every platform and build) get finite
/// arguments only.
const DECOMP: &[(&str, Make)] = &[
    ("minverse", |g| format!("{} {}", g.msq(false), g.om(false))),
    ("mdet", |g| g.msq(false)),
    ("mqr", |g| {
        let n = g.int(1, 4);
        let a = g.mk(1, n, n, false);
        let q = g.mk(1, n, n, false);
        let r = g.mk(1, n, n, false);
        format!("{a} {q} {r}")
    }),
    ("oeigen", |g| {
        let n = g.int(1, 4) as usize;
        format!("{n} {}", g.fs(n * n, false))
    }),
    ("oeigenns", |g| {
        let n = g.int(1, 4) as usize;
        format!("{n} {}", g.fs(n * n, false))
    }),
    ("meigen", |g| format!("{} {}", g.msq(true), g.om(true))),
    ("meigenhelper", |g| {
        format!("{} {} {}", g.msq(true), g.int(0, 1), g.om(true))
    }),
    ("msvd", |g| {
        let (r, c) = (g.int(1, 4), g.int(1, 4));
        let a = g.mk(1, r, c, true);
        let w = g.mk(1, 1, c, true);
        format!("{a} {w} {}", g.om(true))
    }),
    ("msvdvalues", |g| format!("{} {}", g.m(true), g.int(0, 5))),
    ("mcond", |g| g.msq(true)),
    ("mnscond", |g| g.m(true)),
    ("msvdinverse", |g| format!("{} {}", g.m(true), g.om(true))),
    ("mpinverse", |g| format!("{} {}", g.m(true), g.om(true))),
    ("msvdpinverse", |g| format!("{} {}", g.m(true), g.om(true))),
];

/// Primes, draws, Numerical Recipes' arrays and the random-field specifications (§9, §10, §14,
/// §19).
const MISC: &[(&str, Make)] = &[
    ("primes", |g| format!("{}", g.int(-1, 200))),
    ("pfactors", |g| format!("{}", g.int(-1, 2000))),
    ("pclosest", |g| {
        format!("{} {}", g.int(1, 500), g.int(-1, 40))
    }),
    ("pclosestless", |g| {
        format!("{} {}", g.int(1, 500), g.int(-1, 40))
    }),
    ("gaussdraws", |g| {
        format!("{} {}", g.int(0, 1_000_000), g.int(0, 6))
    }),
    ("nrvector", |g| {
        let nl = g.int(-2, 3);
        format!("{nl} {}", nl + g.int(0, 4))
    }),
    ("nrmatrix", |g| {
        let (r0, c0) = (g.int(-1, 2), g.int(-1, 2));
        format!("{r0} {} {c0} {}", r0 + g.int(0, 3), c0 + g.int(0, 3))
    }),
    ("rfname", |g| {
        let n = [
            &b"gaussian"[..],
            b"uniform",
            b"erlang",
            b"poisson",
            b"bogus",
            b"",
        ][g.below(6) as usize];
        g.x(n)
    }),
    ("rfinit", |g| {
        format!("{} {}", g.int(1, 99_999), g.int(0, 99_999))
    }),
];

/// The minimizers (§13).
const MINIMIZERS: &[(&str, Make)] = &[
    ("powell", |g| {
        let (c, n) = g.cost();
        let x = g.fs(n, false);
        let d = g.dirs(n);
        format!("{c} {x} {d} {} {}", g.f(false), g.int(0, 1))
    }),
    ("powell2", |g| {
        let (c, n) = g.cost();
        let x = g.fs(n, false);
        let d = g.dirs(n);
        let ftol = [1e-4f32, 1e-2, 1e-6][g.below(3) as usize].to_bits();
        let lin = [1e-4f32, 1e-2][g.below(2) as usize].to_bits();
        let maxit = [1, 4, 36, 100][g.below(4) as usize];
        format!(
            "{c} {x} {d} {ftol:08x} {lin:08x} {maxit} {} {}",
            g.int(0, 1),
            g.int(0, 1)
        )
    }),
    ("dfpmin", |g| {
        let (c, n) = g.cost();
        let x = g.fs(n, false);
        let tol = [1e-4f32, 1e-2, 0.1][g.below(3) as usize].to_bits();
        let fret = [0.0f32, 50.0, 1e6, -1.0][g.below(4) as usize].to_bits();
        format!(
            "{c} {x} {tol:08x} {fret:08x} {} {} {}",
            g.int(0, 1),
            g.int(0, 1),
            g.int(0, 1)
        )
    }),
];

/// Every command of `table` `per` times (seeded by the group's name), in both modes: the result
/// lines, hashed per mode.
fn group(name: &str, table: &[(&str, Make)], per: usize, env: &Environment) {
    let seed = name.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3)
    });
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
                let res = maths_cmd::run_line(&toks, mode, env);
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
fn histograms() {
    group("histo", HISTO, 10, &Environment::new());
}

#[test]
fn matrices() {
    group("matrix", MATRIX, 10, &Environment::new());
    // The old allocator chosen from the environment (§2.2).
    let mut env = Environment::new();
    env.set(b"FREESURFER_MatrixAlloc_old", b"1");
    group("matrix-old", &MATRIX[..6], 6, &env);
}

#[test]
fn double_matrices() {
    group("dmatrix", DMATRIX, 10, &Environment::new());
}

#[test]
fn decompositions() {
    group("decomp", DECOMP, 10, &Environment::new());
}

#[test]
fn primes_draws_arrays_fields() {
    group("misc", MISC, 8, &Environment::new());
}

#[test]
fn minimizers() {
    group("minimizers", MINIMIZERS, 6, &Environment::new());
}

/// The matrix files' readers and writers on memory streams (§18), in both modes.
#[test]
fn matrix_files() {
    let mut g = G(0x5eed);
    for (mode, tag) in [
        (UpstreamDefects::Fix, "fix"),
        (UpstreamDefects::Reproduce, "reproduce"),
    ] {
        let mut out = Vec::new();
        let mut p = Process::silent(Environment::new(), mode);
        for _ in 0..12 {
            let args = g.m(true);
            let toks: Vec<String> = args.split_whitespace().map(str::to_owned).collect();
            let mut l = maths_cmd::Line::new(toks);
            let Ok(Some(m)) = l.getm(&mut p) else {
                continue;
            };
            for binary in [false, true] {
                let bytes = if binary {
                    p.matrix_write_into(&m)
                } else {
                    p.matrix_ascii_write(&m)
                };
                out.extend_from_slice(&bytes);
                // Read back whole, then cut short.
                for cut in [bytes.len(), bytes.len() / 2, 3] {
                    let mut s = CStream::new(bytes[..cut.min(bytes.len())].to_vec());
                    p.errno = 0;
                    let r = if binary {
                        p.matrix_read_from(&mut s, None)
                    } else {
                        p.matrix_ascii_read_from(&mut s, None)
                    };
                    let mut l = maths_cmd::Line::new(Vec::new());
                    match r {
                        Ok(r) => l.putm_res(&r, None),
                        Err(e) => l.out(format!("EXIT {}", e.status())),
                    }
                    l.puti(i64::from(p.errno));
                    l.puti(s.pos as i64);
                    out.extend_from_slice(l.result().as_bytes());
                    out.push(b'\n');
                }
            }
        }
        for n in p.take_notices().as_slice() {
            out.extend_from_slice(n.line().as_bytes());
            out.push(b'\n');
        }
        check(&format!("mfiles.{tag}"), &out);
    }
}
