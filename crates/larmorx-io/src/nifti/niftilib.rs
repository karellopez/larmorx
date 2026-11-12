// SPDX-License-Identifier: Apache-2.0
//! nifticlib 2.0.0's file names (`nifti1_io.c`, public domain): the extensions it knows, the
//! header file a name stands for, and whether a file is NIfTI-1, ANALYZE or neither, with its
//! messages (FreeSurfer's NIfTI reader calls these).
//!
//! The extensions are `.nii`, `.hdr`, `.img`, `.nia` and, with zlib, `.nii.gz`, `.hdr.gz`,
//! `.img.gz`, in lower case or (with `allow_upper_fext`, nifticlib's default) all upper case; a
//! mixed-case one is refused with a message.

use larmorx_core::cfmt::Streams;

use crate::znz::ZnzFile;

/// nifticlib's global options that these functions read (`g_opts`: a debug level of 1 and
/// upper-case extensions allowed by default), and two points where copies of nifticlib differ
/// from 2.0.0 (FreeSurfer 8.2.0's: no upper-case `.GZ` for `nifti_is_gzfile`, and `.nia.gz`
/// among the compressed extensions).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NiftiOptions {
    pub debug: i32,
    pub allow_upper_fext: bool,
    /// Whether `nifti_is_gzfile` takes `.GZ` as well as `.gz` (2.0.0: yes).
    pub upper_case_gz: bool,
    /// Whether `.nia.gz` is a compressed extension (2.0.0: no).
    pub nia_gz: bool,
}

impl Default for NiftiOptions {
    fn default() -> Self {
        NiftiOptions {
            debug: 1,
            allow_upper_fext: true,
            upper_case_gz: true,
            nia_gz: false,
        }
    }
}

fn lossy(b: &[u8]) -> std::borrow::Cow<'_, str> {
    String::from_utf8_lossy(b)
}

/// C's string: the bytes before the first NUL.
fn cstr(s: &[u8]) -> &[u8] {
    &s[..s.iter().position(|&b| b == 0).unwrap_or(s.len())]
}

fn is_mixedcase(s: &[u8]) -> bool {
    s.iter().any(u8::is_ascii_lowercase) && s.iter().any(u8::is_ascii_uppercase)
}

fn is_uppercase(s: &[u8]) -> bool {
    !s.iter().any(u8::is_ascii_lowercase) && s.iter().any(u8::is_ascii_uppercase)
}

/// `nifti_find_file_extension`: the offset of the name's NIfTI extension, or `None`.
pub fn find_file_extension(
    name: &[u8],
    opts: &NiftiOptions,
    streams: &mut dyn Streams,
) -> Option<usize> {
    let name = cstr(name);
    let len = name.len();
    if len < 4 {
        return None;
    }
    let lower = |s: &[u8]| -> Vec<u8> {
        if opts.allow_upper_fext {
            s.to_ascii_lowercase()
        } else {
            s.to_vec()
        }
    };
    let ext = &name[len - 4..];
    if [&b".nii"[..], b".hdr", b".img", b".nia"].contains(&lower(ext).as_slice()) {
        if is_mixedcase(ext) {
            streams.err(&format!(
                "** mixed case extension '{}' is not valid\n",
                lossy(ext)
            ));
            return None;
        }
        return Some(len - 4);
    }
    if len < 7 {
        return None;
    }
    let ext = &name[len - 7..];
    let gz_list: &[&[u8]] = if opts.nia_gz {
        &[b".nii.gz", b".hdr.gz", b".img.gz", b".nia.gz"]
    } else {
        &[b".nii.gz", b".hdr.gz", b".img.gz"]
    };
    if gz_list.contains(&lower(ext).as_slice()) {
        if is_mixedcase(ext) {
            streams.err(&format!(
                "** mixed case extension '{}' is not valid\n",
                lossy(ext)
            ));
            return None;
        }
        return Some(len - 7);
    }
    if opts.debug > 1 {
        streams.err(&format!(
            "** find_file_ext: failed for name '{}'\n",
            lossy(name)
        ));
    }
    None
}

/// `fileext_compare`: equal, or equal to the known extension in upper case.
fn ext_equal(test: &[u8], known: &[u8]) -> bool {
    test == known || (known.len() <= 7 && test == known.to_ascii_uppercase().as_slice())
}

/// `nifti_is_gzfile`: the name ends in `.gz` (or `.GZ`).
pub fn is_gzfile(name: &[u8]) -> bool {
    is_gzfile_with(name, &NiftiOptions::default())
}

/// [`is_gzfile`] with a copy's options.
pub fn is_gzfile_with(name: &[u8], opts: &NiftiOptions) -> bool {
    let name = cstr(name);
    if name.len() < 3 {
        return false;
    }
    let tail = &name[name.len() - 3..];
    if opts.upper_case_gz {
        ext_equal(tail, b".gz")
    } else {
        tail == b".gz"
    }
}

/// `nifti_makebasename`: the name without its NIfTI extension.
pub fn makebasename(name: &[u8], opts: &NiftiOptions, streams: &mut dyn Streams) -> Vec<u8> {
    let name = cstr(name);
    match find_file_extension(name, opts, streams) {
        Some(at) => name[..at].to_vec(),
        None => name.to_vec(),
    }
}

/// `nifti_validfilename`: not empty, and not an extension alone.
pub fn validfilename(name: &[u8], opts: &NiftiOptions, streams: &mut dyn Streams) -> bool {
    let name = cstr(name);
    if name.is_empty() {
        if opts.debug > 1 {
            streams.err("-- empty filename in nifti_validfilename()\n");
        }
        return false;
    }
    if find_file_extension(name, opts, streams) == Some(0) {
        if opts.debug > 0 {
            streams.err(&format!("-- no prefix for filename '{}'\n", lossy(name)));
        }
        return false;
    }
    true
}

