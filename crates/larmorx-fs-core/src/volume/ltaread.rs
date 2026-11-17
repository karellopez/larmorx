// SPDX-License-Identifier: Apache-2.0
//! Reading a linear transform (spec §21): the type from a file's name (`TransformFileNameType`)
//! and `LTAreadEx` of the four kinds of file: FreeSurfer's `.lta`, MINC's `.xfm`, FSL's matrix and
//! tkregister's `register.dat` (`regio_read_register`, `specs/freesurfer-io.md` §46), and the
//! identity of `identity.nofile`; FS-306 to FS-310 and FS-258.
//!
//! The files and the volumes a MINC transform names are read through the caller's
//! [`TransformInputs`] ([`FileSystem`]: the file system, no volume headers).

use larmorx_core::cscan::is_c_space;
use larmorx_core::defects::{FS_258, FS_306, FS_307, FS_308, FS_309, FS_310};

use crate::maths::matrix::{MATRIX_REAL, Matrix};
use crate::maths::mfiles::{CStream, Call};
use crate::mri::geom::{VolGeom, fixed_name};
use crate::mri::lta::{Lta, init_vol_geom, lta_type, transform_type};
use crate::process::{Process, Stop, codes};

/// `EINVAL`.
const EINVAL: i32 = 22;
/// `ENOENT`.
const ENOENT: i32 = 2;

/// What a transform reader reads (§21): the transform file's bytes and the geometries of the
/// volumes a MINC transform names. The defaults read the file system: a file's bytes, `errno` when
/// it cannot be opened; a named volume that does not exist sets `errno` (`ENOENT`) and one that
/// exists is not read here (its geometry is taken as missing; a program gives `specs/freesurfer-io.md`'s
/// header reader by implementing [`TransformInputs::volume_geometry`]).
pub trait TransformInputs {
    /// The bytes of the file `name`, or the `errno` of a failed open.
    fn read_file(&mut self, name: &[u8]) -> Result<Vec<u8>, i32> {
        std::fs::read(crate::files::path_of(name)).map_err(|e| e.raw_os_error().unwrap_or(ENOENT))
    }

    /// The header's geometry of the volume `name`; `None` (and `errno` set as a failed open
    /// sets it) when there is none.
    fn volume_geometry(&mut self, p: &mut Process, name: &[u8]) -> Option<VolGeom> {
        if let Err(e) = std::fs::metadata(crate::files::path_of(name)) {
            p.errno = e.raw_os_error().unwrap_or(ENOENT);
        }
        None
    }
}

/// The file system, read through [`TransformInputs`]' defaults.
pub struct FileSystem;

impl TransformInputs for FileSystem {}

/// The average brain's geometry (§21): 172 x 220 x 156 voxels of 1 mm, identity cosines, centre
/// (-0.095, -16.51, 9.75), valid.
fn average_305(name: &[u8]) -> VolGeom {
    VolGeom {
        valid: 1,
        width: 172,
        height: 220,
        depth: 156,
        x: [1.0, 0.0, 0.0],
        y: [0.0, 1.0, 0.0],
        z: [0.0, 0.0, 1.0],
        c: [-0.095, -16.51, 9.75],
        fname: fixed_name(name, 1024),
        ..init_vol_geom()
    }
}

/// One value a `scanf` call converted.
#[derive(Clone, Debug, PartialEq)]
enum Got {
    D(i32),
    F(f32),
    S(Vec<u8>),
}

/// One `scanf` call (§21, `specs/freesurfer-maths.md` §18.1) of a format of white space,
/// ordinary characters and the conversions `%d`, `%f`, `%s` (`%*s` read and dropped): the values
/// converted, in order, up to the first failure. White space just before a conversion is the
/// conversion's own skip.
fn scanf(p: &mut Process, s: &mut CStream, fmt: &[u8]) -> Vec<Got> {
    let mut call = Call::new();
    let mut out = Vec::new();
    let mut i = 0;
    while i < fmt.len() {
        let c = fmt[i];
        if is_c_space(c) {
            while i < fmt.len() && is_c_space(fmt[i]) {
                i += 1;
            }
            if fmt.get(i) != Some(&b'%') {
                call.ws(p, s);
            }
            continue;
        }
        if c == b'%' {
            let mut j = i + 1;
            let drop = fmt.get(j) == Some(&b'*');
            if drop {
                j += 1;
            }
            let got = match fmt.get(j) {
                Some(b'd') => call.d(p, s).map(Got::D),
                Some(b'f') => call.f(p, s).map(Got::F),
                _ => call.s(p, s, None).map(Got::S),
            };
            let Some(got) = got else { break };
            if !drop {
                out.push(got);
            }
            i = j + 1;
            continue;
        }
        if !call.lit(p, s, c) {
            break;
        }
        i += 1;
    }
    out
}

