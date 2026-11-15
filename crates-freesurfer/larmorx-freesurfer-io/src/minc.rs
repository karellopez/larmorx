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

/// FreeSurfer 8.2.0's copy of MINC's volume input (`packages/minc`): the dialect of the main
/// package's MINC 1.5.1 volume reader (`larmorx_io::minc`, track C's port) as FreeSurfer's copy
/// behaves, measured by track C's probe (`validation/probes/fs-nrrd-minc`) and this crate's
/// (piece 25 of fs.io's):
///
/// 1. a package error prints its copy's source file and line and the message on standard error
///    and ends the program with exit status 1 (`MI_LOG_PKG_ERROR2`, `MI_LOG_SYS_ERROR1`);
/// 2. no `MINC package entry point` lines; no volume cache;
/// 3. no decompression (FS-788: corrected by default, the file decompressed);
/// 4. an image of type double has no default range: the program ends (FS-775: corrected by
///    default, the doubles read), as does an image of a type volume_io does not know and a
///    dimension of irregular spacing;
/// 5. its `.xfm` dialect is [`FsVolumeIo`] (its own 4 x 4 inverse);
/// 6. writing: a direction cosine of length 0 ends the program after volume_io's warning (exit
///    status 1); a volume of four dimensions (`xspace`, `yspace`, `zspace`, `time`) ends it at the
///    start of the output with `convert_spatial_axis_to_dim_name, bad axis` (FS-797: corrected by
///    default, written as MINC 1.5.1 writes it).
#[derive(Clone, Copy, Debug, Default)]
pub struct FsMinc;

/// The folder FreeSurfer's copy of MINC was compiled in, as its messages print it.
fn minc_prefix() -> String {
    format!(
        "{}/packages/minc/",
        larmorx_freesurfer_core::diag::BUILD_ROOT
    )
}

fn minc_fatal(at: &str, msg: &str, streams: &mut dyn Streams) -> larmorx_io::minc::Exit {
    streams.err(&format!("{}{at} {msg}\n", minc_prefix()));
    larmorx_io::minc::Exit { status: 1 }
}

