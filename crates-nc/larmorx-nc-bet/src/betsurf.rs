// SPDX-License-Identifier: LicenseRef-FSL
// Copyright 2026 Karel Lopez Vilaret (the translation).
//! `betsurf` (fsl-bet2 2111.8, `betsurf.cpp`): the inner skull, outer skull and scalp surfaces
//! (M. Jenkinson, M. Pechaud, S. Smith, "BET2: MR-based estimation of brain, skull and scalp
//! surfaces", OHBM 2005), translated line by line.
//!
//! From bet2's brain surface (`<bet>_mesh.vtk`) and the T1's registration to MNI152
//! (`<t1>_to_std.mat`, which only decides which profiles cross the eyes), every voxel on the
//! surface casts an intensity profile along the surface's normal through the T1 (and the T2,
//! registered to it). The inner skull, outer skull and scalp positions found on each profile are
//! counted into three images, which are smoothed; three surfaces are then deformed onto them in
//! turn: the brain surface outwards to the inner skull (800 iterations), the same surface on to
//! the outer skull (800), and a new sphere from inside to the scalp (1500), each kept outside the
//! previous one's mask. `--t1only` reads the profiles from the T1 alone.

use std::io::Write;

use larmorx_core::defects::{self, Notices};

use crate::bet2::{Fill, Run, crash_message, fill_mask, outside_notice, report_abort};
use larmorx_nc_core::cnum;
use larmorx_nc_core::fdlibm;
use larmorx_nc_core::istream::{Got, IStream};
use larmorx_nc_core::mesh::load::{self, LoadError};
use larmorx_nc_core::mesh::profile::{NOT_FOUND, Profile};
use larmorx_nc_core::mesh::{Mesh, V3};
use larmorx_nc_core::newimagefns;
use larmorx_nc_core::options::{ArgFlag, Opt, OptionParser, Value};
use larmorx_nc_core::settings::Settings;
use larmorx_nc_core::sse::{self, S64};
use larmorx_nc_core::stdsort;
use larmorx_nc_core::volume::{self, FslError, SaveError, SaveOptions, Saved, Volume};

const TITLE: &str = "BETSURF (BET Surface Finder) v2.1 - FMRIB Analysis Group, Oxford";
const EXAMPLES: &str = " betsurf          [options] <t1> <t2> <bet_mesh.vtk> <t1_to_standard.mat> <output>\n betsurf --t1only [options] <t1>      <bet_mesh.vtk> <t1_to_standard.mat> <output>";

/// `trMatrix`: the T1-to-standard matrix, row by row.
type TrMatrix = [f64; 16];

/// MISCMATHS' `Min` and `Max`.
#[inline]
fn fmin(a: f64, b: f64) -> f64 {
    if a < b { a } else { b }
}

#[inline]
fn fmax(a: f64, b: f64) -> f64 {
    if a > b { a } else { b }
}

/// `std::min` and `std::max` (the first operand unless the second is smaller, or larger).
#[inline]
fn std_min(a: f64, b: f64) -> f64 {
    if b < a { b } else { a }
}

#[inline]
fn std_max(a: f64, b: f64) -> f64 {
    if a < b { b } else { a }
}

/// C's `(int) x` for a double (x86-64: `INT_MIN` out of range).
#[inline]
fn trunc_i(x: f64) -> i64 {
    i64::from(cnum::f64_to_i32(x))
}

/// The options of `betsurf`, in the order FSL adds them.
pub fn parser() -> OptionParser {
    let b = |key, help| Opt {
        key,
        default: Value::Bool(false),
        help,
        compulsory: false,
        flag: ArgFlag::NoArgument,
        visible: true,
    };
    OptionParser::new(
        TITLE,
        EXAMPLES,
        vec![
            b("-h,--help", "displays this help, then exits"),
            b("-v,--verbose", "switch on diagnostic messages"),
            b("-1,--t1only", "extraction with t1 only"),
            b("-o,--outline", "generates all surface outlines"),
            b("-m,--mask", "generates binary masks from the meshes"),
            b("-s,--skullmask", "generates skull binary mask"),
            Opt {
                key: "-p,--increased_precision",
                default: Value::Int(0),
                help: "~<int>\tretessellates the meshes the indicated number of times",
                compulsory: false,
                flag: ArgFlag::Requires(1),
                visible: true,
            },
        ],
    )
}

/// What betsurf was asked for.
#[derive(Clone, Debug, Default)]
pub struct BetsurfArgs {
    pub verbose: bool,
    pub t1only: bool,
    pub outline: bool,
    pub mask: bool,
    pub skullmask: bool,
    pub precision: i32,
}

/// `betsurf`'s main (`args[0]` is the program name). Returns the exit status.
pub fn main(args: &[String], run: &Run, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let mut notices = Notices::new(run.mode);
    let code = main_inner(args, run, out, err, &mut notices);
    crate::bet2::print_notices(&notices, err);
    code
}

fn main_inner(
    args: &[String],
    run: &Run,
    out: &mut dyn Write,
    err: &mut dyn Write,
    notices: &mut Notices,
) -> u8 {
    let parser = parser();
    let parsed = match parser.parse_command_line(args, 0, true) {
        Ok(p) => p,
        Err((_, what)) => {
            let _ = err.write_all(parser.usage().as_bytes());
            let _ = writeln!(err, "\n{what}");
            return 1;
        }
    };
    let get = |k: &str| parsed.get(&parser, k);
    if get("-h,--help").bool() {
        let _ = err.write_all(parser.usage().as_bytes());
        return 0;
    }
    let nb_pars = parsed.stop;
    if args[nb_pars.min(args.len())..]
        .iter()
        .any(|s| s.starts_with('-'))
    {
        let _ = err.write_all(parser.usage().as_bytes());
        return 255;
    }
    let a = BetsurfArgs {
        verbose: get("-v,--verbose").bool(),
        t1only: get("-1,--t1only").bool(),
        outline: get("-o,--outline").bool(),
        mask: get("-m,--mask").bool(),
        skullmask: get("-s,--skullmask").bool(),
        precision: get("-p,--increased_precision").int(),
    };
    let rest = &args[nb_pars.min(args.len())..];
    let needed = if a.t1only { 4 } else { 5 };
    let code = if rest.len() < needed {
        let _ = writeln!(err, "too few arguments");
        let _ = err.write_all(parser.usage().as_bytes());
        255
    } else {
        let names = if a.t1only {
            Names {
                t1: rest[0].clone(),
                t2: None,
                mesh: rest[1].clone(),
                matrix: rest[2].clone(),
                output: rest[3].clone(),
            }
        } else {
            Names {
                t1: rest[0].clone(),
                t2: Some(rest[1].clone()),
                mesh: rest[2].clone(),
                matrix: rest[3].clone(),
                output: rest[4].clone(),
            }
        };
        match surfaces(&names, &a, run, out, err, notices) {
            Ok(()) => 0,
            Err(Fail::Exit(c)) => c,
            Err(Fail::Abort(e)) => return report_abort(&e, "betsurf", err),
            Err(Fail::Refused(why)) => {
                let _ = writeln!(err, "betsurf: {why} (larmorx)");
                return 1;
            }
        }
    };
    if a.t1only && code != 0 {
        // FSL-54: t1only_main's status is dropped (exit(0))
        if notices.fix() {
            notices.add(
                &defects::FSL_54,
                format!("--t1only failed: exit status {code}, where betsurf exits 0"),
            );
        } else {
            notices.add(
                &defects::FSL_54,
                format!("exit status 0 for a failure ({code})"),
            );
            return 0;
        }
    }
    code
}

