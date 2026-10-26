// SPDX-License-Identifier: LicenseRef-FSL
// Copyright 2026 Karel Lopez Vilaret (the translation).
//! `bet2`: FSL's brain extraction (fsl-bet2 2111.8, `bet2.cpp`; S. M. Smith, "Fast robust
//! automated brain extraction", Human Brain Mapping 17(3), 2002), translated line by line.
//!
//! A tessellated sphere (an icosahedron subdivided 4 times: 2562 vertices) is placed at the
//! image's centre of gravity with half the estimated head radius, then deformed for 1000
//! iterations: each vertex moves along its neighbours' tangential mean, along its normal by a
//! smoothness term (`tanh` of the local curvature), and along its normal by an intensity term
//! that compares the minimum and maximum intensities found inward along the normal with a
//! threshold between the robust minimum and the local maximum (`-f`, `-g`). A mesh whose
//! vertices crossed is deformed again from the start with a stronger smoothness, up to 10 times.
//! The brain is the inside of the final mesh (a flood fill from the mesh's centroid).

use std::io::Write;

use larmorx_core::defects::{self, Notices, UpstreamDefects};

use larmorx_nc_core::cnum;
use larmorx_nc_core::fdlibm;
use larmorx_nc_core::mesh::{Mesh, V3};
use larmorx_nc_core::options::{ArgFlag, Opt, OptionParser, Value};
use larmorx_nc_core::settings::Settings;
use larmorx_nc_core::sse::{self, S64};
use larmorx_nc_core::volume::{self, Elem, FslError, SaveError, SaveOptions, Saved, Volume};
use larmorx_nc_core::{arma, glibc_double};

const NORMAL_MAX_UPDATE_FRACTION: f64 = 0.5;
const LAMBDA_FIT: f64 = 0.1;
const TITLE: &str = "BET (Brain Extraction Tool) v2.1 - FMRIB Analysis Group, Oxford";
const EXAMPLES: &str = "bet2 <input_fileroot> <output_fileroot> [options]";

/// `bet_parameters`.
#[derive(Clone, Copy, Debug, Default)]
pub struct BetParameters {
    pub min: f64,
    pub max: f64,
    pub t98: f64,
    pub t2: f64,
    pub t: f64,
    pub tm: f64,
    pub radius: f64,
    pub cog: V3,
}

/// MISCMATHS' `Min` (the second operand unless the first is smaller) and `Max`.
#[inline]
fn fmin(a: f64, b: f64) -> f64 {
    if a < b { a } else { b }
}

#[inline]
fn fmax(a: f64, b: f64) -> f64 {
    if a > b { a } else { b }
}

/// C's `(int) x` for a double (x86-64: `INT_MIN` out of range).
#[inline]
fn trunc_i(x: f64) -> i64 {
    i64::from(cnum::f64_to_i32(x))
}

/// The options of `bet2`, in the order FSL adds them (the order of `-h`).
pub fn parser() -> OptionParser {
    let b = |key, help| Opt {
        key,
        default: Value::Bool(false),
        help,
        compulsory: false,
        flag: ArgFlag::NoArgument,
        visible: true,
    };
    let f = |key, default: f32, help, n, visible| Opt {
        key,
        default: Value::Float(default),
        help,
        compulsory: false,
        flag: ArgFlag::Requires(n),
        visible,
    };
    OptionParser::new(
        TITLE,
        EXAMPLES,
        vec![
            b(
                "-o,--outline",
                "generate brain surface outline overlaid onto original image",
            ),
            b("-m,--mask", "generate binary brain mask"),
            b("-s,--skull", "generate approximate skull image"),
            b(
                "-n,--nooutput",
                "don't generate segmented brain image output",
            ),
            f(
                "-f",
                0.5,
                "~<f>\t\tfractional intensity threshold (0->1); default=0.5; smaller values give larger brain outline estimates",
                1,
                true,
            ),
            f(
                "-g",
                0.0,
                "~<g>\t\tvertical gradient in fractional intensity threshold (-1->1); default=0; positive values give larger brain outline at bottom, smaller at top",
                1,
                true,
            ),
            f(
                "-r,--radius",
                0.0,
                "~<r>\thead radius (mm not voxels); initial surface sphere is set to half of this",
                1,
                true,
            ),
            f(
                "-w,--smooth",
                1.0,
                "~<r>\tsmoothness factor; default=1; values smaller than 1 produce more detailed brain surface, values larger than one produce smoother, less detailed surface",
                1,
                true,
            ),
            f(
                "-c",
                0.0,
                "~<x y z>\tcentre-of-gravity (voxels not mm) of initial mesh surface.",
                3,
                true,
            ),
            b(
                "-t,--threshold",
                "-apply thresholding to segmented brain image and mask",
            ),
            b("--ct", "-treat input as CT image (default is MRI)"),
            b("-e,--mesh", "generates brain surface as mesh in vtk format"),
            b("-v,--verbose", "switch on diagnostic messages"),
            b("-h,--help", "displays this help, then exits"),
            f(
                "--parameter_d1",
                7.0,
                "~<d1>\tintensity search depth d1 ( default 7 ).",
                1,
                false,
            ),
            f(
                "--parameter_d2",
                3.0,
                "~<d2>\tintensity search depth d2 ( default 3 ).",
                1,
                false,
            ),
        ],
    )
}

/// What `bet2` was asked to do.
#[derive(Clone, Debug)]
pub struct Bet2Args {
    pub outline: bool,
    pub mask: bool,
    pub skull: bool,
    pub no_output: bool,
    pub frac: f32,
    pub grad: f32,
    pub radius: f32,
    pub smooth: f32,
    pub centre: Option<[f32; 3]>,
    pub threshold: bool,
    pub ct: bool,
    pub mesh: bool,
    pub verbose: bool,
    pub d1: f32,
    pub d2: f32,
}

impl Default for Bet2Args {
    fn default() -> Self {
        Bet2Args {
            outline: false,
            mask: false,
            skull: false,
            no_output: false,
            frac: 0.5,
            grad: 0.0,
            radius: 0.0,
            smooth: 1.0,
            centre: None,
            threshold: false,
            ct: false,
            mesh: false,
            verbose: false,
            d1: 7.0,
            d2: 3.0,
        }
    }
}

/// The surface and masks bet2 computes.
pub struct Bet2Result {
    pub params: BetParameters,
    pub mesh: Mesh,
    /// 0 inside the brain (and on the surface), 1 outside: bet2's `brainmask`.
    pub outside: Volume<i16>,
    pub passes: i32,
}

