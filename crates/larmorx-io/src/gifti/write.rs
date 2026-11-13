// SPDX-License-Identifier: Apache-2.0
//! gifticlib 1.12's writer (`gifti_xml.c`: `gxml_write_image`, `gxml_write_gifti` and the
//! `ewrite_*` functions; `gifti_io.c`: `gifti_write_image`, `gifti_write_extern_DA_data`), byte
//! for byte: C's `%f`, `%g`, `%d`, `%u`, `%lld`, `%llu` and `%Lf` (x86-64's 80-bit `long double`,
//! [`larmorx_core::cfmt::f80_f`]), the Base64 text, the attribute layout and its indentation.
//!
//! GZipBase64Binary data are compressed by zlib-rs at gifticlib's level (through flate2): the
//! decompressed values are zlib's, the compressed bytes may differ from zlib 1.2.11's
//! (`docs/findings/gifti.md`).

use std::io::Write as _;
use std::path::Path;

use flate2::Compression;
use flate2::write::ZlibEncoder;
use larmorx_core::cfmt::{self, Streams};
use larmorx_core::defects::{self as d, Notices};
use larmorx_core::x87::F80;

use super::*;

/// `b64_encode_table`.
const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// `gxml_disp_b64_data` of `data` (empty for no data, as gifticlib writes nothing).
pub fn base64(data: &[u8]) -> Vec<u8> {
    let mut o = Vec::with_capacity(data.len().div_ceil(3) * 4);
    #[allow(clippy::chunks_exact_to_as_chunks)]
    let mut ch = data.chunks_exact(3);
    for c in &mut ch {
        o.extend_from_slice(&[
            B64[(c[0] >> 2) as usize],
            B64[(((c[0] & 3) << 4) | (c[1] >> 4)) as usize],
            B64[(((c[1] & 0xF) << 2) | (c[2] >> 6)) as usize],
            B64[(c[2] & 0x3F) as usize],
        ]);
    }
    match ch.remainder() {
        [a] => o.extend_from_slice(&[
            B64[(a >> 2) as usize],
            B64[((a & 3) << 4) as usize],
            b'=',
            b'=',
        ]),
        [a, b] => o.extend_from_slice(&[
            B64[(a >> 2) as usize],
            B64[(((a & 3) << 4) | (b >> 4)) as usize],
            B64[((b & 0xF) << 2) as usize],
            b'=',
        ]),
        _ => {}
    }
    o
}

struct W<'a> {
    lib: &'a Gifti,
    out: &'a mut dyn Streams,
    notices: &'a mut Notices,
    fix: bool,
    verb: i32,
    dstore: bool,
    depth: i32,
    f: Vec<u8>,
    /// `GXD.zlen`: the last compression buffer's size.
    zlen: i64,
}

fn pad(n: i32) -> String {
    " ".repeat(n.max(0) as usize)
}

