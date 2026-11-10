// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! Labelling and per-label sums, ported from SciPy 1.18.1 `scipy/ndimage/src/_ni_label.pyx`
//! (`_label`) and `scipy/ndimage/_measurements.py` (`label`, `_stats`, `sum_labels`);
//! BSD-3-Clause, the notice is in the crate's root.

use crate::{Footprint, Layout, NdElement, NdimageError, c_strides, unravel};

/// The labels of [`label`]: SciPy's default output type, `int32`, unless the array has `2^31 -
/// 2` elements or more (`intp`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Labels {
    /// `int32` labels, C-ordered.
    I32(Vec<i32>),
    /// `int64` (`intp`) labels, C-ordered.
    I64(Vec<i64>),
}

/// The root of `a` in the union-find `parent` (with path halving).
fn find(parent: &mut [usize], mut a: usize) -> usize {
    while parent[a] != a {
        parent[a] = parent[parent[a]];
        a = parent[a];
    }
    a
}

/// `scipy.ndimage.label(input, structure)`: the connected components of the non-zero elements
/// under the `3^n` symmetric `structure`, numbered from 1 in the order SciPy's `_ni_label`
/// numbers them, by the first element of each in C order (its lines run along the output's
/// last axis, in C order of the others); the background is 0. Returns the labels (C-ordered)
/// and their number.
pub fn label<T: NdElement>(
    input: &[T],
    layout: &Layout,
    structure: &Footprint,
) -> Result<(Labels, usize), NdimageError> {
    let ndim = layout.ndim();
    if structure.shape.len() != ndim {
        return Err(NdimageError::Shape(
            "structure and input must have equal rank".into(),
        ));
    }
    if structure.shape.iter().any(|&s| s != 3) {
        return Err(NdimageError::LabelStructure);
    }
    if structure.mask.iter().ne(structure.mask.iter().rev()) {
        return Err(NdimageError::LabelSymmetry);
    }
    if ndim == 0 || layout.is_empty() {
        return Err(NdimageError::LabelEmpty);
    }
    if input.len() < layout.span() {
        return Err(NdimageError::Shape(
            "input buffer and layout disagree".into(),
        ));
    }
    let n = layout.len();
    // The earlier neighbours in C order: the structure's elements before its centre.
    let centre = (structure.mask.len() - 1) / 2;
    let mut k = vec![0usize; ndim];
    let earlier: Vec<Vec<isize>> = (0..centre)
        .filter(|&lin| structure.mask[lin])
        .map(|lin| {
            unravel(lin, &structure.shape, &mut k);
            k.iter().map(|&i| i as isize - 1).collect()
        })
        .collect();
    let ostrides = c_strides(&layout.shape);
    // provisional labels (0 = background), stored per element in C order
    let mut prov = vec![0usize; n];
    let mut parent: Vec<usize> = vec![0];
    let mut p = vec![0usize; ndim];
    for lin in 0..n {
        unravel(lin, &layout.shape, &mut p);
        if !input[layout.offset(&p)].nonzero() {
            continue;
        }
        let mut cur = 0usize;
        'nb: for dlt in &earlier {
            let mut off = 0usize;
            for d in 0..ndim {
                let c = p[d] as isize + dlt[d];
                if c < 0 || c >= layout.shape[d] as isize {
                    continue 'nb;
                }
                off += c as usize * ostrides[d];
            }
            let l = prov[off];
            if l == 0 {
                continue;
            }
            if cur == 0 {
                cur = find(&mut parent, l);
            } else {
                let (a, b) = (find(&mut parent, cur), find(&mut parent, l));
                if a != b {
                    let (lo, hi) = if a < b { (a, b) } else { (b, a) };
                    parent[hi] = lo;
                    cur = lo;
                }
            }
        }
        if cur == 0 {
            cur = parent.len();
            parent.push(cur);
        }
        prov[lin] = cur;
    }
    // the final numbers: the roots in increasing order (each root is the provisional label of
    // its component's first element in C order)
    let mut number = vec![0usize; parent.len()];
    let mut count = 0usize;
    for l in 1..parent.len() {
        let r = find(&mut parent, l);
        if r == l {
            count += 1;
            number[l] = count;
        } else {
            number[l] = number[r];
        }
    }
    let wide = n >= (1usize << 31) - 2;
    let labels = if wide {
        Labels::I64(prov.iter().map(|&l| number[l] as i64).collect())
    } else {
        Labels::I32(prov.iter().map(|&l| number[l] as i32).collect())
    };
    Ok((labels, count))
}

/// An integer label type for [`sum_labels_bincount`].
pub trait LabelValue: Copy + Send + Sync {
    /// The value as `i64` (C's conversion; `u64` labels above `i64::MAX` are not supported, as
    /// SciPy's `bincount` path does not take them either).
    fn as_i64(self) -> i64;
}

macro_rules! label_value {
    ($($t:ty),*) => {$(
        impl LabelValue for $t {
            #[inline]
            fn as_i64(self) -> i64 {
                self as i64
            }
        }
    )*};
}
label_value!(u8, i8, u16, i16, u32, i32, u64, i64);

/// `scipy.ndimage.sum_labels(input, labels, index)` for a sequence `index` (`_stats`'s
/// `bincount` and `unique` paths, which give the same sums): per label value, the sum in
/// `float64` of the input's elements carrying it, added one by one in C order; an index without
/// any element gives 0. `labels` has the input's shape (its own layout).
pub fn sum_labels_bincount<T: NdElement, L: LabelValue>(
    input: &[T],
    layout: &Layout,
    labels: &[L],
    labels_layout: &Layout,
    index: &[i64],
) -> Result<Vec<f64>, NdimageError> {
    if labels_layout.shape != layout.shape
        || input.len() < layout.span()
        || labels.len() < labels_layout.span()
    {
        return Err(NdimageError::Shape(
            "input and labels must have the same shape".into(),
        ));
    }
    let wanted: std::collections::HashMap<i64, usize> =
        index.iter().enumerate().map(|(i, &v)| (v, i)).collect();
    let mut sums: std::collections::HashMap<i64, f64> = std::collections::HashMap::new();
    let n = layout.len();
    let mut p = vec![0usize; layout.ndim()];
    for lin in 0..n {
        unravel(lin, &layout.shape, &mut p);
        let l = labels[labels_layout.offset(&p)].as_i64();
        if !wanted.contains_key(&l) {
            continue;
        }
        let v = input[layout.offset(&p)].to_f64();
        let s = sums.entry(l).or_insert(0.0);
        *s = larmorx_core::numpy::NpFloat::add(*s, v);
    }
    Ok(index
        .iter()
        .map(|i| sums.get(i).copied().unwrap_or(0.0))
        .collect())
}