/// The files of a run.
struct Names {
    t1: String,
    t2: Option<String>,
    mesh: String,
    matrix: String,
    output: String,
}

/// How a run stops early.
enum Fail {
    /// FSL returns or exits with this status (its message printed already).
    Exit(u8),
    /// An uncaught exception of the image layer.
    Abort(FslError),
    /// A case without a defined result in FSL (a crash, memory never set, a hang).
    Refused(String),
}

/// Reads the matrix file as `f >> M.m11 >> ... >> M.m44` (FSL-53 for values that do not read).
fn read_matrix(name: &str, err: &mut dyn Write, notices: &mut Notices) -> Result<TrMatrix, Fail> {
    let Some(bytes) = load::read_like_ifstream(name) else {
        let _ = writeln!(err, "unable to open {name}");
        return Err(Fail::Exit(255));
    };
    let mut f = IStream::new(&bytes);
    let mut m = [0.0; 16];
    let mut unset = None;
    for (k, v) in m.iter_mut().enumerate() {
        match f.f64() {
            Got::Value(x) => *v = x,
            Got::Failed(x) => {
                *v = x;
                unset.get_or_insert(k);
            }
            Got::Untouched => {
                unset.get_or_insert(k);
            }
        }
    }
    // betsurf uses the first three rows (12 numbers)
    if let Some(k) = unset.filter(|&k| k < 12) {
        let what = format!(
            "the matrix file {name} holds {k} readable numbers before its first unreadable or \
             missing one, and betsurf uses 12"
        );
        if notices.fix() {
            notices.add(&defects::FSL_53, format!("{what}: refused"));
            return Err(Fail::Refused(format!("{what} (FSL-53)")));
        }
        notices.add(
            &defects::FSL_53,
            format!("{what}: the missing ones are 0 (FSL leaves them at values never set)"),
        );
    }
    Ok(m)
}

fn read_t(name: &str, settings: &Settings, err: &mut dyn Write) -> Result<Volume<f32>, Fail> {
    match volume::read_volume::<f32>(name, settings) {
        Ok((v, warning)) => {
            if let Some(w) = warning {
                let _ = writeln!(err, "{w}");
            }
            Ok(v)
        }
        Err(e) => Err(Fail::Abort(e)),
    }
}

/// `draw_segment` (betsurf's: 1 along the segment, points outside the image dropped).
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
            image.set(x, y, z, 1);
        }
        i += mininc;
    }
}

/// `draw_mesh` (betsurf's: the surface drawn as 1).
fn draw_mesh(image: &mut Volume<i16>, m: &Mesh) {
    let dims = (
        f64::from(image.xdim()),
        f64::from(image.ydim()),
        f64::from(image.zdim()),
    );
    let mininc = fmin(dims.0, fmin(dims.1, dims.2)) * 0.5;
    for &t in &m.tri_list {
        let [a, b, c] = m.tris[t].v;
        let (pa, pb, pc) = (m.points[a].coord, m.points[b].coord, m.points[c].coord);
        let mut n = pa.sub(pb);
        let d = n.norm();
        n.normalize();
        let mut j = 0.0;
        while j <= d {
            let p = pb.add(n.scale(j));
            draw_segment(image, p, pc, mininc, dims);
            j += mininc;
        }
    }
}

/// The centroid of a mesh's vertices in voxels, truncated: where betsurf's fill starts.
fn fill_start(image: &Volume<i16>, m: &Mesh) -> [i64; 3] {
    let mut c = V3::default();
    for p in &m.points {
        c = V3::new(c.x + p.coord.x, c.y + p.coord.y, c.z + p.coord.z);
    }
    let f = 1.0 / m.points.len() as f64;
    c = V3::new(c.x * f, c.y * f, c.z * f);
    c = V3::new(
        c.x / f64::from(image.xdim()),
        c.y / f64::from(image.ydim()),
        c.z / f64::from(image.zdim()),
    );
    [trunc_i(c.x), trunc_i(c.y), trunc_i(c.z)]
}

/// `make_mask_from_mesh` (betsurf's): the image with 1 filled in from the mesh's centroid
/// through the 6-connected voxels that are 0 (a start outside the image: FSL-43).
fn make_mask_from_mesh(
    image: &Volume<i16>,
    m: &Mesh,
    notices: &mut Notices,
) -> Result<Volume<i16>, Fail> {
    if m.points.is_empty() && notices.fix() {
        return Err(Fail::Refused(
            "the mesh has no vertices: FSL's betsurf divides by 0 for its centre and writes \
             outside the image"
                .into(),
        ));
    }
    // reproduced, an empty mesh's centre is 0/0: not a number, so its fill is skipped (FSL-43)
    let start = fill_start(image, m);
    fill_from(image, start, notices)
}

fn fill_from(
    image: &Volume<i16>,
    start: [i64; 3],
    notices: &mut Notices,
) -> Result<Volume<i16>, Fail> {
    let mut mask = image.clone();
    match fill_mask(&mut mask, start, 0, 1, notices.fix()) {
        Fill::Inside => {}
        Fill::Outside(s) => outside_notice(notices, s, "the fill of a surface's mask"),
        Fill::Skipped(s) => {
            crate::bet2::skipped_notice(notices, s, "the fill of a surface's mask");
        }
        Fill::Crash(s) => {
            let msg = crash_message("betsurf", s);
            let why = msg
                .trim_start_matches("betsurf: stopped: ")
                .trim_end_matches(" (larmorx)")
                .to_owned();
            return Err(Fail::Refused(why));
        }
    }
    Ok(mask)
}

