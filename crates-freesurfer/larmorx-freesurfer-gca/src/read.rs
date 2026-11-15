// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `GCAread` (`utils/gca.cpp` of FreeSurfer 8.2.0, with `alloc_gcs`): a GCA atlas from a `.gca`
//! file (a name holding `.gcz` anywhere is read through zlib).
//!
//! The file: a big-endian `float` version; below 4 (`GCA_UCHAR_VERSION`) the old layout (node
//! spacing, node grid sizes, inputs, and the flags only for version 3; one byte per label; no
//! priors); 4 and 5 (`GCA_INT_VERSION`, within `FLT_EPSILON`) the current one (prior and node
//! spacings, the prior grid's sizes, which are read and not used, the node grid's sizes, inputs,
//! flags; labels as bytes for exactly 4, as `int`s otherwise); any other version is refused
//! (`GCAread(<name>), version #<v> found, 5.0 expected`). Then the nodes, x-major: their class
//! count and training count, and per class its label, means, covariances and (without
//! `GCA_NO_MRF`) the six neighbours' label lists; then (current layout) the priors: label count,
//! training count, and per label its label and prior. Each class's training count is its node's
//! total times the label's prior at the node's prior point. Then integers are read until the
//! file ends; after `FILE_TAG` each is a tag: the colour table (`reading colortable from GCA
//! file...`, `colortable with <n> entries read (originally <name>)`), the atlas's type, the MR
//! parameters (`reading <n> MR parameters out of GCA header...`, `input <i>: TR=<tr> msec,
//! FA=<deg> deg, TE=<te> msec` for each input), the direction cosines, centre, sizes and voxel
//! sizes, or an unknown tag's error (`GCAread(<name>): unknown tag <hex>`, the reading goes on
//! with the next integer). Last the helper grids ([`crate::gca::gca_setup`]).
//!
//! Defects (rules A to D, each with `reproduce`): FS-749 (the grids' sizes the spacings give,
//! not the file's: by default the file's), FS-750 (a file that ends inside the atlas: by default
//! an error), FS-751 (counts and labels kept in 16 bits: by default an error when one does not
//! fit), FS-752 and FS-759 (the MR parameters of more than 100 inputs written past their room:
//! by default kept), FS-753 (the covariance count of more than 46,340 inputs overflowing: by default
//! exact), FS-755 (a negative neighbour count's allocation error printing an address: the count),
//! FS-756 (allocations beyond the machine's memory), FS-757 (a node mapped outside the prior
//! grid: by default the class weight not computed), FS-758 (damaged compressed data: an error).
//! FS-167 notes each number the file cut.

use std::io::Write;
use std::path::{Path, PathBuf};

use larmorx_core::defects::{
    FS_749, FS_750, FS_751, FS_752, FS_753, FS_755, FS_756, FS_757, FS_758, FS_759,
};
use larmorx_freesurfer_core::cstd::{fmt_f, format_double};
use larmorx_freesurfer_core::diag::{DIAG_SHOW, verbose_on};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADFILE, ERROR_BADPARM, ERROR_NOMEMORY, Fatal};
use larmorx_freesurfer_core::sse::mul;
use larmorx_freesurfer_core::utils;
use larmorx_freesurfer_io::colortab_bin::znz_ctab_read_from_binary;
use larmorx_freesurfer_io::fio;
use larmorx_io::znz::ZnzFile;

use crate::gca::{
    FILE_TAG, GCA_FLASH, GCA_INT_VERSION, GCA_NO_MRF, GCA_NORMAL, GCA_PARAM, GCA_UCHAR_VERSION,
    GCA_UNKNOWN, GIBBS_NEIGHBORS, Gc1d, Gca, GcaNode, GcaPrior, MAX_GCA_INPUTS, TAG_GCA_COLORTABLE,
    TAG_GCA_DIRCOS, TAG_GCA_TYPE, TAG_PARAMETERS, gca_alloc_max, gca_node_to_prior, gca_setup,
    get_prior, libc_enomem, to_int,
};

/// The size beyond which an allocation's success depends on the machine (FS-756): 16 GiB.
const MACHINE_BYTES: i128 = 1 << 34;
/// `sizeof(GC1D)` on x86-64.
const GC1D_BYTES: i128 = 56;
/// `FLT_EPSILON`, as `FEQUAL` compares.
const FLT_EPSILON: f64 = f32::EPSILON as f64;

/// The file as `GCAread` reads it, noting a number the file cut (FS-750).
struct Rd<'a> {
    fp: &'a mut ZnzFile,
    cut: bool,
}

