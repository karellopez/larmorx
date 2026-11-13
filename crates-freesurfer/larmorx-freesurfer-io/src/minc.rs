// SPDX-License-Identifier: LicenseRef-FreeSurfer AND LicenseRef-MNI
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret. FreeSurfer's packages/minc carries MNI's notice (crates-freesurfer's
// licence texts).
//! FreeSurfer 8.2.0's copy of MINC's volume_io (`packages/minc`) as FreeSurfer's reached code reads
//! `.xfm` files (`input_transform_file`): the dialect [`FsVolumeIo`] of the main package's MINC
//! 1.5.1 reader (`larmorx_io::xfm`, track C's 3.1-14), with FreeSurfer's changes as its source
//! defines them (`minc_volume_io.c`, `minc_files.c`, `minc_gaussian.c`):
//!
//! 1. only `Linear` transforms: any other type is `input_transform(): invalid transform type
//!    <name>.` on standard error;
//! 2. a number that does not convert prints `<token> is not a valid double` first;
//! 3. a mismatched keyword prints `Expected "<keyword> =" got str:<token>` (`(null)` at the end of
//!    the file) whatever the caller asks;
//! 4. a NUL byte ends a token; messages are printed with their NUL bytes;
//! 5. `~user` is always `.` (built without `getpwnam`);
//! 6. its own 4 x 4 inverse ([`invert_4x4_matrix`]; a singular matrix prints `invert_4x4_matrix
//!    failed, <reason>` on standard output and the transform becomes the identity both ways).
//!
//! What it does with a compressed `.xfm` (a crash after `gunzip`) is MINC's FS-71 in the main
//! package (decompressed in the process); FreeSurfer never reads one on its default paths.

use std::io::Write;
use std::path::Path;

use larmorx_core::cfmt::Streams;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::sse;
use larmorx_io::xfm::{Dialect, GeneralTransform, Transform, TransformType};

/// The C streams as `larmorx_core::cfmt`'s [`Streams`].
pub struct Std<'a> {
    pub out: &'a mut dyn Write,
    pub err: &'a mut dyn Write,
}

impl Streams for Std<'_> {
    fn out(&mut self, text: &str) {
        let _ = self.out.write_all(text.as_bytes());
    }
    fn err(&mut self, text: &str) {
        let _ = self.err.write_all(text.as_bytes());
    }
}

/// FreeSurfer 8.2.0's volume_io (`packages/minc`).
#[derive(Clone, Copy, Debug, Default)]
pub struct FsVolumeIo;

impl Dialect for FsVolumeIo {
    fn print_error(&self, text: &str, streams: &mut dyn Streams) {
        streams.err(text);
    }
    fn keyword_mismatch(
        &self,
        keyword: &[u8],
        got: Option<&[u8]>,
        _print_error_message: bool,
        streams: &mut dyn Streams,
    ) {
        streams.err(&format!(
            "Expected \"{} =\" got str:{}\n",
            String::from_utf8_lossy(keyword),
            got.map_or("(null)".into(), String::from_utf8_lossy)
        ));
    }
    fn user_home_directory(&self, _user: &[u8]) -> Option<Vec<u8>> {
        Some(b".".to_vec())
    }
    fn nul_ends_token(&self) -> bool {
        true
    }
    fn invert(&self, t: &Transform, streams: &mut dyn Streams) -> Option<Transform> {
        match invert_4x4_matrix(t) {
            Ok(inv) => Some(inv),
            Err(why) => {
                streams.out(&format!("invert_4x4_matrix failed, {why}\n"));
                None
            }
        }
    }
    fn transform_type(&self, name: &[u8], streams: &mut dyn Streams) -> Option<TransformType> {
        if name == b"Linear" {
            return Some(TransformType::Linear);
        }
        streams.err(&format!(
            "input_transform(): invalid transform type {}.\n",
            String::from_utf8_lossy(name)
        ));
        None
    }
    fn bad_real(&self, token: &[u8], streams: &mut dyn Streams) {
        streams.err(&format!(
            "{} is not a valid double\n",
            String::from_utf8_lossy(token)
        ));
    }
}

