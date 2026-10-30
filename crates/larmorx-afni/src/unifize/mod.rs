// SPDX-License-Identifier: Apache-2.0
//! `3dUnifize`: the white-matter intensity of a T1-weighted volume made uniform
//! (`specs/3dUnifize.md`), clean-room.
//!
//! - [`wmi`]: the local white-matter intensity (§4.3), by a sliding window of ranked values;
//! - [`duplo`]: the half-size image and back (§4.2, §4.4);
//! - [`run`]: the program (§5 to §7): the T2 inversion, the white-matter scaling and its
//!   squash, `-GM`, the edge clipping, `-EPI`, the final mask, with AFNI's progress letters.
//!
//! Built for speed: the local intensity, which dominates (a gather and sort of every ball in a
//! direct method), slides the ball and keeps its values as counts per rank (see [`wmi`]); the
//! percentiles of the inversion and the medians of `-GM` are selections, not sorts; every
//! voxel-wise step runs in parallel over z planes; `-EPI`'s volumes run in parallel. The
//! result never depends on the thread count.

pub mod duplo;
pub mod wmi;

use larmorx_core::defects::{self, Notice, UpstreamDefects};
use larmorx_core::math;
use larmorx_core::parallel;
use rayon::prelude::*;

use crate::automask::{self, MaskSettings, NoMask, Output};
use crate::dataset::{AfniImage, BrickData, Bricks};
use crate::geometry;
use crate::msg::Message;
use crate::settings::AfniSettings;

/// What 3dUnifize computes (its options, spec §3 and §8.1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UnifizeOptions {
    /// The ball's radius in voxels (`-Urad`, `-rbt R`).
    pub radius: f32,
    /// The percentiles averaged (`-rbt R b t`).
    pub bottom: f32,
    pub top: f32,
    /// The automask's clip level fraction (`-clfrac`).
    pub clfrac: f32,
    /// How many times `-T2` was given.
    pub t2: u32,
    /// `-T2up`.
    pub t2up: f32,
    pub gm: bool,
    /// The half-size image above a million voxels (off with `-noduplo`).
    pub duplo: bool,
    pub epi: bool,
    pub squash: bool,
    /// `-verb`: the automask's messages among the letters.
    pub verb: bool,
    /// `-ssave`, `-amsave`: the scale factors and the automasked input are wanted.
    pub scale: bool,
    pub automasked: bool,
}

impl Default for UnifizeOptions {
    fn default() -> UnifizeOptions {
        UnifizeOptions {
            radius: 18.3,
            bottom: 70.0,
            top: 80.0,
            clfrac: 0.2,
            t2: 0,
            t2up: 98.5,
            gm: false,
            duplo: true,
            epi: false,
            squash: true,
            verb: false,
            scale: false,
            automasked: false,
        }
    }
}

/// A step of the run's report, in AFNI's order.
#[derive(Debug)]
pub enum Event {
    /// Progress text (not printed with `-quiet`).
    Progress(Message),
    /// Text printed with `-quiet` too.
    Always(Message),
    /// The output written at this point (`-amsave`, `-ssave`).
    Write(Saved),
}

/// The outputs written during the run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Saved {
    Automasked,
    Scale,
}

/// What [`run`] computed.
#[derive(Debug)]
pub struct UnifizeRun {
    pub events: Vec<Event>,
    pub notices: Vec<Notice>,
    /// A fatal error after the events.
    pub failure: Option<String>,
    /// The result (float32; with `-EPI` every volume).
    pub output: Option<Output>,
    pub scale: Option<Output>,
    pub automasked: Option<Output>,
    /// Whether `-EPI`'s path ran (a series; one volume goes the plain way).
    pub epi: bool,
}

/// The automask settings of 3dUnifize (spec §3): 3dAutomask's defaults with the exterior
/// clip on and the fraction `clfrac`.
pub fn mask_settings(clfrac: f32) -> MaskSettings {
    MaskSettings {
        clfrac,
        gradual: true,
        peels: 1,
        threshold: 17,
        exterior_clip: true,
    }
}

