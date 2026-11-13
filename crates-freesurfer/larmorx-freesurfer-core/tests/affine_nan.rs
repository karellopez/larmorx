// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `AffineMV` and `AffineMM` with x86-64's NaN rules on every platform: the bits of 20,000 seeded
//! products (matrices and vectors of ordinary numbers, zeros of both signs, infinities and NaNs of
//! both signs, quiet and signalling, with payloads), hashed (FNV-1a, 64 bits) and checked against the
//! hash recorded on Linux x64 (2026-11-13, track A relay 15). Run under AArch64's NaN rules
//! (`NAN_TRAP_ARM=1` with `NAN_TRAP_DIR`, the porting guide's emulation) it must give the same hash.

use larmorx_freesurfer_core::affine::{AffineMatrix, AffineVector, affine_mm, affine_mv};

const RECORDED: u64 = 0x5297_abf0_83b3_d8b8;

struct Gen(u64);

impl Gen {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
    fn float(&mut self) -> f32 {
        const POOL: [u32; 16] = [
            0x0000_0000,
            0x8000_0000,
            0x3f80_0000,
            0xbf80_0000,
            0x7f80_0000,
            0xff80_0000,
            0x7fc0_0000,
            0xffc0_0000,
            0x7fc0_0001,
            0xffc1_2345,
            0x7f80_0003,
            0xff80_0004,
            0x7f7f_ffff,
            0xff7f_ffff,
            0x0000_0001,
            0x4020_0000,
        ];
        let r = self.next();
        if r.is_multiple_of(3) {
            f32::from_bits(POOL[(r >> 8) as usize % POOL.len()])
        } else {
            ((r >> 16) % 2001) as f32 / 8.0 - 125.0
        }
    }
}

#[test]
fn nan_bits_match_the_official_rules() {
    let mut g = Gen(15);
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut put = |f: f32| {
        for b in f.to_bits().to_le_bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    let mut nans = 0;
    for k in 0..20_000u32 {
        let mut a = AffineMatrix::default();
        let mut b = AffineMatrix::default();
        for i in 0..16 {
            a.mat[i] = g.float();
            b.mat[i] = g.float();
        }
        let mut x = AffineVector::default();
        for i in 0..3 {
            x.vec[i] = g.float();
        }
        x.vec[3] = if k.is_multiple_of(5) { g.float() } else { 1.0 };
        for v in affine_mv(&a, &x).vec {
            nans += usize::from(v.is_nan());
            put(v);
        }
        for v in affine_mm(&a, &b).mat {
            nans += usize::from(v.is_nan());
            put(v);
        }
    }
    assert!(nans > 1000, "{nans}");
    assert_eq!(h, RECORDED, "{h:#018x}");
}