/// `invert_4x4_matrix(mat, inv)` (`minc_gaussian.c`, `invert_4x4_matrix_wkr_fast`): Gaussian
/// elimination with scaled partial pivoting over a row map. Each row's scale is `1 / max |a_ij|`
/// (a row of zeros: `Some row is all zeros`); for columns 0 to 2 the row of the largest scaled
/// `|a_ki|` among those left (the first on ties; none above 0: `Some col is all zeros`) is swapped
/// into place in the map and eliminated from the later rows (`m = a_ki * (1 / a_ii)`, the matrix's
/// columns after `i` and all of the inverse's); a last pivot of 0: `last col all zeros`; then back
/// substitution from the last row (each later row's multiple subtracted, then `* (1 / a_ii)`). The
/// result's row `i` is the permuted inverse's row `map[i]`. Double arithmetic in SSE2's order, with
/// x86-64's NaNs.
pub fn invert_4x4_matrix(m: &Transform) -> Result<Transform, &'static str> {
    let mut mat = [0.0f64; 16];
    for i in 0..4 {
        for j in 0..4 {
            mat[4 * i + j] = m[i][j];
        }
    }
    let mut inv = [0.0f64; 16];
    let mut map = [0usize, 1, 2, 3];
    let mut scale_row = [0.0f64; 4];
    for i in 0..4 {
        let mut max = mat[4 * i].abs();
        for j in 1..4 {
            let d = mat[4 * i + j].abs();
            if max < d {
                max = d;
            }
        }
        if max == 0.0 {
            return Err("Some row is all zeros");
        }
        scale_row[i] = sse::div64(1.0, max);
    }
    for i in 0..4 {
        inv[4 * i + i] = 1.0;
    }
    for i in 0..3 {
        let mi = map[i];
        let mut max_i = i;
        let mut max = sse::mul64(mat[4 * mi + i], scale_row[mi]).abs();
        for i2 in i + 1..4 {
            let mi2 = map[i2];
            let d = sse::mul64(mat[4 * mi2 + i], scale_row[mi2]).abs();
            if max < d {
                max = d;
                max_i = i2;
            }
        }
        if max == 0.0 {
            return Err("Some col is all zeros");
        }
        map.swap(max_i, i);
        let mi = map[i];
        let scale = sse::div64(1.0, mat[4 * mi + i]);
        for i2 in i + 1..4 {
            let mi2 = map[i2];
            let mm = sse::mul64(mat[4 * mi2 + i], scale);
            for j in i + 1..4 {
                mat[4 * mi2 + j] = sse::sub64(mat[4 * mi2 + j], sse::mul64(mm, mat[4 * mi + j]));
            }
            for j in 0..4 {
                inv[4 * mi2 + j] = sse::sub64(inv[4 * mi2 + j], sse::mul64(mm, inv[4 * mi + j]));
            }
        }
    }
    if mat[4 * map[3] + 3] == 0.0 {
        return Err("last col all zeros");
    }
    for i in (0..4).rev() {
        let mi = map[i];
        for i2 in i + 1..4 {
            let scale = mat[4 * mi + i2];
            let mi2 = map[i2];
            for j in 0..4 {
                inv[4 * mi + j] = sse::sub64(inv[4 * mi + j], sse::mul64(scale, inv[4 * mi2 + j]));
            }
        }
        let scale = sse::div64(1.0, mat[4 * mi + i]);
        for j in 0..4 {
            inv[4 * mi + j] = sse::mul64(inv[4 * mi + j], scale);
        }
    }
    let mut r = [[0.0f64; 4]; 4];
    for i in 0..4 {
        for j in 0..4 {
            r[i][j] = inv[4 * map[i] + j];
        }
    }
    Ok(r)
}

/// `input_transform_file(filename, &transform)` as FreeSurfer's volume_io reads it (MINC 1.5.1's
/// reader in [`FsVolumeIo`]'s dialect): the transform, or `None` after its messages (FreeSurfer's
/// `ERROR`). A relative name is opened in `dir`, the program's directory, when given.
pub fn input_transform_file(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    filename: &[u8],
) -> Option<GeneralTransform> {
    let mut streams = Std { out, err };
    let path: Vec<u8> = match dir {
        Some(d) if !filename.is_empty() && filename[0] != b'/' && filename[0] != b'~' => {
            let mut p = d.to_string_lossy().into_owned().into_bytes();
            p.push(b'/');
            p.extend_from_slice(filename);
            p
        }
        _ => filename.to_vec(),
    };
    larmorx_io::xfm::input_transform_file(&path, &FsVolumeIo, &mut streams, &mut g.notices).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inverts_an_affine_and_refuses_singular_ones() {
        let m = [
            [2.0, 0.0, 0.0, 1.0],
            [0.0, 4.0, 0.0, 2.0],
            [0.0, 0.0, 8.0, 3.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        let inv = invert_4x4_matrix(&m).unwrap();
        assert_eq!(inv[0], [0.5, 0.0, 0.0, -0.5]);
        assert_eq!(inv[1], [0.0, 0.25, 0.0, -0.5]);
        assert_eq!(inv[2], [0.0, 0.0, 0.125, -0.375]);
        let z = [[0.0; 4]; 4];
        assert_eq!(invert_4x4_matrix(&z), Err("Some row is all zeros"));
        let mut c = m;
        for r in &mut c {
            r[0] = 0.0;
        }
        assert_eq!(invert_4x4_matrix(&c), Err("Some col is all zeros"));
        let s = [
            [1.0, 2.0, 3.0, 4.0],
            [2.0, 4.0, 6.0, 8.0],
            [0.0, 0.0, 1.0, 1.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        assert_eq!(invert_4x4_matrix(&s), Err("Some col is all zeros"));
    }

    #[test]
    fn reads_only_linear_transforms_with_freesurfers_messages() {
        let mut g = Globals::new(larmorx_core::defects::UpstreamDefects::Fix);
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("a.xfm"),
            "MNI Transform File\nTransform_Type = Grid_Transform;\n",
        )
        .unwrap();
        let t = input_transform_file(&mut g, &mut o, &mut e, Some(dir.path()), b"a.xfm");
        assert!(t.is_none());
        assert!(
            String::from_utf8_lossy(&e)
                .contains("input_transform(): invalid transform type Grid_Transform.\n")
        );
    }
}
