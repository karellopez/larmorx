// SPDX-License-Identifier: Apache-2.0
//! Tests of the RANLUX389 clean room: the spec's reference numbers (`specs/ranlux.md` §2.5, §8,
//! §10.2), which come from black-box runs of FreeSurfer's own functions, and golden checksums of
//! the clean room's own outputs, one per path (§10.6), checked on every platform against the
//! values recorded on Linux x86-64.

use super::sha256::{Sha256, hex};
use super::*;

const FIX: UpstreamDefects = UpstreamDefects::Fix;
const REPRODUCE: UpstreamDefects = UpstreamDefects::Reproduce;
const ZERO_SEED: u64 = 2_147_483_563;

/// The first `count` values of `seed`'s stream, one decimal line each.
fn raw_text(seed: u64, mode: UpstreamDefects, count: usize) -> String {
    let mut r = Ranlux::new(seed, mode);
    (0..count).map(|_| format!("{}\n", r.next_u24())).collect()
}

/// `count` draws of `f`, each as the 16 hex digits of its bits on a line (the probe's form).
fn draws(
    seed: u64,
    mode: UpstreamDefects,
    count: usize,
    mut f: impl FnMut(&mut Ranlux) -> Result<f64, DrawError>,
) -> String {
    let mut r = Ranlux::new(seed, mode);
    let mut s = String::new();
    for _ in 0..count {
        let v = f(&mut r).expect("a draw");
        s.push_str(&format!("{:016x}\n", v.to_bits()));
    }
    s
}

fn bits(v: Result<f64, DrawError>) -> u64 {
    v.expect("a draw").to_bits()
}

/// §2.5's table: the first 12 values, values 25 to 28 and the SHA-256 of the first 100,000.
#[test]
fn the_specs_streams() {
    #[rustfmt::skip]
    let table: [(u64, [u32; 12], [u32; 4], &str); 11] = [
        (53, [2221969, 1586980, 7231467, 13008083, 14055568, 7275409, 14826403, 7390714, 3054659, 16338312, 6216067, 6833642],
         [4618358, 3215875, 9571569, 7955534], "b40812f1162500bca3938e3148b47358a9300d4c7be994cd7e03163e9115541d"),
        (0, [9056646, 12776696, 1011656, 13354708, 5139066, 1388751, 11163902, 7730127, 15531355, 10387694, 5359178, 2422458],
         [14182553, 12156545, 13313883, 7141125], "12d4f2cdf218304041625d02416dc679eba7d14bf3ca30ff282fadce55a1e3b7"),
        (1, [15869483, 7943651, 15963989, 7209501, 1531321, 5202087, 15157652, 11535256, 13036256, 7905523, 3599347, 5193754],
         [13934952, 7006785, 8399129, 15847573], "391c821d14bb056db5e2302e22202721e758206eb7ebe90f8a74ca38c0c20ead"),
        (2, [14961750, 15887388, 15150761, 14419088, 3062727, 10404174, 13538087, 6293382, 9295298, 15811048, 7198695, 10387509],
         [5741855, 14824996, 1184122, 14233928], "87c0b8ce736a3e5633677ec80d807728a0fa62f9fed7d39bc7f2378be831979d"),
        (1234, [3935795, 4662948, 3129627, 4675023, 10704432, 10483180, 14725473, 7470299, 14142625, 7824199, 12409813, 199111],
         [5682984, 9444026, 15876047, 5571961], "edb3efc9bd60f606ead133110dc82c15ead6deb1360cb9c113b9c5fa6fd08338"),
        (2147483562, [907733, 8833564, 813226, 9567714, 15245894, 11575128, 1619563, 5241959, 3740959, 8871692, 13177953, 11583545],
         [64362, 15992654, 11533756, 15540238], "fdde7d836524bd89f00fde457d8039ac5012b5ebbba15e295ababb6db9e2a2d5"),
        (4294967295, [14365963, 312317, 13562131, 10456400, 7149033, 6738065, 11506525, 3307156, 5308652, 10629412, 4310035, 5329702],
         [7558662, 9467569, 246765, 9972969], "cd8cf49f25873643013cc8df819c2571be182e31330a7e99c6c41c5ec6f3ece5"),
        (4294967296, [13458230, 8256054, 12748903, 888772, 8680440, 11940152, 9886962, 14842498, 1567607, 1757720, 7909384, 10523457],
         [11279554, 1176059, 6758885, 39167], "22483b9dba48de1a1fa4d18e2325de2ec38abc68e0f1551a53d915c6ffa99f46"),
        ((1 << 63) - 1, [3870255, 6549656, 8109563, 12752488, 14566788, 7591189, 14597542, 13769507, 11190627, 7623861, 14244068, 696095],
         [355042, 3265491, 7406510, 14546069], "6a65e3a99aed14c45fd3ff53bf5c59c3f0b8609eb81f0408447c659723ce3621"),
        (1 << 63, [13814694, 2283907, 9480795, 13592356, 679021, 3983938, 3799322, 8249581, 9327546, 1247914, 15711101, 10887533],
         [14990511, 5995780, 170277, 3254111], "17809d02bbe72bcdf755d5315c47475de9cfb6c14080ab911cbbd9c7b64e8749"),
        (53u64.wrapping_neg(), [14555247, 15190235, 9545748, 3769132, 2721647, 9501806, 1950812, 9386501, 13722556, 438903, 10561233, 9943657],
         [9380956, 3006348, 10361317, 6655061], "bae173cae0f216f7e3af02218d6c93aa33f7078757e2646ba1fb99101af16f67"),
    ];
    for (seed, first, group2, sha) in table {
        let mut r = Ranlux::new(seed, FIX);
        let v: Vec<u32> = (0..28).map(|_| r.next_u24()).collect();
        assert_eq!(v[..12], first, "seed {seed}");
        assert_eq!(v[24..28], group2, "seed {seed}");
        assert_eq!(hex(raw_text(seed, FIX, 100_000).as_bytes()), sha, "seed {seed}");
        assert!(r.notices().is_empty());
    }
}