/// The automask of `x` (spec §4.1), its messages as events when `verb`.
fn automask_of(
    x: &[f32],
    dims: [usize; 3],
    clfrac: f32,
    verb: bool,
    events: &mut Vec<Event>,
) -> Result<Vec<u8>, NoMask> {
    let mut log = Vec::new();
    let mask = automask::form_mask(x, dims, &mask_settings(clfrac), &mut log);
    if verb {
        events.extend(log.into_iter().map(|t| Event::Progress(Message::More(t))));
    }
    mask
}

/// The median of `v` (AFNI's `qmed_float`, spec §6.1): the middle value, or for an even count
/// `(float)(0.5 * (double)(x + y))` with `x + y` the float sum of the two middle values.
pub fn median(v: &mut [f32]) -> f32 {
    let n = v.len();
    if n == 0 {
        return 0.0;
    }
    let (_, &mut hi, _) = v.select_nth_unstable_by(n / 2, f32::total_cmp);
    if n % 2 == 1 {
        return hi;
    }
    let lo = v[..n / 2]
        .iter()
        .copied()
        .max_by(f32::total_cmp)
        .expect("n >= 2");
    (0.5 * f64::from(lo + hi)) as f32
}

/// The T2 inversion of `x` with `mask` (spec §5): the inside values' `u`-th percentile `U`
/// (`u` clamped to [90, 100]), then `U - x` inside the mask where `x` is positive, 0 outside
/// it or where negative. AFNI-45: when the percentile's index is one past the values, AFNI
/// reads memory there (0 in its runs); the default takes the largest value, `reproduce` 0.
pub fn invert(
    x: &mut [f32],
    mask: &[u8],
    ny: usize,
    u: f32,
    mode: UpstreamDefects,
) -> Option<Notice> {
    if ny == 1 {
        return None;
    }
    let mut inside: Vec<f32> = x
        .iter()
        .zip(mask)
        .filter(|&(_, &m)| m != 0)
        .map(|(&v, _)| v)
        .collect();
    let n = inside.len();
    if n < 100 {
        return None;
    }
    let u = u.clamp(90.0, 100.0);
    let k = ((n as f32 * u) * 0.01f32).round_ties_even() as usize;
    let mut notice = None;
    let top = if k < n {
        *inside.select_nth_unstable_by(k, f32::total_cmp).1
    } else {
        let largest = inside.iter().copied().max_by(f32::total_cmp).unwrap_or(0.0);
        let p = crate::cfmt::g32(u);
        let detail = match mode {
            UpstreamDefects::Fix => format!(
                "-T2up {p} points one past the {n} sorted mask values: the contrast is inverted \
                 about the largest, {}; AFNI reads past them (usually 0, which zeroes the output)",
                crate::cfmt::g32(largest)
            ),
            UpstreamDefects::Reproduce => format!(
                "-T2up {p} points one past the {n} sorted mask values: inverted about 0, where \
                 AFNI reads past them"
            ),
        };
        notice = Some(Notice::new(&defects::AFNI_45, mode, detail));
        if mode.fix() { largest } else { 0.0 }
    };
    x.par_iter_mut().zip(mask.par_iter()).for_each(|(v, &m)| {
        if m == 0 {
            *v = 0.0;
        } else {
            if *v > 0.0 {
                *v = top - *v;
            }
            if *v < 0.0 {
                *v = 0.0;
            }
        }
    });
    notice
}

/// The squash of a scaled value (spec §6, step 3): above 1300, `1300 + 200 * tanhf((v - 1300)
/// / 200)` in float.
#[inline]
pub fn squash(v: f32) -> f32 {
    1300.0f32 + 200.0f32 * math::tanhf((v - 1300.0f32) / 200.0f32)
}

