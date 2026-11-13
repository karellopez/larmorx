// SPDX-License-Identifier: Apache-2.0
//! gifticlib 1.12's reader (`gifti_xml.c`: `gxml_read_image`, the element stack and callbacks,
//! the ASCII, Base64Binary and GZipBase64Binary decoders, external data), on the events of
//! [`crate::xml`]. The file is fed to the parser in the pieces gifticlib's loop reads (its buffer
//! size changes with the arrays it meets), so every piece of character data, and every message
//! that shows one, is gifticlib's.

use std::io::Read;
use std::path::Path;

use flate2::{Decompress, FlushDecompress, Status};
use larmorx_core::cfmt;
use larmorx_core::cfmt::Streams;
use larmorx_core::cscan::{is_c_space, strtod, strtol10};
use larmorx_core::defects::{self as d, Notices};

use super::*;
use crate::xml::{self, Content, Handler};

const ENAMES: [&str; 15] = [
    "Invalid",
    "GIFTI",
    "MetaData",
    "MD",
    "Name",
    "Value",
    "LabelTable",
    "Label",
    "DataArray",
    "CoordinateSystemTransformMatrix",
    "Data",
    "DataSpace",
    "TransformedSpace",
    "MatrixData",
    "CDATA",
];
const E_INVALID: usize = 0;
const E_GIFTI: usize = 1;
const E_META: usize = 2;
const E_MD: usize = 3;
const E_NAME: usize = 4;
const E_VALUE: usize = 5;
const E_LABELTABLE: usize = 6;
const E_LABEL: usize = 7;
const E_DATAARRAY: usize = 8;
const E_CSTM: usize = 9;
const E_DATA: usize = 10;
const E_DATASPACE: usize = 11;
const E_XFORMSPACE: usize = 12;
const E_MATRIXDATA: usize = 13;
const E_CDATA: usize = 14;
const E_LAST: usize = 14;
const MAX_DEPTH: i32 = 10;

/// `b64_decode_table`: 0..63 for the alphabet, 0 for '=', 128 for every other byte.
pub(crate) fn b64_decode(c: u8) -> u8 {
    match c {
        b'A'..=b'Z' => c - b'A',
        b'a'..=b'z' => c - b'a' + 26,
        b'0'..=b'9' => c - b'0' + 52,
        b'+' => 62,
        b'/' => 63,
        b'=' => 0,
        _ => 128,
    }
}

/// Where the current character data goes (`xd->cdata`).
#[derive(Clone, Copy, Debug, PartialEq)]
enum Target {
    None,
    MdName(Meta, usize),
    MdValue(Meta, usize),
    Label(usize),
    DataSpace(usize, usize),
    XformSpace(usize, usize),
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Meta {
    Image,
    Da(usize),
}

/// `gxml_data`'s per-read part and the borrowed library state.
struct Reader<'a> {
    lib: &'a mut Gifti,
    out: &'a mut dyn Streams,
    notices: &'a mut Notices,
    dialect: &'a dyn Dialect,
    fix: bool,
    verb: i32,
    gim: GiftiImage,
    da_list: Option<Vec<i32>>,
    da_ind: usize,
    ele_da: i32,
    exp_da: i32,
    b64_errors: i32,
    /// `xd->errors`: the element stack's depth errors.
    errors: i32,
    skip: i32,
    depth: i32,
    stack: [usize; 11],
    dind: i64,
    clen: usize,
    cdata: Target,
    /// The depth and target of a text element whose child just ended (FS-86).
    cdata_parent: Option<(i32, Target)>,
    dlen: usize,
    doff: usize,
    ddata: Vec<u8>,
    /// Bytes of `ddata` ever written in this read (FS-81's memory never written).
    ddata_init: usize,
    xdata: Vec<u8>,
    zdata: Vec<u8>,
}