/// §2.2: seeds that give the same stream.
#[test]
fn seeds_that_alias() {
    for (a, b) in [
        (0, DEFAULT_SEED),
        (1, 2_147_483_564),
        (2_147_483_562, u64::MAX),
    ] {
        assert_eq!(raw_text(a, FIX, 1000), raw_text(b, FIX, 1000), "{a} {b}");
    }
    assert_ne!(raw_text(1, FIX, 1000), raw_text(2, FIX, 1000));
}

/// Seeding again after draws gives the fresh seeding's stream; `fill_u24` is `next_u24`.
#[test]
fn reseeding_and_filling() {
    let mut r = Ranlux::new(7, FIX);
    for _ in 0..1000 {
        r.next_u24();
    }
    let _ = r.gamma(31.6, 1.0);
    r.reseed(53);
    let mut fresh = Ranlux::new(53, FIX);
    for _ in 0..2000 {
        assert_eq!(r.next_u24(), fresh.next_u24());
    }
    assert_eq!(r.seed(), 53);
    let mut a = Ranlux::new(99, FIX);
    let mut b = a.clone();
    let mut buf = vec![0u32; 1000];
    let mut at = 0;
    for chunk in [1, 23, 24, 25, 7, 100, 389, 431] {
        a.fill_u24(&mut buf[at..at + chunk]);
        at += chunk;
    }
    let one: Vec<u32> = (0..at).map(|_| b.next_u24()).collect();
    assert_eq!(buf[..at], one[..]);
    assert!(buf[..at].iter().all(|&v| v < 1 << 24));
}