impl larmorx_io::minc::Dialect for FsMinc {
    fn pkg_error(
        &self,
        site: larmorx_io::minc::PkgSite,
        _code: i32,
        message: &str,
        streams: &mut dyn Streams,
    ) -> Option<larmorx_io::minc::Exit> {
        use larmorx_io::minc::PkgSite as S;
        // the line of the macro's last line (GCC's __LINE__ of a call spread over lines)
        let at = match site {
            S::Uncompress => "minc_netcdf_convenience.c:402",
            S::AttNonNumeric => "minc_netcdf_convenience.c:786",
            S::AttGetAlloc => "minc_netcdf_convenience.c:801",
            S::AttNonScalar => "minc_netcdf_convenience.c:899",
            S::AttNonChar => "minc_netcdf_convenience.c:940",
            S::VarNonNumeric => "minc_value_conversion.c:229",
            S::VarLoopAlloc => "minc_value_conversion.c:490",
            S::IcvNonNumeric => "minc_image_conversion.c:1015",
            S::IcvNotAttached => "minc_image_conversion.c:1328",
            S::IcvTooFewDims => "minc_image_conversion.c:2599",
            S::IcvTooFewDimsVector => "minc_image_conversion.c:2612",
            // never raised on volume_io's reading path (it sets each property once, before it
            // attaches): the first site of each kind, miicv_setdbl's
            S::IcvAttached => "minc_image_conversion.c:399",
            S::IcvBadProperty => "minc_image_conversion.c:454",
            // the writing path's sites (track C's MINC writer, batch 20); mincWrite reaches none of
            // them (standard names only, dimension variables of at most one dimension, image-max/min
            // over the slower dimensions): their lines in FreeSurfer's copy, not yet probed.
            // micreate_std_variable's name in no list (the copy has two more BAD_STDVAR sites, 603
            // and 1213, that MINC 1.5.1 does not have)
            S::BadStdVar => "minc_convenience.c:1223",
            // two sites share it: MI_create_dim_variable's (611) and MI_create_dimwidth_variable's
            // (689); the first
            S::WrongNDims => "minc_convenience.c:611",
            S::BadSuffix => "minc_convenience.c:681",
            S::MaxMinDims => "minc_convenience.c:754",
        };
        Some(minc_fatal(at, message, streams))
    }
    fn logs_entry_point(&self) -> bool {
        false
    }
    fn cache_threshold(&self) -> Option<i64> {
        None
    }
    fn decompresses(&self, notices: &mut larmorx_core::defects::Notices) -> bool {
        if notices.reproduce() {
            false
        } else {
            notices.add(
                &larmorx_core::defects::FS_788,
                "a compressed MINC file decompressed (FreeSurfer's copy cannot)",
            );
            true
        }
    }
    fn default_range(
        &self,
        nc_type: i32,
        notices: &mut larmorx_core::defects::Notices,
        streams: &mut dyn Streams,
    ) -> Option<larmorx_io::minc::Exit> {
        use larmorx_io::minc::netcdf::{NC_BYTE, NC_DOUBLE, NC_FLOAT, NC_INT, NC_SHORT};
        match nc_type {
            NC_BYTE | NC_SHORT | NC_INT | NC_FLOAT => None,
            NC_DOUBLE if notices.fix() => {
                notices.add(
                    &larmorx_core::defects::FS_775,
                    "a MINC image of type double read (FreeSurfer's copy ends the program)",
                );
                None
            }
            _ => Some(minc_fatal(
                "minc_convenience.c:568",
                "bad data type",
                streams,
            )),
        }
    }
    fn xfm(&self) -> &dyn Dialect {
        &FsVolumeIo
    }
    fn zero_direction_cosine(&self, _streams: &mut dyn Streams) -> Option<larmorx_io::minc::Exit> {
        // the copy ends the program after volume_io's warning (exit status 1), in both modes
        Some(larmorx_io::minc::Exit { status: 1 })
    }
    fn output_dimension_names(
        &self,
        names: &[Vec<u8>],
        notices: &mut larmorx_core::defects::Notices,
        streams: &mut dyn Streams,
    ) -> Option<larmorx_io::minc::Exit> {
        // FS-797: the copy names only the spatial axes; a volume with a fourth dimension (mincWrite's
        // `time`) ends the program. Other dimension lists are not mincWrite's.
        let spatial = |n: &[u8]| matches!(n, b"xspace" | b"yspace" | b"zspace");
        if names.len() == 4
            && names[0] == b"xspace"
            && names[1] == b"yspace"
            && names[2] == b"zspace"
            && !spatial(&names[3])
        {
            if notices.reproduce() {
                streams.err("convert_spatial_axis_to_dim_name, bad axis\n");
                return Some(larmorx_io::minc::Exit { status: 1 });
            }
            notices.add(
                &larmorx_core::defects::FS_797,
                "a volume with a time dimension written as MINC 1.5.1 writes it (FreeSurfer's copy ends the program)",
            );
        }
        None
    }
    fn bad_data_type(
        &self,
        _nc_type: i32,
        streams: &mut dyn Streams,
    ) -> Option<larmorx_io::minc::Exit> {
        Some(minc_fatal(
            "minc_volume_io.c:2289",
            "bad nc_data_type",
            streams,
        ))
    }
    fn irregular_spacing(
        &self,
        widths: bool,
        streams: &mut dyn Streams,
    ) -> Option<larmorx_io::minc::Exit> {
        Some(if widths {
            // never reached: the starts' message comes first
            minc_fatal(
                "minc_volume_io.c:2073",
                "set_volume_irregular_widths NYI",
                streams,
            )
        } else {
            minc_fatal(
                "minc_volume_io.c:2035",
                "set_volume_irregular_starts NYI",
                streams,
            )
        })
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
// the C loops over the row map, as written
#[allow(clippy::needless_range_loop)]
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
/// `ERROR`). A relative name is opened in `dir`, the program's directory, when given: FreeSurfer
/// opens it in its working directory, which volume_io's expansion never sees, so on Windows the
/// folder's backslashes are written as `/` (a separator there too): volume_io's escape (FS-79's
/// `reproduce`, applied by both the existence check and the open) would drop them, and the folder
/// `C:\Users\...` would be read as `C:Users...`.
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
            if cfg!(windows) && !p.starts_with(b"\\\\?\\") {
                for c in p.iter_mut().filter(|c| **c == b'\\') {
                    *c = b'/';
                }
            }
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
