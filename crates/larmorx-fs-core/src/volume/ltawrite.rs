// SPDX-License-Identifier: Apache-2.0
//! Writing a transform array (spec §62): `LTAwriteEx` and `LTAwrite` by the name's extension
//! (any case) and the array's type: MINC's `.xfm`, tkregister's `register.dat`
//! (`specs/freesurfer-io.md` §75's `regio_write_register`), an FSL matrix and FreeSurfer's
//! `.lta`; `LTAdumpVolGeom`. FS-504 where a writer reads a transform the array does not hold.

use std::io::Write;

use crate::clib::printf::sprintf_f64;
use crate::files::path_of;
use crate::maths::Returned;
use crate::maths::matrix::Matrix;
use crate::mri::lta::lta_type;
use crate::mri::{Lta, VolGeom};
use crate::process::{Process, Stop, codes};
use crate::sse;

/// Where a transform writer writes (§62): the file it creates (`fopen(name, "w")`) and the bytes
/// it writes there.
pub trait TransformOutputs {
    /// The file `name` created (emptied when it exists), or the C library's `errno` of the
    /// failure.
    fn create(&mut self, name: &[u8]) -> Result<(), i32>;

    /// Bytes written to the file created last.
    fn write(&mut self, bytes: &[u8]);
}

/// The file system.
#[derive(Default)]
pub struct FileOutputs {
    file: Option<std::fs::File>,
}

impl TransformOutputs for FileOutputs {
    fn create(&mut self, name: &[u8]) -> Result<(), i32> {
        let f = std::fs::File::create(path_of(name)).map_err(|e| e.raw_os_error().unwrap_or(5))?;
        self.file = Some(f);
        Ok(())
    }

    fn write(&mut self, bytes: &[u8]) {
        if let Some(f) = self.file.as_mut() {
            let _ = f.write_all(bytes);
        }
    }
}

/// Four numbers of a row, each `%13.8f` and a space.
fn row13(b: &mut Vec<u8>, m: &Matrix, r: i32) {
    for c in 1..=4 {
        b.extend_from_slice(&sprintf_f64(b"%13.8f ", sse::widen(m.at(r, c))));
    }
}

/// `regio_print_register`'s text (`specs/freesurfer-io.md` §75) with the method `round`: the
/// subject (`subject-unknown` when empty), the three numbers `%f`, the matrix's first three rows
/// (`%18.15e ` each), `0 0 0 1`, `round`.
pub fn register_text(
    subject: &[u8],
    inplane: f32,
    between: f32,
    intensity: f32,
    r: &Matrix,
) -> Vec<u8> {
    let mut b = Vec::with_capacity(400);
    if subject.is_empty() {
        b.extend_from_slice(b"subject-unknown");
    } else {
        b.extend_from_slice(subject);
    }
    b.push(b'\n');
    for v in [inplane, between, intensity] {
        b.extend_from_slice(&sprintf_f64(b"%f", sse::widen(v)));
        b.push(b'\n');
    }
    for row in 1..=3 {
        for c in 1..=4 {
            b.extend_from_slice(&sprintf_f64(b"%18.15e ", sse::widen(r.at(row, c))));
        }
        b.push(b'\n');
    }
    b.extend_from_slice(b"0 0 0 1\nround\n");
    b
}

/// The array's subject up to its first NUL.
fn subject_of(lta: &Lta) -> &[u8] {
    let end = lta
        .subject
        .iter()
        .position(|&b| b == 0)
        .unwrap_or(lta.subject.len());
    &lta.subject[..end]
}

/// The writers (§62).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Writer {
    Minc,
    Register,
    Fsl,
    Lta,
}

impl Process {
    /// `LTAwriteEx(lta, fname)` (§62): by the first rule that applies, MINC's `.xfm` (the
    /// extension `xfm`, or the type MNI), a `register.dat` (`dat` or `reg`, or the type
    /// REGISTER_DAT with any extension but `lta`), an FSL matrix (`fslmat`, or the type FSL),
    /// else FreeSurfer's `.lta`. Returns fs.core's code: 0, -1, -4 or -5.
    pub fn lta_write_ex(&mut self, lta: &Lta, fname: &[u8]) -> Result<i32, Stop> {
        self.lta_write_to(lta, fname, &mut FileOutputs::default())
    }

    /// [`Process::lta_write_ex`] into `out`.
    pub fn lta_write_to(
        &mut self,
        lta: &Lta,
        fname: &[u8],
        out: &mut dyn TransformOutputs,
    ) -> Result<i32, Stop> {
        let ext = self.file_name_extension(fname);
        let is = |e: &[u8]| ext.eq_ignore_ascii_case(e);
        let w = if is(b"xfm") || lta.kind == lta_type::MNI_TRANSFORM_TYPE {
            Writer::Minc
        } else if is(b"dat") || is(b"reg") || (lta.kind == lta_type::REGISTER_DAT && !is(b"lta")) {
            Writer::Register
        } else if is(b"fslmat") || lta.kind == lta_type::FSLREG_TYPE {
            Writer::Fsl
        } else {
            Writer::Lta
        };
        match w {
            Writer::Minc => self.lta_write_minc(lta, fname, out),
            Writer::Register => self.lta_write_register(lta, fname, out),
            Writer::Fsl => self.lta_write_fsl(lta, fname, out),
            Writer::Lta => self.lta_write_lta(lta, fname, out),
        }
    }