/// [`fill_from`] without a notice (the comparison of FSL-47's two starts).
fn fill_quiet(image: &Volume<i16>, start: [i64; 3], fix: bool) -> Volume<i16> {
    let mut mask = image.clone();
    let _ = fill_mask(&mut mask, start, 0, 1, fix);
    mask
}

/// A volume of `image`'s grid and properties, as `short`, every voxel 0 (`copyconvert(image,
/// v); v = 0`).
fn zeros_like<T: volume::Elem>(image: &Volume<T>) -> Volume<i16> {
    let mut v: Volume<i16> = Volume::copyconvert(image);
    v.data.iter_mut().for_each(|x| *x = 0);
    v
}

/// The deformation's state between steps: the mean edge length (`l`, a global in FSL),
/// updated every 50 iterations.
struct Deform {
    l: f64,
}

const RMIN: f64 = 3.33;
const RMAX: f64 = 10.0;

impl Deform {
    /// `standard_step_of_computation(image, m, iteration, E, F, addsmooth, speed, nb_iter,
    /// id, od, vol, mask)`.
    #[allow(clippy::too_many_arguments)]
    fn step(
        &mut self,
        image: &Volume<f32>,
        m: &mut Mesh,
        addsmooth: f32,
        speed: f32,
        nb_iter: i32,
        mask: Option<&Volume<i16>>,
    ) {
        let e = (1.0 / RMIN + 1.0 / RMAX) / 2.0;
        let f = 6.0 / (1.0 / RMIN - 1.0 / RMAX);
        let (xdim, ydim, zdim) = (
            f64::from(image.xdim()),
            f64::from(image.ydim()),
            f64::from(image.zdim()),
        );
        let interp = |p: V3| f64::from(image.interpolate(p.x as f32, p.y as f32, p.z as f32));
        let empty = Volume::<i16>::new(0, 0, 0, 1);
        let vol = mask.is_some();
        let mask = mask.unwrap_or(&empty);
        let interp_mask = |p: V3| f64::from(mask.interpolate(p.x as f32, p.y as f32, p.z as f32));
        // Every operation that NaN or infinite voxels reach computes with SSE's NaNs
        // (larmorx_nc_core::sse), in the source's order.
        let in_voxels = |v: V3| {
            V3::new(
                sse::div_f64(v.x, xdim),
                sse::div_f64(v.y, ydim),
                sse::div_f64(v.z, zdim),
            )
        };
        if nb_iter % 50 == 0 {
            let mut l2 = S64(0.0);
            let mut counter = 0i32;
            for i in 0..m.points.len() {
                counter += 1;
                l2 += m.medium_distance_of_neighbours(i);
            }
            self.l = (l2 / f64::from(counter)).0;
        }
        if nb_iter % 100 == 0 {
            for i in 0..m.points.len() {
                let n = m.local_normal(i);
                let point = m.points[i].coord;
                let ipoint = in_voxels(point);
                let inn = in_voxels(n);
                let mut max = 0.0f64;
                let c_m1 = ipoint.add(inn.scale(-1.0));
                let mut current = interp(c_m1);
                let mut i2 = 1.0f64;
                while i2 < 150.0 {
                    if max > 0.1 {
                        break;
                    }
                    let c_p = ipoint.add(inn.scale(i2));
                    let tmpp = interp(c_p);
                    let tmp = ((S64(tmpp) - current) * 100.0).0;
                    max = fmax(max, tmp);
                    current = tmpp;
                    if tmpp > 0.1 {
                        max = 1.0;
                        break;
                    }
                    i2 += 2.0;
                }
                let data = &mut m.points[i].data;
                data.pop();
                data.push(if max < 0.1 { 1.0 } else { 0.0 });
            }
        }
        let l = self.l;
        for i in 0..m.points.len() {
            let n = m.local_normal(i);
            let dv = m.difference_vector(i);
            let tmp = dv.dot(n);
            let sn = n.scale(tmp);
            let st = dv.sub(sn);
            let u1 = st.scale(0.5);
            let rinv = ((2.0 * S64(sn.dot(n)).abs()) / (S64(l) * l)).0;
            let f2 = ((1.0 + S64(fdlibm::tanh((S64(f) * (S64(rinv) - e)).0))) * 0.5).0;
            let u2 = sn.scale(f2).scale(f64::from(addsmooth));
            let point = m.points[i].coord;
            let ipoint = in_voxels(point);
            let mut f3;
            // FSL sets f2 to 0 below once u2 is computed: a store without effect (FSL-57, kept)
            if m.points[i].data.last().copied() == Some(0.0) {
                let inn = in_voxels(n);
                let c_m = ipoint.add(inn.scale(-1.0));
                let c_p = ipoint.add(inn.scale(1.0));
                let tmp = interp(c_p);
                let gradient = sse::sub_f64(tmp, interp(c_m));
                let tmp2 = sse::mul_f64(gradient, 100.0);
                f3 = std_max(-1.0, std_min(tmp2, 1.0));
                if (0.0..0.1).contains(&tmp2) && tmp < 0.1 {
                    f3 = f64::from(speed);
                }
                if vol {
                    let tmpvol = interp_mask(ipoint);
                    if tmpvol > 0.0 {
                        f3 = fmax(fmax(sse::mul_f64(tmpvol, 0.5), 0.1), f3);
                    }
                }
            } else {
                f3 = 0.0;
                let tmpvol = interp_mask(ipoint);
                if tmpvol > 0.0 {
                    f3 = fmax(sse::mul_f64(tmpvol, 0.5), 0.1);
                }
            }
            let u3 = n.scale(sse::mul_f64(0.05, f3));
            let u = u1.add(u2).add(u3);
            m.points[i].update_coord = point.add(u);
        }
        m.update();
    }
}

/// `special_case(point, n, M)`: whether a profile from `point` (mm) along `n` passes near the
/// eyes in MNI152 space.
fn special_case(point: V3, n: V3, m: &TrMatrix) -> bool {
    let mut result = false;
    let mut test2 = false;
    for i in (0..11).step_by(5) {
        let p = point.add(n.scale(f64::from(i)));
        let mut realx = m[0] * p.x + m[1] * p.y + m[2] * p.z + m[3];
        let mut realy = m[4] * p.x + m[5] * p.y + m[6] * p.z + m[7];
        let mut realz = m[8] * p.x + m[9] * p.y + m[10] * p.z + m[11];
        realx -= 88.0;
        realy -= 128.0;
        realz -= 74.0;
        let plan = realy * 0.0117213 + realz * -0.021433355 - 0.6;
        if plan > 0.0 {
            result = true;
        }
        if realy > 62.0 && realx > -16.0 && realx < 16.0 {
            test2 = true;
        }
    }
    if test2 {
        result = false;
    }
    result
}