/// `adjust_initial_mesh`: the thresholds, the radius, the centre of gravity, the median
/// intensity inside the initial sphere; the mesh moved and scaled there. When no voxel lies
/// strictly between the robust limits inside the sphere (FSL-41: FSL dereferences the end of an
/// empty list and crashes), `Err` unless `fill_undefined`, which gives that median the value 0
/// and says so (`Ok((_, true))`): everything else is FSL's.
#[allow(clippy::too_many_arguments)]
pub fn adjust_initial_mesh(
    image: &Volume<f32>,
    m: &mut Mesh,
    rad: f64,
    xpara: f64,
    ypara: f64,
    zpara: f64,
    warn: &mut String,
    fill_undefined: bool,
) -> Result<(BetParameters, bool), usize> {
    let mut bp = BetParameters::default();
    let (mx, mn) = image.extrema();
    bp.min = f64::from(mn);
    bp.max = f64::from(mx);
    let (t2f, t98f) = image.robust_limits();
    let (t2, t98) = (f64::from(t2f), f64::from(t98f));
    // NaN or infinite voxels reach these sums: SSE's NaNs (larmorx_nc_core::sse)
    let t = (S64(t2) + 0.1 * (S64(t98) - t2)).0;
    bp.t98 = t98;
    bp.t2 = t2;
    bp.t = t;
    let (nx, ny, nz) = (image.xsize(), image.ysize(), image.zsize());
    let (xd, yd, zd) = (
        f64::from(image.xdim()),
        f64::from(image.ydim()),
        f64::from(image.zdim()),
    );
    if rad == 0.0 {
        let mut counter: i64 = 0;
        for k in 0..nz {
            for j in 0..ny {
                for i in 0..nx {
                    let c = f64::from(image.value(i, j, k));
                    if c > t {
                        counter += 1;
                    }
                }
            }
        }
        let mut radius = glibc_double::pow(
            0.75 * counter as f64 * xd * yd * zd / std::f64::consts::PI,
            1.0 / 3.0,
        );
        let mut dimensionality: i16 = 3;
        // image.xsize() * image.xdim(): int64 * float, in float
        // counter * dim * dim: int64 * float * float, in float; then / M_PI / size in double
        let flat = |a: f32, b: f32, size: i64| {
            f64::from(counter as f32 * a * b) / std::f64::consts::PI / size as f64
        };
        if f64::from(nx as f32 * image.xdim()) < radius {
            radius = glibc_double::pow(flat(image.ydim(), image.zdim(), nx), 1.0 / 2.0);
            dimensionality -= 1;
        }
        if f64::from(ny as f32 * image.ydim()) < radius {
            radius = glibc_double::pow(flat(image.xdim(), image.zdim(), ny), 1.0 / 2.0);
            dimensionality -= 1;
        }
        if f64::from(nz as f32 * image.zdim()) < radius {
            radius = glibc_double::pow(flat(image.xdim(), image.ydim(), nz), 1.0 / 2.0);
            dimensionality -= 1;
        }
        if dimensionality < 2 {
            warn.push_str("Warning: Image appears to be 1D\n");
        }
        bp.radius = radius;
    } else {
        bp.radius = rad;
    }
    let center = if xpara == 0.0 && ypara == 0.0 && zpara == 0.0 {
        // `double counter` (kept double "to preserve old results")
        let mut counter = S64(0.0);
        let tmp = (S64(t) - t2).0;
        let mut c3 = V3::default();
        for k in 0..nz {
            for j in 0..ny {
                for i in 0..nx {
                    let mut c = (S64(f64::from(image.value(i, j, k))) - t2).0;
                    if c > tmp {
                        c = fmin(c, (S64(t98) - t2).0);
                        counter += c;
                        let p = V3::new(
                            (S64(c) * i as f64 * xd).0,
                            (S64(c) * j as f64 * yd).0,
                            (S64(c) * k as f64 * zd).0,
                        );
                        c3 = c3.add(p);
                    }
                }
            }
        }
        V3::new(
            sse::div_f64(c3.x, counter.0),
            sse::div_f64(c3.y, counter.0),
            sse::div_f64(c3.z, counter.0),
        )
    } else {
        V3::new(
            sse::mul_f64(xpara, xd),
            sse::mul_f64(ypara, yd),
            sse::mul_f64(zpara, zd),
        )
    };
    bp.cog = center;
    m.translation(center.x, center.y, center.z);
    m.rescale(bp.radius / 2.0, center);

    // tm: the median of the intensities strictly between t2 and t98 inside the sphere
    let mut vm: Vec<f64> = Vec::new();
    let r2 = bp.radius * bp.radius;
    for k in 0..nz {
        for j in 0..ny {
            for i in 0..nx {
                let d = f64::from(image.value(i, j, k));
                // i * xdim: int * float, in float
                let p = V3::new(
                    f64::from(i as f32 * image.xdim()),
                    f64::from(j as f32 * image.ydim()),
                    f64::from(k as f32 * image.zdim()),
                );
                let dp = p.sub(center);
                if d > t2 && d < t98 && dp.dot(dp) < r2 {
                    vm.push(d);
                }
            }
        }
    }
    let med = vm.len() / 2;
    if vm.is_empty() {
        if !fill_undefined {
            return Err(0);
        }
        // FSL reads `*max_element(begin, begin)` of an empty vector: no value; 0 here
        bp.tm = 0.0;
        return Ok((bp, true));
    }
    // nth_element(med - 1) then the largest of the first med values: the (med-1)-th smallest
    // value; with one value (med 0), libstdc++ returns it.
    let tm = if med == 0 {
        vm[0]
    } else {
        let (_, nth, _) = vm.select_nth_unstable_by(med - 1, |a, b| a.total_cmp(b));
        *nth
    };
    bp.tm = tm;
    Ok((bp, false))
}

