// SPDX-License-Identifier: Apache-2.0 AND LicenseRef-MNI
//! The `.xfm` writer: volume_io's `output_transform_file`, `output_transform`,
//! `output_one_transform` and `output_comments` (`gen_xf_io.c`, `mni_io.c`); numbers with C's
//! `%.15g`.

use std::io::Write as _;

use larmorx_core::cfmt::{self, Streams};
use larmorx_core::defects::{self as d, Notices};

use super::files::{Ctx, IoType, OpenFile, lossy, open_file_with_default_suffix};
use super::{Dialect, GeneralTransform, TransformData};

/// `output_comments`: `%` before the comments and after each of their newlines but the last
/// character's; a newline added when they do not end in one.
fn output_comments(out: &mut Vec<u8>, comments: Option<&[u8]>) {
    let Some(c) = comments else {
        return;
    };
    out.push(b'%');
    for (i, &ch) in c.iter().enumerate() {
        out.push(ch);
        if ch == b'\n' && i + 1 < c.len() {
            out.push(b'%');
        }
    }
    if c.last().is_some_and(|&l| l != b'\n') {
        out.push(b'\n');
    }
}

fn g15(v: f64) -> String {
    cfmt::g(v, 15)
}

/// `output_one_transform`; a grid transform's volume is written by the dialect
/// ([`Dialect::write_grid_volume`]), which larmorx cannot do yet (an error).
fn output_one_transform(
    out: &mut Vec<u8>,
    filename: Option<&[u8]>,
    volume_count: &mut i32,
    mut invert: bool,
    t: &GeneralTransform,
    dialect: &dyn Dialect,
    streams: &mut dyn Streams,
    notices: &mut Notices,
) -> Result<(), ()> {
    match &t.data {
        TransformData::Linear { .. } => {
            out.extend_from_slice(b"Transform_Type = Linear;\nLinear_Transform =\n");
            let m = if invert {
                t.inverse_linear_transform(streams)
            } else {
                t.linear_transform(streams)
            }
            .expect("linear");
            for (i, row) in m.iter().take(3).enumerate() {
                let line = format!(
                    " {} {} {} {}",
                    g15(row[0]),
                    g15(row[1]),
                    g15(row[2]),
                    g15(row[3])
                );
                out.extend_from_slice(line.as_bytes());
                if i == 2 {
                    out.push(b';');
                }
                out.push(b'\n');
            }
            Ok(())
        }
        TransformData::ThinPlateSpline {
            n_dimensions,
            points,
            displacements,
        } => {
            out.extend_from_slice(b"Transform_Type = Thin_Plate_Spline_Transform;\n");
            if t.inverse_flag {
                invert = !invert;
            }
            if invert {
                out.extend_from_slice(b"Invert_Flag = True;\n");
            }
            out.extend_from_slice(format!("Number_Dimensions = {n_dimensions};\n").as_bytes());
            out.extend_from_slice(b"Points =\n");
            write_rows(out, points, notices);
            out.extend_from_slice(b"Displacements =\n");
            write_rows(out, displacements, notices);
            Ok(())
        }
        TransformData::Grid { volume, .. } => {
            out.extend_from_slice(b"Transform_Type = Grid_Transform;\n");
            if t.inverse_flag {
                invert = !invert;
            }
            if invert {
                out.extend_from_slice(b"Invert_Flag = True;\n");
            }
            // The volume goes to "<the file's name without its extension>_grid_<n>.mnc".
            let mut prefix = match filename {
                Some(f) if !f.is_empty() => f.to_vec(),
                _ => b"grid".to_vec(),
            };
            if filename.is_some_and(|f| !f.is_empty()) {
                let mut i = prefix.len() as i64 - 1;
                while i > 0 && prefix[i as usize] != b'.' && prefix[i as usize] != b'/' {
                    i -= 1;
                }
                if i >= 0 && prefix[i as usize] == b'.' {
                    prefix.truncate(i as usize);
                }
            }
            let mut volume_filename = prefix;
            volume_filename.extend_from_slice(format!("_grid_{volume_count}.mnc").as_bytes());
            *volume_count += 1;
            let base = Ctx { dialect, notices: &mut *notices }.remove_directories(&volume_filename);
            out.extend_from_slice(b"Displacement_Volume = ");
            out.extend_from_slice(&base);
            out.extend_from_slice(b";\n");
            dialect.write_grid_volume(&volume_filename, volume, streams)
        }
        TransformData::Concatenated(v) => {
            if t.inverse_flag {
                invert = !invert;
            }
            if invert {
                for sub in v.iter().rev() {
                    output_one_transform(out, filename, volume_count, invert, sub, dialect, streams, notices)?;
                }
            } else {
                for sub in v {
                    output_one_transform(out, filename, volume_count, invert, sub, dialect, streams, notices)?;
                }
            }
            Ok(())
        }
    }
}