/// §10.2's first draws and its SHA-256 of 20,000 draws (seed 53), for the draws that take no
/// tangent: FreeSurfer's bits.
#[test]
fn the_specs_draws() {
    let first = |f: fn(&mut Ranlux) -> Result<f64, DrawError>| draws(53, FIX, 3, f);
    assert_eq!(
        first(|r| Ok(r.uniform())),
        "3fc0f3c880000000\n3fb8372400000000\n3fdb95fac0000000\n"
    );
    assert_eq!(
        first(Ranlux::ugaussian),
        "3ff75b40639cd543\nbfce260050c592c5\nbfc3b77b9f954954\n"
    );
    assert_eq!(
        first(|r| r.gamma(3.0, 2.0)),
        "4024e2b5d995c6dd\n400445885bf0942c\n40152c924ed15944\n"
    );
    type Draw = fn(&mut Ranlux) -> Result<f64, DrawError>;
    let cases: [(&str, Draw, &str); 12] = [
        ("uni", |r| Ok(r.uniform()), "0588698480ad8ec24542f5fe26022bf9111f8b62aacac35c9dba4aa63427f583"),
        ("flat", |r| Ok(r.flat(-3.5, 7.25)), "269e29edca7cc8250c431f4881377e474b907b096a7f4c42a8a2de89fe9babad"),
        ("gaussian", |r| r.gaussian(2.5), "dddaf9501cc31f055ae579170575a39dee41feaf306e30c3c7d3529db56dec72"),
        ("ugauss", Ranlux::ugaussian, "d361b5e25ba5462d3a2c778159eae840620ecbfc7dbab5c8d4c45c95034afd1b"),
        ("exponential", |r| r.exponential(0.75), "169ea159bfca04c0d46d9778918769faa122a691af010ab112173b1e85d2a7c5"),
        ("gamma 0.4", |r| r.gamma(0.4, 1.5), "25e6ffbbfd151efb44bb0cb517567551bf6992802bb7c8ecb7f588de6a22fecb"),
        ("gamma 3", |r| r.gamma(3.0, 2.0), "cb0281404d16edaf9bbda1ca583db153b65c86cbfdf1e71a11a809ff57be5aba"),
        ("gamma 7.3", |r| r.gamma(7.3, 0.5), "cf37d457abf62138bf99e2238ce112228df0563709a56fce5481d9a4d781d58f"),
        ("chisq", |r| r.chisq(5.0), "0d3b457e1a84203627d9def14eb2df8fd053ddc524e1dd860f15d0c139c4eb1e"),
        ("fdist", |r| r.fdist(4.0, 9.0), "8f2606831e0f4ee964814a5638e17f4f005b360c7b90951daeae26b77d038309"),
        ("tdist 1.5", |r| r.tdist(1.5), "5a3f4a1edc6c8ff7fb422cbc0e1c68fddc7ac74945f4f18846b4c619a26db2ae"),
        ("tdist 7", |r| r.tdist(7.0), "3b2a14c67236ed124516a79cf768df21d7d89b19b366a5d5a9505bed817db095"),
    ];
    for (name, f, sha) in cases {
        assert_eq!(hex(draws(53, FIX, 20_000, f).as_bytes()), sha, "{name}");
    }
}