/// `step_of_computation`.
#[allow(clippy::too_many_arguments)]
pub fn step_of_computation(
    image: &Volume<f32>,
    m: &mut Mesh,
    bet_main_parameter: f64,
    pass: i32,
    increase_smoothing: f64,
    iteration_number: i32,
    l: &mut f64,
    t2: f64,
    tm: f64,
    t: f64,
    e: f64,
    f: f64,
    zcog: f64,
    radius: f64,
    local_th: f64,
    d1: i32,
    d2: i32,
) {
    let (nx, ny, nz) = (image.xsize(), image.ysize(), image.zsize());
    let xdim = if f64::from(nx as f32 * image.xdim()) > radius {
        f64::from(image.xdim())
    } else {
        2.0 * radius
    };
    let ydim = if f64::from(ny as f32 * image.ydim()) > radius {
        f64::from(image.ydim())
    } else {
        2.0 * radius
    };
    let zdim = if f64::from(nz as f32 * image.zdim()) > radius {
        f64::from(image.zdim())
    } else {
        2.0 * radius
    };
    let dscale = fmin(fmin(fmin(xdim, ydim), zdim), 1.0);

    if iteration_number == 50 || iteration_number % 100 == 0 {
        let mut sum = S64(0.0);
        let mut counter = 0i32;
        for p in 0..m.points.len() {
            counter += 1;
            sum += m.medium_distance_of_neighbours(p);
        }
        *l = (sum / f64::from(counter)).0;
    }
    // Every operation that NaN or infinite voxels reach computes with SSE's NaNs
    // (larmorx_nc_core::sse), in the source's order.
    let lv = *l;
    let fit = (S64(NORMAL_MAX_UPDATE_FRACTION) * LAMBDA_FIT * lv).0;
    let d1f = f64::from(d1);
    let d2f = f64::from(d2);
    for p in 0..m.points.len() {
        let n = m.local_normal(p);
        let dv = m.difference_vector(p);
        let tmp = dv.dot(n);
        let sn = V3::new((S64(n.x) * tmp).0, (S64(n.y) * tmp).0, (S64(n.z) * tmp).0);
        let st = dv.sub(sn);
        let u1 = V3::new(
            (S64(st.x) * 0.5).0,
            (S64(st.y) * 0.5).0,
            (S64(st.z) * 0.5).0,
        );
        let rinv = ((2.0 * S64(sn.dot(n)).abs()) / (S64(lv) * lv)).0;
        let mut f2 = ((1.0 + S64(fdlibm::tanh((S64(f) * (S64(rinv) - e)).0))) * 0.5).0;
        if pass > 0 && tmp > 0.0 {
            f2 = (S64(f2) * increase_smoothing).0;
            f2 = fmin(f2, 1.0);
        }
        let u2 = sn.scale(f2);

        let coord = m.points[p].coord;
        let mut f3 = 0.0;
        let mut local_t = bet_main_parameter;
        if local_th != 0.0 {
            local_t = fmin(
                1.0,
                fmax(
                    0.0,
                    (S64(bet_main_parameter) + S64(local_th) * (S64(coord.z) - zcog) / radius).0,
                ),
            );
        }
        let mut imin = tm;
        let mut imax = t;
        let pp = coord.add(n.scale(-1.0));
        let mut iv = (S64(pp.x) / xdim + 0.5).0;
        let mut jv = (S64(pp.y) / ydim + 0.5).0;
        let mut kv = (S64(pp.z) / zdim + 0.5).0;
        if image.in_bounds(trunc_i(iv), trunc_i(jv), trunc_i(kv)) {
            let mut im = f64::from(image.value(trunc_i(iv), trunc_i(jv), trunc_i(kv)));
            imin = fmin(imin, im);
            imax = fmax(imax, im);
            let mut nxv = sse::div_f64(n.x, xdim);
            let mut nyv = sse::div_f64(n.y, ydim);
            let mut nzv = sse::div_f64(n.z, zdim);
            let back = f64::from(d1.wrapping_sub(1));
            let i2 = trunc_i((S64(iv) - S64(back) * nxv).0);
            let j2 = trunc_i((S64(jv) - S64(back) * nyv).0);
            let k2 = trunc_i((S64(kv) - S64(back) * nzv).0);
            nxv = sse::mul_f64(nxv, dscale);
            nyv = sse::mul_f64(nyv, dscale);
            nzv = sse::mul_f64(nzv, dscale);
            if image.in_bounds(i2, j2, k2) {
                im = f64::from(image.value(i2, j2, k2));
                imin = fmin(imin, im);
                let mut gi = 2.0;
                while gi < d1f {
                    iv = sse::sub_f64(iv, nxv);
                    jv = sse::sub_f64(jv, nyv);
                    kv = sse::sub_f64(kv, nzv);
                    im = f64::from(image.value(trunc_i(iv), trunc_i(jv), trunc_i(kv)));
                    imin = fmin(imin, im);
                    if gi < d2f {
                        imax = fmax(imax, im);
                    }
                    gi += dscale;
                }
                imin = fmax(t2, imin);
                imax = fmin(tm, imax);
                let tl = ((S64(imax) - t2) * local_t + t2).0;
                f3 = if (S64(imax) - t2).0 > 0.0 {
                    (2.0 * (S64(imin) - tl) / (S64(imax) - t2)).0
                } else {
                    ((S64(imin) - tl) * 2.0).0
                };
            }
        }
        f3 = sse::mul_f64(f3, fit);
        let u3 = n.scale(f3);
        let u = u1.add(u2).add(u3);
        m.points[p].update_coord = coord.add(u);
    }
    m.update();
}

/// `draw_segment` (on a short image, writing 0).
fn draw_segment(image: &mut Volume<i16>, p1: V3, p2: V3, mininc: f64, dims: (f64, f64, f64)) {
    let mut n = p1.sub(p2);
    let d = n.norm();
    n.normalize();
    let mut i = 0.0;
    while i <= d {
        let p = p2.add(n.scale(i));
        let x = trunc_i((p.x / dims.0 + 0.5).floor());
        let y = trunc_i((p.y / dims.1 + 0.5).floor());
        let z = trunc_i((p.z / dims.2 + 0.5).floor());
        if image.in_bounds(x, y, z) {
            image.set(x, y, z, 0);
        }
        i += mininc;
    }
}

/// `draw_mesh` (bet2's: the surface drawn as 0 in a short image).
pub fn draw_mesh(image: &Volume<i16>, m: &Mesh) -> Volume<i16> {
    let dims = (
        f64::from(image.xdim()),
        f64::from(image.ydim()),
        f64::from(image.zdim()),
    );
    let mininc = fmin(dims.0, fmin(dims.1, dims.2)) * 0.5;
    let mut res = image.clone();
    for &t in &m.tri_list {
        let [a, b, c] = m.tris[t].v;
        let (pa, pb, pc) = (m.points[a].coord, m.points[b].coord, m.points[c].coord);
        let mut n = pa.sub(pb);
        let d = n.norm();
        n.normalize();
        let mut j = 0.0;
        while j <= d {
            let p = pb.add(n.scale(j));
            draw_segment(&mut res, p, pc, mininc, dims);
            j += mininc;
        }
    }
    res
}

/// `draw_mesh_bis`: the outline drawn at the image's maximum, rounding coordinates by
/// truncation of `x + 0.5`.
pub fn draw_mesh_bis(image: &Volume<f32>, m: &Mesh) -> Volume<f32> {
    let dims = (
        f64::from(image.xdim()),
        f64::from(image.ydim()),
        f64::from(image.zdim()),
    );
    let mininc = fmin(dims.0, fmin(dims.1, dims.2)) * 0.5;
    let mut res = image.clone();
    let max = f64::from(image.max()) as f32;
    for &t in &m.tri_list {
        let [a, b, c] = m.tris[t].v;
        let (pa, pb, pc) = (m.points[a].coord, m.points[b].coord, m.points[c].coord);
        let mut n = pa.sub(pb);
        let d = n.norm();
        n.normalize();
        let mut j = 0.0;
        while j <= d {
            let p1 = pb.add(n.scale(j));
            let mut n2 = p1.sub(pc);
            let d2 = n2.norm();
            n2.normalize();
            let mut i = 0.0;
            while i <= d2 {
                let p = pc.add(n2.scale(i));
                let x = trunc_i(p.x / dims.0 + 0.5);
                let y = trunc_i(p.y / dims.1 + 0.5);
                let z = trunc_i(p.z / dims.2 + 0.5);
                if res.in_bounds(x, y, z) {
                    res.set(x, y, z, max);
                }
                i += mininc;
            }
            j += mininc;
        }
    }
    res
}