    /// `LTAwrite(lta, fname)` (§62): the same as [`Process::lta_write_ex`].
    pub fn lta_write(&mut self, lta: &Lta, fname: &[u8]) -> Result<i32, Stop> {
        self.lta_write_ex(lta, fname)
    }

    /// FS-504 where a writer reads the first transform's matrix: an array without transforms,
    /// or a first matrix smaller than 3 x 4.
    fn lta_first_matrix(&mut self, lta: &Lta, function: &str) -> Result<Matrix, Stop> {
        match lta.xforms.first().and_then(|t| t.m_l.as_ref()) {
            Some(m) if m.rows >= 3 && m.cols >= 4 => Ok(m.clone()),
            _ => {
                self.lta_fs504(
                    function,
                    "an array without a first transform's 3 x 4 matrix",
                )?;
                Err(Stop::Exit(252))
            }
        }
    }

    /// MINC's `.xfm` (§62).
    fn lta_write_minc(
        &mut self,
        lta: &Lta,
        fname: &[u8],
        f: &mut dyn TransformOutputs,
    ) -> Result<i32, Stop> {
        if let Err(e) = f.create(fname) {
            self.errno = e;
            let mut m = b"ltaMNIwrite: could not open file ".to_vec();
            m.extend_from_slice(fname);
            self.error_printf(codes::NO_FILE, &m);
            return Ok(codes::NO_FILE);
        }
        let mut b = b"MNI Transform File\n%Generated by ".to_vec();
        b.extend_from_slice(&self.progname);
        if lta.xforms.is_empty() {
            f.write(&b);
            self.lta_fs504("ltaMNIwrite", "an array without transforms")?;
            return Ok(0);
        }
        let t = &lta.xforms[0];
        b.extend_from_slice(b" src ");
        b.extend_from_slice(&t.src.fname);
        b.extend_from_slice(b" dst ");
        b.extend_from_slice(&t.dst.fname);
        b.extend_from_slice(b"\n\nTransform_Type = Linear;\nLinear_Transform =\n");
        let rows = match lta.kind {
            lta_type::LINEAR_RAS_TO_RAS | lta_type::MNI_TRANSFORM_TYPE => {
                f.write(&b);
                b.clear();
                Some(self.lta_first_matrix(lta, "ltaMNIwrite")?)
            }
            lta_type::LINEAR_VOX_TO_VOX => {
                f.write(&b);
                b.clear();
                let m = self.lta_first_matrix(lta, "ltaMNIwrite")?;
                let (src, dst) = (t.src.clone(), t.dst.clone());
                let i2r = self.vg_i_to_r(&dst)?;
                let r2i = self.vg_r_to_i(&src)?;
                let inner = match self.matrix_multiply(Some(&m), r2i.as_ref(), None)? {
                    Returned::New(x) => Some(x),
                    _ => None,
                };
                match self.matrix_multiply(Some(&i2r), inner.as_ref(), None)? {
                    Returned::New(x) => Some(x),
                    _ => None,
                }
            }
            _ => None,
        };
        if let Some(m) = rows {
            for r in 1..=3 {
                row13(&mut b, &m, r);
                if r == 3 {
                    b.push(b';');
                }
                b.push(b'\n');
            }
        }
        f.write(&b);
        Ok(codes::NONE)
    }

    /// tkregister's `register.dat` (§62, `specs/freesurfer-io.md` §75).
    fn lta_write_register(
        &mut self,
        lta: &Lta,
        fname: &[u8],
        f: &mut dyn TransformOutputs,
    ) -> Result<i32, Stop> {
        if lta.xforms.is_empty() {
            self.lta_fs504(
                "LTAwrite",
                "an array without transforms written as register.dat",
            )?;
            return Ok(0);
        }
        let m = self.lta_first_matrix(lta, "LTAwrite")?;
        let src = &lta.xforms[0].src;
        let text = register_text(subject_of(lta), src.xsize, src.zsize, lta.fscale, &m);
        match f.create(fname) {
            Ok(()) => {
                f.write(&text);
                Ok(codes::NONE)
            }
            Err(e) => {
                self.errno = e;
                let mut b = b"regio_write_register: ".to_vec();
                b.extend_from_slice(crate::clib::strerror(e).as_bytes());
                b.push(b'\n');
                self.put_err(&b);
                Ok(codes::NO_FILE)
            }
        }
    }