impl Rd<'_> {
    fn note(&mut self, before: i64, n: i64) {
        let after = self.fp.tell();
        if before < 0 || after - before != n {
            self.cut = true;
        }
    }

    /// `znzreadInt(file)`.
    fn int(&mut self, g: &mut Globals, err: &mut dyn Write) -> i32 {
        let b = self.fp.tell();
        let v = fio::znz_read_int(g, err, self.fp);
        self.note(b, 4);
        v
    }

    /// `znzreadFloat(file)`.
    fn float(&mut self, g: &mut Globals, err: &mut dyn Write) -> f32 {
        let b = self.fp.tell();
        let v = fio::znz_read_float(g, err, self.fp);
        self.note(b, 4);
        v
    }

    /// `znzread1(&v, file)`: one unsigned byte.
    fn byte(&mut self, g: &mut Globals) -> i32 {
        let b = self.fp.tell();
        let (_, v) = fio::znzread1(g, self.fp);
        self.note(b, 1);
        v
    }
}

/// How `fopen` or `gzopen` of the atlas's name went.
enum Opened {
    File(ZnzFile),
    /// A folder: Linux opens it and its first read fails (`EISDIR`).
    Folder,
    /// The open failed with Linux's `errno`.
    Failed(i32),
}

/// Linux's `errno` for a name that cannot be opened for reading: `ENOTDIR` when a folder of its
/// path is a file (Windows reports that as not found), else the error's own.
fn open_errno(path: &Path) -> i32 {
    for a in path.ancestors().skip(1) {
        if a.as_os_str().is_empty() {
            break;
        }
        if a.is_file() {
            return 20;
        }
    }
    match std::fs::File::open(path) {
        Err(e) => log::errno_of(&e).unwrap_or(2),
        Ok(_) => 2,
    }
}

fn open(path: &PathBuf, gzipped: bool) -> Opened {
    if path.is_dir() {
        return Opened::Folder;
    }
    match ZnzFile::open(path.as_os_str().as_encoded_bytes(), b"rb", gzipped) {
        Some(f) => Opened::File(f),
        None => Opened::Failed(open_errno(path)),
    }
}

/// `ErrorReturn(NULL, (code, message))` (with the `errno` the call left).
fn fail(g: &mut Globals, err: &mut dyn Write, code: i32, message: &str) -> Option<Gca> {
    log::error_printf(g, err, code, message);
    None
}

/// An allocation that fails (`calloc` of a negative count, `ENOMEM`): the program ends.
fn alloc_exit(g: &mut Globals, err: &mut dyn Write, message: &str) -> Result<(), Fatal> {
    g.errno = libc_enomem();
    log::error_exit(g, err, ERROR_NOMEMORY, message)
}

/// `(ninputs * (ninputs + 1)) / 2` in C's `int` (wrapping as the official build computes it).
fn covar_count_c(ninputs: i32) -> i32 {
    ninputs.wrapping_mul(ninputs.wrapping_add(1)) / 2
}

/// The covariance count of `ninputs` inputs: exact by default, C's `int` with `reproduce`
/// (FS-753 where they differ).
fn covar_count(g: &mut Globals, ninputs: i32) -> i64 {
    let exact = i64::from(ninputs) * (i64::from(ninputs) + 1) / 2;
    let c = covar_count_c(ninputs);
    if i64::from(c) != exact {
        g.notices.add(
            &FS_753,
            format!(
                "GCAread: {ninputs} inputs give {exact} covariance values per class, {c} in FreeSurfer's 32-bit arithmetic"
            ),
        );
        if g.notices.reproduce() {
            return i64::from(c);
        }
    }
    exact
}

/// `alloc_gcs(nlabels, flags, ninputs)` for `nlabels` > 0: the classes, empty; the program ends
/// where FreeSurfer's allocation fails (`could not allocate mean (<n>) and covariance (<m>)
/// matrices` for a negative input or covariance count; FS-756 beyond the machine's memory).
fn alloc_gcs(
    g: &mut Globals,
    err: &mut dyn Write,
    nlabels: i32,
    flags: i32,
    ninputs: i32,
    ncovars: i64,
) -> Result<(), Fatal> {
    let per_class = i128::from(ninputs.max(0)) * 4 + i128::from(ncovars.max(0)) * 4;
    if i128::from(nlabels) * GC1D_BYTES > MACHINE_BYTES
        || i128::from(nlabels) * per_class > MACHINE_BYTES
    {
        g.notices.add(
            &FS_756,
            format!(
                "GCAread: {nlabels} classes of {ninputs} inputs allocated before reading them: FreeSurfer's allocation fails or succeeds by the machine's memory"
            ),
        );
        if g.notices.reproduce() {
            alloc_exit(
                g,
                err,
                &format!("alloc_gcs({nlabels}, {flags:x}): could not allocated {nlabels} gcs"),
            )?;
        }
    }
    if ninputs < 0 || ncovars < 0 {
        alloc_exit(
            g,
            err,
            &format!("could not allocate mean ({ninputs}) and covariance ({ncovars}) matrices"),
        )?;
    }
    Ok(())
}

