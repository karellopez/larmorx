// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! `skimage.segmentation.random_walker` (scikit-image 0.26.0,
//! `segmentation/random_walker_segmentation.py`) in its iterative modes, with the SciPy sparse
//! arithmetic under it (SciPy 1.18.1 and 1.15.3: `scipy/sparse/_compressed.py`, `_coo.py`,
//! `sparsetools/csr.h`, `scipy/sparse/linalg/_isolve/iterative.py`); notices in `super` and the
//! crate's root.
//!
//! **The system.** `_preprocess` ([`preprocess`]) renumbers the labels (`np.unique`'s inverse
//! minus the index of `0`), finds the seeds and unlabeled pixels that the others cannot reach
//! (`binary_propagation` with the cross structure) and, when some exist or a label is negative,
//! a mask of the pixels that take part. The graph has one edge per pair of neighbours along each
//! axis whose two ends take part; its weight is `-(exp(scale * g) + 1e-10)` where `g` is the sum
//! over the channels of `(diff / spacing)²` and `scale = -beta / (10 * data.std())` (divided by
//! `sqrt(channels)` for multichannel data). The Laplacian's diagonal is minus the sum of each
//! column, taken in increasing row order (SciPy's `ones @ lap` runs `csc_matvecs` over the
//! transpose). The system `A` is the Laplacian's rows and columns of the unlabeled pixels, the
//! right-hand side of label `l` the sum of the weights to the seeds of label `l` (`B @
//! seeds_mask`: `csr_matmat`, in increasing column order).
//!
//! **Types.** The gradients are computed in the type numpy promotes the data and the spacing to
//! (`float32` data and `float32` spacing, as nibabel's zooms: `float32`; a `float64` or integer
//! spacing, the default `np.ones(3)` included: `float64`); the weights, and so the whole linear
//! system and the conjugate gradients, in `float32` only when the gradients and the scale are
//! `float32` (multichannel data make the scale `float64`).
//!
//! **The order of the sums.** SciPy 1.15 adds the diagonal after the other entries of each row
//! (`setdiag` through COO, unsorted); SciPy 1.18 sorts each row afterwards, so the diagonal sits
//! between the smaller and larger columns ([`SparseOrder`]). The products of the conjugate
//! gradients' `A @ p` are added in that order, so the probabilities differ between the two
//! releases (relay 1's measurement: 74-90 % of the voxels of `brain_masker`'s probabilities).
//!
//! **The conjugate gradients** are SciPy's `cg` (`rtol=tol`, `atol=0`, `maxiter = 10 n`), with
//! the Jacobi preconditioner `1.0 / A.diagonal()` (`cg_j`) or none (`cg`), every inner product
//! and norm numpy's (`np.dot` over OpenBLAS's Haswell kernels, [`NpFloat::np_dot`]), every vector
//! update numpy's (`p *= beta; p += z`, `x += alpha * p`, `r -= alpha * q`, each a rounded
//! product then a rounded sum), the stopping test `norm(r) < atol` in the system's type. One
//! system per label; they are independent, so they run in parallel.
//!
//! **A defect kept behind a switch** ([`Numbering`]): with a mask, scikit-image numbers the
//! Laplacian's nodes by the ends of the remaining edges (`np.unique(edges)`) but the labels by the
//! mask; a pixel of the mask without a neighbour in it (a seed surrounded by pruned pixels) shifts
//! every later node, so the system pairs labels with the wrong pixels. larmorx numbers both by the
//! mask by default (the two agree whenever no such pixel exists).

use rayon::prelude::*;

use larmorx_core::numpy::{NpFloat, reduce};

use crate::morphology::{BinaryErosion, binary_erosion, dilation_arguments};
use crate::{Layout, NdimageError, generate_binary_structure};

/// The solver (`mode`): `'cg_j'` (conjugate gradients with the Jacobi preconditioner, the
/// default) or `'cg'` (none).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mode {
    /// `'cg'`.
    Cg,
    /// `'cg_j'`.
    CgJ,
}

/// The order of the entries within a row of SciPy's Laplacian (module documentation).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SparseOrder {
    /// SciPy 1.15: the diagonal last.
    Scipy115,
    /// SciPy 1.16 onwards (1.18.1): sorted.
    #[default]
    Scipy118,
}