/// The fill of FSL's `make_mask_from_mesh` (bet2's and betsurf's): from `start`, every voxel of
/// value `from` 6-connected to it becomes `to`, the start itself whatever its value. (FSL pushes a
/// voxel each time it is seen with `from` and sets it when popped; marking it when pushed fills the
/// same voxels with far less memory.)
///
/// A start outside the image is FSL-43: FSL writes `to` there, outside the volume, and reads its
/// neighbours outside it too. One voxel outside (a single slice: the centroid falls just below
/// it), FSL's write lands beside the volume's buffer and the run goes on: corrected (`fix`), the
/// fill starts at the nearest voxel inside the image; reproduced, only what FSL defines is done:
/// no write outside, and of the start's neighbours FSL's bounds allow, those inside the image are
/// examined (those outside count as not `from`). Further outside (a centroid that is not a number
/// truncates to `INT_MIN`) FSL writes far from the volume and crashes: `fix` fills from the
/// nearest voxel when the start is a real position, and refuses otherwise ([`Fill::Crash`]);
/// reproduced, the write FSL crashes on is not done and nothing is filled ([`Fill::Skipped`]:
/// D13's failure cases, what FSL defines kept, the rest left as it was).
pub fn fill_mask(mask: &mut Volume<i16>, start: [i64; 3], from: i16, to: i16, fix: bool) -> Fill {
    let (nx, ny, nz) = (mask.xsize(), mask.ysize(), mask.zsize());
    if nx <= 0 || ny <= 0 || nz <= 0 {
        return if fix {
            Fill::Crash(start)
        } else {
            Fill::Skipped(start)
        };
    }
    let inside = mask.in_bounds(start[0], start[1], start[2]);
    let real = start.iter().all(|&c| c != i64::from(i32::MIN));
    let near = start[0] >= -1
        && start[1] >= -1
        && start[2] >= -1
        && start[0] <= nx
        && start[1] <= ny
        && start[2] <= nz;
    if !inside && !(if fix { real } else { near }) {
        return if fix {
            Fill::Crash(start)
        } else {
            Fill::Skipped(start)
        };
    }
    let neighbours = move |[x, y, z]: [i64; 3]| {
        [
            (x > 0).then_some([x - 1, y, z]),
            (y > 0).then_some([x, y - 1, z]),
            (z > 0).then_some([x, y, z - 1]),
            (nx > x + 1).then_some([x + 1, y, z]),
            (ny > y + 1).then_some([x, y + 1, z]),
            (nz > z + 1).then_some([x, y, z + 1]),
        ]
    };
    let flood = |mask: &mut Volume<i16>, mut current: Vec<[i64; 3]>| {
        while let Some(p) = current.pop() {
            for [x, y, z] in neighbours(p).into_iter().flatten() {
                if mask.value(x, y, z) == from {
                    mask.set(x, y, z, to);
                    current.push([x, y, z]);
                }
            }
        }
    };
    // from the nearest voxel inside the image (the start itself when it is inside)
    let from_nearest = |mask: &mut Volume<i16>| {
        let s = [
            start[0].clamp(0, nx - 1),
            start[1].clamp(0, ny - 1),
            start[2].clamp(0, nz - 1),
        ];
        mask.set(s[0], s[1], s[2], to);
        flood(mask, vec![s]);
    };
    // FSL's defined part one voxel outside: the start's neighbours inside the image
    let from_neighbours = |mask: &mut Volume<i16>| {
        let mut current = Vec::new();
        for [x, y, z] in neighbours(start).into_iter().flatten() {
            if mask.in_bounds(x, y, z) && mask.value(x, y, z) == from {
                mask.set(x, y, z, to);
                current.push([x, y, z]);
            }
        }
        flood(mask, current);
    };
    if inside {
        from_nearest(mask);
        return Fill::Inside;
    }
    if !fix {
        from_neighbours(mask);
        return Fill::Outside(start);
    }
    if near {
        // the correction is named only where it changes the mask (D13): FSL's fill as well,
        // on a copy (a start one voxel outside: a single slice)
        let mut theirs = mask.clone();
        from_neighbours(&mut theirs);
        from_nearest(mask);
        if theirs.data == mask.data {
            return Fill::Inside;
        }
    } else {
        // reproduced, nothing is filled here (FSL crashes): named only where the fill changes
        // the mask
        let before = mask.data.clone();
        from_nearest(mask);
        if before == mask.data {
            return Fill::Inside;
        }
    }
    Fill::Outside(start)
}

/// How [`fill_mask`] started.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fill {
    Inside,
    /// Outside the image, handled (FSL-43's notice).
    Outside([i64; 3]),
    /// Where FSL writes far outside the volume and crashes, corrected: nothing filled, refused.
    Crash([i64; 3]),
    /// The same, reproduced: the write FSL crashes on is not done and nothing is filled
    /// (FSL-43's notice).
    Skipped([i64; 3]),
}

/// The FSL-43 notice of a fill FSL crashes on, reproduced without it ([`Fill::Skipped`]).
pub(crate) fn skipped_notice(notices: &mut Notices, start: [i64; 3], what: &str) {
    notices.add(
        &defects::FSL_43,
        format!(
            "{what} starts at voxel {} {} {}, far outside the image (or not a number), where FSL \
             writes far outside the volume and crashes: not filled (the mask holds the drawn \
             surface only); everything else is FSL's",
            start[0], start[1], start[2]
        ),
    );
}

/// The refusal of a fill FSL crashes on (FSL-43).
pub(crate) fn crash_message(prog: &str, start: [i64; 3]) -> String {
    format!(
        "{prog}: stopped: the surface's centre lies outside the image (voxel {} {} {}), where \
         FSL's {prog} starts its fill of a mask, writing outside the image (FSL-43) (larmorx)",
        start[0], start[1], start[2]
    )
}

/// The FSL-43 notice of a fill that started outside the image.
pub(crate) fn outside_notice(notices: &mut Notices, start: [i64; 3], what: &str) {
    let detail = if notices.fix() {
        format!(
            "{what} starts at voxel {} {} {}, outside the image: filled from the nearest voxel inside it",
            start[0], start[1], start[2]
        )
    } else {
        format!(
            "{what} starts at voxel {} {} {}, outside the image, where FSL writes and reads outside \
             the volume: filled from the start's neighbours inside it",
            start[0], start[1], start[2]
        )
    };
    notices.add(&defects::FSL_43, detail);
}

/// `make_mask_from_mesh`: 1 everywhere, the surface drawn as 0, then 0 filled from the mesh's
/// centroid ([`fill_mask`]; the start, when outside the image: FSL-43).
pub fn make_mask_from_mesh(image: &Volume<f32>, m: &Mesh, fix: bool) -> (Volume<i16>, Fill) {
    let mut mask: Volume<i16> = Volume::copyconvert(image);
    for v in mask.data.iter_mut() {
        *v = 1;
    }
    let mut mask = draw_mesh(&mask, m);
    let (xd, yd, zd) = (
        f64::from(image.xdim()),
        f64::from(image.ydim()),
        f64::from(image.zdim()),
    );
    let mut c = V3::default();
    for p in &m.points {
        c = V3::new(c.x + p.coord.x, c.y + p.coord.y, c.z + p.coord.z);
    }
    let f = 1.0 / m.points.len() as f64;
    c = V3::new(c.x * f, c.y * f, c.z * f);
    c = V3::new(c.x / xd, c.y / yd, c.z / zd);
    let start = [trunc_i(c.x), trunc_i(c.y), trunc_i(c.z)];
    let outside = fill_mask(&mut mask, start, 1, 0, fix);
    (mask, outside)
}

