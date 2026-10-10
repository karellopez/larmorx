// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Karel Lopez Vilaret.
//! Small 4D NIfTI files for the command-line tests.

#![allow(dead_code)]

use std::path::Path;
use std::process::{Command, Output};

use larmorx_core::affine::Affine;
use larmorx_core::element::{DataType, Element};
use larmorx_core::ndarray::{ArrayViewD, IxDyn, ShapeBuilder};
use larmorx_io::nifti::{self, NiftiHeader, NiftiVersion, WriteOptions};

pub const SHAPE: [usize; 4] = [5, 3, 4, 24];

/// Smooth series with a trend and some noise-like detail, different in every voxel.
pub fn values() -> Vec<f64> {
    let n: usize = SHAPE.iter().product();
    let nvox = SHAPE[0] * SHAPE[1] * SHAPE[2];
    (0..n)
        .map(|i| {
            let (v, t) = ((i % nvox) as f64, (i / nvox) as f64);
            800.0
                + 3.0 * v
                + 40.0 * (0.4 * t + 0.3 * v).sin()
                + 2.5 * t
                + 7.0 * ((i * 7919 % 101) as f64 / 101.0)
        })
        .collect()
}

/// Writes a 4D file of `data` (TR 2 s), with `scl_slope` = `slope`.
pub fn write<T: Element>(path: &Path, data: &[T], data_type: DataType, slope: f64) {
    let mut h = NiftiHeader::new(NiftiVersion::V1, &SHAPE, data_type).unwrap();
    let affine = Affine::from_zooms([2.0, 2.0, 3.0], [-4.0, -2.0, -6.0]);
    h.set_sform(&affine, 1);
    h.set_qform(&affine, 1).unwrap();
    h.pixdim[4] = 2.0;
    h.xyzt_units = 2 | 8;
    h.scl_slope = slope;
    h.scl_inter = 0.0;
    let view = ArrayViewD::from_shape(IxDyn(&SHAPE).f(), data).unwrap();
    nifti::write(path, &h, view, &WriteOptions::default()).unwrap();
}

pub fn float32(path: &Path) {
    let v: Vec<f32> = values().iter().map(|&x| x as f32).collect();
    write(path, &v, DataType::F32, 0.0);
}

pub fn int16_scaled(path: &Path) {
    let v: Vec<i16> = values().iter().map(|&x| (x * 4.0).round() as i16).collect();
    write(path, &v, DataType::I16, 0.25);
}

pub fn run(exe: &str, args: &[&str]) -> Output {
    Command::new(exe).args(args).output().unwrap()
}

/// The stored data bytes and the header of a NIfTI file.
pub fn read(path: &Path) -> (Vec<u8>, NiftiHeader) {
    let image = nifti::read(
        path,
        &nifti::ReadOptions {
            scaling: nifti::Scaling::Raw,
            n_threads: 1,
        },
    )
    .unwrap();
    let bytes = match &image.data {
        larmorx_core::array::DynArray::F32(a) => {
            a.t().iter().flat_map(|v| v.to_le_bytes()).collect()
        }
        larmorx_core::array::DynArray::I16(a) => {
            a.t().iter().flat_map(|v| v.to_le_bytes()).collect()
        }
        larmorx_core::array::DynArray::U8(a) => a.t().iter().copied().collect(),
        other => panic!("unexpected data type {}", other.data_type()),
    };
    (bytes, image.header)
}
