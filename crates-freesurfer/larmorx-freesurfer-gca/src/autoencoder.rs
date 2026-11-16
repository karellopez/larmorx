// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Stacked autoencoders (`SAE`, `AE`, `include/autoencoder.h`; `utils/autoencoder.cpp`) of
//! FreeSurfer 8.2.0, as far as `mri_read` reads one as a volume (track A's relay 41 of
//! FreeSurfer's foundations): `SAEread` (1622) with `AEread` (1598), `SAEalloc` (72), `AEalloc`
//! (103), `SAEactivateNetwork` (303) with `AEactivateLayer` (309), `AEdump` (1652),
//! `SAElayerWeightsToMRI` (2008), `SAEinputWeightsToMRI` (1538), `SAEfree` (238, nothing a caller
//! sees) and `mri_read`'s branch of type `MGH_AUTOENCODER` (mriio.cpp 695-704).
//!
//! An autoencoder file (`.ae`, `.sae`, `.csae`, as `SAEwrite` lays it out) is a text line `whalf
//! nencoders scale type nlevels`, a volume-geometry block (`writeVolGeom`), then per layer seven
//! matrices in `MatrixWriteInto`'s binary form (the input-to-hidden weights, the output bias, the
//! hidden bias, the hidden-to-output weights, the input, hidden and output vectors) and the
//! layer's kernel size (`fwriteInt`). Read as a volume, layer 0 gives its input weights as a
//! window of `2 whalf + 1` voxels a side (one slice when `type` has `AUTOENCODER_2D`), one frame
//! per hidden unit and level; a later layer (the frame `#<layer>` of the name) its weights
//! through the layers before it, as a kernel of one slice, one frame per hidden unit.
//!
//! Defects (`larmorx_core::defects`; `docs/findings/freesurfer-io.md` Part 36): FS-877 (a header
//! line that does not hold its five numbers), FS-878 (a layer or matrix missing: FreeSurfer
//! dereferences a null pointer), FS-879 (`SAEalloc`'s and the kernels' sizes past 32 bits),
//! FS-880 and FS-925 (weights read past a layer's matrix's columns: the next rows', then memory
//! contents past the matrix), FS-921 (a later layer's volume takes the
//! autoencoder's stored extent; FS-927 where that extent is larger than the kernel), FS-922 (`SAEalloc`'s check run of a window of one voxel with no
//! inputs), FS-923 (that check's dump of a layer without hidden units), FS-924 (`MatrixReadFrom`
//! far past the file's end, in fs.core), FS-04 (the random weights `SAEalloc` makes are seeded
//! from the clock without `FREESURFER_SEED`).

use std::io::{Read, Write};
use std::path::Path;

use larmorx_core::defects::{
    FS_877, FS_878, FS_879, FS_880, FS_921, FS_922, FS_923, FS_925, FS_927,
};
use larmorx_core::math::Libm;
use larmorx_freesurfer_core::alloc::{CALLOC_LIMIT, ENOMEM};
use larmorx_freesurfer_core::cscan::Scanf;
use larmorx_freesurfer_core::cstd::{fmt_f, note_glibc07};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, ERROR_NOFILE, Fatal};
use larmorx_freesurfer_core::matrix::{
    MATRIX_REAL, Matrix, matrix_alloc, matrix_copy, matrix_multiply,
};
use larmorx_freesurfer_core::matrix_io::matrix_read_from;
use larmorx_freesurfer_core::matrix_ops::{matrix_add, matrix_print};
use larmorx_freesurfer_core::random::random_number;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_core::sse;
use larmorx_freesurfer_core::utils::nint;
use larmorx_freesurfer_io::cfile::CFile;
use larmorx_freesurfer_io::fio;
use larmorx_freesurfer_mri::mri::{
    MAX_INDEX, MRI_FLOAT, MRI_INT, Mri, mri_alloc_sequence, mri_copy_header, use_vol_geom_to_mri,
};
use larmorx_freesurfer_mri::vol_geom::VolGeom;
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};
use larmorx_freesurfer_volume::arith::mri_clear;
use larmorx_freesurfer_volume::vgeom::read_vol_geom;

/// `FOCUSED_AUTOENCODER`: one output (the window's centre).
pub const FOCUSED_AUTOENCODER: i32 = 0x0002;
/// `AUTOENCODER_2D`: a window of one slice.
pub const AUTOENCODER_2D: i32 = 0x0008;

/// One layer (`AUTO_ENCODER`): the matrices a reader keeps (the training's are never made here).
#[derive(Clone, Debug)]
pub struct Ae {
    pub m_input_to_hidden: Matrix,
    pub v_output_bias: Matrix,
    pub v_hidden_bias: Matrix,
    pub m_hidden_to_output: Matrix,
    pub v_hidden_net: Matrix,
    pub v_hidden: Matrix,
    pub v_input: Matrix,
    pub v_output: Matrix,
    pub ksize: i32,
}

