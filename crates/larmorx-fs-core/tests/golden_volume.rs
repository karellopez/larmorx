// SPDX-License-Identifier: Apache-2.0
//! Golden cases of fs.volume's clean room (`specs/freesurfer-volume.md` §69.5, CLAUDE.md rule
//! 10): every command of the black-box probe's grammar (`tests/volume_cmd/`, the comparison's
//! candidate commands) on seeded arguments, in both defect modes, one SHA-256 per group of the
//! spec's parts 1 to 7b, of §54's change of a transform array's type and of fs.bspline
//! (`specs/freesurfer-bspline.md`) over every result line (the printed values, both streams and the notices;
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
        "21c898491f6aed35f86d925c22490801e1e4084b75992a47828732f167f93e52",
    ),
    (
        "bspline.reproduce",
        "affbe200de469b9e60a05c0e57834a8930e3618bdc76de6f83345073b240dded",
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
        "45094de4e58042c463941a205be23adab83705ae5c13d5dbd3f11fe12d330318",
    ),
    (
        "info.reproduce",
        "1948f6ffc67df060955b7fbee088894926114571929e42dbf544fbe671f47793",
    ),
    (
        "image.fix",
        "f01c640e5c3fcecac2e5ac8634fbea2930eb4309c73d37b6e9337a9bd411d402",
    ),
    (
        "image.reproduce",
        "cf9f477daf76bd4dd6568eaec0866341e3de05ec30ad2ab3517e589c5f8685e6",
    ),
    (
        "nbhd.fix",
        "9a47cd3c257e73a85406088f7f0b5cd5c71401fe52ee808c3e97b0fbb6d74a81",
    ),
    (
        "nbhd.reproduce",
        "9cf501836aef91a56c196c40474c0ae58337110e7d67a3e787ce50e671156550",
    ),
    (
        "ltapv.fix",
        "20fa987393d707a6c45de7a3b731a9b1e1acd7977a8b5ec24c0a4738ba66cd07",
    ),
    (
        "ltapv.reproduce",
        "39efdf79eb4a39d4353631d0fc2f208da784b394840efc9007358eb6554e7745",
    ),
    (
        "ltatypes.fix",
        "b2bc745208eec2a832b8ccf6b89023960999e2d2c64b3e0471d91a0d7d73c750",
    ),
    (
        "fr8.fix",
        "bec60a69764b3de153b8bd6bf670f902466d7a54752d2a25de372f744cdff9c8",
    ),
    (
        "fr8.reproduce",
        "e810e59a1851774a844cd7a1648f78aa4881799c42471df73660d59aeeb5aa5d",
    ),
    (
        "vox8.fix",
        "fc9f3142c82828389754d607f0f90c36af568d4f6abae6c21421373f86f96818",
    ),
    (
        "vox8.reproduce",
        "ca5889e40a68782916e1027e931c1d813f2ffcb5af6dd7b3fc0f1e82de49e6e0",
    ),
    (
        "ltatypes.reproduce",
        "078a4d6b39e0066c95ae84f37d034204af6e9ae64226197c73bdb93f1da835ac",
    ),
    (
        "lta54.fix",
        "81ade48d5a99463e67434bd76b30e60e650e20d42201d90b3bdf2d86086e46a4",
    ),
    (
        "lta54.reproduce",
        "93d1fa98179bae5b9497845e15ecd94a83158f55694febe75de5d70adf0dd595",
    ),
    (
        "segs57.fix",
        "178c5e4e37fbda6bbda8b3819c5e13695af0b50d9685fe9e2e4694d91a0f2392",
    ),
    (
        "segs57.reproduce",
        "e89796efaca01f7295f895553d65559d74726c9ba4dba75af0497539498d1568",
    ),
    (
        "part9.fix",
        "5f4f7b5c791b57e172ab378bad0f1d6e0a172a71a6761eb7f779de89030cfa50",
    ),
    (
        "part9.reproduce",
        "4812be080c938fff62a644c69092d9e347faa3f47ad11d771d862440275baf13",
    ),
    (
        "part10.fix",
        "717c089bd04f4f31c7c270255a014601f45b69d635a55970d30727727c652c1a",
    ),
    (
        "part10.reproduce",
        "ba929f5fd03d002eb6a6341ba8836a082b4121a44b34825a28b1291097cba075",
    ),
    (
        "part10b.fix",
        "19d5ff20c93d5a5c1f678aa91c58069a3228e420b2477c6e88d41a70dea9f532",
    ),
    (
        "part10b.reproduce",
        "791be1f5596d4af53232de9ddb39c72d91b232bc5201a59fa205030ed61dce13",
    ),
    (
        "part10c.fix",
        "e9383ae410dc37f88ab78653883fd1d3bc028884bce54da54ab628ed39f00b40",
    ),
    (
        "part10c.reproduce",
        "4dda0a62e8665c8d22b32d3b607b67cef03683cfdd59c1c9f42ce250b8186972",
    ),
    (
        "part11a.fix",
        "43bb797bd521343d77d133fe193bc04f84c79c21d6fbeccb999561dfb740e274",
    ),
    (
        "part11a.reproduce",
        "9a9295c376090091ec10dd98c950672c454646a23e020c3135f0469e0d60b323",
    ),
    (
        "part11b.fix",
        "b27c8dfa564760c973de32c277d33970d1ae234bdc3e02be2d3c05c57b335996",
    ),
    (
        "part11b.reproduce",
        "ee5d16a95fb5770acb84fc410bc1a7f5b861fa71eeba0f59e009b4273431df39",
    ),
    (
        "part11c.fix",
        "a1613f76d512c31ebb799ca6f8279ffff9548ae59b020cd8e52b7ace33261c23",
    ),
    (
        "part11c.reproduce",
        "8c6ddaf3e3c3a19eab637e94eba6684ad7cb7e347e9d4bdb2f1d57eee4e71db8",
    ),
    (
        "part11b-writers.fix",
        "bbf2a654af4edf7ba270490be593ec3eeaf80ba93478f300fe9881f95eb2b97e",
    ),
    (
        "part11b-writers.reproduce",
        "90703dedecc99d56842b2c71df9168ded74438cf8c2836cdb998229f957fd4c6",
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

// ---- part 7b (§34, §35) and §54's change of type

/// I, an image: rows cols format frames, the sizes, a fill and the header's overrides.
fn img(g: &mut G, fmts: &[&str]) -> String {
    let (rows, cols) = (g.int(0, 3), g.int(0, 3));
    let fmt = g.pick(fmts).to_owned();
    let nf = g.pick(&["1", "1", "1", "2", "0"]).to_owned();
    let sizes = [
        g.pick(&["3f800000", "40000000", "00000000", "1e3ce508", "7fc00000"])
            .to_owned(),
        g.pick(&["3f800000", "3f000000", "00000000"]).to_owned(),
    ];
    let fill = if fmt == "3" && !g.one_in(3) {
        format!("t{}", g.int(0, 33))
    } else {
        g.int(-1, 255).to_string()
    };
    let mut ov = ["-1"; 5];
    if g.one_in(6) {
        ov[g.below(5) as usize] = g.pick(&["0", "1", "2", "3"]);
    }
    format!(
        "{rows} {cols} {fmt} {nf} {} {} {fill} {}",
        sizes[0],
        sizes[1],
        ov.join(" ")
    )
}

/// A painting with labels 0 to 3 mostly (and the special labels of [`G::label`]).
fn lpaint(g: &mut G, shape: &str) -> String {
    if g.one_in(3) {
        return g.paint(shape);
    }
    let (w, h, d) = ext3(shape);
    let n = g.int(0, 6);
    let mut v = vec![n.to_string()];
    for _ in 0..n {
        let (x, y, z) = (g.int(0, w - 1), g.int(0, h - 1), g.int(0, d - 1));
        v.push(format!(
            "{x} {y} {z} {:08x}",
            (g.int(0, 3) as f32).to_bits()
        ));
    }
    v.join(" ")
}

/// A shape's frame count.
fn frames_of(shape: &str) -> i64 {
    shape
        .split(' ')
        .nth(4)
        .and_then(|x| x.parse().ok())
        .unwrap_or(1)
}

const IMAGE: &[(&str, Make)] = &[
    ("toimgv", |g| {
        let s = g.shape(NUMERIC);
        let a = g.w_of(&s);
        let p = lpaint(g, &s);
        let i = if g.one_in(3) {
            img(g, &["0", "0", "3"])
        } else {
            "null".to_owned()
        };
        let slice = g.pick(&["-1", "-1", "0", "1", "2", "5"]);
        let view = g.pick(&["0", "1", "2", "0", "1", "2", "3"]);
        let frame = g.int(0, (frames_of(&s) - 1).max(0));
        format!("{a} {p} {i} {slice} {view} {frame}")
    }),
    ("toimg", |g| {
        let a = g.w(&[0, 0, 1, 3, 3, 2, 4]);
        let i = if g.one_in(3) {
            img(g, &["0", "2", "3", "1"])
        } else {
            "null".to_owned()
        };
        format!("{a} 0 {i} {}", g.int(-1, 3))
    }),
    ("img2mri", |g| {
        if g.one_in(4) {
            img(g, &["4", "6", "36"])
        } else {
            img(g, &["0", "1", "2", "3", "35"])
        }
    }),
];

const NBHD: &[(&str, Make)] = &[
    ("smpar", |g| {
        let t = if g.one_in(6) { 0 } else { 4 };
        let s = format!("3 3 {} {t} 1 0", g.int(3, 4));
        let a = g.w_of(&s);
        format!(
            "{a} {} {}",
            lpaint(g, &s),
            g.pick(&["0", "1", "2", "3", "9", "26"])
        )
    }),
    ("erasebp", |g| {
        let a = g.w(NUMERIC);
        format!("{a} {}", g.int(-1, 2))
    }),
    ("nbhd", |g| {
        let s = g.shape(NUMERIC);
        let (w, h, d) = ext3(&s);
        let a = g.w_of(&s);
        let n = g.int(1, 4);
        let mut v = vec![format!("{a} {} {n}", lpaint(g, &s))];
        for _ in 0..n {
            let (k, ws) = (g.int(0, 3), g.int(-1, 5));
            let (x, y, z) = (g.int(-1, w), g.int(-1, h), g.int(-1, d));
            let fr = g.int(0, (frames_of(&s) - 1).max(0));
            v.push(format!("{k} {ws} {x} {y} {z} {fr} {}", g.f(true)));
        }
        v.join(" ")
    }),
    ("nearnz", |g| {
        let s = g.shape(&[0, 0, 0, 3, 4]);
        let (w, h, d) = ext3(&s);
        let a = g.w_of(&s);
        let n = g.int(1, 3);
        let mut v = vec![format!("{a} {} {n}", lpaint(g, &s))];
        for _ in 0..n {
            let c: Vec<String> = [w, h, d]
                .iter()
                .map(|&e| {
                    let x = g.int(0, e - 1) as f64 + [0.0, 0.4, -0.4, 0.5][g.below(4) as usize];
                    format!("{:016x}", x.to_bits())
                })
                .collect();
            v.push(format!("{} {} {}", g.int(-1, 5), c.join(" "), g.int(0, 1)));
        }
        v.join(" ")
    }),
    ("lmeans", |g| {
        let s = g.shape(NUMERIC);
        let (w, h, d) = ext3(&s);
        let a = g.w_of(&s);
        let ls = format!("{w} {h} {d} {} 1 0", g.pick(&["0", "1", "3"]));
        let lab = g.w_of(&ls);
        format!(
            "{} {a} {lab} {} {} {} {} {} {} {}",
            g.int(0, 4),
            lpaint(g, &ls),
            g.int(0, 3),
            g.int(-1, w),
            g.int(-1, h),
            g.int(-1, d),
            g.int(-1, 2),
            g.int(0, 1)
        )
    }),
    ("mkpos", |g| {
        let s = g.shape(NUMERIC);
        let a = g.w_of(&s);
        format!("{a} {}", g.dst(&s, false))
    }),
    ("mlbv", |g| {
        let s = g.shape(&[0, 0, 1, 3, 4]);
        let a = g.w_of(&s);
        let p = lpaint(g, &s);
        format!(
            "{a} {p} {} {} {} {}",
            g.dst(&s, false),
            g.int(0, 3),
            g.pick(&["0", "1", "7", "300", "-1"]),
            g.int(0, 1)
        )
    }),
    ("lnbhd", |g| {
        let s = g.shape(&[0, 0, 1, 3]);
        let (w, h, d) = ext3(&s);
        let a = g.w_of(&s);
        let p = lpaint(g, &s);
        let vals = if g.one_in(3) {
            "null".to_owned()
        } else {
            g.w_of(&format!("{w} {h} {d} 3 1 0"))
        };
        format!(
            "{a} {p} {vals} {} {} {} {} {} {}",
            g.int(0, w - 1),
            g.int(0, h - 1),
            g.int(0, d - 1),
            g.int(0, 2),
            g.int(0, 5),
            g.int(0, 1)
        )
    }),
    ("linbhd", |g| {
        let s = g.shape(&[0, 0, 1, 3]);
        let (w, h, d) = ext3(&s);
        let a = g.w_of(&s);
        let n = g.int(1, 4);
        let mut v = vec![format!("{a} {} {n}", lpaint(g, &s))];
        for _ in 0..n {
            v.push(format!(
                "{} {} {} {} {} {} {}",
                g.int(0, 1),
                g.int(-1, w),
                g.int(-1, h),
                g.int(-1, d),
                g.int(-1, 2),
                g.int(0, 3),
                g.int(-1, 3)
            ));
        }
        v.join(" ")
    }),
    ("lbbox", |g| {
        let s = g.shape(&[0, 1, 3, 4]);
        let a = g.w_of(&s);
        format!("{a} {} {}", lpaint(g, &s), g.int(0, 4))
    }),
    ("fillbox", |g| {
        let a = g.w(NUMERIC);
        let b: Vec<String> = (0..6)
            .map(|k| {
                if k < 3 {
                    g.int(-3, 4).to_string()
                } else {
                    g.int(-1, 5).to_string()
                }
            })
            .collect();
        format!("{a} {} {}", b.join(" "), g.f(true))
    }),
    ("mskz", |g| {
        let s = g.shape(NUMERIC);
        let a = g.w_of(&s);
        let ms = if g.one_in(5) {
            g.shape(NUMERIC)
        } else {
            s.clone()
        };
        let m = g.w_of(&ms);
        let p = lpaint(g, &ms);
        format!("{a} {m} {p} {}", g.dst(&s, true))
    }),
    ("clipwm", |g| {
        let s = g.shape(&[0, 0, 0, 3, 1]);
        let (w, h, d) = ext3(&s);
        let t = g.w_of(&s);
        let ws = format!("{w} {h} {d} {} 1 0", g.pick(&["0", "0", "3"]));
        let wm = g.w_of(&ws);
        format!("{t} {wm} {}", lpaint(g, &ws))
    }),
];

/// An LT of a transform array for `ltachg`: geometries valid mostly; an inverse without a matrix
/// sometimes (FS-505).
fn chlt(g: &mut G, inverse: bool) -> String {
    let m = if inverse && g.one_in(5) {
        "null".to_owned()
    } else {
        let mut m = g.mat4();
        while m == "null" {
            m = g.mat4();
        }
        m
    };
    let geo = |g: &mut G| {
        let special = g.one_in(4);
        let s = g.geom(special);
        let s = if g.one_in(8) {
            s
        } else {
            format!("1{}", &s[1..])
        };
        format!("{s} {} null", g.int(0, 1))
    };
    let (gs, gd) = (geo(g), geo(g));
    format!(
        "{} {} {} {} {m} null null {} 0 {gs} {gd}",
        g.f(false),
        g.f(false),
        g.f(false),
        g.f(false),
        g.int(0, 2)
    )
}

const LTAPV: &[(&str, Make)] = &[("ltachg", |g| {
    let n = g.pick(&["1", "1", "1", "2", "0"]).to_owned();
    let k: usize = n.parse().unwrap_or(1);
    let ty = g.pick(&["2", "2", "2", "1"]).to_owned();
    let target = if ty == "1" {
        "1"
    } else {
        g.pick(&["1", "1", "1", "2"])
    };
    let fw: Vec<String> = (0..k).map(|_| chlt(g, false)).collect();
    let inv: Vec<String> = (0..k).map(|_| chlt(g, true)).collect();
    format!(
        "{n} {ty} 3f800000 x {} {} {target}",
        fw.join(" "),
        inv.join(" ")
    )
})];

/// `ltachg` between the voxel, RAS-to-RAS and physical-voxel types (§54's six changes), a target
/// those types do not support and an array of another type.
const LTATYPES: &[(&str, Make)] = &[("ltachg", |g| {
    let n = g.pick(&["1", "1", "2", "0"]).to_owned();
    let k: usize = n.parse().unwrap_or(1);
    let ty = g.pick(&["0", "1", "2", "0", "1", "2", "5"]).to_owned();
    let target = g.pick(&["0", "1", "2", "0", "1", "2", "3"]).to_owned();
    let fw: Vec<String> = (0..k).map(|_| chlt(g, false)).collect();
    let inv: Vec<String> = (0..k).map(|_| chlt(g, true)).collect();
    format!(
        "{n} {ty} 3f800000 x {} {} {target}",
        fw.join(" "),
        inv.join(" ")
    )
})];

/// An invertible 4 x 4 matrix (M): a permutation with signs and scales, and a translation.
fn pool_mat(g: &mut G) -> String {
    let perm = [
        [0, 1, 2],
        [1, 0, 2],
        [2, 0, 1],
        [0, 2, 1],
        [1, 2, 0],
        [2, 1, 0],
    ][g.int(0, 5) as usize];
    let mut v = vec!["00000000".to_owned(); 16];
    for (r, &c) in perm.iter().enumerate() {
        v[4 * r + c] = g
            .pick(&["3f800000", "bf800000", "40000000", "3f000000"])
            .to_owned();
    }
    for r in 0..3 {
        v[4 * r + 3] = g.f(false);
    }
    v[15] = "3f800000".to_owned();
    format!("4 4 {}", v.join(" "))
}

/// A pool of three valid geometries for §54's chains: (extent, the 19 fields).
fn lta_pool(g: &mut G) -> Vec<(String, String)> {
    let z = "00000000";
    (0..3)
        .map(|_| {
            let e = format!("{} {} {}", g.int(2, 4), g.int(2, 4), g.int(1, 3));
            let mut f = vec![g.size(false), g.size(false), g.size(false), "1".to_owned()];
            f.extend(g.cosines(false));
            f.extend((0..3).map(|_| g.f(false)));
            f.extend([z.to_owned(), z.to_owned(), z.to_owned()]);
            (e, f.join(" "))
        })
        .collect()
}

/// An array (LTA) of `n` transforms of type `ty` between the pool's geometries, chained (each
/// source the previous destination) unless `chained` is false, from `start` when given; the
/// inverses' matrices absent sometimes, `m_dL` absent rarely (FS-198). Returns the text and the
/// first source's and the last destination's places in the pool.
fn pool_lta(
    g: &mut G,
    pool: &[(String, String)],
    ty: &str,
    n: usize,
    chained: bool,
    start: Option<usize>,
) -> (String, usize, usize) {
    let zeros = format!("4 4 {}", ["00000000"; 16].join(" "));
    let gf = |k: usize| format!("1 {} {} x 1 null", pool[k].0, pool[k].1);
    let first = g.int(0, 2) as usize;
    let mut idx = vec![start.unwrap_or(first)];
    for _ in 0..n {
        let last = idx[idx.len() - 1];
        let next = if chained {
            (last + 1 + g.int(0, 1) as usize) % 3
        } else {
            g.int(0, 2) as usize
        };
        idx.push(next);
    }
    let lt = |g: &mut G, inverse: bool, a: usize, b: usize| {
        let m = if inverse && g.one_in(8) {
            "null".to_owned()
        } else if g.one_in(8) {
            let mut m = g.mat4();
            while m == "null" {
                m = g.mat4();
            }
            m
        } else {
            pool_mat(g)
        };
        let dl = if g.one_in(16) {
            "null".to_owned()
        } else {
            zeros.clone()
        };
        format!(
            "{} {} {} {} {m} {dl} {dl} {} {} {} {}",
            g.f(false),
            g.f(false),
            g.f(false),
            g.pick(&["3f800000", "3f000000"]),
            g.int(0, 1),
            g.pick(&["0", "0", "3"]),
            gf(a),
            gf(b)
        )
    };
    let fw: Vec<String> = (0..n).map(|i| lt(g, false, idx[i], idx[i + 1])).collect();
    let inv: Vec<String> = (0..n).map(|i| lt(g, true, idx[i + 1], idx[i])).collect();
    let subject = g.pick(&["x", "x", "x62657274"]).to_owned();
    let fscale = g.pick(&["3f800000", "00000000", "3f000000"]).to_owned();
    let text = format!(
        "{n} {ty} {fscale} {subject} {} {}",
        fw.join(" "),
        inv.join(" ")
    );
    (text, idx[0], idx[n])
}

/// A W of a pool geometry (a byte volume of its extent, plain header fields).
fn pool_w(pool: &[(String, String)], k: usize) -> String {
    let z11 = ["00000000"; 11].join(" ");
    let d = pool[k].0.split(' ').nth(2).unwrap_or("1");
    format!(
        "{} 0 1 0 5 {} 1 {d} {z11} 0000000000000000 0 00000000 00000000 0 1 null null",
        pool[k].0, pool[k].1
    )
}

/// An array of a type for `ltachg` and its target, over a fresh pool.
fn lta54_change(g: &mut G, types: &[&str], targets: &[&str]) -> String {
    let pool = lta_pool(g);
    let ty = g.pick(types).to_owned();
    let n = g.int(1, 3) as usize - usize::from(g.one_in(12));
    let chained = g.one_in(2);
    let (a, _, _) = pool_lta(g, &pool, &ty, n, chained, None);
    format!("{a} {}", g.pick(targets))
}

/// §54's other operations: the changes to and from REGISTER_DAT, FSL and coronal RAS (with
/// `FSLOUTPUTTYPE` and a diagnostic level), the inversions, concatenations, reductions, source and
/// target tests and prints.
const LTA54: &[(&str, Make)] = &[
    ("+env=FSLOUTPUTTYPE=NIFTI_GZ ltachg", |g| {
        lta54_change(
            g,
            &["14", "15", "21", "1", "0", "2"],
            &["14", "15", "21", "1", "0", "2", "5"],
        )
    }),
    ("+env=FSLOUTPUTTYPE=ANALYZE ltachg", |g| {
        lta54_change(g, &["14", "15", "1"], &["15", "14", "1", "0"])
    }),
    ("ltachg", |g| {
        lta54_change(g, &["14", "21"], &["1", "0", "14"])
    }),
    ("+gdiagno=1 +env=FSLOUTPUTTYPE=NIFTI ltachg", |g| {
        lta54_change(g, &["14", "15", "21"], &["1", "0", "14", "15"])
    }),
    ("ltainv", |g| {
        let pool = lta_pool(g);
        let k = g.int(0, 2);
        let n = g.int(0, 3) as usize;
        let ty = g.pick(&["1", "0", "2"]).to_owned();
        let chained = g.one_in(2);
        let (a, _, _) = pool_lta(g, &pool, &ty, n, chained, None);
        if k == 1 {
            let m = if g.one_in(4) { (n + 1) % 4 } else { n };
            let (b, _, _) = pool_lta(g, &pool, "0", m, false, None);
            format!("1 {a} {b}")
        } else {
            format!("{k} {a}")
        }
    }),
    ("+env=FSLOUTPUTTYPE=NIFTI_GZ ltacat", |g| {
        let pool = lta_pool(g);
        let k = g.int(1, 3) as usize;
        let mut arrays = Vec::new();
        let mut last: Option<usize> = None;
        for _ in 0..k {
            let ty = g.pick(&["1", "1", "0", "2", "14"]).to_owned();
            let n = g.int(1, 2) as usize - usize::from(g.one_in(16));
            // mostly from where the previous array ended
            let start = if g.one_in(4) { None } else { last };
            let (text, _, b) = pool_lta(g, &pool, &ty, n, true, start);
            arrays.push(text);
            last = Some(b);
        }
        format!("{k} {} {}", arrays.join(" "), g.int(0, 1))
    }),
    ("ltacat2", |g| {
        let pool = lta_pool(g);
        let n1 = g.int(1, 2) as usize;
        let (a, _, e) = pool_lta(g, &pool, "1", n1, true, None);
        let n2 = g.int(1, 2) as usize;
        let start = if g.one_in(3) { None } else { Some(e) };
        let (b, _, _) = pool_lta(g, &pool, "1", n2, true, start);
        format!("{a} {b} {}", g.int(0, 1))
    }),
    ("ltared", |g| {
        let pool = lta_pool(g);
        let n = g.int(0, 3) as usize;
        let ty = g.pick(&["1", "0", "14"]).to_owned();
        let chained = !g.one_in(4);
        let (a, _, _) = pool_lta(g, &pool, &ty, n, chained, None);
        a
    }),
    ("ltasrc", |g| {
        let pool = lta_pool(g);
        let n = g.int(1, 3) as usize - usize::from(g.one_in(10));
        let (a, s, d) = pool_lta(g, &pool, "1", n, true, None);
        let k = [s, d, 0, 1, 2][g.int(0, 4) as usize];
        format!("{a} {}", pool_w(&pool, k))
    }),
    ("ltaprt", |g| {
        let pool = lta_pool(g);
        let n = g.int(0, 2) as usize;
        let ty = g.pick(&["0", "1", "14", "2", "15", "21", "-1"]).to_owned();
        let (a, _, _) = pool_lta(g, &pool, &ty, n, false, None);
        a
    }),
];

/// A painted W of the shape (labels 0 to 3 at a few voxels).
fn wl(g: &mut G, s: &str) -> String {
    let w = g.w_of(s);
    format!("{w} {}", lpaint(g, s))
}

/// A W of the shape, not painted.
fn w0(g: &mut G, s: &str) -> String {
    format!("{} 0", g.w_of(s))
}

/// A W of the shape painted, or `null` one time in `k`.
fn wl_or_null(g: &mut G, s: &str, k: u64) -> String {
    if g.one_in(k) {
        "null".to_owned()
    } else {
        wl(g, s)
    }
}

/// A shape of the extent of `s` with another type and frame count.
fn like(s: &str, kind: i64, frames: i64) -> String {
    let (w, h, d) = ext3(s);
    format!("{w} {h} {d} {kind} {frames} 0")
}

/// Part 8's §37 and §38 (tiv, which reads files, is in the defect tests).
const FR8: &[(&str, Make)] = &[
    ("pvlab", |g| {
        let s = g.shape(&[0, 3, 3, 4]);
        let vs = like(&s, 3, 1);
        let lab = wl(g, &s);
        let vals = wl_or_null(g, &vs, 8);
        let label = g.int(-1, 4);
        let maxl = g.pick(&["4", "4", "5", "1", "0"]).to_owned();
        let mix = if g.one_in(2) {
            "null".to_owned()
        } else {
            g.w_of(&vs)
        };
        let nbr = if g.one_in(2) {
            "null".to_owned()
        } else {
            g.w_of(&vs)
        };
        let sb = if g.one_in(4) {
            let f = g.int(1, 3);
            wl(g, &like(&s, 3, f))
        } else {
            "null".to_owned()
        };
        format!("{lab} {vals} {label} {maxl} {mix} {nbr} {sb}")
    }),
    ("randn", |g| {
        let n: Vec<i64> = vec![g.int(1, 3), g.int(1, 3), g.int(1, 3), g.int(1, 2)];
        let given = if g.one_in(2) {
            "null".to_owned()
        } else {
            let s = format!(
                "{} {} {} {} {} 0",
                n[0],
                n[1],
                n[2],
                g.pick(&["3", "3", "0"]),
                n[3]
            );
            g.w_of(&s)
        };
        format!(
            "{} {} {} {} {} {} {} {given}",
            n[0],
            n[1],
            n[2],
            n[3],
            g.f(true),
            g.f(true),
            g.int(0, 99)
        )
    }),
    ("const", |g| {
        let n: Vec<i64> = vec![g.int(1, 3), g.int(1, 3), g.int(1, 3), g.int(1, 2)];
        let given = if g.one_in(3) {
            "null".to_owned()
        } else {
            let t = g.pick(&["0", "1", "2", "3", "4", "9", "10"]).to_owned();
            let s = format!("{} {} {} {t} {} 0", n[0], n[1], n[2], n[3]);
            g.w_of(&s)
        };
        let v = g
            .pick(&[
                "43960000", "bf800000", "471c4080", "c0200000", "4f32d05e", "7fc00000",
            ])
            .to_owned();
        let v = if g.one_in(2) { v } else { g.f(true) };
        format!("{} {} {} {} {v} {given}", n[0], n[1], n[2], n[3])
    }),
    ("nseq", |g| {
        let s = g.shape(&[0, 3, 3, 4, 10, 1]);
        format!("{} {} {}", w0(g, &s), g.int(0, 2), g.f(true))
    }),
    ("nfr", |g| {
        let s = g.shape(NUMERIC);
        format!("{} {} {}", w0(g, &s), g.int(0, 2), g.f(false))
    }),
    ("sort", |g| {
        let s = g.shape(NUMERIC);
        let a = w0(g, &s);
        let m = wl_or_null(g, &like(&s, 3, 1), 2);
        let d = g.dst(&s, m != "null");
        format!("{a} {m} {d}")
    }),
    ("revsl", |g| {
        let s = g.shape(NUMERIC);
        let f = frames_of(&s);
        let a = w0(g, &s);
        let short = g.int(0, 1);
        let bv = if g.one_in(3) {
            "null".to_owned()
        } else {
            matrix(g, (f - short).max(0), 1)
        };
        let bc = if g.one_in(3) {
            "null".to_owned()
        } else {
            matrix(g, f, 3)
        };
        format!("{a} {bv} {bc} {} {}", g.int(0, 2), g.dst(&s, false))
    }),
    ("cdtype", |g| {
        let a = g.w(NUMERIC);
        format!("{a} {}", g.pick(&["0", "1", "2", "3", "4", "10", "6"]))
    }),
    ("ordidx", |g| {
        let s = g.shape(NUMERIC);
        format!("{} {}", w0(g, &s), g.int(0, 9))
    }),
    ("vperm", |g| {
        // seed 0 is "no seed yet", which FS-04's `reproduce` takes from the clock
        let s = g.shape(NUMERIC);
        format!("{} {}", w0(g, &s), g.int(1, 99))
    }),
    ("nonmax", |g| {
        let s = g.shape(&[0, 3, 3, 4]);
        let a = w0(g, &s);
        format!("{a} {} {} {}", g.dst(&s, false), g.f(false), g.int(-1, 2))
    }),
    ("xrpad", |g| {
        let s = g.shape(NUMERIC);
        let (w, h, d) = ext3(&s);
        let a = w0(g, &s);
        let r = if g.one_in(3) {
            "null".to_owned()
        } else {
            format!(
                "{} {} {} {} {} {}",
                g.int(0, w - 1),
                g.int(0, h - 1),
                g.int(0, d - 1),
                g.int(1, w),
                g.int(1, h),
                g.int(1, d)
            )
        };
        format!("{a} {r} {}", g.int(0, 2))
    }),
    ("rms", |g| {
        let s = g.shape(NUMERIC);
        let o = like(&s, g.pick(&["3", "0", "4"]).parse().unwrap_or(3), 1);
        format!("{} {}", w0(g, &s), g.w_of(&o))
    }),
    ("vfrac", |g| {
        let s = g.shape(&[3]);
        let seg = wl(g, &like(&s, 3, 1));
        let nf = g.int(1, 3);
        let fr = g.w_of(&like(&s, 3, nf));
        let m = if g.one_in(4) {
            g.mat4()
        } else {
            let mut e = vec!["00000000".to_owned(); 16];
            for k in 0..3 {
                e[5 * k] = g.pick(&["3f000000", "3f800000", "40000000"]).to_owned();
                e[4 * k + 3] = g.pick(&["00000000", "3f800000", "bf800000"]).to_owned();
            }
            e[15] = "3f800000".to_owned();
            format!("4 4 {}", e.join(" "))
        };
        format!("{} {m} {seg} {fr}", g.w_of(&s))
    }),
    ("cdt", |g| {
        let s = g.shape(NUMERIC);
        let (a, b) = (w0(g, &s), w0(g, &s));
        format!("{a} {b} {}", g.dst(&s, true))
    }),
    ("diffv", |g| {
        let s = g.shape(NUMERIC);
        let (a, b) = (w0(g, &s), w0(g, &s));
        let m = wl_or_null(g, &like(&s, 3, 1), 2);
        format!("{a} {b} {m} {}", g.d(true))
    }),
    ("applyv", |g| {
        let s = g.shape(NUMERIC);
        let (w, h, d) = ext3(&s);
        let a = w0(g, &s);
        let m = wl_or_null(g, &like(&s, 3, 1), 2);
        let n = g.int(0, 3);
        let mut rows = Vec::new();
        for _ in 0..n {
            for e in [w, h, d] {
                rows.push(format!("{:016x}", (g.int(-1, e) as f64).to_bits()));
            }
            rows.push(g.d(false));
            rows.push(g.d(true));
        }
        let dd = g.dst(&s, m != "null");
        format!("{a} {m} {n} {} {dd}", rows.join(" "))
    }),
    ("dimmis", |g| {
        let (a, b) = (g.w(NUMERIC), g.w(NUMERIC));
        format!("{a} {b} {}", g.int(0, 1))
    }),
];

/// Part 8's §39.
const VOX8: &[(&str, Make)] = &[
    ("fpow", |g| {
        let s = g.shape(&[3]);
        let n = g.int(0, 3);
        let p: Vec<String> = (0..n)
            .map(|_| {
                let v = g
                    .pick(&["3f800000", "3f000000", "40000000", "3fc00000"])
                    .to_owned();
                if g.one_in(3) { g.f(true) } else { v }
            })
            .collect();
        format!("{} {n} {}", w0(g, &s), p.join(" "))
    }),
    ("mbin", |g| {
        let s = g.shape(NUMERIC);
        let a = w0(g, &s);
        let d = g.dst(&s, false);
        let tail = g
            .pick(&[
                "null",
                "x706f73",
                "x4e4547",
                "x616273",
                "x706f",
                "x",
                "x6162736f6c757465",
            ])
            .to_owned();
        format!("{a} {d} {} {tail} {}", g.f(false), g.int(0, 1))
    }),
    ("reshape", |g| {
        let s = g.shape(&[0, 1, 2, 3, 4, 10, 9]);
        let (w, h, d) = ext3(&s);
        let f = frames_of(&s).max(1);
        let n = w * h * d * f;
        let (a, b, c) = if g.one_in(4) {
            (n + 1, 1, 1)
        } else {
            (1, n, 1)
        };
        format!("{} {a} {b} {c} 1", w0(g, &s))
    }),
    ("msq", |g| {
        let k = g.int(0, 2);
        let s = g.shape(NUMERIC);
        let a = w0(g, &s);
        let m = wl_or_null(g, &like(&s, 3, 1), 2);
        let d = g.dst(&s, m != "null");
        format!("{k} {a} {m} {d}")
    }),
    ("mmax", |g| {
        let s = g.shape(NUMERIC);
        let (a, b) = (w0(g, &s), w0(g, &s));
        format!("{a} {b} {}", g.dst(&s, true))
    }),
    ("mconst", |g| {
        let s = g.shape(NUMERIC);
        let a = w0(g, &s);
        format!("{a} {} {} {}", g.d(true), g.int(0, 1), g.dst(&s, false))
    }),
    ("fbin", |g| {
        let s = g.shape(NUMERIC);
        let a = w0(g, &s);
        let m = wl_or_null(g, &like(&s, 3, 1), 2);
        format!("{a} {} {m}", g.d(false))
    }),
    ("vote", |g| {
        let s = g.shape(&[0, 3, 4]);
        let a = w0(g, &s);
        let m = wl_or_null(g, &like(&s, 3, 1), 2);
        let d = g.dst(&s, m != "null");
        format!("{a} {m} {d} {}", g.int(0, 1))
    }),
    ("crop", |g| {
        let s = g.shape(NUMERIC);
        let (w, h, d) = ext3(&s);
        let a = w0(g, &s);
        format!(
            "{a} {} {} {} {} {} {}",
            g.int(-1, w - 1),
            g.int(0, h - 1),
            g.int(0, d - 1),
            g.int(0, w),
            g.int(0, h - 1),
            g.int(0, d - 1)
        )
    }),
    ("revs", |g| {
        let s = g.shape(NUMERIC);
        let a = w0(g, &s);
        format!("{a} {}", g.dst(&s, false))
    }),
    ("cutend", |g| {
        let s = g.shape(NUMERIC);
        let a = w0(g, &s);
        format!("{a} {}", g.int(0, 2))
    }),
    ("segids", |g| {
        let s = g.shape(NUMERIC);
        let a = wl(g, &s);
        format!("{a} 0 {}", g.int(0, 1))
    }),
    ("hcos", |g| {
        let s = g.shape(NUMERIC);
        let a = w0(g, &s);
        format!("{a} {} {}", g.d(true), g.dst(&s, false))
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

#[test]
fn slices_as_images() {
    group("image", IMAGE, 12);
}

#[test]
fn neighbourhoods_and_labels() {
    group("nbhd", NBHD, 6);
}

#[test]
fn transform_arrays_changed_to_ras() {
    group("ltapv", LTAPV, 16);
}

#[test]
fn transform_arrays_between_types() {
    group("ltatypes", LTATYPES, 24);
}

#[test]
fn partial_volumes_and_sequences() {
    group("fr8", FR8, 8);
}

#[test]
fn voxelwise_operations() {
    group("vox8", VOX8, 8);
}

#[test]
fn transform_array_operations() {
    group("lta54", LTA54, 8);
}

/// A W of the extent and type (1 `int`, 3 `float`) whose voxels are labels drawn from `labels`
/// (the fill `i<word>,...`), the rest of its fields seeded.
fn seg_w(g: &mut G, (w, h, d): (i64, i64, i64), kind: i64, frames: i64, labels: &[i64]) -> String {
    let n = (w * h * d * frames) as usize;
    let words: Vec<String> = (0..n)
        .map(|_| {
            let l = labels[g.below(labels.len() as u64) as usize];
            if kind == 3 {
                (l as f32).to_bits().to_string()
            } else {
                l.to_string()
            }
        })
        .collect();
    // an ordinary geometry (as the pool's: sizes, cosines and centre without special values)
    let mut f = vec![g.size(false), g.size(false), g.size(false), "1".to_owned()];
    f.extend(g.cosines(false));
    f.extend((0..3).map(|_| g.f(false)));
    f.extend(["00000000"; 3].map(str::to_owned));
    let z11 = ["00000000"; 11].join(" ");
    format!(
        "{w} {h} {d} {kind} {frames} 0 i{} {} 1 {d} {z11} 0000000000000000 0 00000000 00000000 0 1 null null",
        words.join(","),
        f.join(" ")
    )
}

/// A colour table of `nlab` places (a tenth empty) of tissue types -2 to 2, with a tissue-type
/// table of `ntt` places (none for a negative `ntt`).
fn tissue_ct(g: &mut G, nlab: i64, ntt: i64) -> String {
    let mut p = vec![
        nlab.to_string(),
        xs(b"lut"),
        "0".into(),
        "0".into(),
        "x".into(),
    ];
    for i in 0..nlab {
        if g.one_in(10) {
            p.push("0".into());
            continue;
        }
        p.push("1".into());
        p.push(xs(format!("lab{i}").as_bytes()));
        for _ in 0..4 {
            p.push(g.int(0, 255).to_string());
        }
        p.extend(["3f000000"; 4].map(str::to_owned));
        p.push(g.int(-2, 2).to_string());
        p.push("0".into());
    }
    if ntt < 0 {
        p.push("null".into());
    } else {
        p.extend([
            ntt.to_string(),
            xs(b"t"),
            "0".into(),
            "0".into(),
            "x".into(),
        ]);
        for k in 0..ntt {
            p.push("1".into());
            p.push(xs(format!("type{k}").as_bytes()));
            p.extend(["0"; 4].map(str::to_owned));
            p.extend(["00000000"; 4].map(str::to_owned));
            p.extend(["0".to_owned(), "0".to_owned()]);
        }
        p.push("null".into());
    }
    p.join(" ")
}

/// A small extent.
fn ext(g: &mut G, lo: i64, hi: i64) -> (i64, i64, i64) {
    (g.int(lo, hi), g.int(lo, hi), g.int(lo, hi))
}

/// One of the integers.
fn pi(g: &mut G, v: &[i64]) -> i64 {
    v[g.below(v.len() as u64) as usize]
}

/// §55 to §57: segmentations onto another grid (votes, thresholds, the diagnostic levels), the
/// array's version without upsampling, SPM warps, the names' codes; segment borders,
/// statistics, indices, concatenations, edits, shifts, the entorhinal fix and the junction;
/// the crops.
const SEGS57: &[(&str, Make)] = &[
    ("a2v", |g| {
        let e = ext(g, 1, 3);
        let kind = pi(g, &[1, 3]);
        let seg = seg_w(g, e, kind, 1, &[0, 1, 1, 2, 3, 4]);
        let tkr = if g.one_in(3) { g.mat4() } else { pool_mat(g) };
        let te = ext(g, 1, 3);
        let tk = pi(g, &[0, 1, 3]);
        let t = seg_w(g, te, tk, 1, &[0]);
        let ft = g.pick(&["0000000000000000", "3fe0000000000000", "3fd3333333333333"]);
        let ct = match g.int(0, 3) {
            0 => "null".to_owned(),
            1 => tissue_ct(g, 5, -1),
            2 => tissue_ct(g, 5, 0),
            _ => {
                let n = g.int(1, 3);
                tissue_ct(g, 6, n)
            }
        };
        format!("{seg} 0 {tkr} {t} {ft} {ct}")
    }),
    ("+gdiagno=2 a2v", |g| {
        let e = ext(g, 1, 3);
        let seg = seg_w(g, e, 1, 1, &[1, 2, 3]);
        let te = ext(g, 1, 2);
        let t = seg_w(g, te, 1, 1, &[0]);
        let m = pool_mat(g);
        format!("{seg} 0 {m} {t} 0000000000000000 null")
    }),
    ("a2vmu", |g| {
        let pool = lta_pool(g);
        let ty = g.pick(&["1", "0"]);
        let (lta, s, d) = pool_lta(g, &pool, ty, 1, true, None);
        let k = [s, d, s, d, 0, 1, 2][g.int(0, 6) as usize];
        let shape = format!("{} 0 1 0", pool[k].0);
        let w = pool_w(&pool, k);
        let ct = if g.one_in(2) {
            "null".to_owned()
        } else {
            tissue_ct(g, 6, 2)
        };
        let p = lpaint(g, &shape);
        format!("{w} {p} {lta} 0000000000000000 -1 {ct}")
    }),
    ("spmw", |g| {
        let pool = lta_pool(g);
        let ty = g.pick(&["0", "1"]);
        let (lta, s, _) = pool_lta(g, &pool, ty, 1, true, None);
        let (lta, v) = if g.one_in(2) {
            let nf = g.int(1, 2);
            (
                "null".to_owned(),
                seg_w(g, (3, 3, 2), 3, nf, &[0, 1, 5, -3]),
            )
        } else {
            let e: Vec<i64> = pool[s]
                .0
                .split(' ')
                .map(|x| x.parse().unwrap_or(1))
                .collect();
            (lta, seg_w(g, (e[0], e[1], e[2]), 3, 1, &[0, 1, 5, -3]))
        };
        let we = ext(g, 2, 4);
        let wf = pi(g, &[3, 3, 2]);
        let warp = seg_w(g, we, 3, wf, &[-2, -1, 0, 1, 2]);
        let d = if g.one_in(3) {
            let nf = g.int(1, 2);
            g.w_of(&format!("3 3 3 3 {nf} 0"))
        } else {
            "new".to_owned()
        };
        let (lr, interp) = (g.int(0, 1), g.pick(&["0", "1", "1", "2"]));
        format!("{v} 0 {lta} {warp} 0 {lr} {interp} {d}")
    }),
    ("icode", |g| {
        let n = g.pick(&[
            "nearest",
            "TRILINEAR",
            "sinc",
            "cubic",
            "rint",
            "Floor",
            "tkregister",
            "tkre",
            "",
            "x",
        ]);
        xs(n.as_bytes())
    }),
    ("sbound", |g| {
        let e = ext(g, 1, 4);
        let kind = pi(g, &[1, 3]);
        format!("{} 0", seg_w(g, e, kind, 1, &[0, 1, 2, 2]))
    }),
    ("sborder", |g| {
        let e = ext(g, 1, 3);
        let a = seg_w(g, e, 1, 1, &[0, 1, 2, 5]);
        format!("{a} 0 {}", g.int(-1, 4))
    }),
    ("idxno", |g| {
        let s = g.shape(&[0, 1, 3]);
        let a = w0(g, &s);
        format!("{a} {}", g.dst(&s, false))
    }),
    ("ssrob", |g| {
        let e = ext(g, 1, 3);
        let nf = g.int(1, 2);
        let seg = seg_w(g, e, 1, 1, &[0, 1, 1, 2]);
        let m = g.w_of(&format!("{} {} {} 3 {nf} 0", e.0, e.1, e.2));
        let pct = g.pick(&[
            "00000000", "41200000", "42480000", "42700000", "7fc00000", "c1200000",
        ]);
        format!("{seg} 0 {m} 0 {} {} {pct}", g.int(0, 3), g.int(0, nf))
    }),
    ("sfavg", |g| {
        let e = ext(g, 1, 3);
        let seg = seg_w(g, e, 1, 1, &[0, 1, 2]);
        let nf = g.int(1, 3);
        let m = g.w_of(&format!("{} {} {} 3 {nf} 0", e.0, e.1, e.2));
        format!("{seg} 0 {m} 0 {}", g.int(0, 3))
    }),
    ("zcat", |g| {
        let w = g.int(1, 3);
        let (h, d1, d2) = (g.int(1, 3), g.int(1, 3), g.int(1, 3));
        let a = seg_w(g, (w, h, d1), 1, 1, &[1, 2, 3, 4]);
        let b = seg_w(g, (w, h, d2), 1, 1, &[5, 6, 7]);
        let nskip = g.int(-1, d2);
        let dst = g.pick(&["new", "new", "self", "other"]);
        format!("{a} 0 {b} 0 {nskip} {dst}")
    }),
    ("aedit", |g| {
        let e = ext(g, 1, 3);
        let n = seg_w(g, e, 1, 1, &[0, 1, 2]);
        let o = seg_w(g, e, 1, 1, &[0, 1, 2]);
        let m = seg_w(g, e, 1, 1, &[0, 1, 2]);
        let dst = g.pick(&["new", "self", "other"]);
        format!("{n} 0 {o} 0 {m} 0 {dst}")
    }),
    ("shdim", |g| {
        let e = ext(g, 1, 3);
        let (kind, nf) = (pi(g, &[1, 3]), g.int(1, 2));
        let a = seg_w(g, e, kind, nf, &[0, 1, 2, 3, 4, 5]);
        format!("{a} 0 {} {} {}", g.int(0, 4), g.int(-2, 2), g.int(0, 1))
    }),
    ("fento", |g| {
        let e = ext(g, 1, 3);
        let kind = pi(g, &[1, 3]);
        let a = seg_w(g, e, kind, 1, &[0, 110, 5]);
        let ento = seg_w(g, e, 1, 1, &[0, 3006, 4006, 3201, 4201, 7030, 7031]);
        let lh = g.pick(&["406fe00000000000", "0000000000000000"]);
        let rh = g.d(false);
        format!("{a} 0 {ento} 0 {} {lh} {rh} {}", g.int(0, 4), g.int(0, 1))
    }),
    ("lacj", |g| {
        let e = ext(g, 3, 4);
        let seg = seg_w(g, e, 1, 1, &[3, 3, 42, 18, 54, 0]);
        let d = if g.one_in(3) {
            "new".to_owned()
        } else {
            seg_w(g, e, 1, 1, &[0, 7030])
        };
        format!("{seg} 0 {} {d}", g.int(0, 3))
    }),
    ("cfov", |g| {
        let e = ext(g, 1, 4);
        let m = seg_w(g, e, 3, 1, &[0, 0, 1, 1, 2]);
        let th = g.pick(&["3fe0000000000000", "0000000000000000", "7ff8000000000000"]);
        format!("{m} 0 {} {} {} {th}", g.int(0, 4), g.int(1, 4), g.int(1, 4))
    }),
    ("ccrs", |g| {
        let e = ext(g, 1, 4);
        let (kind, nf) = (pi(g, &[1, 3]), g.int(1, 2));
        let v = seg_w(g, e, kind, nf, &[0, 1, 2, 3]);
        let c: Vec<String> = (0..3).map(|_| g.d(false)).collect();
        let keep = if g.one_in(2) { "0" } else { "2 1 3" };
        let f = [g.int(0, 3), g.int(1, 3), g.int(1, 3)];
        format!("{v} 0 {} {} {} {} {keep}", c.join(" "), f[0], f[1], f[2])
    }),
    ("crras", |g| {
        let pool = lta_pool(g);
        let ty = g.pick(&["1", "0"]);
        let (lta, s, d) = pool_lta(g, &pool, ty, 1, true, None);
        let k = [s, d, 0][g.int(0, 2) as usize];
        let shape = format!("{} 0 1 0", pool[k].0);
        let p = lpaint(g, &shape);
        let v = format!("{} {p}", pool_w(&pool, k));
        let lta = if g.one_in(3) { "null".to_owned() } else { lta };
        let valid = g.int(0, 1);
        let c: Vec<String> = (0..3).map(|_| g.d(false)).collect();
        let f = [g.int(1, 3), g.int(1, 3), g.int(1, 3)];
        format!(
            "{v} {valid} {} {} {} {} 0 {lta}",
            c.join(" "),
            f[0],
            f[1],
            f[2]
        )
    }),
];

#[test]
fn segmentations_segments_and_crops() {
    group("segs57", SEGS57, 6);
}

/// A target-to-source matrix: none, a pool's or one of `mat4`'s.
fn v2v_mat(g: &mut G) -> String {
    match g.int(0, 2) {
        0 => "null".to_owned(),
        1 => pool_mat(g),
        _ => g.mat4(),
    }
}

/// A shift map of the extent (`W P od`) or `null`.
fn vsm_of(g: &mut G, e: (i64, i64, i64)) -> String {
    if g.one_in(4) {
        return "null".to_owned();
    }
    let m = seg_w(g, e, 3, 1, &[0, 1, -1, 2]);
    format!("{m} 0 0000000000000000")
}

/// Part 9 (§41 to §44, §55's upsampling and §56's `MRIvol2VolTkRegVSM`).
const PART9: &[(&str, Make)] = &[
    ("vv", |g| {
        let e = ext(g, 1, 3);
        let nf = g.int(1, 2);
        let kind = pi(g, &[0, 1, 3]);
        let s = seg_w(g, e, kind, nf, &[0, 1, 2, 5, -3]);
        let te = ext(g, 1, 3);
        let t = seg_w(g, te, 3, nf, &[0]);
        let m = v2v_mat(g);
        let interp = g.pick(&["0", "1", "2", "5"]);
        let od = g.pick(&["0000000000000000", "4014000000000000"]);
        let param = g.pick(&["3f800000", "40000000"]);
        format!("{s} 0 {od} {t} 0 {m} {interp} {param}")
    }),
    ("+gdiagno=1 vv", |g| {
        let e = ext(g, 1, 3);
        let s = seg_w(g, e, 3, 1, &[0, 1, 2]);
        let t = seg_w(g, e, 3, 1, &[0]);
        let m = pool_mat(g);
        format!("{s} 0 0000000000000000 {t} 0 {m} 1 3f800000")
    }),
    ("vvsm", |g| {
        let e = ext(g, 2, 3);
        let s = seg_w(g, e, 3, 1, &[0, 1, 2, 3]);
        let te = ext(g, 1, 3);
        let t = seg_w(g, te, 3, 1, &[0]);
        let m = v2v_mat(g);
        let interp = g.pick(&["0", "1"]);
        let vsm = vsm_of(g, e);
        format!(
            "{s} 0 0000000000000000 {t} 0 {m} {interp} 3f800000 {vsm} null {}",
            g.int(-3, 3)
        )
    }),
    ("+gdiag=10000000 vvtk", |g| {
        let e = ext(g, 2, 3);
        let s = seg_w(g, e, 3, 1, &[0, 1, 2, 3]);
        let t = seg_w(g, e, 3, 1, &[0]);
        let m = if g.one_in(2) {
            "null".to_owned()
        } else {
            pool_mat(g)
        };
        let vsm = vsm_of(g, e);
        format!(
            "{s} 0 0000000000000000 {t} 0 {m} 1 3f800000 {vsm} null {}",
            g.int(-3, 3)
        )
    }),
    ("vvk", |g| {
        let e = ext(g, 1, 3);
        let s = seg_w(g, e, 3, 1, &[0, 1, 2]);
        let te = ext(g, 1, 2);
        let nf = g.pick(&["8", "8", "2"]);
        let t = seg_w(g, te, 3, nf.parse().unwrap_or(8), &[0]);
        let m = v2v_mat(g);
        format!("{s} 0 {t} 0 {m}")
    }),
    ("vvd", |g| {
        let e = ext(g, 1, 3);
        let mv = seg_w(g, e, 1, 1, &[0]);
        let te = ext(g, 1, 3);
        let t = seg_w(g, te, 1, 1, &[0]);
        let m = if g.one_in(2) {
            "null".to_owned()
        } else {
            pool_mat(g)
        };
        format!("{mv} 0 {t} 0 {m}")
    }),
    ("rd2lta", |g| {
        let e = ext(g, 1, 3);
        let t = seg_w(g, e, 1, 1, &[0]);
        let me = ext(g, 1, 3);
        let mv = seg_w(g, me, 1, 1, &[0]);
        let m = if g.one_in(2) {
            "null".to_owned()
        } else {
            pool_mat(g)
        };
        format!("{t} {mv} {m}")
    }),
    ("maskup", |g| {
        let e = ext(g, 2, 4);
        let s = seg_w(g, e, 3, 1, &[0, 0, 1, 2]);
        let mk = if g.one_in(2) {
            "null".to_owned()
        } else {
            format!("{} 0", seg_w(g, e, 3, 1, &[0, 0, 1]))
        };
        format!(
            "{s} 0 {mk} {} {} {}",
            g.int(-1, 3),
            g.int(-1, 2),
            g.int(0, 1)
        )
    }),
    ("rgbb", |g| {
        let e = ext(g, 1, 5);
        let m = seg_w(g, e, 3, 1, &[0, 0, 0, 1]);
        match g.int(0, 2) {
            1 => {
                let p: Vec<String> = (0..6).map(|_| g.int(-1, 2).to_string()).collect();
                format!("{m} 0 1 {}", p.join(" "))
            }
            k => format!("{m} 0 {k} {}", g.int(-1, 2)),
        }
    }),
    ("rgprt", |g| {
        let v: Vec<String> = (0..6).map(|_| g.int(-9, 99).to_string()).collect();
        v.join(" ")
    }),
    ("cov", |g| {
        let e = ext(g, 1, 3);
        let nf = g.int(1, 3);
        let a = seg_w(g, e, 3, nf, &[0, 1, 2, -1]);
        let mk = if g.one_in(2) {
            "null".to_owned()
        } else {
            format!("{} 0", seg_w(g, e, 3, 1, &[0, 1, 1]))
        };
        format!("{a} 0 {mk}")
    }),
    ("mpca", |g| {
        let e = ext(g, 1, 3);
        let nf = g.int(1, 3);
        let a = seg_w(g, e, 3, nf, &[0, 1, 2, -1, 3]);
        let mk = if g.one_in(2) {
            "null".to_owned()
        } else {
            format!("{} 0", seg_w(g, e, 3, 1, &[0, 1, 1]))
        };
        format!("{a} 0 {mk}")
    }),
    ("pcast", |g| {
        let n = g.int(0, 4);
        let v: Vec<&str> = (0..n)
            .map(|_| g.pick(&["00000000", "3f800000", "40490fdb", "7fc00000", "7f800000"]))
            .collect();
        let sep = if v.is_empty() { "" } else { " " };
        format!("{n} {}{sep}{}", v.join(" "), g.pick(&["0", "2"]))
    }),
    ("fdr", |g| {
        let n = g.int(1, 2);
        let mut s = n.to_string();
        for _ in 0..n {
            let e = ext(g, 1, 3);
            let nf = g.int(1, 2);
            let v = seg_w(g, e, 3, nf, &[0, 1, 2, -1, 3]);
            let mk = if g.one_in(2) {
                "null".to_owned()
            } else {
                format!("{} 0", seg_w(g, e, 3, 1, &[0, 1, 1]))
            };
            let out = g.pick(&["new", "self"]);
            s.push_str(&format!(" {v} 0 {} {mk} {out}", g.int(0, 1)));
        }
        let fdr = g.pick(&["3fa999999999999a", "3fe0000000000000"]);
        format!(
            "{s} {fdr} {} {} {} {} {}",
            g.int(-1, 1),
            g.int(0, 1),
            g.int(0, 1),
            g.int(0, 1),
            g.int(0, 1)
        )
    }),
    ("vxn", |g| {
        let e = ext(g, 1, 4);
        let nf = g.int(1, 2);
        let m = seg_w(g, e, 3, nf, &[0, 1, 2, 3, 7]);
        let mut s = format!("{m} 0 3");
        for _ in 0..3 {
            s.push_str(&format!(
                " {} {} {} {} {}",
                g.int(0, e.0 - 1),
                g.int(0, e.1 - 1),
                g.int(0, e.2 - 1),
                g.int(-1, 8),
                g.int(0, 1)
            ));
        }
        s
    }),
    ("a2vmu", |g| {
        let pool = lta_pool(g);
        let ty = g.pick(&["1", "0"]);
        let (lta, s, d) = pool_lta(g, &pool, ty, 1, true, None);
        let k = [s, d, s, d, 0][g.int(0, 4) as usize];
        let shape = format!("{} 0 1 0", pool[k].0);
        let w = pool_w(&pool, k);
        let ct = if g.one_in(2) {
            "null".to_owned()
        } else {
            tissue_ct(g, 6, 2)
        };
        let p = lpaint(g, &shape);
        let usf = g.pick(&["1", "2"]);
        format!("{w} {p} {lta} 0000000000000000 {usf} {ct}")
    }),
];

#[test]
fn part9_resampling_regions_covariance_and_neighbourhoods() {
    group("part9", PART9, 4);
}

/// `null` or an array for §46: mostly one RAS-to-RAS transform between a pool's valid
/// geometries; rarely a voxel-to-voxel one, two transforms or none (the program's ends).
fn tal_lta(g: &mut G) -> String {
    if g.one_in(5) {
        return "null".to_owned();
    }
    let pool = lta_pool(g);
    let ty = if g.one_in(6) { "0" } else { "1" };
    let n = [1, 1, 1, 1, 1, 2, 0][g.below(7) as usize];
    pool_lta(g, &pool, ty, n, false, None).0
}

/// A 4 x 4 matrix for `TransformWithMatrix`: a pool's, or `mat4`'s when it is 4 x 4.
fn tal_m4(g: &mut G) -> String {
    let m = g.mat4();
    if m.starts_with("4 4 ") {
        m
    } else {
        pool_mat(g)
    }
}

/// `n` points (three doubles each), special values in some.
fn tal_points(g: &mut G) -> String {
    let n = g.int(1, 2);
    let mut v = vec![n.to_string()];
    for _ in 0..3 * n {
        let special = g.one_in(6);
        v.push(g.d(special));
    }
    v.join(" ")
}

/// A double's bits.
fn dbits(x: f64) -> String {
    format!("{:016x}", x.to_bits())
}

/// A sparse volume for §48: zeros with a few labels (negatives, and large values).
fn dil_w(g: &mut G, e: (i64, i64, i64), kind: i64, frames: i64) -> String {
    seg_w(g, e, kind, frames, &[0, 0, 0, 0, 1, 2, 5, -3, 250])
}

/// Part 10a (§46 and §48).
const PART10: &[(&str, Make)] = &[
    ("talcras", |g| {
        let e = ext(g, 1, 3);
        let kind = pi(g, &[0, 1, 3, 4]);
        let w = seg_w(g, e, kind, 1, &[0, 1, 2]);
        format!("{w} 0 {}", tal_lta(g))
    }),
    ("+env=NO_AVERAGE305= +gdiag=10000040 talcras", |g| {
        let e = ext(g, 1, 3);
        let w = seg_w(g, e, 3, 1, &[0, 1]);
        format!("{w} 0 {}", tal_lta(g))
    }),
    ("talmat", |g| {
        let k = g.int(0, 3);
        let e = ext(g, 1, 3);
        let kind = pi(g, &[0, 3]);
        let w = seg_w(g, e, kind, 1, &[0]);
        format!("{k} {w} 0 {}", tal_lta(g))
    }),
    ("+gdiag=10000000 talpt", |g| {
        let k = g.int(0, 4);
        if k == 4 {
            let m = tal_m4(g);
            return format!("4 {m} {}", tal_points(g));
        }
        let e = ext(g, 1, 3);
        let w = seg_w(g, e, 3, 1, &[0]);
        let a = tal_lta(g);
        format!("{k} {w} 0 {a} {}", tal_points(g))
    }),
    ("talvol", |g| {
        let k = g.int(0, 2);
        let e = ext(g, 1, 3);
        let kind = pi(g, &[0, 3]);
        let w = seg_w(g, e, kind, 1, &[0, 1, 2, 5]);
        let t = if k == 2 || g.one_in(2) {
            let te = ext(g, 1, 3);
            format!("{} 0", seg_w(g, te, 3, 1, &[0]))
        } else {
            "new".to_owned()
        };
        let a = tal_lta(g);
        let interp = if k == 0 {
            format!(" {}", g.pick(&["0", "1", "2"]))
        } else {
            String::new()
        };
        format!("{k} {w} 0 {t} {a}{interp}")
    }),
    ("dilate", |g| {
        let e = ext(g, 1, 3);
        let (kind, nf) = (pi(g, &[0, 1, 3, 4]), g.int(1, 2));
        let s = dil_w(g, e, kind, nf);
        let d = match g.int(0, 3) {
            0 => "new".to_owned(),
            1 => "self".to_owned(),
            2 => {
                let k = pi(g, &[0, 3]);
                seg_w(g, e, k, nf, &[0])
            }
            _ => {
                let o = ext(g, 1, 3);
                seg_w(g, o, 3, nf, &[0])
            }
        };
        format!("{s} 0 {d}")
    }),
    ("dilu", |g| {
        let e = ext(g, 1, 3);
        let kind = pi(g, &[0, 0, 3]);
        let s = dil_w(g, e, kind, 1);
        let d = match g.int(0, 2) {
            0 => "new".to_owned(),
            1 => "self".to_owned(),
            _ => seg_w(g, e, 0, 1, &[0]),
        };
        format!("{s} 0 {d}")
    }),
    ("msegs", |g| {
        let e = ext(g, 1, 3);
        let kind = pi(g, &[0, 1, 3]);
        let s = dil_w(g, e, kind, 1);
        let d = match g.int(0, 2) {
            0 => "new".to_owned(),
            1 => "self".to_owned(),
            _ => seg_w(g, e, 3, 1, &[0]),
        };
        let n = g.int(0, 3);
        let list: Vec<String> = (0..n)
            .map(|_| g.pick(&["0", "1", "2", "5", "-3", "250"]).to_owned())
            .collect();
        let sep = if n > 0 { " " } else { "" };
        format!(
            "{s} 0 {d} {n}{sep}{} {}",
            list.join(" "),
            g.pick(&["1", "9", "-2"])
        )
    }),
    ("sstat", |g| {
        let e = ext(g, 1, 3);
        let nf = g.int(1, 2);
        let (ks, kv) = (pi(g, &[0, 1, 3]), pi(g, &[0, 3]));
        let s = dil_w(g, e, ks, 1);
        let v = seg_w(g, e, kv, nf, &[0, 1, 2, 5, -3, 7]);
        let id = g.pick(&["0", "1", "2", "5", "7"]);
        format!("{s} 0 {v} 0 {id} {}", g.int(0, nf - 1))
    }),
    ("hseg", |g| {
        let e = ext(g, 1, 3);
        let ks = pi(g, &[0, 1, 3]);
        let s = dil_w(g, e, ks, 1);
        let v = seg_w(g, e, 3, 1, &[0, 1, 2, 5, -3, 7]);
        let id = g.pick(&["0", "1", "2", "5"]);
        let lo = [0.0, -4.0, 0.5][g.below(3) as usize];
        let hi = lo + [8.0, 3.0, 0.0, -2.0][g.below(4) as usize];
        let dl = [1.0, 2.0, 0.5][g.below(3) as usize];
        format!("{s} 0 {v} 0 {id} {} {} {}", dbits(lo), dbits(hi), dbits(dl))
    }),
];

#[test]
fn part10_talairach_dilation_and_segments() {
    group("part10", PART10, 4);
}

/// A float volume filled from the probe's table (`t<seed>`: zeros, halves, NaNs, infinities, ...),
/// in `seg_w`'s ordinary geometry.
fn tab_w(g: &mut G, (w, h, d): (i64, i64, i64), frames: i64) -> String {
    let seed = g.int(0, 33);
    let mut f = vec![g.size(false), g.size(false), g.size(false), "1".to_owned()];
    f.extend(g.cosines(false));
    f.extend((0..3).map(|_| g.f(false)));
    f.extend(["00000000"; 3].map(str::to_owned));
    let z11 = ["00000000"; 11].join(" ");
    format!(
        "{w} {h} {d} 3 {frames} 0 t{seed} {} 1 {d} {z11} 0000000000000000 0 00000000 00000000 0 1 null null",
        f.join(" ")
    )
}

/// A label volume of §50 (labels inside and outside the byte range, halves for floats).
fn lab_w(g: &mut G, e: (i64, i64, i64), kind: i64, frames: i64) -> String {
    if kind == 3 && g.one_in(3) {
        return tab_w(g, e, frames);
    }
    seg_w(g, e, kind, frames, &[0, 0, 1, 2, 5, -3, 250, 300])
}

/// A destination of §50: none, the source itself, the second volume (`two`) or a volume of the
/// extent.
fn dst50(g: &mut G, e: (i64, i64, i64), two: bool) -> String {
    match g.int(0, if two { 3 } else { 2 }) {
        0 => "new".to_owned(),
        1 => "self".to_owned(),
        2 => {
            let k = pi(g, &[0, 3]);
            format!("{} 0", seg_w(g, e, k, 1, &[0]))
        }
        _ => "other".to_owned(),
    }
}

/// Part 10b (§49 and §50); `MRIconform`'s 256^3 result is beyond the golden size (rule 10):
/// its matrix and the template only.
const PART10B: &[(&str, Make)] = &[
    ("conform", |g| {
        let e = ext(g, 1, 3);
        let w = {
            let k = pi(g, &[0, 3]);
            let f = 1;
            seg_w(g, e, k, f, &[0, 1, 2])
        };
        if g.one_in(2) {
            return format!("1 {w} 0");
        }
        let wd = g.pick(&["0", "4", "8"]);
        let sz = [1.0, 0.5, 2.0][g.below(3) as usize];
        format!("2 {w} 0 {wd} {} {}", dbits(sz), g.int(0, 1))
    }),
    ("union", |g| {
        let e = ext(g, 1, 3);
        let (a, b) = (lab_w(g, e, 0, 1), {
            let k = pi(g, &[0, 3]);
            let f = 1;
            lab_w(g, e, k, f)
        });
        format!("{a} 0 {b} 0 {}", dst50(g, e, true))
    }),
    ("isect", |g| {
        let e = ext(g, 1, 3);
        let (a, b) = (lab_w(g, e, 3, 1), lab_w(g, e, 1, 1));
        format!("{a} 0 {b} 0 {}", dst50(g, e, true))
    }),
    ("mand", |g| {
        let e = ext(g, 1, 3);
        let (a, b) = (lab_w(g, e, 3, 1), lab_w(g, e, 0, 1));
        format!("{a} 0 {b} 0 {} {}", dst50(g, e, true), g.int(-1, 3))
    }),
    ("mxor", |g| {
        let e = ext(g, 1, 3);
        let (a, b) = (lab_w(g, e, 1, 1), lab_w(g, e, 3, 1));
        format!(
            "{a} 0 {b} 0 {} {} {}",
            dst50(g, e, true),
            g.int(0, 2),
            g.int(1, 5)
        )
    }),
    ("compl", |g| {
        let e = ext(g, 1, 3);
        format!(
            "{} 0 {}",
            {
                let k = pi(g, &[0, 3]);
                let f = 1;
                lab_w(g, e, k, f)
            },
            dst50(g, e, false)
        )
    }),
    ("erode", |g| {
        let e = ext(g, 1, 3);
        format!(
            "{} 0 {}",
            {
                let k = pi(g, &[0, 3, 4]);
                let f = g.int(1, 2);
                lab_w(g, e, k, f)
            },
            dst50(g, e, false)
        )
    }),
    ("erode2d", |g| {
        let e = ext(g, 1, 3);
        format!(
            "{} 0 {}",
            {
                let k = pi(g, &[0, 3]);
                let f = 1;
                lab_w(g, e, k, f)
            },
            dst50(g, e, false)
        )
    }),
    ("dil6", |g| {
        let e = ext(g, 1, 3);
        format!("{} 0 {}", lab_w(g, e, 3, 1), dst50(g, e, false))
    }),
    ("mnot", |g| {
        let e = ext(g, 1, 3);
        format!(
            "{} 0 {}",
            {
                let k = pi(g, &[1, 3]);
                let f = 1;
                lab_w(g, e, k, f)
            },
            dst50(g, e, false)
        )
    }),
    ("b2c", |g| {
        let e = ext(g, 1, 3);
        format!("{} 0 new", lab_w(g, e, 3, 1))
    }),
    ("edges", |g| {
        let e = ext(g, 1, 3);
        let v = [7.5, -1.0, f64::NAN][g.below(3) as usize];
        format!(
            "{} 0 {} {}",
            {
                let f = g.int(1, 2);
                lab_w(g, e, 3, f)
            },
            dbits(v),
            dst50(g, e, false)
        )
    }),
    ("erodenn", |g| {
        let e = ext(g, 1, 3);
        let d = if g.one_in(3) { "self" } else { "new" };
        format!(
            "{} 0 {d} {} {}",
            lab_w(g, e, 3, 1),
            g.int(0, 3),
            g.int(0, 1)
        )
    }),
    ("erodeseg", |g| {
        let e = ext(g, 2, 4);
        format!(
            "{} 0 new {} {}",
            lab_w(g, e, 1, 1),
            g.int(1, 2),
            g.int(0, 3)
        )
    }),
    ("dilseg", |g| {
        let e = ext(g, 1, 3);
        let mask = if g.one_in(2) {
            "null".to_owned()
        } else {
            format!("{} 0", lab_w(g, e, 3, 1))
        };
        let n = g.pick(&["1", "2", "-1"]);
        format!("{} 0 new {n} {mask} 0 {}", lab_w(g, e, 1, 1), dbits(0.5))
    }),
    ("dtl", |g| {
        let e = ext(g, 1, 4);
        let (a, b) = (lab_w(g, e, 0, 1), lab_w(g, e, 0, 1));
        let lab = g.pick(&["1", "2", "300"]);
        format!(
            "{} {a} 0 {b} 0 {} {lab} {} {}",
            g.int(0, 1),
            dst50(g, e, true),
            g.int(0, 3),
            g.int(0, 3)
        )
    }),
    ("dlab", |g| {
        let e = ext(g, 1, 3);
        let k = g.int(0, 1);
        let a = {
            let k = pi(g, &[0, 3]);
            let f = 1;
            lab_w(g, e, k, f)
        };
        let d = if g.one_in(2) { "self" } else { "new" };
        format!(
            "{k} {a} 0 {d} {} {}",
            g.pick(&["1", "2", "250"]),
            g.int(0, 2)
        )
    }),
    ("openn", |g| {
        let e = ext(g, 1, 3);
        let k = g.int(0, 2);
        let a = {
            let k = pi(g, &[0, 3]);
            let f = 1;
            lab_w(g, e, k, f)
        };
        let order = if k == 0 {
            format!(" {}", g.int(-1, 2))
        } else {
            String::new()
        };
        format!("{k} {a} 0 {}{order}", dst50(g, e, false))
    }),
    ("rlist", |g| {
        let e = ext(g, 1, 3);
        let a = {
            let k = pi(g, &[1, 3]);
            let f = 1;
            lab_w(g, e, k, f)
        };
        let mask = if g.one_in(2) {
            "null".to_owned()
        } else {
            format!("{} 0", lab_w(g, e, 3, 1))
        };
        let d = if g.one_in(3) {
            format!("{} 0", seg_w(g, e, 0, 1, &[0]))
        } else {
            "new".to_owned()
        };
        format!("{a} 0 2 1 7 5 -2 {mask} {d}")
    }),
    ("rvonly", |g| {
        let e = ext(g, 1, 3);
        let a = {
            let k = pi(g, &[0, 3]);
            let f = g.int(1, 2);
            lab_w(g, e, k, f)
        };
        let (i, o) = (
            g.pick(&["3f800000", "40000000"]),
            g.pick(&["40e00000", "7fc00000"]),
        );
        format!("{a} 0 {} {i} {o}", dst50(g, e, false))
    }),
    ("mmask", |g| {
        let e = ext(g, 1, 3);
        let (a, m) = (lab_w(g, e, 3, 1), {
            let k = pi(g, &[0, 3]);
            let f = 1;
            lab_w(g, e, k, f)
        });
        format!(
            "{} {a} 0 {m} 0 {} {} 40f00000",
            g.int(0, 1),
            dst50(g, e, true),
            g.int(0, 2)
        )
    }),
    ("+env=FS_MRIMASK_ALLOW_DIFF_GEOM=0 +gdiag=1 mmask 0", |g| {
        let e = ext(g, 1, 3);
        let (a, m) = (lab_w(g, e, 3, 1), lab_w(g, e, 3, 1));
        format!("{a} 0 {m} 0 new 1 40f00000")
    }),
    ("tmask", |g| {
        let e = ext(g, 1, 3);
        let k = pi(g, &[0, 3]);
        let (a, m) = (lab_w(g, e, k, 1), lab_w(g, e, k, 1));
        let o = g.pick(&["43960000", "bf800000", "40400000", "7fc00000"]);
        format!("{a} 0 {m} 0 {} 3f800000 {o}", dst50(g, e, true))
    }),
    ("grow", |g| {
        let e = ext(g, 2, 4);
        let (a, f) = (lab_w(g, e, 0, 1), seg_w(g, e, 0, 1, &[0, 0, 0, 9]));
        format!("{a} 0 {f} 0 {} 9", g.pick(&["0", "1", "2"]))
    }),
    ("onfg", |g| {
        let e = ext(g, 1, 3);
        let (a, f, b) = (lab_w(g, e, 0, 1), lab_w(g, e, 0, 1), lab_w(g, e, 0, 1));
        format!("{a} 0 {f} 0 {b} 0")
    }),
    ("offbg", |g| {
        let e = ext(g, 2, 4);
        let (a, b) = (
            seg_w(g, e, 0, 1, &[0, 0, 0, 7, 9]),
            seg_w(g, e, 0, 1, &[0, 0, 1]),
        );
        format!("{a} 0 {b} 0")
    }),
    ("cplab", |g| {
        let e = ext(g, 1, 3);
        let a = {
            let k = pi(g, &[0, 3]);
            let f = 1;
            lab_w(g, e, k, f)
        };
        let d = if g.one_in(2) {
            "self".to_owned()
        } else {
            format!("{} 0", {
                let k = pi(g, &[0, 3]);
                let f = 1;
                seg_w(g, e, k, f, &[0])
            })
        };
        format!("{a} 0 {d} {}", g.pick(&["1", "2", "300"]))
    }),
    ("cplv", |g| {
        let e = ext(g, 1, 3);
        let (a, l) = (lab_w(g, e, 3, 1), lab_w(g, e, 1, 1));
        let d = match g.int(0, 2) {
            0 => "self".to_owned(),
            1 => "other".to_owned(),
            _ => format!("{} 0", seg_w(g, e, 3, 1, &[0])),
        };
        format!("{a} 0 {l} 0 {d} {}", g.pick(&["1", "2"]))
    }),
    ("slv", |g| {
        let e = ext(g, 1, 3);
        let (a, l) = (lab_w(g, e, 3, 1), lab_w(g, e, 1, 1));
        format!(
            "{a} 0 {l} 0 {} {} 41200000",
            dst50(g, e, true),
            g.pick(&["1", "2"])
        )
    }),
    ("cntv", |g| {
        let e = ext(g, 1, 3);
        let k = g.int(0, 3);
        let a = {
            let k = pi(g, &[0, 3]);
            let f = g.int(1, 2);
            lab_w(g, e, k, f)
        };
        if k == 3 {
            return format!("3 {a} 0");
        }
        format!("{k} {a} 0 {}", g.pick(&["0", "1", "2", "300"]))
    }),
    ("cnbhd", |g| {
        let e = ext(g, 1, 3);
        let a = lab_w(g, e, 1, 1);
        format!("{a} 0 3 3 0 0 0 1 1 1 1 0 2 5 2 1 1 0")
    }),
    ("cent", |g| {
        let e = ext(g, 1, 3);
        format!("{} 0", lab_w(g, e, 3, 1))
    }),
    ("pthr", |g| {
        let e = ext(g, 1, 3);
        let a = lab_w(g, e, 3, 1);
        let mask = if g.one_in(2) {
            "null".to_owned()
        } else {
            format!("{} 0", lab_w(g, e, 3, 1))
        };
        let pct = [50.0, 10.0, 99.0, 0.0, f64::NAN][g.below(5) as usize];
        format!("{a} 0 {mask} 0 {}", dbits(pct))
    }),
    ("dilo", |g| {
        let e = ext(g, 1, 3);
        let v = [5.0, f64::NAN][g.below(2) as usize];
        format!("{} 0 {} new", lab_w(g, e, 3, 1), dbits(v))
    }),
];

#[test]
fn part10b_conforming_sets_morphology_masks() {
    group("part10b", PART10B, 3);
}

/// A volume of §52: its buffer's words from values of its type (bytes and shorts packed, floats
/// with NaNs and an infinity now and then), an ordinary geometry.
fn hist_w(g: &mut G, (w, h, d): (i64, i64, i64), kind: i64, frames: i64, seg: bool) -> String {
    let n = (w * h * d * frames).max(1) as usize;
    let words: Vec<String> = (0..n)
        .map(|_| match kind {
            0 => {
                let v: &[u32] = if seg {
                    &[80, 110, 80, 110, 128, 90, 120]
                } else {
                    &[0, 1, 2, 5, 80, 110, 200, 255]
                };
                (0..4)
                    .map(|k| v[g.below(v.len() as u64) as usize] << (8 * k))
                    .fold(0u32, |a, b| a | b)
                    .to_string()
            }
            4 | 10 => {
                let v: &[i64] = if kind == 4 {
                    &[-5, 0, 1, 2, 50, 300]
                } else {
                    &[0, 1, 2, 50, 300]
                };
                let a = v[g.below(v.len() as u64) as usize] as u32 & 0xffff;
                let b = v[g.below(v.len() as u64) as usize] as u32 & 0xffff;
                (a | (b << 16)).to_string()
            }
            3 => {
                const V: [u32; 10] = [
                    0x0000_0000,
                    0x3f80_0000,
                    0x4020_0000,
                    0x42c8_0000,
                    0xc040_0000,
                    0x437a_0000,
                    0x42a0_0000,
                    0x42dc_0000,
                    0x7fc0_0000,
                    0xffc1_2345,
                ];
                if g.one_in(40) {
                    return 0x7f80_0000u32.to_string();
                }
                let k = if g.one_in(6) { g.below(10) } else { g.below(8) };
                V[k as usize].to_string()
            }
            _ => g.int(-3, 9).to_string(),
        })
        .collect();
    let mut f = vec![g.size(false), g.size(false), g.size(false), "1".to_owned()];
    f.extend(g.cosines(false));
    f.extend((0..3).map(|_| g.f(false)));
    f.extend(["00000000"; 3].map(str::to_owned));
    let z11 = ["00000000"; 11].join(" ");
    format!(
        "{w} {h} {d} {kind} {frames} 0 i{} {} 1 {d} {z11} 0000000000000000 0 00000000 00000000 0 1 null null",
        words.join(","),
        f.join(" ")
    )
}

/// H, a given histogram (or `null`): a few bins allocated, some in use, old contents.
fn hist_h(g: &mut G) -> String {
    if g.one_in(2) {
        return "null".to_owned();
    }
    let alloc = pi(g, &[0, 1, 4, 16, 40]);
    let nb = g.int(0, alloc);
    let mut p = vec![
        alloc.to_string(),
        nb.to_string(),
        g.pick(&["3f000000", "3f800000"]).to_owned(),
        format!("{:08x}", (g.int(-5, 5) as f32).to_bits()),
        format!("{:08x}", (g.int(5, 50) as f32).to_bits()),
    ];
    p.extend((0..nb).map(|_| format!("{:08x}", (g.int(-10, 300) as f32).to_bits())));
    p.extend((0..nb).map(|_| format!("{:08x}", (g.int(0, 9) as f32).to_bits())));
    p.join(" ")
}

/// R, a region of the extent: inside, overlapping or outside, sometimes empty.
fn hist_r(g: &mut G, (w, h, d): (i64, i64, i64)) -> String {
    let mut a = Vec::new();
    for e in [w, h, d] {
        a.push(g.int(-1, e));
    }
    for e in [w, h, d] {
        a.push(g.int(-1, e + 1));
    }
    a.iter().map(i64::to_string).collect::<Vec<_>>().join(" ")
}

/// One of the histogram volumes' types (the region histogram refuses ints and longs).
fn hist_kind(g: &mut G) -> i64 {
    pi(g, &[0, 0, 3, 3, 4, 10, 1, 2])
}

/// Part 10c (§52).
const PART10C: &[(&str, Make)] = &[
    ("hreg", |g| {
        let e = ext(g, 1, 3);
        let (k, f) = (hist_kind(g), g.int(1, 2));
        let w = hist_w(g, e, k, f, false);
        let h = hist_h(g);
        let nb = g.pick(&["0", "0", "1", "3", "-1"]);
        format!("{w} 0 {h} {nb} {}", hist_r(g, e))
    }),
    ("hvox", |g| {
        let e = ext(g, 1, 3);
        let k = hist_kind(g);
        let w = hist_w(g, e, k, 1, false);
        let h = hist_h(g);
        let nb = g.pick(&["0", "0", "2", "-1"]);
        let (x, y, z) = (g.int(-1, e.0), g.int(-1, e.1), g.int(-1, e.2));
        let ws = g.int(0, 3);
        let t = if g.one_in(2) {
            "null".to_owned()
        } else {
            let k = pi(g, &[0, 3]);
            format!("{} 0", hist_w(g, e, k, 1, false))
        };
        let th = g.pick(&["00000000", "42480000", "7fc00000"]);
        format!("{w} 0 {h} {nb} {x} {y} {z} {ws} {t} {th}")
    }),
    ("hthr", |g| {
        let e = ext(g, 1, 3);
        let (k, f) = (hist_kind(g), g.int(1, 2));
        let w = hist_w(g, e, k, f, false);
        let h = hist_h(g);
        let nb = g.pick(&["0", "2", "5"]);
        let r = hist_r(g, e);
        let tk = pi(g, &[0, 3]);
        let t = hist_w(g, e, tk, f, false);
        let th = g.pick(&["00000000", "42480000", "7fc00000"]);
        format!("{w} 0 {h} {nb} {r} {t} 0 {th} {}", g.int(0, f))
    }),
    ("hist", |g| {
        let e = ext(g, 1, 3);
        let (k, f) = (hist_kind(g), g.int(1, 3));
        let w = hist_w(g, e, k, f, false);
        format!("{w} 0 {}", g.pick(&["0", "0", "1", "2", "5"]))
    }),
    ("geq", |g| {
        let e = ext(g, 1, 3);
        let k = pi(g, &[0, 0, 3, 4, 1]);
        let w = hist_w(g, e, k, 1, false);
        let h = hist_h(g);
        let lo = g.pick(&["0", "1", "-1", "3"]);
        let hi = g.pick(&["5", "300", "2"]);
        format!("{w} 0 {h} {lo} {hi} {}", g.int(0, 1))
    }),
    ("heq", |g| {
        let e = ext(g, 1, 3);
        let (ka, kb) = (pi(g, &[0, 0, 3, 1]), pi(g, &[0, 3]));
        let (a, b) = (hist_w(g, e, ka, 1, false), hist_w(g, e, kb, 1, false));
        let d = match g.int(0, 3) {
            0 => "new".to_owned(),
            1 => "self".to_owned(),
            2 => "other".to_owned(),
            _ => format!("{} 0", seg_w(g, e, 3, 1, &[0])),
        };
        format!(
            "{a} 0 {b} 0 {d} {} {}",
            g.pick(&["0", "1"]),
            g.pick(&["255", "100", "40"])
        )
    }),
    ("hlr", |g| {
        let e = ext(g, 1, 3);
        let k = hist_kind(g);
        let w = hist_w(g, e, k, 1, false);
        let lab = if g.one_in(4) {
            "self".to_owned()
        } else {
            let k = pi(g, &[0, 3, 1]);
            format!("{} 0", seg_w(g, e, k, 1, &[1, 2, 3]))
        };
        let r = hist_r(g, e);
        format!(
            "{w} 0 {lab} {r} {} {}",
            g.int(0, 3),
            g.pick(&["0", "0", "-1", "3"])
        )
    }),
    ("hlab", |g| {
        let e = ext(g, 1, 3);
        let k = hist_kind(g);
        let w = hist_w(g, e, k, 1, false);
        let lab = if g.one_in(4) {
            "self".to_owned()
        } else {
            let k = pi(g, &[0, 3, 1]);
            format!("{} 0", seg_w(g, e, k, 1, &[0, 1, 2, 3]))
        };
        format!("{w} 0 {lab} {} {}", g.int(1, 3), g.pick(&["0", "-1", "3"]))
    }),
    ("hsegm", |g| {
        let e = ext(g, 2, 3);
        let k = pi(g, &[0, 0, 3]);
        let w = hist_w(g, e, k, 1, true);
        let d = match g.int(0, 3) {
            0 => "new".to_owned(),
            1 => "self".to_owned(),
            _ => format!("{} 0", seg_w(g, e, 0, 1, &[128, 128, 0, 255])),
        };
        format!(
            "{w} 0 {d} {} {} {} {} {}",
            g.pick(&["85", "90"]),
            g.pick(&["125", "130"]),
            g.pick(&["95", "100"]),
            g.pick(&["1", "3", "2"]),
            g.pick(&["bf800000", "3f800000", "40000000"])
        )
    }),
    ("+gdiag=4040 hsegm", |g| {
        let e = ext(g, 2, 3);
        let w = hist_w(g, e, 0, 1, true);
        format!("{w} 0 self 85 130 100 3 40000000")
    }),
];

#[test]
fn part10c_histograms() {
    group("part10c", PART10C, 4);
}

/// A binary-like volume for §59 and §60 (zeros, ones, labels, special values) of a type, painted
/// with nothing.
fn bin_w(g: &mut G, e: (i64, i64, i64), kind: i64, frames: i64) -> String {
    format!(
        "{} 0",
        seg_w(g, e, kind, frames, &[0, 0, 1, 1, 1, 2, 255, -1])
    )
}

/// M: `null` or a binary-like volume.
fn bin_m(g: &mut G, e: (i64, i64, i64)) -> String {
    if g.one_in(3) {
        "null".to_owned()
    } else {
        let k = pi(g, &[0, 3]);
        bin_w(g, e, k, 1)
    }
}

/// BF, a `BinarizeMRI`'s fields.
fn bin_fields(g: &mut G) -> String {
    let ty = pi(g, &[1, 1, 2, 3]);
    let n = if ty == 2 || g.one_in(6) {
        g.int(0, 2)
    } else {
        0
    };
    let list: Vec<String> = (0..n).map(|_| g.int(0, 2).to_string()).collect();
    let th = |g: &mut G| dbits([0.5, 1.5, f64::INFINITY, -1.0, f64::NAN][g.below(5) as usize]);
    let (lo, hi) = (th(g), th(g));
    let (mlo, mhi) = (dbits(0.5), dbits(f64::INFINITY));
    let edges: Vec<String> = (0..3).map(|_| g.int(0, 1).to_string()).collect();
    let on = dbits([1.0, 300.0, 2.5][g.below(3) as usize]);
    let off = dbits([0.0, -1.0][g.below(2) as usize]);
    let (fs, fe) = (g.int(-1, 1), g.int(-1, 2));
    format!(
        "{ty} {lo} {hi} {n} {} {mlo} {mhi} {} {on} {off} {fs} {fe} {} {} {} {}",
        list.join(" "),
        edges.join(" "),
        g.int(0, 1),
        g.pick(&["-1", "-1", "0", "3"]),
        g.int(0, 1),
        g.int(0, 1)
    )
}

/// DF, a `DEMorphBinVol`'s fields.
fn dem_fields(g: &mut G) -> String {
    format!(
        "{} {} {} {} {} {} {}",
        g.pick(&["1", "2", "1", "2", "1", "2", "0"]),
        g.pick(&["1", "2", "3", "1", "2", "3", "4", "0"]),
        g.int(-1, 3),
        g.int(0, 2),
        g.pick(&["0", "0", "0", "1"]),
        g.pick(&["-1", "-1", "0", "3", "1"]),
        g.int(0, 1)
    )
}

/// Part 11a (§59, §60) and §64's and §65's functions of `mri_cc`.
const PART11A: &[(&str, Make)] = &[
    ("vvfill", |g| {
        let pool = lta_pool(g);
        let ty = g.pick(&["0", "1"]);
        let (lta, s, d) = pool_lta(g, &pool, ty, 1, false, None);
        let k = if g.one_in(2) { s } else { d };
        let w = pool_w(&pool, k);
        let m = if g.one_in(2) {
            "null".to_owned()
        } else {
            format!("{} 0", pool_w(&pool, k))
        };
        format!("{w} 0 {m} {lta} {} {} new", g.int(-1, 2), g.int(0, 1))
    }),
    ("fbnwm", |g| {
        let e = ext(g, 3, 5);
        let (k1, k2) = (pi(g, &[0, 0, 3, 4, 1]), pi(g, &[0, 0, 3]));
        let t1 = hist_w(g, e, k1, 1, false);
        let wm = hist_w(g, e, k2, 1, false);
        format!("{t1} 0 {wm} 0")
    }),
    ("qecc", |g| {
        let e = ext(g, 2, 4);
        let k = pi(g, &[0, 3]);
        let w = bin_w(g, e, k, 1);
        let m = bin_m(g, e);
        let (x, y, z) = (g.int(-1, e.0), g.int(-1, e.1), g.int(-1, e.2));
        format!("{w} {m} {x} {y} {z}")
    }),
    ("vtero", |g| {
        let e = ext(g, 3, 5);
        let k = pi(g, &[0, 3]);
        let w = bin_w(g, e, k, 1);
        format!("{w} {}", bin_m(g, e))
    }),
    ("vtdil", |g| {
        let e = ext(g, 2, 4);
        let k = pi(g, &[0, 3]);
        let w = bin_w(g, e, k, 1);
        format!("{w} {}", g.pick(&["1", "2", "255", "0"]))
    }),
    ("fscm", |g| {
        let e = ext(g, 2, 4);
        let scm = hist_w(g, e, 3, 1, false);
        let aseg = seg_w(g, e, 1, 1, &[0, 2, 3, 41, 42, 17, 53, 18, 54, 5, 44, 10]);
        format!("{scm} 0 {aseg} 0 {}", g.int(-1, 2))
    }),
    ("binmri", |g| {
        let e = ext(g, 2, 3);
        let f = bin_fields(g);
        let k = g.int(1, 2);
        let mut calls = Vec::new();
        for _ in 0..k {
            let fr = g.int(1, 3);
            let kind = pi(g, &[0, 3]);
            let a = if g.one_in(8) {
                "null".to_owned()
            } else {
                bin_w(g, e, kind, fr)
            };
            let m = bin_m(g, e);
            let d = g.pick(&["new", "new", "self"]).to_owned();
            calls.push(format!("{a} {m} {d}"));
        }
        format!("{f} {k} {}", calls.join(" "))
    }),
    ("binq", |g| {
        let e = ext(g, 2, 3);
        let f = bin_fields(g);
        let w = bin_w(g, e, 3, 2);
        let m = bin_m(g, e);
        let (x, y, z) = (g.int(0, e.0 - 1), g.int(0, e.1 - 1), g.int(0, e.2 - 1));
        format!("{f} {w} {m} {x} {y} {z} {}", g.int(-1, 1))
    }),
    ("bindump", |g| {
        let e = ext(g, 1, 2);
        let f = bin_fields(g);
        format!("{f} {}", bin_m(g, e))
    }),
    ("demchk", |g| {
        let e = ext(g, 3, 4);
        let f = dem_fields(g);
        let a = bin_m(g, e);
        let m = bin_m(g, e);
        let e2 = if g.one_in(3) { ext(g, 3, 4) } else { e };
        format!("{f} {a} {m} {}", bin_m(g, e2))
    }),
    ("demdump", dem_fields),
    ("demcf", |g| {
        let e = ext(g, 3, 4);
        let f = dem_fields(g);
        let fr = g.int(1, 2);
        let k = pi(g, &[0, 3]);
        format!("{f} {} {}", bin_w(g, e, k, fr), g.int(-1, 2))
    }),
    ("demm", |g| {
        let e = ext(g, 3, 5);
        let f = dem_fields(g);
        let k = pi(g, &[0, 3]);
        let fr = g.int(1, 2);
        format!("{f} {}", bin_w(g, e, k, fr))
    }),
    ("demt", |g| {
        let e = ext(g, 3, 5);
        let f = dem_fields(g);
        let k = pi(g, &[0, 3]);
        let fr = g.int(1, 2);
        let w = bin_w(g, e, k, fr);
        let lo = dbits([0.5, 1.5, -1.0][g.below(3) as usize]);
        let hi = dbits([f64::INFINITY, 1.0, 300.0][g.below(3) as usize]);
        format!("{f} {w} {lo} {hi} {} null", bin_m(g, e))
    }),
    ("deml", |g| {
        let e = ext(g, 3, 5);
        let f = dem_fields(g);
        let k = pi(g, &[0, 1]);
        let w = bin_w(g, e, k, 1);
        let n = g.int(0, 2);
        let list: Vec<String> = (0..n).map(|_| g.int(0, 2).to_string()).collect();
        format!("{f} {w} {n} {} {} null", list.join(" "), bin_m(g, e))
    }),
    ("trns", |g| {
        let t = if g.one_in(6) {
            "null".to_owned()
        } else {
            pool_mat(g)
        };
        let o = if g.one_in(6) {
            "null".to_owned()
        } else {
            format!("4 4 {}", ["00000000"; 16].join(" "))
        };
        format!("{} {t} {o}", g.int(0, 1))
    }),
    ("vlcir", |g| {
        let e = ext(g, 2, 4);
        let k = pi(g, &[0, 3]);
        let w = bin_w(g, e, k, 1);
        let (lo, hi) = (
            g.pick(&["3f800000", "00000000"]),
            g.pick(&["3f800000", "40000000"]),
        );
        let vl = match g.below(3) {
            0 => "null".to_owned(),
            1 => format!("{} 1", g.int(1, 3)),
            _ => format!("{} 2", g.int(20, 40)),
        };
        format!(
            "{w} {lo} {hi} {vl} {} {} {}",
            g.int(0, 2),
            g.int(0, 1),
            hist_r(g, e)
        )
    }),
];

#[test]
fn part11a_fills_topology_binary_volumes_and_lists() {
    group("part11a", PART11A, 4);
}

/// VF, a voxel list with its fields: VL, or `f alloc nvox type mask k` and `k` voxels (indices
/// 0 to 3, values with ties and NaN).
fn vf(g: &mut G) -> String {
    if g.one_in(3) {
        return format!("{} {}", g.int(1, 6), g.int(0, 6));
    }
    let alloc = g.int(1, 6);
    let k = g.int(0, alloc + 1);
    let mut p = vec![
        "f".to_owned(),
        alloc.to_string(),
        g.int(0, alloc).to_string(),
        g.int(0, 2).to_string(),
        g.int(0, 15).to_string(),
        k.to_string(),
    ];
    for _ in 0..k {
        for _ in 0..4 {
            p.push(g.int(0, 3).to_string());
        }
        for _ in 0..3 {
            p.push(g.pick(&["3f000000", "3fc00000", "c0000000"]).to_owned());
        }
        p.push(
            g.pick(&["3f800000", "40000000", "40000000", "7fc00000", "80000000"])
                .to_owned(),
        );
        p.push(g.f(false));
    }
    p.join(" ")
}

/// An array of 0 to 2 transforms of a type over a fresh pool, with the pool and its ends.
fn p11b_lta(g: &mut G, types: &[&str], most: i64) -> (String, Vec<(String, String)>, usize, usize) {
    let pool = lta_pool(g);
    let ty = g.pick(types).to_owned();
    let lo = if g.one_in(8) { 0 } else { 1 };
    let n = g.int(lo, most) as usize;
    let (lta, s, d) = pool_lta(g, &pool, &ty, n, false, None);
    (lta, pool, s, d)
}

/// Part 11b (§62 to §65): the commands without files; the writers and `DevolveXFM` are
/// `part11b_writers`'.
const PART11B: &[(&str, Make)] = &[
    ("ltadvg", |g| g.geom(true)),
    ("ltapt", |g| {
        let (lta, ..) = p11b_lta(g, &["0", "1"], 3);
        let m = if g.one_in(2) {
            "null".to_owned()
        } else {
            format!("4 4 {}", ["00000000"; 16].join(" "))
        };
        format!(
            "{lta} {} {} {} {m} {}",
            g.f(false),
            g.f(false),
            g.f(false),
            g.pick(&["0", "77"])
        )
    }),
    ("ltaxf", |g| {
        let (lta, pool, s, d) = p11b_lta(g, &["0", "1", "2", "1", "0", "14"], 2);
        let w = pool_w(&pool, s);
        let dst = if g.one_in(2) {
            "new".to_owned()
        } else {
            pool_w(&pool, d)
        };
        format!("{w} 0 {dst} {lta} {}", g.pick(&["-1", "0", "1", "3"]))
    }),
    ("ltacor", |g| p11b_lta(g, &["0"], 1).0),
    ("fixmni", |g| format!("{} {} {}", g.f(true), g.f(true), g.f(true))),
    ("ltamod", |g| {
        let (lta, pool, s, d) = p11b_lta(g, &["0", "1"], 2);
        let a = if g.one_in(3) {
            "null".to_owned()
        } else {
            format!("{} 0", pool_w(&pool, s))
        };
        let b = if g.one_in(3) {
            "null".to_owned()
        } else {
            format!("{} 0", pool_w(&pool, (d + g.int(0, 1) as usize) % 3))
        };
        let t = dbits([f64::from(f32::EPSILON), 1e-3, 0.0, f64::NAN][g.below(4) as usize]);
        format!("{lta} {a} {b} {t}")
    }),
    ("angrm", |g| format!("{} {} {}", g.d(true), g.d(true), g.d(true))),
    ("affp2m", |g| {
        let p: Vec<String> = (0..12).map(|_| g.d(true)).collect();
        let m = if g.one_in(2) { "null".to_owned() } else { g.mat4() };
        format!("{} {m}", p.join(" "))
    }),
    ("affm2p", |g| g.mat4()),
    ("ltarot", |g| p11b_lta(g, &["0", "1"], 1).0),
    ("rmsmj", |g| format!("{} {} {}", pool_mat(g), pool_mat(g), g.d(true))),
    ("vlcre", |g| {
        let e = ext(g, 2, 4);
        let k = pi(g, &[0, 3]);
        let fr = g.int(1, 2);
        let w = bin_w(g, e, k, fr);
        let (lo, hi) = (
            g.pick(&["3f800000", "00000000"]),
            g.pick(&["3f800000", "40000000"]),
        );
        let l = if g.one_in(3) { "null".to_owned() } else { vf(g) };
        format!("{w} {lo} {hi} {l} {} {}", g.int(0, 2), g.int(0, 1))
    }),
    ("vlsort", |g| {
        let a = vf(g);
        let b = if g.one_in(2) { "null".to_owned() } else { vf(g) };
        format!("{a} {b}")
    }),
    ("vlfree", |g| if g.one_in(2) { "null".to_owned() } else { vf(g) }),
    ("vlstat", |g| {
        let a = vf(g);
        let m = if g.one_in(3) {
            "null".to_owned()
        } else {
            let e = ext(g, 2, 4);
            bin_w(g, e, 3, 1)
        };
        format!("{a} {m}")
    }),
    ("vlcopy", |g| {
        let a = vf(g);
        let b = if g.one_in(2) { "null".to_owned() } else { vf(g) };
        format!("{} {a} {b} {} {}", g.int(0, 1), g.int(0, 2), g.int(0, 3))
    }),
    ("vladd", |g| {
        let a = vf(g);
        format!(
            "{} {a} {} {} {} {} {} {} {} {}",
            g.int(0, 3),
            g.int(0, 3),
            g.int(0, 3),
            g.int(0, 3),
            g.f(false),
            g.f(false),
            g.f(false),
            g.f(false),
            g.f(false)
        )
    }),
];

#[test]
fn part11b_moves_rotations_and_voxel_lists() {
    group("part11b", PART11B, 4);
}

/// M of a pool geometry (W P), or none, or (rarely) a volume of `ras_good_flag` 0.
fn p11c_m(g: &mut G, pool: &[(String, String)], k: usize) -> String {
    if g.one_in(4) {
        return "null".to_owned();
    }
    let w = pool_w(pool, k);
    if g.one_in(6) {
        // a pool W: the extent, type, frames, header flag, fill, three sizes, `ras_good_flag`
        let mut t: Vec<&str> = w.split(' ').collect();
        t[10] = "0";
        return format!("{} 0", t.join(" "));
    }
    format!("{w} 0")
}

/// Part 11c (§67).
const PART11C: &[(&str, Make)] = &[
    ("ltaixf", |g| {
        let (lta, pool, s, d) = p11b_lta(g, &["0", "1", "2", "1", "0", "14"], 2);
        let w = pool_w(&pool, s);
        let dst = if g.one_in(2) {
            "new".to_owned()
        } else {
            pool_w(&pool, d)
        };
        format!("{w} 0 {dst} {lta} {}", g.pick(&["0", "1", "3"]))
    }),
    ("ltav2r", |g| {
        let (lta, pool, s, d) = p11b_lta(g, &["0", "1", "2"], 3);
        let a = p11c_m(g, &pool, s);
        let b = p11c_m(g, &pool, d);
        format!("{lta} {a} {b}")
    }),
    ("ltar2v", |g| {
        let (lta, pool, s, d) = p11b_lta(g, &["0", "1", "2"], 3);
        let a = if g.one_in(8) {
            "null".to_owned()
        } else {
            format!("{} 0", pool_w(&pool, s))
        };
        let b = p11c_m(g, &pool, d);
        format!("{lta} {a} {b}")
    }),
    ("ltacmp", |g| {
        let (lta, ..) = p11b_lta(g, &["0", "1"], 2);
        let left = if g.one_in(3) { "null".to_owned() } else { pool_mat(g) };
        let right = if g.one_in(3) { "null".to_owned() } else { g.mat4() };
        match g.below(3) {
            0 => format!("{lta} {left} {right} 0"),
            1 => {
                let (other, ..) = p11b_lta(g, &["0", "1"], 2);
                format!("{lta} {left} {right} 1 {other}")
            }
            _ => format!("{lta} {left} {right} 2"),
        }
    }),
];

#[test]
fn part11c_inverse_move_conversions_and_composition() {
    group("part11c", PART11C, 6);
}

/// The bytes as `x<hex>`.
fn xh(b: &[u8]) -> String {
    let mut s = "x".to_owned();
    for x in b {
        s.push_str(&format!("{x:02x}"));
    }
    s
}

/// CRC-32 (gzip's).
fn crc32(b: &[u8]) -> u32 {
    let mut c = 0xffff_ffffu32;
    for &x in b {
        c ^= u32::from(x);
        for _ in 0..8 {
            c = if c & 1 != 0 { (c >> 1) ^ 0xedb8_8320 } else { c >> 1 };
        }
    }
    !c
}

/// A gzip stream of one stored block (the same bytes on every platform).
fn gzip_stored(b: &[u8]) -> Vec<u8> {
    let mut o = vec![0x1f, 0x8b, 8, 0, 0, 0, 0, 0, 0, 3, 1];
    let n = b.len() as u16;
    o.extend_from_slice(&n.to_le_bytes());
    o.extend_from_slice(&(!n).to_le_bytes());
    o.extend_from_slice(b);
    o.extend_from_slice(&crc32(b).to_le_bytes());
    o.extend_from_slice(&(b.len() as u32).to_le_bytes());
    o
}

/// An MGH header (big-endian): version 1, the extent, frames 1, type 0, dof 0, the RAS flag and,
/// with it, sizes, cosines and centre; then `pad` zero bytes of voxels.
fn mgh_header(g: &mut G, ras: bool, pad: usize) -> Vec<u8> {
    let mut o = Vec::new();
    for v in [1, g.int(1, 4), g.int(1, 4), g.int(1, 3), 1, 0, 0] {
        o.extend_from_slice(&(v as i32).to_be_bytes());
    }
    o.extend_from_slice(&i16::from(ras).to_be_bytes());
    if ras {
        let mut f: Vec<u32> = (0..3).map(|_| u32::from_str_radix(&g.size(false), 16).unwrap_or(0)).collect();
        f.extend(g.cosines(false).iter().map(|s| u32::from_str_radix(s, 16).unwrap_or(0)));
        f.extend((0..3).map(|_| g.fbits(false)));
        for x in f {
            o.extend_from_slice(&x.to_be_bytes());
        }
    }
    o.extend(std::iter::repeat_n(0u8, pad));
    o
}

/// The writers (`ltawrmem`: §62 in memory) and `DevolveXFMWithSubjectsDir` (`devxfmmem`: §63 on
/// a subjects' folder in memory), with the environment's prefixes.
#[test]
fn part11b_writers() {
    let seed = 0x5eed_11b0u64;
    let env = Environment::new();
    for (mode, tag) in [
        (UpstreamDefects::Fix, "fix"),
        (UpstreamDefects::Reproduce, "reproduce"),
    ] {
        let mut g = G(seed);
        let mut all = Vec::new();
        let mut lines = Vec::new();
        for _ in 0..24 {
            let (lta, ..) = p11b_lta(&mut g, &["0", "1", "2", "12", "14", "15", "3"], 2);
            let name = g.pick(&[
                "t.lta", "t.LTA", "t.xfm", "t.XFM", "t.dat", "t.reg", "t.fslmat", "t.txt", "t",
                "no/t.lta", "no/t.xfm", "",
            ]);
            let mut pre = vec![format!(
                "+env=FREESURFER_REPLACEMENT_FOR_CREATION_TIME_STRING={}",
                g.pick(&["T0", ""])
            )];
            match g.below(4) {
                0 => pre.push("+env=USER=u1".to_owned()),
                1 => pre.push("+env=USER=".to_owned()),
                2 => pre.push("+env=LOGNAME=l2".to_owned()),
                _ => {}
            }
            if g.one_in(2) {
                pre.push("+env=FSLOUTPUTTYPE=NIFTI_GZ".to_owned());
            }
            lines.push(format!("{} ltawrmem {} {lta} {}", pre.join(" "), g.int(0, 1), xh(name.as_bytes())));
        }
        for _ in 0..16 {
            let mk = pi(&mut g, &[0, 1, 2, 2, 2]);
            let subj = g.pick(&["bert", "s1"]);
            let sd = g.pick(&["null", "subjects", "subjects", "nowhere"]);
            let sdx = if sd == "null" {
                "null".to_owned()
            } else {
                xh(sd.as_bytes())
            };
            let pre = if sd == "null" && g.one_in(2) {
                "+env=SUBJECTS_DIR=subjects "
            } else {
                ""
            };
            let diag = if g.one_in(3) { "+gdiag=10000040 " } else { "" };
            let ras = !g.one_in(4);
            let ok = pi(&mut g, &[0, 1, 1, 2, 3]);
            let orig = match ok {
                1 => {
                    let h = mgh_header(&mut g, ras, 4);
                    let b = if g.one_in(6) { b"not gzip".to_vec() } else { gzip_stored(&h) };
                    format!("1 {}", xh(&b))
                }
                2 => format!("2 {}", xh(&mgh_header(&mut g, ras, 0))),
                k => k.to_string(),
            };
            let (name, text) = match g.below(4) {
                0 => (
                    "null",
                    "MNI Transform File\n\nTransform_Type = Linear;\nLinear_Transform =\n 1.1 0.02 0 -2.5\n -0.01 0.95 0.1 14\n 0 -0.05 1.05 -7.25;\n",
                ),
                1 => ("x7265672e6c7461", "not a transform\n"),
                2 => (
                    "null",
                    "MNI Transform File\n\nTransform_Type = Linear;\nLinear_Transform =\n 1 0 0 2\n 0 1 0 -3\n 0 0 1 4;\n",
                ),
                _ => ("null", ""),
            };
            let xb = if text.is_empty() {
                "nofile".to_owned()
            } else {
                xh(text.as_bytes())
            };
            let m = if g.one_in(4) { pool_mat(&mut g) } else { "null".to_owned() };
            lines.push(format!(
                "{pre}{diag}devxfmmem {mk} {} {sdx} {name} {orig} {xb} {m}",
                xh(subj.as_bytes())
            ));
        }
        for line in &lines {
            let toks: Vec<String> = line.split_whitespace().map(str::to_owned).collect();
            let res = volume_cmd::run_line(&toks, mode, &env);
            all.extend_from_slice(line.as_bytes());
            all.push(b'\n');
            all.extend_from_slice(res.as_bytes());
            all.push(b'\n');
        }
        check(&format!("part11b-writers.{tag}"), &all);
    }
}
