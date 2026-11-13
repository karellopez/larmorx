// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `validation/investigations/glibc-stdio-probe/scanf_end.c`'s 20 formats as [`Scanf`]
//! directives (shared by fs.core's `sscanf` test and fs.io's `fscanf` test).

use larmorx_freesurfer_core::cscan::Scanf;

/// `scanf_end.c`'s formats, directive by directive.
pub fn format(id: u32, sc: &mut Scanf, e: &mut i32) {
    const W: usize = usize::MAX;
    match id {
        0 => {
            sc.d(e, true);
            sc.text(e, b" ");
            sc.d(e, true);
            sc.text(e, b" ");
            sc.d(e, true);
            sc.text(e, b"\n");
        }
        1 => {
            sc.f(e, true);
            sc.text(e, b" ");
            sc.f(e, true);
            sc.text(e, b"   ");
        }
        2 => {
            sc.f(e, true);
            sc.text(e, b"  ");
        }
        3 => sc.text(e, b"\n"),
        4 => {
            sc.s(e, W, true);
        }
        5 => {
            sc.f(e, true);
        }
        6 => {
            sc.lf(e, true);
        }
        7 => {
            sc.d(e, true);
        }
        8 => {
            sc.f(e, true);
            sc.text(e, b" ");
            sc.d(e, true);
        }
        9 => {
            sc.s(e, W, true);
            sc.text(e, b" ");
            sc.f(e, true);
            sc.text(e, b" ");
            sc.f(e, true);
        }
        10 => {
            sc.s(e, W, false);
            sc.text(e, b" ");
            sc.d(e, true);
        }
        11 => {
            sc.f(e, true);
            sc.text(e, b" ");
            sc.f(e, true);
            sc.text(e, b" ");
            sc.f(e, true);
        }
        12 => {
            sc.d(e, true);
            sc.text(e, b" ");
            sc.d(e, true);
        }
        13 => {
            sc.s(e, W, true);
            sc.text(e, b" ");
            sc.s(e, W, true);
        }
        14 => {
            sc.text(e, b" ");
            sc.f(e, true);
        }
        15 => {
            sc.f(e, true);
            sc.text(e, b"\n");
        }
        16 => {
            sc.text(e, b"x");
            sc.d(e, true);
        }
        17 => {
            sc.lf(e, true);
            sc.text(e, b" ");
            sc.lf(e, true);
        }
        18 => {
            sc.d(e, false);
            sc.text(e, b" ");
            sc.f(e, true);
        }
        19 => {
            sc.s(e, 5, true);
            sc.text(e, b" ");
            sc.d(e, true);
        }
        _ => panic!("format {id}"),
    }
}