/// The images profiles are read from: the T1 and, unless `--t1only`, the T2.
struct Images<'a> {
    t1: &'a Volume<f32>,
    t2: Option<&'a Volume<f32>>,
}

/// A profile's estimates, in the order FSL returns them, or why it was dropped.
type Estimates = Vec<f64>;

/// One profile through `image` from `point` (voxels) along `n` (voxels per mm), at the
/// positions `from`, `from + 0.5`, ... below `to`.
fn sample(image: &Volume<f32>, point: V3, n: V3, from: f64, to: f64) -> Profile {
    let mut p = Profile::new();
    let mut d = from;
    while d < to {
        let c = point.add(n.scale(d));
        p.add(
            d,
            f64::from(image.interpolate(c.x as f32, c.y as f32, c.z as f32)),
        );
        d += 0.5;
    }
    p.init_roi();
    p
}

/// The outcome of one profile: its estimates, and whether FSL's search read past the profile
/// (FSL-51) or would end the program (FSL-50).
struct Outcome {
    est: Estimates,
    past_end: bool,
    exits: bool,
}

/// `t1only_special_extract`: profiles that cross the eyes.
fn t1only_special_extract(t1: &Volume<f32>, point: V3, n: V3) -> Outcome {
    const INNER_DEPTH: f64 = 3.0;
    let mut pt1 = sample(t1, point, n, -INNER_DEPTH, 100.0);
    let outskin = pt1.last_point_over(pt1.end(), 0.2);
    let _check = pt1.last_point_over(outskin - 1.5, 0.2);
    pt1.set_rroi(outskin);
    let mut inskull = pt1.next_point_under(-INNER_DEPTH, 0.25);
    if inskull > 5.0 {
        inskull = 0.0;
    }
    let outskull = pt1.next_point_over(inskull, 0.35);
    Outcome {
        est: vec![inskull, outskull, outskin],
        past_end: pt1.past_end,
        exits: false,
    }
}

/// `t1only_co_ext`.
fn t1only_co_ext(t1: &Volume<f32>, point: V3, n: V3) -> Outcome {
    const INNER_DEPTH: f64 = 3.0;
    let mut pt1 = sample(t1, point, n, -INNER_DEPTH, 60.0);
    let mut outskin = pt1.last_point_over(pt1.end(), 0.2);
    let check = pt1.last_point_over(outskin - 1.5, 0.2);
    if outskin - check > 2.0 {
        outskin = check;
    }
    pt1.set_rroi(outskin);
    let b = pt1.begin();
    let inskull = pt1.next_point_under(b, 0.25);
    pt1.set_lroi(inskull);
    let mut output = true;
    let mut outskull2 = pt1.last_point_over(outskin, 0.75);
    outskull2 = pt1.last_point_under(outskull2, 0.20);
    let b = pt1.begin();
    let mut minabs = pt1.next_point_under(b, 0.30);
    if minabs == NOT_FOUND {
        output = false;
    }
    minabs = fmax(minabs, -INNER_DEPTH);
    let mut localminabs = minabs;
    if output {
        let mut stop = false;
        let lowthreshold = pt1.threshold(0.15);
        let upthreshold = pt1.threshold(0.60);
        let mut test = 0i32;
        let len = pt1.v.len();
        for k in 0..len {
            let s = pt1.v[k];
            if !stop && s.abs >= minabs && k != len && k != 0 {
                if s.val > upthreshold {
                    stop = true;
                }
                if s.val > lowthreshold {
                    test += 1;
                }
                if s.val < lowthreshold {
                    test -= 1;
                }
                if test < 0 {
                    test = 0;
                }
                if test == 12 {
                    stop = true;
                }
                if s.val < lowthreshold {
                    localminabs = s.abs;
                }
            }
        }
    }
    let outskull = pt1.next_point_over(localminabs, 0.15);
    if outskull2 - outskull < -2.0 || outskull2 - outskull >= 2.0 {
        output = false;
    }
    if outskin - outskull2 < 2.0 {
        output = false;
    }
    let est = if output {
        vec![inskull, outskull2, outskin]
    } else {
        vec![outskin]
    };
    Outcome {
        est,
        past_end: pt1.past_end,
        exits: false,
    }
}

/// `special_extract` (T1 and T2): profiles that cross the eyes. (FSL also marks the CSF in an
/// image it never writes; that part changes nothing and is left out.)
fn special_extract(t1: &Volume<f32>, t2: &Volume<f32>, point: V3, n: V3) -> Outcome {
    const INNER_DEPTH: f64 = 3.0;
    let mut pt1 = sample(t1, point, n, -30.0, 100.0);
    let mut pt2 = sample(t2, point, n, -30.0, 100.0);
    pt1.set_lroi(-INNER_DEPTH);
    pt2.set_lroi(-INNER_DEPTH);
    let b = pt1.begin();
    let mut inskull = pt1.next_point_under(b, 0.25);
    let inskullt1 = pt2.next_point_under(inskull, 0.30);
    if inskullt1 > inskull {
        inskull = inskullt1;
    }
    if inskull > 15.0 {
        inskull = 0.0;
    }
    let mut alloutput = true;
    let outskin = pt1.last_point_over(pt1.end(), 0.2);
    let check = pt1.last_point_over(outskin - 1.5, 0.2);
    if outskin - check > 2.0 {
        alloutput = false;
    }
    pt1.set_rroi(outskin);
    let val = pt1.next_point_under(inskull, 0.25);
    let mut outskull = pt1.next_point_over(val, 0.35);
    if val - inskull >= 7.0 {
        outskull = inskull + 0.5;
    }
    if outskull - inskull < 0.0 {
        outskull = inskull + 0.5;
    }
    let est = if alloutput {
        vec![outskin, outskull, inskull]
    } else {
        Vec::new()
    };
    Outcome {
        est,
        past_end: pt1.past_end || pt2.past_end,
        exits: false,
    }
}

