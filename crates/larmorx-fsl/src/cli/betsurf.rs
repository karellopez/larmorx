// SPDX-License-Identifier: Apache-2.0
//! `larmorx fsl betsurf`: FSL's `betsurf` command line (`specs/bet.md` §6, §7, §9).

use std::io::{BufRead, Write};

use larmorx_core::defects::{self, Notice, UpstreamDefects};

use super::Settings;
use super::bet2::{FSLOUTPUTTYPE_UNSET, print_notices, read_failure_of, read_image, write_vtk};
use super::fmt::g;
use super::options::{self, Given, ParseError, Spec};
use crate::bet::image::{self, Input, Output, Values};
use crate::bet::mesh::{Mesh, V3};
use crate::bet::raster::{self, Rounding};
use crate::bet::{Vol, pooled};
use crate::betsurf::{self as bs, WorkGrid, meshfile};

/// betsurf's usage, exactly (standard error).
pub const USAGE: &str = "\nPart of FSL (ID: 2412.6-dirty)\nBETSURF (BET Surface Finder) v2.1 - FMRIB Analysis Group, Oxford\n\nUsage: \n betsurf          [options] <t1> <t2> <bet_mesh.vtk> <t1_to_standard.mat> <output>\n betsurf --t1only [options] <t1>      <bet_mesh.vtk> <t1_to_standard.mat> <output>\n\nOptional arguments (You may optionally specify one or more of):\n\t-h,--help\tdisplays this help, then exits\n\t-v,--verbose\tswitch on diagnostic messages\n\t-1,--t1only\textraction with t1 only\n\t-o,--outline\tgenerates all surface outlines\n\t-m,--mask\tgenerates binary masks from the meshes\n\t-s,--skullmask\tgenerates skull binary mask\n\t-p,--increased_precision <int>\tretessellates the meshes the indicated number of times\n\n\n";

const fn flag(short: char, long: &'static str) -> Spec {
    Spec {
        short: Some(short),
        long: Some(long),
        values: 0,
        numeric: false,
        integer: false,
    }
}

const HELP: usize = 0;
const VERBOSE: usize = 1;
const T1ONLY: usize = 2;
const OUTLINE: usize = 3;
const MASK: usize = 4;
const SKULL: usize = 5;
const PRECISION: usize = 6;

const SPECS: [Spec; 7] = [
    flag('h', "help"),
    flag('v', "verbose"),
    flag('1', "t1only"),
    flag('o', "outline"),
    flag('m', "mask"),
    flag('s', "skullmask"),
    // an integer, read as C's strtol of base 0 on the whole word (observed: `0x10` and `-3`
    // read, `08`, `1.5` and `1e3` do not)
    Spec {
        short: Some('p'),
        long: Some("increased_precision"),
        values: 1,
        numeric: true,
        integer: true,
    },
];

/// The most retessellations larmorx runs: each one makes four times the triangles, and from 11
/// on the vertex numbers would not fit the mesh's 32-bit indices (FSL runs out of memory far
/// sooner).
pub const MAX_PRECISION: i64 = 10;

/// betsurf's options as given.
#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    pub verbose: bool,
    pub t1only: bool,
    pub outline: bool,
    pub mask: bool,
    pub skull: bool,
    pub precision: i64,
    pub t1: String,
    pub t2: Option<String>,
    pub mesh: String,
    pub matrix: String,
    pub output: String,
}

/// What [`parse`] decided.
pub enum Parsed {
    Run(Box<Options>),
    /// Stop with this status (the messages written); `t1only` for FSL-54.
    Exit(u8),
    /// Too few arguments: `too few arguments` and the usage written; FSL exits 0 with
    /// `--t1only` (FSL-54).
    TooFew {
        t1only: bool,
    },
}