    /// An FSL matrix (§62): a one-transform copy changed to the FSL type, its four rows.
    fn lta_write_fsl(
        &mut self,
        lta: &Lta,
        fname: &[u8],
        f: &mut dyn TransformOutputs,
    ) -> Result<i32, Stop> {
        if let Err(e) = f.create(fname) {
            self.errno = e;
            let mut m = b"ltaFSLwrite: could not open file ".to_vec();
            m.extend_from_slice(fname);
            self.error_printf(codes::NO_FILE, &m);
            return Ok(codes::NO_FILE);
        }
        if lta.xforms.is_empty() {
            self.lta_fs504("ltaFSLwrite", "an array without transforms")?;
            return Ok(0);
        }
        let mut one = self.lta_alloc(1, None)?;
        let t = &lta.xforms[0];
        one.xforms[0].m_l = t.m_l.clone();
        one.xforms[0].src = t.src.clone();
        one.xforms[0].dst = t.dst.clone();
        one.kind = lta.kind;
        if one.kind != lta_type::FSLREG_TYPE {
            self.lta_change_type(&mut one, lta_type::FSLREG_TYPE)?;
        }
        if one.kind != lta_type::FSLREG_TYPE {
            self.error_printf(
                codes::BAD_PARM,
                b"ltaFSLwrite: could not convert to FSL format",
            );
            return Ok(codes::BAD_PARM);
        }
        let m = self.lta_first_matrix(&one, "ltaFSLwrite")?;
        let mut b = Vec::with_capacity(240);
        for r in 1..=4 {
            row13(&mut b, &m, r);
            b.push(b'\n');
        }
        f.write(&b);
        Ok(codes::NONE)
    }

    /// FreeSurfer's `.lta` (§62): the two comment lines and `LTAprint`'s text.
    fn lta_write_lta(
        &mut self,
        lta: &Lta,
        fname: &[u8],
        f: &mut dyn TransformOutputs,
    ) -> Result<i32, Stop> {
        if let Err(e) = f.create(fname) {
            self.errno = e;
            let mut m = b"LTAwrite(".to_vec();
            m.extend_from_slice(fname);
            m.extend_from_slice(b"): can't create file");
            self.error_printf(codes::BAD_FILE, &m);
            return Ok(codes::BAD_FILE);
        }
        let user = match self.env.get("USER") {
            Some(u) => u.to_vec(),
            None => match self.env.get("LOGNAME") {
                Some(u) => u.to_vec(),
                None => b"UNKNOWN".to_vec(),
            },
        };
        let mut b = b"# transform file ".to_vec();
        b.extend_from_slice(fname);
        b.extend_from_slice(b"\n# created by ");
        b.extend_from_slice(&user);
        b.extend_from_slice(b" on ");
        b.extend_from_slice(&self.current_date_time(true));
        b.extend_from_slice(b"\n\n");
        match self.lta_print_text(lta) {
            Ok(t) => {
                b.extend_from_slice(&t);
                f.write(&b);
                Ok(codes::NONE)
            }
            Err(t) => {
                b.extend_from_slice(&t);
                f.write(&b);
                self.lta_fs504("LTAprint", "a transform without a 4 x 4 matrix")?;
                Ok(0)
            }
        }
    }

    /// `LTAdumpVolGeom(fp, vg)` (§62) to standard output: the geometry's fields one per line
    /// (the numbers `%f`). Returns 0.
    pub fn lta_dump_vol_geom(&mut self, vg: &VolGeom) -> i32 {
        let mut b = Vec::with_capacity(300);
        b.extend_from_slice(format!("valid  {}\n", vg.valid).as_bytes());
        b.extend_from_slice(format!("width  {}\n", vg.width).as_bytes());
        b.extend_from_slice(format!("height {}\n", vg.height).as_bytes());
        b.extend_from_slice(format!("depth  {}\n", vg.depth).as_bytes());
        for (name, v) in [
            ("xsize  ", vg.xsize),
            ("ysize  ", vg.ysize),
            ("zsize  ", vg.zsize),
        ] {
            b.extend_from_slice(name.as_bytes());
            b.extend_from_slice(&sprintf_f64(b"%f", sse::widen(v)));
            b.push(b'\n');
        }
        for (name, d) in [("xdc    ", vg.x), ("ydc    ", vg.y), ("zdc    ", vg.z)] {
            b.extend_from_slice(name.as_bytes());
            for (k, v) in d.into_iter().enumerate() {
                if k > 0 {
                    b.push(b' ');
                }
                b.extend_from_slice(&sprintf_f64(b"%f", sse::widen(v)));
            }
            b.push(b'\n');
        }
        b.extend_from_slice(b"fname  ");
        b.extend_from_slice(&vg.fname);
        b.push(b'\n');
        self.put_out(&b);
        0
    }
}