fn lossy(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

/// `whitespace_len`.
fn whitespace_len(s: &[u8]) -> usize {
    if s.is_empty() || s[0] == 0 {
        return 0;
    }
    s.iter().position(|&c| !is_c_space(c)).unwrap_or(s.len())
}

/// `partial_buf_size`.
fn partial_buf_size(nbytes: i64) -> i32 {
    let ib = nbytes as i32;
    if ib <= GXML_MIN_BSIZE {
        GXML_MIN_BSIZE
    } else if ib <= 64 * 1024 {
        ib
    } else if ib <= 10 * 1024 * 1024 {
        (ib / 10 + 0xfff) & !0xfff
    } else {
        1024 * 1024
    }
}

/// `show_depth`.
fn depth_prefix(depth: i32, show: bool) -> String {
    let pad = " ".repeat((3 * depth).max(0) as usize);
    if show {
        format!("{pad} {depth:02} ")
    } else {
        format!("{pad}    ")
    }
}

/// `%.*s` with C's precision (a byte count; a 0 byte ends the string first), as text.
fn prec(s: &[u8], n: usize) -> String {
    let s = &s[..n.min(s.len())];
    lossy(&s[..s.iter().position(|&c| c == 0).unwrap_or(s.len())])
}

/// zlib's `uncompress` into a buffer of `out.len()` bytes: (status, bytes written). As zlib
/// 1.2.11's `uncompress2`: `inflate` without flushing until the stream ends, the output is full
/// (`Z_BUF_ERROR`, -5) or the input is used up (`Z_DATA_ERROR`, -3).
fn uncompress(out: &mut [u8], src: &[u8]) -> (i32, usize) {
    let mut z = Decompress::new(true);
    loop {
        let (i0, o0) = (z.total_in() as usize, z.total_out() as usize);
        let r = z.decompress(&src[i0..], &mut out[o0..], FlushDecompress::None);
        let (i1, n) = (z.total_in() as usize, z.total_out() as usize);
        match r {
            Ok(Status::StreamEnd) => return (0, n),
            Err(_) => return (-3, n),
            Ok(_) if n == out.len() => {
                // zlib still reads a trailer that needs no output space
                let i2 = z.total_in() as usize;
                let r = z.decompress(&src[i2..], &mut [], FlushDecompress::None);
                return (
                    if matches!(r, Ok(Status::StreamEnd)) {
                        0
                    } else {
                        -5
                    },
                    n,
                );
            }
            Ok(_) if i1 == i0 && n == o0 => return (-3, n),
            Ok(_) => {}
        }
    }
}

impl Gifti {
    /// `gifti_read_image`: the image, or `None` after gifticlib's messages.
    pub fn read_image(
        &mut self,
        fname: &Path,
        read_data: bool,
        dialect: &dyn Dialect,
        out: &mut dyn Streams,
        notices: &mut Notices,
    ) -> Option<GiftiImage> {
        let v = self.verb;
        self.xml_set_verb(v);
        self.gxml_read_image(fname, read_data, None, dialect, out, notices)
    }

    /// `gifti_read_da_list`: only the arrays of `dalist` (in its order, repeats copied).
    pub fn read_da_list(
        &mut self,
        fname: &Path,
        read_data: bool,
        dalist: &[i32],
        dialect: &dyn Dialect,
        out: &mut dyn Streams,
        notices: &mut Notices,
    ) -> Option<GiftiImage> {
        let v = self.verb;
        self.xml_set_verb(v);
        let list = (!dalist.is_empty()).then_some(dalist);
        self.gxml_read_image(fname, read_data, list, dialect, out, notices)
    }

    fn gxml_read_image(
        &mut self,
        fname: &Path,
        read_data: bool,
        dalist: Option<&[i32]>,
        dialect: &dyn Dialect,
        out: &mut dyn Streams,
        notices: &mut Notices,
    ) -> Option<GiftiImage> {
        let fix = notices.fix();
        let verb = self.xml_verb;
        let mut r = Reader {
            lib: self,
            out,
            notices,
            dialect,
            fix,
            verb,
            gim: GiftiImage::default(),
            da_list: None,
            da_ind: 0,
            ele_da: 0,
            exp_da: 0,
            b64_errors: 0,
            errors: 0,
            skip: 0,
            depth: 0,
            stack: [0; 11],
            dind: 0,
            clen: 0,
            cdata: Target::None,
            cdata_parent: None,
            dlen: 0,
            doff: 0,
            ddata: Vec::new(),
            ddata_init: 0,
            xdata: Vec::new(),
            zdata: Vec::new(),
        };
        r.init(dalist);
        r.lib.dstore = read_data as i32;
        let name = fname.to_string_lossy().into_owned();
        let mut f = match std::fs::File::open(fname) {
            Ok(f) => f,
            Err(_) => {
                r.out
                    .err(&format!("** failed to open GIFTI XML file '{name}'\n"));
                return None;
            }
        };
        let mut bsize = 0i32;
        r.reset_xml_buf(&mut bsize);
        if r.verb > 1 {
            r.out.err(&format!("-- reading gifti image '{name}'\n"));
            if let Some(l) = &r.da_list {
                r.out.err(&format!("   (length {} DA list)\n", l.len()));
            }
            r.out.err(&format!("-- using {bsize} byte XML buffer\n"));
            if r.verb > 4 {
                r.show_enames();
            }
        }
        let mut parser = xml::Parser::new();
        if r.lib.xml_verb > 3 {
            r.out.err("-- parser initialized\n");
        }
        let mut ok = true;
        let mut pcount = 1;
        let mut buf = Vec::new();
        loop {
            r.reset_xml_buf(&mut bsize);
            buf.resize(bsize as usize, 0);
            let blen = read_full(&mut f, &mut buf);
            let done = blen < 8; // `blen < sizeof(buf)`: the size of a pointer
            if r.verb > 3 {
                r.out.err(&format!("-- XML_Parse # {pcount}\n"));
            }
            pcount += 1;
            if parser.parse(&buf[..blen], done, &mut r).is_err() {
                let code = parser.error_code();
                r.out.err(&format!(
                    "** {} at line {}\n",
                    xml::error_string(code).unwrap_or("(null)"),
                    parser.current_line_number() as u32
                ));
                ok = false;
                break;
            }
            if done {
                break;
            }
        }
        r.save_state(ok);
        if !ok {
            let gim = std::mem::take(&mut r.gim);
            r.lib.free_image(gim, r.out);
        }
        if r.verb > 1 {
            if ok {
                let mb = r.lib.gim_da_size(&r.gim, true, r.out);
                r.out.err(&format!(
                    "-- have gifti image '{name}', ({} DA elements = {mb} MB)\n",
                    r.gim.num_da
                ));
            } else {
                r.out.err(&format!("** gifti image '{name}', failure\n"));
            }
        }
        if !ok {
            return None;
        }
        if let (Some(orig), Some(_)) = (dalist, &r.da_list)
            && r.apply_da_list_order(orig) != 0
        {
            r.out.err("** failed apply_da_list_order\n");
            r.lib.gxd.gim = false;
            let gim = std::mem::take(&mut r.gim);
            r.lib.free_image(gim, r.out);
            return None;
        }
        Some(r.gim)
    }
}

fn read_full(f: &mut std::fs::File, buf: &mut [u8]) -> usize {
    let mut n = 0;
    while n < buf.len() {
        match f.read(&mut buf[n..]) {
            Ok(0) | Err(_) => break,
            Ok(k) => n += k,
        }
    }
    n
}

impl Reader<'_> {
    /// `init_gxml_data` (not the user variables).
    fn init(&mut self, dalist: Option<&[i32]>) {
        if let Some(l) = dalist {
            let mut c: Vec<i32> = l.to_vec();
            c.sort_unstable();
            c.dedup();
            if self.verb > 1 {
                let o: String = l.iter().map(|v| format!(" {v}")).collect();
                self.out.err(&format!("-- original da_list:{o}\n"));
                let u: String = c.iter().map(|v| format!(" {v}")).collect();
                self.out.err(&format!("++ unique, sorted da_list:{u}\n"));
            }
            self.da_list = Some(c);
        }
        if self.verb > 2 {
            let l = &*self.lib;
            let mut s = format!(
                "-- user opts: gxml_data :\n   verb        : {}\n   dstore      : {}\n   indent      : {}\n   buf_size    : {}\n   b64_check   : {}\n   zlevel      : {}\n   da_len      : {}\n",
                l.xml_verb,
                l.dstore,
                l.indent,
                l.buf_size,
                l.b64_check,
                l.zlevel,
                self.da_list.as_ref().map_or(0, |v| v.len())
            );
            if self.verb > 3 {
                s += &format!(
                    "   da_list     : {}\n",
                    if self.da_list.is_some() {
                        "<set>"
                    } else {
                        "(nil)"
                    }
                );
                s += &self.lib.gxd.lines(0);
            }
            self.out.err(&s);
        }
        self.lib.gxd = GxdState::default();
    }

    /// What this read leaves in `GXD` (its buffers freed; `gim` set unless it failed).
    fn save_state(&mut self, gim: bool) {
        self.lib.gxd = GxdState {
            ele_da: self.ele_da,
            exp_da: self.exp_da,
            b64_errors: self.b64_errors,
            errors: self.errors,
            skip: self.skip,
            depth: self.depth,
            dind: self.dind,
            clen: self.clen as i64,
            doff: self.doff as i64,
            zlen: self.zdata.len() as i64,
            cdata: self.cdata != Target::None,
            gim,
        };
    }

    fn reset_xml_buf(&mut self, bsize: &mut i32) {
        if *bsize == self.lib.buf_size {
            if self.verb > 3 {
                self.out
                    .err(&format!("-- buffer kept at {} bytes\n", *bsize));
            }
            return;
        }
        if self.verb > 2 {
            self.out.err(&format!(
                "++ update buf, {} to {} bytes\n",
                *bsize, self.lib.buf_size
            ));
        }
        *bsize = self.lib.buf_size;
    }

    fn show_enames(&mut self) {
        let mut s = String::from("-------------------------------\n++ ename list :\n");
        for (c, n) in ENAMES.iter().enumerate() {
            s += &format!("    {c:02} : {n}\n");
        }
        s += "-------------------------------\n";
        self.out.err(&s);
    }

    fn show_stack(&mut self, mesg: &str) {
        let mut s = format!("{mesg}stack[{}]", self.depth);
        for c in 0..self.depth.max(0) as usize {
            s += &format!(" : {}", ENAMES[self.stack[c]]);
        }
        self.out.err(&(s + "\n"));
    }

    fn show_attrs(&mut self, etype: usize, attr: &[(Vec<u8>, Vec<u8>)]) {
        let mut s = format!(
            "{}: element {}\n",
            depth_prefix(self.depth, true),
            ENAMES[etype]
        );
        for (n, v) in attr {
            s += &format!(
                "{}      attr: {}='{}'\n",
                depth_prefix(self.depth, false),
                lossy(n),
                lossy(v)
            );
        }
        self.out.err(&s);
    }

    /// The current array (`gim->darray[numDA-1]`), when there is one.
    fn cur_da(&self) -> Option<usize> {
        let n = self.gim.num_da as usize;
        (n >= 1 && n <= self.gim.darray.len() && self.gim.darray[n - 1].is_some()).then(|| n - 1)
    }

    fn da(&mut self, i: usize) -> &mut DataArray {
        self.gim.darray[i].as_mut().expect("a current DataArray")
    }

    /// An access gifticlib makes through a null or out-of-range pointer (it crashes there):
    /// larmorx skips the element instead, with a notice.
    fn crash(&mut self, what: &str) -> i32 {
        self.notices.add(
            &d::FS_85,
            format!(
                "{what}: gifticlib reads through an unset pointer (a crash); the element is skipped"
            ),
        );
        self.skip = self.depth;
        1
    }

    fn epush(&mut self, etype: usize, ename: &[u8], attr: &[(Vec<u8>, Vec<u8>)]) -> i32 {
        // a forgotten text target applies only until the next element starts
        self.cdata_parent = None;
        if self.depth < 0 || self.depth > MAX_DEPTH {
            self.out.err(&format!(
                "** push: stack depth {} out of [0,{MAX_DEPTH}] range\n",
                self.depth
            ));
            self.errors += 1;
            return 1;
        }
        if self.verb > 4 {
            self.out.err(&format!(
                "{}++ push {etype:02}: '{}'\n",
                depth_prefix(self.depth, true),
                ENAMES[etype]
            ));
        }
        self.stack[self.depth as usize] = etype;
        self.depth += 1;
        if self.skip != 0 {
            if self.verb > 2 {
                self.out.err(&format!(
                    "-- skip={}, depth={}, skipping element '{}'\n",
                    self.skip,
                    self.depth,
                    lossy(ename)
                ));
            }
            return 0;
        }
        if etype == E_INVALID {
            if self.verb > 0 {
                self.out.err(&format!(
                    "** pushed invalid element, '{}', skip depth {}\n",
                    lossy(ename),
                    self.depth
                ));
            }
            self.skip = self.depth;
            return 1;
        }
        if self.verb > 5 {
            self.show_stack("++ ");
        }
        if !self.stack_is_valid() {
            return 1;
        }
        match etype {
            E_GIFTI => self.push_gifti(attr),
            E_META => self.push_meta(),
            E_MD => self.push_md(),
            E_NAME => self.push_name_value(true),
            E_VALUE => self.push_name_value(false),
            E_LABELTABLE => self.push_lt(),
            E_LABEL => self.push_label(attr),
            E_DATAARRAY => self.push_darray(attr),
            E_CSTM => self.push_cstm(),
            E_DATA => self.push_data(),
            E_DATASPACE => self.push_space(true),
            E_XFORMSPACE => self.push_space(false),
            E_MATRIXDATA => self.push_xform(),
            _ => 0,
        }
    }

    fn stack_is_valid(&mut self) -> bool {
        if self.depth <= 0 {
            return self.depth == 0;
        }
        let etype = self.stack[self.depth as usize - 1];
        if self.depth == 1 {
            if etype != E_GIFTI {
                self.show_stack("** invalid element on ");
                return false;
            }
            return true;
        }
        let parent = self.stack[self.depth as usize - 2];
        let mut valid = true;
        let bad_parent = match etype {
            E_META => parent != E_GIFTI && parent != E_DATAARRAY,
            E_MD => parent != E_META,
            E_NAME | E_VALUE => parent != E_MD,
            E_LABELTABLE | E_DATAARRAY => parent != E_GIFTI,
            E_LABEL => parent != E_LABELTABLE,
            E_CSTM | E_DATA => parent != E_DATAARRAY,
            E_DATASPACE | E_XFORMSPACE | E_MATRIXDATA => parent != E_CSTM,
            E_CDATA => ![
                E_NAME,
                E_VALUE,
                E_LABEL,
                E_DATASPACE,
                E_XFORMSPACE,
                E_MATRIXDATA,
            ]
            .contains(&parent),
            _ => {
                valid = false;
                false
            }
        };
        if bad_parent && self.lib.xml_verb != 0 {
            self.out.err(&format!(
                "** {}: bad parent '{}'\n",
                ENAMES[etype], ENAMES[parent]
            ));
        }
        if (!valid || bad_parent) && self.lib.xml_verb > 1 {
            self.show_stack("** invalid ");
        }
        valid
    }

    fn push_gifti(&mut self, attr: &[(Vec<u8>, Vec<u8>)]) -> i32 {
        if self.lib.verb > 5 {
            self.out.err("-- clearing gifti_image\n");
        }
        self.gim = GiftiImage::default();
        for (n, v) in attr {
            let mut g = std::mem::take(&mut self.gim);
            let r = self.lib.str2attr_gifti(&mut g, n, v, self.out);
            if r != 0 {
                self.lib.add_to_nvpairs(&mut g.ex_atrs, n, v, self.out);
            }
            self.gim = g;
        }
        if self.verb > 1 {
            self.out
                .err(&format!("++ set {} GIFTI attr(s)\n", attr.len()));
        }
        if self.verb > 3 {
            self.lib
                .disp_gifti_image(Some("push:"), &self.gim, false, self.out, self.notices);
        }
        if self.gim.num_da >= 0 {
            self.exp_da = self.gim.num_da;
            self.gim.num_da = 0;
            if self.verb > 1 {
                self.out
                    .err(&format!("-- expecting {} DA elements\n", self.exp_da));
            }
        }
        0
    }

    /// `find_current_MetaData`: which metadata the element `cdepth` below `MetaData` fills.
    fn find_meta(&mut self, cdepth: i32) -> Option<Meta> {
        if self.depth < 2 + cdepth {
            // gifticlib prints the address of its global state here.
            self.out
                .err(&format!("FMeta: bad params (<gxml_data>,{cdepth})\n"));
            return None;
        }
        let parent = self.stack[(self.depth - 2 - cdepth) as usize];
        if parent == E_GIFTI {
            Some(Meta::Image)
        } else if parent == E_DATAARRAY {
            if self.gim.darray.is_empty() {
                self.out.err("** FMeta: gim->darry not initialized\n");
                return None;
            }
            let i = self.gim.num_da - 1;
            match self.cur_da() {
                Some(k) => Some(Meta::Da(k)),
                None => {
                    self.out
                        .err(&format!("** FMeta: gim->darry[{i}] not initialized\n"));
                    None
                }
            }
        } else {
            self.out.err(&format!(
                "** FMeta: child of invalid parent '{}'\n",
                ENAMES[parent]
            ));
            None
        }
    }

    fn meta_mut(&mut self, m: Meta) -> &mut NvPairs {
        match m {
            Meta::Image => &mut self.gim.meta,
            Meta::Da(i) => &mut self.da(i).meta,
        }
    }

    fn push_meta(&mut self) -> i32 {
        let Some(m) = self.find_meta(0) else {
            return self.crash("MetaData without its parent's metadata");
        };
        if !self.meta_mut(m).is_empty() {
            self.out.err("** push meta: already initialized??\n");
            return 1;
        }
        0
    }

    fn push_md(&mut self) -> i32 {
        let Some(m) = self.find_meta(1) else { return 1 };
        let md = self.meta_mut(m);
        md.name.push(None);
        md.value.push(None);
        0
    }

    fn push_name_value(&mut self, name: bool) -> i32 {
        let Some(m) = self.find_meta(2) else { return 1 };
        let n = self.meta_mut(m).len();
        if n == 0 {
            return self.crash(if name { "Name" } else { "Value" });
        }
        let t = if name {
            Target::MdName(m, n - 1)
        } else {
            Target::MdValue(m, n - 1)
        };
        self.cdata = t;
        self.set_target(None);
        self.clen = 0;
        0
    }

    fn push_lt(&mut self) -> i32 {
        if !self.gim.labeltable.is_empty() {
            self.out.err("** multiple giiLabelTables?\n");
        }
        0
    }

    fn push_label(&mut self, attr: &[(Vec<u8>, Vec<u8>)]) -> i32 {
        let lt = &mut self.gim.labeltable;
        lt.key.push(0);
        lt.label.push(None);
        let n = lt.len();
        if attr.is_empty() {
            // expat always passes an attribute list (perhaps empty): get_label_attrs returns 0,
            // and the key is never set.
            self.notices.add(
                &d::FS_80,
                "a Label without attributes: its key is memory gifticlib never writes (0 here)",
            );
        } else {
            let mut key = 0;
            let mut rgba = [0f32; 4];
            let rv = self.get_label_attrs(attr, &mut key, &mut rgba);
            if rv == 1 || rv == 2 {
                self.gim.labeltable.key[n - 1] = key;
            } else {
                self.notices.add(
                    &d::FS_80,
                    "a Label whose Key attribute is missing or wrong: its key is memory gifticlib never writes (0 here)",
                );
            }
            if rv == 2 {
                self.add_label_rgba(rgba);
            }
        }
        self.cdata = Target::Label(n - 1);
        self.set_target(None);
        self.clen = 0;
        0
    }

    fn add_label_rgba(&mut self, rgba: [f32; 4]) {
        let lt = &mut self.gim.labeltable;
        let n = lt.len();
        if n > 1 && lt.rgba.is_none() {
            self.out.err(&format!(
                "** first RGBA at Label {}, so table is incomplete\n",
                n - 1
            ));
            return;
        }
        let v = lt.rgba.get_or_insert_with(Vec::new);
        v.resize(4 * n, None);
        for k in 0..4 {
            v[4 * (n - 1) + k] = Some(rgba[k]);
        }
        if self.verb > 4 {
            let w = rgba_of(&self.gim.labeltable, 0, self.notices);
            self.out.err(&format!(
                "-- adding Label RGBA {} {} {} {}\n",
                cfmt::g(w[0] as f64, 6),
                cfmt::g(w[1] as f64, 6),
                cfmt::g(w[2] as f64, 6),
                cfmt::g(w[3] as f64, 6)
            ));
        }
    }

    fn get_label_attrs(
        &mut self,
        attr: &[(Vec<u8>, Vec<u8>)],
        key: &mut i32,
        rgba: &mut [f32; 4],
    ) -> i32 {
        let lind = self.gim.labeltable.len() as i32 - 1;
        let mut found = 0;
        for (n, v) in attr {
            match n.as_slice() {
                b"Key" | b"Index" => {
                    *key = atol(v) as i32;
                    found |= 1;
                }
                b"Red" | b"Green" | b"Blue" | b"Alpha" => {
                    let (k, nm) = match n.as_slice() {
                        b"Red" => (0, "Red"),
                        b"Green" => (1, "Green"),
                        b"Blue" => (2, "Blue"),
                        _ => (3, "Alpha"),
                    };
                    let (x, used) = strtod(v);
                    rgba[k] = x as f32;
                    if used == 0 {
                        self.out
                            .err(&format!("** bad GIFTI label {lind} {nm} attr\n"));
                        self.show_attrs(E_LABEL, attr);
                        return -1;
                    }
                    found |= 1 << (k + 1);
                }
                _ => {
                    self.out
                        .err(&format!("** unknown GIFTI label {lind} attr\n"));
                    self.show_attrs(E_LABEL, attr);
                    return -1;
                }
            }
        }
        if found == 0 {
            self.out
                .err(&format!("** GIFTI label {lind}, missing 'Key' attr\n"));
            return 0;
        } else if found != 1 && found != 31 {
            self.out
                .err(&format!("** GIFTI label {lind}, partial attributes\n"));
            self.show_attrs(E_LABEL, attr);
            return -1;
        }
        if self.verb > 2 {
            if found == 1 {
                self.out.err(&format!("-- have Label Key {key}\n"));
            } else {
                self.out.err(&format!(
                    "-- have Label Key {key}, RGBA {} {} {} {}\n",
                    cfmt::g(rgba[0] as f64, 6),
                    cfmt::g(rgba[1] as f64, 6),
                    cfmt::g(rgba[2] as f64, 6),
                    cfmt::g(rgba[3] as f64, 6)
                ));
            }
        }
        if found == 1 { 1 } else { 2 }
    }

    fn push_darray(&mut self, attr: &[(Vec<u8>, Vec<u8>)]) -> i32 {
        self.ele_da += 1;
        if let Some(l) = &self.da_list {
            if self.da_ind < l.len() && l[self.da_ind] == self.ele_da - 1 {
                if self.verb > 1 {
                    self.out
                        .err(&format!("++ keeping DA[{}]\n", self.ele_da - 1));
                }
                self.da_ind += 1;
            } else {
                if self.verb > 1 {
                    self.out
                        .err(&format!("++ skipping DA[{}]\n", self.ele_da - 1));
                }
                self.skip = self.depth;
                return 1;
            }
        }
        let mut g = std::mem::take(&mut self.gim);
        self.lib.add_empty_darray(&mut g, 1, self.out);
        self.gim = g;
        let i = self.gim.num_da as usize - 1;
        let mut da = self.gim.darray[i].take().unwrap_or_default();
        self.lib
            .set_da_atrs(&mut da, attr, true, self.fix, self.out);
        let _ = self
            .lib
            .valid_data_array(Some(&da), self.verb > 1, self.fix, self.out);
        if da.nvals > 0 && da.nbyper > 0 {
            self.update_xml_buf_size(da.nvals.wrapping_mul(da.nbyper as i64));
        }
        if self.verb > 4 {
            self.lib
                .disp_data_array(Some("push:"), Some(&da), false, self.out);
        }
        self.gim.darray[i] = Some(da);
        0
    }

    fn update_xml_buf_size(&mut self, bytes: i64) {
        if bytes < 0 {
            if self.verb > 1 {
                self.out.err(&format!(
                    "** bad update_xml_buf_size with <gxml_data> and {bytes}\n"
                ));
            }
            return;
        }
        let n = partial_buf_size(bytes);
        if n != self.lib.buf_size {
            if self.verb > 2 {
                self.out.err(&format!(
                    "++ update XML buf size, {} to {n} (for {bytes})\n",
                    self.lib.buf_size
                ));
            }
            self.lib.buf_size = n;
        }
    }

    /// `update_partial_buffer` for `ddata` (`zdata` with `full`).
    fn update_partial(&mut self, which_z: bool, bytes: i64, full: bool) -> i32 {
        let bsize = if full {
            bytes as i32
        } else {
            partial_buf_size(bytes)
        };
        if bytes <= 0 {
            self.out
                .err(&format!("** UPB: bad params (<buf>,<blen>,{bytes})\n"));
            return 1;
        }
        let (buf_empty, len) = if which_z {
            (self.zdata.is_empty(), self.zdata.len())
        } else {
            (self.ddata.is_empty(), self.dlen)
        };
        if !buf_empty || len != bsize as usize {
            if self.lib.xml_verb > 2 {
                self.out.err(&format!(
                    "++ UPB, alloc {bsize} bytes (from {bytes}, {}) for buff\n",
                    full as i32
                ));
            }
            let n = bsize.max(0) as usize;
            if which_z {
                self.zdata.resize(n, 0);
            } else {
                self.ddata.resize(n, 0);
                self.ddata_init = self.ddata_init.min(n);
                self.dlen = n;
            }
        }
        0
    }

    fn push_cstm(&mut self) -> i32 {
        let Some(i) = self.cur_da() else {
            return self.crash("CoordinateSystemTransformMatrix outside a DataArray");
        };
        let intent = self.da(i).intent;
        if self.verb > 0
            && let Some(m) = self.dialect.coordsys_intent_message(i as i32, intent)
        {
            self.out.err(&m);
        }
        let mut da = self.gim.darray[i].take().expect("current DataArray");
        self.lib.add_empty_cs(&mut da, self.out);
        self.gim.darray[i] = Some(da);
        0
    }

    fn push_data(&mut self) -> i32 {
        let Some(i) = self.cur_da() else {
            return self.crash("Data outside a DataArray");
        };
        self.dind = 0;
        self.doff = 0;
        if self.fix {
            self.lib.piece = [0; 3];
        }
        if self.lib.dstore == 0 {
            if self.verb > 3 {
                self.out.err(&format!("-- skipping data[{i}]\n"));
            }
            self.skip = self.depth;
            return 1;
        }
        let (nbyper, nvals, encoding) = {
            let da = self.da(i);
            (da.nbyper, da.nvals, da.encoding)
        };
        let bytes = (nbyper as i64).wrapping_mul(nvals);
        if self.update_partial(false, bytes, false) != 0 {
            return 1;
        }
        if encoding == GIFTI_ENCODING_B64GZ {
            let zsize = (bytes as f64 * 1.01 + 12.0) as i32;
            if self.verb > 2 {
                self.out
                    .err("++ creating extra zdata for zlib extraction\n");
            }
            if self.update_partial(true, zsize as i64, true) != 0 {
                return 1;
            }
        }
        if nvals <= 0 || nbyper <= 0 {
            self.out.err(&format!(
                "** PD: bad vals,bytes = {}, {nbyper}\n",
                nvals as u32
            ));
            return 1;
        }
        self.da(i).data = Some(vec![0u8; bytes as usize]);
        if self.verb > 3 {
            self.out
                .err(&format!("++ PD: alloc {bytes} bytes for darray[{i}]\n"));
        }
        0
    }

    fn push_space(&mut self, data: bool) -> i32 {
        let Some(i) = self.cur_da() else {
            return self.crash(if data {
                "DataSpace"
            } else {
                "TransformedSpace"
            });
        };
        let ncs = self.da(i).num_cs();
        let (tag, what) = if data {
            ("PD", "dataspace")
        } else {
            ("PX", "xformspace")
        };
        if ncs <= 0 {
            self.out.err(&format!(
                "** {tag}: bad numCS {ncs} in darray {i}, skipping..."
            ));
            self.skip = self.depth;
            return 1;
        }
        let k = ncs as usize - 1;
        if self.da(i).coordsys[k].is_none() {
            self.out
                .err(&format!("** found {what} without coordsys, skipping...\n"));
            self.skip = self.depth;
            return 1;
        }
        self.cdata = if data {
            Target::DataSpace(i, k)
        } else {
            Target::XformSpace(i, k)
        };
        self.set_target(None);
        self.clen = 0;
        0
    }

    fn push_xform(&mut self) -> i32 {
        let Some(i) = self.cur_da() else {
            return self.crash("MatrixData outside a DataArray");
        };
        let ncs = self.da(i).num_cs();
        if ncs <= 0 {
            self.out.err(&format!(
                "** PXform: bad numCS {ncs} in darray {i}, skipping..."
            ));
            self.skip = self.depth;
            return 1;
        }
        if self.da(i).coordsys[ncs as usize - 1].is_none() {
            self.out
                .err("** found xform without coordsys, skipping...\n");
            self.skip = self.depth;
            return 1;
        }
        if self.xdata.is_empty() {
            self.xdata = vec![0; 2048];
        }
        self.dind = 0;
        self.doff = 0;
        0
    }

    fn epop(&mut self, etype: usize, ename: &[u8]) -> i32 {
        // gifticlib forgets where text goes; text that still follows inside the same element
        // is written through that null pointer (FS-86): kept here to append it instead.
        if self.cdata != Target::None {
            self.cdata_parent = Some((self.depth - 1, self.cdata));
        }
        self.cdata = Target::None;
        self.clen = 0;
        if self.skip == self.depth {
            if self.verb > 2 {
                self.out.err(&format!(
                    "-- popping skip element '{}' at depth {}\n",
                    lossy(ename),
                    self.depth
                ));
            }
            self.skip = 0;
        } else {
            match etype {
                E_DATA => {
                    if self.verb > 3 {
                        self.out.err(&format!("-- data dind = {}\n", self.dind));
                    }
                    if self.cur_da().is_none() {
                        // the end of a Data inside an array a da_list leaves out, before any
                        // array is stored: gifticlib reads its array list at index -1
                        self.notices.add(
                            &d::FS_85,
                            "a Data element of an array left out by the DataArray list, before any array is stored: gifticlib reads through an unset pointer (a crash)",
                        );
                    }
                    if let Some(i) = self.cur_da()
                        && self.da(i).data.is_some()
                        && self.dind == 0
                    {
                        if self.verb > 3 {
                            self.out.err("   (freeing data)\n");
                        }
                        self.da(i).data = None;
                    }
                }
                E_DATAARRAY => {
                    self.pop_darray();
                }
                E_GIFTI => {
                    if self.ele_da != self.exp_da {
                        self.out.err(&format!(
                            "** found {} DAs, expected {}\n",
                            self.ele_da, self.exp_da
                        ));
                    } else if let Some(l) = &self.da_list
                        && l.len() != self.da_ind
                    {
                        let (a, b) = (self.da_ind, l.len());
                        self.out.err(&format!("** stored {a} DAs, wanted {b}\n"));
                    }
                    if self.verb > 2 {
                        self.lib.disp_gifti_image(
                            Some("pop:"),
                            &self.gim,
                            self.verb > 4,
                            self.out,
                            self.notices,
                        );
                    }
                    if self.verb > 1 {
                        if self.gim.swapped != 0 {
                            self.out.err("++ data was byte-swapped\n");
                        }
                        if self.gim.compressed != 0 {
                            self.out.err("++ data was compressed\n");
                        }
                    }
                }
                _ => {}
            }
        }
        self.depth -= 1;
        if self.verb > 5 {
            self.out.err(&format!(
                "{}++ pop {etype:02} : '{}'\n",
                depth_prefix(self.depth, true),
                ENAMES[etype]
            ));
        }
        if self.depth < 0 || self.depth > MAX_DEPTH {
            self.out.err(&format!(
                "** pop: stack depth {} out of [0,{MAX_DEPTH}] range\n",
                self.depth
            ));
            self.errors += 1;
            return -1;
        }
        0
    }

    fn pop_darray(&mut self) -> i32 {
        let Some(i) = self.cur_da() else { return 1 };
        if self.b64_errors > 0 {
            let ck = self.lib.b64_check;
            if ck == GIFTI_B64_CHECK_DETECT {
                self.out
                    .err(&format!("** bad base64 chars found in DataArray[{i}]\n"));
            } else if ck == GIFTI_B64_CHECK_COUNT || ck == GIFTI_B64_CHECK_SKIPNCOUNT {
                self.out.err(&format!(
                    "** {} bad base64 chars found in DataArray[{i}]\n",
                    self.b64_errors
                ));
            }
            self.b64_errors = 0;
        }
        let (enc, has_data, nvals, nbyper) = {
            let da = self.da(i);
            (da.encoding, da.data.is_some(), da.nvals, da.nbyper)
        };
        let want = nvals.wrapping_mul(nbyper as i64);
        if enc == GIFTI_ENCODING_B64GZ && has_data {
            if self.verb > 2 {
                self.out.err(&format!(
                    "-- uncompressing {} bytes into {want}\n",
                    self.dind
                ));
            }
            let zin = self.zdata[..(self.dind.max(0) as usize).min(self.zdata.len())].to_vec();
            let dst = self.da(i).data.as_mut().expect("data");
            // through a scratch buffer: zlib-rs may write past what it reports as produced (its
            // copies run in blocks), zlib never does; only the bytes produced reach the data
            let mut tmp = vec![0u8; dst.len()];
            let (rv, olen) = uncompress(&mut tmp, &zin);
            dst[..olen].copy_from_slice(&tmp[..olen]);
            let olen = olen as i64;
            if rv != 0 {
                self.out.err(&format!("** uncompress fails for DA[{i}]\n"));
                self.out.err(match rv {
                    -4 => "   (zlib failure, not enough memory)\n",
                    -5 => "   (zlib failure, output buffer too short)\n",
                    _ => "   (zlib failure, corrupted data)\n",
                });
            } else if self.verb > 2 || (self.verb > 1 && self.gim.num_da == 1) {
                self.out.err(&format!(
                    "-- uncompressed buffer ({}% of {olen} bytes)\n",
                    cfmt::f(100.0 * self.dind as f64 / olen as f64, 2)
                ));
            }
            if olen != want {
                self.out.err(&format!(
                    "** uncompressed buf is {olen} bytes, expected {want}\n"
                ));
            }
            self.gim.compressed = 1;
        }
        if self.da(i).ext_fname.as_ref().is_some_and(|n| !n.is_empty()) {
            let mut da = self.gim.darray[i].take().expect("current DataArray");
            self.lib.read_extern_da_data(&mut da, self.fix, self.out);
            self.gim.darray[i] = Some(da);
        }
        let (dt, has_data, endian, enc) = {
            let da = self.da(i);
            (da.datatype, da.data.is_some(), da.endian, da.encoding)
        };
        if has_data && enc != GIFTI_ENCODING_ASCII {
            let swapsize = self
                .lib
                .datatype_sizes(dt, self.fix, self.out)
                .map_or(0, |s| s.1);
            if swapsize <= 0 {
                self.out
                    .err(&format!("** bad swapsize {swapsize} for dtype {dt}\n"));
                return 1;
            }
            let n = want / swapsize as i64;
            let mut data = self.da(i).data.take().expect("data");
            if self
                .lib
                .check_swap(&mut data, endian, n, swapsize, self.out)
                != 0
            {
                self.gim.swapped = 1;
            }
            self.da(i).data = Some(data);
        }
        0
    }

    fn set_target(&mut self, v: CStr) {
        let t = self.cdata;
        if let Some(slot) = self.target_slot(t) {
            *slot = v;
        }
    }

    fn target_slot(&mut self, t: Target) -> Option<&mut CStr> {
        Some(match t {
            Target::None => return None,
            Target::MdName(m, k) => &mut self.meta_mut(m).name[k],
            Target::MdValue(m, k) => &mut self.meta_mut(m).value[k],
            Target::Label(k) => &mut self.gim.labeltable.label[k],
            Target::DataSpace(i, k) => &mut self.da(i).coordsys[k].as_mut().expect("cs").dataspace,
            Target::XformSpace(i, k) => {
                &mut self.da(i).coordsys[k].as_mut().expect("cs").xformspace
            }
        })
    }

    fn append_to_cdata(&mut self, s: &[u8]) -> i32 {
        if self.cdata == Target::None {
            // gifticlib writes through its null text pointer here (a crash; FS-86). The text of
            // an element whose target was forgotten when a child of it ended (a CDATA section,
            // an element) is appended to it; text with no target at all is dropped.
            let top = if self.depth > 0 {
                self.stack[(self.depth - 1) as usize]
            } else {
                0
            };
            let at = |d: i32| d == self.depth || (d + 1 == self.depth && top == E_CDATA);
            let saved = self.cdata_parent.filter(|(d, _)| at(*d));
            self.notices.add(
                &d::FS_86,
                "text where gifticlib has no place for it (after a CDATA section or a child element inside a text element, or inside a misplaced one): gifticlib writes it through a null pointer (a crash)",
            );
            match saved {
                Some((_, t)) => {
                    self.cdata = t;
                    self.clen = self
                        .target_slot(t)
                        .and_then(|v| v.as_ref())
                        .map_or(0, |v| v.len() + 1);
                }
                None => return 1,
            }
        }
        let t = self.cdata;
        let verb = self.verb;
        if s.is_empty() {
            self.out.err(&format!(
                "** A2CD, bad params (<gxml_data>,<cdata>,{})\n",
                s.len()
            ));
            return 1;
        }
        let first = self.target_slot(t).is_none_or(|v| v.is_none());
        self.clen = if first {
            s.len() + 1
        } else {
            self.clen + s.len()
        };
        if verb > 4 {
            self.out.err(&format!(
                "++ a2cdata, len {}, clen {}, data '{}'\n",
                s.len(),
                self.clen,
                lossy(s)
            ));
        }
        if let Some(slot) = self.target_slot(t) {
            let v = slot.get_or_insert_with(Vec::new);
            if first {
                v.clear();
            }
            v.extend_from_slice(s);
        }
        0
    }

    fn append_to_data(&mut self, s: &[u8]) -> i32 {
        let Some(i) = self.cur_da() else { return 1 };
        if self.dlen == 0 || self.ddata.is_empty() || self.dind < 0 {
            self.out.err(&format!(
                "** A2D: bad setup (<da>,{},{},{})\n",
                self.dlen,
                if self.ddata.is_empty() {
                    "(nil)"
                } else {
                    "<buf>"
                },
                self.dind
            ));
            return 1;
        }
        let (has, enc, nvals, nbyper) = {
            let da = self.da(i);
            (da.data.is_some(), da.encoding, da.nvals, da.nbyper)
        };
        if !has {
            self.out.err("** A2D: no data allocated\n");
            return 1;
        }
        match enc {
            GIFTI_ENCODING_ASCII => self.append_to_data_ascii(i, s),
            GIFTI_ENCODING_B64BIN => {
                self.append_to_data_b64(i, false, nvals.wrapping_mul(nbyper as i64), s)
            }
            GIFTI_ENCODING_B64GZ => {
                let tot = self.zdata.len() as i64;
                self.append_to_data_b64(i, true, tot, s)
            }
            _ => {
                let n = list_index2string(List::Encoding, enc, self.lib.verb, self.out);
                self.out
                    .err(&format!("** A2D: invalid encoding value {enc} ({n})\n"));
                1
            }
        }
    }

    /// `copy_b64_data`: the piece copied into `ddata` at `doff` (bad characters handled by
    /// `b64_check`); the number of bytes stored.
    fn copy_b64(&mut self, src: &[u8]) -> usize {
        if self.verb > 1 {
            let c = src.iter().filter(|&&b| b64_decode(b) == 128).count();
            if c > 0 {
                self.out.err(&format!("CB64D: found {c} bad b64 chars\n"));
                if self.verb > 5 {
                    let mut s = String::from("-- bad b64 chars:");
                    for &b in src.iter().filter(|&&b| b64_decode(b) == 128) {
                        s += &format!(" 0x{b:02x}");
                    }
                    s.push('\n');
                    self.out.err(&s);
                }
            }
        }
        let doff = self.doff;
        let mut errs = 0;
        let ck = self.lib.b64_check;
        let apply = match ck {
            GIFTI_B64_CHECK_SKIP | GIFTI_B64_CHECK_SKIPNCOUNT => {
                let mut n = 0;
                for &b in src {
                    if b64_decode(b) == 128 {
                        errs += 1;
                    } else {
                        self.ddata[doff + n] = b;
                        n += 1;
                    }
                }
                if ck == GIFTI_B64_CHECK_SKIP {
                    errs = 0;
                }
                n
            }
            _ => {
                if !(GIFTI_B64_CHECK_NONE..=GIFTI_B64_CHECK_COUNT).contains(&ck) {
                    self.out.err(&format!("** CB64D: b64_check = {ck}\n"));
                }
                self.ddata[doff..doff + src.len()].copy_from_slice(src);
                if ck == GIFTI_B64_CHECK_DETECT {
                    errs = src.iter().any(|&b| b64_decode(b) == 128) as i32;
                } else if ck == GIFTI_B64_CHECK_COUNT {
                    errs = src.iter().filter(|&&b| b64_decode(b) == 128).count() as i32;
                }
                src.len()
            }
        };
        self.ddata[doff + apply] = 0;
        self.ddata_init = self.ddata_init.max(doff + apply + 1);
        self.b64_errors += errs;
        apply
    }

    fn append_to_data_b64(&mut self, i: usize, z: bool, tot: i64, cdata: &[u8]) -> i32 {
        if self.verb > 4 {
            self.out.err(&format!(
                "++ appending {} base64 binary bytes to data\n",
                cdata.len()
            ));
        }
        let mut rem_in = cdata.len();
        while rem_in > 0 {
            let start = cdata.len() - rem_in;
            let mut copy_len = self.dlen - self.doff - 1;
            let unused = if copy_len > rem_in {
                let u = copy_len - rem_in;
                copy_len = rem_in;
                u
            } else {
                0
            };
            let old_doff = self.doff;
            let apply = self.copy_b64(&cdata[start..start + copy_len]);
            let mut needed = tot - self.dind;
            if self.verb > 5 {
                self.out
                    .err(&format!("-- {needed} bytes left at offset {}\n", self.dind));
            }
            let n_in = old_doff + apply;
            let rv = self.decode_b64(i, z, n_in, &mut needed);
            if rv < 0 {
                self.doff = 0;
                return 1;
            }
            self.doff = rv as usize;
            if self.doff >= self.dlen - 1 {
                if self.verb != 0 {
                    self.out.err("** A2Db64: failed to process buffer\n");
                }
                self.out.err(&format!("** rem = {}\n", self.doff));
                self.doff = 0;
            }
            if self.doff > 0 {
                let wrong = self.dlen - (unused + 1) - self.doff;
                let right = n_in - self.doff;
                if self.verb > 5 {
                    self.out.err(&format!(
                        "++ A2Db64: move {} bytes from {} (blen {})\n",
                        self.doff,
                        self.dlen - unused - self.doff,
                        self.dlen
                    ));
                }
                let from = if wrong == right {
                    right
                } else if self.fix {
                    self.notices.add(
                        &d::FS_81,
                        "the base64 remainder carried from its real place",
                    );
                    right
                } else {
                    if wrong + self.doff > self.ddata_init {
                        self.notices.add(
                            &d::FS_81,
                            "base64 bytes gifticlib takes from memory it never wrote read as 0",
                        );
                        for k in self.ddata_init.max(wrong)..wrong + self.doff {
                            self.ddata[k] = 0;
                        }
                    }
                    wrong
                };
                self.ddata.copy_within(from..from + self.doff, 0);
                if self.verb > 6 {
                    self.out.err(&format!(
                        "   bytes are '{}'\n",
                        prec(&self.ddata, self.doff)
                    ));
                }
            }
            rem_in -= copy_len;
            self.dind = tot - needed;
        }
        0
    }

    /// `decode_b64` of `ddata[..cdlen]` into the array's data (or `zdata`) at `dind`.
    fn decode_b64(&mut self, i: usize, z: bool, cdlen: usize, needed: &mut i64) -> i32 {
        let blocks = cdlen / 4;
        let rem = (cdlen % 4) as i32;
        if self.verb > 4 {
            self.out.err(&format!(
                "-- DB64: decode len {cdlen}, remain {}\n",
                *needed
            ));
        }
        if *needed <= 0 {
            if cdlen > 0 {
                self.out
                    .err(&format!("** DB64: {cdlen} bytes left without a home\n"));
            }
            return 0;
        }
        let din: Vec<u8> = self.ddata[..cdlen].to_vec();
        let base = self.dind as usize;
        let mut decoded = Vec::with_capacity(3 * blocks);
        let mut ind = 0;
        while ind < blocks && *needed >= 3 {
            let w = &din[4 * ind..4 * ind + 4];
            let (a, b, c) = quad(w);
            decoded.extend_from_slice(&[a, b, c]);
            ind += 1;
            *needed -= 3;
        }
        let mut assigned = 3 * ind;
        if ind + 1 < blocks || (ind < blocks && *needed == 0) {
            self.write_decoded(i, z, base, &decoded);
            if self.verb > 6 {
                self.hex("decoded b64: 0x ", &decoded);
            }
            self.out.err("** decode_b64: more data than space\n");
            return -1;
        }
        if ind < blocks {
            let (a, b, _) = quad(&din[4 * ind..4 * ind + 4]);
            if *needed >= 1 {
                decoded.push(a);
            }
            if *needed >= 2 {
                decoded.push(b);
            }
            assigned += *needed as usize;
            *needed = 0;
        }
        self.write_decoded(i, z, base, &decoded);
        if self.verb > 6 {
            let n = assigned.min(decoded.len());
            self.hex("decoded b64: 0x ", &decoded[..n]);
        }
        rem
    }

    fn hex(&mut self, mesg: &str, bytes: &[u8]) {
        let mut s = String::from(mesg);
        for &b in bytes {
            // `%02x` of a (signed) char: bytes above 0x7f print as ffffff..
            if b > 0x7f {
                s += &format!(" {:08x}", b as i8 as i32);
            } else {
                s += &format!(" {b:02x}");
            }
        }
        self.out.err(&s);
    }

    fn write_decoded(&mut self, i: usize, z: bool, base: usize, bytes: &[u8]) {
        let dst: &mut Vec<u8> = if z {
            &mut self.zdata
        } else {
            self.da(i).data.as_mut().expect("data")
        };
        let end = (base + bytes.len()).min(dst.len());
        if base < end {
            dst[base..end].copy_from_slice(&bytes[..end - base]);
        }
    }

    fn append_to_data_ascii(&mut self, i: usize, cdata: &[u8]) -> i32 {
        if self.verb > 4 {
            self.out.err(&format!(
                "++ appending {} ASCII bytes to data\n",
                cdata.len()
            ));
        }
        if whitespace_len(cdata) == cdata.len() {
            self.doff = 0;
            return 0;
        }
        let (dt, nvals, nbyper) = {
            let da = self.da(i);
            (da.datatype, da.nvals, da.nbyper)
        };
        let mut rem_len = cdata.len();
        while rem_len > 0 {
            let start = cdata.len() - rem_len;
            if is_c_space(cdata[start]) {
                self.doff = 0;
            }
            let mut copy_len = self.dlen - self.doff - 1;
            let unused = if copy_len > rem_len {
                let u = copy_len - rem_len;
                copy_len = rem_len;
                u
            } else {
                0
            };
            let doff = self.doff;
            self.ddata[doff..doff + copy_len].copy_from_slice(&cdata[start..start + copy_len]);
            self.ddata[doff + copy_len] = 0;
            self.ddata_init = self.ddata_init.max(doff + copy_len + 1);
            let mut rem_vals = nvals - self.dind;
            if self.verb > 5 {
                self.out.err(&format!(
                    "-- {rem_vals} vals left at offset {}, nbyper {nbyper}\n",
                    self.dind
                ));
            }
            if self.dind == 0 {
                self.lib.mod_prev_data = 0;
            }
            let src = self.ddata[..doff + copy_len].to_vec();
            let mut data = self.da(i).data.take().expect("data");
            let mut mp = self.lib.mod_prev_data;
            let base = self.dind;
            let rv = self.decode_ascii(
                &src,
                dt,
                Dest::Bytes(&mut data),
                base,
                &mut rem_vals,
                &mut mp,
            );
            self.lib.mod_prev_data = mp;
            self.da(i).data = Some(data);
            if rv < 0 {
                self.doff = 0;
                return 1;
            }
            self.doff = rv as usize;
            if self.doff >= self.dlen - 1 {
                if self.verb != 0 {
                    self.out.err("** A2D: failed to process buffer\n");
                }
                self.out.err(&format!("** rem = {}\n", self.doff));
                self.doff = 0;
            }
            if self.doff > 0 {
                if self.verb > 5 {
                    self.out.err(&format!(
                        "++ A2D: move {} bytes from {} (blen {})\n",
                        self.doff,
                        self.dlen - unused - self.doff,
                        self.dlen
                    ));
                }
                let from = self.dlen - (unused + 1) - self.doff;
                self.ddata.copy_within(from..from + self.doff, 0);
                if self.verb > 6 {
                    self.out.err(&format!(
                        "   bytes are '{}'\n",
                        prec(&self.ddata, self.doff)
                    ));
                }
            }
            rem_len -= copy_len;
            self.dind = nvals - rem_vals;
        }
        0
    }

    fn append_to_xform(&mut self, cdata: &[u8]) -> i32 {
        let Some(i) = self.cur_da() else { return 1 };
        let ncs = self.da(i).num_cs();
        if self.xdata.is_empty() || self.dind < 0 || ncs <= 0 {
            self.out.err(&format!(
                "** A2X: bad setup (<da>,{},<buf>,{},{ncs})\n",
                self.xdata.len(),
                self.dind
            ));
            return 1;
        } else if self.verb > 4 {
            self.out
                .err(&format!("++ appending {} bytes to xform\n", cdata.len()));
        }
        let k = ncs as usize - 1;
        if self.da(i).coordsys[k].is_none() {
            self.out.err(&format!("** A2X: bad coordsys[{k}]\n"));
            return 1;
        }
        if whitespace_len(cdata) == cdata.len() {
            self.doff = 0;
            return 0;
        }
        let xlen = self.xdata.len();
        let mut rem_len = cdata.len();
        while rem_len > 0 {
            let start = cdata.len() - rem_len;
            if is_c_space(cdata[start]) {
                self.doff = 0;
            }
            let mut copy_len = xlen - self.doff - 1;
            let unused = if copy_len > rem_len {
                let u = copy_len - rem_len;
                copy_len = rem_len;
                u
            } else {
                0
            };
            let doff = self.doff;
            self.xdata[doff..doff + copy_len].copy_from_slice(&cdata[start..start + copy_len]);
            self.xdata[doff + copy_len] = 0;
            let mut rem_vals = 16 - self.dind;
            if self.dind == 0 {
                self.lib.mod_prev_xform = 0;
            }
            let src = self.xdata[..doff + copy_len].to_vec();
            let mut xf = self.da(i).coordsys[k].as_ref().expect("cs").xform;
            let mut mp = self.lib.mod_prev_xform;
            let base = self.dind;
            let rv = self.decode_ascii(
                &src,
                NIFTI_TYPE_FLOAT64,
                Dest::Xform(&mut xf),
                base,
                &mut rem_vals,
                &mut mp,
            );
            self.lib.mod_prev_xform = mp;
            self.da(i).coordsys[k].as_mut().expect("cs").xform = xf;
            if rv < 0 {
                self.doff = 0;
                return 1;
            }
            self.doff = rv as usize;
            if self.doff >= xlen - 1 {
                if self.verb != 0 {
                    self.out.err("** A2X: failed to process buffer\n");
                }
                self.out.err(&format!("** rem = {}\n", self.doff));
                self.doff = 0;
            }
            if self.doff > 0 {
                // gifticlib shows the Data element's buffer and its length here, not the
                // MatrixData buffer it moves (FS-87); corrected: the MatrixData buffer's.
                let blen = if self.fix { xlen } else { self.dlen };
                if self.verb > 5 {
                    self.out.err(&format!(
                        "++ A2X: move {} bytes from {} (blen {blen})\n",
                        self.doff,
                        blen as i64 - unused as i64 - self.doff as i64,
                    ));
                }
                let from = xlen - (unused + 1) - self.doff;
                self.xdata.copy_within(from..from + self.doff, 0);
                if self.verb > 6 {
                    let s = if self.fix {
                        prec(&self.xdata, self.doff)
                    } else if self.ddata.is_empty() {
                        if self.doff >= 6 {
                            "(null)".to_owned()
                        } else {
                            String::new()
                        }
                    } else {
                        if self.ddata_init < self.doff {
                            self.notices.add(
                                &d::FS_87,
                                "the MatrixData reader's verbosity-7 message shows the Data buffer, partly memory never written (shown from zeros)",
                            );
                        }
                        prec(&self.ddata, self.doff)
                    };
                    self.out.err(&format!("   bytes are '{s}'\n"));
                }
            }
            rem_len -= copy_len;
            self.dind = 16 - rem_vals;
        }
        0
    }

    /// `decode_ascii`: values read from `cdata` into `dest` from value `base` on; the bytes that
    /// may need to be read again (or -1).
    fn decode_ascii(
        &mut self,
        cdata: &[u8],
        t: i32,
        mut dest: Dest,
        base: i64,
        nvals: &mut i64,
        mod_prev: &mut i32,
    ) -> i32 {
        let cdlen = cdata.len();
        let verb = self.verb;
        if verb > 4 {
            self.out.err(&format!(
                "-- DA: type {}, len {cdlen}, nvals {}\n",
                datatype2str(t),
                *nvals
            ));
        }
        let mut vals: i64 = 0;
        if self.doff > 0 && *mod_prev != 0 {
            if verb > 4 {
                self.out.err(&format!(
                    "++ DA: re-proc '{}' from '{}'...\n",
                    prec(cdata, self.doff),
                    prec(cdata, self.doff + 15)
                ));
            }
            vals -= 1;
        }
        let float = matches!(
            t,
            NIFTI_TYPE_FLOAT32 | NIFTI_TYPE_FLOAT64 | NIFTI_TYPE_COMPLEX64 | NIFTI_TYPE_COMPLEX128
        );
        let pieces = match t {
            NIFTI_TYPE_COMPLEX64 | NIFTI_TYPE_COMPLEX128 => 2,
            NIFTI_TYPE_RGB24 => 3,
            NIFTI_TYPE_UINT8 | NIFTI_TYPE_INT16 | NIFTI_TYPE_INT32 | NIFTI_TYPE_FLOAT32
            | NIFTI_TYPE_FLOAT64 | NIFTI_TYPE_INT8 | NIFTI_TYPE_UINT16 | NIFTI_TYPE_INT64 => 1,
            _ => {
                self.out
                    .err(&format!("** decode_ascii cannot decode type {t}\n"));
                return -1;
            }
        };
        let slot = match t {
            NIFTI_TYPE_COMPLEX64 => 0,
            NIFTI_TYPE_RGB24 => 1,
            _ => 2,
        };
        let mut piece = if pieces > 1 { self.lib.piece[slot] } else { 0 };
        let mut p1 = 0usize;
        let mut prev = 0usize;
        let mut trace = String::new();
        while vals < 0 || vals < *nvals {
            let (dval, lval, used) = if float {
                let (v, u) = strtod(&cdata[p1..]);
                (v, 0i64, u)
            } else {
                let (v, u) = strtol10(&cdata[p1..]);
                (0.0, v, u)
            };
            if used == 0 {
                break;
            }
            prev = p1;
            p1 += used;
            let idx = (base + vals) * pieces as i64 + piece as i64;
            dest.store(t, idx, dval, lval);
            if verb > 6 {
                trace += &match t {
                    NIFTI_TYPE_FLOAT32 | NIFTI_TYPE_FLOAT64 => format!(
                        "  v {} ({})",
                        cfmt::f(dest.load_f(t, idx), 6),
                        cfmt::f(dval, 6)
                    ),
                    NIFTI_TYPE_COMPLEX64 | NIFTI_TYPE_COMPLEX128 => format!(
                        "  v {} ({})",
                        cfmt::f(dest.load_f(t, (base + vals) * 2), 6),
                        cfmt::f(dval, 6)
                    ),
                    NIFTI_TYPE_RGB24 => {
                        format!("  v {} ({lval})", dest.load_i(t, (base + vals) * 3))
                    }
                    _ => format!("  v {} ({lval})", dest.load_i(t, idx)),
                };
            }
            if pieces == 1 || piece == pieces - 1 {
                vals += 1;
            }
            if pieces > 1 {
                piece = (piece + 1) % pieces;
            }
        }
        if pieces > 1 {
            if self.lib.piece[slot] != 0 && !self.fix {
                self.notices
                    .add(&d::FS_83, "an array started in the middle of a value");
            }
            self.lib.piece[slot] = piece;
        }
        if verb > 6 {
            trace.push('\n');
            self.out.err(&trace);
        }
        if vals > 0 {
            *nvals -= vals;
        }
        let mut remain;
        if p1 < cdlen && is_c_space(cdata[p1]) {
            remain = cdlen - p1;
            *mod_prev = 0;
        } else {
            remain = cdlen - prev;
            *mod_prev = 1;
        }
        if whitespace_len(&cdata[cdlen - remain..]) == remain {
            remain = 0;
        }
        if verb > 6 {
            self.out.err(&format!("-- DA: remain = {remain}\n"));
        }
        remain as i32
    }

    /// `apply_da_list_order`: the arrays in the order asked for (repeats copied).
    fn apply_da_list_order(&mut self, orig: &[i32]) -> i32 {
        let list = self.da_list.clone().unwrap_or_default();
        let nda = self.gim.num_da;
        if orig.is_empty() || list.is_empty() || nda <= 0 {
            return 0;
        }
        let mut errors = 0;
        let mut taken = vec![false; nda as usize];
        let mut new: Vec<Option<DataArray>> = Vec::with_capacity(orig.len());
        for &want in orig {
            let Some(oldc) = (0..nda as usize).find(|&o| list.get(o) == Some(&want)) else {
                self.out.err(&format!(
                    "** ADLO: failed to find index {want} in da_list\n"
                ));
                errors += 1;
                new.push(None);
                continue;
            };
            if self.verb > 3 {
                self.out.err(&format!(
                    "++ found orig index {want} at sorted list {oldc}\n"
                ));
            }
            if !taken[oldc] {
                taken[oldc] = true;
                new.push(self.gim.darray[oldc].clone());
            } else {
                let src = self.gim.darray[oldc].clone();
                new.push(src.map(|s| self.lib.copy_data_array(&s, true, self.out)));
            }
        }
        for oldc in 0..nda as usize {
            if !taken[oldc] {
                self.out.err("** ADLO: taken list is not all set\n");
                errors += 1;
            }
        }
        self.gim.num_da = new.len() as i32;
        self.gim.darray = new;
        for c in 0..self.gim.darray.len() {
            if self.gim.darray[c].is_none() {
                self.out.err("** ADLO: copied darray not full\n");
                errors += 1;
            }
        }
        errors
    }
}

