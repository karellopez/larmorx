// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `fscnpy` (`utils/fscnpy.cpp` 1-537, `include/fscnpy.h`): FreeSurfer's reader of NumPy's `.npy`
//! files, `fscnpy::npy2mri` (`MRIread`'s `NPY_FILE`), with the class's constructor and its
//! `__parse_header_dictionary`, `__getmritype`, `__getarchendian` and `__contiguousreorder`.
//! `fscnpy::read` (no program calls it) and `fscnpy::write` (empty) are not ported.
//!
//! The header's text is parsed as FreeSurfer parses it, in place (the NUL bytes it writes over a
//! key's colon, the descr's closing quote, fortran_order's comma and the shape's separators
//! decide what later comparisons see). The shape is kept as FreeSurfer's `std::vector<int>` keeps
//! it ([`ShapeVec`]: libstdc++'s growth by doubling, the places `pop_back` and `erase` leave
//! behind), because the volume's constructor reads past a list of fewer than 3 extents (FS-206).
//! The machine's byte order is little-endian (`<`) on every platform larmorx supports.
//!
//! Defects (rules A to D; `docs/findings/freesurfer-io.md`, Part 3): FS-199 version 3.0's
//! header length (A: 4 bytes; `reproduce` 3), FS-200 other versions (B: an error in both modes),
//! FS-201 a header beyond the stack (a crash; read in both modes), FS-202 a file ending inside its
//! header (memory: zeros in both modes), FS-203 reading past the header (memory: the parse stops
//! there in both modes), FS-204 a shape's empty element (a crash: skipped in both modes), FS-205
//! a shape of only 1s (a crash: one extent kept in both modes), FS-206 (`larmorx_freesurfer_mri`),
//! FS-207 a missing descr or fortran_order (B: an error; `reproduce` zeros) or shape (an error in
//! both modes), FS-208 big-endian 8-byte integers (A), FS-209 the verbose addresses (byte offsets
//! in both modes), FS-210 a fortran_order neither True nor False (B; `reproduce` True), FS-211 a
//! slice of 2 GiB or more (64-bit counts in both modes), FS-212 a short file (B; `reproduce` zeros),
//! FS-213 a header not ending at a multiple of 16 bytes (A: the data right after it; `reproduce`
//! the bytes up to the multiple skipped).

use std::io::Write;
use std::path::Path;

use larmorx_core::defects::{
    FS_199, FS_200, FS_201, FS_202, FS_203, FS_204, FS_205, FS_207, FS_208, FS_209, FS_210,
    FS_211, FS_212, FS_213,
};
use larmorx_freesurfer_core::cstd::atoi;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_mri::mri::{
    MRI_BITMAP, MRI_FLOAT, MRI_INT, MRI_LONG, MRI_SHORT, MRI_TENSOR, MRI_UCHAR, MRI_USHRT, Mri,
};

use crate::bfileio;
use crate::cfile::CFile;

/// `__magic_string`.
const MAGIC: &[u8; 6] = b"\x93NUMPY";
/// `__getarchendian()` on a little-endian machine.
const ARCH_ENDIAN: u8 = b'<';
/// Header lengths from which FreeSurfer's program, which keeps the header on its stack (8 MiB by
/// default), is taken to crash (FS-201).
const STACK_HEADER: u64 = 7 << 20;

/// A `std::vector<int>` as libstdc++ keeps it: its places (`None`: memory never written), of which
/// the first `len` are the elements.
#[derive(Clone, Debug, Default)]
pub struct ShapeVec {
    slots: Vec<Option<i32>>,
    len: usize,
}

impl ShapeVec {
    /// `push_back(v)`: a full vector moves to a buffer of twice its places (one for an empty
    /// one), whose places after the elements were never written.
    pub fn push(&mut self, v: i32) {
        if self.len == self.slots.len() {
            let cap = (2 * self.slots.len()).max(1);
            self.slots.resize(cap, None);
        }
        self.slots[self.len] = Some(v);
        self.len += 1;
    }

    /// `pop_back()`: the place keeps its value.
    pub fn pop(&mut self) {
        self.len -= 1;
    }