/// How the Laplacian's nodes are numbered when a mask prunes pixels (module documentation).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Numbering {
    /// By the mask, as the labels are (larmorx's default).
    #[default]
    Mask,
    /// By the ends of the remaining edges, as scikit-image numbers them (`reproduce`).
    Edges,
}

/// Errors, with scikit-image's messages.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum RwError {
    /// `ValueError` of `_preprocess`.
    #[error(
        "No seeds provided in label image: please ensure it contains at least one positive value"
    )]
    NoSeeds,
    /// The arrays and the shape disagree.
    #[error("{0}")]
    Shape(String),
    /// More unlabeled pixels than 32-bit indices count.
    #[error("too many pixels for the random walker's index type")]
    TooLarge,
    /// The propagation of `_preprocess` failed.
    #[error(transparent)]
    Ndimage(#[from] NdimageError),
}

/// scikit-image's warnings of a solved system.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RwWarning {
    /// "Conjugate gradient convergence to tolerance not achieved. Consider decreasing beta to
    /// improve system conditionning."
    NotConverged,
    /// "The probability range is outside [0, 1] given the tolerance `prob_tol`. Consider
    /// decreasing `beta` and/or decreasing `tol`."
    ProbabilityRange,
}

/// The labels after `_preprocess`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Prepared {
    /// The renumbered labels (`inv_idx - zero_idx`), C-ordered like the input.
    pub labels: Vec<i64>,
    /// The number of positive labels.
    pub nlabels: usize,
    /// The pixels that take part, when some are pruned (`None`: all of them).
    pub mask: Option<Vec<bool>>,
    /// The unlabeled pixels no seed reaches (left at probability 0).
    pub isolated: Vec<bool>,
}

/// What `_preprocess` decided.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Preprocessed {
    /// No label is 0: scikit-image warns "Random walker only segments unlabeled areas, where
    /// labels == 0. No zero valued areas in labels were found. Returning provided labels." and
    /// returns the labels.
    NoZero,
    /// Every unlabeled pixel is isolated: scikit-image warns "All unlabeled pixels are isolated,
    /// they could not be determined by the random walker algorithm." and returns these labels
    /// (the input with `-1` on the isolated pixels).
    AllIsolated(Vec<i64>),
    /// A system to solve.
    Ready(Prepared),
}

/// `binary_propagation(input, mask=mask)` with the cross structure (`binary_dilation(...,
/// iterations=-1)`).
fn propagate(input: &[u8], mask: &[u8], shape: [usize; 3]) -> Result<Vec<u8>, NdimageError> {
    let s = generate_binary_structure(3, 1);
    let (rs, ro) = dilation_arguments(&s, &[0, 0, 0]);
    let layout = Layout::c(&shape);
    binary_erosion(
        input,
        &layout,
        BinaryErosion {
            structure: &rs,
            origins: &ro,
            iterations: -1,
            border_value: false,
            invert: true,
            brute_force: false,
        },
        Some((mask, &layout)),
    )
}

/// scikit-image's `_preprocess(labels)` of C-ordered labels of `shape` (a two-dimensional image
/// as `(rows, columns, 1)`).
pub fn preprocess(labels: &[i64], shape: [usize; 3]) -> Result<Preprocessed, RwError> {
    let n: usize = shape.iter().product();
    if labels.len() != n || n == 0 {
        return Err(RwError::Shape(
            "Incompatible data and labels shapes.".into(),
        ));
    }
    let mut values = labels.to_vec();
    values.sort_unstable();
    values.dedup();
    if *values.last().expect("not empty") <= 0 {
        return Err(RwError::NoSeeds);
    }
    let Ok(zero_idx) = values.binary_search(&0) else {
        return Ok(Preprocessed::NoZero);
    };
    let null: Vec<u8> = labels.iter().map(|&l| u8::from(l == 0)).collect();
    let mut pos: Vec<u8> = labels.iter().map(|&l| u8::from(l > 0)).collect();
    let mut mask: Vec<u8> = labels.iter().map(|&l| u8::from(l >= 0)).collect();
    let fill = propagate(&null, &mask, shape)?;
    let mut isolated: Vec<bool> = (0..n).map(|i| pos[i] != 0 && fill[i] == 0).collect();
    let any_isolated = isolated.iter().any(|&b| b);
    for i in 0..n {
        if isolated[i] {
            pos[i] = 0;
        }
    }
    let mask_out = if values[0] < 0 || any_isolated {
        let reach = propagate(&pos, &mask, shape)?;
        isolated = (0..n).map(|i| reach[i] == 0 && null[i] != 0).collect();
        if (0..n).filter(|&i| null[i] != 0).all(|i| isolated[i]) {
            let mut out = labels.to_vec();
            for i in 0..n {
                if isolated[i] {
                    out[i] = -1;
                }
            }
            return Ok(Preprocessed::AllIsolated(out));
        }
        for i in 0..n {
            if isolated[i] {
                mask[i] = 0;
            }
        }
        Some(mask.iter().map(|&m| m != 0).collect())
    } else {
        None
    };
    let renumbered = labels
        .iter()
        .map(|l| values.binary_search(l).expect("a label value") as i64 - zero_idx as i64)
        .collect();
    Ok(Preprocessed::Ready(Prepared {
        labels: renumbered,
        nlabels: values.len() - zero_idx - 1,
        mask: mask_out,
        isolated,
    }))
}