/// A count or label kept in 16 bits (FS-751): by default an error when it does not fit (`None`);
/// `reproduce` keeps its low 16 bits.
fn fits16(
    g: &mut Globals,
    err: &mut dyn Write,
    fname: &str,
    what: &str,
    v: i32,
    signed: bool,
) -> Option<i32> {
    let kept = if signed {
        i32::from(v as i16)
    } else {
        i32::from(v as u16)
    };
    if kept == v {
        return Some(v);
    }
    g.notices.add(
        &FS_751,
        format!("GCAread: {what} {v} kept in 16 bits as {kept}"),
    );
    if g.notices.reproduce() {
        return Some(kept);
    }
    g.errno = 0;
    log::error_printf(
        g,
        err,
        ERROR_BADFILE,
        &format!("GCAread({fname}): {what} {v} does not fit 16 bits"),
    );
    None
}

/// How the reading of the grids ended.
enum Grids {
    Done,
    /// The file ended (FS-750): `reproduce` keeps what was read.
    Cut,
    /// An error (`None` for `GCAread`).
    Refused,
}

/// The cut's notice; by default the error (FS-750).
fn cut(g: &mut Globals, err: &mut dyn Write, fname: &str, where_: &str) -> Grids {
    g.notices.add(
        &FS_750,
        format!(
            "GCAread: the file ends inside the {where_}: the rest read as empty (memory contents in FreeSurfer)"
        ),
    );
    if g.notices.reproduce() {
        return Grids::Cut;
    }
    g.errno = 0;
    log::error_printf(
        g,
        err,
        ERROR_BADFILE,
        &format!("GCAread({fname}): the file ends inside the atlas's {where_}"),
    );
    Grids::Refused
}

/// One node's classes (after its two counts).
#[allow(clippy::too_many_arguments)]
fn read_node(
    g: &mut Globals,
    err: &mut dyn Write,
    rd: &mut Rd<'_>,
    gca: &mut Gca,
    fname: &str,
    version: f32,
    flags: i32,
    ncovars: i64,
    gcan: &mut GcaNode,
    at: [i32; 3],
) -> Result<Option<Grids>, Fatal> {
    let [x, y, z] = at;
    if gcan.nlabels != 0 {
        if gcan.nlabels < 0 {
            alloc_exit(
                g,
                err,
                &format!(
                    "GCAread({fname}): could not allocate {} labels @ ({x},{y},{z})",
                    gcan.nlabels
                ),
            )?;
        }
        alloc_gcs(g, err, gcan.nlabels, flags, gca.ninputs, ncovars)?;
    }
    let old = f64::from(version) < GCA_UCHAR_VERSION;
    let bytes = old || f64::from(version) == GCA_UCHAR_VERSION;
    for _ in 0..gcan.nlabels {
        let mut gc = Gc1d::default();
        let label = if bytes {
            rd.byte(g)
        } else {
            let v = rd.int(g, err);
            match fits16(g, err, fname, "a label of", v, false) {
                Some(v) => v,
                None => return Ok(Some(Grids::Refused)),
            }
        };
        let label = label as u16;
        if old && i32::from(label) > gca.max_label {
            gca.max_label = i32::from(label);
        }
        gcan.labels.push(label);
        for _ in 0..gca.ninputs.max(0) {
            gc.means.push(rd.float(g, err));
            if rd.cut {
                gcan.gcs.push(gc);
                return Ok(Some(Grids::Cut));
            }
        }
        for _ in 0..ncovars.max(0) {
            gc.covars.push(rd.float(g, err));
            if rd.cut {
                gcan.gcs.push(gc);
                return Ok(Some(Grids::Cut));
            }
        }
        if rd.cut {
            gcan.gcs.push(gc);
            return Ok(Some(Grids::Cut));
        }
        if gca.flags & GCA_NO_MRF == 0 {
            for _ in 0..GIBBS_NEIGHBORS {
                let v = rd.int(g, err);
                if rd.cut {
                    gcan.gcs.push(gc);
                    return Ok(Some(Grids::Cut));
                }
                let Some(nl) = fits16(g, err, fname, "a neighbour label count of", v, true) else {
                    return Ok(Some(Grids::Refused));
                };
                if nl < 0 {
                    g.notices.add(
                        &FS_755,
                        format!(
                            "GCAread: a neighbour label count of {nl}: FreeSurfer's allocation error prints part of a memory address for the count"
                        ),
                    );
                    alloc_exit(
                        g,
                        err,
                        &format!("GCAread({fname}): couldn't expand gcs to {nl}"),
                    )?;
                }
                gc.nlabels.push(nl as i16);
                let (mut labels, mut priors) = (Vec::new(), Vec::new());
                for _ in 0..nl {
                    let v = rd.int(g, err);
                    let p = rd.float(g, err);
                    if rd.cut {
                        break;
                    }
                    let Some(l) = fits16(g, err, fname, "a neighbour label of", v, false) else {
                        return Ok(Some(Grids::Refused));
                    };
                    labels.push(l as u16);
                    priors.push(p);
                }
                gc.labels.push(labels);
                gc.label_priors.push(priors);
                if rd.cut {
                    gcan.gcs.push(gc);
                    return Ok(Some(Grids::Cut));
                }
            }
        }
        gcan.gcs.push(gc);
    }
    Ok(None)
}

