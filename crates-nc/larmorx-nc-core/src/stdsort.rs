// SPDX-License-Identifier: LicenseRef-FSL
// Copyright 2026 Karel Lopez Vilaret.
//! `std::sort` and `std::nth_element` as GCC's libstdc++ implements them, as FSL's replicas use
//! them: the generic introsort and introselect are larmorx-core's ([`larmorx_core::stdsort`],
//! re-exported here), written from the SGI STL's published description; this module keeps what
//! is FSL's. Neither sort is stable: elements that compare equal end up in the order
//! libstdc++'s sequence of comparisons and moves decides, which FSL's mesh sweep
//! (`real_self_intersection`, vertices sorted by x) depends on, and so does the order of NaN
//! values in `calc_bval` (FSL-22, [`sort_f32_checked`]).
//! The SGI STL: Copyright (c) 1994 Hewlett-Packard Company, Copyright (c) 1996 Silicon Graphics
//! Computer Systems, Inc.; their permission notice is in `crates-nc/NOTICE`.

pub use larmorx_core::stdsort::{Outside, nth_element_f64, sort_by, sort_checked};

/// `std::sort(v.begin(), v.end())` on floats with `operator<`, moving elements exactly as
/// libstdc++ does, for a vector that may hold NaN values ([`sort_checked`]): FSL-22's
/// `reproduce` (`calc_bval`'s border values) and newimage's percentiles of values that may be
/// NaN. [`Outside`] where libstdc++ would read past the vector's ends (memory C++ does not own),
/// with `v` partly sorted.
pub fn sort_f32_checked(v: &mut [f32]) -> Result<(), Outside> {
    sort_checked(v)
}

/// [`sort_f32_checked`] for doubles (`std::sort` of a `std::vector<double>`).
pub fn sort_f64_checked(v: &mut [f64]) -> Result<(), Outside> {
    sort_checked(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_sort_with_nan() {
        // Inside the vector: NaN values end where libstdc++'s moves put them, not last.
        let mut v = vec![1.0, f32::NAN, 0.5, 2.0];
        assert_eq!(sort_f32_checked(&mut v), Ok(()));
        assert_eq!(&v[..2], &[0.5, 1.0]);
        assert!(v[2].is_nan() && v[3] == 2.0);
        // A NaN first is the insertion's guard: the numbers after it are sorted, it stays.
        let mut v = vec![f32::NAN, 3.0, 2.0, 1.0];
        assert_eq!(sort_f32_checked(&mut v), Ok(()));
        assert!(v[0].is_nan() && v[1..] == [1.0, 2.0, 3.0]);
    }

    /// libstdc++'s own results (`validation/probes/libstdcxx-sort/sort_probe.cc`, the hash of
    /// larmorx-core's test of the same name): GCC 11.4's and 14.3's libstdc++ give this hash
    /// (FSL 6.0.7.17 was built with GCC 13.2), with no read outside any vector.
    #[test]
    fn checked_sort_gives_libstdcxx_order_with_nan() {
        let mut s = 0x2545f4914f6cdd1du64;
        let mut next = || {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            s
        };
        let mut h = 0xcbf29ce484222325u64;
        let mut mix = |b: u8| h = (h ^ u64::from(b)).wrapping_mul(0x100000001b3);
        let (mut cases, mut outside, mut nan_inside) = (0, 0, 0);
        for n in [
            0usize, 1, 2, 3, 5, 16, 17, 18, 31, 32, 33, 64, 100, 257, 1000, 5000,
        ] {
            for f in [0u64, 10, 100, 500, 900, 1000] {
                for m in [3u64, 1000] {
                    for _ in 0..4 {
                        let mut v: Vec<f32> = (0..n)
                            .map(|_| {
                                let r = next();
                                if r % 1000 < f {
                                    f32::from_bits(if (r >> 20) & 1 == 1 {
                                        0xffc00000
                                    } else {
                                        0x7fc00000
                                    })
                                } else {
                                    ((r >> 10) % m) as f32
                                }
                            })
                            .collect();
                        let any_nan = v.iter().any(|x| x.is_nan());
                        if sort_f32_checked(&mut v).is_err() {
                            outside += 1;
                            mix(0xee);
                        } else {
                            nan_inside += usize::from(any_nan);
                            for x in &v {
                                x.to_bits().to_le_bytes().into_iter().for_each(&mut mix);
                            }
                        }
                        mix(0x55);
                        cases += 1;
                    }
                }
            }
        }
        assert_eq!((cases, outside, nan_inside), (768, 0, 495));
        assert_eq!(h, 0x63bfd9ba2c340dd2);
    }
}