/// The grey-matter scaling (spec §6.1) of the white-matter-scaled image `g`; whether it was
/// applied.
pub fn grey_matter(g: &mut [f32], dims: [usize; 3]) -> bool {
    let mut above: Vec<f32> = g.par_iter().copied().filter(|&v| v > 1000.0).collect();
    if above.len() < 111 {
        return false;
    }
    let p = median(&mut above);
    #[allow(clippy::excessive_precision)]
    let top = 1000.0f32 - 1.987_654_321_f32 * (p - 1000.0f32);
    let mut low = automask::clip::clip_level(g, dims, 0.4321);
    let mut band: Vec<f32> = g
        .par_iter()
        .copied()
        .filter(|&v| v >= low && v <= top)
        .collect();
    if band.len() < 111 {
        return false;
    }
    let gm = median(&mut band);
    let f = (1000.0f32 - 666.0f32) / (1000.0f32 - gm);
    low *= 0.333f32;
    g.par_iter_mut().for_each(|v| {
        *v = if *v >= low {
            let w = f * (*v - 1000.0f32) + 1000.0f32;
            if w < 0.0 { 0.0 } else { w }
        } else {
            0.0
        };
    });
    true
}

/// The edge clipping of `-T2 -T2` (spec §6.2), repeated until a pass clips nothing.
pub fn clip_edges(v: &mut [f32], dims: [usize; 3]) {
    let [nx, ny, nz] = dims;
    let plane = nx * ny;
    let u1 = 1000.0f32 * 1.111f32;
    let u2 = 1000.0f32 * 1.055f32;
    loop {
        let c = v.to_vec();
        let zeros = |i: usize, j: usize, k: usize| -> usize {
            let at = |x: usize, y: usize, z: usize| c[x + nx * (y + ny * z)];
            [
                at(i.saturating_sub(1), j, k),
                at((i + 1).min(nx - 1), j, k),
                at(i, j.saturating_sub(1), k),
                at(i, (j + 1).min(ny - 1), k),
                at(i, j, k.saturating_sub(1)),
                at(i, j, (k + 1).min(nz - 1)),
            ]
            .iter()
            .filter(|&&x| x == 0.0)
            .count()
        };
        let clipped: Vec<usize> = (0..nz)
            .into_par_iter()
            .flat_map_iter(|k| {
                let mut out = Vec::new();
                for j in 0..ny {
                    for i in 0..nx {
                        let p = i + nx * j + plane * k;
                        let x = c[p];
                        if x >= u2 {
                            let z = zeros(i, j, k);
                            if (x >= u1 && z > 2) || z > 4 {
                                out.push(p);
                            }
                        }
                    }
                }
                out
            })
            .collect();
        if clipped.is_empty() {
            return;
        }
        for p in clipped {
            v[p] = 0.0;
        }
    }
}

/// The float image of sub-brick `t` (spec §2): each stored value as float, non-finite 0, times
/// the brick factor when it is neither 0 nor 1 (a negative one included).
pub fn float_volume(bricks: &Bricks, t: usize) -> Vec<f32> {
    let [nx, ny, nz, _] = bricks.shape;
    let n = nx * ny * nz;
    let f = bricks.factors[t];
    let scale = f != 0.0 && f != 1.0;
    let finish = |x: f32| if scale { x * f } else { x };
    match &bricks.data {
        BrickData::Byte(v) => v[t * n..(t + 1) * n]
            .par_iter()
            .map(|&x| finish(f32::from(x)))
            .collect(),
        BrickData::Short(v) => v[t * n..(t + 1) * n]
            .par_iter()
            .map(|&x| finish(f32::from(x)))
            .collect(),
        BrickData::Float(v) => v[t * n..(t + 1) * n]
            .par_iter()
            .map(|&x| finish(if x.is_finite() { x } else { 0.0 }))
            .collect(),
    }
}