/// A stacked autoencoder (`STACKED_AUTO_ENCODER`) as `SAEread` gives it.
#[derive(Clone, Debug)]
pub struct Sae {
    pub whalf: i32,
    pub nencoders: i32,
    pub scale: f64,
    pub stype: i32,
    pub nlevels: i32,
    /// The layers from the first (`first`, then each `next`); empty when the file ended before
    /// the first (FreeSurfer's `first` is then `NULL`).
    pub layers: Vec<Ae>,
    pub vg: VolGeom,
}

/// C's `FILE` read as a byte stream (`MatrixReadFrom`'s `fread`s).
struct ReadC<'a>(&'a mut CFile);

impl Read for ReadC<'_> {
    fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
        Ok(self.0.read_bytes(b))
    }
}

/// `VectorAlloc(n, MATRIX_REAL)` and `MatrixAlloc(rows, cols, MATRIX_REAL)` (a real matrix is
/// always made, or the program ends: FS-13).
fn alloc(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    rows: i32,
    cols: i32,
) -> Result<Option<Matrix>, Fatal> {
    matrix_alloc(g, out, err, rows, cols, MATRIX_REAL)
}

/// FS-878's refusal: FreeSurfer dereferences a null pointer here (a crash); both modes stop with
/// `what` as an error.
fn null_layer(g: &mut Globals, err: &mut dyn Write, what: &str) {
    g.notices.add(
        &FS_878,
        format!("{what}: FreeSurfer dereferences a null pointer (a crash); refused"),
    );
    log::error_printf(
        g,
        err,
        ERROR_BADPARM,
        &format!("{what} (FreeSurfer crashes here)\n"),
    );
}

/// FS-879: a size past 32 bits. By default `None` after the error `<what>: <detail> does not fit
/// an int`; with `reproduce` FreeSurfer's wrapped value.
fn int_size(
    g: &mut Globals,
    err: &mut dyn Write,
    what: &str,
    exact: i64,
    wrapped: i32,
) -> Option<i32> {
    if exact == i64::from(wrapped) {
        return Some(wrapped);
    }
    g.notices.add(
        &FS_879,
        format!("{what}: {exact} does not fit an int (FreeSurfer takes {wrapped})"),
    );
    if g.notices.reproduce() {
        return Some(wrapped);
    }
    log::error_printf(
        g,
        err,
        ERROR_BADPARM,
        &format!("{what}: {exact} does not fit an int\n"),
    );
    None
}

/// `AEalloc(prev, ninputs, nhidden, noutputs)` (103): a layer of `ninputs` inputs, `nhidden`
/// hidden units and `noutputs` outputs, its weights drawn with `randomNumber` (two draws per
/// weight, the second kept: within +-0.1; then the biases within +-1, each bias vector divided by
/// its Euclidean norm). `None` (`NULL`) after `AEaalloc(<ninputs>, <nhidden>): could not alloc
/// hidden class labels` when that allocation fails (a negative `nhidden`). The matrices'
/// allocations end the program as `MatrixAlloc` does (FS-13). `prev` changes nothing a caller
/// sees.
fn ae_alloc(
    g: &mut Globals,
    env: &Environment,
    out: &mut dyn Write,
    err: &mut dyn Write,
    ninputs: i32,
    nhidden: i32,
    noutputs: i32,
) -> Result<Option<Ae>, Fatal> {
    // the first output vector (replaced below, as FreeSurfer does)
    if alloc(g, out, err, noutputs, 1)?.is_none() {
        return Ok(None);
    }
    let Some(v_input) = alloc(g, out, err, ninputs, 1)? else {
        return Ok(None);
    };
    let Some(mut m_input_to_hidden) = alloc(g, out, err, nhidden, ninputs)? else {
        return Ok(None);
    };
    let Some(mut m_hidden_to_output) = alloc(g, out, err, noutputs, nhidden)? else {
        return Ok(None);
    };
    let Some(v_output) = alloc(g, out, err, noutputs, 1)? else {
        return Ok(None);
    };
    let Some(mut v_hidden_bias) = alloc(g, out, err, nhidden, 1)? else {
        return Ok(None);
    };
    // calloc(nhidden, sizeof(int)): a negative count wraps to a size no allocation gives
    let bytes = (i64::from(nhidden) as u64).checked_mul(4);
    if bytes.is_none_or(|b| b > CALLOC_LIMIT) {
        g.errno = ENOMEM;
        log::error_printf(
            g,
            err,
            ERROR_NOFILE,
            &format!("AEaalloc({ninputs}, {nhidden}): could not alloc hidden class labels"),
        );
        return Ok(None);
    }
    let Some(v_hidden_net) = alloc(g, out, err, nhidden, 1)? else {
        return Ok(None);
    };
    let Some(v_hidden) = alloc(g, out, err, nhidden, 1)? else {
        return Ok(None);
    };
    let Some(mut v_output_bias) = alloc(g, out, err, noutputs, 1)? else {
        return Ok(None);
    };
    let wt_lim = 0.1;
    for i in 1..=ninputs {
        for j in 1..=nhidden {
            random_number(g, env, out, err, -1.0, 1.0)?;
            let w = random_number(g, env, out, err, -wt_lim, wt_lim)?;
            m_input_to_hidden.set_relt(j, i, w as f32);
        }
    }
    for j in 1..=nhidden {
        for k in 1..=noutputs {
            random_number(g, env, out, err, -1.0, 1.0)?;
            let w = random_number(g, env, out, err, -wt_lim, wt_lim)?;
            m_hidden_to_output.set_relt(k, j, w as f32);
        }
    }
    for v in [&mut v_hidden_bias, &mut v_output_bias] {
        let n = v.rows;
        let mut norm = 0.0f64;
        for j in 1..=n {
            let w = random_number(g, env, out, err, -1.0, 1.0)?;
            norm = sse::add64(norm, sse::mul64(w, w));
            v.set_relt(j, 1, w as f32);
        }
        let norm = sse::sqrt64(norm);
        for j in 1..=n {
            let x = sse::div64(f64::from(v.relt(j, 1)), norm);
            v.set_relt(j, 1, x as f32);
        }
    }
    // average_act: calloc(v_hidden_bias->rows, sizeof(double)), never refused here (the count
    // is the hidden units', checked above)
    Ok(Some(Ae {
        m_input_to_hidden,
        v_output_bias,
        v_hidden_bias,
        m_hidden_to_output,
        v_hidden_net,
        v_hidden,
        v_input,
        v_output,
        ksize: 0,
    }))
}