/// `sscanf(line, fmt, ...)`.
fn sscanf(p: &mut Process, line: &[u8], fmt: &[u8]) -> Vec<Got> {
    let mut s = CStream::new(line.to_vec());
    scanf(p, &mut s, fmt)
}

/// `fgetl` of a stream (`specs/freesurfer-core.md` §10, `n`-byte buffer).
fn fgetl(s: &mut CStream, n: usize) -> Option<Vec<u8>> {
    let mut cur = std::io::Cursor::new(&s.bytes[s.pos..]);
    let line = crate::strings::fgetl(&mut cur, n);
    s.pos += cur.position() as usize;
    if line.is_none() {
        s.eof = true;
    }
    line
}

/// The words of a line (C's white space between them).
fn words(line: &[u8]) -> Vec<&[u8]> {
    line.split(|&c| is_c_space(c))
        .filter(|w| !w.is_empty())
        .collect()
}

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

/// A transform file's bytes, or the error of a failed open (`errno` set).
fn read_file(p: &mut Process, inputs: &mut dyn TransformInputs, name: &[u8]) -> Option<CStream> {
    match inputs.read_file(name) {
        Ok(b) => Some(CStream::new(b)),
        Err(e) => {
            p.errno = e;
            None
        }
    }
}

/// `TransformFileNameType`'s types (§21).
pub mod file_type {
    use crate::mri::lta::lta_type;
    pub const MORPH: i32 = lta_type::MORPH_3D_TYPE;
    pub const LTA: i32 = lta_type::TRANSFORM_ARRAY_TYPE;
    pub const MINC: i32 = lta_type::MNI_TRANSFORM_TYPE;
    pub const FSL: i32 = lta_type::FSLREG_TYPE;
    pub const REGISTER_DAT: i32 = lta_type::REGISTER_DAT;
}

impl Process {
    /// `TransformFileNameType(fname)` (§21): the type a file's name gives. FS-306: a name of 500
    /// bytes or more is classified whole (FreeSurfer writes past the memory it classifies it in).
    pub fn transform_file_name_type(&mut self, fname: &[u8]) -> i32 {
        if fname.len() >= 500 {
            self.note(
                &FS_306,
                "a transform's name of 500 bytes or more classified whole (FreeSurfer writes past its memory)",
            );
        }
        // the text after the last `@`, else after the last `.` of the name cut at its first `#`
        let ext: Option<&[u8]> = match fname.iter().rposition(|&c| c == b'@') {
            Some(at) => Some(&fname[at + 1..]),
            None => {
                let cut = fname
                    .iter()
                    .position(|&c| c == b'#')
                    .map_or(fname, |k| &fname[..k]);
                cut.iter().rposition(|&c| c == b'.').map(|d| &cut[d + 1..])
            }
        };
        let Some(ext) = ext else {
            return file_type::LTA;
        };
        // compared upper-cased up to a `#`
        let ext: Vec<u8> = ext
            .iter()
            .take_while(|&&c| c != b'#')
            .map(u8::to_ascii_uppercase)
            .collect();
        match ext.as_slice() {
            b"M3D" | b"M3Z" | b"MGZ" | b"NII" => file_type::MORPH,
            b"OCT" | b"LTA" => file_type::LTA,
            b"XFM" => file_type::MINC,
            b"FSLMAT" => file_type::FSL,
            _ if fname.len() > 6 && fname.ends_with(b"nii.gz") => file_type::MORPH,
            _ => file_type::REGISTER_DAT,
        }
    }

    /// `LTAreadEx(fname)` (§21), the files and the volumes a MINC transform names read through
    /// `inputs`: the transform array, or `None` after the reader's error.
    pub fn lta_read_ex(
        &mut self,
        fname: &[u8],
        inputs: &mut dyn TransformInputs,
    ) -> Result<Option<Lta>, Stop> {
        let kind = self.transform_file_name_type(fname);
        let file = fname
            .iter()
            .rposition(|&c| c == b'/')
            .map_or(fname, |k| &fname[k + 1..]);
        if file == b"identity.nofile" {
            let mut lta = self.lta_alloc(1, None)?;
            lta.kind = lta_type::LINEAR_RAS_TO_RAS;
            lta.xforms[0].kind = lta_type::LINEAR_RAS_TO_RAS;
            return Ok(Some(lta));
        }
        match kind {
            file_type::MINC => self.lt_mni_read_ex(fname, inputs),
            file_type::FSL => self.lt_fsl_read(fname, inputs),
            file_type::REGISTER_DAT => self.lta_read_register_dat(fname, inputs),
            _ => self.lta_read_file(fname, inputs),
        }
    }