/// §7: NaNs, infinities and zeros, as the probe gave them (seed 53).
#[test]
fn special_values() {
    let nan = |payload: u64| f64::from_bits(0x7ff8_0000_0000_0000 | payload);
    let mut r = Ranlux::new(53, FIX);
    assert_eq!(r.flat(nan(0), 1.0).to_bits(), 0x7ff8_0000_0000_0000);
    assert_eq!(r.flat(-nan(0), 1.0).to_bits(), 0xfff8_0000_0000_0000);
    assert_eq!(r.flat(nan(0x123), 1.0).to_bits(), 0x7ff8_0000_0000_0123);
    assert_eq!(r.flat(1.0, nan(0x123)).to_bits(), 0x7ff8_0000_0000_0123);
    assert_eq!(r.flat(nan(5), nan(0x123)).to_bits(), 0x7ff8_0000_0000_0123);
    assert_eq!(r.flat(f64::INFINITY, 1.0).to_bits(), f64::INFINITY.to_bits());
    assert_eq!(bits(r.gaussian(nan(0x77))), 0x7ff8_0000_0000_0077);
    assert_eq!(bits(r.exponential(nan(0x77))), 0xfff8_0000_0000_0077);
    // shape 0 is the point mass at 0, with the sign the arithmetic gives
    assert_eq!(bits(r.gamma(0.0, 1.0)), 0x8000_0000_0000_0000);
    assert_eq!(bits(r.chisq(0.0)), 0x8000_0000_0000_0000);
    assert_eq!(bits(r.tdist(0.0)), 0xfff8_0000_0000_0000);
    assert_eq!(bits(r.fdist(0.0, 3.0)), 0xfff8_0000_0000_0000);
    // the flat draw over an infinite end where U = 0 gives x86-64's default NaN (seed 2147483563's
    // stream is 0 under reproduce)
    let mut z = Ranlux::new(ZERO_SEED, REPRODUCE);
    assert_eq!(z.flat(0.0, f64::INFINITY).to_bits(), 0xfff8_0000_0000_0000);
    assert_eq!(z.flat(-2.5, f64::INFINITY).to_bits(), 0xfff8_0000_0000_0000);
}

/// FS-100 (§8.1): shapes outside [0, 2³²).
#[test]
fn fs100_gamma_shapes() {
    let first = |a: f64, mode| bits(Ranlux::new(53, mode).gamma(a, 1.0));
    // reproduce: FreeSurfer's values (the spec's §8.1, observed with fs_rng_probe)
    assert_eq!(first(4_294_967_296.5, REPRODUCE), 0x400a_dd92_293d_2764);
    assert_eq!(first(4_294_967_308.25, REPRODUCE), 0x402c_bf45_c94d_9555);
    assert_eq!(first(8_589_934_600.0, REPRODUCE), 0x4020_eaa5_24ba_c70f);
    for a in [1e300, f64::NAN, f64::INFINITY] {
        assert_eq!(first(a, REPRODUCE), 0x400a_dd92_293d_2764, "{a}");
    }
    let mut r = Ranlux::new(53, REPRODUCE);
    let _ = r.gamma(f64::NAN, 1.0);
    assert_eq!(r.notices()[0].defect.id, "FS-100");
    assert_eq!(r.notices()[0].mode, REPRODUCE);
    // the default: the whole shape, G_large of its integer part plus G_frac of its fraction
    for (a, b) in [(4_294_967_296.5, 1.0), (4_294_967_308.25, 2.0), (8_589_934_600.0, 0.5)] {
        let mut r = Ranlux::new(53, FIX);
        let got = r.gamma(a, b).unwrap();
        let mut parts = Ranlux::new(53, FIX);
        let n = a.floor();
        let mut g = parts.gamma_large(n).unwrap();
        if a != n {
            g += parts.gamma_frac(a - n).unwrap();
        }
        assert_eq!(got.to_bits(), (g * b).to_bits(), "{a}");
        assert!((got / (a * b) - 1.0).abs() < 1e-3, "{a}: {got}");
        let notice = &r.notices()[0];
        assert_eq!(notice.defect.id, "FS-100");
        assert!(notice.line().contains("uses the whole shape"), "{}", notice.line());
    }
    // the errors
    for (a, mode, text) in [
        (f64::NAN, FIX, "FS-100: the gamma shape NaN is not a finite number"),
        (f64::INFINITY, FIX, "FS-100: the gamma shape inf is not a finite number"),
        (f64::NEG_INFINITY, FIX, "FS-100: the gamma shape -inf is not a finite number"),
        (f64::NEG_INFINITY, REPRODUCE, "FS-100: the gamma shape -inf is not a finite number"),
        (-1.5, FIX, "FS-100: the gamma shape -1.5 is negative"),
        (-1.5, REPRODUCE, "FS-100: the gamma shape -1.5 is negative"),
        (-1e300, FIX, "FS-100: the gamma shape -1e300 is negative"),
    ] {
        let e = Ranlux::new(53, mode).gamma(a, 1.0).unwrap_err();
        assert_eq!(e.to_string(), text);
    }
    assert!(Ranlux::new(53, FIX).chisq(-3.0).is_err());
    assert!(Ranlux::new(53, FIX).tdist(-1.0).is_err());
    // shapes in range give no notice
    let mut r = Ranlux::new(53, FIX);
    let _ = r.gamma(4_294_967_295.5, 1.0).unwrap();
    assert!(r.notices().is_empty());
}