/// `h2c`: Hounsfield units to Cormack units (`--ct`).
pub fn h2c(image: &Volume<f32>) -> Volume<f32> {
    let mut result = image.clone();
    let mx = image.max();
    let mn = image.min();
    if mn < -1024.0 {
        for v in result.data.iter_mut() {
            if *v < -1024.0 {
                *v = -1024.0;
            }
        }
    }
    let mn: f32 = -1024.0;
    let _range = mx - mn;
    let scale: f32 = 10.0;
    let uninteresting_dark: f32 = 900.0;
    let interesting_mid: f32 = 200.0;
    for v in result.data.iter_mut() {
        *v -= mn;
    }
    let mut extra1 = result.clone();
    for v in extra1.data.iter_mut() {
        *v -= uninteresting_dark;
        if *v < 0.0 {
            *v = 0.0;
        }
    }
    let mut extra9 = extra1.clone();
    for v in extra9.data.iter_mut() {
        if *v > interesting_mid {
            *v = interesting_mid;
        }
        *v *= scale - 1.0;
    }
    for ((r, e9), e1) in result.data.iter_mut().zip(&extra9.data).zip(&extra1.data) {
        // result = extra9 + extra1 + result
        let s = *e9 + *e1 + *r;
        *r = if s > 3000.0 { 100.0 } else { s };
    }
    result
}

/// `find_skull`: an approximate skull image (100 where the skull's outer edge is found along
/// each vertex's normal).
pub fn find_skull(image: &Volume<f32>, m: &Mesh, t2: f64, t: f64, t98: f64) -> Volume<f32> {
    const SKULL_SEARCH: f64 = 30.0;
    const SKULL_START: f64 = -3.0;
    let mut result = image.clone();
    for v in result.data.iter_mut() {
        *v = 0.0;
    }
    let mut volmesh: Volume<i16> = Volume::copyconvert(image);
    for v in volmesh.data.iter_mut() {
        *v = 1;
    }
    let volmesh = draw_mesh(&volmesh, m);
    let (xsize, ysize, zsize) = (
        volmesh.xsize() as f64,
        volmesh.ysize() as f64,
        volmesh.zsize() as f64,
    );
    let (xdim, ydim, zdim) = (
        f64::from(volmesh.xdim()),
        f64::from(volmesh.ydim()),
        f64::from(volmesh.zdim()),
    );
    let scale = fmin(xdim, fmin(ydim, zdim));
    let interp = |p: V3| f64::from(image.interpolate(p.x as f32, p.y as f32, p.z as f32));
    let inside =
        |p: V3| p.x >= 0.0 && p.y >= 0.0 && p.z >= 0.0 && p.x < xsize && p.y < ysize && p.z < zsize;
    for i in 0..m.points.len() {
        let mut max_neighbour = 0.0;
        let normal = m.local_normal(i);
        let n = V3::new(normal.x / xdim, normal.y / ydim, normal.z / zdim);
        let me = m.points[i].coord;
        for &nei in &m.points[i].neighbours {
            max_neighbour = fmax(me.sub(m.points[nei].coord).norm(), max_neighbour);
        }
        max_neighbour = (max_neighbour / 2.0).ceil();
        let mp = V3::new(me.x / xdim, me.y / ydim, me.z / zdim);
        let kz0 = trunc_i((mp.z - max_neighbour / zdim).floor());
        let kz1 = trunc_i((mp.z + max_neighbour / zdim).floor());
        let jy0 = trunc_i((mp.y - max_neighbour / ydim).floor());
        let jy1 = trunc_i((mp.y + max_neighbour / ydim).floor());
        let ix0 = trunc_i((mp.x - max_neighbour / xdim).floor());
        let ix1 = trunc_i((mp.x + max_neighbour / xdim).floor());
        for ck in kz0..=kz1 {
            for cj in jy0..=jy1 {
                for ci in ix0..=ix1 {
                    let point = V3::new(ci as f64, cj as f64, ck as f64);
                    let realpoint = V3::new(ci as f64 * xdim, cj as f64 * ydim, ck as f64 * zdim);
                    let mut compute = false;
                    if volmesh.get(ci, cj, ck) == 0 {
                        let mut mindist = 10000.0;
                        for &nei in &m.points[i].neighbours {
                            mindist = fmin(realpoint.sub(m.points[nei].coord).norm(), mindist);
                        }
                        if mindist >= realpoint.sub(me).norm() {
                            compute = true;
                        }
                    }
                    if !compute {
                        continue;
                    }
                    let mut maxval = t;
                    let mut minval = interp(point);
                    let mut d_max = 0.0;
                    let mut d = 0.0;
                    while d < SKULL_SEARCH {
                        let current = point.add(n.scale(d));
                        let val = interp(current);
                        if val > maxval {
                            maxval = val;
                            d_max = d;
                        }
                        if val < minval {
                            minval = val;
                        }
                        d += scale * 0.5;
                    }
                    let _ = minval;
                    if maxval > t {
                        let mut d_min = SKULL_START;
                        let mut max_j = -1_000_000.0;
                        let mut d = SKULL_START;
                        while d < d_max {
                            let current = point.add(n.scale(d));
                            if inside(current) {
                                let tmpf = d / 30.0 - interp(current) / (t98 - t2);
                                if tmpf > max_j {
                                    max_j = tmpf;
                                    d_min = d;
                                }
                            }
                            d += scale * 0.5;
                        }
                        let mut maxgrad = 0.0;
                        let mut d_skull = 0.0;
                        let current2 = point.add(n.scale(d_min));
                        if inside(current2) {
                            let mut val2 = interp(current2);
                            let mut d = d_min + scale;
                            while d < d_max {
                                let current = point.add(n.scale(d));
                                if inside(current) {
                                    let val = interp(current);
                                    let grad = val - val2;
                                    val2 = val;
                                    if grad > 0.0 {
                                        if grad > maxgrad {
                                            maxgrad = grad;
                                            d_skull = d;
                                        } else {
                                            d = d_max;
                                        }
                                    }
                                }
                                d += 0.5 * scale;
                            }
                        }
                        if maxgrad > 0.0 {
                            let c3 = point.add(n.scale(d_skull));
                            if inside(c3) {
                                result.set(trunc_i(c3.x), trunc_i(c3.y), trunc_i(c3.z), 100.0);
                            }
                        }
                    }
                }
            }
        }
    }
    result
}

/// The settings of a run besides its options.
pub struct Run<'a> {
    pub fsl: &'a Settings,
    pub mode: UpstreamDefects,
    /// Threads for the work that can be shared (betsurf's smoothing); 1 or more. The results
    /// do not depend on it.
    pub threads: usize,
    /// Whether `reproduce` fills what FSL leaves undefined where FSL crashes (D13's failure
    /// cases: bet2 and betsurf run alone), or stops as FSL's run does (bet2 inside the `bet`
    /// script, whose ERR trap then prints its failure: FSL's defined outcome).
    pub fill: bool,
}

