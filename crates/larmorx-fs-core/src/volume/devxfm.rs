// SPDX-License-Identifier: Apache-2.0
//! `DevolveXFMWithSubjectsDir` and `DevolveXFM` (spec §63): a subject's transform made to work
//! with tkregister's coordinates, `XFM (N inv(T))`, from the subject's original volume's header
//! (an MGH or MGZ header as `specs/freesurfer-io.md` §43 reads it alone, or the slice folder's
//! error) and its transform (`LTAreadEx`). FS-887 for paths of 2,000 bytes or more; FS-167 for a
//! header cut by the end of its file.

use std::io::Read;

use larmorx_core::defects::{FS_167, FS_887};

use super::ltaread::FileSystem;
use crate::files::path_of;
use crate::maths::matrix::Matrix;
use crate::maths::{Output, Returned};
use crate::mri::geom::DIAG_SHOW;
use crate::mri::lta::lta_type;
use crate::mri::{Mri, types};
use crate::process::{DIAG_VERBOSE, Process, Stop, codes};

/// `ENOMEM`.
const ENOMEM: i32 = 12;

/// The longest path FreeSurfer builds without writing past its buffer (bytes, with the NUL).
const PATH_BUFFER: usize = 2000;

/// A frame count FreeSurfer's frame records cannot be allocated for (black box: 1,736,075,632
/// frames fail on the reference machine; a clean-room bound well above any volume's).
const FRAMES_LIMIT: i64 = 1 << 24;

/// The bytes of a file as zlib's `gzread` gives them: a gzip stream decompressed (its members
/// one after the other; the bytes before a damaged part), any other file as it is (zlib's
/// transparent mode).
fn gz_bytes(raw: &[u8]) -> Vec<u8> {
    if raw.len() < 2 || raw[0] != 0x1f || raw[1] != 0x8b {
        return raw.to_vec();
    }
    let mut out = Vec::new();
    let mut d = flate2::read::MultiGzDecoder::new(raw);
    let mut buf = [0u8; 8192];
    loop {
        match d.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => out.extend_from_slice(&buf[..n]),
        }
    }
    out
}

/// The header's numbers read one after the other, big-endian, as fs.io's znz readers give them
/// at the end of the file: the missing bytes of an `int` are the previous `int`'s file-order bytes
/// with `reproduce` (FS-167), zeros by default; a short's and a float's zeros in both modes.
struct Header<'a> {
    b: &'a [u8],
    pos: usize,
    slot: [u8; 4],
    cut: bool,
}

impl Header<'_> {
    fn take(&mut self, n: usize) -> &[u8] {
        let end = (self.pos + n).min(self.b.len());
        let s = &self.b[self.pos..end];
        self.pos = end;
        if s.len() < n {
            self.cut = true;
        }
        s
    }

    fn int(&mut self, reproduce: bool) -> i32 {
        let s = self.take(4).to_vec();
        let mut v = if reproduce { self.slot } else { [0u8; 4] };
        v[..s.len()].copy_from_slice(&s);
        self.slot = v;
        i32::from_be_bytes(v)
    }

    fn short(&mut self) -> i16 {
        let s = self.take(2);
        let mut v = [0u8; 2];
        v[..s.len()].copy_from_slice(s);
        i16::from_be_bytes(v)
    }

    fn float(&mut self) -> f32 {
        let s = self.take(4);
        let mut v = [0u8; 4];
        v[..s.len()].copy_from_slice(s);
        f32::from_be_bytes(v)
    }
}

impl Process {
    /// FS-887 (both modes): a path of 2,000 bytes or more ends the program.
    fn devxfm_path(&mut self, path: &[u8]) -> Result<(), Stop> {
        if path.len() >= PATH_BUFFER {
            self.note(
                &FS_887,
                format!("DevolveXFM: a path of {} bytes: the program ended (FreeSurfer writes past its 2,000 bytes)", path.len()),
            );
            self.error_exit(
                codes::BAD_PARM,
                format!("DevolveXFM: a path of {} bytes (FreeSurfer writes past its buffer here)\n", path.len())
                    .as_bytes(),
            )?;
        }
        Ok(())
    }

