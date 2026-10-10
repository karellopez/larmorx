// SPDX-License-Identifier: Apache-2.0
//! Properties of the SciPy-compatible interpolation; bit-parity with SciPy itself is checked by
//! the `resample-series` validation suite (`validation/`), which runs SciPy in-process.

use super::*;

fn volume(shape: [usize; 3]) -> Vec<f32> {
    let mut v = Vec::new();
    for k in 0..shape[2] {
        for j in 0..shape[1] {
            for i in 0..shape[0] {
                let x = i as f32 * 1.7 - j as f32 * 0.3 + (k * k) as f32 * 0.25;
                v.push(x + ((i * 7 + j * 3 + k * 5) % 11) as f32);
            }
        }
    }
    v
}

#[test]
fn mode_names_round_trip() {
    for m in Mode::ALL {
        assert_eq!(Mode::from_name(m.name()), Some(m));
        assert_eq!(m.name().parse::<Mode>().unwrap(), m);
    }
    assert!(Mode::from_name("bogus").is_none());
}

/// Without prefiltering, orders 0 and 1 reproduce the samples at integer coordinates, and with
/// it every order does (to rounding), except `reflect`: SciPy's reflect prefilter is not an
/// exact interpolant on short axes (up to 1e-2 here with order 5, in SciPy too;
/// `docs/findings/scipy-ndimage.md`).
#[test]
fn integer_coordinates_reproduce_the_samples() {
    let shape = [6, 5, 4];
    let data = volume(shape);
    let points: Vec<[f64; 3]> = (0..shape[2])
        .flat_map(|k| {
            (0..shape[1])
                .flat_map(move |j| (0..shape[0]).map(move |i| [i as f64, j as f64, k as f64]))
        })
        .collect();
    for order in 0..=5 {
        for mode in Mode::ALL {
            if order >= 2 && matches!(mode, Mode::Reflect | Mode::GridMirror) {
                continue;
            }
            let mut out = vec![0.0f64; points.len()];
            map_coordinates(&data, shape, &points, &mut out, order, mode, 0.0, true).unwrap();
            for (o, d) in out.iter().zip(&data) {
                let tol = if order <= 1 {
                    0.0
                } else {
                    1e-9 * d.abs().max(1.0) as f64
                };
                assert!(
                    (o - f64::from(*d)).abs() <= tol,
                    "order {order} {mode}: {o} vs {d}"
                );
            }
        }
    }
}

/// The prefilter's result does not depend on whether lines are filtered in parallel.
#[test]
fn prefilter_is_thread_invariant() {
    let shape = [9, 7, 300];
    let data: Vec<f64> = volume(shape).iter().map(|&v| f64::from(v)).collect();
    for order in 2..=5 {
        for mode in Mode::ALL {
            let mut a = data.clone();
            let mut b = data.clone();
            spline_filter(&mut a, &shape, order, mode, false).unwrap();
            larmorx_core::parallel::with_threads(4, || {
                spline_filter(&mut b, &shape, order, mode, true).unwrap();
            })
            .unwrap();
            assert!(a.iter().zip(&b).all(|(x, y)| x.to_bits() == y.to_bits()));
        }
    }
}

#[test]
fn constant_mode_outside_gives_cval_and_grid_constant_blends() {
    let shape = [5, 5, 5];
    let data = vec![1.0f32; 125];
    let pts = [
        [-0.5, 2.0, 2.0],
        [4.2, 2.0, 2.0],
        [2.0, 2.0, 2.0],
        [f64::NAN, 1.0, 1.0],
    ];
    let mut out = [0.0f64; 4];
    map_coordinates(&data, shape, &pts, &mut out, 3, Mode::Constant, -7.0, true).unwrap();
    assert_eq!(out[0], -7.0);
    assert_eq!(out[1], -7.0);
    assert!((out[2] - 1.0).abs() < 1e-12);
    assert_eq!(out[3], -7.0);
    map_coordinates(
        &data,
        shape,
        &pts,
        &mut out,
        3,
        Mode::GridConstant,
        0.0,
        true,
    )
    .unwrap();
    assert!(out[0] > 0.0 && out[0] < 1.0, "{}", out[0]);
    assert!(out[3].is_nan());
}

#[test]
fn errors() {
    let data = vec![0.0f32; 8];
    let mut out = [0.0f32; 1];
    assert_eq!(
        map_coordinates(
            &data,
            [2, 2, 2],
            &[[0.0; 3]],
            &mut out,
            6,
            Mode::Mirror,
            0.0,
            true
        ),
        Err(NdimageError::Order(6))
    );
    let mut d = vec![0.0f64; 8];
    assert_eq!(
        spline_filter(&mut d, &[2, 2, 2], 1, Mode::Mirror, false),
        Err(NdimageError::Order(1))
    );
    assert!(spline_filter1d(&mut d, &[2, 2, 2], 3, 3, Mode::Mirror, false).is_err());
}

#[test]
fn c_casts_follow_x86_64() {
    assert_eq!(c_cast_i64(f64::NAN), i64::MIN);
    assert_eq!(c_cast_i64(1e300), i64::MIN);
    assert_eq!(c_cast_i64(-2.7), -2);
    assert_eq!(<i16 as Output>::from_interp(-2.5), -3);
    assert_eq!(<u8 as Output>::from_interp(300.0), 255);
    assert_eq!(<u8 as Output>::from_interp(f64::NAN), 0);
}

#[test]
fn floor_matches_the_c_library() {
    let mut s = 0x9e37_79b9_7f4a_7c15u64;
    let specials = [
        0.0,
        -0.0,
        0.5,
        -0.5,
        1.0,
        -1.0,
        -1e-300,
        1e-300,
        4503599627370495.5,
        -4503599627370495.5,
        4503599627370496.0,
        9.1e18,
        -9.3e18,
        1e300,
        f64::INFINITY,
        f64::NEG_INFINITY,
        0.49999999999999994,
        -0.49999999999999994,
    ];
    for &x in &specials {
        assert_eq!(floor(x).to_bits(), x.floor().to_bits(), "{x:e}");
    }
    assert!(floor(f64::NAN).is_nan());
    for _ in 0..1_000_000 {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        let x = f64::from_bits(s);
        let y = (s % 2_000_001) as f64 / 1000.0 - 1000.0;
        for v in [x, y, y + 0.5] {
            assert!(
                floor(v).to_bits() == v.floor().to_bits() || v.is_nan(),
                "{v:e}"
            );
        }
    }
}