/// Prints an uncaught exception of the image layer: what FSL printed before aborting, then
/// larmorx's own line (FSL's program aborts: no defined result in either mode).
pub(crate) fn report_abort(e: &FslError, prog: &str, err: &mut dyn Write) -> u8 {
    let _ = err.write_all(e.printed_text().as_bytes());
    let what = e.what.replace('\n', " / ");
    let _ = writeln!(
        err,
        "{prog}: stopped: FSL's {prog} aborts here with an uncaught {} ({what}) (larmorx)",
        e.type_name
    );
    1
}

/// Prints the notices of FSL's defects as the `larmorx-nc` program prints every tool's:
/// `larmorx-nc: upstream defect <ID> (<tool>) corrected|reproduced: ...`.
pub(crate) fn print_notices(notices: &Notices, err: &mut dyn Write) {
    for n in notices.as_slice() {
        let _ = writeln!(err, "larmorx-nc: {}", n.line());
    }
}

/// Prints why a save failed: `FSLOUTPUTTYPE`'s messages (FSL exits with status 1), or an
/// uncaught exception ([`report_abort`]). Returns the exit status.
pub(crate) fn report_save(e: &SaveError, prog: &str, err: &mut dyn Write) -> u8 {
    match e {
        SaveError::OutputType(o) => {
            for l in &o.printed {
                let _ = writeln!(err, "{l}");
            }
            1
        }
        SaveError::Exception(e) => report_abort(e, prog, err),
    }
}

/// `bet2`'s `main`: `args[0]` is the program name. Returns the exit status.
pub fn main(args: &[String], run: &Run, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let mut notices = Notices::new(run.mode);
    let code = main_inner(args, run, out, err, &mut notices);
    crate::bet2::print_notices(&notices, err);
    code
}

/// [`main`] collecting its notices into `notices` (printed by the caller): how `bet` runs it.
pub fn main_with(
    args: &[String],
    run: &Run,
    out: &mut dyn Write,
    err: &mut dyn Write,
    notices: &mut Notices,
) -> u8 {
    main_inner(args, run, out, err, notices)
}

fn main_inner(
    args: &[String],
    run: &Run,
    out: &mut dyn Write,
    err: &mut dyn Write,
    notices: &mut Notices,
) -> u8 {
    let parser = parser();
    let argc = args.len();
    if argc < 3 {
        if argc == 1 {
            let _ = err.write_all(parser.usage().as_bytes());
            return 1;
        }
        let s = &args[1];
        if s.contains("-h") || s.contains("--help") {
            let _ = err.write_all(parser.usage().as_bytes());
            return 0;
        }
        let _ = writeln!(err, "error: not enough arguments, use bet -h for help");
        return 255;
    }
    let inputname = args[1].clone();
    let outputname = args[2].clone();
    if inputname.starts_with('-') || outputname.starts_with('-') {
        let _ = writeln!(
            err,
            "error : two first arguments should be input and output names, see bet -h for help"
        );
        return 255;
    }
    let parsed = match parser.parse(args, 2) {
        Ok(p) => p,
        Err((_, what)) => {
            let _ = err.write_all(parser.usage().as_bytes());
            let _ = writeln!(err, "\n{what}");
            return 1;
        }
    };
    let get = |k: &str| parsed.get(&parser, k);
    let c = get("-c");
    let a = Bet2Args {
        outline: get("-o,--outline").bool(),
        mask: get("-m,--mask").bool(),
        skull: get("-s,--skull").bool(),
        no_output: get("-n,--nooutput").bool(),
        frac: get("-f").float(),
        grad: get("-g").float(),
        radius: get("-r,--radius").float(),
        smooth: get("-w,--smooth").float(),
        centre: c.set.then(|| [c.float_at(0), c.float_at(1), c.float_at(2)]),
        threshold: get("-t,--threshold").bool(),
        ct: get("--ct").bool(),
        mesh: get("-e,--mesh").bool(),
        verbose: get("-v,--verbose").bool(),
        d1: get("--parameter_d1").float(),
        d2: get("--parameter_d2").float(),
    };
    if a.frac < 0.0 || a.frac > 1.0 {
        let _ = writeln!(
            err,
            "\nFractional intensity threshold must be between 0 and 1!"
        );
        return 1;
    }
    if a.grad < -1.0 || a.grad > 1.0 {
        let _ = writeln!(err, "\nVertical gradient must be between -1 and 1!");
        return 1;
    }
    if get("-h,--help").bool() {
        let _ = err.write_all(parser.usage().as_bytes());
        return 0;
    }
    // FSL-42 (rule B): values without meaning, which bet2 uses as given.
    if let Some(what) = meaningless_values(&a) {
        if notices.fix() {
            notices.add(
                &defects::FSL_42,
                format!("{what}: refused (bet2 computes with it)"),
            );
            let _ = writeln!(err, "\nbet2: {what} (larmorx)");
            return 1;
        }
        notices.add(
            &defects::FSL_42,
            format!("{what}: used as given, as bet2 does"),
        );
    }
    // FSL-40: bet2 removes ".hdr" (and, meaning ".img", ".hdr" again) wherever it appears.
    let (in_name, out_name) = match strip_names(&inputname, &outputname, notices) {
        Ok(v) => v,
        Err(code) => {
            let _ = writeln!(
                err,
                "bet2: stopped: FSL's bet2 aborts here with an uncaught std::out_of_range \
                 (basic_string::erase: a name with .img but no .hdr, FSL-40) (larmorx)"
            );
            return code;
        }
    };
    let out_name = if out_name == "default__default" {
        format!("{in_name}_brain")
    } else {
        out_name
    };
    let testvol: Volume<f32> = match volume::read_volume::<f32>(&in_name, run.fsl) {
        Ok((v, warning)) => {
            if let Some(w) = warning {
                let _ = writeln!(err, "{w}");
            }
            v
        }
        Err(e) => return report_abort(&e, "bet2", err),
    };
    // With --ct the extraction runs on the converted image and the brain is cut from the
    // original one; without it there is one image (no copy: the memory FSL's bet2 needs).
    let (testvol, ctvol) = if a.ct {
        (h2c(&testvol), Some(testvol))
    } else {
        (testvol, None)
    };
    match compute(&testvol, &a, run.fill, out, err, notices) {
        Ok(res) => write_outputs(
            &testvol,
            ctvol.as_ref(),
            &a,
            res,
            &out_name,
            run,
            err,
            notices,
        ),
        Err(code) => code,
    }
}

/// The notice of FSL-56 for outputs that carry `n` extensions of different sizes.
pub(crate) fn offset_notice(notices: &mut Notices, n: usize) {
    let detail = if notices.fix() {
        format!("the outputs' vox_offset counts each of their {n} extensions' sizes")
    } else {
        format!("the outputs' vox_offset counts the first of their {n} extensions' size {n} times")
    };
    notices.add(&defects::FSL_56, detail);
}