/// The spacing in its numpy type (`np.asarray(spacing)`): `float32` keeps a `float32` image's
/// gradients in `float32`; anything else (`float64`, integers, the default ones) makes them
/// `float64`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Spacing {
    /// A `float32` (or `float16`, or narrower integer) spacing.
    F32([f32; 3]),
    /// A `float64` (or wider integer) spacing.
    F64([f64; 3]),
}

/// The arguments of [`random_walker`] (scikit-image's, after its checks).
#[derive(Clone, Debug)]
pub struct RandomWalker<'a, D> {
    /// The image, C-ordered `(x, y, z, channels)` (a two-dimensional image with `z = 1`), as
    /// `img_as_float` leaves it (`float32` or `float64`).
    pub data: &'a [D],
    /// The spatial shape.
    pub shape: [usize; 3],
    /// The number of channels (1 without `channel_axis`).
    pub channels: usize,
    /// `data.std()` as numpy computed it on the caller's array (its memory order decides the
    /// order of numpy's sums); `None`: computed here from the C-ordered data.
    pub std: Option<D>,
    /// Whether `channel_axis` was given (the scale is then divided by `sqrt(channels)`).
    pub multichannel: bool,
    /// The spacing per axis.
    pub spacing: Spacing,
    /// `beta` (a Python number).
    pub beta: f64,
    /// The solver.
    pub mode: Mode,
    /// `tol`.
    pub tol: f64,
    /// `prob_tol`.
    pub prob_tol: f64,
    /// SciPy's order of the Laplacian's rows.
    pub order: SparseOrder,
    /// The numbering of a masked Laplacian.
    pub numbering: Numbering,
}

/// The solution of every label's system.
#[derive(Clone, Debug, PartialEq)]
pub struct RwOutput {
    /// `X`: for each label, the probability of every unlabeled pixel (in the order of
    /// [`RwOutput::unlabeled`]), in the system's type, widened to `f64`.
    pub x: Vec<Vec<f64>>,
    /// The unlabeled pixels of the system, in C order.
    pub unlabeled: Vec<usize>,
    /// Whether the system ran in `float32`.
    pub float32: bool,
    /// The conjugate gradients' iterations per label.
    pub iterations: Vec<usize>,
    /// scikit-image's warnings.
    pub warnings: Vec<RwWarning>,
    /// Whether scikit-image's numbering of the masked Laplacian (by the edges' ends) differs from
    /// the mask's here (a pixel of the mask without a neighbour in it, SKI-01): the result then
    /// depends on [`Numbering`].
    pub numbering_differs: bool,
}

impl RwOutput {
    /// `return_full_prob=True`: the `(nlabels, x, y, z)` C-ordered `float64` probabilities
    /// (each label's solution on the unlabeled pixels, 1 on its seeds, 0 elsewhere).
    pub fn full_prob(&self, prep: &Prepared) -> Vec<f64> {
        let n = prep.labels.len();
        let mut out = vec![0.0f64; prep.nlabels * n];
        for (l, row) in out.chunks_mut(n.max(1)).enumerate() {
            for (a, &g) in self.unlabeled.iter().enumerate() {
                row[g] = self.x[l][a];
            }
            for (g, &lab) in prep.labels.iter().enumerate() {
                if lab == l as i64 + 1 {
                    row[g] = 1.0;
                }
            }
        }
        out
    }