impl W<'_> {
    fn put(&mut self, s: &str) {
        self.f.extend_from_slice(s.as_bytes());
    }
    fn putb(&mut self, b: &[u8]) {
        self.f.extend_from_slice(b);
    }

    /// `ewrite_str_attr` (and the int and long long forms, with `value` already printed).
    fn attr(&mut self, name: &[u8], value: &[u8], spaces: i32, first: bool) {
        if first {
            self.put(" ");
        } else {
            self.put(&format!("\n{}", pad(spaces)));
        }
        self.putb(name);
        self.put("=\"");
        self.putb(value);
        self.put("\"");
    }

    fn write_gifti(&mut self, gim: &GiftiImage) {
        if self.verb > 1 {
            let mb = self.lib.gim_da_size(gim, true, self.out);
            self.out.err(&format!(
                "++ gifti image, numDA = {}, size = {mb} MB\n",
                gim.num_da
            ));
        }
        self.put(&format!(
            "<?xml version=\"{GIFTI_XML_VERSION}\" encoding=\"{GIFTI_XML_ENCODING}\"?>\n<!DOCTYPE GIFTI SYSTEM \"{GIFTI_XML_DTD_SOURCE}\">\n"
        ));
        self.put("<GIFTI");
        let mut first = true;
        if let Some(v) = &gim.version {
            self.put(" Version=\"");
            self.putb(v);
            self.put("\"");
            first = false;
        }
        self.put(&format!(
            "{}NumberOfDataArrays=\"{}\"",
            if first { " " } else { "  " },
            gim.num_da
        ));
        self.ex_atrs(&gim.ex_atrs, "GIFTI".len() as i32 + 2);
        self.put(">\n");
        self.depth += 1;
        self.meta(&gim.meta);
        self.label_table(&gim.labeltable);
        if gim.darray.is_empty() {
            if self.verb > 0 {
                self.out.err("** gifti_image, missing darray\n");
            }
        } else {
            for c in 0..gim.num_da.max(0) as usize {
                if let Some(Some(da)) = gim.darray.get(c) {
                    self.darray(da);
                }
            }
        }
        self.depth -= 1;
        self.put("</GIFTI>\n");
    }

    fn ex_atrs(&mut self, nvp: &NvPairs, offset: i32) {
        let spaces = self.lib.indent * self.depth + offset;
        if self.verb > 2 {
            self.out.err(&format!("++ write {} ex_atr's\n", nvp.len()));
        }
        for c in 0..nvp.len() {
            let n = nvp.name[c].clone().unwrap_or_default();
            let v = nvp.value[c].clone().unwrap_or_default();
            self.attr(&n, &v, spaces, false);
        }
    }

    /// `ewrite_text_ele`.
    fn text_ele(&mut self, ele: &str, cdata: &CStr, attr: &str, spaces: i32, in_cdata: bool) {
        self.put(&format!("{}<{ele}{attr}>", pad(spaces)));
        if in_cdata {
            self.put("<![CDATA[");
        }
        if let Some(c) = cdata {
            self.putb(c);
        }
        if in_cdata {
            self.put("]]>");
        }
        self.put(&format!("</{ele}>\n"));
    }

    fn label_table(&mut self, lt: &LabelTable) {
        let spaces = self.lib.indent * self.depth;
        if self.verb > 3 {
            self.out.err("++ write giiLabelTable\n");
        }
        if lt.is_empty() {
            self.put(&format!("{}<LabelTable/>\n", pad(spaces)));
            return;
        }
        self.put(&format!("{}<LabelTable>\n", pad(spaces)));
        // gifticlib steps its colour pointer only past written labels.
        let mut k = 0;
        for c in 0..lt.len() {
            if lt.label[c].is_none() {
                if self.verb > 1 {
                    self.out.err(&format!("** label[{c}] unset\n"));
                }
                continue;
            }
            let attr = if lt.rgba.is_some() {
                let v = rgba_of(lt, k, self.notices);
                k += 1;
                format!(
                    " Key=\"{}\" Red=\"{}\" Green=\"{}\" Blue=\"{}\" Alpha=\"{}\"",
                    lt.key[c],
                    cfmt::g(v[0] as f64, 6),
                    cfmt::g(v[1] as f64, 6),
                    cfmt::g(v[2] as f64, 6),
                    cfmt::g(v[3] as f64, 6)
                )
            } else {
                format!(" Key=\"{}\"", lt.key[c])
            };
            let ind = self.lib.indent;
            self.text_ele("Label", &lt.label[c], &attr, spaces + ind, true);
        }
        self.put(&format!("{}</LabelTable>\n", pad(spaces)));
    }

    fn meta(&mut self, md: &NvPairs) {
        let mut spaces = self.lib.indent * self.depth;
        if self.verb > 3 {
            self.out.err("++ write giiMetaData\n");
        }
        if md.is_empty() {
            self.put(&format!("{}<MetaData/>\n", pad(spaces)));
            return;
        }
        if self.verb > 3 {
            self.out.err(&format!("   MD length = {}\n", md.len()));
        }
        self.put(&format!("{}<MetaData>\n", pad(spaces)));
        let ind = self.lib.indent;
        for c in 0..md.len() {
            if md.name[c].is_none() {
                if self.verb > 1 {
                    self.out.err(&format!("** MD[{c}] unset\n"));
                }
                continue;
            }
            spaces += ind;
            self.put(&format!("{}<MD>\n", pad(spaces)));
            spaces += ind;
            self.text_ele("Name", &md.name[c], "", spaces, true);
            self.text_ele("Value", &md.value[c], "", spaces, true);
            spaces -= ind;
            self.put(&format!("{}</MD>\n", pad(spaces)));
            spaces -= ind;
        }
        self.put(&format!("{}</MetaData>\n", pad(spaces)));
    }

    fn darray(&mut self, da: &DataArray) {
        let spaces = self.lib.indent * self.depth;
        if self.verb > 3 {
            self.out.err("++ write giiDataArray\n");
        }
        let offset = "DataArray".len() as i32 + 2 + spaces;
        self.put(&format!("{}<DataArray", pad(spaces)));
        self.attr(
            b"Intent",
            intent_to_string(da.intent).as_bytes(),
            offset,
            true,
        );
        self.attr(
            b"DataType",
            datatype2str(da.datatype).as_bytes(),
            offset,
            false,
        );
        let io = list_index2string(List::IndexOrder, da.ind_ord, self.lib.verb, self.out);
        self.attr(b"ArrayIndexingOrder", io.as_bytes(), offset, false);
        self.attr(
            b"Dimensionality",
            da.num_dim.to_string().as_bytes(),
            offset,
            false,
        );
        for c in 0..da.num_dim.max(0) as usize {
            // C reads past `dims[6]` into the next fields (`encoding`, `endian`).
            let v = match c {
                0..=5 => da.dims[c],
                6 => da.encoding,
                7 => da.endian,
                _ => {
                    self.notices.add(
                        &d::FS_85,
                        "a dimension past Dim7 written as 0 (gifticlib reads a pointer's bytes)",
                    );
                    0
                }
            };
            self.attr(
                format!("Dim{}", (b'0' + c as u8) as char).as_bytes(),
                v.to_string().as_bytes(),
                offset,
                false,
            );
        }
        let en = list_index2string(List::Encoding, da.encoding, self.lib.verb, self.out);
        self.attr(b"Encoding", en.as_bytes(), offset, false);
        let ed = list_index2string(List::Endian, this_endian(), self.lib.verb, self.out);
        self.attr(b"Endian", ed.as_bytes(), offset, false);
        let ext = da.ext_fname.clone().unwrap_or_default();
        self.attr(b"ExternalFileName", &ext, offset, false);
        if !ext.is_empty() {
            self.attr(
                b"ExternalFileOffset",
                da.ext_offset.to_string().as_bytes(),
                offset,
                false,
            );
        } else {
            self.attr(b"ExternalFileOffset", b"", offset, false);
        }
        self.put(">\n");
        self.depth += 1;
        self.meta(&da.meta);
        for cs in da.coordsys.iter().flatten() {
            self.coordsys(cs);
        }
        self.data(da);
        self.depth -= 1;
        self.put(&format!("{}</DataArray>\n", pad(spaces)));
    }

    fn coordsys(&mut self, cs: &CoordSystem) {
        let mut spaces = self.lib.indent * self.depth;
        if self.verb > 3 {
            self.out.err("++ write giiCoordSystem\n");
        }
        self.put(&format!(
            "{}<CoordinateSystemTransformMatrix>\n",
            pad(spaces)
        ));
        spaces += self.lib.indent;
        self.text_ele("DataSpace", &cs.dataspace, "", spaces, true);
        self.text_ele("TransformedSpace", &cs.xformspace, "", spaces, true);
        self.put(&format!("{}<MatrixData>\n", pad(spaces)));
        for r in 0..4 {
            let mut s = pad(spaces + self.lib.indent);
            for c in 0..4 {
                s += &cfmt::f(cs.xform[r][c], 6);
                s.push(' ');
            }
            s.push('\n');
            self.put(&s);
        }
        self.put(&format!("{}</MatrixData>\n", pad(spaces)));
        spaces -= self.lib.indent;
        self.put(&format!(
            "{}</CoordinateSystemTransformMatrix>\n",
            pad(spaces)
        ));
    }

    fn data(&mut self, da: &DataArray) -> i32 {
        let spaces = self.lib.indent * self.depth;
        if self.verb > 3 {
            let e = list_index2string(List::Encoding, da.encoding, self.lib.verb, self.out);
            self.out.err(&format!("++ write {e} Data\n"));
        }
        let Some(data) = da.data.as_deref().filter(|_| da.nvals > 0 && da.nbyper > 0) else {
            self.put(&format!("{}<Data/>\n", pad(spaces)));
            return 0;
        };
        let mut errs = 0;
        if da.encoding == GIFTI_ENCODING_EXTBIN {
            self.put(&format!("{}<Data/>\n", pad(spaces)));
        } else {
            self.put(&format!("{}<Data>", pad(spaces)));
        }
        let nbytes = (da.nvals.wrapping_mul(da.nbyper as i64).max(0) as usize).min(data.len());
        if self.dstore {
            match da.encoding {
                GIFTI_ENCODING_ASCII => {
                    self.put("\n");
                    let (rows, cols, _) = self.lib.da_rows_cols(da, self.out);
                    for r in 0..rows.max(0) {
                        self.data_line(data, da.datatype, r, cols, spaces + self.lib.indent);
                    }
                    self.put(&pad(spaces));
                }
                GIFTI_ENCODING_B64BIN => {
                    let b = base64(&data[..nbytes]);
                    self.putb(&b);
                }
                GIFTI_ENCODING_B64GZ => {
                    let level = if self.lib.zlevel < 0 {
                        6
                    } else {
                        self.lib.zlevel as u32
                    };
                    // update_partial_buffer(&zdata, &zlen, blen, 1): compress2's buffer
                    let blen = (nbytes as f64 * 1.01 + 12.0) as u64 as i64;
                    if self.lib.xml_verb > 2 {
                        self.out.err(&format!(
                            "++ UPB, alloc {} bytes (from {blen}, 1) for buff\n",
                            blen as i32
                        ));
                    }
                    self.zlen = blen as i32 as i64;
                    let mut e = ZlibEncoder::new(Vec::new(), Compression::new(level));
                    let _ = e.write_all(&data[..nbytes]);
                    let z = e.finish().unwrap_or_default();
                    if self.verb > 2 {
                        self.out.err(&format!(
                            "-- compress buffer ({}% of {nbytes} bytes)...\n",
                            cfmt::f(100.0 * z.len() as f64 / nbytes as f64, 2)
                        ));
                        self.out.err("-- compression succeeded\n");
                    }
                    let b = base64(&z);
                    self.putb(&b);
                }
                GIFTI_ENCODING_EXTBIN => {
                    if self.lib.write_extern_da_data(da, self.fix, self.out) != 0 {
                        errs = 1;
                    }
                }
                e => {
                    self.out.err(&format!("** unknown data encoding, {e}\n"));
                    errs = 1;
                }
            }
        }
        if da.encoding != GIFTI_ENCODING_EXTBIN {
            self.put("</Data>\n");
        }
        errs
    }

    /// `ewrite_data_line`: row `row` of `cols` values.
    fn data_line(&mut self, data: &[u8], t: i32, row: i64, cols: i64, space: i32) {
        if row < 0 || cols <= 0 {
            return;
        }
        let mut s = pad(space);
        let get = |off: usize, n: usize| -> Option<&[u8]> { data.get(off..off + n) };
        let r = row as usize;
        let k = cols as usize;
        let mut missing = false;
        macro_rules! ints {
            ($n:expr, $conv:expr) => {
                for c in 0..k {
                    match get((r * k + c) * $n, $n) {
                        Some(b) => s += &format!("{} ", $conv(b)),
                        None => missing = true,
                    }
                }
            };
        }
        match t {
            NIFTI_TYPE_UINT8 => ints!(1, |b: &[u8]| b[0]),
            NIFTI_TYPE_INT8 => ints!(1, |b: &[u8]| b[0] as i8),
            NIFTI_TYPE_INT16 => ints!(2, |b: &[u8]| i16::from_le_bytes([b[0], b[1]])),
            NIFTI_TYPE_UINT16 => ints!(2, |b: &[u8]| u16::from_le_bytes([b[0], b[1]])),
            NIFTI_TYPE_INT32 => ints!(4, |b: &[u8]| i32::from_le_bytes(b.try_into().expect("4"))),
            NIFTI_TYPE_UINT32 => ints!(4, |b: &[u8]| u32::from_le_bytes(b.try_into().expect("4"))),
            NIFTI_TYPE_INT64 => ints!(8, |b: &[u8]| i64::from_le_bytes(b.try_into().expect("8"))),
            NIFTI_TYPE_UINT64 => ints!(8, |b: &[u8]| u64::from_le_bytes(b.try_into().expect("8"))),
            NIFTI_TYPE_FLOAT32 => ints!(4, |b: &[u8]| cfmt::f(
                f32::from_le_bytes(b.try_into().expect("4")) as f64,
                6
            )),
            NIFTI_TYPE_FLOAT64 => ints!(8, |b: &[u8]| cfmt::f(
                f64::from_le_bytes(b.try_into().expect("8")),
                6
            )),
            NIFTI_TYPE_COMPLEX64 | NIFTI_TYPE_COMPLEX128 => {
                let n = if t == NIFTI_TYPE_COMPLEX64 { 4 } else { 8 };
                for c in 0..k {
                    let off = (r * 2 * k + 2 * c) * n;
                    let v = |o: usize| -> Option<f64> {
                        let b = get(o, n)?;
                        Some(if n == 4 {
                            f32::from_le_bytes(b.try_into().expect("4")) as f64
                        } else {
                            f64::from_le_bytes(b.try_into().expect("8"))
                        })
                    };
                    match (v(off), v(off + n)) {
                        (Some(a), Some(b)) => {
                            s += &format!("{} {}   ", cfmt::f(a, 6), cfmt::f(b, 6))
                        }
                        _ => missing = true,
                    }
                }
            }
            NIFTI_TYPE_RGB24 => {
                for c in 0..k {
                    match get(r * 3 * k + 3 * c, 3) {
                        Some(b) => s += &format!("{} {} {}   ", b[0], b[1], b[2]),
                        None => missing = true,
                    }
                }
            }
            NIFTI_TYPE_FLOAT128 | NIFTI_TYPE_COMPLEX256 => {
                let per = if t == NIFTI_TYPE_FLOAT128 { 1 } else { 2 };
                for c in 0..k {
                    let mut parts = Vec::new();
                    for p in 0..per {
                        let off = ((r * k + c) * per + p) * 16;
                        let mut b = [0u8; 10];
                        let have = data.len().saturating_sub(off).min(10);
                        if have < 10 {
                            missing = true;
                        }
                        if have > 0 {
                            b[..have].copy_from_slice(&data[off..off + have]);
                        }
                        parts.push(cfmt::f80_f(F80::from_le_bytes(b), 6));
                    }
                    s += &if per == 1 {
                        format!("{} ", parts[0])
                    } else {
                        format!("{} {}   ", parts[0], parts[1])
                    };
                }
            }
            _ => {
                self.put(&s);
                self.out
                    .err(&format!("** write_data_line, unknown type {t}\n"));
                return;
            }
        }
        if missing {
            let id = if t == NIFTI_TYPE_FLOAT128 {
                &d::FS_82
            } else {
                &d::FS_85
            };
            self.notices.add(id, "values past the end of the array's data written from zeros (gifticlib reads past it)");
        }
        s.push('\n');
        self.put(&s);
    }
}

