// SPDX-License-Identifier: Apache-2.0
use super::F80;
use super::cases::{self, Conv, Family, Op, Pattern, SplitMix64};
use super::hardware;

#[test]
fn constants_and_simple_values() {
    assert_eq!(F80::ONE.to_bits(), 0x3fff_8000_0000_0000_0000);
    assert_eq!(F80::from_f64(1.0).to_bits(), F80::ONE.to_bits());
    assert_eq!(F80::from_f64(-2.5).to_bits(), 0xc000_a000_0000_0000_0000);
    assert_eq!(F80::from_i64(3).to_bits(), 0x4000_c000_0000_0000_0000);
    assert_eq!(
        (F80::ONE + F80::ONE).to_bits(),
        F80::from_f64(2.0).to_bits()
    );
    assert_eq!(
        (F80::from_f64(1.0) / F80::from_f64(3.0)).to_bits(),
        0x3ffd_aaaa_aaaa_aaaa_aaab
    );
    assert_eq!(
        F80::from_f64(2.0).sqrt().to_bits(),
        0x3fff_b504_f333_f9de_6484
    );
    assert_eq!((F80::ONE + F80::EPSILON).to_f64(), 1.0);
    assert_eq!(F80::MAX * F80::from_f64(2.0), F80::INFINITY);
    assert_eq!(
        (F80::INFINITY - F80::INFINITY).to_bits(),
        F80::INDEFINITE.to_bits()
    );
    assert_eq!((F80::ZERO / F80::ZERO).to_bits(), F80::INDEFINITE.to_bits());
    assert_eq!(
        F80::from_f64(-1.0).sqrt().to_bits(),
        F80::INDEFINITE.to_bits()
    );
    assert_eq!(F80::NEG_ZERO.sqrt().to_bits(), F80::NEG_ZERO.to_bits());
    assert_eq!((F80::ONE - F80::ONE).to_bits(), F80::ZERO.to_bits());
    assert_eq!(
        (F80::NEG_ZERO + F80::NEG_ZERO).to_bits(),
        F80::NEG_ZERO.to_bits()
    );
    assert!(F80::ZERO == F80::NEG_ZERO);
    assert!(F80::INDEFINITE != F80::INDEFINITE);
    assert_eq!(F80::from_f64(-2.75).to_i32_trunc(), -2);
    assert_eq!(F80::from_f64(2.5).round_ties_even_to_i64(), 2);
    assert_eq!(F80::from_f64(3.5).round_ties_even_to_i64(), 4);
    assert_eq!(F80::from_f64(1e30).to_i32_trunc(), i32::MIN);
    assert_eq!(
        F80::from_f64(1.5e19).to_u64_trunc(),
        15_000_000_000_000_000_000
    );
    // The smallest denormal halved: a tie, rounded to even (zero).
    assert_eq!(
        (F80::MIN_POSITIVE_DENORMAL / F80::from_f64(2.0)).to_bits(),
        0
    );
    assert_eq!(
        (F80::MIN_POSITIVE_DENORMAL * F80::from_f64(1.5)).to_bits(),
        2
    );
}

#[test]
fn exact_round_trips() {
    let mut rng = SplitMix64::new(1);
    for _ in 0..20_000 {
        let d = f64::from_bits(rng.next_u64());
        if !d.is_nan() {
            assert_eq!(F80::from_f64(d).to_f64().to_bits(), d.to_bits());
        }
        let f = f32::from_bits(rng.next_u64() as u32);
        if !f.is_nan() {
            assert_eq!(F80::from_f32(f).to_f32().to_bits(), f.to_bits());
        }
        let i = rng.next_u64() as i64;
        assert_eq!(F80::from_i64(i).to_i64_trunc(), i);
        let u = rng.next_u64();
        assert_eq!(F80::from_u64(u).to_u64_trunc(), u);
    }
}

/// The hardware's results (`hardware.rs`, recorded by `validation/probes/x87-extended` on the
/// reference machine's x87) on the first 1,024 cases of every operation and family: the software
/// gives the same bits on every platform.
#[test]
fn the_hardwares_results_on_every_family() {
    let mut wrong = Vec::new();
    for (o, op) in Op::ALL.iter().enumerate() {
        for (f, fam) in Family::ALL.iter().enumerate() {
            let mut rng = SplitMix64::new(cases::seed(o, f));
            let h = cases::fnv((0..1024).map(|_| {
                let (a, b) = fam.pair(&mut rng);
                op.apply(a, b)
            }));
            if h != hardware::OP_FAMILY_HASHES[o][f] {
                wrong.push(format!("{op:?} {fam:?}"));
            }
        }
    }
    assert!(wrong.is_empty(), "differ from the x87: {wrong:?}");
}

#[test]
fn the_hardwares_results_on_every_pair_of_special_values() {
    let s = cases::specials();
    for (o, op) in Op::ALL.iter().enumerate() {
        let h = cases::fnv(
            s.iter()
                .flat_map(|&a| s.iter().map(move |&b| op.apply(a, b))),
        );
        assert_eq!(h, hardware::SPECIAL_HASHES[o], "{op:?}");
    }
}

#[test]
fn the_hardwares_conversions() {
    for (c, conv) in Conv::ALL.iter().enumerate() {
        let mut rng = SplitMix64::new(cases::from_seed(c));
        let h = cases::fnv((0..1024).map(|_| conv.apply(conv.input(&mut rng))));
        assert_eq!(h, hardware::CONV_HASHES[c], "{conv:?}");
    }
}

/// The `long double` patterns as GCC 8.4.0 compiles them at `-O3`.
#[test]
fn the_hardwares_patterns() {
    for (p, pat) in Pattern::ALL.iter().enumerate() {
        let mut rng = SplitMix64::new(cases::pattern_seed(p));
        let mut words = Vec::new();
        for _ in 0..256 {
            let x = pat.inputs(&mut rng);
            words.extend(pat.apply(&x).into_iter().map(u128::from));
        }
        assert_eq!(cases::fnv(words), hardware::PATTERN_HASHES[p], "{pat:?}");
    }
}