    /// `return_full_prob=False`: the renumbered labels with every unlabeled pixel given the
    /// label of largest probability (`np.argmax(X, axis=0) + 1`: the first largest, a NaN
    /// first of all).
    pub fn labels(&self, prep: &Prepared) -> Vec<i64> {
        let mut out = prep.labels.clone();
        for (a, &g) in self.unlabeled.iter().enumerate() {
            let mut best = 0usize;
            let mut bv = self.x[0][a];
            if !bv.is_nan() {
                for l in 1..prep.nlabels {
                    let v = self.x[l][a];
                    if v.is_nan() {
                        best = l;
                        break;
                    }
                    if v > bv {
                        best = l;
                        bv = v;
                    }
                }
            }
            out[g] = best as i64 + 1;
        }
        out
    }
}

/// The unlabeled rows of the system: at most seven entries each (six neighbours and the
/// diagonal), in SciPy's order.
struct Ell<W> {
    cols: Vec<u32>,
    vals: Vec<W>,
    lens: Vec<u8>,
}

const NONE: u32 = u32::MAX;

/// numpy's unary minus (a sign flip: `-(+0)` is `-0`, a NaN keeps its payload).
#[inline(always)]
fn neg<W: NpFloat>(v: W) -> W {
    W::from_f64(-v.to_f64())
}

/// `scikit-image`'s `random_walker` after `_preprocess`: the system of every label solved
/// (module documentation). Runs on the threads of the current rayon pool; the result does not
/// depend on their number.
pub fn random_walker<D: NpFloat>(
    rw: &RandomWalker<'_, D>,
    prep: &Prepared,
) -> Result<RwOutput, RwError> {
    let n: usize = rw.shape.iter().product();
    if rw.channels == 0 || rw.data.len() != n * rw.channels || prep.labels.len() != n {
        return Err(RwError::Shape(
            "Incompatible data and labels shapes.".into(),
        ));
    }
    let d32 = std::mem::size_of::<D>() == 4;
    match rw.spacing {
        Spacing::F32(s) if d32 => {
            if rw.multichannel {
                solve::<D, f32, f64>(rw, prep, s)
            } else {
                solve::<D, f32, f32>(rw, prep, s)
            }
        }
        Spacing::F32(s) => solve::<D, f64, f64>(rw, prep, s.map(f64::from)),
        Spacing::F64(s) => solve::<D, f64, f64>(rw, prep, s),
    }
}

/// The C-order neighbours of `g` in increasing index order: `(neighbour, edge index, axis)`,
/// where the edge's weight is stored at the smaller end.
#[inline]
fn neighbours(g: usize, shape: [usize; 3], out: &mut [(usize, usize, usize); 6]) -> usize {
    let [nx, ny, nz] = shape;
    let (sx, sy) = (ny * nz, nz);
    let k = g % nz;
    let j = (g / nz) % ny;
    let i = g / sx;
    let mut c = 0;
    if i > 0 {
        out[c] = (g - sx, g - sx, 0);
        c += 1;
    }
    if j > 0 {
        out[c] = (g - sy, g - sy, 1);
        c += 1;
    }
    if k > 0 {
        out[c] = (g - 1, g - 1, 2);
        c += 1;
    }
    if k + 1 < nz {
        out[c] = (g + 1, g, 2);
        c += 1;
    }
    if j + 1 < ny {
        out[c] = (g + sy, g, 1);
        c += 1;
    }
    if i + 1 < nx {
        out[c] = (g + sx, g, 0);
        c += 1;
    }
    c
}