    /// An error of a reader whose text ends with the file's name and `tail`.
    fn lt_error(&mut self, code: i32, head: &str, fname: &[u8], tail: &str) {
        let mut m = head.as_bytes().to_vec();
        m.extend_from_slice(fname);
        m.extend_from_slice(tail.as_bytes());
        self.error_printf(code, &m);
    }

    /// `ltaReadFile` (§21): an `.lta` (and the morphs' names, and names without an extension).
    fn lta_read_file(
        &mut self,
        fname: &[u8],
        inputs: &mut dyn TransformInputs,
    ) -> Result<Option<Lta>, Stop> {
        let Some(mut s) = read_file(self, inputs, fname) else {
            self.lt_error(codes::NO_FILE, "ltaReadFile(", fname, "): can't open file");
            return Ok(None);
        };
        let Some(l1) = fgetl(&mut s, 1023) else {
            self.lt_error(codes::BAD_FILE, "ltaReadFile(", fname, "): can't read data");
            return Ok(None);
        };
        let Some(l2) = fgetl(&mut s, 1023) else {
            // FS-309: FreeSurfer crashes on a file of a single line
            self.note(
                &FS_309,
                "an .lta of a single line refused (FreeSurfer crashes)",
            );
            self.lt_error(codes::BAD_FILE, "ltaReadFile(", fname, "): can't read data");
            return Ok(None);
        };
        let kind = sscanf(self, &l1, b"type      = %d\n");
        let n = sscanf(self, &l2, b"nxforms   = %d\n");
        let kind = match kind.first() {
            Some(Got::D(v)) => Some(*v),
            _ => None,
        };
        let n = match n.first() {
            Some(Got::D(v)) => Some(*v),
            _ => None,
        };
        if kind.is_none() || n.is_none() {
            if !self.mode().reproduce() {
                self.note(
                    &FS_308,
                    "an .lta whose first two lines are not type = <n> and nxforms = <n> refused (FreeSurfer takes memory contents)",
                );
                self.lt_error(codes::BAD_FILE, "ltaReadFile(", fname, "): can't read data");
                return Ok(None);
            }
            self.note(
                &FS_308,
                "an .lta's type or number of transforms not given: 0 (FreeSurfer takes memory contents)",
            );
        }
        let mut lta = self.lta_alloc(n.unwrap_or(0), None)?;
        lta.kind = kind.unwrap_or(0);
        for lt in lta.xforms.iter_mut() {
            let v = scanf(self, &mut s, b"mean      = %f %f %f\n");
            for (k, g) in v.iter().enumerate() {
                if let Got::F(x) = g {
                    match k {
                        0 => lt.x0 = *x,
                        1 => lt.y0 = *x,
                        _ => lt.z0 = *x,
                    }
                }
            }
            if let Some(Got::F(x)) = scanf(self, &mut s, b"sigma     = %f\n").first() {
                lt.sigma = *x;
            }
            self.matrix_ascii_read_from(&mut s, lt.m_l.as_mut())?;
        }
        for lt in lta.xforms.iter_mut() {
            let Some(line) = s.fgets(1023) else { break };
            if line.starts_with(b"src volume info") {
                self.read_vol_geom(&mut s, &mut lt.src);
                if let Some(line) = s.fgets(1023)
                    && line.starts_with(b"dst volume info")
                {
                    self.read_vol_geom(&mut s, &mut lt.dst);
                }
            }
        }
        lta.subject.clear();
        lta.fscale = 0.15;
        while let Some(line) = s.fgets(1023) {
            if line.starts_with(b"subject") {
                if let Some(Got::S(w)) = sscanf(self, &line, b"%*s %s").first() {
                    lta.subject = fixed_name(w, 1024);
                }
            } else if line.starts_with(b"fscale")
                && let Some(Got::F(x)) = sscanf(self, &line, b"%*s %f").first()
            {
                lta.fscale = *x;
            }
        }
        Ok(Some(lta))
    }