/// `-EPI`'s median volume (spec §7): at each voxel the median of its series, each value times
/// its sub-brick's factor when positive (AFNI-01: a negative factor is dropped in AFNI, applied
/// by default). Returns the volume and whether a negative factor was met.
pub fn median_volume(bricks: &Bricks, mode: UpstreamDefects) -> (Vec<f32>, bool) {
    let [nx, ny, nz, nt] = bricks.shape;
    let n = nx * ny * nz;
    let negative = bricks.factors.iter().any(|&f| f < 0.0);
    let factor = |t: usize| {
        let f = bricks.factors[t];
        (f > 0.0 || (f < 0.0 && mode.fix())).then_some(f)
    };
    let factors: Vec<Option<f32>> = (0..nt).map(factor).collect();
    let value = |t: usize, p: usize| -> f32 {
        let x = match &bricks.data {
            BrickData::Byte(v) => f32::from(v[t * n + p]),
            BrickData::Short(v) => f32::from(v[t * n + p]),
            BrickData::Float(v) => {
                let x = v[t * n + p];
                if x.is_finite() { x } else { 0.0 }
            }
        };
        match factors[t] {
            Some(f) => x * f,
            None => x,
        }
    };
    let out = (0..n)
        .into_par_iter()
        .map_init(
            || vec![0f32; nt],
            |s, p| {
                for (t, slot) in s.iter_mut().enumerate() {
                    *slot = value(t, p);
                }
                median(s)
            },
        )
        .collect();
    (out, negative)
}

/// 3dUnifize on `image` (spec §4 to §7) on `n_threads` threads (the result does not depend on
/// it). `image` is modified only by AFNI-23's correction.
pub fn run(
    image: &mut AfniImage,
    options: &UnifizeOptions,
    mode: UpstreamDefects,
    settings: &AfniSettings,
    n_threads: usize,
) -> Result<UnifizeRun, String> {
    // AFNI-75's guard (spec §9.1) is decided here, on the caller's thread: a limit given to
    // this computation (`larmorx_core::alloc::with_memory_limit`) belongs to it.
    let refusal = if options.epi && mode.fix() {
        let (g, _) = geometry::output_geometry(&image.geometry, mode);
        epi_guard(g.pixdim, n_threads)
    } else {
        None
    };
    parallel::with_threads(n_threads, || {
        run_here(image, options, mode, settings, refusal.as_deref())
    })
    .map_err(|e| e.to_string())?
}

/// The radius of `-EPI` (spec §7): 18.3 mm in voxels, `cbrtf` of the voxel volume, at least
/// 4.14.
pub fn epi_radius(steps: [f32; 3]) -> f32 {
    let u = math::cbrtf((steps[0] * steps[1] * steps[2]).abs());
    ((18.3 / f64::from(u)) as f32).max(4.14)
}

/// The memory AFNI's method needs for `-EPI`'s ball of neighbours (spec §9.1): with `r` the
/// radius and `workers` the local mean's workers, `(4/3 · π · r³ + 1) · (24 + 4 · workers)`
/// bytes, in double precision (the ball's points, 12 bytes each, counted twice while its list
/// grows, and each worker's buffer of one 4-byte value per point). It is the prediction for
/// AFNI's method, whatever this implementation's own memory: [`wmi::local_intensity`] keeps only
/// the lines of the ball that reach the grid, far less.
pub fn epi_ball_bytes(r: f32, workers: usize) -> f64 {
    let r = f64::from(r);
    (4.0 / 3.0 * std::f64::consts::PI * r * r * r + 1.0) * (24.0 + 4.0 * workers as f64)
}

/// A voxel size for AFNI-75's message: its magnitude with up to six decimals and no trailing
/// zeros (`0.003`, `0.05`, `3`).
fn voxel_size_text(v: f32) -> String {
    let s = format!("{:.6}", f64::from(v.abs()));
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() { "0".into() } else { s.into() }
}