impl Gifti {
    /// `gifti_write_image`: 0 on success, 1 after gifticlib's message. With `update_ok` (the
    /// default) the image's metadata gains `gifticlib-version`, as gifticlib's does.
    pub fn write_image(
        &mut self,
        gim: &mut GiftiImage,
        fname: &Path,
        write_data: bool,
        out: &mut dyn Streams,
        notices: &mut Notices,
    ) -> i32 {
        let v = self.verb;
        self.xml_set_verb(v);
        let verb = self.xml_verb;
        let name = fname.to_string_lossy().into_owned();
        if verb > 1 {
            // three writes, as gifticlib's: the size's own messages come between them
            out.err(&format!(
                "++ writing gifti image ({} data) to '{name}'",
                if write_data { "with" } else { "no" }
            ));
            if write_data {
                let mb = self.gim_da_size(gim, true, out);
                out.err(&format!(" ({} DA elements = {mb} MB)", gim.num_da));
            }
            out.err("\n");
        }
        if verb > 2 {
            let mut s = format!(
                "-- user opts: gxml_data :\n   verb        : {}\n   dstore      : {}\n   indent      : {}\n   buf_size    : {}\n   b64_check   : {}\n   zlevel      : {}\n   da_len      : 0\n",
                self.xml_verb, self.dstore, self.indent, self.buf_size, self.b64_check, self.zlevel
            );
            if verb > 3 {
                s += "   da_list     : (nil)\n";
                s += &self.gxd.lines(0);
            }
            out.err(&s);
        }
        self.gxd = GxdState {
            gim: true,
            ..GxdState::default()
        };
        self.dstore = write_data as i32;
        if self.update_ok != 0 {
            if verb > 2 {
                out.err(&format!(
                    "++ setting GIFTI MD: gifticlib-version to {GIFTI_VERSION}\n"
                ));
            }
            let mut md = std::mem::take(&mut gim.meta);
            self.add_to_meta(
                &mut md,
                b"gifticlib-version",
                GIFTI_VERSION.as_bytes(),
                true,
                out,
                notices,
            );
            gim.meta = md;
        }
        let Ok(mut file) = std::fs::File::create(fname) else {
            out.err(&format!("** failed to open '{name}' for gifti write\n"));
            return 1;
        };
        let fix = notices.fix();
        let mut w = W {
            lib: self,
            out,
            notices,
            fix,
            verb,
            dstore: write_data,
            depth: 0,
            f: Vec::new(),
            zlen: 0,
        };
        w.write_gifti(gim);
        let _ = file.write_all(&w.f);
        let zlen = w.zlen;
        self.gxd.zlen = zlen;
        0
    }