    /// A transform array of one transform, its matrix's first three rows given (§21: the MINC and
    /// FSL readers): `sigma` 1, the last row (0, 0, 0, 1).
    fn lta_of_rows(&mut self, kind: i32, rows: &[[f32; 4]; 3]) -> Result<Lta, Stop> {
        let mut lta = self.lta_alloc(1, None)?;
        lta.kind = kind;
        let lt = &mut lta.xforms[0];
        lt.sigma = 1.0;
        let mut m = Matrix::zeros(4, 4, MATRIX_REAL);
        for (r, row) in rows.iter().enumerate() {
            for (c, v) in row.iter().enumerate() {
                m.set(r as i32 + 1, c as i32 + 1, *v);
            }
        }
        m.set(4, 4, 1.0);
        lt.m_l = Some(m);
        Ok(lta)
    }

    /// Three rows `%f %f %f %f`, one line each (§21); the 1-based number of a row that is
    /// missing.
    fn four_floats(&mut self, line: &[u8]) -> Option<[f32; 4]> {
        let v = sscanf(self, line, b"%f %f %f %f");
        if v.len() < 4 {
            return None;
        }
        let mut out = [0f32; 4];
        for (k, g) in v.iter().take(4).enumerate() {
            if let Got::F(x) = g {
                out[k] = *x;
            }
        }
        Some(out)
    }

    /// `ltFSLread` (§21): FSL's matrix, type `FSLREG_TYPE`.
    fn lt_fsl_read(
        &mut self,
        fname: &[u8],
        inputs: &mut dyn TransformInputs,
    ) -> Result<Option<Lta>, Stop> {
        let Some(mut s) = read_file(self, inputs, fname) else {
            self.lt_error(codes::NO_FILE, "ltFSLread: could not open file ", fname, "");
            return Ok(None);
        };
        let mut rows = [[0f32; 4]; 3];
        for (r, row) in rows.iter_mut().enumerate() {
            let got = fgetl(&mut s, 1023).and_then(|l| self.four_floats(&l));
            let Some(v) = got else {
                let head = format!("ltFSLread: could not read row {} from ", r + 1);
                self.lt_error(codes::BAD_FILE, &head, fname, "");
                return Ok(None);
            };
            *row = v;
        }
        Ok(Some(self.lta_of_rows(lta_type::FSLREG_TYPE, &rows)?))
    }

    /// `ltMNIreadEx` (§21): MINC's `.xfm`, type RAS-to-RAS, the volumes its information lines
    /// name.
    fn lt_mni_read_ex(
        &mut self,
        fname: &[u8],
        inputs: &mut dyn TransformInputs,
    ) -> Result<Option<Lta>, Stop> {
        let Some(mut s) = read_file(self, inputs, fname) else {
            self.lt_error(codes::NO_FILE, "ltMNIreadEx: could not open file ", fname, "");
            return Ok(None);
        };
        // the line buffer: the last line fgetl gave (kept when it gives none)
        let mut buf: Vec<u8> = Vec::new();
        let next = |s: &mut CStream, buf: &mut Vec<u8>| -> bool {
            match fgetl(s, 1023) {
                Some(l) => {
                    *buf = l;
                    true
                }
                None => false,
            }
        };
        next(&mut s, &mut buf);
        if !buf.starts_with(b"MNI Transform File") {
            self.lt_error(
                codes::BAD_FILE,
                "ltMNIreadEx:",
                fname,
                " does not start as 'MNI Transform File'",
            );
            return Ok(None);
        }
        let mut info: Vec<Vec<u8>> = Vec::new();
        next(&mut s, &mut buf);
        if buf.first() == Some(&b'%') {
            info.push(buf.clone());
            next(&mut s, &mut buf);
            if buf.first() == Some(&b'%') {
                info.push(buf.clone());
                // the following `%` lines skipped; FS-307: at the end of the file the skip ends
                loop {
                    if !next(&mut s, &mut buf) {
                        if buf.first() == Some(&b'%') {
                            self.note(
                                &FS_307,
                                "an .xfm that ends in a % line after its second % line: the skip ends at the end of the file (FreeSurfer never returns)",
                            );
                        }
                        break;
                    }
                    if buf.first() != Some(&b'%') {
                        break;
                    }
                }
                next(&mut s, &mut buf);
                if buf.starts_with(b"Transform_Type") {
                    next(&mut s, &mut buf);
                }
            } else if buf.starts_with(b"Transform_Type") {
                next(&mut s, &mut buf);
            }
        } else if buf.starts_with(b"Transform_Type") {
            next(&mut s, &mut buf);
        }
        let mut rows = [[0f32; 4]; 3];
        for (r, row) in rows.iter_mut().enumerate() {
            let got = if next(&mut s, &mut buf) {
                let line = buf.clone();
                self.four_floats(&line)
            } else {
                None
            };
            let Some(v) = got else {
                let head = format!("ltMNIreadEx: could not read row {} from ", r + 1);
                let mut tail = b" (".to_vec();
                tail.extend_from_slice(&buf);
                tail.push(b')');
                let mut m = head.into_bytes();
                m.extend_from_slice(fname);
                m.extend_from_slice(&tail);
                self.error_printf(codes::BAD_FILE, &m);
                return Ok(None);
            };
            *row = v;
        }
        let mut lta = self.lta_of_rows(lta_type::LINEAR_RAS_TO_RAS, &rows)?;
        if let Some(first) = info.first().cloned() {
            let (src, dst) = self.mni_volume_names(&first, info.get(1).map(Vec::as_slice));
            let lt = &mut lta.xforms[0];
            for (name, g) in [(src, &mut lt.src), (dst, &mut lt.dst)] {
                if let Some(name) = name {
                    *g = match inputs.volume_geometry(self, &name) {
                        Some(h) => h,
                        None if contains(&name, b"average_305") => average_305(&name),
                        None => VolGeom {
                            fname: fixed_name(&name, 1024),
                            ..init_vol_geom()
                        },
                    };
                }
            }
            if self.env.is_set("USE_AVERAGE305") && lt.dst.valid == 0 {
                let name = lt.dst.fname.clone();
                lt.dst = average_305(&name);
                self.put_err(b"INFO: using average_305 info, since \n");
                self.put_err(b"INFO: environment var USE_AVERAGE305 set\n");
            }
        }
        Ok(Some(lta))
    }