/// Parses betsurf's command line (§6.1), printing FSL's messages.
pub fn parse(args: &[String], err: &mut dyn Write) -> Parsed {
    let words: Vec<String> = args.to_vec();
    let (given, used) = match parse_words(&words) {
        Ok(g) => g,
        Err(e) => {
            let _ = write!(err, "{USAGE}\n{}\n", e.0);
            return Parsed::Exit(1);
        }
    };
    let precision = match &given[PRECISION] {
        Some(Given::Numbers(v)) => v[0] as i64,
        _ => 0,
    };
    if given[HELP].is_some() {
        let _ = err.write_all(USAGE.as_bytes());
        return Parsed::Exit(0);
    }
    let rest = &words[used..];
    if rest.iter().any(|w| w.starts_with('-')) {
        let _ = err.write_all(USAGE.as_bytes());
        return Parsed::Exit(255);
    }
    let t1only = given[T1ONLY].is_some();
    let need = if t1only { 4 } else { 5 };
    if rest.len() < need {
        let _ = write!(err, "too few arguments\n{USAGE}");
        return Parsed::TooFew { t1only };
    }
    let (t1, t2, mesh, matrix, output) = if t1only {
        (
            rest[0].clone(),
            None,
            rest[1].clone(),
            rest[2].clone(),
            rest[3].clone(),
        )
    } else {
        (
            rest[0].clone(),
            Some(rest[1].clone()),
            rest[2].clone(),
            rest[3].clone(),
            rest[4].clone(),
        )
    };
    Parsed::Run(Box::new(Options {
        verbose: given[VERBOSE].is_some(),
        t1only,
        outline: given[OUTLINE].is_some(),
        mask: given[MASK].is_some(),
        skull: given[SKULL].is_some(),
        precision,
        t1,
        t2,
        mesh,
        matrix,
        output,
    }))
}

/// The options before the first word not starting with `-` (a lone `-` consumed as an empty
/// run); how many words they took.
fn parse_words(words: &[String]) -> Result<(Vec<Option<Given>>, usize), ParseError> {
    let mut given: Vec<Option<Given>> = vec![None; SPECS.len()];
    let mut i = 0;
    while i < words.len() {
        if words[i] == "-" {
            i += 1;
            continue;
        }
        let (g, used) = options::parse(&words[i..], &SPECS, true)?;
        for (k, v) in g.into_iter().enumerate() {
            if v.is_some() {
                given[k] = v;
            }
        }
        i += used;
        if words.get(i).is_some_and(|w| w == "-") {
            continue;
        }
        break;
    }
    Ok((given, i))
}

/// A refusal of larmorx's own, after the notices.
fn refuse(err: &mut dyn Write, notices: &[Notice], message: &str) -> u8 {
    print_notices(err, notices);
    let _ = writeln!(err, "larmorx fsl betsurf: {message} (larmorx)");
    1
}

/// Runs betsurf.
pub fn main(args: &[String], settings: &Settings, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let parsed = parse(args, err);
    let mode = match settings.defects() {
        Ok(m) => m,
        Err(e) => {
            let _ = writeln!(err, "larmorx fsl betsurf: {e} (larmorx)");
            return 1;
        }
    };
    match parsed {
        Parsed::Run(o) => run(&o, settings, mode, out, err),
        Parsed::Exit(code) => code,
        Parsed::TooFew { t1only } => fsl54(err, t1only, mode, "too few arguments"),
    }
}

/// FSL-54: `--t1only` exits 0 also when it fails.
fn fsl54(err: &mut dyn Write, t1only: bool, mode: UpstreamDefects, what: &str) -> u8 {
    if !t1only {
        return 255;
    }
    let n = Notice::new(
        &defects::FSL_54,
        mode,
        match mode {
            UpstreamDefects::Fix => format!("{what}: exit status 255; FSL exits 0 with --t1only"),
            UpstreamDefects::Reproduce => {
                format!("{what}: exit status 0 with --t1only, as FSL")
            }
        },
    );
    print_notices(err, &[n]);
    if mode.reproduce() { 0 } else { 255 }
}

/// The brain mesh's file: its text, or the cancelled mesh (`c`), or FSL's failure.
enum MeshText {
    Text(String),
    Cancelled,
}