fn print_matrix(g: &mut Globals, out: &mut dyn Write, err: &mut dyn Write, m: Option<&Matrix>) {
    let mut buf = Vec::new();
    matrix_print(g, Some(&mut buf), out, err, m, 5);
    let _ = out.write_all(&buf);
}

/// `AEdump(ae)` (1652) of a layer that is not being trained: with one input the line
/// `f(I=<input> * wij=<w> + bj=<b> = <net>) = <hidden> * wjk=<w> + bk=<b> = <output>, E=<output -
/// input>` (each `%2.2f` of the first element), else each vector and matrix by name with
/// `MatrixPrint` (the error vector, which a reader never has, gives `MatrixPrint: mat = NULL!`).
fn ae_dump(g: &mut Globals, out: &mut dyn Write, err: &mut dyn Write, ae: &Ae) {
    if ae.v_input.rows == 1 {
        // every matrix has its first element: one input, one output, hidden units checked
        let i = ae.v_input.relt(1, 1);
        let wij = ae.m_input_to_hidden.relt(1, 1);
        let bj = ae.v_hidden_bias.relt(1, 1);
        let net = ae.v_hidden_net.relt(1, 1);
        let h = ae.v_hidden.relt(1, 1);
        let wjk = ae.m_hidden_to_output.relt(1, 1);
        let bk = ae.v_output_bias.relt(1, 1);
        let o = ae.v_output.relt(1, 1);
        let e = sse::sub(o, i);
        let f = |x: f32| fmt_f(f64::from(x), 2, 2);
        let _ = writeln!(
            out,
            "f(I={} * wij={} + bj={} = {}) = {} * wjk={} + bk={} = {}, E={}",
            f(i),
            f(wij),
            f(bj),
            f(net),
            f(h),
            f(wjk),
            f(bk),
            f(o),
            f(e)
        );
        return;
    }
    let _ = writeln!(out, "v_input");
    print_matrix(g, out, err, Some(&ae.v_input));
    let _ = writeln!(out, "v_output");
    print_matrix(g, out, err, Some(&ae.v_output));
    let _ = writeln!(out, "v_error");
    print_matrix(g, out, err, None);
    let _ = writeln!(out, "v_hidden_bias");
    print_matrix(g, out, err, Some(&ae.v_hidden_bias));
    let _ = writeln!(out, "v_hidden_net");
    print_matrix(g, out, err, Some(&ae.v_hidden_net));
    let _ = writeln!(out, "v_hidden");
    print_matrix(g, out, err, Some(&ae.v_hidden));
    let _ = writeln!(out, "m_input_to_hidden");
    print_matrix(g, out, err, Some(&ae.m_input_to_hidden));
    let _ = writeln!(out, "v_output_bias");
    print_matrix(g, out, err, Some(&ae.v_output_bias));
    let _ = writeln!(out, "m_hidden_to_output");
    print_matrix(g, out, err, Some(&ae.m_hidden_to_output));
}