    /// The source and destination volumes an `.xfm`'s information lines name (§21).
    fn mni_volume_names(
        &mut self,
        first: &[u8],
        second: Option<&[u8]>,
    ) -> (Option<Vec<u8>>, Option<Vec<u8>>) {
        let w = words(first);
        if first.starts_with(b"%Generated by") {
            // the word after the first word holding `src`, resp. `dst`; FS-310: none after it
            let mut after = |key: &[u8]| -> Option<Vec<u8>> {
                let k = w.iter().position(|x| contains(x, key))?;
                match w.get(k + 1) {
                    Some(x) => Some(x.to_vec()),
                    None => {
                        self.note(
                            &FS_310,
                            "an .xfm's %Generated by line ending with a word holding src or dst: no volume named (FreeSurfer crashes)",
                        );
                        None
                    }
                }
            };
            let src = after(b"src");
            let dst = after(b"dst");
            return (src, dst);
        }
        if contains(first, b"minctracc") {
            let mnc: Vec<&[u8]> = w.iter().copied().filter(|x| contains(x, b".mnc")).collect();
            return (
                mnc.first().map(|x| x.to_vec()),
                mnc.get(1).map(|x| x.to_vec()),
            );
        }
        let second_word = |line: &[u8]| -> Option<Vec<u8>> {
            words(line)
                .get(1)
                .filter(|x| contains(x, b".mnc"))
                .map(|x| x.to_vec())
        };
        let dst = second_word(first);
        let src = second.and_then(second_word);
        (src, dst)
    }

    /// `ltaReadRegisterDat` (§21): tkregister's `register.dat`.
    fn lta_read_register_dat(
        &mut self,
        fname: &[u8],
        inputs: &mut dyn TransformInputs,
    ) -> Result<Option<Lta>, Stop> {
        self.put_out(
            b"INFO: This REGISTER_DAT transform is valid only for volumes between  COR types with c_(r,a,s) = 0.\n",
        );
        let Some(reg) = self.regio_read_register(fname, inputs)? else {
            self.lt_error(codes::BAD_FILE, "ltaReadRegisterDat: could not read ", fname, "");
            return Ok(None);
        };
        let mut lta = self.lta_alloc(1, None)?;
        lta.kind = lta_type::REGISTER_DAT;
        lta.subject = fixed_name(&reg.subject, 1024);
        lta.fscale = reg.intensity;
        let lt = &mut lta.xforms[0];
        lt.sigma = 1.0;
        lt.m_l = Some(reg.matrix);
        lt.src.xsize = reg.inplane;
        lt.src.ysize = reg.inplane;
        lt.src.zsize = reg.betplane;
        lt.src.valid = 0;
        lt.dst.valid = 0;
        lt.kind = transform_type::UNKNOWN;
        Ok(Some(lta))
    }