    /// `erase(begin())` of a vector with elements: the rest moves down one place, the last place
    /// keeps its value.
    pub fn erase_front(&mut self) {
        for k in 0..self.len - 1 {
            self.slots[k] = self.slots[k + 1];
        }
        self.len -= 1;
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The element `i` (`i < len`).
    pub fn get(&self, i: usize) -> i32 {
        self.slots[i].expect("an element of the vector")
    }

    /// The elements.
    pub fn elements(&self) -> Vec<i32> {
        (0..self.len).map(|i| self.get(i)).collect()
    }

    /// What the places after the elements hold, up to the buffer's end.
    pub fn beyond(&self) -> Vec<Option<i32>> {
        self.slots[self.len..].to_vec()
    }
}

/// The header's text as FreeSurfer holds it: `len` bytes, those the file gave and then (FS-202)
/// zeros; `tail` is set by a read of a byte the file did not give.
struct Dict {
    b: Vec<u8>,
    len: u64,
    tail: bool,
}

impl Dict {
    fn at(&mut self, p: usize) -> Option<u8> {
        if p as u64 >= self.len {
            return None;
        }
        if p >= self.b.len() {
            self.tail = true;
        }
        Some(self.b.get(p).copied().unwrap_or(0))
    }

    /// `*p = '\0'` (the bytes after the file's are zeros already).
    fn nul(&mut self, p: usize) {
        if p < self.b.len() {
            self.b[p] = 0;
        }
    }

    /// The C string at `p` (up to its NUL, which the parse always has before the end).
    fn cstr(&self, p: usize) -> &[u8] {
        let s = self.b.get(p..).unwrap_or(&[]);
        let n = s.iter().position(|&c| c == 0).unwrap_or(s.len());
        &s[..n]
    }