/// The values bet2 accepts but cannot compute with (FSL-42).
fn meaningless_values(a: &Bet2Args) -> Option<String> {
    if a.frac.is_nan() {
        return Some("the fractional intensity threshold (-f) is not a number".into());
    }
    if a.grad.is_nan() {
        return Some("the vertical gradient (-g) is not a number".into());
    }
    if !a.radius.is_finite() || a.radius < 0.0 {
        return Some(format!(
            "the head radius (-r) must be a finite number of millimetres, 0 or more (0 estimates it), not {}",
            cnum::stream(f64::from(a.radius))
        ));
    }
    if !a.smooth.is_finite() || a.smooth <= 0.0 {
        return Some(format!(
            "the smoothness factor (-w) must be a finite number above 0, not {}",
            cnum::stream(f64::from(a.smooth))
        ));
    }
    if let Some(c) = a.centre
        && c.iter().any(|v| !v.is_finite())
    {
        return Some("the centre (-c) must be three finite numbers of voxels".into());
    }
    for (name, v) in [("--parameter_d1", a.d1), ("--parameter_d2", a.d2)] {
        if !v.is_finite() || !(-2_147_483_648.0..2_147_483_648.0).contains(&v) {
            return Some(format!(
                "{name} must be a finite number within the range of an integer, not {}",
                cnum::stream(f64::from(v))
            ));
        }
    }
    None
}

/// bet2's handling of `.hdr` and `.img` in the input and output names (FSL-40): it erases
/// `.hdr` at its first occurrence, and for a name containing `.img` it erases at the position
/// of `.hdr` (out of range when there is none: std::out_of_range, an abort). The correction
/// removes the image extension `.hdr` or `.img` only at the end of a name.
fn strip_names(input: &str, output: &str, notices: &mut Notices) -> Result<(String, String), u8> {
    let fsl = |s: &str| -> Option<String> {
        let mut s = s.to_owned();
        if let Some(p) = s.find(".hdr") {
            s.replace_range(p..p + 4, "");
        }
        if s.contains(".img") {
            let p = s.find(".hdr")?;
            s.replace_range(p..(p + 4).min(s.len()), "");
        }
        Some(s)
    };
    let fixed = |s: &str| -> String {
        for ext in [".hdr", ".img"] {
            if let Some(b) = s.strip_suffix(ext) {
                return b.to_owned();
            }
        }
        s.to_owned()
    };
    let mut pair = Vec::new();
    for name in [output, input] {
        let original = fsl(name);
        let corrected = fixed(name);
        if original.as_deref() != Some(corrected.as_str()) {
            if notices.fix() {
                notices.add(
                    &defects::FSL_40,
                    format!(
                        "'{name}' read as '{corrected}' (bet2 {})",
                        match &original {
                            Some(o) => format!("reads '{o}'"),
                            None => "aborts".into(),
                        }
                    ),
                );
            } else {
                notices.add(
                    &defects::FSL_40,
                    match &original {
                        Some(o) => format!("'{name}' read as '{o}', as bet2 does"),
                        None => format!("'{name}': bet2 aborts (std::out_of_range)"),
                    },
                );
            }
        }
        if notices.fix() {
            pair.push(corrected);
        } else {
            match original {
                Some(o) => pair.push(o),
                None => return Err(1),
            }
        }
    }
    let input = pair.pop().unwrap_or_default();
    let output = pair.pop().unwrap_or_default();
    Ok((input, output))
}

/// The surface and the brain mask, with bet2's verbose messages. `fill`: whether `reproduce`
/// fills what FSL leaves undefined where FSL crashes ([`Run::fill`]).
pub fn compute(
    testvol: &Volume<f32>,
    a: &Bet2Args,
    fill: bool,
    out: &mut dyn Write,
    err: &mut dyn Write,
    notices: &mut Notices,
) -> Result<Bet2Result, u8> {
    let (mut xarg, mut yarg, mut zarg) = (0.0f64, 0.0, 0.0);
    if let Some(c) = a.centre {
        let v = [f64::from(c[0]), f64::from(c[1]), f64::from(c[2]), 1.0];
        let v = arma::gemv_tinysq(&testvol.niftivox2newimagevox_mat(), &v, false);
        xarg = v[0];
        yarg = v[1];
        zarg = v[2];
    }
    let bet_main_parameter = glibc_double::pow(f64::from(a.frac), 0.275);
    let mut m = Mesh::icosa(5);
    let mut warn = String::new();
    let bp = match adjust_initial_mesh(
        testvol,
        &mut m,
        f64::from(a.radius),
        xarg,
        yarg,
        zarg,
        &mut warn,
        notices.reproduce() && fill,
    ) {
        Ok((bp, filled)) => {
            let _ = err.write_all(warn.as_bytes());
            if filled {
                notices.add(
                    &defects::FSL_41,
                    "no voxel inside the initial sphere has an intensity strictly between the \
                     2nd and 98th percentiles, so the median brain intensity is undefined (FSL \
                     reads the end of an empty list and crashes): it is taken as 0 and the run \
                     goes on; every other value is FSL's",
                );
            }
            bp
        }
        Err(_) => {
            let _ = err.write_all(warn.as_bytes());
            let (detail, how) = if notices.reproduce() {
                (
                    "no voxel inside the initial sphere has an intensity strictly between the \
                     2nd and 98th percentiles (a constant or binary image), so the median brain \
                     intensity is undefined: FSL's bet2 crashes here and the bet script stops \
                     with its failure message, as here",
                    "the bet script stops here, as FSL's does",
                )
            } else {
                (
                    "no voxel inside the initial sphere has an intensity strictly between the \
                     2nd and 98th percentiles (a constant or binary image), so the median brain \
                     intensity is undefined: refused",
                    "LARMORX_UPSTREAM_DEFECTS=reproduce fills it with 0 and goes on",
                )
            };
            notices.add(&defects::FSL_41, detail);
            let _ = writeln!(
                err,
                "bet2: stopped: no voxel inside the initial sphere has an intensity strictly \
                 between the 2nd and 98th percentiles, so the median brain intensity is \
                 undefined; FSL's bet2 reads an empty list here and crashes (FSL-41; {how}) \
                 (larmorx)"
            );
            return Err(1);
        }
    };
    if a.verbose {
        let _ = writeln!(
            out,
            "min {} thresh2 {} thresh {} thresh98 {} max {}",
            cnum::stream(bp.min),
            cnum::stream(bp.t2),
            cnum::stream(bp.t),
            cnum::stream(bp.t98),
            cnum::stream(bp.max)
        );
        let _ = writeln!(
            out,
            "c-of-g {} {} {} mm",
            cnum::stream(bp.cog.x),
            cnum::stream(bp.cog.y),
            cnum::stream(bp.cog.z)
        );
        let _ = writeln!(out, "radius {} mm", cnum::stream(bp.radius));
        let _ = writeln!(out, "median within-brain intensity {}", cnum::stream(bp.tm));
    }
    let moriginal = m.copy();
    let rmin = 3.33 * f64::from(a.smooth);
    let rmax = f64::from(10.0f32 * a.smooth);
    let e = (1.0 / rmin + 1.0 / rmax) / 2.0;
    let f = 6.0 / (1.0 / rmin - 1.0 / rmax);
    const NB_ITER: i32 = 1000;
    const SELF_INTERSECTION_THRESHOLD: f64 = 4000.0;
    let d1 = cnum::f64_to_i32(f64::from(a.d1));
    let d2 = cnum::f64_to_i32(f64::from(a.d2));
    let grad = f64::from(a.grad);
    let mut l = 0.0;
    for i in 0..NB_ITER {
        step_of_computation(
            testvol,
            &mut m,
            bet_main_parameter,
            0,
            0.0,
            i,
            &mut l,
            bp.t2,
            bp.tm,
            bp.t,
            e,
            f,
            bp.cog.z,
            bp.radius,
            grad,
            d1,
            d2,
        );
    }
    let mut msgs = String::new();
    let tmp = m.self_intersection(&moriginal, &mut msgs);
    let _ = err.write_all(msgs.as_bytes());
    if a.verbose && !a.mesh {
        let _ = writeln!(
            out,
            "self-intersection total {} (threshold=4000.0) ",
            cnum::stream(tmp)
        );
    }
    let mut self_intersection = if !a.mesh {
        tmp > SELF_INTERSECTION_THRESHOLD
    } else {
        m.real_self_intersection()
    };
    let mut pass = 0;
    if a.verbose && a.mesh && self_intersection {
        let _ = writeln!(out, "the mesh is self-intersecting ");
    }
    while self_intersection {
        if self_intersection && a.verbose {
            let _ = writeln!(out, "thus will rerun with higher smoothness constraint");
        }
        m.assign(&moriginal);
        l = 0.0;
        pass += 1;
        for i in 0..NB_ITER {
            let mut incfactor = pow10(pass + 1);
            if f64::from(i) > 0.75 * f64::from(NB_ITER) {
                incfactor =
                    4.0 * (1.0 - f64::from(i) / f64::from(NB_ITER)) * (incfactor - 1.0) + 1.0;
            }
            step_of_computation(
                testvol,
                &mut m,
                bet_main_parameter,
                pass,
                incfactor,
                i,
                &mut l,
                bp.t2,
                bp.tm,
                bp.t,
                e,
                f,
                bp.cog.z,
                bp.radius,
                grad,
                d1,
                d2,
            );
        }
        let mut msgs = String::new();
        let tmp = m.self_intersection(&moriginal, &mut msgs);
        let _ = err.write_all(msgs.as_bytes());
        self_intersection = if !a.mesh {
            tmp > SELF_INTERSECTION_THRESHOLD
        } else {
            m.real_self_intersection()
        };
        if a.verbose && !a.mesh {
            let _ = writeln!(
                out,
                "self-intersection total {} (threshold=4000.0) ",
                cnum::stream(tmp)
            );
        }
        if a.verbose && a.mesh && self_intersection {
            let _ = writeln!(out, "the mesh is self-intersecting");
        }
        if pass == 10 {
            self_intersection = false;
        }
    }
    let (outside, start) = make_mask_from_mesh(testvol, &m, notices.fix() || !fill);
    match start {
        Fill::Inside => {}
        // named only where an output shows the mask (the brain, `-m`): with `-n` and no `-m`
        // the correction changes nothing written (D13: a notice where the result differs)
        Fill::Outside(s) if !a.no_output || a.mask => {
            outside_notice(notices, s, "the fill of the brain mask")
        }
        Fill::Outside(_) => {}
        Fill::Skipped(s) => skipped_notice(notices, s, "the fill of the brain mask"),
        Fill::Crash(s) => {
            let _ = writeln!(err, "{}", crash_message("bet2", s));
            return Err(1);
        }
    }
    Ok(Bet2Result {
        params: bp,
        mesh: m,
        outside,
        passes: pass,
    })
}

