// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `utils/gifti.cpp` 47-212: FreeSurfer's two helpers over gifticlib's structures
//! (`larmorx_io::gifti`, track C's port of gifticlib 1.12), `gifti_alloc_and_add_darray` and
//! `gifti_get_DA_value_2D` (both `static` in FreeSurfer: its readers and writers of GIFTI
//! surfaces, the rest of `gifti.cpp`, call them; queue item 3.1-26 brings those). Messages go to
//! standard error, as `fprintf(stderr, ...)`.
//!
//! Defects (`docs/findings/freesurfer-io.md`, Part 3): FS-214 (the address printed for an array
//! without data: `(nil)` in both modes), FS-215 (COMPLEX64 values read as doubles: refused by
//! default as an unsupported type, rule B; `reproduce` the double), FS-216 (a 32-bit element
//! place: 64-bit in both modes). A signed `char` reads `NIFTI_TYPE_INT8` (x86-64 Linux's `char`,
//! the official build's; AArch64 Linux's is unsigned, not FreeSurfer's official platform).

use larmorx_core::cfmt::Streams;
use larmorx_core::defects::{FS_214, FS_215, FS_216};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_io::gifti::{
    DataArray, GIFTI_IND_ORD_COL_MAJOR, GIFTI_IND_ORD_ROW_MAJOR, Gifti, GiftiImage,
    NIFTI_TYPE_COMPLEX64, NIFTI_TYPE_FLOAT32, NIFTI_TYPE_FLOAT64, NIFTI_TYPE_INT8,
    NIFTI_TYPE_INT16, NIFTI_TYPE_INT32, NIFTI_TYPE_UINT8, NIFTI_TYPE_UINT16, NIFTI_TYPE_UINT32,
};

/// `gifti_alloc_and_add_darray(image)`: a new empty data array added at the end of `image`
/// (`gifti_add_empty_darray(image, 1)`, its messages at gifticlib's verbosity), or `None` after
/// `** gifti_alloc_and_add_darray: NULL image` (no image) or the add's failure.
pub fn gifti_alloc_and_add_darray<'a>(
    gx: &Gifti,
    image: Option<&'a mut GiftiImage>,
    out: &mut dyn Streams,
) -> Option<&'a mut DataArray> {
    let Some(image) = image else {
        out.err("** gifti_alloc_and_add_darray: NULL image\n");
        return None;
    };
    if gx.add_empty_darray(image, 1, out) != 0 {
        out.err("** gifti_alloc_and_add_darray: gifti_add_empty_darray failed\n");
        return None;
    }
    let last = (image.num_da as usize).checked_sub(1)?;
    image.darray.get_mut(last)?.as_mut()
}

fn fail(out: &mut dyn Streams, text: &str) -> Fatal {
    out.err(text);
    Fatal::Exit(1)
}

/// The `n` little-endian bytes of `data` at element `k`; zeros beyond the bytes (the caller's
/// array must hold its dimensions' values, as gifticlib's always do).
fn element<const N: usize>(data: &[u8], k: i64) -> [u8; N] {
    let mut b = [0u8; N];
    if k >= 0 {
        let at = (k as u128) * (N as u128);
        for (i, x) in b.iter_mut().enumerate() {
            if let Some(&v) = usize::try_from(at + i as u128)
                .ok()
                .and_then(|j| data.get(j))
            {
                *x = v;
            }
        }
    }
    b
}