/// `GII_B64_decode4`.
fn quad(w: &[u8]) -> (u8, u8, u8) {
    let t = |c: u8| b64_decode(c);
    (
        (t(w[0]) << 2) | (t(w[1]) >> 4),
        (t(w[1]) << 4) | (t(w[2]) >> 2),
        (t(w[2]) << 6) | t(w[3]),
    )
}

/// Where `decode_ascii` stores: an array's bytes (`nbyper` each) or a 4 x 4 matrix.
enum Dest<'a> {
    Bytes(&'a mut Vec<u8>),
    Xform(&'a mut [[f64; 4]; 4]),
}

impl Dest<'_> {
    /// Element `idx` (a piece index for complex and RGB values; -1 and below reach the values
    /// before the destination's start, as C's `ptr[vals]` with `vals == -1` does).
    fn store(&mut self, t: i32, idx: i64, dval: f64, lval: i64) {
        match self {
            Dest::Xform(m) => {
                if (0..16).contains(&idx) {
                    m[idx as usize / 4][idx as usize % 4] = dval;
                }
            }
            Dest::Bytes(v) => {
                let (size, bytes): (usize, Vec<u8>) = match t {
                    NIFTI_TYPE_UINT8 | NIFTI_TYPE_RGB24 => (1, vec![lval as u8]),
                    NIFTI_TYPE_INT8 => (1, vec![lval as i8 as u8]),
                    NIFTI_TYPE_INT16 => (2, (lval as i16).to_le_bytes().to_vec()),
                    NIFTI_TYPE_UINT16 => (2, (lval as u16).to_le_bytes().to_vec()),
                    NIFTI_TYPE_INT32 => (4, (lval as i32).to_le_bytes().to_vec()),
                    NIFTI_TYPE_INT64 => (8, lval.to_le_bytes().to_vec()),
                    NIFTI_TYPE_FLOAT32 | NIFTI_TYPE_COMPLEX64 => {
                        (4, (dval as f32).to_le_bytes().to_vec())
                    }
                    _ => (8, dval.to_le_bytes().to_vec()),
                };
                if idx < 0 {
                    return;
                }
                let off = idx as usize * size;
                if off + size <= v.len() {
                    v[off..off + size].copy_from_slice(&bytes);
                }
            }
        }
    }

    fn load_f(&self, t: i32, idx: i64) -> f64 {
        match self {
            Dest::Xform(m) => {
                if (0..16).contains(&idx) {
                    m[idx as usize / 4][idx as usize % 4]
                } else {
                    0.0
                }
            }
            Dest::Bytes(v) => {
                let size = if matches!(t, NIFTI_TYPE_FLOAT32 | NIFTI_TYPE_COMPLEX64) {
                    4
                } else {
                    8
                };
                let off = (idx.max(0) as usize) * size;
                if off + size > v.len() {
                    return 0.0;
                }
                if size == 4 {
                    f32::from_le_bytes(v[off..off + 4].try_into().expect("4")) as f64
                } else {
                    f64::from_le_bytes(v[off..off + 8].try_into().expect("8"))
                }
            }
        }
    }

    fn load_i(&self, t: i32, idx: i64) -> i64 {
        let Dest::Bytes(v) = self else { return 0 };
        let i = idx.max(0) as usize;
        let g = |n: usize| -> Option<&[u8]> { v.get(i * n..i * n + n) };
        match t {
            NIFTI_TYPE_UINT8 | NIFTI_TYPE_RGB24 => g(1).map_or(0, |b| b[0] as i64),
            NIFTI_TYPE_INT8 => g(1).map_or(0, |b| b[0] as i8 as i64),
            NIFTI_TYPE_INT16 => g(2).map_or(0, |b| i16::from_le_bytes([b[0], b[1]]) as i64),
            NIFTI_TYPE_UINT16 => g(2).map_or(0, |b| u16::from_le_bytes([b[0], b[1]]) as i64),
            NIFTI_TYPE_INT32 => {
                g(4).map_or(0, |b| i32::from_le_bytes(b.try_into().expect("4")) as i64)
            }
            _ => g(8).map_or(0, |b| i64::from_le_bytes(b.try_into().expect("8"))),
        }
    }
}