/// The node grid (both layouts).
fn read_nodes(
    g: &mut Globals,
    err: &mut dyn Write,
    rd: &mut Rd<'_>,
    gca: &mut Gca,
    fname: &str,
    version: f32,
    flags: i32,
) -> Result<Grids, Fatal> {
    let ncovars = covar_count(g, gca.ninputs);
    for x in 0..gca.node_width {
        for y in 0..gca.node_height {
            for z in 0..gca.node_depth {
                let mut gcan = GcaNode {
                    nlabels: rd.int(g, err),
                    total_training: rd.int(g, err),
                    ..GcaNode::default()
                };
                if rd.cut {
                    gcan.nlabels = 0;
                    gca.nodes.push(gcan);
                    return Ok(cut(g, err, fname, "nodes"));
                }
                let r = read_node(
                    g,
                    err,
                    rd,
                    gca,
                    fname,
                    version,
                    flags,
                    ncovars,
                    &mut gcan,
                    [x, y, z],
                )?;
                gcan.nlabels = gcan.nlabels.min(gcan.gcs.len() as i32).max(0);
                gca.nodes.push(gcan);
                match r {
                    None => {}
                    Some(Grids::Cut) => return Ok(cut(g, err, fname, "nodes")),
                    Some(other) => return Ok(other),
                }
            }
        }
    }
    Ok(Grids::Done)
}

/// The prior grid (current layout).
fn read_priors(
    g: &mut Globals,
    err: &mut dyn Write,
    rd: &mut Rd<'_>,
    gca: &mut Gca,
    fname: &str,
    version: f32,
) -> Result<Grids, Fatal> {
    let bytes = f64::from(version) == GCA_UCHAR_VERSION;
    for x in 0..gca.prior_width {
        for y in 0..gca.prior_height {
            for z in 0..gca.prior_depth {
                let v = rd.int(g, err);
                let total_training = rd.int(g, err);
                if rd.cut {
                    gca.priors.push(GcaPrior::default());
                    return Ok(cut(g, err, fname, "priors"));
                }
                let Some(nlabels) = fits16(g, err, fname, "a prior label count of", v, true) else {
                    return Ok(Grids::Refused);
                };
                if nlabels < 0 {
                    alloc_exit(
                        g,
                        err,
                        &format!(
                            "GCAread({fname}): could not allocate {nlabels} labels @ ({x},{y},{z})"
                        ),
                    )?;
                }
                let mut gcap = GcaPrior {
                    nlabels: nlabels as i16,
                    total_training,
                    ..GcaPrior::default()
                };
                for _ in 0..nlabels {
                    let label = if bytes {
                        rd.byte(g)
                    } else {
                        let v = rd.int(g, err);
                        match fits16(g, err, fname, "a label of", v, false) {
                            Some(v) => v,
                            None => return Ok(Grids::Refused),
                        }
                    };
                    let p = rd.float(g, err);
                    if rd.cut {
                        break;
                    }
                    if label > gca.max_label {
                        gca.max_label = label;
                    }
                    gcap.labels.push(label as u16);
                    gcap.priors.push(p);
                }
                gcap.nlabels = gcap.labels.len() as i16;
                gca.priors.push(gcap);
                if rd.cut {
                    return Ok(cut(g, err, fname, "priors"));
                }
            }
        }
    }
    Ok(Grids::Done)
}