/// `gifti_get_DA_value_2D(da, row, col)`: the value at (`row`, `col`) of a one- or
/// two-dimensional data array, as a double. FreeSurfer's failures end the program with status 1
/// after a message on standard error: no array or no data (`invalid params: data=(nil)`), a
/// one-dimensional array asked for a column other than 0, an array of another dimension, an
/// unknown index order, an index outside the array, a type it does not convert.
pub fn gifti_get_da_value_2d(
    g: &mut Globals,
    out: &mut dyn Streams,
    da: Option<&DataArray>,
    row: i32,
    col: i32,
) -> Result<f64, Fatal> {
    let data = match da.map(|d| d.data.as_deref()) {
        Some(Some(data)) => data,
        found => {
            if found.is_some() {
                g.notices.add(
                    &FS_214,
                    "a data array without data: FreeSurfer's message prints the array's memory address (it differs from run to run); printed as (nil)",
                );
            }
            return Err(fail(
                out,
                "** gifti_get_DA_value_2D, invalid params: data=(nil)\n",
            ));
        }
    };
    let da = da.expect("an array with data");
    let (dims_0, dims_1) = if da.num_dim == 1 {
        if col != 0 {
            return Err(fail(
                out,
                &format!(
                    "** gifti_get_DA_value_2D, array dim is 1 but trying to access 2D data element (col={col})\n"
                ),
            ));
        }
        (da.dims[0], 1)
    } else if da.num_dim != 2 {
        return Err(fail(
            out,
            &format!("** gifti_get_DA_value_2D, array dim is {}\n", da.num_dim),
        ));
    } else {
        (da.dims[0], da.dims[1])
    };
    if da.ind_ord != GIFTI_IND_ORD_ROW_MAJOR && da.ind_ord != GIFTI_IND_ORD_COL_MAJOR {
        return Err(fail(
            out,
            &format!(
                "** gifti_get_DA_value_2D, unknown ind_ord: {}\n",
                da.ind_ord
            ),
        ));
    }
    let (dim0, dim1) = (row, col);
    if dim0 < 0 || dim0 >= dims_0 || dim1 < 0 || dim1 >= dims_1 {
        return Err(fail(
            out,
            &format!(
                "** gifti_get_DA_value_2D, invalid params: dim0_index={dim0} (max={dims_0}), dim1_index={dim1} (max={dims_1})\n"
            ),
        ));
    }
    let row_major = da.ind_ord == GIFTI_IND_ORD_ROW_MAJOR;
    // FreeSurfer's place: the product in 32 bits, then added to the pointer
    let (k, k32) = if row_major {
        (
            i64::from(dim0) * i64::from(dims_1) + i64::from(dim1),
            i64::from(dim0.wrapping_mul(dims_1)) + i64::from(dim1),
        )
    } else {
        (
            i64::from(dim0) + i64::from(dim1) * i64::from(dims_0),
            i64::from(dim0) + i64::from(dim1.wrapping_mul(dims_0)),
        )
    };
    let supported = [
        NIFTI_TYPE_UINT8,
        NIFTI_TYPE_INT16,
        NIFTI_TYPE_INT32,
        NIFTI_TYPE_FLOAT32,
        NIFTI_TYPE_FLOAT64,
        NIFTI_TYPE_COMPLEX64,
        NIFTI_TYPE_INT8,
        NIFTI_TYPE_UINT16,
        NIFTI_TYPE_UINT32,
    ];
    let unsupported = |out: &mut dyn Streams| {
        fail(
            out,
            &format!(
                "** gifti_get_DA_value_2D, unsupported type {}-unknown, or can't convert to double\n",
                da.datatype
            ),
        )
    };
    if !supported.contains(&da.datatype) {
        return Err(unsupported(out));
    }
    if da.datatype == NIFTI_TYPE_COMPLEX64 {
        let reproduce = g.notices.reproduce();
        g.notices.add(
            &FS_215,
            format!(
                "a COMPLEX64 array: FreeSurfer reads its values as 8-byte doubles{}",
                if reproduce {
                    ""
                } else {
                    "; refused as a type it cannot convert"
                }
            ),
        );
        if !reproduce {
            return Err(unsupported(out));
        }
    }
    if k32 != k {
        g.notices.add(
            &FS_216,
            format!(
                "element {k} of a GIFTI array, whose place FreeSurfer computes in 32 bits ({k32}); read at {k}"
            ),
        );
    }
    Ok(match da.datatype {
        NIFTI_TYPE_UINT8 => f64::from(element::<1>(data, k)[0]),
        NIFTI_TYPE_INT16 => f64::from(i16::from_le_bytes(element(data, k))),
        NIFTI_TYPE_INT32 => f64::from(i32::from_le_bytes(element(data, k))),
        NIFTI_TYPE_FLOAT32 => f64::from(f32::from_le_bytes(element(data, k))),
        NIFTI_TYPE_FLOAT64 | NIFTI_TYPE_COMPLEX64 => f64::from_le_bytes(element(data, k)),
        NIFTI_TYPE_INT8 => f64::from(element::<1>(data, k)[0] as i8),
        NIFTI_TYPE_UINT16 => f64::from(u16::from_le_bytes(element(data, k))),
        _ => f64::from(u32::from_le_bytes(element(data, k))),
    })
}