impl Handler for Reader<'_> {
    fn start_element(&mut self, name: &[u8], attrs: &[(Vec<u8>, Vec<u8>)]) {
        let etype = (1..=E_LAST)
            .rev()
            .find(|&e| ENAMES[e].as_bytes() == name)
            .unwrap_or(0);
        if self.verb > 3 {
            self.show_attrs(etype, attrs);
        }
        let _ = self.epush(etype, name, attrs);
    }

    fn end_element(&mut self, name: &[u8]) {
        let etype = (1..=E_LAST)
            .rev()
            .find(|&e| ENAMES[e].as_bytes() == name)
            .unwrap_or(0);
        let _ = self.epop(etype, name);
    }

    fn characters(&mut self, s: &[u8]) {
        let length = s.len();
        if self.skip > 0 {
            if self.verb > 3 {
                self.out.err(&format!("-- skipping char [{length}]\n"));
            }
            return;
        }
        if self.depth < 1 {
            return;
        }
        let mut parent = self.stack[self.depth as usize - 1];
        if parent == E_CDATA && self.depth >= 2 {
            parent = self.stack[self.depth as usize - 2];
        }
        let wlen = if parent != E_DATA {
            whitespace_len(s)
        } else {
            0
        };
        match parent {
            E_DATA => {
                let _ = self.append_to_data(s);
            }
            E_MATRIXDATA => {
                let _ = self.append_to_xform(s);
            }
            E_GIFTI | E_META | E_MD | E_LABELTABLE | E_DATAARRAY | E_CSTM => {
                if wlen != length && self.verb != 0 {
                    self.out.err(&format!(
                        "** invalid chars under {}: '{}'\n",
                        ENAMES[parent],
                        lossy(s)
                    ));
                }
            }
            E_NAME | E_VALUE | E_LABEL | E_DATASPACE | E_XFORMSPACE => {
                if self.verb > 4 {
                    self.out
                        .err(&format!("++ append cdata, parent {}\n", ENAMES[parent]));
                }
                let _ = self.append_to_cdata(s);
            }
            E_CDATA => {
                self.out.err("** CDATA is the parent of CDATA???\n");
                return;
            }
            _ => {
                self.out
                    .err(&format!("** unknown parent of char: {parent}\n"));
                return;
            }
        }
        let (text, len): (&[u8], usize) = if wlen == length {
            if self.verb < 5 {
                return;
            }
            (b"whitespace", 10)
        } else {
            (s, length)
        };
        if self.verb > 4 {
            let len = if parent == E_DATA && len > 40 {
                40
            } else {
                len
            };
            self.out.err(&format!(
                "{}char[{length}]: {}\n",
                depth_prefix(self.depth, true),
                prec(text, len)
            ));
        }
    }

    fn processing_instruction(&mut self, target: &[u8], data: &[u8]) {
        if self.verb > 3 {
            self.out.err(&format!(
                "{}instr: {}='{}'\n",
                depth_prefix(self.depth, true),
                lossy(target),
                lossy(data)
            ));
        }
    }

    fn comment(&mut self, data: &[u8]) {
        if self.verb > 1 {
            self.out.err(&format!(
                "{}comment: '{}'\n",
                depth_prefix(self.depth, true),
                lossy(data)
            ));
        }
    }

    fn start_cdata(&mut self) {
        if self.verb > 3 {
            self.out
                .err(&format!("{}cdata_start\n", depth_prefix(self.depth, true)));
        }
        let _ = self.epush(E_CDATA, b"CDATA", &[]);
    }

    fn end_cdata(&mut self) {
        let _ = self.epop(E_CDATA, b"CDATA");
    }

    fn default(&mut self, s: &[u8]) {
        let length = s.len();
        let (text, len): (&[u8], usize) = if whitespace_len(s) == length {
            if self.verb < 4 {
                return;
            }
            (b"whitespace", 10)
        } else {
            (s, length)
        };
        if self.verb > 3 {
            self.out.err(&format!(
                "{}default XML element [{length}]: '{}'\n",
                depth_prefix(self.depth, true),
                prec(text, len)
            ));
        }
    }

    fn xml_decl(&mut self, version: Option<&[u8]>, encoding: Option<&[u8]>, standalone: i32) {
        if self.verb > 2 {
            let s = |v: Option<&[u8]>| v.map_or("(null)".to_owned(), lossy);
            self.out.err(&format!(
                "{}xmldec ver = {}, enc = {}, standalone = {standalone}\n",
                depth_prefix(self.depth, true),
                s(version),
                s(encoding)
            ));
        }
    }

    fn start_doctype(
        &mut self,
        name: &[u8],
        sysid: Option<&[u8]>,
        pubid: Option<&[u8]>,
        has_internal_subset: bool,
    ) {
        if self.verb > 2 {
            let s = |v: Option<&[u8]>| v.map_or("NULL".to_owned(), lossy);
            self.out.err(&format!(
                "{}start_doctype, dt='{}', sid='{}',pid='{}', sub={}\n",
                depth_prefix(self.depth, true),
                lossy(name),
                s(sysid),
                s(pubid),
                has_internal_subset as i32
            ));
        }
    }

    fn end_doctype(&mut self) {
        if self.verb > 2 {
            self.out
                .err(&format!("{}end_doctype\n", depth_prefix(self.depth, true)));
        }
    }

    fn element_decl(&mut self, name: &[u8], model: &Content) {
        if self.verb > 2 {
            // gifticlib prints the children's address; larmorx a fixed placeholder.
            self.out.err(&format!(
                "{}{}: type={}, quant={}, name={}, numc={}, cp={}\n",
                depth_prefix(self.depth, true),
                lossy(name),
                model.kind,
                model.quant,
                model.name.as_deref().map_or("(null)".to_owned(), lossy),
                model.children.len(),
                if model.children.is_empty() {
                    "(nil)"
                } else {
                    "<children>"
                }
            ));
        }
    }
}