/// AFNI-75 corrected (spec §9.1): why `-EPI` on voxels of `steps` mm is refused, when the
/// memory its ball of neighbours needs (`epi_ball_bytes`, with `n_threads` workers, 0 for the
/// machine's) exceeds the limit (`larmorx_core::alloc::check_plan`: a limit given to this
/// computation, `LARMORX_MEMORY_LIMIT`, above 4 GiB the machine's memory). `None`: it fits.
pub fn epi_guard(steps: [f32; 3], n_threads: usize) -> Option<String> {
    let r = epi_radius(steps);
    let need = epi_ball_bytes(r, parallel::resolve_threads(n_threads));
    let limit = larmorx_core::alloc::check_plan(need).err()?;
    let hint = if steps.iter().any(|v| v.abs() < 0.01) {
        " (a header whose units are micrometres makes the voxels 1000 times smaller than they \
         look)"
    } else {
        ""
    };
    Some(format!(
        "3dUnifize -EPI would need about {} for its ball of neighbours (radius {r:.1} voxels: \
         18.3 mm over the voxels' {} x {} x {} mm), more than {limit}{hint}",
        larmorx_core::alloc::gib(need),
        voxel_size_text(steps[0]),
        voxel_size_text(steps[1]),
        voxel_size_text(steps[2]),
    ))
}

fn progress(events: &mut Vec<Event>, text: &str) {
    events.push(Event::Progress(Message::Text(text.into())));
}

fn volume(dims: [usize; 3], v: Vec<f32>) -> Bricks {
    Bricks::new(
        [dims[0], dims[1], dims[2], 1],
        BrickData::Float(v),
        vec![0.0],
    )
    .expect("the data fit the grid")
}

const NO_TIME: crate::dataset::OutputTiming = crate::dataset::OutputTiming {
    tr: 0.0,
    toffset: 0.0,
    slices: None,
};