    /// The bytes [`Gifti::write_image`] writes, without a file (and without touching `gim`'s
    /// metadata: the caller adds `gifticlib-version` if it wants it).
    pub fn write_to_bytes(
        &self,
        gim: &GiftiImage,
        write_data: bool,
        out: &mut dyn Streams,
        notices: &mut Notices,
    ) -> Vec<u8> {
        let fix = notices.fix();
        let mut w = W {
            lib: self,
            out,
            notices,
            fix,
            verb: self.xml_verb,
            dstore: write_data,
            depth: 0,
            f: Vec::new(),
            zlen: 0,
        };
        w.write_gifti(gim);
        w.f
    }

    /// `gifti_write_extern_DA_data`: the data appended to the external file, which must end at
    /// `ext_offset`.
    pub fn write_extern_da_data(&self, da: &DataArray, fix: bool, out: &mut dyn Streams) -> i32 {
        let Some(name) = da.ext_fname.clone().filter(|n| !n.is_empty()) else {
            return 0;
        };
        let n = String::from_utf8_lossy(&name).into_owned();
        if self.verb > 4 {
            out.err(&format!("-- external write to '{n}'\n"));
        }
        if da.ext_offset < 0 {
            out.err(&format!(
                "** bad offset for external DA data write, {}\n",
                da.ext_offset
            ));
            return 1;
        }
        let Some(data) = &da.data else {
            out.err("** no data for external DA data write\n");
            return 1;
        };
        if !self.valid_dims(da, true, fix, out) {
            out.err("** cannot write external DA data with bad dims...\n");
            return 1;
        }
        let nbytes = da.nvals.wrapping_mul(da.nbyper as i64);
        let Ok(mut f) = std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .read(true)
            .open(&n)
        else {
            out.err(&format!("** ext write: failed to open '{n}' for append\n"));
            return 1;
        };
        let posn = f.metadata().map(|m| m.len() as i64).unwrap_or(-1);
        if posn != da.ext_offset {
            out.err(&format!(
                "** ext write: cur posn ({posn}) not ext_offset ({}) in file {n}\n",
                da.ext_offset
            ));
            return 1;
        }
        let k = (nbytes.max(0) as usize).min(data.len());
        let ok = f.write_all(&data[..k]).is_ok();
        let written = if ok { k as i64 } else { 0 };
        if written != nbytes {
            out.err(&format!(
                "** ext_write: appended only {written} of {nbytes} bytes to {n}\n"
            ));
            return 1;
        }
        if self.verb > 2 {
            out.err(&format!(
                "-- appended {nbytes} bytes to external '{n}' @ {}\n",
                da.ext_offset
            ));
        }
        0
    }
}