impl Gifti {
    /// `gifti_read_extern_DA_data`: 0 on success.
    pub fn read_extern_da_data(&self, da: &mut DataArray, fix: bool, out: &mut dyn Streams) -> i32 {
        let Some(name) = da.ext_fname.clone().filter(|n| !n.is_empty()) else {
            return 0;
        };
        let n = lossy(&name);
        if self.verb > 4 {
            out.err(&format!("-- external read of '{n}'\n"));
        }
        if da.ext_offset < 0 {
            out.err(&format!(
                "** want external DA data with bad offset {}\n",
                da.ext_offset
            ));
            return 1;
        } else if da.data.is_some() {
            out.err("** want external DA data but data already allocated\n");
            return 1;
        } else if !self.valid_dims(da, true, fix, out) {
            out.err("** cannot read external DA data with bad dims...\n");
            return 1;
        }
        let nbytes = da.nvals.wrapping_mul(da.nbyper as i64);
        let mut data = vec![0u8; nbytes.max(0) as usize];
        let path = std::path::PathBuf::from(n.clone());
        let mut f = match std::fs::File::open(&path) {
            Ok(f) => f,
            Err(_) => {
                da.data = Some(data);
                out.err(&format!("** ext read: failed to open '{n}'\n"));
                return 1;
            }
        };
        use std::io::{Seek, SeekFrom};
        if f.seek(SeekFrom::Start(da.ext_offset as u64)).is_err() {
            da.data = Some(data);
            out.err(&format!(
                "** ext read: failed to seek to {} in '{n}'\n",
                da.ext_offset
            ));
            return 1;
        }
        let nread = read_full(&mut f, &mut data) as i64;
        da.data = Some(data);
        if nread != nbytes {
            out.err(&format!(
                "** ext_read: read only {nread} of {nbytes} bytes from {n}\n"
            ));
            return 1;
        }
        if self.verb > 2 {
            out.err(&format!(
                "-- read {nbytes} bytes from external '{n}' @ {}\n",
                da.ext_offset
            ));
        }
        0
    }
}