fn mesh_text(
    name: &str,
    settings: &Settings,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<MeshText, u8> {
    if name == "c" {
        let _ = writeln!(out, "cancelled");
        return Ok(MeshText::Cancelled);
    }
    match std::fs::read(settings.path(name)) {
        Ok(bytes) => Ok(MeshText::Text(String::from_utf8_lossy(&bytes).into_owned())),
        Err(_) => {
            let _ = write!(
                err,
                "error opening file\ncould not open the following file: {name}\nterminating.\n"
            );
            Err(255)
        }
    }
}

/// The 4D warning of FSL's image layer.
const TRUNCATED: &str = "Warning: An input intended to be a single 3D volume has multiple timepoints. Input will be truncated to first volume, but this functionality is deprecated and will be removed in a future release.";

/// Runs betsurf with parsed options.
pub fn run(
    o: &Options,
    settings: &Settings,
    mode: UpstreamDefects,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    // `manual_input`: the mesh's name asked for on standard input first (§6.2)
    let mut asked = o.clone();
    if o.mesh == "manual_input" {
        let _ = write!(out, "loading mesh : enter file name / c to cancel: ");
        let _ = out.flush();
        let mut line = String::new();
        match std::io::stdin().lock().read_line(&mut line) {
            Ok(n) if n > 0 => {
                if line.ends_with('\n') {
                    line.pop();
                }
                asked.mesh = line;
            }
            _ => {
                let _ = writeln!(
                    err,
                    "larmorx fsl betsurf: no mesh file name on standard input; FSL waits for one forever (larmorx)"
                );
                return 1;
            }
        }
    }
    run_in(&asked, settings, mode, out, err)
}

fn run_in(
    o: &Options,
    settings: &Settings,
    mode: UpstreamDefects,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let n_threads = settings.threads();
    let mut notices: Vec<Notice> = Vec::new();
    if o.precision > MAX_PRECISION {
        return refuse(
            err,
            &[],
            &format!(
                "-p {} retessellates the meshes more than {MAX_PRECISION} times: they would hold more vertices than any memory (each time four times the triangles)",
                o.precision
            ),
        );
    }
    let precision = o.precision.max(0) as usize;
    // the mesh
    let text = match mesh_text(&o.mesh, settings, out, err) {
        Ok(t) => t,
        Err(code) => return code,
    };
    let read = match text {
        MeshText::Text(t) => meshfile::read(&t),
        MeshText::Cancelled => meshfile::Read {
            problems: vec!["the mesh was cancelled ('c'): it has no vertices".into()],
            ..meshfile::Read::default()
        },
    };
    if read.points.is_empty() {
        let n = Notice::new(
            &defects::FSL_52,
            mode,
            "the brain mesh has no vertices: refused (FSL computes with its centre, which is not a number, and crashes)",
        );
        return refuse(err, &[n], "the brain mesh has no vertices (FSL-52)");
    }
    if !read.problems.is_empty() {
        let what = read.problems.join("; ");
        let n = Notice::new(
            &defects::FSL_52,
            mode,
            match mode {
                UpstreamDefects::Fix => format!("{}: {what}; refused", o.mesh),
                UpstreamDefects::Reproduce => format!(
                    "{}: {what}; the values FSL never sets taken as 0, the faces naming missing vertices left out",
                    o.mesh
                ),
            },
        );
        if mode.fix() {
            return refuse(
                err,
                &[n],
                &format!("the brain mesh {} does not read: {what} (FSL-52)", o.mesh),
            );
        }
        notices.push(n);
    }
    let brain = Mesh::new(read.points, read.triangles);
    // the matrix
    let matrix = match std::fs::read(settings.path(&o.matrix)) {
        Ok(bytes) => {
            let (m, count) = bs::read_matrix(&String::from_utf8_lossy(&bytes));
            if count < 12 {
                let n = Notice::new(
                    &defects::FSL_53,
                    mode,
                    match mode {
                        UpstreamDefects::Fix => format!(
                            "{} holds {count} readable numbers of the 12 betsurf uses: refused",
                            o.matrix
                        ),
                        UpstreamDefects::Reproduce => format!(
                            "{} holds {count} readable numbers of the 12 betsurf uses: the others taken as 0",
                            o.matrix
                        ),
                    },
                );
                if mode.fix() {
                    return refuse(
                        err,
                        &[n],
                        &format!(
                            "the matrix {} holds fewer than 12 numbers (FSL-53)",
                            o.matrix
                        ),
                    );
                }
                notices.push(n);
            }
            m
        }
        Err(_) => {
            print_notices(err, &notices);
            let _ = writeln!(err, "unable to open {}", o.matrix);
            return fsl54(err, o.t1only, mode, "a matrix file that cannot be opened");
        }
    };
    // the images
    let mut t1 = match read_image(settings, &o.t1, mode, &mut notices, n_threads) {
        Ok(i) => i,
        Err(e) => {
            print_notices(err, &notices);
            return read_failure_of("betsurf", err, &o.t1, &e);
        }
    };
    if t1.volumes > 1 {
        let _ = writeln!(err, "{TRUNCATED}");
    }
    let mut t2: Option<Input> = match &o.t2 {
        None => None,
        Some(name) => match read_image(settings, name, mode, &mut notices, n_threads) {
            Ok(i) => {
                if i.volumes > 1 {
                    let _ = writeln!(err, "{TRUNCATED}");
                }
                Some(i)
            }
            Err(e) => {
                print_notices(err, &notices);
                return read_failure_of("betsurf", err, name, &e);
            }
        },
    };
    if let Some(t2) = &t2
        && (t2.vol.shape != t1.vol.shape
            || (0..3).any(|a| (t2.vol.voxel[a] as f32) != (t1.vol.voxel[a] as f32)))
    {
        let n = Notice::new(
            &defects::FSL_48,
            mode,
            match mode {
                UpstreamDefects::Fix => format!(
                    "the T2's grid ({:?} voxels of {:?} mm) is not the T1's ({:?} of {:?}): refused",
                    t2.vol.shape, t2.vol.voxel, t1.vol.shape, t1.vol.voxel
                ),
                UpstreamDefects::Reproduce => {
                    "the T2's grid is not the T1's: it is sampled at the T1's voxel coordinates, as FSL does".into()
                }
            },
        );
        if mode.fix() {
            return refuse(
                err,
                &[n],
                "the T2 is not on the T1's grid (FSL-48): register it to the T1 first",
            );
        }
        notices.push(n);
    }
    let shape = t1.vol.shape;
    let voxel = t1.vol.voxel;
    let n_vox = shape.iter().product::<usize>();
    // §6.3: the robust maximum inside the brain surface
    let pts = brain.points.clone();
    let inside = raster_count(&brain, &pts, shape, voxel);
    let mask0 = bs::surface_mask(
        &brain,
        &pts,
        shape,
        voxel,
        Rounding::Floor,
        vec![0.0; n_vox],
        None,
        mode,
        &mut notices,
    );
    let thr = if inside == 0 {
        let n = Notice::new(
            &defects::FSL_58,
            mode,
            match mode {
                UpstreamDefects::Fix => {
                    "the brain surface has no voxel in the T1's image: refused (FSL reads the 98th percentile from an empty list)"
                }
                UpstreamDefects::Reproduce => {
                    "the brain surface has no voxel in the T1's image: the 98th percentile taken as 0 (FSL reads it from an empty list)"
                }
            },
        );
        if mode.fix() {
            return refuse(
                err,
                &[n],
                "the brain surface lies outside the T1's image (FSL-58)",
            );
        }
        notices.push(n);
        0.0
    } else {
        match bs::robust_maximum(&t1.vol, &mask0, mode, &mut notices) {
            Ok(t) => t,
            Err(bs::Refusal::Defect(n)) => {
                let id = n.defect.id;
                return refuse(err, &[n], &format!("the T1 cannot be used ({id})"));
            }
        }
    };
    drop(mask0);
    // §6.4: the profiles, on the T1 clamped in place (no copy of it is held)
    let grid = WorkGrid::new(&pts, &t1.vol, o.t1only);
    let empty = Vol::new([0, 0, 0], voxel, Vec::new());
    let t1c = in_pool(n_threads, std::mem::replace(&mut t1.vol, empty), |mut v| {
        bs::clamp(&mut v, thr);
        v
    });
    if o.verbose {
        let _ = writeln!(out, "extracting profiles");
    }
    let votes = pooled(n_threads, || {
        bs::votes(
            &brain,
            &pts,
            &t1c,
            t2.as_ref().map(|i| &i.vol),
            &matrix,
            &grid,
            mode.fix(),
            // FSL-50 (FSL ends at a profile whose outer-skull search range is empty,
            // specs/bet.md §6.4 step 7) is not reproduced: on every image where such a profile
            // was seen (43 images of the random-testing trials of 2026-10-31 and 2026-11-04, one
            // with FSL's own failure at its output type) FSL 6.0.7.23 runs on and counts only the
            // profile's scalp, as the default does; when FSL ends is a question for the spec
            // writer (docs/findings/fsl-bet-original.md)
            false,
        )
    });
    // the images are not needed any more (the outputs take the T1's header only)
    drop(t1c);
    if let Some(t2) = t2.as_mut() {
        t2.vol.data = Vec::new();
    }
    if votes.stop {
        // FSL-50 reproduced: FSL ends here, silently
        let n = Notice::new(
            &defects::FSL_50,
            mode,
            "a profile's search range is empty: the run ends with status 255 and no output, as FSL's",
        );
        print_notices(err, &[&notices[..], &[n]].concat());
        return 255;
    }
    if votes.overruns > 0 {
        notices.push(Notice::new(
            &defects::FSL_51,
            mode,
            match mode {
                UpstreamDefects::Fix => format!(
                    "{} profile(s) whose searches read past their last sample dropped; FSL reads memory it never set there",
                    votes.overruns
                ),
                UpstreamDefects::Reproduce => format!(
                    "{} profile(s) whose searches read past their last sample: the samples beyond read as nothing there (FSL reads memory it never set)",
                    votes.overruns
                ),
            },
        ));
    }
    if o.verbose {
        let _ = writeln!(out, " nb of profiles : {}", votes.profiles);
        // the profiles dropped and those of which only the scalp counts (observed: FSL's
        // percentage counts both)
        let removed = (votes.dropped + votes.scalp_only) as f64;
        let pct = if votes.profiles == 0 {
            "-nan".to_owned()
        } else {
            g(100.0 * removed / votes.profiles as f64)
        };
        let _ = writeln!(out, " removed profiles : {pct}%");
        let _ = writeln!(out, "blurring");
    }
    let gshape = grid.shape;
    let gvox = grid.voxel;
    // quiet: betsurf names no GLIBC-07 (its fit cannot tell whether a weight's last bit moved it)
    let libm = larmorx_core::math::Libm::quiet(mode);
    let smoothed = |v: Vec<f32>| in_pool(n_threads, v, |v| bs::smooth(v, gshape, gvox, 3.0, &libm));
    let inner_s = smoothed(votes.inner);
    let scalp_s = smoothed(votes.scalp);
    let mut outer_counts = votes.outer;
    if o.verbose {
        let _ = writeln!(out, "computation");
    }
    let gvol = |data: Vec<f32>| Vol::new(gshape, gvox, data);
    // the outputs
    let ext = settings
        .get("FSL_LOAD_NIFTI_EXTENSIONS")
        .is_some_and(|v| options::atof(v) != 0.0);
    let output_type = settings.get("FSLOUTPUTTYPE").map(str::to_owned);
    let root = settings.name(o.output.as_str());
    let output = Output {
        template: &t1.header,
        flipped: t1.flipped,
        shape,
        volumes: 1,
        // the T1's display range kept (observed)
        cal: None,
        extensions: ext,
        defects: mode,
        n_threads,
        descrip: image::DESCRIP,
        rewrite_quaternion: true,
    };
    let mut written_notices: Vec<Notice> = Vec::new();
    let mut write_image = |suffix: &str, data: Vec<i16>, err: &mut dyn Write| -> Result<(), u8> {
        let ot = match &output_type {
            None => {
                let _ = err.write_all(FSLOUTPUTTYPE_UNSET.as_bytes());
                return Err(1);
            }
            Some(t) if !image::OUTPUT_TYPES.contains(&t.as_str()) => {
                let _ = writeln!(err, "{}", image::unknown_output_type_message(t));
                return Err(1);
            }
            Some(t) => t.clone(),
        };
        let stem = format!("{root}_{suffix}");
        let Some((name, version)) = image::output_file(&stem, &ot) else {
            return Err(1);
        };
        match image::write(&name, version, &output, Values::I16(data)) {
            Ok(n) => {
                for x in n {
                    defects::push_once(&mut written_notices, x);
                }
                Ok(())
            }
            Err(e) => {
                let _ = writeln!(
                    err,
                    "larmorx fsl betsurf: {name} cannot be written ({e}); FSL aborts here (larmorx)"
                );
                Err(1)
            }
        }
    };
    let t1grid = |p: &[V3]| grid.out_of_grid(p);
    let drawn = |mesh: &Mesh, p: &[V3]| -> Vec<i16> {
        let mut v = vec![0i16; n_vox];
        raster::draw(mesh, p, shape, voxel, Rounding::Floor, |k| v[k] = 1);
        v
    };
    let filled =
        |mesh: &Mesh, p: &[V3], start: Option<V3>, notices: &mut Vec<Notice>| -> Vec<i16> {
            bs::surface_mask(
                mesh,
                p,
                shape,
                voxel,
                Rounding::Floor,
                vec![0.0; n_vox],
                start,
                mode,
                notices,
            )
            .into_iter()
            .map(|x| x as i16)
            .collect()
        };
    let vtk = |suffix: &str, mesh: &Mesh, p: &[V3], err: &mut dyn Write| {
        let name = format!("{root}_{suffix}_mesh.vtk");
        if write_vtk(&name, mesh, p).is_err() {
            let _ = writeln!(err, "error opening file {name}");
        }
    };
    let finish = |err: &mut dyn Write, notices: &[Notice], more: &[Notice], code: u8| -> u8 {
        print_notices(err, notices);
        print_notices(err, more);
        code
    };
    // phase 1: the inner skull
    if o.verbose {
        let _ = writeln!(out, " inner skull");
    }
    let inner_img = gvol(inner_s);
    let inner = pooled(n_threads, || {
        let mut p = grid.into_grid(&pts);
        bs::deform(&brain, &mut p, &inner_img, None, 800, 0.5);
        p
    });
    if precision > 0 && o.verbose {
        let _ = writeln!(out, "  increased precision");
    }
    let (in_mesh, in_pts) = pooled(n_threads, || {
        refined(&brain, &inner, &inner_img, None, 0.5, precision)
    });
    let in_t1 = t1grid(&in_pts);
    vtk("inskull", &in_mesh, &in_t1, err);
    if o.outline {
        if o.verbose {
            let _ = writeln!(out, "  outline");
        }
        if let Err(c) = write_image("inskull_mesh", drawn(&in_mesh, &in_t1), err) {
            return finish(err, &notices, &written_notices, c);
        }
    }
    let mut inner_mask_t1: Option<Vec<i16>> = None;
    if o.mask || o.skull {
        let m = filled(&in_mesh, &in_t1, None, &mut notices);
        if o.mask {
            if o.verbose {
                let _ = writeln!(out, "  mask");
            }
            if let Err(c) = write_image("inskull_mask", m.clone(), err) {
                return finish(err, &notices, &written_notices, c);
            }
        }
        inner_mask_t1 = Some(m);
    }
    // phase 2: the inner skull's mask on the working grid (the smoothed inner votes truncated
    // to integers as barriers, the surface drawn and filled); the outer votes set to 0 inside
    let inner_work = bs::surface_mask(
        &brain,
        &inner,
        gshape,
        gvox,
        Rounding::Floor,
        pooled(n_threads, || bs::truncated(&inner_img.data)),
        None,
        mode,
        &mut notices,
    );
    for (c, &m) in outer_counts.iter_mut().zip(&inner_work) {
        if m == 1.0 {
            *c = 0.0;
        }
    }
    let outer_img = gvol(smoothed(outer_counts));
    let inner_mask_vol = gvol(inner_work);
    // phase 3: the outer skull
    if o.verbose {
        let _ = writeln!(out, " outer skull");
    }
    let outer = pooled(n_threads, || {
        let mut p = inner.clone();
        bs::deform(&brain, &mut p, &outer_img, Some(&inner_mask_vol), 800, 0.5);
        p
    });
    drop(inner_mask_vol);
    if precision > 0 && o.verbose {
        let _ = writeln!(out, "  increased precision");
    }
    let (out_mesh, out_pts) = pooled(n_threads, || {
        refined(&brain, &outer, &outer_img, None, 0.5, precision)
    });
    drop(outer_img);
    let out_t1 = t1grid(&out_pts);
    vtk("outskull", &out_mesh, &out_t1, err);
    if o.outline {
        if o.verbose {
            let _ = writeln!(out, "  outline");
        }
        if let Err(c) = write_image("outskull_mesh", drawn(&out_mesh, &out_t1), err) {
            return finish(err, &notices, &written_notices, c);
        }
    }
    let mut outer_mask_t1: Option<Vec<i16>> = None;
    if o.mask || o.skull {
        let m = filled(&out_mesh, &out_t1, None, &mut notices);
        if o.mask {
            if o.verbose {
                let _ = writeln!(out, "  mask");
            }
            if let Err(c) = write_image("outskull_mask", m.clone(), err) {
                return finish(err, &notices, &written_notices, c);
            }
        }
        outer_mask_t1 = Some(m);
    }
    if o.skull {
        if o.verbose {
            let _ = writeln!(out, " skull mask");
        }
        let edge = if o.t1only {
            drawn(&out_mesh, &out_t1)
        } else {
            drawn(&in_mesh, &in_t1)
        };
        let inm = inner_mask_t1.as_deref().unwrap_or(&[]);
        let outm = outer_mask_t1.as_deref().unwrap_or(&[]);
        let skull: Vec<i16> = (0..n_vox)
            .map(|k| {
                let v = i32::from(outm[k]) * (1 - i32::from(inm[k])) + i32::from(edge[k]);
                v.min(1) as i16
            })
            .collect();
        if let Err(c) = write_image("skull_mask", skull, err) {
            return finish(err, &notices, &written_notices, c);
        }
    }
    // phase 4: the outer skull's mask on the working grid (from the inner skull's votes)
    let outer_work = bs::surface_mask(
        &brain,
        &outer,
        gshape,
        gvox,
        Rounding::Floor,
        pooled(n_threads, || bs::truncated(&inner_img.data)),
        None,
        mode,
        &mut notices,
    );
    drop(inner_img);
    let outer_mask_vol = gvol(outer_work);
    // phase 5: the scalp, from a new sphere inside the outer skull
    if o.verbose {
        let _ = writeln!(out, " skin");
    }
    let scalp_img = gvol(scalp_s);
    let c = bs::centroid(&outer);
    let r = 0.75 * bs::mean_radius(&outer, c);
    let (skin_mesh0, skin0) = bs::sphere(c, r);
    let skin = pooled(n_threads, || {
        let mut p = skin0.clone();
        bs::deform(
            &skin_mesh0,
            &mut p,
            &scalp_img,
            Some(&outer_mask_vol),
            1500,
            1.5,
        );
        p
    });
    if precision > 0 && o.verbose {
        let _ = writeln!(out, "  increased precision");
    }
    // `-p`: the scalp itself retessellated, 100 more steps each at the scalp's speed and without
    // the mask (observed: the closest to FSL's of the four readings)
    let (skin_mesh, skin_pts) = pooled(n_threads, || {
        refined(&skin_mesh0, &skin, &scalp_img, None, 1.5, precision)
    });
    let skin_t1 = t1grid(&skin_pts);
    vtk("outskin", &skin_mesh, &skin_t1, err);
    if o.outline {
        if o.verbose {
            let _ = writeln!(out, "  outline");
        }
        if let Err(c) = write_image("outskin_mesh", drawn(&skin_mesh, &skin_t1), err) {
            return finish(err, &notices, &written_notices, c);
        }
    }
    if o.mask {
        if o.verbose {
            let _ = writeln!(out, "  mask");
        }
        // FSL-47: --t1only fills the scalp's mask from the outer skull's centroid in the
        // working grid
        let start = if o.t1only && mode.reproduce() {
            Some(bs::centroid(&outer))
        } else {
            None
        };
        let m = filled(&skin_mesh, &skin_t1, start, &mut notices);
        if o.t1only {
            let other_start = if mode.reproduce() {
                None
            } else {
                Some(bs::centroid(&outer))
            };
            let other = filled(&skin_mesh, &skin_t1, other_start, &mut Vec::new());
            if other != m {
                let k = other.iter().zip(&m).filter(|(a, b)| a != b).count();
                notices.push(Notice::new(
                    &defects::FSL_47,
                    mode,
                    match mode {
                        UpstreamDefects::Fix => format!(
                            "the scalp's mask is filled from the scalp surface's centroid; FSL fills it from the outer skull's centroid in the padded grid ({k} voxels differ)"
                        ),
                        UpstreamDefects::Reproduce => format!(
                            "the scalp's mask is filled from the outer skull's centroid in the padded grid, as FSL does ({k} voxels differ from the scalp surface's own)"
                        ),
                    },
                ));
            }
        }
        if let Err(c) = write_image("outskin_mask", m, err) {
            return finish(err, &notices, &written_notices, c);
        }
    }
    finish(err, &notices, &written_notices, 0)
}

/// `f(value)` in a pool of exactly `n_threads` threads ([`pooled`]), the value moved in (not
/// copied): `f` runs once, in the pool or, where no pool can be made, here.
fn in_pool<T: Send, R: Send>(n_threads: usize, value: T, f: impl Fn(T) -> R + Send + Sync) -> R {
    let cell = std::sync::Mutex::new(Some(value));
    pooled(n_threads, || {
        let v = cell
            .lock()
            .ok()
            .and_then(|mut g| g.take())
            .expect("in_pool runs its function once");
        f(v)
    })
}

/// `-p k`: a copy of the surface retessellated and deformed 100 more steps, `k` times (the
/// surface itself unchanged); without `-p` the surface.
fn refined(
    mesh: &Mesh,
    points: &[V3],
    image: &Vol,
    mask: Option<&Vol>,
    speed: f64,
    precision: usize,
) -> (Mesh, Vec<V3>) {
    let mut m = mesh.clone();
    let mut p = points.to_vec();
    for _ in 0..precision {
        let (m2, mut p2) = bs::retessellate(&m, &p);
        bs::deform(&m2, &mut p2, image, mask, 100, speed);
        m = m2;
        p = p2;
    }
    (m, p)
}

/// The number of the surface's drawn voxels inside the image.
fn raster_count(mesh: &Mesh, points: &[V3], shape: [usize; 3], voxel: [f64; 3]) -> usize {
    let mut n = 0;
    raster::draw(mesh, points, shape, voxel, Rounding::Floor, |_| n += 1);
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strtol() {
        use super::super::options::strtol_whole;
        assert_eq!(strtol_whole("0x10"), Some(16));
        assert_eq!(strtol_whole("07"), Some(7));
        assert_eq!(strtol_whole("08"), None);
        assert_eq!(strtol_whole("-3"), Some(-3));
        assert_eq!(strtol_whole("1.5"), None);
        assert_eq!(strtol_whole("1e3"), None);
        assert_eq!(strtol_whole("0"), Some(0));
    }

    fn w(s: &[&str]) -> Vec<String> {
        s.iter().map(|x| (*x).to_owned()).collect()
    }

    #[test]
    fn command_lines() {
        let mut e = Vec::new();
        assert!(matches!(parse(&w(&["-h"]), &mut e), Parsed::Exit(0)));
        let mut e = Vec::new();
        assert!(matches!(
            parse(&w(&["a"]), &mut e),
            Parsed::TooFew { t1only: false }
        ));
        assert!(String::from_utf8_lossy(&e).starts_with("too few arguments\n\nPart of FSL"));
        let mut e = Vec::new();
        assert!(matches!(
            parse(&w(&["a", "-o", "b", "c", "d", "e"]), &mut e),
            Parsed::Exit(255)
        ));
        let mut e = Vec::new();
        assert!(matches!(
            parse(&w(&["-p", "08", "-1"]), &mut e),
            Parsed::Exit(1)
        ));
        assert!(
            String::from_utf8_lossy(&e).ends_with("-p: Couldn't set_value! valstr=\"08 -1\"!\n")
        );
        let mut e = Vec::new();
        match parse(&w(&["-", "a", "b", "c", "d", "e"]), &mut e) {
            Parsed::Run(o) => assert_eq!((o.t1.as_str(), o.output.as_str()), ("a", "e")),
            _ => panic!(),
        }
        let mut e = Vec::new();
        match parse(&w(&["-p", "-1", "-1", "a", "b", "c", "d"]), &mut e) {
            Parsed::Run(o) => assert_eq!((o.precision, o.t1only), (-1, true)),
            _ => panic!(),
        }
    }
}