/// `co_ext` (T1 and T2). (The CSF image FSL computes and never writes is left out.)
fn co_ext(t1: &Volume<f32>, t2: &Volume<f32>, point: V3, n: V3) -> Outcome {
    const INNER_DEPTH: f64 = 3.0;
    let mut pt1 = sample(t1, point, n, -30.0, 60.0);
    let mut pt2 = sample(t2, point, n, -30.0, 60.0);
    pt1.set_lroi(-INNER_DEPTH);
    pt2.set_lroi(-INNER_DEPTH);
    let mut outskin = pt1.last_point_over(pt1.end(), 0.2);
    let check = pt1.last_point_over(outskin - 1.5, 0.2);
    if outskin - check > 2.0 {
        outskin = check;
    }
    let outskin2 = pt2.last_point_over(pt2.end(), 0.25);
    let mut alloutput = true;
    if (outskin - outskin2).abs() > 5.0 {
        // artefact might be present ...
        let b = outskin >= outskin2;
        let m = fmin(outskin, outskin2) + 15.0;
        if b {
            let m2 = pt1.last_point_over(m, 0.2);
            if (m2 - outskin2).abs() > 10.0 {
                alloutput = false;
            } else {
                outskin = m2;
            }
        } else {
            let m2 = pt2.last_point_over(m, 0.25);
            if (m2 - outskin).abs() > 10.0 {
                alloutput = false;
            }
            // else FSL keeps outskin as it is ("outskin = outskin": FSL-49, kept)
        }
    }
    let outer_skin = outskin;
    pt1.set_rroi(outer_skin);
    pt2.set_rroi(outer_skin);
    let mut est = Vec::new();
    let mut exits = false;
    if alloutput {
        let b2 = pt2.begin();
        let mut inskull = pt1.next_point_under(b2, 0.25);
        let inskullt1 = pt2.next_point_under(inskull, 0.30);
        if inskullt1 > inskull {
            inskull = inskullt1;
        }
        pt2.set_lroi(inskull);
        let mut gaps = Vec::new();
        let mut sum = -50.0;
        let mut t = inskull;
        let mut counter = 0;
        let mut step = false;
        while t != NOT_FOUND && counter < 6 {
            if step {
                t = pt2.next_point_under(t, 0.30);
                counter += 1;
                gaps.push(t);
            } else {
                t = pt2.next_point_over(t, 0.40);
                if sum == -50.0 {
                    sum = t;
                }
            }
            step = !step;
        }
        let search_begin = if counter > 1 && (sum - inskull) < 10.0 {
            gaps[0]
        } else {
            inskull + 0.5
        };
        pt1.set_lroi(search_begin);
        let mut outskull2 = pt1.last_point_over(outskin, 0.75);
        outskull2 = pt1.last_point_under(outskull2, 0.30);
        if outskull2 == NOT_FOUND {
            outskull2 = search_begin;
        }
        let b1 = pt1.begin();
        let minabs = pt1.next_point_under(b1, 0.30);
        let mut output = true;
        if minabs == NOT_FOUND {
            // FSL: output = false; exit(-1) (FSL-50)
            output = false;
            exits = true;
        }
        let mut localminabs = minabs;
        if output {
            let mut stop = false;
            let lowthreshold = pt1.threshold(0.15);
            let upthreshold = pt1.threshold(0.25);
            let len = pt1.v.len();
            for k in 0..len {
                let s = pt1.v[k];
                if !stop && s.abs >= minabs && k != len && k != 0 {
                    if s.val > upthreshold {
                        stop = true;
                    }
                    if s.val < lowthreshold {
                        localminabs = s.abs;
                    }
                }
            }
        }
        let outskull = pt1.next_point_over(localminabs, 0.15);
        if outskull2 - outskull >= 3.0 || outskull - outskull2 >= 2.0 {
            output = false;
        }
        if outskin - outskull < 2.0 {
            output = false;
        }
        if outskull - inskull < 0.0 {
            output = false;
        }
        est = if output {
            vec![outskin, outskull, inskull]
        } else {
            vec![outskin]
        };
    }
    Outcome {
        est,
        past_end: pt1.past_end || pt2.past_end,
        exits,
    }
}

/// The three counting images (FSL's `write0`, `write1`, `write2`: inner skull, outer skull,
/// scalp), on the grid padded by `pad_minus` voxels below each axis.
struct Counts {
    inskull: Volume<f32>,
    outskull: Volume<f32>,
    outskin: Volume<f32>,
}

/// `write_ext_skull` / `t1only_write_ext_skull`: every profile's estimates counted into the
/// images.
#[allow(clippy::too_many_arguments)]
fn write_ext_skull(
    counts: &mut Counts,
    img: &Images,
    m: &mut Mesh,
    mat: &TrMatrix,
    pad: [i64; 3],
    verbose: bool,
    out: &mut dyn Write,
    notices: &mut Notices,
) -> Result<(), Fail> {
    let t1 = img.t1;
    let (xdim, ydim, zdim) = (
        f64::from(t1.xdim()),
        f64::from(t1.ydim()),
        f64::from(t1.zdim()),
    );
    let mut meshimage = zeros_like(t1);
    draw_mesh(&mut meshimage, m);
    let mut glob_counter = 0i32;
    let mut rem_counter = 0i32;
    let mut past_end = 0usize;
    let add = |v: &mut Volume<f32>, c: V3| {
        let x = trunc_i((c.x + 0.5).floor()) + pad[0];
        let y = trunc_i((c.y + 0.5).floor()) + pad[1];
        let z = trunc_i((c.z + 0.5).floor()) + pad[2];
        if v.in_bounds(x, y, z) {
            let i = v.idx(x, y, z);
            v.data[i] += 1.0;
        }
    };
    for i in 0..m.points.len() {
        m.points[i].data.clear();
        let normal = m.local_normal(i);
        let n = V3::new(normal.x / xdim, normal.y / ydim, normal.z / zdim);
        let me = m.points[i].coord;
        let mut max_neighbour = 0.0;
        for &nei in &m.points[i].neighbours {
            max_neighbour = fmax(me.sub(m.points[nei].coord).norm(), max_neighbour);
        }
        let max_neighbour = (max_neighbour / 2.0).ceil();
        let mp = V3::new(me.x / xdim, me.y / ydim, me.z / zdim);
        let range = |c: f64, d: f64| {
            trunc_i((c - max_neighbour / d).floor())..=trunc_i((c + max_neighbour / d).floor())
        };
        for ck in range(mp.z, zdim) {
            for cj in range(mp.y, ydim) {
                for ci in range(mp.x, xdim) {
                    let point = V3::new(ci as f64, cj as f64, ck as f64);
                    let realpoint = V3::new(ci as f64 * xdim, cj as f64 * ydim, ck as f64 * zdim);
                    if meshimage.get(ci, cj, ck) != 1 {
                        continue;
                    }
                    let mut mindist = 10000.0;
                    for &nei in &m.points[i].neighbours {
                        mindist = fmin(realpoint.sub(m.points[nei].coord).norm(), mindist);
                    }
                    // FSL computes the profile when mindist >= the distance (not for NaN)
                    let compute = mindist >= realpoint.sub(me).norm();
                    if !compute {
                        continue;
                    }
                    glob_counter += 1;
                    let special = special_case(realpoint, normal, mat);
                    let mut o = match (img.t2, special) {
                        (None, false) => t1only_co_ext(t1, point, n),
                        (None, true) => t1only_special_extract(t1, point, n),
                        (Some(t2), false) => co_ext(t1, t2, point, n),
                        (Some(t2), true) => special_extract(t1, t2, point, n),
                    };
                    if o.exits {
                        let what = format!(
                            "the profile from voxel {ci} {cj} {ck} has no point below its 30% \
                             threshold (an empty search range)"
                        );
                        if notices.reproduce() {
                            notices.add(
                                &defects::FSL_50,
                                format!("{what}: betsurf stops with status 255, as FSL's does"),
                            );
                            return Err(Fail::Exit(255));
                        }
                        notices.add(
                            &defects::FSL_50,
                            format!("{what}: the profile is dropped, where FSL's betsurf stops"),
                        );
                    }
                    if o.past_end {
                        past_end += 1;
                        if notices.fix() {
                            o.est.clear();
                        }
                    }
                    if o.est.len() == 3 {
                        let (a, b, c) = (
                            point.add(n.scale(o.est[0])),
                            point.add(n.scale(o.est[1])),
                            point.add(n.scale(o.est[2])),
                        );
                        if img.t2.is_some() {
                            add(&mut counts.outskin, a);
                            add(&mut counts.outskull, b);
                            add(&mut counts.inskull, c);
                        } else {
                            add(&mut counts.inskull, a);
                            add(&mut counts.outskull, b);
                            add(&mut counts.outskin, c);
                        }
                    } else {
                        rem_counter += 1;
                        if o.est.len() == 1 {
                            add(&mut counts.outskin, point.add(n.scale(o.est[0])));
                        }
                    }
                }
            }
        }
    }
    if past_end > 0 {
        let what = format!("{past_end} profile search(es) read past the profile's last sample");
        if notices.fix() {
            notices.add(
                &defects::FSL_51,
                format!("{what}: those profiles are dropped"),
            );
        } else {
            notices.add(
                &defects::FSL_51,
                format!(
                    "{what}, where FSL reads memory it never set: larmorx reads nothing there \
                     (the results can differ from FSL's)"
                ),
            );
        }
    }
    if verbose {
        let _ = writeln!(out, " nb of profiles : {glob_counter}");
        let removed = if glob_counter == 0 {
            "-nan".to_owned()
        } else {
            cnum::stream(100.0 * f64::from(rem_counter) / f64::from(glob_counter))
        };
        let _ = writeln!(out, " removed profiles : {removed}%");
    }
    Ok(())
}