/// `SAEactivateNetwork(sae)` of a one-layer autoencoder (`AEactivateLayer(first, first->v_input)`,
/// 309; no inputs zeroed, no next layer): `hidden_net = W_ih input + hidden_bias`, `hidden =
/// 1 / (1 + exp(-hidden_net))` (in `double`, stored as `float`), `output = W_ho hidden +
/// output_bias`; the matrix products accumulate in `float` (`MatrixMultiply`).
fn activate(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    ae: &mut Ae,
) -> Result<(), Fatal> {
    let libm = Libm::new(g.mode());
    matrix_multiply(
        g,
        out,
        err,
        Some(&ae.m_input_to_hidden),
        Some(&ae.v_input),
        Some(&mut ae.v_hidden_net),
    )?;
    let net = ae.v_hidden_net.clone();
    matrix_add(
        g,
        out,
        err,
        &net,
        &ae.v_hidden_bias,
        Some(&mut ae.v_hidden_net),
    )?;
    for row in 1..=ae.v_hidden.rows {
        let x = match ae.v_hidden_net.relt_index(row, 1) {
            Some(i) => ae.v_hidden_net.raw(i).unwrap_or(0.0),
            None => 0.0,
        };
        let o = sse::div64(1.0, sse::add64(1.0, libm.exp(-f64::from(x))));
        ae.v_hidden.set_relt(row, 1, o as f32);
    }
    note_glibc07(g, &libm, "exp");
    matrix_multiply(
        g,
        out,
        err,
        Some(&ae.m_hidden_to_output),
        Some(&ae.v_hidden),
        Some(&mut ae.v_output),
    )?;
    let o = ae.v_output.clone();
    matrix_add(g, out, err, &o, &ae.v_output_bias, Some(&mut ae.v_output))?;
    Ok(())
}

/// `SAEalloc(whalf, nlevels, type, scale)` (72): `allocating SAE with inputs/hidden <n>/<h>` on
/// standard output, where the window is `2 whalf + 1` voxels a side, `n` its voxels (one slice
/// with `AUTOENCODER_2D`) times `nlevels` and `h = nint(scale n)`; the first layer
/// ([`ae_alloc`], one output with `FOCUSED_AUTOENCODER`). A window of one voxel (`whalf` 0) is
/// checked: the input set to 0, the network run and dumped ([`ae_dump`]), then the input 1, run
/// and dumped. `Ok(None)` where the reading stops (FS-879's sizes by default, FS-878, FS-922).
fn sae_alloc(
    g: &mut Globals,
    env: &Environment,
    out: &mut dyn Write,
    err: &mut dyn Write,
    whalf: i32,
    nlevels: i32,
    stype: i32,
    scale: f64,
) -> Result<Option<()>, Fatal> {
    let exact_w = 2 * i64::from(whalf) + 1;
    let Some(wsize) = int_size(
        g,
        err,
        "SAEalloc: the window",
        exact_w,
        whalf.wrapping_mul(2).wrapping_add(1),
    ) else {
        return Ok(None);
    };
    let w = i64::from(wsize);
    let (mut exact, mut ninputs) = (
        w * w * i64::from(nlevels),
        wsize.wrapping_mul(wsize).wrapping_mul(nlevels),
    );
    if stype & AUTOENCODER_2D == 0 {
        exact *= w;
        ninputs = ninputs.wrapping_mul(wsize);
    }
    let Some(ninputs) = int_size(g, err, "SAEalloc: the inputs", exact, ninputs) else {
        return Ok(None);
    };
    let nhidden = nint(g, scale * f64::from(ninputs));
    let _ = writeln!(out, "allocating SAE with inputs/hidden {ninputs}/{nhidden}");
    let noutputs = if stype & FOCUSED_AUTOENCODER != 0 {
        1
    } else {
        ninputs
    };
    let first = ae_alloc(g, env, out, err, ninputs, nhidden, noutputs)?;
    if whalf == 0 {
        let Some(mut ae) = first else {
            null_layer(g, err, "SAEalloc: the first layer was not made");
            return Ok(None);
        };
        if ae.v_input.relt_index(1, 1).is_none() {
            g.notices.add(
                &FS_922,
                format!(
                    "SAEalloc: a window of one voxel with {ninputs} inputs: FreeSurfer writes the first input outside the layer's memory; refused"
                ),
            );
            log::error_printf(
                g,
                err,
                ERROR_BADPARM,
                &format!(
                    "SAEalloc: a window of one voxel with {ninputs} inputs cannot be checked (FreeSurfer writes outside its memory here)\n"
                ),
            );
            return Ok(None);
        }
        if ae.v_input.rows == 1 && ae.m_input_to_hidden.rows < 1 {
            g.notices.add(
                &FS_923,
                "SAEalloc: a window of one voxel with no hidden units: FreeSurfer's dump reads the first weight of an empty matrix (a crash); refused",
            );
            log::error_printf(
                g,
                err,
                ERROR_BADPARM,
                "SAEalloc: a window of one voxel with no hidden units cannot be checked (FreeSurfer crashes here)\n",
            );
            return Ok(None);
        }
        for input in [0.0f32, 1.0] {
            ae.v_input.set_relt(1, 1, input);
            activate(g, out, err, &mut ae)?;
            ae_dump(g, out, err, &ae);
        }
    }
    Ok(Some(()))
}