fn solve<D: NpFloat, G: NpFloat, W: NpFloat>(
    rw: &RandomWalker<'_, D>,
    prep: &Prepared,
    spacing: [G; 3],
) -> Result<RwOutput, RwError> {
    let shape = rw.shape;
    let n: usize = shape.iter().product();
    let c = rw.channels;
    let strides = [shape[1] * shape[2], shape[2], 1];
    let mask = prep.mask.as_deref();
    let in_mask = |g: usize| mask.is_none_or(|m| m[g]);

    // The weights: scale = -beta / (10 * data.std()) in the data's type.
    let std = rw
        .std
        .unwrap_or_else(|| reduce::std_axis(rw.data, [1, n * c, 1], 0)[0]);
    let scale_d = D::from_f64(-rw.beta).div(D::from_f64(10.0).mul(std));
    let scale = if rw.multichannel {
        W::from_f64(scale_d.to_f64() / (c as f64).sqrt())
    } else {
        W::from_f64(scale_d.to_f64())
    };
    let eps = W::from_f64(1.0e-10);
    let data = rw.data;
    // lapval[ax][g]: the Laplacian's entry of the edge (g, g + stride[ax]).
    let lapval: Vec<Vec<W>> = (0..3)
        .map(|ax| {
            let s = strides[ax];
            (0..n)
                .into_par_iter()
                .with_min_len(4096)
                .map(|g| {
                    let idx = [g / strides[0], (g / strides[1]) % shape[1], g % shape[2]];
                    if idx[ax] + 1 >= shape[ax] || !in_mask(g) || !in_mask(g + s) {
                        return W::ZERO;
                    }
                    let mut grad = G::ZERO;
                    for ch in 0..c {
                        let diff = data[(g + s) * c + ch].sub(data[g * c + ch]);
                        let q = G::from_f64(diff.to_f64()).div(spacing[ax]);
                        let sq = q.mul(q);
                        grad = if ch == 0 { sq } else { grad.add(sq) };
                    }
                    let w = scale.mul(W::from_f64(grad.to_f64())).np_exp().add(eps);
                    neg(w)
                })
                .collect()
        })
        .collect();

    // System positions (the labels' numbering) and Laplacian indices.
    let (sys_nodes, n_sys): (Option<Vec<usize>>, usize) = match mask {
        None => (None, n),
        Some(m) => {
            let nodes: Vec<usize> = (0..n).filter(|&g| m[g]).collect();
            let len = nodes.len();
            (Some(nodes), len)
        }
    };
    if n_sys >= NONE as usize {
        return Err(RwError::TooLarge);
    }
    let node_of_sys = |k: usize| sys_nodes.as_ref().map_or(k, |v| v[k]);
    let labels_sys: Vec<i64> = (0..n_sys).map(|k| prep.labels[node_of_sys(k)]).collect();
    // lap_of[g]: the Laplacian index of grid node g; lap_node[k]: its node.
    let has_edge = |g: usize| {
        let mut nb = [(0, 0, 0); 6];
        let m = neighbours(g, shape, &mut nb);
        nb[..m].iter().any(|&(h, _, _)| in_mask(h))
    };
    let (lap_of, lap_node): (Vec<u32>, Vec<usize>) = match (mask, rw.numbering) {
        (None, _) => ((0..n as u32).collect(), (0..n).collect()),
        (Some(_), Numbering::Mask) => {
            let mut of = vec![NONE; n];
            let nodes = sys_nodes.clone().expect("a mask");
            for (k, &g) in nodes.iter().enumerate() {
                of[g] = k as u32;
            }
            (of, nodes)
        }
        (Some(m), Numbering::Edges) => {
            let mut of = vec![NONE; n];
            let mut nodes = Vec::new();
            for g in 0..n {
                if m[g] && has_edge(g) {
                    of[g] = nodes.len() as u32;
                    nodes.push(g);
                }
            }
            (of, nodes)
        }
    };
    let n_lap = lap_node.len();
    let numbering_differs = match mask {
        None => false,
        Some(m) => (0..n).any(|g| m[g] && !has_edge(g)),
    };
    let mut unl_pos = vec![NONE; n_sys];
    let mut unl: Vec<usize> = Vec::new();
    for (k, &l) in labels_sys.iter().enumerate() {
        if l == 0 {
            unl_pos[k] = unl.len() as u32;
            unl.push(k);
        }
    }
    let u = unl.len();
    let nlabels = prep.nlabels;

    // The rows of A, the diagonal of A (`A.diagonal()`) and the right-hand sides (`rhs[a *
    // nlabels + l]`).
    let mut ell = Ell {
        cols: vec![0u32; 7 * u],
        vals: vec![W::ZERO; 7 * u],
        lens: vec![0u8; u],
    };
    let mut diag = vec![W::ZERO; u];
    let mut rhs_flat = vec![W::ZERO; u * nlabels];
    ell.cols
        .par_chunks_mut(7)
        .zip(ell.vals.par_chunks_mut(7))
        .zip(ell.lens.par_iter_mut())
        .zip(diag.par_iter_mut())
        .zip(rhs_flat.par_chunks_mut(nlabels.max(1)))
        .enumerate()
        .with_min_len(1024)
        .for_each(|(a, ((((cols, vals), len), dg), b))| {
            let k = unl[a];
            if k >= n_lap {
                // an empty row of the misnumbered Laplacian: its diagonal is -(+0) = -0
                let d = neg(W::ZERO);
                cols[0] = a as u32;
                vals[0] = d;
                *len = 1;
                *dg = W::ZERO.add(d);
                return;
            }
            let g = lap_node[k];
            let mut nb = [(0, 0, 0); 6];
            let m = neighbours(g, shape, &mut nb);
            let mut colsum = W::ZERO;
            let mut entries = [(0usize, W::ZERO); 6];
            let mut ne = 0;
            for &(h, e, ax) in &nb[..m] {
                if !in_mask(h) {
                    continue;
                }
                let v = lapval[ax][e];
                colsum = colsum.add(v);
                let kh = lap_of[h] as usize;
                let lbl = labels_sys[kh];
                if lbl == 0 {
                    entries[ne] = (unl_pos[kh] as usize, v);
                    ne += 1;
                } else if lbl > 0 {
                    // B = -rows[:, seeds]; csr_matmat adds each label's products from +0
                    let li = lbl as usize - 1;
                    b[li] = b[li].add(neg(v));
                }
            }
            let d = neg(colsum);
            let mut t = 0;
            let mut push = |col: usize, v: W| {
                cols[t] = col as u32;
                vals[t] = v;
                t += 1;
            };
            match rw.order {
                SparseOrder::Scipy115 => {
                    for &(col, v) in &entries[..ne] {
                        push(col, v);
                    }
                    push(a, d);
                }
                SparseOrder::Scipy118 => {
                    let mut placed = false;
                    for &(col, v) in &entries[..ne] {
                        if !placed && col > a {
                            push(a, d);
                            placed = true;
                        }
                        push(col, v);
                    }
                    if !placed {
                        push(a, d);
                    }
                }
            }
            *len = t as u8;
            *dg = W::ZERO.add(d);
        });
    let rhs: Vec<Vec<W>> = (0..nlabels)
        .map(|l| (0..u).map(|a| rhs_flat[a * nlabels + l]).collect())
        .collect();
    drop(rhs_flat);
    drop(lapval);

    let minv: Option<Vec<W>> = match rw.mode {
        Mode::CgJ => Some(diag.iter().map(|&d| W::from_f64(1.0).div(d)).collect()),
        Mode::Cg => None,
    };
    let solved: Vec<(Vec<W>, usize, usize)> = rhs
        .into_par_iter()
        .map(|b| cg(&ell, minv.as_deref(), b, rw.tol))
        .collect();
    let mut warnings = Vec::new();
    if solved.iter().any(|s| s.1 > 0) {
        warnings.push(RwWarning::NotConverged);
    }
    // X.min() < -prob_tol or X.max() > 1 + prob_tol (numpy's min and max propagate a NaN).
    let mut lo: Option<W> = None;
    let mut hi: Option<W> = None;
    let mut nan = false;
    for (x, _, _) in &solved {
        for &v in x {
            if v.is_nan() {
                nan = true;
            }
            if lo.is_none_or(|l| v < l) {
                lo = Some(v);
            }
            if hi.is_none_or(|h| v > h) {
                hi = Some(v);
            }
        }
    }
    if !nan
        && let (Some(lo), Some(hi)) = (lo, hi)
        && (lo < W::from_f64(-rw.prob_tol) || hi > W::from_f64(1.0 + rw.prob_tol))
    {
        warnings.push(RwWarning::ProbabilityRange);
    }
    let unlabeled = unl.iter().map(|&k| node_of_sys(k)).collect();
    Ok(RwOutput {
        x: solved
            .iter()
            .map(|(x, _, _)| x.iter().map(|v| v.to_f64()).collect())
            .collect(),
        unlabeled,
        float32: std::mem::size_of::<W>() == 4,
        iterations: solved.iter().map(|s| s.2).collect(),
        warnings,
        numbering_differs,
    })
}