/// Each class's training count: its node's total times its label's prior at the node's prior
/// point (FS-757 for a point outside the prior grid: by default not computed).
fn class_training(g: &mut Globals, gca: &mut Gca) {
    // FreeSurfer looks up every node's prior point, empty nodes too: the grid's corners show
    // whether one is outside (the map is monotonic along each axis)
    let (w, h0, d0) = (gca.node_width, gca.node_height, gca.node_depth);
    if w > 0 && h0 > 0 && d0 > 0 {
        for c in 0..8 {
            let n = [
                if c & 1 == 0 { 0 } else { w - 1 },
                if c & 2 == 0 { 0 } else { h0 - 1 },
                if c & 4 == 0 { 0 } else { d0 - 1 },
            ];
            let p = gca_node_to_prior(g, gca, n);
            if gca.prior(p[0], p[1], p[2]).is_none() {
                g.notices.add(
                    &FS_757,
                    format!(
                        "GCAread: node ({},{},{}) maps to prior ({},{},{}) outside the {}x{}x{} prior grid: FreeSurfer reads outside the grid (its class weights not computed here)",
                        n[0], n[1], n[2], p[0], p[1], p[2], gca.prior_width, gca.prior_height, gca.prior_depth
                    ),
                );
                break;
            }
        }
    }
    let (h, d) = (
        gca.node_height.max(0) as usize,
        gca.node_depth.max(0) as usize,
    );
    for i in 0..gca.nodes.len() {
        if gca.nodes[i].nlabels == 0 {
            continue;
        }
        let x = (i / (h * d)) as i32;
        let y = ((i / d) % h) as i32;
        let z = (i % d) as i32;
        let [xp, yp, zp] = gca_node_to_prior(g, gca, [x, y, z]);
        let Some(gcap) = gca.prior(xp, yp, zp).cloned() else {
            continue;
        };
        let node = &gca.nodes[i];
        let tt = node.total_training;
        let ntr: Vec<i32> = node
            .labels
            .iter()
            .map(|&l| {
                let p = get_prior(&gcap, i32::from(l));
                to_int(g, f64::from(mul(tt as f32, p)), "GCAread")
            })
            .collect();
        for (gc, v) in gca.nodes[i].gcs.iter_mut().zip(ntr) {
            gc.ntraining = v;
        }
    }
}

/// The MR parameters of input `n` past `MAX_GCA_INPUTS` as FreeSurfer stores them (FS-752,
/// `reproduce`): over the struct's next fields. `false` once a pointer is overwritten (FreeSurfer
/// crashes later).
struct Overflow {
    /// `TRs`, `FAs`, `TEs`, the direction cosines and centre, `width`, `height`, `depth` and
    /// the voxel sizes, as the struct lays them out.
    mem: Vec<u8>,
    clobbered: bool,
}

const OVERFLOW_BYTES: usize = 3 * 8 * MAX_GCA_INPUTS + 12 * 4 + 3 * 4 + 3 * 4;

impl Overflow {
    fn of(gca: &Gca) -> Overflow {
        let mut mem = Vec::with_capacity(OVERFLOW_BYTES);
        for a in [&gca.trs, &gca.fas, &gca.tes] {
            for v in a.iter().take(MAX_GCA_INPUTS) {
                mem.extend_from_slice(&v.to_le_bytes());
            }
        }
        for v in [
            gca.x_r, gca.x_a, gca.x_s, gca.y_r, gca.y_a, gca.y_s, gca.z_r, gca.z_a, gca.z_s,
            gca.c_r, gca.c_a, gca.c_s,
        ] {
            mem.extend_from_slice(&v.to_le_bytes());
        }
        for v in [gca.width, gca.height, gca.depth] {
            mem.extend_from_slice(&v.to_le_bytes());
        }
        for v in [gca.xsize, gca.ysize, gca.zsize] {
            mem.extend_from_slice(&v.to_le_bytes());
        }
        Overflow {
            mem,
            clobbered: false,
        }
    }

    fn put(&mut self, array: usize, n: usize, v: f64) {
        let off = 8 * (array * MAX_GCA_INPUTS + n);
        if off + 8 > OVERFLOW_BYTES {
            self.clobbered = true;
            return;
        }
        self.mem[off..off + 8].copy_from_slice(&v.to_le_bytes());
    }

    fn back(&self, gca: &mut Gca) {
        let f64_at = |o: usize| f64::from_le_bytes(self.mem[o..o + 8].try_into().unwrap());
        let f32_at = |o: usize| f32::from_le_bytes(self.mem[o..o + 4].try_into().unwrap());
        let i32_at = |o: usize| i32::from_le_bytes(self.mem[o..o + 4].try_into().unwrap());
        for n in 0..MAX_GCA_INPUTS {
            gca.trs[n] = f64_at(8 * n);
            gca.fas[n] = f64_at(8 * (MAX_GCA_INPUTS + n));
            gca.tes[n] = f64_at(8 * (2 * MAX_GCA_INPUTS + n));
        }
        let b = 24 * MAX_GCA_INPUTS;
        let f: Vec<f32> = (0..12).map(|i| f32_at(b + 4 * i)).collect();
        [
            gca.x_r, gca.x_a, gca.x_s, gca.y_r, gca.y_a, gca.y_s, gca.z_r, gca.z_a, gca.z_s,
            gca.c_r, gca.c_a, gca.c_s,
        ] = f.try_into().unwrap();
        gca.width = i32_at(b + 48);
        gca.height = i32_at(b + 52);
        gca.depth = i32_at(b + 56);
        gca.xsize = f32_at(b + 60);
        gca.ysize = f32_at(b + 64);
        gca.zsize = f32_at(b + 68);
    }
}