/// `(int) ceil(x)`.
fn ceil_i(x: f64) -> i32 {
    cnum::f64_to_i32(x.ceil())
}

/// Saves a short image (`save_volume(v, name) < 0` returns -1 from main).
fn save(
    v: &Volume<i16>,
    name: &str,
    run: &Run,
    err: &mut dyn Write,
    notices: &mut Notices,
) -> Result<(), Fail> {
    if volume::offset_defect_applies(v, run.fsl) {
        crate::bet2::offset_notice(notices, v.extensions.len());
    }
    let opts =
        SaveOptions::new(volume::BUILDSTRING.as_bytes()).reproducing_offset(run.mode.reproduce());
    match v.save_volume(name, run.fsl, &opts) {
        Ok(Saved::Written) => Ok(()),
        Ok(Saved::Nothing) => Err(Fail::Exit(255)),
        Err(SaveError::OutputType(o)) => {
            for l in &o.printed {
                let _ = writeln!(err, "{l}");
            }
            Err(Fail::Exit(1))
        }
        Err(SaveError::Exception(e)) => Err(Fail::Abort(e)),
    }
}

fn save_mesh(m: &Mesh, name: &str, err: &mut dyn Write) {
    let mut msgs = String::new();
    m.save_vtk(name, &mut msgs);
    let _ = err.write_all(msgs.as_bytes());
}

