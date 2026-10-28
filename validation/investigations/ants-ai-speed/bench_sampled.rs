// SPDX-License-Identifier: Apache-2.0
//! Micro-benchmark of the sampled Mattes path (`metric::mattes::Mattes::evaluate_sampled`): a
//! head of 128 x 128 x 96, every 10th voxel (antsAffineInitializer's sampling), an affine
//! transform near the identity (`BENCH_ROTATE`: turned by 0.3 and 0.2 rad, as the search's
//! start points turn it); the fastest of 150 value-only and of 40 value-and-derivative
//! evaluations, in ns a point, and a checksum of the results, so that a change that alters a
//! bit shows (8cd38f7b2502e1f2 unturned, a788368be6ac6766 turned). Copy it into
//! `crates/larmorx-registration/tests/` and run
//! `BENCH_THREADS=1 cargo test --release -p larmorx-registration --test bench_sampled -- --ignored --nocapture`
//! (kept here, not there, so that the workspace's tests do not build it).
use std::sync::Arc;
use std::time::Instant;

use larmorx_core::IndexFloor;
use larmorx_registration::method::{LevelProblem, MetricKind, gradient_image};
use larmorx_registration::metric::{ImageMetricv4, MetricImage};
use larmorx_registration::optimizer::Problem;
use larmorx_registration::transform::{Composite, Kind, LinearXfm, Xfm};
use larmorx_registration::{Image, Space};

fn head(shift: f64) -> Image<f32, 3> {
    let (nx, ny, nz) = (128usize, 128usize, 96usize);
    let space = Space::<3>::new(
        [nx, ny, nz],
        [0, 0, 0],
        [1.5; 3],
        [0.0; 3],
        [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
    )
    .unwrap();
    let mut data = Vec::with_capacity(nx * ny * nz);
    let mut seed = 11u64;
    for k in 0..nz {
        for j in 0..ny {
            for i in 0..nx {
                let x = i as f64 - 64.0 - shift;
                let y = j as f64 - 64.0;
                let z = (k as f64 - 48.0) * 1.3;
                let r = (x * x + y * y + z * z).sqrt();
                let mut v = if r < 50.0 { 400.0 + i as f64 } else { 20.0 };
                if r < 25.0 {
                    v += 300.0;
                }
                if r < 12.0 {
                    v += 150.0;
                }
                seed = seed
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                v += ((seed >> 33) as f64 / (1u64 << 31) as f64 - 0.5) * 20.0;
                data.push(v as f32);
            }
        }
    }
    Image::new(space, data)
}

#[test]
#[ignore]
fn bench_sampled_mattes() {
    let fixed = Arc::new(head(0.0));
    let moving = Arc::new(head(2.0));
    let threads: usize = std::env::var("BENCH_THREADS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1);
    let space = fixed.space.clone();
    let mut points = Vec::new();
    for o in (0..space.len()).step_by(10) {
        let idx = space.index_of(o);
        let p = space.index_to_point(&idx);
        points.push(std::array::from_fn(|k| f64::from(p[k] as f32)));
    }
    let gradient = Arc::new(gradient_image::<f32, 3>(&moving, 1).unwrap());
    let metric = ImageMetricv4 {
        virtual_space: space.clone(),
        fixed: MetricImage::Float(Arc::clone(&fixed)),
        moving: MetricImage::Float(Arc::clone(&moving)),
        fixed_mask: None,
        moving_mask: None,
        fixed_transform: None,
        samples: Some(Arc::new(points)),
        fixed_gradient_image: None,
        moving_gradient_image: Some(gradient),
        pixel_f32: true,
        local_support: false,
        outside: Default::default(),
        n_threads: Some(threads),
    };
    let mut t = LinearXfm::<f64, 3>::new(Kind::Affine);
    t.set_center([96.0, 96.0, 72.0]);
    let floor = IndexFloor::default();
    let mut problem = LevelProblem::single(
        metric,
        MetricKind::Mattes { bins: 32 },
        Composite::new(vec![Xfm::Linear(t)]),
        &floor,
    )
    .unwrap();
    let mut base = problem.parameters();
    if std::env::var_os("BENCH_ROTATE").is_some() {
        // 0.3 rad about z, then 0.2 about x (as the search's start points turn the image).
        let (c1, s1) = (0.3f64.cos(), 0.3f64.sin());
        let (c2, s2) = (0.2f64.cos(), 0.2f64.sin());
        let rz = [[c1, -s1, 0.0], [s1, c1, 0.0], [0.0, 0.0, 1.0]];
        let rx = [[1.0, 0.0, 0.0], [0.0, c2, -s2], [0.0, s2, c2]];
        for i in 0..3 {
            for j in 0..3 {
                base[i * 3 + j] = (0..3).map(|k| rz[i][k] * rx[k][j]).sum();
            }
        }
    }
    let reps = 150;
    let mut check = 0u64;
    let mut tv = f64::MAX;
    for r in 0..reps {
        let mut p = base.clone();
        p[9] = 0.01 * (r % 40) as f64;
        p[0] = base[0] + 0.001 * (r % 40) as f64;
        problem.set_parameters(&p).unwrap();
        let t0 = Instant::now();
        let v = problem.value().unwrap();
        tv = tv.min(t0.elapsed().as_secs_f64());
        if r < 40 {
            check = check.rotate_left(7) ^ v.to_bits();
        }
    }
    let mut td = f64::MAX;
    for r in 0..40 {
        let mut p = base.clone();
        p[9] = 0.01 * (r % 10) as f64;
        p[1] = 0.002 * (r % 10) as f64;
        problem.set_parameters(&p).unwrap();
        let t0 = Instant::now();
        let (v, d) = problem.value_and_derivative().unwrap();
        td = td.min(t0.elapsed().as_secs_f64());
        if r < 10 {
            check = check.rotate_left(7) ^ v.to_bits();
            for x in d {
                check = check.rotate_left(3) ^ x.to_bits();
            }
        }
    }
    let n = 128 * 128 * 96 / 10;
    eprintln!(
        "BENCH threads={threads} value {:.2} ms ({:.1} ns/pt), derivative {:.2} ms ({:.1} ns/pt), check {check:016x}",
        tv * 1e3,
        tv * 1e9 / n as f64,
        td * 1e3,
        td * 1e9 / n as f64
    );
}