/// FS-101 (§8.2): the seeds whose state is all zero.
#[test]
fn fs101_zero_seeds() {
    let corrected: Vec<u32> = [16_777_215; 10]
        .into_iter()
        .chain([16_777_214, 16_777_215])
        .collect();
    // m * 2147483563 for m = 1 to 8 and -1 to -4 (observed with fs_rng_probe; -5 and 9 are not)
    for m in [1i64, 2, 3, 4, 5, 6, 7, 8, -1, -2, -3, -4] {
        let seed = (m * ZERO_SEED as i64) as u64;
        let mut r = Ranlux::new(seed, FIX);
        let v: Vec<u32> = (0..12).map(|_| r.next_u24()).collect();
        assert_eq!(v, corrected, "{m}");
        assert_eq!(r.notices().len(), 1);
        assert!(r.notices()[0].line().contains(&format!("seed {seed} makes")));
        let mut z = Ranlux::new(seed, REPRODUCE);
        assert!((0..10_000).all(|_| z.next_u24() == 0), "{m}");
        assert_eq!(z.notices()[0].defect.id, "FS-101");
    }
    for m in [-5i64, 9] {
        let r = Ranlux::new((m * ZERO_SEED as i64) as u64, FIX);
        assert!(r.notices().is_empty(), "{m}");
    }
    // reproduce: FreeSurfer's values where its draws return, an error where they never do
    let mut z = Ranlux::new(ZERO_SEED, REPRODUCE);
    assert_eq!(z.uniform(), 0.0);
    assert_eq!(z.flat(3.0, 5.0), 3.0);
    assert_eq!(bits(z.gamma(0.0, 1.0)), 0x8000_0000_0000_0000);
    assert_eq!(bits(z.gamma(12.0, 1.0)), 11f64.to_bits());
    assert_eq!(bits(z.gamma(25.0, 0.5)), 12f64.to_bits());
    assert_eq!(bits(z.gamma_large(30.0)), 29f64.to_bits());
    assert_eq!(bits(z.chisq(24.0)), 22f64.to_bits());
    assert_eq!(bits(z.fdist(24.0, 24.0)), 1f64.to_bits());
    let never = |e: Result<f64, DrawError>, draw: &str| {
        assert_eq!(
            e.unwrap_err().to_string(),
            format!("FS-101: with seed 2147483563 FreeSurfer's {draw} never returns")
        );
    };
    never(z.uniform_pos(), "positive uniform draw");
    never(z.gaussian(1.0), "Gaussian draw");
    never(z.ugaussian(), "unit Gaussian draw");
    never(z.exponential(1.0), "exponential draw");
    never(z.gamma(0.4, 1.0), "gamma draw");
    never(z.gamma(3.0, 1.0), "gamma draw");
    never(z.gamma(12.5, 1.0), "gamma draw");
    never(z.gamma_int(5), "gamma draw");
    never(z.gamma_frac(0.5), "gamma draw");
    never(z.gamma_large(0.5), "gamma draw");
    never(z.chisq(5.0), "chi-squared draw");
    never(z.fdist(4.0, 9.0), "F draw");
    never(z.tdist(1.5), "t draw");
    never(z.tdist(7.0), "t draw");
    // the default draws from the corrected stream
    let mut f = Ranlux::new(ZERO_SEED, FIX);
    assert!(f.gaussian(1.0).is_ok() && f.tdist(7.0).is_ok());
    // take_notices empties the list and keeps the mode
    assert_eq!(f.take_notices().len(), 1);
    assert!(f.notices().is_empty());
    assert_eq!(f.mode(), FIX);
}