/// The run: `main`'s T1+T2 path, or `t1only_main`.
fn surfaces(
    names: &Names,
    a: &BetsurfArgs,
    run: &Run,
    out: &mut dyn Write,
    err: &mut dyn Write,
    notices: &mut Notices,
) -> Result<(), Fail> {
    let say = |out: &mut dyn Write, s: &str| {
        if a.verbose {
            let _ = writeln!(out, "{s}");
        }
    };
    let mut stdin = std::io::stdin().lock();
    let mut m = match load::load(&names.mesh, out, &mut stdin, notices.reproduce()) {
        Ok((m, fills)) => {
            if !fills.is_empty() {
                let shown: Vec<&str> = fills.iter().take(3).map(String::as_str).collect();
                let more = if fills.len() > 3 {
                    format!(" (and {} more)", fills.len() - 3)
                } else {
                    String::new()
                };
                notices.add(
                    &defects::FSL_52,
                    format!(
                        "the mesh file {} leaves values undefined in FSL, filled: {}{more}",
                        names.mesh,
                        shown.join("; ")
                    ),
                );
            }
            m
        }
        Err(LoadError::Exit { printed, code }) => {
            let _ = err.write_all(printed.as_bytes());
            return Err(Fail::Exit(code));
        }
        Err(LoadError::Refused(why)) => {
            if why.contains("FSL-52") {
                notices.add(
                    &defects::FSL_52,
                    format!("the mesh file {}: refused ({why})", names.mesh),
                );
            }
            return Err(Fail::Refused(why));
        }
    };
    let mat = read_matrix(&names.matrix, err, notices)?;
    let mut t1 = read_t(&names.t1, run.fsl, err)?;
    let t2 = match &names.t2 {
        Some(n) => Some(read_t(n, run.fsl, err)?),
        None => None,
    };
    if let Some(t2) = &t2 {
        let same = t2.dims[..3] == t1.dims[..3]
            && t2.xdim() == t1.xdim()
            && t2.ydim() == t1.ydim()
            && t2.zdim() == t1.zdim();
        if !same {
            let what = format!(
                "the T2 ({} x {} x {} voxels of {} x {} x {} mm) is not on the T1's grid ({} x {} \
                 x {} of {} x {} x {} mm)",
                t2.dims[0],
                t2.dims[1],
                t2.dims[2],
                cnum::stream(f64::from(t2.xdim())),
                cnum::stream(f64::from(t2.ydim())),
                cnum::stream(f64::from(t2.zdim())),
                t1.dims[0],
                t1.dims[1],
                t1.dims[2],
                cnum::stream(f64::from(t1.xdim())),
                cnum::stream(f64::from(t1.ydim())),
                cnum::stream(f64::from(t1.zdim()))
            );
            if notices.fix() {
                notices.add(&defects::FSL_48, format!("{what}: refused"));
                return Err(Fail::Refused(format!(
                    "{what}; register it to the T1 first (FSL-48)"
                )));
            }
            notices.add(
                &defects::FSL_48,
                format!("{what}: sampled at the T1's voxel coordinates, as FSL does"),
            );
        }
    }
    let (xdim, ydim, zdim) = (
        f64::from(t1.xdim()),
        f64::from(t1.ydim()),
        f64::from(t1.zdim()),
    );
    let (xsize, ysize, zsize) = (t1.xsize(), t1.ysize(), t1.zsize());

    // the brain's robust maximum (useful if the skull is too bright)
    let thr = {
        let mut brain_mask = zeros_like(&t1);
        draw_mesh(&mut brain_mask, &m);
        let brain_mask = make_mask_from_mesh(&brain_mask, &m, notices)?;
        let mut hist: Vec<f64> = Vec::new();
        for (k, &b) in brain_mask.data.iter().enumerate() {
            if b == 1 {
                hist.push(f64::from(t1.data[k]));
            }
        }
        let size = hist.len() as i32;
        let e98 = ceil_i(0.98 * f64::from(size));
        let nth = (e98 - 1) as usize;
        if hist.iter().any(|v| v.is_nan()) {
            let what = "the T1 holds NaN inside the brain surface, which makes FSL's 98th \
                        percentile undefined";
            if notices.fix() {
                notices.add(&defects::FSL_55, format!("{what}: refused"));
                return Err(Fail::Refused(format!("{what} (FSL-55)")));
            }
            let outside = stdsort::nth_element_f64(&mut hist, nth);
            notices.add(
                &defects::FSL_55,
                if outside {
                    format!(
                        "{what}: libstdc++'s selection, which reads outside the values here; \
                         larmorx stops at their ends"
                    )
                } else {
                    format!("{what}: libstdc++'s selection, as FSL runs it")
                },
            );
        } else {
            hist.select_nth_unstable_by(nth, f64::total_cmp);
        }
        hist[nth] * 1.1
    };
    for v in t1.data.iter_mut() {
        if f64::from(*v) > thr {
            *v = thr as f32;
        }
    }

    // the padding that keeps the mesh 3 voxels inside the working grid
    let (mut infxm, mut infym, mut infzm) = (0i32, 0i32, 0i32);
    let (mut infxp, mut infyp, mut infzp) = (0i32, 0i32, 0i32);
    for p in &m.points {
        let (px, py, pz) = (p.coord.x / xdim, p.coord.y / ydim, p.coord.z / zdim);
        if px < 3.0 {
            infxm = infxm.max(ceil_i(3.0 - px));
        }
        if py < 3.0 {
            infym = infym.max(ceil_i(3.0 - py));
        }
        if pz < 3.0 {
            infzm = infzm.max(ceil_i(3.0 - pz));
        }
        if xsize as f64 - px < 3.0 {
            infxp = infxp.max(ceil_i(f64::from(3 - xsize as i32) + px));
        }
        if ysize as f64 - py < 3.0 {
            infyp = infyp.max(ceil_i(f64::from(3 - ysize as i32) + py));
        }
        if zsize as f64 - pz < 3.0 {
            infzp = infzp.max(ceil_i(f64::from(3 - zsize as i32) + pz));
        }
    }
    let extra = if a.t1only { 1 } else { 2 };
    let (infxm, infym, infzm) = (infxm + extra, infym + extra, infzm + extra);
    let (infxp, infyp, infzp) = (infxp + extra, infyp + extra, infzp + extra);
    let mut write0 = Volume::<f32>::new(
        xsize + i64::from(infxm) + i64::from(infxp),
        ysize + i64::from(infym) + i64::from(infyp),
        zsize + i64::from(infzm) + i64::from(infzp),
        1,
    );
    write0.pixdims[0] = t1.xdim();
    write0.pixdims[1] = t1.ydim();
    write0.pixdims[2] = t1.zdim();
    let mut counts = Counts {
        inskull: write0.clone(),
        outskull: write0.clone(),
        outskin: write0,
    };

    say(out, "extracting profiles");
    let images = Images {
        t1: &t1,
        t2: t2.as_ref(),
    };
    write_ext_skull(
        &mut counts,
        &images,
        &mut m,
        &mat,
        [i64::from(infxm), i64::from(infym), i64::from(infzm)],
        a.verbose,
        out,
        notices,
    )?;

    say(out, "blurring");
    const BLURRING: f32 = 3.0;
    const SMOOTHNESS: f32 = 1.0;
    let write0 = newimagefns::smooth(&counts.inskull, BLURRING, run.threads);
    let write2 = newimagefns::smooth(&counts.outskin, BLURRING, run.threads);
    let mut write1 = counts.outskull;
    say(out, "computation");
    for p in &mut m.points {
        p.data.push(0.0);
    }
    let shift = (
        xdim * f64::from(infxm),
        ydim * f64::from(infym),
        zdim * f64::from(infzm),
    );
    m.translation(shift.0, shift.1, shift.2);
    let mut deform = Deform { l: 0.0 };

    // inner skull
    say(out, " inner skull");
    for c in 0..800 {
        deform.step(&write0, &mut m, SMOOTHNESS, 0.5, c, None);
    }
    let mut mprecise = Mesh::default();
    if a.precision > 0 {
        mprecise.assign(&m);
        say(out, "  increased precision");
        for _ in 0..a.precision {
            mprecise.retessellate();
        }
        for c in 0..100 {
            deform.step(&write0, &mut mprecise, SMOOTHNESS, 0.5, c, None);
        }
    }
    let mut realmesh = Mesh::default();
    realmesh.assign(if a.precision == 0 { &m } else { &mprecise });
    realmesh.translation(-shift.0, -shift.1, -shift.2);
    save_mesh(
        &realmesh,
        &format!("{}_inskull_mesh.vtk", names.output),
        err,
    );

    let mut maskinskull2: Option<Volume<i16>> = None;
    let mut meshinskull: Option<Volume<i16>> = None;
    let maskinskull = {
        let mut base: Volume<i16> = Volume::copyconvert(&write0);
        draw_mesh(&mut base, &m);
        let maskinskull = make_mask_from_mesh(&base, &m, notices)?;
        let mut output1 = zeros_like(&t1);
        if a.outline || a.mask || a.skullmask {
            draw_mesh(&mut output1, &realmesh);
        }
        if a.skullmask && !a.t1only {
            meshinskull = Some(output1.clone());
        }
        if a.outline {
            say(out, "  outline");
            save(
                &output1,
                &format!("{}_inskull_mesh", names.output),
                run,
                err,
                notices,
            )?;
        }
        if a.mask || a.skullmask {
            let smask = make_mask_from_mesh(&output1, &realmesh, notices)?;
            if a.mask {
                say(out, "  mask");
                save(
                    &smask,
                    &format!("{}_inskull_mask", names.output),
                    run,
                    err,
                    notices,
                )?;
            }
            if a.skullmask {
                maskinskull2 = Some(smask);
            }
        }
        maskinskull
    };
    for (k, w) in write1.data.iter_mut().enumerate() {
        if *w > 0.0 && maskinskull.data[k] == 1 {
            *w = 0.0;
        }
    }
    let write1 = newimagefns::smooth(&write1, BLURRING, run.threads);

    // outer skull
    say(out, " outer skull");
    for c in 0..800 {
        deform.step(&write1, &mut m, SMOOTHNESS, 0.5, c, Some(&maskinskull));
    }
    drop(maskinskull);
    if a.precision > 0 {
        say(out, "  increased precision");
        mprecise.assign(&m);
        for _ in 0..a.precision {
            mprecise.retessellate();
        }
        for c in 0..100 {
            deform.step(&write1, &mut mprecise, SMOOTHNESS, 0.5, c, None);
        }
    }
    drop(write1);
    let mut realmesh2 = Mesh::default();
    realmesh2.assign(if a.precision == 0 { &m } else { &mprecise });
    realmesh2.translation(-shift.0, -shift.1, -shift.2);
    save_mesh(
        &realmesh2,
        &format!("{}_outskull_mesh.vtk", names.output),
        err,
    );

    let mut maskoutskull2: Option<Volume<i16>> = None;
    let mut meshoutskull: Option<Volume<i16>> = None;
    let maskoutskull = {
        let mut base: Volume<i16> = Volume::copyconvert(&write0);
        draw_mesh(&mut base, &m);
        let maskoutskull = make_mask_from_mesh(&base, &m, notices)?;
        let mut output1 = zeros_like(&t1);
        if a.mask || a.outline || a.skullmask {
            draw_mesh(&mut output1, &realmesh2);
        }
        if a.skullmask && a.t1only {
            meshoutskull = Some(output1.clone());
        }
        if a.outline {
            say(out, "  outline");
            save(
                &output1,
                &format!("{}_outskull_mesh", names.output),
                run,
                err,
                notices,
            )?;
        }
        if a.mask || a.skullmask {
            let smask = make_mask_from_mesh(&output1, &realmesh2, notices)?;
            if a.mask {
                say(out, "  mask");
                save(
                    &smask,
                    &format!("{}_outskull_mask", names.output),
                    run,
                    err,
                    notices,
                )?;
            }
            if a.skullmask {
                maskoutskull2 = Some(smask);
            }
        }
        maskoutskull
    };
    drop(write0);

    if a.skullmask {
        let o2 = maskoutskull2.as_ref().expect("set with -s");
        let i2 = maskinskull2.as_ref().expect("set with -s");
        let edge = if a.t1only {
            meshoutskull.as_ref()
        } else {
            meshinskull.as_ref()
        }
        .expect("set with -s");
        let mut smask = o2.clone();
        say(out, " skull mask");
        for k in 0..smask.data.len() {
            let v = i32::from(o2.data[k]) * (1 - i32::from(i2.data[k])) + i32::from(edge.data[k]);
            smask.data[k] = v.min(1) as i16;
        }
        save(
            &smask,
            &format!("{}_skull_mask", names.output),
            run,
            err,
            notices,
        )?;
    }

    // the scalp, from a sphere inside the outer skull
    say(out, " skin");
    let mut m2 = Mesh::icosa(5);
    for p in &mut m2.points {
        p.data.push(0.0);
    }
    let mut p = V3::default();
    let mut counter = 0i32;
    for q in &m.points {
        counter += 1;
        p = V3::new(p.x + q.coord.x, p.y + q.coord.y, p.z + q.coord.z);
    }
    let inv = 1.0 / f64::from(counter);
    p = V3::new(p.x * inv, p.y * inv, p.z * inv);
    let mut radius = 0.0;
    for q in &m.points {
        radius += q.coord.sub(p).norm();
    }
    radius /= f64::from(counter);
    radius *= 0.75;
    m2.rescale(radius, V3::default());
    m2.translation(p.x, p.y, p.z);
    for c in 0..1500 {
        deform.step(&write2, &mut m2, SMOOTHNESS, 1.5, c, Some(&maskoutskull));
    }
    drop(maskoutskull);
    if a.precision > 0 {
        say(out, "  increased precision");
        for _ in 0..a.precision {
            m2.retessellate();
        }
        for c in 0..100 {
            deform.step(&write2, &mut m2, SMOOTHNESS, 1.5, c, None);
        }
    }
    drop(write2);
    m2.translation(-shift.0, -shift.1, -shift.2);
    save_mesh(&m2, &format!("{}_outskin_mesh.vtk", names.output), err);

    if a.outline || a.mask {
        let mut output1 = zeros_like(&t1);
        draw_mesh(&mut output1, &m2);
        if a.outline {
            say(out, "  outline");
            save(
                &output1,
                &format!("{}_outskin_mesh", names.output),
                run,
                err,
                notices,
            )?;
        }
        if a.mask {
            say(out, "  mask");
            let mask = if a.t1only {
                // FSL-47: FSL fills from the outer skull surface's centre, in the padded grid
                let wrong = fill_start(&output1, &m);
                let right = fill_start(&output1, &m2);
                let fix = notices.fix();
                let theirs = fill_quiet(&output1, wrong, fix);
                let ours = fill_quiet(&output1, right, fix);
                if theirs.data != ours.data {
                    notices.add(
                        &defects::FSL_47,
                        if fix {
                            "the scalp mask is filled from the scalp surface's centre"
                        } else {
                            "the scalp mask is filled from the outer skull surface's centre in \
                             the padded grid, as FSL does"
                        },
                    );
                }
                let start = if fix { right } else { wrong };
                // the fill again, with its notice or refusal (FSL-43)
                let chosen = fill_from(&output1, start, notices)?;
                let _ = (ours, theirs);
                chosen
            } else {
                make_mask_from_mesh(&output1, &m2, notices)?
            };
            save(
                &mask,
                &format!("{}_outskin_mask", names.output),
                run,
                err,
                notices,
            )?;
        }
    }
    Ok(())
}