/// The tagged section's `TAG_PARAMETERS`.
#[allow(clippy::too_many_arguments)]
fn read_parameters(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    rd: &mut Rd<'_>,
    gca: &mut Gca,
    clobbered: &mut bool,
) -> Result<(), Fatal> {
    let nparms = rd.int(g, err);
    let _ = writeln!(out, "reading {nparms} MR parameters out of GCA header...");
    let over = gca.ninputs > 0 && gca.ninputs as usize > MAX_GCA_INPUTS;
    // the room for 100 inputs, the next fields, then the atlas's references (109: the last
    // input whose parameters stay before them)
    if over && gca.ninputs <= 109 {
        g.notices.add(
            &FS_752,
            format!(
                "GCAread: the MR parameters of {} inputs: FreeSurfer writes those past the 100th over the atlas's other fields (kept here)",
                gca.ninputs
            ),
        );
    } else if over {
        g.notices.add(
            &FS_759,
            format!(
                "GCAread: the MR parameters of {} inputs: FreeSurfer writes them over the atlas's references and crashes (kept here)",
                gca.ninputs
            ),
        );
    }
    let mut model = (over && g.notices.reproduce()).then(|| Overflow::of(gca));
    for n in 0..gca.ninputs.max(0) as usize {
        let tr = f64::from(rd.float(g, err));
        let fa = f64::from(rd.float(g, err));
        let te = f64::from(rd.float(g, err));
        match model.as_mut() {
            Some(m) if n >= MAX_GCA_INPUTS => {
                m.put(0, n, tr);
                m.put(1, n, fa);
                m.put(2, n, te);
            }
            Some(m) => {
                m.put(0, n, tr);
                m.put(1, n, fa);
                m.put(2, n, te);
                gca.trs[n] = tr;
                gca.fas[n] = fa;
                gca.tes[n] = te;
            }
            None => {
                if n >= gca.trs.len() {
                    gca.trs.resize(n + 1, 0.0);
                    gca.fas.resize(n + 1, 0.0);
                    gca.tes.resize(n + 1, 0.0);
                }
                gca.trs[n] = tr;
                gca.fas[n] = fa;
                gca.tes[n] = te;
            }
        }
        let deg = (360.0 * fa) / (2.0 * std::f64::consts::PI);
        let _ = writeln!(
            out,
            "input {n}: TR={} msec, FA={} deg, TE={} msec",
            fmt_f(tr, 2, 1),
            fmt_f(deg, 2, 1),
            fmt_f(te, 2, 1)
        );
        if rd.cut {
            break;
        }
    }
    if let Some(m) = model {
        m.back(gca);
        if m.clobbered {
            *clobbered = true;
        }
    }
    Ok(())
}

/// How the tagged section's reading ended.
enum Tags {
    Done,
    /// The file ended inside a tag's parameters (FS-750).
    Cut,
    /// zlib's error (FS-758).
    Damaged,
}