/// `y = A @ p` (`csr_matvecs`: each row's products added in the row's order, from +0).
fn matvec<W: NpFloat>(a: &Ell<W>, p: &[W], y: &mut [W]) {
    y.par_iter_mut()
        .enumerate()
        .with_min_len(2048)
        .for_each(|(i, yi)| {
            let mut sum = W::ZERO;
            for t in 7 * i..7 * i + a.lens[i] as usize {
                sum = sum.add(a.vals[t].mul(p[a.cols[t] as usize]));
            }
            *yi = sum;
        });
}

/// SciPy's `cg(A, b, rtol=tol, atol=0, M=M)` from x0 = 0: the solution, `info` and the
/// iterations run.
fn cg<W: NpFloat>(a: &Ell<W>, minv: Option<&[W]>, b: Vec<W>, rtol: f64) -> (Vec<W>, usize, usize) {
    let n = b.len();
    let bnrm2 = W::np_dot(&b, &b).sqrt();
    // _get_atol_rtol: max(float(atol), float(rtol) * float(b_norm)) with atol = 0.
    let cand = rtol * bnrm2.to_f64();
    let atol = if cand > 0.0 { cand } else { 0.0 };
    if bnrm2 == W::ZERO {
        return (b, 0, 0);
    }
    let maxiter = 10 * n;
    // `norm(r) < atol`: numpy compares a float32 norm with the Python float in float32.
    let atol_w = W::from_f64(atol);
    let mut x = vec![W::ZERO; n];
    let mut r = b;
    let mut p = vec![W::ZERO; n];
    let mut q = vec![W::ZERO; n];
    let mut z = vec![W::ZERO; n];
    let mut rho_prev = W::ZERO;
    for iteration in 0..maxiter {
        if W::np_dot(&r, &r).sqrt() < atol_w {
            return (x, 0, iteration);
        }
        // z = psolve(r): the Jacobi preconditioner's `0 + m * r` (dia_matvec), or r itself.
        let zr: &[W] = match minv {
            Some(m) => {
                z.par_iter_mut()
                    .zip(m)
                    .zip(&r)
                    .with_min_len(8192)
                    .for_each(|((zi, &mi), &ri)| *zi = W::ZERO.add(mi.mul(ri)));
                &z
            }
            None => &r,
        };
        let rho_cur = W::np_dot(&r, zr);
        if iteration > 0 {
            let beta = rho_cur.div(rho_prev);
            p.par_iter_mut()
                .zip(zr)
                .with_min_len(8192)
                .for_each(|(pi, &zi)| *pi = pi.mul(beta).add(zi));
        } else {
            p.copy_from_slice(zr);
        }
        matvec(a, &p, &mut q);
        let alpha = rho_cur.div(W::np_dot(&p, &q));
        x.par_iter_mut()
            .zip(&p)
            .with_min_len(8192)
            .for_each(|(xi, &pi)| *xi = xi.add(alpha.mul(pi)));
        r.par_iter_mut()
            .zip(&q)
            .with_min_len(8192)
            .for_each(|(ri, &qi)| *ri = ri.sub(alpha.mul(qi)));
        rho_prev = rho_cur;
    }
    (x, maxiter, maxiter)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preprocess_renumbers_and_finds_no_zero() {
        let labels = [0i64, 5, 0, 9];
        let Preprocessed::Ready(p) = preprocess(&labels, [1, 2, 2]).unwrap() else {
            panic!("a system")
        };
        assert_eq!(p.labels, vec![0, 1, 0, 2]);
        assert_eq!(p.nlabels, 2);
        assert!(p.mask.is_none());
        assert_eq!(
            preprocess(&[1, 2], [1, 1, 2]).unwrap(),
            Preprocessed::NoZero
        );
        assert_eq!(preprocess(&[0, -1], [1, 1, 2]), Err(RwError::NoSeeds));
    }

    #[test]
    fn two_seeds_on_a_line() {
        // 1 . . . 2: the probabilities of label 1 fall from 1 to 0 along the line.
        let data = [0.0f32, 0.1, 0.2, 0.3, 0.4];
        let labels = [1i64, 0, 0, 0, 2];
        let Preprocessed::Ready(p) = preprocess(&labels, [1, 1, 5]).unwrap() else {
            panic!("a system")
        };
        for order in [SparseOrder::Scipy115, SparseOrder::Scipy118] {
            let rw = RandomWalker {
                data: &data,
                shape: [1, 1, 5],
                channels: 1,
                std: None,
                multichannel: false,
                spacing: Spacing::F32([1.0; 3]),
                beta: 130.0,
                mode: Mode::CgJ,
                tol: 1e-3,
                prob_tol: 1e-3,
                order,
                numbering: Numbering::Mask,
            };
            let out = random_walker(&rw, &p).unwrap();
            assert!(out.float32);
            assert_eq!(out.unlabeled, vec![1, 2, 3]);
            let x = &out.x[0];
            assert!(x[0] > x[1] && x[1] > x[2]);
            let full = out.full_prob(&p);
            assert_eq!(full[0], 1.0);
            assert_eq!(full[5 + 4], 1.0);
        }
    }
}
