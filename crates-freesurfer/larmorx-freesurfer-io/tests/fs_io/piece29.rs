// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Piece 29 (track A relay 36): `cnf` (`mriConformed`, `MRIfindMinSize` and `MRIfindRightSize` of
//! one geometry: extents 0, 1, 255 to 257, 300, 1,000 and 2^24 + 1, voxel sizes ordinary, near 1
//! in the fourth decimal, 0, negative, huge, subnormal, NaN (quiet, signalling) and infinite,
//! direction cosines coronal, axial, sagittal, oblique, zero or NaN, good or not, conform sizes
//! ordinary, 0, negative, NaN) and `acl` (`MRIaddCommandLine` 0 to 1,001 times on a new volume:
//! a line, its number appended; lines empty, long, holding a NUL).

use std::io::Write;

use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_io::mriio;
use larmorx_freesurfer_mri::mri::{self, MRI_UCHAR};
use larmorx_freesurfer_mri::vol_geom::VolGeom;
use larmorx_freesurfer_volume::conformed;

use super::{Gen, Out, Work, dec, enc};

pub const KINDS: [&str; 2] = ["cnf", "acl"];

fn size(g: &mut Gen) -> f32 {
    match g.below(20) {
        0 => f32::from_bits(0x7fc0_0000),
        1 => f32::from_bits(0x7fa0_0001),
        2 => f32::INFINITY,
        3 => 0.0,
        4 => -1.0,
        5 => 1e30,
        6 => f32::from_bits(1),
        7 => 1.00004,
        8 => 0.99996,
        9 => 1.00005,
        10..=13 => 1.0,
        _ => g.pick(&[0.5f32, 0.7, 0.9375, 1.2, 1.5, 2.0, 3.0, 0.25]),
    }
}

pub fn generate(g: &mut Gen, kind: &str, _portable: bool) -> Vec<String> {
    let mut t = vec![kind.to_owned()];
    if kind == "acl" {
        t.push(g.pick(&[0, 1, 2, 3, 999, 1000, 1001]).to_string());
        let line: Vec<u8> = match g.below(5) {
            0 => Vec::new(),
            1 => b"mri_convert a.mgz b.mgz\0hidden ".to_vec(),
            2 => vec![b'x'; 3000],
            _ => b"mri_convert --conform in.mgz out.mgz ".to_vec(),
        };
        t.push(enc(Some(&line)));
        return t;
    }
    for _ in 0..3 {
        t.push(
            if g.chance(2) {
                256
            } else {
                g.pick(&[0, 1, 255, 257, 300, 1000, 16_777_217, -1])
            }
            .to_string(),
        );
    }
    for _ in 0..3 {
        t.push(format!("{:08x}", size(g).to_bits()));
    }
    let cos: [f32; 9] = match g.below(7) {
        0 => [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
        1 => [0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0],
        2 => [0.6, 0.8, 0.0, -0.8, 0.6, 0.0, 0.0, 0.0, 1.0],
        3 => [0.0; 9],
        4 => [f32::NAN, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 1.0, 0.0],
        _ => [-1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 1.0, 0.0],
    };
    for c in cos {
        t.push(format!("{:08x}", c.to_bits()));
    }
    t.push(if g.chance(6) { "0" } else { "1" }.to_owned());
    let cs: f32 = match g.below(8) {
        0 => f32::NAN,
        1 => 0.0,
        2 => -1.0,
        3 => 0.5,
        4 => 0.7,
        _ => 1.0,
    };
    t.push(format!("{:08x}", cs.to_bits()));
    t
}

fn f32_of(s: &str) -> f32 {
    f32::from_bits(u32::from_str_radix(s, 16).unwrap())
}

pub fn run(
    tk: &[&str],
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    o: &mut Out,
    _w: &Work,
    extra: &mut Vec<&'static str>,
) -> Result<(), u8> {
    let fatal = |f: larmorx_freesurfer_core::log::Fatal| f.status();
    if tk[0] == "acl" {
        let n: i32 = tk[1].parse().unwrap();
        let line = dec(tk[2]).unwrap();
        let mut m = mri::mri_alloc_header(g, out, err, 1, 1, 1, MRI_UCHAR, 1).map_err(fatal)?;
        let mut r = 0i64;
        for i in 0..n {
            let mut l = line.clone();
            l.extend_from_slice(i.to_string().as_bytes());
            r += i64::from(mriio::mri_add_command_line(g, err, &mut m, &l));
        }
        o.i(r);
        let k = m.cmdlines.len();
        o.i(k as i64);
        if k > 0 && k <= mri::MAX_CMDS {
            for l in [&m.cmdlines[0], &m.cmdlines[k - 1]] {
                o.0.push_str(&enc(Some(l)));
                o.0.push(' ');
            }
        }
        if g.notices.as_slice().iter().any(|x| x.defect.id == "FS-842") {
            // FreeSurfer's count is memory contents from then on
            extra.push("SKIP");
        }
        return Ok(());
    }
    let p: Vec<i32> = tk[1..4].iter().map(|s| s.parse().unwrap()).collect();
    let f: Vec<f32> = tk[4..16].iter().map(|s| f32_of(s)).collect();
    let vg = VolGeom {
        width: p[0],
        height: p[1],
        depth: p[2],
        xsize: f[0],
        ysize: f[1],
        zsize: f[2],
        x_r: f[3],
        x_a: f[4],
        x_s: f[5],
        y_r: f[6],
        y_a: f[7],
        y_s: f[8],
        z_r: f[9],
        z_a: f[10],
        z_s: f[11],
        ras_good_flag: tk[16].parse().unwrap(),
        ..VolGeom::default()
    };
    let cs = f32_of(tk[17]);
    g.errno = 0;
    o.i(i64::from(conformed::mri_conformed(g, &vg)));
    let (min, cw) = conformed::mri_find_min_size(g, &vg);
    o.x(&min.to_bits().to_be_bytes(), 4);
    o.i(i64::from(cw));
    o.i(i64::from(conformed::mri_find_right_size(g, err, &vg, cs)));
    Ok(())
}