/// `pow(10.0, (double) k)`: exact for these small powers.
fn pow10(k: i32) -> f64 {
    let mut v = 1.0;
    for _ in 0..k {
        v *= 10.0;
    }
    v
}

/// Writes bet2's outputs: the brain (unless `-n`), `_mask`, `_overlay`, `_mesh.vtk`, `_skull`.
#[allow(clippy::too_many_arguments)]
fn write_outputs(
    testvol: &Volume<f32>,
    ctvol: Option<&Volume<f32>>,
    a: &Bet2Args,
    res: Bet2Result,
    out_name: &str,
    run: &Run,
    err: &mut dyn Write,
    notices: &mut Notices,
) -> u8 {
    if volume::offset_defect_applies(testvol, run.fsl) {
        offset_notice(notices, testvol.extensions.len());
    }
    // The masks are the result's own (moved, not copied), so that bet2 needs no more memory
    // than FSL's.
    let mut brainmask = res.outside;
    if a.threshold {
        let t = res.params.t;
        for (b, &v) in brainmask.data.iter_mut().zip(&testvol.data) {
            if f64::from(v) < t {
                *b = 1;
            }
        }
    }
    if !a.no_output {
        let mut output = ctvol.unwrap_or(testvol).clone();
        let n = output.nvoxels() as usize;
        for (o, &b) in output.data[..n].iter_mut().zip(&brainmask.data) {
            // a NaN voxel stays its NaN; an infinite one outside the brain is SSE's NaN
            *o = sse::mul_f32(*o, f32::from(1 - b));
        }
        if let Some(code) = save_output(&output, out_name, run, err) {
            return code;
        }
    }
    if a.mask {
        let mut m = brainmask;
        m.display_max = 1.0;
        m.display_min = 0.0;
        for v in m.data.iter_mut() {
            *v = 1 - *v;
        }
        if let Some(code) = save_output(&m, &format!("{out_name}_mask"), run, err) {
            return code;
        }
    }
    if a.outline {
        let outline = draw_mesh_bis(testvol, &res.mesh);
        if let Some(code) = save_output(&outline, &format!("{out_name}_overlay"), run, err) {
            return code;
        }
    }
    if a.mesh {
        let mut msgs = String::new();
        res.mesh
            .save_vtk(&format!("{out_name}_mesh.vtk"), &mut msgs);
        let _ = err.write_all(msgs.as_bytes());
    }
    if a.skull {
        let skull = find_skull(
            testvol,
            &res.mesh,
            res.params.t2,
            res.params.t,
            res.params.t98,
        );
        let bskull: Volume<i16> = Volume::copyconvert(&skull);
        if let Some(code) = save_output(&bskull, &format!("{out_name}_skull"), run, err) {
            return code;
        }
    }
    0
}

/// `save_volume(v, name)` of one of bet2's outputs: newimage's `descrip`, NewNifti's data
/// offset of several extensions reproduced only in `reproduce` (FSL-56). `Some(status)` when
/// bet2 stops: an image with no volume (FSL returns -1 from `main`: 255), `FSLOUTPUTTYPE`'s
/// failure, or an exception.
pub(crate) fn save_output<T: Elem>(
    v: &Volume<T>,
    name: &str,
    run: &Run,
    err: &mut dyn Write,
) -> Option<u8> {
    let opts =
        SaveOptions::new(volume::BUILDSTRING.as_bytes()).reproducing_offset(run.mode.reproduce());
    match v.save_volume(name, run.fsl, &opts) {
        Ok(Saved::Written) => None,
        Ok(Saved::Nothing) => Some(255),
        Err(e) => Some(report_save(&e, "bet2", err)),
    }
}