/// The tagged section (after the grids).
fn read_tags(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    rd: &mut Rd<'_>,
    gca: &mut Gca,
    fname: &str,
    clobbered: &mut bool,
) -> Result<Tags, Fatal> {
    let mut tag = 0i32;
    // FreeSurfer loops while the count is not 0: zlib's error (-1) never ends it (FS-758)
    loop {
        let r = fio::znz_read_int_ex(rd.fp, &mut tag);
        if r < 0 {
            return Ok(Tags::Damaged);
        }
        if r == 0 {
            break;
        }
        if tag != FILE_TAG {
            continue;
        }
        loop {
            let r = fio::znz_read_int_ex(rd.fp, &mut tag);
            if r < 0 {
                return Ok(Tags::Damaged);
            }
            if r == 0 {
                break;
            }
            match tag {
                TAG_GCA_COLORTABLE => {
                    let _ = out.write_all(b"reading colortable from GCA file...\n");
                    gca.ct = znz_ctab_read_from_binary(g, out, err, Some(&mut *rd.fp));
                    if let Some(ct) = &gca.ct {
                        let mut line = format!(
                            "colortable with {} entries read (originally ",
                            ct.nentries()
                        )
                        .into_bytes();
                        line.extend_from_slice(&ct.fname);
                        line.extend_from_slice(b")\n");
                        let _ = out.write_all(&line);
                    }
                }
                TAG_GCA_TYPE => {
                    rd.int(g, err);
                    gca.gtype = rd.int(g, err);
                    if verbose_on(g) {
                        let what = match gca.gtype {
                            GCA_NORMAL => "Normal gca type",
                            GCA_PARAM => "T1/PD gca type",
                            GCA_FLASH => "FLASH gca type",
                            _ => {
                                gca.gtype = GCA_UNKNOWN;
                                "Unknown"
                            }
                        };
                        let _ = writeln!(out, "setting gca type = {what}");
                    }
                }
                TAG_PARAMETERS => read_parameters(g, out, err, rd, gca, clobbered)?,
                TAG_GCA_DIRCOS => {
                    let mut f = [0f32; 12];
                    for v in &mut f {
                        *v = rd.float(g, err);
                    }
                    [
                        gca.x_r, gca.x_a, gca.x_s, gca.y_r, gca.y_a, gca.y_s, gca.z_r, gca.z_a,
                        gca.z_s, gca.c_r, gca.c_a, gca.c_s,
                    ] = f;
                    gca.width = rd.int(g, err);
                    gca.height = rd.int(g, err);
                    gca.depth = rd.int(g, err);
                    gca.xsize = rd.float(g, err);
                    gca.ysize = rd.float(g, err);
                    gca.zsize = rd.float(g, err);
                    if g.gdiag & DIAG_SHOW != 0 && verbose_on(g) {
                        let p = |a: f32| format_double("% .4f", f64::from(a)).unwrap_or_default();
                        let _ = writeln!(out, "Direction cosines read:");
                        let _ = writeln!(
                            out,
                            " x_r = {}, y_r = {}, z_r = {}",
                            p(gca.x_r),
                            p(gca.y_r),
                            p(gca.z_r)
                        );
                        let _ = writeln!(
                            out,
                            " x_a = {}, y_a = {}, z_a = {}",
                            p(gca.x_a),
                            p(gca.y_a),
                            p(gca.z_a)
                        );
                        let _ = writeln!(
                            out,
                            " x_s = {}, y_s = {}, z_s = {}",
                            p(gca.x_s),
                            p(gca.y_s),
                            p(gca.z_s)
                        );
                        let _ = writeln!(
                            out,
                            " c_r = {}, c_a = {}, c_s = {}",
                            p(gca.c_r),
                            p(gca.c_a),
                            p(gca.c_s)
                        );
                    }
                }
                _ => log::error_printf(
                    g,
                    err,
                    ERROR_BADFILE,
                    &format!("GCAread({fname}): unknown tag {:x}\n", tag as u32),
                ),
            }
            if rd.cut {
                return Ok(Tags::Cut);
            }
        }
        break;
    }
    Ok(Tags::Done)
}