    /// The header of an MGH or MGZ volume (`specs/freesurfer-io.md` §43, the header alone): a
    /// volume without voxels of the header's extent, type and frames, its geometry (the coronal
    /// orientation without the RAS flag). `None` where the reader gives none.
    pub fn mgh_read_header(&mut self, name: &[u8]) -> Result<Option<Mri>, Stop> {
        let Ok(raw) = std::fs::read(path_of(name)) else {
            let mut m = b"mghRead(".to_vec();
            m.extend_from_slice(name);
            m.extend_from_slice(b", -1): could not open file");
            self.error_printf(codes::BAD_FILE, &m);
            return Ok(None);
        };
        let lower = name.to_ascii_lowercase();
        let compressed = lower.ends_with(b".mgz") || lower.windows(6).any(|w| w == b"mgh.gz");
        let bytes = if compressed { gz_bytes(&raw) } else { raw };
        let reproduce = self.mode().reproduce();
        let mut h = Header {
            b: &bytes,
            pos: 0,
            slot: [0; 4],
            cut: false,
        };
        let version = h.take(4).to_vec();
        if version.len() < 4 {
            let mut m = b"mghRead(".to_vec();
            m.extend_from_slice(name);
            m.extend_from_slice(b", -1): read error");
            self.error_printf(codes::BAD_FILE, &m);
            return Ok(None);
        }
        h.slot.copy_from_slice(&version);
        let w = h.int(reproduce);
        let ht = h.int(reproduce);
        let d = h.int(reproduce);
        let mut nframes = h.int(reproduce);
        let kind = h.int(reproduce);
        let _dof = h.int(reproduce);
        let flag = h.short();
        let mut f = [0.0f32; 15];
        if flag > 0 {
            for v in f.iter_mut() {
                *v = h.float();
            }
        }
        if h.cut {
            if reproduce {
                self.note(
                    &FS_167,
                    "an MGH header cut by the end of its file: each missing int's bytes are the int read before it's",
                );
            } else {
                self.note(
                    &FS_167,
                    "an MGH header cut by the end of its file: its missing bytes read as zeros (FreeSurfer's are memory contents)",
                );
            }
        }
        if kind == types::TENSOR {
            nframes = 9;
        }
        if i64::from(nframes) > FRAMES_LIMIT {
            self.errno = ENOMEM;
            self.error_exit(
                codes::NO_MEMORY,
                format!("MRIalloc: could not allocate {nframes} frame\n").as_bytes(),
            )?;
            return Ok(None);
        }
        let mut m = self.mri_alloc_header(w, ht, d, kind, nframes)?;
        if flag > 0 {
            let g = &mut m.geom;
            g.xsize = f[0];
            g.ysize = f[1];
            g.zsize = f[2];
            g.x = [f[3], f[4], f[5]];
            g.y = [f[6], f[7], f[8]];
            g.z = [f[9], f[10], f[11]];
            g.c = [f[12], f[13], f[14]];
            g.ras_good_flag = 1;
            m.ps = f[0];
            m.thick = f[2];
        } else {
            let g = &mut m.geom;
            g.x = [-1.0, 0.0, 0.0];
            g.y = [0.0, 0.0, -1.0];
            g.z = [0.0, 1.0, 0.0];
            g.c = [0.0, 0.0, 0.0];
            if self.gdiag & DIAG_SHOW != 0 && self.gdiag & DIAG_VERBOSE != 0 {
                let rule = "-".repeat(65);
                let mut b = format!("{rule}\nCould not find the direction cosine information.\nWill use the CORONAL orientation.\nIf not suitable, please provide the information in ").into_bytes();
                b.extend_from_slice(name);
                b.extend_from_slice(format!(".\n{rule}\n").as_bytes());
                self.put_err(&b);
            }
        }
        self.reinit_cache(&mut m.geom)?;
        Ok(Some(m))
    }

    /// `DevolveXFM(subject, XFM, xfmname)` (§63): [`Process::devolve_xfm_with_subjects_dir`]
    /// without a subjects' folder.
    pub fn devolve_xfm(
        &mut self,
        subject: &[u8],
        xfm: Option<&mut Matrix>,
        xfmname: Option<&[u8]>,
    ) -> Result<Returned<Matrix>, Stop> {
        self.devolve_xfm_with_subjects_dir(subject, xfm, xfmname, None)
    }