/// `nifti_is_complete_filename`: a valid name that has a NIfTI extension.
pub fn is_complete_filename(name: &[u8], opts: &NiftiOptions, streams: &mut dyn Streams) -> bool {
    let name = cstr(name);
    if name.is_empty() {
        if opts.debug > 1 {
            streams.err("-- empty filename in nifti_validfilename()\n");
        }
        return false;
    }
    match find_file_extension(name, opts, streams) {
        None => {
            if opts.debug > 0 {
                streams.err(&format!(
                    "-- no nifti valid extension for filename '{}'\n",
                    lossy(name)
                ));
            }
            false
        }
        Some(0) => {
            if opts.debug > 0 {
                streams.err(&format!("-- no prefix for filename '{}'\n", lossy(name)));
            }
            false
        }
        Some(_) => true,
    }
}

/// `nifti_fileexists`: `znzopen(name, "rb", 1)` succeeds.
pub fn fileexists(name: &[u8]) -> bool {
    ZnzFile::open(cstr(name), b"rb", true).is_some_and(|f| {
        f.close();
        true
    })
}

/// `nifti_findhdrname`: the header file a name stands for: the name itself when it exists and is
/// not an `.img` file; otherwise the base name with `.nii`, `.nii.gz`, `.hdr`, `.hdr.gz` (`.hdr`
/// first for an `.img` name; upper case for an upper-case extension), the first that exists.
pub fn findhdrname(name: &[u8], opts: &NiftiOptions, streams: &mut dyn Streams) -> Option<Vec<u8>> {
    let name = cstr(name);
    if !validfilename(name, opts, streams) {
        return None;
    }
    let basename = makebasename(name, opts, streams);
    let ext = find_file_extension(name, opts, streams);
    let mut efirst = 1usize;
    let eisupper = ext.is_some_and(|at| is_uppercase(&name[at..]));
    if let Some(at) = ext {
        if fileexists(name) {
            let e = &name[at..];
            // fileext_n_compare(ext, ".img", 4) != 0
            let img = e.len() >= 4 && (&e[..4] == b".img" || &e[..4] == b".IMG");
            if !img {
                return Some(name.to_vec());
            }
            efirst = 0;
        }
    }
    let mut elist: [Vec<u8>; 2] = [b".hdr".to_vec(), b".nii".to_vec()];
    let mut extzip = b".gz".to_vec();
    if eisupper {
        for e in elist.iter_mut() {
            e.make_ascii_uppercase();
        }
        extzip.make_ascii_uppercase();
    }
    for _ in 0..2 {
        let mut hdr = basename.clone();
        hdr.extend_from_slice(&elist[efirst]);
        if fileexists(&hdr) {
            return Some(hdr);
        }
        hdr.extend_from_slice(&extzip);
        if fileexists(&hdr) {
            return Some(hdr);
        }
        efirst = 1 - efirst;
    }
    None
}

/// `is_nifti_file`: 1 for a NIfTI-1 file (`n+1`), 2 for a NIfTI-1 pair (`ni1`), 0 for ANALYZE
/// (`sizeof_hdr` 348, in either byte order), -1 otherwise (no such file, a short header, ...).
pub fn is_nifti_file(name: &[u8], opts: &NiftiOptions, streams: &mut dyn Streams) -> i32 {
    let name = cstr(name);
    if !validfilename(name, opts, streams) {
        return -1;
    }
    let Some(tmpname) = findhdrname(name, opts, streams) else {
        if opts.debug > 0 {
            streams.err(&format!("** no header file found for '{}'\n", lossy(name)));
        }
        return -1;
    };
    let Some(mut fp) = ZnzFile::open(&tmpname, b"rb", is_gzfile_with(&tmpname, opts)) else {
        return -1;
    };
    let mut hdr = [0u8; 348];
    let ii = fp.read(&mut hdr, 1, 348, streams);
    fp.close();
    if ii < 348 {
        return -1;
    }
    let magic = &hdr[344..348];
    let version = magic[0] == b'n'
        && magic[3] == 0
        && (magic[1] == b'i' || magic[1] == b'+')
        && (b'1'..=b'9').contains(&magic[2]);
    if version {
        return if magic[1] == b'+' { 1 } else { 2 };
    }
    let sizeof_hdr = i32::from_ne_bytes([hdr[0], hdr[1], hdr[2], hdr[3]]);
    if sizeof_hdr == 348 || sizeof_hdr.swap_bytes() == 348 {
        return 0;
    }
    -1
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_core::cfmt::{Captured, Stream};

    #[test]
    fn extensions() {
        let o = NiftiOptions::default();
        let mut c = Captured::default();
        assert_eq!(find_file_extension(b"a.nii", &o, &mut c), Some(1));
        assert_eq!(find_file_extension(b"a.NII.GZ", &o, &mut c), Some(1));
        assert_eq!(find_file_extension(b"a.nia.gz", &o, &mut c), None);
        assert_eq!(find_file_extension(b"a.Nii", &o, &mut c), None);
        assert_eq!(
            c.text(Stream::Err),
            "** mixed case extension '.Nii' is not valid\n"
        );
        assert!(is_gzfile(b"x.GZ"));
        assert!(!is_gzfile(b"x.Gz"));
        let mut c = Captured::default();
        assert!(!validfilename(b".nii", &o, &mut c));
        assert_eq!(c.text(Stream::Err), "-- no prefix for filename '.nii'\n");
        assert_eq!(makebasename(b"d/a.hdr.gz", &o, &mut c), b"d/a".to_vec());
    }
}