/// `GCAread(fname)`: the atlas, or `None` after FreeSurfer's message (`GCAread(<name>): could
/// not open file` with the open's `errno`, `could not read file`, the version's refusal) or a
/// default refusal (FS-750, FS-751); the program ends where FreeSurfer's allocations fail.
pub fn gca_read(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    fname: &str,
) -> Result<Option<Gca>, Fatal> {
    let gzipped = fname.contains(".gcz");
    let path = utils::resolve(dir, fname);
    let mut fp = match open(&path, gzipped) {
        Opened::File(f) => f,
        Opened::Folder => {
            if gzipped {
                // zlib's read of a folder fails (-1), which FreeSurfer takes for a count
                g.notices.add(
                    &FS_758,
                    "GCAread: a folder read through zlib: FreeSurfer takes the version from memory contents",
                );
            }
            g.errno = 21;
            return Ok(fail(
                g,
                err,
                ERROR_BADPARM,
                &format!("GCAread({fname}): could not read file"),
            ));
        }
        Opened::Failed(e) => {
            g.errno = e;
            return Ok(fail(
                g,
                err,
                ERROR_BADPARM,
                &format!("GCAread({fname}): could not open file"),
            ));
        }
    };
    let mut version = 0f32;
    let r = fio::znz_read_float_ex(&mut fp, &mut version);
    if r < 0 {
        g.notices.add(
            &FS_758,
            "GCAread: the compressed data is damaged at its start: FreeSurfer takes the version from memory contents",
        );
    }
    if r <= 0 {
        return Ok(fail(
            g,
            err,
            ERROR_BADPARM,
            &format!("GCAread({fname}): could not read file"),
        ));
    }
    let mut rd = Rd {
        fp: &mut fp,
        cut: false,
    };
    let v = f64::from(version);
    let (mut gca, grids) = if v < GCA_UCHAR_VERSION {
        let ns = rd.float(g, err);
        let nw = [rd.int(g, err), rd.int(g, err), rd.int(g, err)];
        let ninputs = rd.int(g, err);
        let flags = if v == 3.0 { rd.int(g, err) } else { 0 };
        if rd.cut && !matches!(cut(g, err, fname, "header"), Grids::Cut) {
            return Ok(None);
        }
        let whd = nw.map(|n| to_int(g, f64::from(mul(ns, n as f32)), "GCAread"));
        let grid = stored_grid(g, ns, ns, whd, nw, None);
        let mut gca = gca_alloc_max(g, out, err, ninputs, ns, ns, whd, 0, flags, grid)?;
        let grids = read_nodes(g, err, &mut rd, &mut gca, fname, version, flags)?;
        (gca, grids)
    } else {
        // !FEQUAL(version, 4) && !FEQUAL(version, 5): a NaN version equals neither
        if !((v - GCA_UCHAR_VERSION).abs() < FLT_EPSILON)
            && !((v - GCA_INT_VERSION).abs() < FLT_EPSILON)
        {
            return Ok(fail(
                g,
                err,
                ERROR_BADFILE,
                &format!(
                    "GCAread({fname}), version #{} found, {} expected",
                    fmt_f(v, 2, 1),
                    fmt_f(GCA_INT_VERSION, 2, 1)
                ),
            ));
        }
        let ps = rd.float(g, err);
        let ns = rd.float(g, err);
        let pw = [rd.int(g, err), rd.int(g, err), rd.int(g, err)];
        let nw = [rd.int(g, err), rd.int(g, err), rd.int(g, err)];
        let ninputs = rd.int(g, err);
        let flags = rd.int(g, err);
        if rd.cut && !matches!(cut(g, err, fname, "header"), Grids::Cut) {
            return Ok(None);
        }
        let whd = nw.map(|n| to_int(g, f64::from(mul(ns, n as f32)), "GCAread"));
        let grid = stored_grid(g, ps, ns, whd, nw, Some(pw));
        let mut gca = gca_alloc_max(g, out, err, ninputs, ps, ns, whd, 0, flags, grid)?;
        let mut grids = read_nodes(g, err, &mut rd, &mut gca, fname, version, flags)?;
        if matches!(grids, Grids::Done) {
            grids = read_priors(g, err, &mut rd, &mut gca, fname, version)?;
        }
        (gca, grids)
    };
    match grids {
        Grids::Refused => return Ok(None),
        Grids::Cut | Grids::Done => {}
    }
    class_training(g, &mut gca);
    let mut clobbered = false;
    if !rd.cut {
        match read_tags(g, out, err, &mut rd, &mut gca, fname, &mut clobbered)? {
            Tags::Done => {}
            Tags::Cut => {
                if !matches!(cut(g, err, fname, "tagged parameters"), Grids::Cut) {
                    return Ok(None);
                }
            }
            Tags::Damaged => {
                g.notices.add(
                    &FS_758,
                    "GCAread: the compressed data is damaged after the grids: FreeSurfer's reading of the tagged section never ends",
                );
                g.errno = 0;
                log::error_printf(
                    g,
                    err,
                    ERROR_BADFILE,
                    &format!("GCAread({fname}): the compressed data is damaged"),
                );
                return Ok(None);
            }
        }
    }
    if clobbered {
        // FreeSurfer's next use of the overwritten pointers (the helper grids freed) crashes
        return Err(Fatal::Exit(139));
    }
    gca_setup(g, out, err, &mut gca)?;
    drop(rd);
    fp.close();
    Ok(Some(gca))
}

/// FS-749: the grids' sizes the spacings give (`gcaAllocMax`) against the file's (`pw`: the
/// prior grid's, current layout; the old layout's prior grid is its node grid). `None` keeps the
/// computed sizes (they agree, or `reproduce`).
fn stored_grid(
    g: &mut Globals,
    ps: f32,
    ns: f32,
    whd: [i32; 3],
    nw: [i32; 3],
    pw: Option<[i32; 3]>,
) -> Option<[i32; 6]> {
    let derived = crate::gca::grid_sizes(g, ps, ns, whd);
    let pw = pw.unwrap_or(nw);
    let stored = [nw[0], nw[1], nw[2], pw[0], pw[1], pw[2]];
    if derived == stored {
        return None;
    }
    g.notices.add(
        &FS_749,
        format!(
            "GCAread: the file's grids {}x{}x{} (nodes) and {}x{}x{} (priors), {}x{}x{} and {}x{}x{} from the spacings",
            stored[0], stored[1], stored[2], stored[3], stored[4], stored[5], derived[0],
            derived[1], derived[2], derived[3], derived[4], derived[5]
        ),
    );
    if g.notices.reproduce() {
        None
    } else {
        Some(stored)
    }
}
