// SPDX-License-Identifier: Apache-2.0
//! The replica's side of `validation/probes/afni-volreg-trace/volreg_probe.c`: the same
//! commands on the same raw float32 volumes, with the replica's routines, so each can be
//! compared with AFNI's own library bit for bit (development tool). The C library functions
//! are glibc's (`reproduce`), as AFNI's build calls them.
//!
//! ```text
//! cargo run --release -p larmorx-afni-replica --example volreg_probe -- blur in.raw nx ny nz dx dy dz sx sy sz out.raw
//! cargo run --release -p larmorx-afni-replica --example volreg_probe -- fir in.raw nx ny nz dx dy dz sigma out.raw
//! cargo run --release -p larmorx-afni-replica --example volreg_probe -- rot in.raw nx ny nz dx dy dz method pad ax1 th1 ax2 th2 ax3 th3 sx sy sz out.raw
//! ```

use larmorx_afni_replica::blur::Blur;
use larmorx_afni_replica::rot3d::Rotator;
use larmorx_afni_replica::shift::Method;
use larmorx_core::defects::UpstreamDefects;

fn read(name: &str, n: usize) -> Vec<f32> {
    let b = std::fs::read(name).expect("input");
    assert_eq!(b.len(), 4 * n, "size of {name}");
    b.chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

fn write(name: &str, v: &[f32]) {
    let b: Vec<u8> = v.iter().flat_map(|x| x.to_le_bytes()).collect();
    std::fs::write(name, b).expect("output");
}

fn method(code: i32) -> Method {
    match code {
        0 => Method::Nn,
        1 => Method::Linear,
        3 => Method::Fourier,
        4 => Method::Quintic,
        5 => Method::Heptic,
        _ => Method::Cubic,
    }
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let f = |i: usize| a[i].parse::<f64>().expect("a number") as f32;
    let u = |i: usize| a[i].parse::<usize>().expect("a count");
    let mode = UpstreamDefects::Reproduce;
    match a[0].as_str() {
        "blur" if a.len() == 12 => {
            let n = [u(2), u(3), u(4)];
            let mut v = read(&a[1], n[0] * n[1] * n[2]);
            let mut b = Blur::new(0.0, false, mode);
            b.blur_volume_3d(n, [f(5), f(6), f(7)], &mut v, [f(8), f(9), f(10)]);
            write(&a[11], &v);
        }
        "fir" if a.len() == 10 => {
            let n = [u(2), u(3), u(4)];
            let mut v = read(&a[1], n[0] * n[1] * n[2]);
            let b = Blur::new(0.0, false, mode);
            b.fir_blur_volume(n, [f(5), f(6), f(7)], &mut v, f(8));
            write(&a[9], &v);
        }
        "rot" if a.len() == 20 => {
            let n = [u(2), u(3), u(4)];
            let v = read(&a[1], n[0] * n[1] * n[2]);
            let mut r = Rotator::new(method(a[8].parse().expect("a method")), u(9), mode);
            let ax = [a[10].parse().expect("an axis"), a[12].parse().expect("an axis"), a[14].parse().expect("an axis")];
            let out = r.rota3d(&v, n, [f(5), f(6), f(7)], ax, [f(11), f(13), f(15)], -1, [f(16), f(17), f(18)]);
            write(&a[19], &out);
        }
        _ => {
            eprintln!("usage: see the source");
            std::process::exit(2);
        }
    }
}