    /// `DevolveXFMWithSubjectsDir(subject, XFM, xfmname, sdir)` (§63): the subject's original
    /// volume's header (`orig.mgz`, `orig.mgh`, the slice folder `orig`), the transform read
    /// (made coronal RAS when voxel-to-voxel) unless given, then `XFM (N inv(T))`. Nothing
    /// (after its line on standard output) where a step fails.
    pub fn devolve_xfm_with_subjects_dir(
        &mut self,
        subject: &[u8],
        xfm: Option<&mut Matrix>,
        xfmname: Option<&[u8]>,
        sdir: Option<&[u8]>,
    ) -> Result<Returned<Matrix>, Stop> {
        let sd = match sdir {
            Some(s) => s.to_vec(),
            None => match self.env.get("SUBJECTS_DIR") {
                Some(s) => s.to_vec(),
                None => {
                    self.put_out(b"ERROR: SUBJECTS_DIR not defined\n");
                    return Ok(Returned::Nothing);
                }
            },
        };
        let mut base = sd.clone();
        base.push(b'/');
        base.extend_from_slice(subject);
        self.devxfm_path(&base)?;
        if !std::fs::metadata(path_of(&base)).is_ok_and(|m| m.is_dir()) {
            let mut b = b"ERROR: cannot find subject ".to_vec();
            b.extend_from_slice(subject);
            b.extend_from_slice(b" in ");
            b.extend_from_slice(&sd);
            b.push(b'\n');
            self.put_out(&b);
            return Ok(Returned::Nothing);
        }
        let mut orig = base.clone();
        orig.extend_from_slice(b"/mri/orig");
        self.devxfm_path(&orig)?;
        let show = self.gdiag & DIAG_SHOW != 0 && self.gdiag & DIAG_VERBOSE != 0;
        let mut header: Option<Mri> = None;
        for ext in [&b".mgz"[..], b".mgh"] {
            let mut name = orig.clone();
            name.extend_from_slice(ext);
            if show {
                let mut b = b"Trying ".to_vec();
                b.extend_from_slice(&name);
                b.push(b'\n');
                self.put_out(&b);
            }
            if std::fs::File::open(path_of(&name)).is_err() {
                continue;
            }
            if let Some(m) = self.mgh_read_header(&name)? {
                header = Some(m);
                break;
            }
        }
        if header.is_none() {
            if show {
                let mut b = b"Trying ".to_vec();
                b.extend_from_slice(&orig);
                b.push(b'\n');
                self.put_out(&b);
            }
            if std::fs::metadata(path_of(&orig)).is_ok_and(|m| m.is_dir()) {
                // the slice folder's reader: its description file must open (not probed beyond)
                let mut info = orig.clone();
                info.extend_from_slice(b"/COR-.info");
                self.errno = 0;
                let mut m = b"corRead(): can't open file ".to_vec();
                m.extend_from_slice(&info);
                self.error_printf(codes::BAD_FILE, &m);
            }
        }
        let Some(header) = header else {
            let mut b = b"ERROR: could not read header for ".to_vec();
            b.extend_from_slice(&orig);
            b.push(b'\n');
            self.put_out(&b);
            return Ok(Returned::Nothing);
        };
        // the transform, unless given
        let mut own: Option<Matrix> = None;
        if xfm.is_none() {
            let mut path = base.clone();
            path.extend_from_slice(b"/mri/transforms/");
            path.extend_from_slice(xfmname.unwrap_or(b"talairach.xfm"));
            self.devxfm_path(&path)?;
            if std::fs::File::open(path_of(&path)).is_err() {
                let mut b = b"ERROR: could not open ".to_vec();
                b.extend_from_slice(&path);
                b.extend_from_slice(b" for reading \n");
                self.put_out(&b);
                return Ok(Returned::Nothing);
            }
            let Some(mut lta) = self.lta_read_ex(&path, &mut FileSystem)? else {
                let mut b = b"ERROR: reading ".to_vec();
                b.extend_from_slice(&path);
                b.push(b'\n');
                self.put_out(&b);
                return Ok(Returned::Nothing);
            };
            if lta.kind == lta_type::LINEAR_VOX_TO_VOX {
                self.lta_voxel_transform_to_coronal_ras_transform(&mut lta)?;
            }
            if lta.xforms.is_empty() {
                self.lta_fs504("DevolveXFM", "a transform file without transforms")?;
                return Ok(Returned::Nothing);
            }
            own = match self.matrix_copy(lta.xforms[0].m_l.as_ref(), Output::New)? {
                Returned::New(m) => Some(m),
                _ => None,
            };
        }
        let n = self.extract_i_to_r(&header.geom)?;
        let t = self.vox2ras_tkreg(&header.geom)?;
        let ti = match self.matrix_inverse(Some(&t), None)? {
            Returned::New(m) => Some(m),
            _ => None,
        };
        let nt = match self.matrix_multiply(Some(&n), ti.as_ref(), None)? {
            Returned::New(m) => Some(m),
            _ => None,
        };
        match xfm {
            Some(x) => {
                let cur = x.clone();
                match self.matrix_multiply(Some(&cur), nt.as_ref(), Some(x))? {
                    Returned::Nothing => Ok(Returned::Nothing),
                    _ => Ok(Returned::Output),
                }
            }
            None => {
                let Some(mut x) = own else {
                    return Ok(Returned::Nothing);
                };
                let cur = x.clone();
                match self.matrix_multiply(Some(&cur), nt.as_ref(), Some(&mut x))? {
                    Returned::Nothing => Ok(Returned::Nothing),
                    _ => Ok(Returned::New(x)),
                }
            }
        }
    }
}