/// What [`ae_read`] gives.
enum Layer {
    Read(Box<Ae>),
    /// `NULL`: the stream was at its end.
    End,
    /// FreeSurfer dereferences a null matrix (FS-878): refused.
    Refused,
}

/// `AEread(fp, prev)` (1598): `NULL` when the stream is at its end (`feof`); else the
/// input-to-hidden weights and the output bias (`MatrixReadFrom` into new matrices), a layer of
/// their sizes ([`ae_alloc`]: the weights' columns as inputs, rows as hidden units, the bias's
/// rows as outputs) with the two copied in (`MatrixCopy`: nothing copied where the layer's matrix
/// is smaller), then the hidden bias, the hidden-to-output weights, the input, hidden and output
/// vectors read into the layer's own (`MatrixReadFrom: specified matrix does not match file` for
/// another size: the stream is then past the three numbers only) and the kernel size
/// (`freadInt`).
fn ae_read(
    g: &mut Globals,
    env: &Environment,
    out: &mut dyn Write,
    err: &mut dyn Write,
    fp: &mut CFile,
) -> Result<Layer, Fatal> {
    if fp.feof() {
        return Ok(Layer::End);
    }
    let m_ih = matrix_read_from(g, out, err, &mut ReadC(fp), None)?.into_new();
    let v_ob = matrix_read_from(g, out, err, &mut ReadC(fp), None)?.into_new();
    let (Some(m_ih), Some(v_ob)) = (m_ih, v_ob) else {
        null_layer(
            g,
            err,
            "AEread: a layer's weights or output bias were not read",
        );
        return Ok(Layer::Refused);
    };
    let Some(mut ae) = ae_alloc(g, env, out, err, m_ih.cols, m_ih.rows, v_ob.rows)? else {
        null_layer(g, err, "AEread: the layer was not made");
        return Ok(Layer::Refused);
    };
    matrix_copy(g, out, err, Some(&v_ob), Some(&mut ae.v_output_bias))?;
    matrix_copy(g, out, err, Some(&m_ih), Some(&mut ae.m_input_to_hidden))?;
    for m in [
        &mut ae.v_hidden_bias,
        &mut ae.m_hidden_to_output,
        &mut ae.v_input,
        &mut ae.v_hidden,
        &mut ae.v_output,
    ] {
        matrix_read_from(g, out, err, &mut ReadC(fp), Some(m))?;
    }
    ae.ksize = fio::fread_int(g, fp);
    Ok(Layer::Read(Box::new(ae)))
}