    /// The first position from `p` that is not a zero of the tail after the file's bytes (the
    /// tail's zeros match none of the characters the scans stop at but NUL itself).
    fn skip_tail(&self, p: usize) -> usize {
        if p >= self.b.len() {
            self.len.min(usize::MAX as u64) as usize
        } else {
            p
        }
    }
}

/// What the parse finds.
#[derive(Default)]
struct Header {
    endian: u8,
    dtype_char: u8,
    dtype_len: i32,
    has_descr: bool,
    fortran: Option<Vec<u8>>,
    has_shape: bool,
    shape: ShapeVec,
    empty_element: bool,
}

/// `__parse_header_dictionary`'s loop over the text; `None` where it reads past the end.
fn parse(d: &mut Dict, h: &mut Header) -> Option<()> {
    let mut p = 0usize;
    loop {
        let c = d.at(p)?;
        if c == b'\n' {
            return Some(());
        }
        if matches!(c, b'{' | b'}' | b',' | b' ') {
            p += 1;
            continue;
        }
        let key = p;
        loop {
            let c = d.at(p)?;
            if c == b':' || c == b'\n' {
                break;
            }
            p += 1;
            if c == 0 {
                p = p.max(d.skip_tail(p));
            }
        }
        if d.at(p)? == b':' {
            d.nul(p);
            p += 1;
            while d.at(p)? == b' ' {
                p += 1;
            }
            let k = d.cstr(key).to_vec();
            if k == b"'descr'" {
                p += 1;
                let value = p;
                h.endian = d.at(value)?;
                h.dtype_char = d.at(value + 1)?;
                h.has_descr = true;
                p += 2;
                let value = p;
                while d.at(p)?.is_ascii_digit() {
                    p += 1;
                }
                d.nul(p);
                h.dtype_len = atoi(d.cstr(value));
            } else if k == b"'fortran_order'" {
                let value = p;
                while d.at(p)? != b',' {
                    p += 1;
                    p = p.max(d.skip_tail(p));
                }
                d.nul(p);
                h.fortran = Some(d.cstr(value).to_vec());
            } else if k == b"'shape'" {
                h.has_shape = true;
                while !d.at(p)?.is_ascii_digit() {
                    p += 1;
                    p = p.max(d.skip_tail(p));
                }
                let mut value = Some(p);
                loop {
                    let c = d.at(p)?;
                    if c.is_ascii_digit() && value.is_none() {
                        value = Some(p);
                    } else if c == b',' || c == b')' {
                        let done = c == b')';
                        d.nul(p);
                        match value {
                            Some(v) => h.shape.push(atoi(d.cstr(v))),
                            None => h.empty_element = true,
                        }
                        value = None;
                        if done {
                            p += 1;
                            break;
                        }
                    }
                    p += 1;
                    p = p.max(d.skip_tail(p));
                }
            }
        }
        p += 1;
    }
}

/// glibc's `%p`.
fn ptr(x: u64) -> String {
    if x == 0 {
        "(nil)".to_owned()
    } else {
        format!("0x{x:x}")
    }
}

fn exit_with(out: &mut dyn Write, text: &str) -> Fatal {
    let _ = out.write_all(text.as_bytes());
    Fatal::Exit(1)
}

/// `fscnpy().npy2mri(npy, verbose)`: the volume of the `.npy` file `npy` (a name relative to
/// `dir`, the program's directory, when given), its messages on `out`. FreeSurfer's failures end
/// the program with status 1 (`Error reading numpy file <npy>`, `Error <npy> is not npy file`,
/// `numpy file contains <n>D data. Cannot fit them in MRI structure.`, the volume's allocation);
/// `None` is FreeSurfer's `NULL` (a C-order array of a type it cannot reorder).
pub fn npy2mri(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    npy: &str,
    verbose: bool,
) -> Result<Option<Mri>, Fatal> {
    let reproduce = g.notices.reproduce();
    let Ok(mut fp) = CFile::open(dir, npy, "rb") else {
        return Err(exit_with(out, &format!("Error reading numpy file {npy}\n")));
    };
    // the magic string (strcmp of a zero-filled 7-byte buffer: equal only for all six bytes)
    let mut magic = [0u8; 6];
    let got = fp.read_bytes(&mut magic);
    if got < 6 || &magic != MAGIC {
        return Err(exit_with(out, &format!("Error {npy} is not npy file\n")));
    }
    let mut one = [0u8; 1];
    let major = if fp.read_bytes(&mut one) == 1 {
        one[0]
    } else {
        0
    };
    let minor = if fp.read_bytes(&mut one) == 1 {
        one[0]
    } else {
        0
    };
    // the header length's size: __header_len_bytes[major - 1] of a two-entry table
    let hlb: usize = match major {
        1 => 2,
        2 => 4,
        3 => {
            let fs = if minor == 0 {
                "3 bytes (its table has no entry for version 3: the bytes after it, the version's own, the unknown rest taken as 0), so the header's text starts a byte early and the reading runs past it"
            } else {
                "3 + 256 x the minor version bytes into an 8-byte length (its table has no entry for version 3): a crash"
            };
            g.notices.add(
                &FS_199,
                format!(
                    "a .npy file of version {major}.{minor}: FreeSurfer reads its header length from {fs}; {}",
                    if reproduce && minor == 0 {
                        "3 bytes read"
                    } else {
                        "NumPy's 4 bytes read"
                    }
                ),
            );
            if reproduce && minor == 0 { 3 } else { 4 }
        }
        _ => {
            g.notices.add(
                &FS_200,
                format!(
                    "a .npy file of version {major}.{minor}: FreeSurfer takes the size of its header length from memory outside its table (a crash or a header from memory contents); refused"
                ),
            );
            return Err(exit_with(
                out,
                &format!("Error {npy}: npy format version {major}.{minor} is not supported\n"),
            ));
        }
    };
    let mut lb = [0u8; 4];
    fp.read_bytes(&mut lb[..hlb]);
    let header_len = u64::from(u32::from_le_bytes(lb));
    if header_len >= STACK_HEADER {
        g.notices.add(
            &FS_201,
            format!(
                "a .npy header of {header_len} bytes, which FreeSurfer keeps on its program's stack (8 MiB by default): a crash; read whole"
            ),
        );
    }
    // the header's text
    let mut b: Vec<u8> = Vec::new();
    let mut chunk = vec![0u8; 1 << 16];
    while (b.len() as u64) < header_len {
        let want = (header_len - b.len() as u64).min(chunk.len() as u64) as usize;
        let n = fp.read_bytes(&mut chunk[..want]);
        b.extend_from_slice(&chunk[..n]);
        if n < want {
            break;
        }
    }
    // the padding: FreeSurfer skips to the next multiple of 16 bytes
    let total = 6 + 2 + hlb as u64 + header_len;
    if total % 16 != 0 {
        let rest = 16 - total % 16;
        g.notices.add(
            &FS_213,
            format!(
                "a .npy header ending at byte {total}, not a multiple of 16: FreeSurfer skips the next {rest} bytes{}",
                if reproduce {
                    ""
                } else {
                    "; the data read right after the header, as NumPy reads it"
                }
            ),
        );
        if reproduce {
            let mut skip = vec![0u8; rest as usize];
            fp.read_bytes(&mut skip);
        }
    }
    let got = b.len();
    let mut d = Dict {
        b,
        len: header_len,
        tail: false,
    };
    let mut h = Header::default();
    let parsed = parse(&mut d, &mut h);
    if d.tail {
        g.notices.add(
            &FS_202,
            format!(
                "the file ends {got} bytes into its {header_len}-byte .npy header, and the header's parse reaches the bytes it lacks: FreeSurfer takes them from memory contents; zeros"
            ),
        );
    }
    if parsed.is_none() {
        g.notices.add(
            &FS_203,
            "the .npy header's parse runs past its end: FreeSurfer reads memory beyond it (a crash, or values from memory contents); stopped at the end",
        );
    }
    if h.empty_element {
        g.notices.add(
            &FS_204,
            "a .npy shape with an empty element (a one-dimensional array's (n,)): FreeSurfer crashes; the element skipped",
        );
    }
    if h.shape.is_empty() {
        let id = if h.has_shape { &FS_205 } else { &FS_207 };
        g.notices.add(
            id,
            "a .npy header without a shape: FreeSurfer crashes; refused",
        );
        return Err(exit_with(out, &format!("Error {npy}: npy header has no shape\n")));
    }
    for (key, have) in [("descr", h.has_descr), ("fortran_order", h.fortran.is_some())] {
        if !have {
            g.notices.add(
                &FS_207,
                format!(
                    "a .npy header without {key}: FreeSurfer takes it from memory contents; {}",
                    if reproduce { "zeros" } else { "refused" }
                ),
            );
            if !reproduce {
                return Err(exit_with(out, &format!("Error {npy}: npy header has no {key}\n")));
            }
        }
    }
    let fortran = match h.fortran.as_deref() {
        None => false,
        Some(b"False") => false,
        Some(b"True") => true,
        Some(v) => {
            let v = String::from_utf8_lossy(v).into_owned();
            g.notices.add(
                &FS_210,
                format!(
                    "a .npy fortran_order of {v:?}: FreeSurfer takes it as True; {}",
                    if reproduce { "True" } else { "refused" }
                ),
            );
            if !reproduce {
                return Err(exit_with(
                    out,
                    &format!("Error {npy}: npy header's fortran_order is {v}, not True or False\n"),
                ));
            }
            true
        }
    };
    if verbose {
        let _ = writeln!(out, "archendian : < (little endian)");
        let _ = writeln!(out, "NPY version: {major}.{minor}");
        let _ = writeln!(out, "NPY header_len: {header_len}");
        let _ = out.write_all(b"NPY dict.descr : ");
        let _ = out.write_all(&[h.endian, h.dtype_char]);
        let _ = writeln!(out, "{}", h.dtype_len);
        let _ = writeln!(
            out,
            "NPY dict.fortran_order: {}",
            if fortran { "True" } else { "False" }
        );
        let _ = out.write_all(b"NPY dict.shape: {");
        for v in h.shape.elements() {
            let _ = write!(out, " {v}, ");
        }
        let _ = out.write_all(b"}\n");
    }
    // the squeeze: trailing, then leading extents of 1 (FreeSurfer's loops never stop at one)
    let mut shape = h.shape;
    let mut squeezed = false;
    let all_ones = shape.elements().iter().all(|&v| v == 1);
    if all_ones {
        g.notices.add(
            &FS_205,
            "a .npy array whose extents are all 1: FreeSurfer crashes; one extent kept",
        );
    }
    while shape.len() > 1 && shape.get(shape.len() - 1) == 1 {
        squeezed = true;
        shape.pop();
    }
    while shape.len() > 1 && shape.get(0) == 1 {
        shape.erase_front();
        squeezed = true;
    }
    let ndims = shape.len();
    if verbose {
        let _ = write!(
            out,
            "NPY shape dimension: {ndims} ({}) ( ",
            if squeezed { "squeezed" } else { "" }
        );
        for v in shape.elements() {
            let _ = write!(out, "{v}, ");
        }
        let _ = out.write_all(b")\n");
        if ARCH_ENDIAN != h.endian {
            let _ = writeln!(out, "Data is in different endian. Need to swap bytes");
        } else {
            let _ = writeln!(out, "Data is in same endian");
        }
    }
    if ndims > 4 {
        return Err(exit_with(
            out,
            &format!("numpy file contains {ndims}D data. Cannot fit them in MRI structure.\n"),
        ));
    }
    let dtype = mri_type(out, h.dtype_char, h.dtype_len);
    let elements = shape.elements();
    let beyond = shape.beyond();
    let mut mri = Mri::new_beyond(g, out, err, &elements, &beyond, dtype, true)?;
    if verbose {
        let _ = writeln!(
            out,
            "MRI dimensions: {} x {} x {} x {}",
            mri.vg.width, mri.vg.height, mri.vg.depth, mri.nframes
        );
        let name = match mri.mtype {
            MRI_UCHAR => "UCHAR",
            MRI_SHORT => "SHORT",
            MRI_USHRT => "USHRT",
            MRI_INT => "INT",
            MRI_LONG => "LONG",
            MRI_BITMAP => "BITMAP",
            MRI_TENSOR => "TENSOR",
            MRI_FLOAT => "FLOAT",
            _ => "UNKNOWN",
        };
        let _ = writeln!(out, "          type: {name} ({})", mri.mtype);
    }
    // the voxels, slice by slice: dtype_len x width x height bytes each (an int in FreeSurfer)
    let (w, ht, dp, nf) = (mri.vg.width, mri.vg.height, mri.vg.depth, mri.nframes);
    let bytes_per_slice = i64::from(h.dtype_len) * i64::from(w) * i64::from(ht);
    let fs32 = i64::from(h.dtype_len.wrapping_mul(w).wrapping_mul(ht));
    if fs32 != bytes_per_slice && dp > 0 && nf > 0 {
        g.notices.add(
            &FS_211,
            format!(
                "a slice of {bytes_per_slice} bytes, which FreeSurfer counts in 32 bits ({fs32}): read wrongly or past the volume; counted in 64 bits"
            ),
        );
    }
    let swap = ARCH_ENDIAN != h.endian;
    if swap && h.dtype_len == 8 && bytes_per_slice > 0 && dp > 0 && nf > 0 {
        g.notices.add(
            &FS_208,
            format!(
                "a big-endian array of 8-byte values: FreeSurfer reverses them in 4-byte halves{}",
                if reproduce { "" } else { "; reversed whole" }
            ),
        );
    }
    let mut bytesread: u64 = 0;
    let slice_bytes = mri.vox_per_slice.wrapping_mul(mri.bytes_per_vox as u64);
    let mut buf = vec![0u8; bytes_per_slice.max(0) as usize];
    for f in 0..nf.max(0) {
        for s in 0..dp.max(0) {
            let n = fp.read_bytes(&mut buf);
            bytesread += n as u64;
            if swap {
                let count = bytes_per_slice;
                match h.dtype_len {
                    2 => {
                        bfileio::byteswapbufshort(g, &mut buf, count);
                    }
                    4 => {
                        bfileio::byteswapbuffloat(g, &mut buf, count);
                    }
                    8 if reproduce => {
                        bfileio::byteswapbuffloat(g, &mut buf, count);
                    }
                    8 => {
                        bfileio::byteswapbufdouble(g, &mut buf, count);
                    }
                    _ => {}
                }
            }
            let slice = i64::from(s) + i64::from(f) * i64::from(dp);
            let at = (slice as u64).wrapping_mul(slice_bytes) as usize;
            if let Some(chunk) = mri.chunk.as_mut() {
                let end = at.saturating_add(buf.len()).min(chunk.len());
                if at < end {
                    chunk[at..end].copy_from_slice(&buf[..end - at]);
                }
            }
            // FreeSurfer reads into the slice: bytes it did not get stay as they were (zeros)
            buf.iter_mut().for_each(|x| *x = 0);
        }
    }
    fp.close();
    if verbose {
        let _ = writeln!(out, "{bytesread} bytes read");
    }
    let expected = (bytes_per_slice.max(0) as u64)
        .wrapping_mul(dp.max(0) as u64)
        .wrapping_mul(nf.max(0) as u64);
    if bytesread < expected {
        g.notices.add(
            &FS_212,
            format!(
                "a .npy file with {bytesread} of its array's {expected} bytes: FreeSurfer leaves the rest 0 without a message{}",
                if reproduce { "" } else { "; refused" }
            ),
        );
        if !reproduce {
            return Err(exit_with(
                out,
                &format!(
                    "Error {npy}: {bytesread} bytes of data read, {expected} expected\n"
                ),
            ));
        }
    }
    if !fortran {
        return contiguous_reorder(g, out, err, mri, &elements, &beyond, dtype, verbose);
    }
    Ok(Some(mri))
}

/// `__getmritype()`: FreeSurfer's type of a NumPy type, or -1 after its warning.
fn mri_type(out: &mut dyn Write, dtype_char: u8, dtype_len: i32) -> i32 {
    let warn_len = |out: &mut dyn Write| {
        let _ = out.write_all(b"WARN: !!! unsupported __dtype_char=");
        let _ = out.write_all(&[dtype_char]);
        let _ = writeln!(out, ", __dtype_len={dtype_len}");
    };
    match dtype_char {
        b'f' if dtype_len == 4 => MRI_FLOAT,
        b'i' if dtype_len == 2 => MRI_SHORT,
        b'i' if dtype_len == 4 => MRI_INT,
        b'i' if dtype_len == 8 => MRI_LONG,
        b'u' if dtype_len == 1 => MRI_UCHAR,
        b'u' if dtype_len == 2 => MRI_USHRT,
        b'f' | b'i' | b'u' => {
            warn_len(out);
            -1
        }
        _ => {
            let _ = out.write_all(b"WARN: !!! unsupported __dtype_char=");
            let _ = out.write_all(&[dtype_char]);
            let _ = out.write_all(b"!!!\n");
            -1
        }
    }
}

/// `__contiguousreorder(mri)`: a C-order array's voxels placed in FreeSurfer's (Fortran) order, in
/// a new volume of the same shape; `None` (FreeSurfer's `NULL`) for a type it does not reorder.
#[allow(clippy::too_many_arguments)]
fn contiguous_reorder(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mri: Mri,
    shape: &[i32],
    beyond: &[Option<i32>],
    dtype: i32,
    verbose: bool,
) -> Result<Option<Mri>, Fatal> {
    if ![MRI_UCHAR, MRI_SHORT, MRI_USHRT, MRI_INT, MRI_LONG, MRI_FLOAT].contains(&dtype) {
        let _ = writeln!(out, "Unsupported datatype, no array contiguous reordering");
        return Ok(None);
    }
    let _ = writeln!(out, "array contiguous reordering ...");
    let mut new = Mri::new_beyond(g, out, err, shape, beyond, dtype, true)?;
    let (width, height, depth, nframes) = (
        mri.vg.width.max(0) as u64,
        mri.vg.height.max(0) as u64,
        mri.vg.depth.max(0) as u64,
        mri.nframes.max(0) as u64,
    );
    let bpv = mri.bytes_per_vox as u64;
    let vox_per_row = nframes;
    let vox_per_slice = nframes * depth;
    let vox_per_vol = nframes * depth * height;
    if verbose && width * height * depth * nframes > 0 {
        g.notices.add(
            &FS_209,
            "verbose reordering: FreeSurfer prints the voxels' memory addresses (they differ from run to run); printed as byte offsets in the two volumes",
        );
    }
    let src = mri.chunk.as_deref().unwrap_or(&[]);
    let dst = new.chunk.get_or_insert_with(Vec::new);
    let mut at = 0u64;
    for c in 0..nframes {
        for r in 0..depth {
            for s in 0..height {
                for f in 0..width {
                    let voxel = c + r * vox_per_row + s * vox_per_slice + f * vox_per_vol;
                    let from = bpv * voxel;
                    if verbose {
                        let _ = writeln!(out, "memcpy({}, {}, {bpv})", ptr(from), ptr(at));
                        let _ = writeln!(
                            out,
                            "      offset_voxel = {voxel}, offset_bytes = {from}"
                        );
                    }
                    let (a, n) = (at as usize, bpv as usize);
                    dst[a..a + n].copy_from_slice(&src[from as usize..from as usize + n]);
                    at += bpv;
                }
            }
        }
    }
    Ok(Some(new))
}