#[allow(clippy::too_many_lines)]
fn run_here(
    image: &mut AfniImage,
    o: &UnifizeOptions,
    mode: UpstreamDefects,
    settings: &AfniSettings,
    refusal: Option<&str>,
) -> Result<UnifizeRun, String> {
    let mut events = Vec::new();
    let mut notices = Vec::new();
    notices.extend(automask::intercept_correction(image, mode, "3dUnifize"));
    let [nx, ny, nz, nt] = image.bricks.shape;
    let dims = [nx, ny, nz];
    let nvox = nx * ny * nz;
    let mut o = *o;
    let (out_geometry, _) = geometry::output_geometry(&image.geometry, mode);
    if o.epi {
        if o.gm {
            events.push(Event::Always(Message::Info("-EPI turns off -GM".into())));
            o.gm = false;
        }
        o.radius = epi_radius(out_geometry.pixdim);
        o.t2 = 1;
        o.duplo = false;
        events.push(Event::Always(Message::Info(format!(
            "-EPI changes -Urad to {:.1} voxels",
            o.radius
        ))));
        // AFNI-75 (spec §9.1): a ball of neighbours beyond the memory limit is refused before
        // any work, one volume included; `reproduce` attempts it as AFNI does.
        if let Some(why) = refusal {
            notices.push(Notice::new(
                &defects::AFNI_75,
                mode,
                format!("{why}: an error (exit status 1); AFNI fills the machine"),
            ));
            return Ok(UnifizeRun {
                events,
                notices,
                failure: Some(format!(
                    "{why}. An error (AFNI-75); LARMORX_UPSTREAM_DEFECTS=reproduce attempts it \
                     as AFNI does"
                )),
                output: None,
                scale: None,
                automasked: None,
                epi: false,
            });
        }
    }
    progress(&mut events, " + Pre-processing: ");
    let epi = o.epi && nt > 1;
    if o.epi && nt == 1 {
        events.push(Event::Always(Message::Warning(
            "-EPI was used, but only 1 volume in the input dataset".into(),
        )));
    }
    if !o.epi && nt > 1 {
        events.push(Event::Always(Message::Warning(format!(
            "Only processing sub-brick #0 (out of {nt})"
        ))));
    }
    let failed = |mut events: Vec<Event>, notices: Vec<Notice>, why: &str| {
        progress(&mut events, "\n");
        Ok(UnifizeRun {
            events,
            notices,
            failure: Some(why.into()),
            output: None,
            scale: None,
            automasked: None,
            epi: false,
        })
    };
    // 1. The float image, or -EPI's median volume.
    let mut x = if epi {
        progress(&mut events, "M");
        let (m, negative) = median_volume(&image.bricks, mode);
        if negative {
            let detail = match mode {
                UpstreamDefects::Fix => {
                    "a negative scale factor is applied to the series whose median -EPI scales \
                     by; AFNI drops it there (and applies it to each volume), so its median has \
                     the wrong sign"
                }
                UpstreamDefects::Reproduce => {
                    "a negative scale factor is dropped from the series whose median -EPI scales \
                     by, as AFNI does: the median has the wrong sign"
                }
            };
            notices.push(Notice::new(&defects::AFNI_01, mode, detail));
        }
        m
    } else {
        float_volume(&image.bricks, 0)
    };
    // 2. The T2 inversion.
    let mut inversion_mask = None;
    if o.t2 >= 1 {
        progress(&mut events, "I");
        match automask_of(&x, dims, o.clfrac, o.verb, &mut events) {
            Ok(m) => {
                notices.extend(invert(&mut x, &m, ny, o.t2up, mode));
                inversion_mask = Some(m);
            }
            Err(NoMask) => {
                return failed(
                    events,
                    notices,
                    "the automask of a 2D image smaller than 16 x 16 is empty (NULL): AFNI \
                     crashes inverting the contrast with it (AFNI-46)",
                );
            }
        }
    }
    // 3. The white-matter scaling.
    progress(&mut events, "A");
    let mask = match automask_of(&x, dims, o.clfrac, o.verb, &mut events) {
        Ok(m) => m,
        Err(NoMask) => {
            let why = if epi {
                "Can't process EPI data: scale image missing :("
            } else {
                "Can't compute Unifize-d dataset for some reason :-("
            };
            return failed(events, notices, why);
        }
    };
    let a: Vec<f32> = x
        .par_iter()
        .zip(mask.par_iter())
        .map(|(&v, &m)| if m != 0 { v } else { 0.0 })
        .collect();
    let automasked = o.automasked.then(|| volume(dims, a.clone()));
    if o.automasked {
        events.push(Event::Write(Saved::Automasked));
    }
    let use_duplo = o.duplo && !epi && nvox > 1_000_000;
    let (wmi, dots) = if use_duplo {
        progress(&mut events, "D");
        let (h, hd) = duplo::down(&a, dims);
        progress(&mut events, "V");
        let (hv, dots) =
            wmi::local_intensity(&h, hd, 0.5f32 * o.radius + 0.001f32, o.bottom, o.top);
        let up = duplo::up_dims(hd, dims);
        let full = if up == dims {
            duplo::up(&hv, hd, dims)
        } else {
            // AFNI-47: an axis of 2 or 3 voxels comes back as 1.
            let detail = match mode {
                UpstreamDefects::Fix => format!(
                    "the half-size scale image ({}x{}x{}) comes back as {}x{}x{}, not the \
                     input's {nx}x{ny}x{nz}: it is expanded to the input's grid; AFNI reads its \
                     smaller image as if it were the input's, past its end",
                    hd[0], hd[1], hd[2], up[0], up[1], up[2]
                ),
                UpstreamDefects::Reproduce => format!(
                    "the half-size scale image ({}x{}x{}) comes back as {}x{}x{}, not the \
                     input's {nx}x{ny}x{nz}: it is read as if it were the input's, as AFNI \
                     does, with 0 past its end",
                    hd[0], hd[1], hd[2], up[0], up[1], up[2]
                ),
            };
            notices.push(Notice::new(&defects::AFNI_47, mode, detail));
            if mode.fix() {
                duplo::up(&hv, hd, dims)
            } else {
                let mut small = duplo::up(&hv, hd, up);
                small.resize(nvox, 0.0);
                small
            }
        };
        (full, dots)
    } else {
        progress(&mut events, "V");
        wmi::local_intensity(&a, dims, o.radius, o.bottom, o.top)
    };
    progress(&mut events, &".".repeat(dots));
    progress(&mut events, "U");
    let p: Vec<f32> = wmi
        .par_iter()
        .map(|&w| if w <= 0.0 { 0.0 } else { 1000.0f32 / w })
        .collect();
    let squashed: usize = x
        .par_iter_mut()
        .zip(p.par_iter())
        .map(|(v, &f)| {
            *v *= f;
            if o.squash && *v > 1300.0 {
                *v = squash(*v);
                1
            } else {
                0
            }
        })
        .sum();
    progress(&mut events, "W");
    if squashed > 0 {
        progress(&mut events, &format!("[s{squashed}]"));
    }
    let scale = o.scale.then(|| volume(dims, p.clone()));
    if o.scale {
        events.push(Event::Write(Saved::Scale));
    }
    let header_of = |bricks: &Bricks,
                     timing: &crate::dataset::OutputTiming,
                     notices: &mut Vec<Notice>,
                     messages: &mut Vec<Message>| {
        automask::header_for(image, bricks, timing, mode, settings, notices, messages)
    };
    let mut out_messages = Vec::new();
    let saved = |b: Option<Bricks>, notices: &mut Vec<Notice>, m: &mut Vec<Message>| {
        b.map(|b| header_of(&b, &NO_TIME, notices, m).map(|h| (h, b)))
            .transpose()
    };
    let automasked = saved(automasked, &mut notices, &mut out_messages)?;
    let scale = saved(scale, &mut notices, &mut out_messages)?;
    // 4. -EPI: every volume.
    if epi {
        progress(&mut events, "E");
        let imask = inversion_mask.as_ref().expect("-EPI inverts");
        let vols: Vec<Vec<f32>> = (0..nt)
            .into_par_iter()
            .map(|t| {
                let mut v = float_volume(&image.bricks, t);
                let mut n = Vec::new();
                n.extend(invert(&mut v, imask, ny, o.t2up, mode));
                for (x, &f) in v.iter_mut().zip(&p) {
                    *x *= f;
                }
                n.extend(invert(&mut v, imask, ny, o.t2up, mode));
                (v, n)
            })
            .collect::<Vec<_>>()
            .into_iter()
            .map(|(v, n)| {
                for x in n {
                    defects::push_once(&mut notices, x);
                }
                v
            })
            .collect();
        for t in 0..nt {
            if t % 100 == 47 {
                events.push(Event::Always(Message::Text(".".into())));
            }
        }
        progress(&mut events, "\n");
        let mut data = Vec::with_capacity(nvox * nt);
        for v in vols {
            data.extend(v);
        }
        let bricks = Bricks::new([nx, ny, nz, nt], BrickData::Float(data), vec![0.0; nt])
            .expect("the data fit the grid");
        let timing = automask::copy_timing(image, mode, &mut notices);
        let header = header_of(&bricks, &timing, &mut notices, &mut out_messages)?;
        return Ok(UnifizeRun {
            events,
            notices,
            failure: None,
            output: Some((header, bricks)),
            scale,
            automasked,
            epi: true,
        });
    }
    // 5. -GM.
    if o.gm && grey_matter(&mut x, dims) {
        progress(&mut events, "G");
    }
    // 6. -T2 once: inverted back; twice: the edges clipped; more: AFNI-48.
    match o.t2 {
        1 => {
            progress(&mut events, "I");
            let m = inversion_mask.as_ref().expect("inverted once");
            notices.extend(invert(&mut x, m, ny, o.t2up, mode));
        }
        2 => {
            progress(&mut events, "e");
            clip_edges(&mut x, dims);
        }
        _ => {}
    }
    // 7. The final mask.
    let fin = automask_of(&x, dims, o.clfrac, o.verb, &mut events)
        .map_err(|NoMask| "the final automask of a plane smaller than 16 x 16".to_string())?;
    progress(&mut events, "m");
    x.par_iter_mut().zip(fin.par_iter()).for_each(|(v, &m)| {
        if m == 0 {
            *v = 0.0;
        }
    });
    progress(&mut events, "\n");
    let bricks = volume(dims, x);
    let header = header_of(&bricks, &NO_TIME, &mut notices, &mut out_messages)?;
    Ok(UnifizeRun {
        events,
        notices,
        failure: None,
        output: Some((header, bricks)),
        scale,
        automasked,
        epi: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn medians() {
        assert_eq!(median(&mut [3.0, 1.0, 2.0]), 2.0);
        assert_eq!(median(&mut [4.0, 1.0, 3.0, 2.0]), 2.5);
        // An even count: the float sum of the two middle values, halved in double.
        let (a, b) = (16_777_216.0f32, 3.0f32);
        let expected = (0.5 * f64::from(a + b)) as f32;
        assert_eq!(median(&mut [b, a, 0.0, 1e9]), expected);
    }

    #[test]
    fn inversion() {
        let ny = 2;
        let mut x: Vec<f32> = (0..200).map(|i| i as f32).collect();
        let mask = vec![1u8; 200];
        assert!(invert(&mut x, &mask, ny, 98.5, UpstreamDefects::Fix).is_none());
        // U is the value at index rint(200 * 98.5 * 0.01) = 197.
        assert_eq!(x[0], 0.0);
        assert_eq!(x[1], 196.0);
        assert_eq!(x[197], 0.0);
        assert_eq!(x[199], 0.0);
        // At 100 the index is n (AFNI-45).
        let mut y: Vec<f32> = (1..=200).map(|i| i as f32).collect();
        let n = invert(&mut y, &mask, ny, 100.0, UpstreamDefects::Fix).unwrap();
        assert_eq!(n.defect.id, "AFNI-45");
        assert_eq!(y[0], 199.0);
        let mut z: Vec<f32> = (1..=200).map(|i| i as f32).collect();
        invert(&mut z, &mask, ny, 100.0, UpstreamDefects::Reproduce).unwrap();
        assert!(z.iter().all(|&v| v == 0.0));
        // Fewer than 100 values, or one row: nothing.
        let mut w = vec![5.0f32; 50];
        invert(&mut w, &[1; 50], 2, 98.5, UpstreamDefects::Fix);
        assert!(w.iter().all(|&v| v == 5.0));
    }

    /// AFNI-75 (spec §9.1): the prediction, its words, and the limit given to the computation
    /// first; the replica's numbers for the spec's cases (two workers).
    #[test]
    fn the_epi_ball_guard() {
        use larmorx_core::alloc::with_memory_limit;
        let um3 = [0.003f32; 3];
        assert_eq!(epi_radius(um3), 6100.0);
        let need = epi_ball_bytes(6100.0, 2);
        assert_eq!(larmorx_core::alloc::gib(need), "2.834e4 GiB");
        assert_eq!(
            larmorx_core::alloc::gib(epi_ball_bytes(18300.0, 2)),
            "7.651e5 GiB"
        );
        assert_eq!(
            larmorx_core::alloc::gib(epi_ball_bytes(366.0, 2)),
            "6.12 GiB"
        );
        assert_eq!(voxel_size_text(0.003), "0.003");
        assert_eq!(voxel_size_text(-0.05), "0.05");
        assert_eq!(voxel_size_text(3.0), "3");
        let why = with_memory_limit(4 << 30, || epi_guard(um3, 2)).unwrap();
        assert_eq!(
            why,
            "3dUnifize -EPI would need about 2.834e4 GiB for its ball of neighbours (radius \
             6100.0 voxels: 18.3 mm over the voxels' 0.003 x 0.003 x 0.003 mm), more than the \
             4.00 GiB given to this computation (a header whose units are micrometres makes the \
             voxels 1000 times smaller than they look)"
        );
        let why = with_memory_limit(4 << 30, || epi_guard([0.05; 3], 2)).unwrap();
        assert!(
            why.contains("about 6.12 GiB") && why.ends_with("computation"),
            "{why}"
        );
        // What fits is never refused: 0.3 mm (61 voxels) and 3 mm.
        assert!(with_memory_limit(4 << 30, || epi_guard([0.3; 3], 2)).is_none());
        assert!(with_memory_limit(4 << 30, || epi_guard([3.0; 3], 32)).is_none());
    }

    #[test]
    fn squash_is_continuous_at_1300() {
        assert_eq!(squash(1300.0), 1300.0);
        assert_eq!(squash(5000.0), 1500.0);
        assert!(squash(1400.0) < 1400.0);
    }
}