/// `SAEread(fname)` (1622): `SAEread(<name>): could not open file` (`ERROR_NOFILE`) and `None`
/// when the file does not open; else its header line (`fscanf("%d %d %lf %d %d\n")`: `File(<name>)
/// Warning: expected number of values to read does not match those read` on standard error when
/// it does not give five numbers, FS-877), [`sae_alloc`], the volume geometry (`readVolGeom`), the
/// first layer and `nencoders - 1` more ([`ae_read`]; `layer <k> read with <h> hidden units` after
/// each). A first layer the file does not hold (its end) leaves the autoencoder without layers;
/// a later one is FS-878.
pub fn sae_read(
    g: &mut Globals,
    env: &Environment,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    fname: &str,
) -> Result<Option<Sae>, Fatal> {
    let mut fp = match CFile::open(dir, fname, "rb") {
        Ok(f) => f,
        Err(e) => {
            g.errno = e;
            log::error_printf(
                g,
                err,
                ERROR_NOFILE,
                &format!("SAEread({fname}): could not open file"),
            );
            return Ok(None);
        }
    };
    let mut v = [None::<i32>; 4];
    let scale;
    let n = {
        let mut errno = g.errno;
        let mut sc = Scanf::new(&mut fp);
        v[0] = sc.d(&mut errno, true);
        sc.text(&mut errno, b" ");
        v[1] = sc.d(&mut errno, true);
        sc.text(&mut errno, b" ");
        scale = sc.lf(&mut errno, true);
        sc.text(&mut errno, b" ");
        v[2] = sc.d(&mut errno, true);
        sc.text(&mut errno, b" ");
        v[3] = sc.d(&mut errno, true);
        sc.text(&mut errno, b"\n");
        let n = sc.finish(&mut errno);
        g.errno = errno;
        n
    };
    if n != 5 {
        let _ = writeln!(
            err,
            "File({fname}) Warning: expected number of values to read does not match those read"
        );
        g.notices.add(
            &FS_877,
            format!(
                "SAEread: the header line gave {n} of its 5 numbers; FreeSurfer goes on with memory contents for the others"
            ),
        );
        if !g.notices.reproduce() {
            log::error_printf(
                g,
                err,
                ERROR_BADPARM,
                &format!("SAEread({fname}): the header line does not hold its 5 numbers\n"),
            );
            return Ok(None);
        }
    }
    let [whalf, nencoders, stype, nlevels] = v.map(|x| x.unwrap_or(0));
    let scale = scale.unwrap_or(0.0);
    if sae_alloc(g, env, out, err, whalf, nlevels, stype, scale)?.is_none() {
        return Ok(None);
    }
    // the autoencoder's geometry as calloc leaves it (VOL_GEOM's constructor is not run): zeros
    let mut vg = VolGeom {
        xsize: 0.0,
        ysize: 0.0,
        zsize: 0.0,
        x_r: 0.0,
        y_s: 0.0,
        z_a: 0.0,
        ..VolGeom::default()
    };
    read_vol_geom(g, err, &mut fp, &mut vg);
    let mut sae = Sae {
        whalf,
        nencoders,
        scale,
        stype,
        nlevels,
        layers: Vec::new(),
        vg,
    };
    match ae_read(g, env, out, err, &mut fp)? {
        Layer::Read(ae) => sae.layers.push(*ae),
        Layer::End => {
            if nencoders > 1 {
                null_layer(
                    g,
                    err,
                    &format!("SAEread({fname}): the file ends before its first layer"),
                );
                return Ok(None);
            }
        }
        Layer::Refused => return Ok(None),
    }
    for i in 1..nencoders {
        match ae_read(g, env, out, err, &mut fp)? {
            Layer::Read(ae) => {
                let _ = writeln!(
                    out,
                    "layer {} read with {} hidden units",
                    i + 1,
                    ae.v_hidden.rows
                );
                sae.layers.push(*ae);
            }
            Layer::End => {
                null_layer(
                    g,
                    err,
                    &format!(
                        "SAEread({fname}): the file ends before layer {} of {nencoders}",
                        i + 1
                    ),
                );
                return Ok(None);
            }
            Layer::Refused => return Ok(None),
        }
    }
    fp.close();
    Ok(Some(sae))
}

/// `*MATRIX_RELT(m, r, c)` of a layer's weights as the volumes read them: past the matrix's
/// columns FreeSurfer reads on into the next rows (FS-880), then past the matrix (FS-925: memory
/// contents). By default `Err(())` (the reading stops); with `reproduce` the array's float there
/// (the next row's), 0 outside the array (noted as memory).
fn weight(g: &mut Globals, m: &Matrix, r: i32, c: i32, what: &str) -> Result<f32, ()> {
    if r >= 1 && r <= m.rows && c >= 1 && c <= m.cols {
        return Ok(m.relt(r, c));
    }
    let at = m.relt_index(r, c).and_then(|i| m.raw(i));
    // two IDs: a probe tells the variants apart by the ID (a run keeps each ID's first detail)
    if at.is_some() {
        g.notices.add(
            &FS_880,
            format!(
                "{what}: weight ({r}, {c}) of a {} x {} matrix: another weight of the matrix",
                m.rows, m.cols
            ),
        );
    } else {
        g.notices.add(
            &FS_925,
            format!(
                "{what}: weight ({r}, {c}) of a {} x {} matrix: memory outside the matrix (0 here)",
                m.rows, m.cols
            ),
        );
    }
    if g.notices.reproduce() {
        Ok(at.unwrap_or(0.0))
    } else {
        Err(())
    }
}

fn weights_refused(g: &mut Globals, err: &mut dyn Write, what: &str, m: &Matrix, need: i64) {
    log::error_printf(
        g,
        err,
        ERROR_BADPARM,
        &format!(
            "{what}: the layer's weights have {} inputs, the volume needs {need}\n",
            m.cols
        ),
    );
}