    /// `regio_read_register(name, ...)` for a name that is not an `.lta`
    /// (`specs/freesurfer-io.md` §46): the subject, the resolutions, the intensity, the matrix and
    /// the rounding method; `None` after its messages.
    pub fn regio_read_register(
        &mut self,
        name: &[u8],
        inputs: &mut dyn TransformInputs,
    ) -> Result<Option<Register>, Stop> {
        let Some(mut s) = read_file(self, inputs, name) else {
            self.perror(b"regio_read_register()");
            let mut m = b"Could not open ".to_vec();
            m.extend_from_slice(name);
            m.push(b'\n');
            self.put_err(&m);
            return Ok(None);
        };
        let fail = |p: &mut Process, what: &str| {
            p.perror(b"regio_read_register()");
            let mut m = format!("Error reading {what} from ").into_bytes();
            m.extend_from_slice(name);
            m.push(b'\n');
            p.put_err(&m);
        };
        let Some(Got::S(subject)) = scanf(self, &mut s, b"%s").first().cloned() else {
            fail(self, "subject");
            return Ok(None);
        };
        if subject.len() >= 1000 {
            self.note(
                &FS_258,
                "a register.dat's subject of 1,000 bytes or more kept whole (FreeSurfer writes past its buffer)",
            );
        }
        let mut vals = [0f32; 3];
        for (k, what) in ["inplaneres", "betplaneres", "intensity"].iter().enumerate() {
            let Some(Got::F(x)) = scanf(self, &mut s, b"%f").first().cloned() else {
                fail(self, what);
                return Ok(None);
            };
            vals[k] = x;
        }
        let mut matrix = Matrix::zeros(4, 4, MATRIX_REAL);
        for r in 0..4 {
            for c in 0..4 {
                let Some(Got::F(x)) = scanf(self, &mut s, b"%f").first().cloned() else {
                    fail(self, &format!("R[{r}][{c}]"));
                    return Ok(None);
                };
                matrix.set(r + 1, c + 1, x);
            }
        }
        let method = match scanf(self, &mut s, b"%s").first().cloned() {
            None => 2,
            Some(Got::S(w)) => {
                if w.len() >= 1000 {
                    self.note(
                        &FS_258,
                        "a register.dat's method name of 1,000 bytes or more kept whole (FreeSurfer writes past its buffer)",
                    );
                }
                let lower = w.to_ascii_lowercase();
                if w == b"round" || w == b"rint" {
                    0
                } else if w == b"floor" {
                    1
                } else if lower.starts_with(b"tkreg") {
                    2
                } else {
                    let mut m = b"ERROR: regio_read_register(): float2int method ".to_vec();
                    m.extend_from_slice(&w);
                    m.extend_from_slice(b" from file ");
                    m.extend_from_slice(name);
                    m.extend_from_slice(b", match not found\n");
                    self.put_out(&m);
                    return Ok(None);
                }
            }
            Some(_) => 2,
        };
        Ok(Some(Register {
            subject,
            inplane: vals[0],
            betplane: vals[1],
            intensity: vals[2],
            matrix,
            method,
        }))
    }

    /// `perror(prefix)`: `<prefix>: <errno's text>` on standard error. Then `errno` is EINVAL, as
    /// glibc's `perror` leaves it on a standard error not yet oriented (`specs/freesurfer-io.md`
    /// §46; the volume probe's case).
    fn perror(&mut self, prefix: &[u8]) {
        let mut m = prefix.to_vec();
        m.extend_from_slice(b": ");
        m.extend_from_slice(crate::clib::strerror(self.errno).as_bytes());
        m.push(b'\n');
        self.put_err(&m);
        self.errno = EINVAL;
    }
}

/// What `regio_read_register` reads (`specs/freesurfer-io.md` §46).
#[derive(Clone, Debug, PartialEq)]
pub struct Register {
    pub subject: Vec<u8>,
    /// The in-plane resolution.
    pub inplane: f32,
    /// The between-plane resolution.
    pub betplane: f32,
    /// The intensity scale.
    pub intensity: f32,
    pub matrix: Matrix,
    /// The rounding method: 0 round, 1 floor, 2 tkregister's.
    pub method: i32,
}