/// One golden case: its name and the text whose SHA-256 is recorded.
fn golden_cases() -> Vec<(&'static str, String)> {
    let d = |seed, mode, f: fn(&mut Ranlux) -> Result<f64, DrawError>| draws(seed, mode, 100, f);
    const NAN2: f64 = f64::from_bits(0x7ff8_0000_0000_0123);
    vec![
        ("raw/53", raw_text(53, FIX, 1000)),
        ("raw/1", raw_text(1, FIX, 1000)),
        ("raw/4294967296", raw_text(4_294_967_296, FIX, 1000)),
        ("uni", d(53, FIX, |r| Ok(r.uniform()))),
        ("uni_pos", d(53, FIX, Ranlux::uniform_pos)),
        ("flat", d(53, FIX, |r| Ok(r.flat(-3.5, 7.25)))),
        ("gaussian", d(53, FIX, |r| r.gaussian(2.5))),
        ("ugauss", d(53, FIX, Ranlux::ugaussian)),
        ("exponential", d(53, FIX, |r| r.exponential(0.75))),
        ("gamma_int/5", d(53, FIX, |r| r.gamma_int(5))),
        ("gamma_large/31.6", d(53, FIX, |r| r.gamma_large(31.6))),
        ("gamma_frac/0.4", d(53, FIX, |r| r.gamma_frac(0.4))),
        ("gamma/7.3 (int + frac)", d(53, FIX, |r| r.gamma(7.3, 0.5))),
        ("gamma/25 (large)", d(53, FIX, |r| r.gamma(25.0, 1.0))),
        ("gamma/12.5 (large + frac)", d(53, FIX, |r| r.gamma(12.5, 2.0))),
        ("chisq", d(53, FIX, |r| r.chisq(5.0))),
        ("fdist", d(53, FIX, |r| r.fdist(30.0, 50.0))),
        ("tdist/1.5 (first branch)", d(53, FIX, |r| r.tdist(1.5))),
        ("tdist/7 (second branch)", d(53, FIX, |r| r.tdist(7.0))),
        ("nan/flat", d(53, FIX, |r| Ok(r.flat(NAN2, f64::NAN)))),
        ("nan/gaussian", d(53, FIX, |r| r.gaussian(-f64::NAN))),
        ("nan/exponential", d(53, FIX, |r| r.exponential(NAN2))),
        ("nan/tdist", d(53, FIX, |r| r.tdist(NAN2))),
        ("fs100/fix", d(53, FIX, |r| r.gamma(4_294_967_308.25, 1.0))),
        ("fs100/reproduce", d(53, REPRODUCE, |r| r.gamma(4_294_967_308.25, 1.0))),
        ("fs101/fix", raw_text(ZERO_SEED, FIX, 1000)),
        ("fs101/fix gaussian", d(ZERO_SEED, FIX, Ranlux::ugaussian)),
        ("fs101/reproduce gamma", d(ZERO_SEED, REPRODUCE, |r| r.gamma(12.0, 1.0))),
    ]
}