/// `SAEinputWeightsToMRI(sae, NULL)` (1538): a float volume of the window (`2 whalf + 1` voxels a
/// side, one slice with `AUTOENCODER_2D`), one frame per hidden unit and level (hidden unit
/// slowest), voxel `(x, y, z)` of frame `h nlevels + n` the first layer's weight of hidden unit
/// `h` from input `((n wsize + x) wsize + y) depth + z` (counted from 1). `None` where FS-880
/// stops it.
fn sae_input_weights_to_mri(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    sae: &Sae,
) -> Result<Option<Mri>, Fatal> {
    let first = &sae.layers[0];
    let wsize = sae.whalf.wrapping_mul(2).wrapping_add(1);
    let nhidden = first.v_hidden.rows;
    let exact = i64::from(nhidden) * i64::from(sae.nlevels);
    let Some(nframes) = int_size(
        g,
        err,
        "SAEinputWeightsToMRI: the frames",
        exact,
        nhidden.wrapping_mul(sae.nlevels),
    ) else {
        return Ok(None);
    };
    let depth = if sae.stype & AUTOENCODER_2D != 0 {
        1
    } else {
        wsize
    };
    let mut mri = mri_alloc_sequence(g, out, err, wsize, wsize, depth, MRI_FLOAT, nframes)?;
    let depth = mri.vg.depth;
    let m = &first.m_input_to_hidden;
    let mut frame = 0i32;
    for hidden in 0..nhidden {
        let mut i = 1i32;
        for _n in 0..sae.nlevels {
            for xk in 0..wsize {
                for yk in 0..wsize {
                    for zk in 0..depth {
                        let Ok(w) = weight(g, m, hidden + 1, i, "SAEinputWeightsToMRI") else {
                            let need = i64::from(sae.nlevels)
                                * i64::from(wsize)
                                * i64::from(wsize)
                                * i64::from(depth);
                            weights_refused(g, err, "SAEinputWeightsToMRI", m, need);
                            return Ok(None);
                        };
                        mri_set_vox_val(g, err, &mut mri, xk, yk, zk, frame, w, 0)?;
                        i = i.wrapping_add(1);
                    }
                }
            }
            frame = frame.wrapping_add(1);
        }
    }
    Ok(Some(mri))
}

