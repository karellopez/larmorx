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
use crate::mri::lta::{Lta, init_vol_geom, lta_type};
use crate::process::{DIAG_VERBOSE, Process, Stop, codes};

/// `ENOENT`.
const ENOENT: i32 = 2;

/// What a transform reader reads (§21): the transform file's bytes and the geometries of the
/// volumes a MINC transform names. The defaults read the file system: a file's bytes, `errno` when
/// it cannot be opened; a named volume that does not exist gives its `errno` (`ENOENT`), and one
/// that exists is not read here (the default geometry, as for a header that does not read; a
/// program gives `specs/freesurfer-io.md`'s header reader by implementing
/// [`TransformInputs::volume_geometry`]).
pub trait TransformInputs {
    /// The bytes of the file `name`, or the `errno` of a failed open.
    fn read_file(&mut self, name: &[u8]) -> Result<Vec<u8>, i32> {
        std::fs::read(crate::files::path_of(name)).map_err(|e| e.raw_os_error().unwrap_or(ENOENT))
    }

    /// The volume `name`: `Ok(Some(g))` its header's geometry, `Ok(None)` a volume that exists
    /// but whose header does not read, `Err(errno)` one that does not exist (`stat` fails).
    fn volume_geometry(&mut self, _p: &mut Process, name: &[u8]) -> Result<Option<VolGeom>, i32> {
        match std::fs::metadata(crate::files::path_of(name)) {
            Ok(_) => Ok(None),
            Err(e) => Err(e.raw_os_error().unwrap_or(ENOENT)),
        }
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

/// `fgetl(buf, n, fp)` through its line buffer (§21's `.xfm` and FSL readers): the returned text
/// (without its leading white space, cut at its `#`, without its `\n`), `None` at the end of the
/// file. `buf` holds the piece `fgets` stored last: a returned line cut at its `#` and without its
/// `\n` (its leading white space kept); a comment or blank piece skipped before the end of the
/// file as `fgets` read it; unchanged at the end of the file with nothing read.
fn fgetl_buf(s: &mut CStream, n: usize, buf: &mut Vec<u8>) -> Option<Vec<u8>> {
    loop {
        let piece = s.fgets(n)?;
        let mut piece = crate::strings::c_str(&piece).to_vec();
        let start = piece
            .iter()
            .position(|&c| !is_c_space(c))
            .unwrap_or(piece.len());
        if start == piece.len() || piece[start] == b'#' {
            *buf = piece;
            continue;
        }
        if let Some(i) = piece.iter().position(|&c| c == b'#') {
            piece.truncate(i);
        }
        if piece.last() == Some(&b'\n') {
            piece.pop();
        }
        let text = piece[start..].to_vec();
        *buf = piece;
        return Some(text);
    }
}

/// The words of an `.xfm`'s information line (§21: split at spaces; a tab belongs to a word).
fn space_words(line: &[u8]) -> Vec<&[u8]> {
    line.split(|&c| c == b' ')
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
        // the name cut at its first `#`; more than 6 bytes ending in `nii.gz`: a morph
        let cut_at = fname.iter().position(|&c| c == b'#').unwrap_or(fname.len());
        let cut = &fname[..cut_at];
        if cut.len() > 6 && cut.ends_with(b"nii.gz") {
            return file_type::MORPH;
        }
        // the extension: after the last `@` of the whole name (an `@` before the `#` ends at
        // the `#`), else after the last `.` of the cut name; the cut name upper-cased, the text
        // after the `#` as written
        let ext: Vec<u8> = match fname.iter().rposition(|&c| c == b'@') {
            Some(at) if at < cut_at => cut[at + 1..].to_ascii_uppercase(),
            Some(at) => fname[at + 1..].to_vec(),
            None => match cut.iter().rposition(|&c| c == b'.') {
                Some(d) => cut[d + 1..].to_ascii_uppercase(),
                None => return file_type::LTA,
            },
        };
        match ext.as_slice() {
            b"M3D" | b"M3Z" | b"MGZ" | b"NII" => file_type::MORPH,
            b"OCT" | b"LTA" => file_type::LTA,
            b"XFM" => file_type::MINC,
            b"FSLMAT" => file_type::FSL,
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
        if crate::strings::file_name_only(fname) == b"identity.nofile" {
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
            // FS-309, both modes: the end of the program (FreeSurfer crashes)
            self.note(
                &FS_309,
                "an .lta of a single line ends the program (FreeSurfer crashes)",
            );
            let mut m = b"ltaReadFile(".to_vec();
            m.extend_from_slice(fname);
            m.extend_from_slice(b"): no second line (FreeSurfer crashes here)\n");
            self.error_exit(codes::BAD_FILE, &m)?;
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
            // FS-308, both modes: the unset value taken as 0
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

    /// The array of one transform of the MINC and FSL readers (§21): `LTAalloc(1, NULL)`'s, its
    /// `sigma` 1 and its mean (0, 0, 0).
    fn lta_one(&mut self) -> Result<Lta, Stop> {
        let mut lta = self.lta_alloc(1, None)?;
        let lt = &mut lta.xforms[0];
        lt.sigma = 1.0;
        (lt.x0, lt.y0, lt.z0) = (0.0, 0.0, 0.0);
        Ok(lta)
    }

    /// Row `r` (1 to 3) of a transform's matrix from a line's text scanned with `%f %f %f %f`
    /// (§21, step 4): the numbers not matched keep their values.
    fn scan_row(&mut self, m: Option<&mut Matrix>, r: i32, text: &[u8]) {
        let v = sscanf(self, text, b"%f %f %f %f");
        if let Some(m) = m {
            for (k, g) in v.iter().take(4).enumerate() {
                if let Got::F(x) = g {
                    m.set(r, k as i32 + 1, *x);
                }
            }
        }
    }

    /// `ltFSLread` (§21): FSL's matrix, the array's type `FSLREG_TYPE`.
    fn lt_fsl_read(
        &mut self,
        fname: &[u8],
        inputs: &mut dyn TransformInputs,
    ) -> Result<Option<Lta>, Stop> {
        let Some(mut s) = read_file(self, inputs, fname) else {
            self.lt_error(codes::NO_FILE, "ltFSLread: could not open file ", fname, "");
            return Ok(None);
        };
        let mut lta = self.lta_one()?;
        let mut buf = Vec::new();
        for r in 1..=3 {
            let Some(text) = fgetl_buf(&mut s, 900, &mut buf) else {
                let head = format!("ltFSLread: could not read row {r} from ");
                self.lt_error(codes::BAD_FILE, &head, fname, "");
                return Ok(None);
            };
            self.scan_row(lta.xforms[0].m_l.as_mut(), r, &text);
        }
        lta.kind = lta_type::FSLREG_TYPE;
        Ok(Some(lta))
    }

    /// `ltMNIreadEx` (§21): MINC's `.xfm`, the array's type RAS-to-RAS, the volumes its
    /// information lines name. The line tests read `fgetl`'s buffer (§21's steps 1 to 6).
    fn lt_mni_read_ex(
        &mut self,
        fname: &[u8],
        inputs: &mut dyn TransformInputs,
    ) -> Result<Option<Lta>, Stop> {
        let Some(mut s) = read_file(self, inputs, fname) else {
            self.lt_error(
                codes::NO_FILE,
                "ltMNIreadEx: could not open file ",
                fname,
                "",
            );
            return Ok(None);
        };
        let mut lta = self.lta_one()?;
        let mut buf: Vec<u8> = Vec::new();
        let pct = |b: &[u8]| b.first() == Some(&b'%');
        // 1
        fgetl_buf(&mut s, 900, &mut buf);
        if !buf.starts_with(b"MNI Transform File") {
            self.lt_error(
                codes::NO_FILE,
                "ltMNIreadEx:",
                fname,
                " does not start as 'MNI Transform File'",
            );
            return Ok(None);
        }
        // 2, 3
        let mut info: Option<(Vec<u8>, Vec<u8>)> = None;
        fgetl_buf(&mut s, 900, &mut buf);
        if pct(&buf) {
            let i1 = buf.clone();
            fgetl_buf(&mut s, 900, &mut buf);
            let mut i2 = Vec::new();
            if pct(&buf) {
                i2 = buf.clone();
                while pct(&buf) {
                    if fgetl_buf(&mut s, 900, &mut buf).is_none() && pct(&buf) {
                        // FS-307, both modes: FreeSurfer never returns here
                        self.note(
                            &FS_307,
                            "an .xfm whose last piece is a % line after its second % line ends the program (FreeSurfer never returns)",
                        );
                        let mut m = b"ltMNIreadEx: ".to_vec();
                        m.extend_from_slice(fname);
                        m.extend_from_slice(
                            b" ends with a % line (FreeSurfer never returns here)\n",
                        );
                        self.error_exit(codes::BAD_FILE, &m)?;
                        return Ok(None);
                    }
                }
                fgetl_buf(&mut s, 900, &mut buf);
                if buf.starts_with(b"Transform_Type") {
                    fgetl_buf(&mut s, 900, &mut buf);
                }
            } else if buf.starts_with(b"Transform_Type") {
                fgetl_buf(&mut s, 900, &mut buf);
            }
            info = Some((i1, i2));
        } else if buf.starts_with(b"Transform_Type") {
            fgetl_buf(&mut s, 900, &mut buf);
        }
        // 4
        for r in 1..=3 {
            let Some(text) = fgetl_buf(&mut s, 900, &mut buf) else {
                let mut m = format!("ltMNIreadEx: could not read row {r} from ").into_bytes();
                m.extend_from_slice(fname);
                m.extend_from_slice(b" (");
                m.extend_from_slice(&buf);
                m.push(b')');
                self.error_printf(codes::BAD_FILE, &m);
                return Ok(None);
            };
            self.scan_row(lta.xforms[0].m_l.as_mut(), r, &text);
        }
        // 5
        if let Some((i1, i2)) = info {
            let (src, dst) = self.mni_volume_names(&i1, &i2)?;
            let verbose = self.gdiag & DIAG_VERBOSE != 0;
            let show = verbose && self.gdiag & crate::mri::geom::DIAG_SHOW != 0;
            let mut geoms = Vec::with_capacity(2);
            for name in [&src, &dst] {
                let g = match name {
                    None => init_vol_geom(),
                    Some(n) => {
                        let mut g = match inputs.volume_geometry(self, n) {
                            Ok(Some(h)) => h,
                            Ok(None) => init_vol_geom(),
                            Err(e) => {
                                self.errno = e;
                                if contains(n, b"average_305") {
                                    if show {
                                        self.put_out(
                                            b"INFO: The transform was made with average_305.mnc.\n",
                                        );
                                    }
                                    average_305(n)
                                } else {
                                    init_vol_geom()
                                }
                            }
                        };
                        g.fname = fixed_name(n, 1024);
                        g
                    }
                };
                geoms.push(g);
            }
            let lt = &mut lta.xforms[0];
            lt.dst = geoms.pop().expect("two geometries");
            lt.src = geoms.pop().expect("two geometries");
            if self.env.is_set("USE_AVERAGE305") && lt.dst.valid == 0 {
                let name = lt.dst.fname.clone();
                lt.dst = average_305(&name);
                self.put_err(b"INFO: using average_305 info, since \n");
                self.put_err(b"INFO: environment var USE_AVERAGE305 set\n");
            }
        }
        // 6
        lta.kind = lta_type::LINEAR_RAS_TO_RAS;
        Ok(Some(lta))
    }

    /// The source and destination volumes an `.xfm`'s information lines `I1` and `I2` name
    /// (§21, step 5), each name printed with verbose diagnostics. FS-310: a `src` or `dst` word
    /// with no word after it ends the program (both modes).
    #[allow(clippy::type_complexity)]
    fn mni_volume_names(
        &mut self,
        i1: &[u8],
        i2: &[u8],
    ) -> Result<(Option<Vec<u8>>, Option<Vec<u8>>), Stop> {
        let verbose = self.gdiag & DIAG_VERBOSE != 0;
        let say = |p: &mut Process, src: bool, n: &[u8]| {
            if verbose {
                let mut m = if src {
                    b"INFO: Src volume ".to_vec()
                } else {
                    b"INFO: Target volume ".to_vec()
                };
                m.extend_from_slice(n);
                m.push(b'\n');
                p.put_out(&m);
            }
        };
        let w = space_words(i1);
        let (mut src, mut dst) = (None, None);
        if contains(i1, b"%Generated by") {
            let mut k = 0;
            while k < w.len() {
                let is_src = contains(w[k], b"src");
                if is_src || contains(w[k], b"dst") {
                    let Some(next) = w.get(k + 1) else {
                        self.note(
                            &FS_310,
                            "an .xfm's %Generated by line ending with a word holding src or dst ends the program (FreeSurfer crashes)",
                        );
                        self.error_exit(
                            codes::BAD_FILE,
                            b"mincFindVolume: no word after src or dst (FreeSurfer crashes here)\n",
                        )?;
                        return Ok((None, None));
                    };
                    say(self, is_src, next);
                    if is_src {
                        src = Some(next.to_vec());
                        k += 2;
                        continue;
                    }
                    dst = Some(next.to_vec());
                    break;
                }
                k += 1;
            }
        } else if contains(i1, b"minctracc") {
            for x in w.iter().filter(|x| contains(x, b".mnc")) {
                if self.mnc_count == 0 {
                    say(self, true, x);
                    src = Some(x.to_vec());
                    self.mnc_count = 1;
                } else {
                    say(self, false, x);
                    dst = Some(x.to_vec());
                    self.mnc_count = 0;
                    break;
                }
            }
        } else {
            dst = w
                .get(1)
                .filter(|x| contains(x, b".mnc"))
                .map(|x| x.to_vec());
            if w.len() > 1 {
                src = space_words(i2)
                    .get(1)
                    .filter(|x| contains(x, b".mnc"))
                    .map(|x| x.to_vec());
            }
            if let Some(n) = &src {
                say(self, true, n);
            }
            if let Some(n) = &dst {
                say(self, false, n);
            }
        }
        Ok((src, dst))
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
            self.lt_error(
                codes::BAD_FILE,
                "ltaReadRegisterDat: could not read ",
                fname,
                "",
            );
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
        for (k, what) in ["inplaneres", "betplaneres", "intensity"]
            .iter()
            .enumerate()
        {
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

    /// `perror(prefix)`: `<prefix>: <errno's text>` on standard error; `errno` stays as it was
    /// (the C library's `perror` on a standard error a program has written to:
    /// `specs/freesurfer-io.md` §46, spec §21).
    fn perror(&mut self, prefix: &[u8]) {
        let mut m = prefix.to_vec();
        m.extend_from_slice(b": ");
        m.extend_from_slice(crate::clib::strerror(self.errno).as_bytes());
        m.push(b'\n');
        self.put_err(&m);
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