/// The golden checksums, recorded on Linux x86-64 (`cargo test -p larmorx-core ranlux` prints a
/// changed or missing one).
const GOLDEN: &[(&str, &str)] = &[
    ("raw/53", "6e986c30a381ee90a0ed6255128612adb3a88b5208ff826e0dcbac76dec7b728"),
    ("raw/1", "ac0668ed0e64ee8ea08a4f1ce60828ae864c9178ac3438a3d6a2745134a2985a"),
    ("raw/4294967296", "62e038a5203940f786b9511a6684d12f7f748fa9fce123c207a33e77e3d95cc7"),
    ("uni", "eab69b0b4ef52f38a184fe77587d22955f51af337a0809598b2da058a55fc251"),
    ("uni_pos", "eab69b0b4ef52f38a184fe77587d22955f51af337a0809598b2da058a55fc251"),
    ("flat", "cb1e0a4107eb987866489230a5412c3347b3fe9968052396dce2292355406986"),
    ("gaussian", "2340c20e335c8df3e977f6ec877d8d1a8725833d1da647583707af33d879616f"),
    ("ugauss", "135ea8792c68135db32a9b1310fd20018392862b389c2e0e312769511d7494e7"),
    ("exponential", "5725e7ed5fdc20d88e7641e539a07608a51c774f26b4ab97bf78632074c9b98e"),
    ("gamma_int/5", "7d22f5026d65a55fcd8bedb4bb2d7308db3dc3dd7d92d34d2cfea1383645ac83"),
    ("gamma_large/31.6", "23b84b1d692a04dc67531dab75041134990c0a660dcea5e933b5446534549098"),
    ("gamma_frac/0.4", "ef4d86f9beccd5c2d9cc8c90568d3557f2c102b4e1a3473b458e5aead83c6023"),
    ("gamma/7.3 (int + frac)", "88f47d7a38081bd5df979f501722a252db2f544b5a2e8adc2a089012ef4e28e0"),
    ("gamma/25 (large)", "068630285c1d2d7c77ee1b778d641e300fa799b79ead1a7448845113c0d75bec"),
    ("gamma/12.5 (large + frac)", "a2c3f2a51b8ba30f3cb331d35deb5d6540106524816488d9f46e589a21d69fd5"),
    ("chisq", "e48944c31d09313c1fb023295f00d986c5000f4653c03a5942a03d1e76988da0"),
    ("fdist", "f38dab75083df93699a0928128e4ddc8a21d4e5dfcaa0bb98b2eb7938aec646d"),
    ("tdist/1.5 (first branch)", "469ab7161244b2699048550645ef8e8605e7abfe0c46be407412259efc2d5016"),
    ("tdist/7 (second branch)", "e72ecf7597a95d7a63f919494b70d8b92581126d300b5c00b585743a83237860"),
    ("nan/flat", "d7d2b277dc352f48f434df3cbc59247f43b3d2b55f65ce692636d24bc7d2b3a0"),
    ("nan/gaussian", "d272cffd572b7748080d9f2b9d43df2d9caa30624ae467098da7c8980c1ee4b2"),
    ("nan/exponential", "11bea01bac460b42a840269812ad107d4241a25ab222ee99182ac965b4f712c8"),
    ("nan/tdist", "819fd7bf7e19f1815b76ea0825d631bb427e22b53a5ea5a0629ab0849e402bb6"),
    ("fs100/fix", "2be644fb8fcfcd8ac25701c325db35f84b1e1f48a65c33353e3b01d9f485abb7"),
    ("fs100/reproduce", "ef664dc0bcab93e9568bfac16cef15754721459446bf9dc9dd1181a57bd08430"),
    ("fs101/fix", "6709595d3378720316ab9c22bf088fbfa0a4af467e04c4621a1dd90abf3e8b3c"),
    ("fs101/fix gaussian", "c08baaa6766813f57a582cc2b18801a15328a980e8c2d6e84ad6f158e944fb37"),
    ("fs101/reproduce gamma", "5a261f328016f2bbea3938ed94bc3a9cf2bc9d4e2840ad3ba3d692f5a24cf94a"),
];

#[test]
fn golden() {
    let mut bad = Vec::new();
    for (name, text) in golden_cases() {
        let mut h = Sha256::new();
        h.update(text.as_bytes());
        let got = h.hex();
        match GOLDEN.iter().find(|(n, _)| *n == name) {
            Some((_, want)) if *want == got => {}
            _ => bad.push(format!("    (\"{name}\", \"{got}\"),")),
        }
    }
    assert_eq!(GOLDEN.len(), golden_cases().len(), "cases:\n{}", bad.join("\n"));
    assert!(bad.is_empty(), "changed:\n{}", bad.join("\n"));
}