/// `SAElayerWeightsToMRI(sae, layer)` (2008): a layer at or past the autoencoder's count ends the
/// program (`CSAlayerWeightsToMRI: invalid layer <l> (<n> encoders)`, `ERROR_BADPARM`); layer 0 or
/// below the input weights ([`sae_input_weights_to_mri`]); a later layer `l` its weights through
/// the layer before: the previous layer's volume `P` (of width `wp`, `hp = (wp - 1) / 2`), a
/// float volume of one slice, `ksize + 2 hp` voxels a side, one frame per hidden unit, with the
/// autoencoder's geometry (FS-921: its extent too, in FreeSurfer); for each hidden unit `h`,
/// each previous hidden unit `p` and kernel place `(x, y)` (the visible unit `v`, counted in that
/// order), each voxel `(xp, yp)` of `P` adds `P(xp, yp, p) w(h, v)` to voxel `(x + xp, y + yp)`
/// (the index tables' value: a place past the edge is the edge's) and counts it; then each voxel
/// is divided by its count (in `float`).
pub fn sae_layer_weights_to_mri(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    sae: &Sae,
    layer: i32,
) -> Result<Option<Mri>, Fatal> {
    if layer >= sae.nencoders {
        log::error_exit(
            g,
            err,
            ERROR_BADPARM,
            &format!(
                "CSAlayerWeightsToMRI: invalid layer {layer} ({} encoders)\n",
                sae.nencoders
            ),
        )?;
        return Ok(None);
    }
    if sae.layers.is_empty() {
        null_layer(
            g,
            err,
            "SAElayerWeightsToMRI: the autoencoder has no first layer",
        );
        return Ok(None);
    }
    if layer <= 0 {
        return sae_input_weights_to_mri(g, out, err, sae);
    }
    let l = layer as usize;
    let (ae, ae_prev) = (&sae.layers[l], &sae.layers[l - 1]);
    let Some(mri_prev) = sae_layer_weights_to_mri(g, out, err, sae, layer - 1)? else {
        return Ok(None);
    };
    let whalf_prev = (mri_prev.vg.width - 1) / 2;
    let exact = i64::from(ae.ksize) + 2 * i64::from(whalf_prev);
    let Some(width) = int_size(
        g,
        err,
        "SAElayerWeightsToMRI: the kernel",
        exact,
        ae.ksize.wrapping_add(whalf_prev.wrapping_mul(2)),
    ) else {
        return Ok(None);
    };
    let nhidden = ae.v_hidden.rows;
    let mut mri = mri_alloc_sequence(g, out, err, width, width, 1, MRI_FLOAT, nhidden)?;
    let mut src = sae.vg.clone();
    if (src.width, src.height, src.depth) != (width, width, 1) {
        // a stored extent past the kernel's makes FreeSurfer read past the voxels' memory
        let larger = src.width > width || src.height > width || src.depth > 1;
        let what = format!(
            "SAElayerWeightsToMRI: the layer's volume of {width} x {width} x 1 voxels takes the autoencoder's extent {} x {} x {}",
            src.width, src.height, src.depth
        );
        if !g.notices.reproduce() {
            g.notices
                .add(&FS_921, format!("{what}: the kernel's extent kept"));
            src.width = width;
            src.height = width;
            src.depth = 1;
        } else if larger {
            // its own ID: a probe tells it from FS-921's kept header by the ID
            g.notices.add(
                &FS_927,
                format!("{what}: FreeSurfer reads past the voxels' memory; refused"),
            );
            log::error_printf(
                g,
                err,
                ERROR_BADPARM,
                &format!(
                    "SAElayerWeightsToMRI: the autoencoder's extent {} x {} x {} is larger than the layer's kernel ({width} x {width} x 1; FreeSurfer reads outside its memory here)\n",
                    src.width, src.height, src.depth
                ),
            );
            return Ok(None);
        } else {
            g.notices.add(&FS_921, format!("{what} (FreeSurfer's)"));
        }
    }
    use_vol_geom_to_mri(g, out, err, &src, &mut mri.vg)?;
    let mut counts = mri_alloc_sequence(g, out, err, width, width, 1, MRI_INT, 1)?;
    mri_copy_header(g, out, err, &mri, Some(&mut counts))?;
    let m = &ae.m_input_to_hidden;
    let index = |t: &Option<Vec<i32>>, i: i32| -> Option<i32> {
        let k = usize::try_from(i64::from(i) + i64::from(MAX_INDEX)).ok()?;
        t.as_ref()?.get(k).copied()
    };
    let (pw, ph) = (mri_prev.vg.width, mri_prev.vg.height);
    for h in 0..nhidden {
        let mut v = 0i32;
        for hp in 0..ae_prev.v_hidden.rows {
            for x in 0..ae.ksize {
                for y in 0..ae.ksize {
                    for xp in 0..pw {
                        let xk = index(&mri.xi, x.wrapping_add(xp)).unwrap_or(0);
                        for yp in 0..ph {
                            let yk = index(&mri.yi, y.wrapping_add(yp)).unwrap_or(0);
                            let count = mri_get_vox_val(g, err, &counts, xk, yk, 0, 0, 0)?;
                            let count =
                                larmorx_freesurfer_core::cstd::double_to_int_x86(f64::from(count))
                                    .wrapping_add(1);
                            mri_set_vox_val(g, err, &mut counts, xk, yk, 0, 0, count as f32, 0)?;
                            let val = mri_get_vox_val(g, err, &mri, xk, yk, 0, h, 0)?;
                            let val_prev = mri_get_vox_val(g, err, &mri_prev, xp, yp, 0, hp, 0)?;
                            let Ok(w) =
                                weight(g, m, h + 1, v.wrapping_add(1), "SAElayerWeightsToMRI")
                            else {
                                let need = i64::from(ae_prev.v_hidden.rows)
                                    * i64::from(ae.ksize)
                                    * i64::from(ae.ksize);
                                weights_refused(g, err, "SAElayerWeightsToMRI", m, need);
                                return Ok(None);
                            };
                            // the official build: mulss into val_prev, then addss of val into
                            // it (val_prev's NaN first)
                            let val_prev = sse::mul(val_prev, w);
                            let sum = sse::add(val_prev, val);
                            mri_set_vox_val(g, err, &mut mri, xk, yk, 0, h, sum, 0)?;
                        }
                    }
                    v = v.wrapping_add(1);
                }
            }
        }
        for x in 0..counts.vg.width {
            for y in 0..counts.vg.height {
                let count = mri_get_vox_val(g, err, &counts, x, y, 0, 0, 0)?;
                let count = larmorx_freesurfer_core::cstd::double_to_int_x86(f64::from(count));
                if count > 0 {
                    let val = mri_get_vox_val(g, err, &mri, x, y, 0, h, 0)?;
                    mri_set_vox_val(g, err, &mut mri, x, y, 0, h, sse::div(val, count as f32), 0)?;
                }
            }
        }
        mri_clear(g, err, &mut counts)?;
    }
    Ok(Some(mri))
}

/// `mri_read`'s branch of type `MGH_AUTOENCODER` (mriio.cpp 695-704): the autoencoder
/// ([`sae_read`]; none: `MRIread(<name>): could not read autoencoder`, `ERROR_BADPARM`, nothing)
/// and the volume of layer `start_frame` ([`sae_layer_weights_to_mri`]); `mri_read` then takes
/// every frame of it.
#[allow(clippy::too_many_arguments)]
pub fn read_autoencoder(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    dir: Option<&Path>,
    fname: &str,
    start_frame: i32,
) -> Result<Option<Mri>, Fatal> {
    let Some(sae) = sae_read(g, env, out, err, dir, fname)? else {
        log::error_printf(
            g,
            err,
            ERROR_BADPARM,
            &format!("MRIread({fname}): could not read autoencoder\n"),
        );
        return Ok(None);
    };
    sae_layer_weights_to_mri(g, out, err, &sae, start_frame)
}

/// Registers [`read_autoencoder`] with fs.io, whose `mri_read` reads autoencoders through it.
pub fn register_io() {
    larmorx_freesurfer_io::upper::register(|u| {
        u.read_autoencoder = Some(read_autoencoder);
    });
}
