// SPDX-License-Identifier: Apache-2.0
//! Properties every interpolator must have; parity with ITK is checked by the validation suite.

use super::*;

const SIZE: [usize; 3] = [7, 6, 5];

fn ramp() -> Vec<f64> {
    let mut v = Vec::new();
    for k in 0..SIZE[2] {
        for j in 0..SIZE[1] {
            for i in 0..SIZE[0] {
                v.push(1.0 + 2.0 * i as f64 - 0.5 * j as f64 + 3.0 * k as f64);
            }
        }
    }
    v
}

fn labels() -> Vec<f64> {
    let mut v = Vec::new();
    for k in 0..SIZE[2] {
        for j in 0..SIZE[1] {
            for i in 0..SIZE[0] {
                v.push(
                    if i < 3 {
                        2.0
                    } else if j < 2 {
                        7.0
                    } else {
                        0.0
                    } + (k / 3) as f64 * 10.0,
                );
            }
        }
    }
    v
}

fn all_methods() -> Vec<Interpolation> {
    let mut m = vec![Interpolation::Linear, Interpolation::NearestNeighbor];
    m.extend((0..=5).map(|order| Interpolation::BSpline { order }));
    m.push(Interpolation::Gaussian {
        sigma: [1.0; 3],
        alpha: 1.0,
    });
    m.extend(
        [
            Window::Cosine,
            Window::Hamming,
            Window::Welch,
            Window::Lanczos,
            Window::Blackman,
        ]
        .map(Interpolation::WindowedSinc),
    );
    m
}

#[test]
fn exact_at_voxel_centres() {
    let data = ramp();
    let vol = Volume::new(&data, SIZE);
    for method in all_methods() {
        let interp = Interpolator::new(vol, &method, [1.0; 3]).unwrap();
        for idx in [[0, 0, 0], [3, 2, 1], [6, 5, 4], [1, 4, 2]] {
            let expected = vol.at(idx[0], idx[1], idx[2]);
            let got = interp.evaluate(idx.map(|i| i as f64));
            let tol = match method {
                Interpolation::Gaussian { .. } => f64::INFINITY, // smooths by design
                Interpolation::BSpline { order } if order > 1 => 1e-9,
                _ => 0.0,
            };
            assert!(
                (got - expected).abs() <= tol,
                "{method:?} at {idx:?}: {got} vs {expected}"
            );
        }
    }
}

#[test]
fn linear_matches_trilinear_inside_and_clamps_at_edges() {
    let data = ramp();
    let interp =
        Interpolator::new(Volume::new(&data, SIZE), &Interpolation::Linear, [1.0; 3]).unwrap();
    // The ramp is linear, so trilinear interpolation is exact inside the grid.
    let at = |i: f64, j: f64, k: f64| 1.0 + 2.0 * i - 0.5 * j + 3.0 * k;
    assert_eq!(interp.evaluate([2.25, 1.5, 3.75]), at(2.25, 1.5, 3.75));
    // Within half a voxel outside: the edge value along that axis (no extrapolation).
    assert_eq!(interp.evaluate([-0.4, 1.0, 1.0]), at(0.0, 1.0, 1.0));
    assert_eq!(interp.evaluate([6.4, 1.0, 1.0]), at(6.0, 1.0, 1.0));
}

#[test]
fn nearest_rounds_half_up() {
    let data = ramp();
    let interp = Interpolator::new(
        Volume::new(&data, SIZE),
        &Interpolation::NearestNeighbor,
        [1.0; 3],
    )
    .unwrap();
    assert_eq!(
        interp.evaluate([1.5, 0.0, 0.0]),
        interp.evaluate([2.0, 0.0, 0.0])
    );
    assert_eq!(
        interp.evaluate([1.4999, 0.0, 0.0]),
        interp.evaluate([1.0, 0.0, 0.0])
    );
}

#[test]
fn constant_images_stay_constant() {
    let data = vec![3.5f32; SIZE.iter().product()];
    let vol = Volume::new(&data, SIZE);
    for method in all_methods().into_iter().chain([
        Interpolation::MultiLabel {
            sigma: [1.0; 3],
            alpha: 4.0,
        },
        Interpolation::GenericLabel,
    ]) {
        let interp = Interpolator::new(vol, &method, [1.0; 3]).unwrap();
        let got = interp.evaluate([3.3, 2.7, 2.1]);
        let tol = if matches!(method, Interpolation::WindowedSinc(_)) {
            0.05
        } else {
            1e-9
        };
        assert!((got - 3.5).abs() <= tol, "{method:?}: {got}");
    }
}

#[test]
fn label_interpolators_return_existing_labels() {
    let data = labels();
    let vol = Volume::new(&data, SIZE);
    let present: Vec<f64> = vec![0.0, 2.0, 7.0, 10.0, 12.0, 17.0];
    for method in [
        Interpolation::MultiLabel {
            sigma: [1.0; 3],
            alpha: 4.0,
        },
        Interpolation::GenericLabel,
    ] {
        let interp = Interpolator::new(vol, &method, [1.0; 3]).unwrap();
        for c in [
            [0.2, 0.1, 0.0],
            [2.6, 1.4, 2.5],
            [5.9, 4.9, 3.9],
            [3.0, 2.0, 1.0],
        ] {
            let label = interp.evaluate(c);
            assert!(present.contains(&label), "{method:?} at {c:?}: {label}");
        }
        // Deep inside a region: that region's label.
        assert_eq!(interp.evaluate([0.0, 4.0, 0.0]), 2.0);
    }
}

#[test]
fn bspline_orders_are_validated() {
    let data = ramp();
    assert!(matches!(
        Interpolator::new(
            Volume::new(&data, SIZE),
            &Interpolation::BSpline { order: 6 },
            [1.0; 3]
        ),
        Err(InterpError::BSplineOrder(6))
    ));
    assert!(is_inside([-0.5, 0.0, 4.49], SIZE) && !is_inside([0.0, 5.5, 0.0], SIZE));
}