/// The rows of a spline's points or displacements: ` %.15g` per value, `;` after the last row.
/// volume_io writes no `;` at all for an empty list, which then cannot be read back (FS-74);
/// larmorx writes it on a line of its own.
fn write_rows(out: &mut Vec<u8>, rows: &[Vec<f64>], notices: &mut Notices) {
    if rows.is_empty() {
        notices.add(
            &d::FS_74,
            "an empty Points list written with its ';' (volume_io writes none)",
        );
        out.extend_from_slice(b";\n");
        return;
    }
    for (i, row) in rows.iter().enumerate() {
        for &v in row {
            out.push(b' ');
            out.extend_from_slice(g15(v).as_bytes());
        }
        if i + 1 == rows.len() {
            out.push(b';');
        }
        out.push(b'\n');
    }
}

/// `output_transform`: the file's text (header, comments, a blank line, the transform).
/// `filename` and `volume_count` name grid volumes as volume_io does.
pub fn output_transform(
    filename: Option<&[u8]>,
    volume_count: Option<&mut i32>,
    comments: Option<&[u8]>,
    t: &GeneralTransform,
    dialect: &dyn Dialect,
    streams: &mut dyn Streams,
    notices: &mut Notices,
) -> Result<Vec<u8>, ()> {
    let mut out = b"MNI Transform File\n".to_vec();
    output_comments(&mut out, comments);
    out.push(b'\n');
    let mut own = 0;
    let count = volume_count.unwrap_or(&mut own);
    output_one_transform(&mut out, filename, count, false, t, dialect, streams, notices)?;
    Ok(out)
}

/// [`output_transform`] as text (the file is ASCII unless a comment is not).
pub fn output_transform_to_string(t: &GeneralTransform, comments: Option<&str>) -> String {
    let mut quiet = larmorx_core::cfmt::Silent;
    let mut notices = Notices::default();
    let bytes = output_transform(
        None,
        None,
        comments.map(str::as_bytes),
        t,
        &super::Mni,
        &mut quiet,
        &mut notices,
    )
    .unwrap_or_default();
    lossy(&bytes).into_owned()
}

/// `output_transform_file`: writes the transform to `filename` (`.xfm` added when the name has
/// no extension; `~` and `$` expanded).
pub fn output_transform_file(
    filename: &[u8],
    comments: Option<&[u8]>,
    t: &GeneralTransform,
    dialect: &dyn Dialect,
    streams: &mut dyn Streams,
    notices: &mut Notices,
) -> Result<(), ()> {
    let file =
        open_file_with_default_suffix(filename, b"xfm", IoType::Write, dialect, streams, notices)?;
    let OpenFile::Write(mut file) = file else {
        unreachable!("opened for writing")
    };
    let mut count = 0;
    let text = output_transform(Some(filename), Some(&mut count), comments, t, dialect, streams, notices)?;
    file.write_all(&text).map_err(|_| ())
}
