// SPDX-License-Identifier: Apache-2.0
//! GIFTI files as gifticlib 1.12 (Richard Reynolds, NIH/NIMH; public domain) reads and writes them:
//! the library's structures, its checks, copies and displays (`gifti_io.c`), its reader
//! ([`read`], `gifti_xml.c`'s element callbacks over [`crate::xml`], the pure-Rust parser in
//! expat 2.0.1's place) and its writer ([`write`], byte for byte).
//!
//! The library's settings and its few persistent counters are one [`Gifti`] value (the C library
//! keeps them in two globals); every message goes to a [`Streams`], at the verbosity the C code
//! prints it. Data are kept as gifticlib keeps them on a little-endian machine: the values' bytes
//! in little-endian order, `long double` as x86-64's 80-bit format in 16 bytes.
//!
//! [`Dialect`]: the two points where a program built on gifticlib may change it (the version
//! check of [`Gifti::valid_gifti_image`] and the coordinate-system intent check of the reader);
//! [`Gifticlib`] is gifticlib's own behaviour. A replica of such a program supplies its own.
//!
//! D13's corrections of gifticlib's defects are named in [`defects`] (FS-80 to FS-87); the
//! source is `gifti_io.c` and `gifti_xml.c` of gifticlib's history at version 1.12
//! (`PROVENANCE.md`).

// gifticlib's functions return C's status integers; the index loops are the C code's.
#![allow(clippy::needless_range_loop, clippy::too_many_arguments)]

pub mod read;
#[cfg(test)]
mod tests;
pub mod write;

use larmorx_core::cfmt::{self, Streams};
use larmorx_core::defects::{self as d, Notices};

/// D13 defects of gifticlib 1.12 that larmorx corrects (`docs/findings/gifti.md`; their register
/// entries are `larmorx_core::defects::FS_80` to `FS_84`).
pub mod defects {
    /// A label table whose `Label`s carry colours for some labels only (the colour array is
    /// shorter than the table or has holes), or a `Label` whose `Key` is missing or wrong (its
    /// key is never set): the writer, the checks and the display read memory never written.
    /// Both modes: those colours and keys read as 0, with a notice.
    pub const LABEL_RGBA_HOLES: &str = "FS-80";
    /// Base64 text holding characters outside the alphabet (skipped, b64_check's default) whose
    /// last group of four is cut at the end of a piece of character data: the bytes carried to
    /// the next piece are taken from the wrong place. Corrected: the real remainder is carried.
    pub const B64_CARRY: &str = "FS-81";
    /// `NIFTI_TYPE_FLOAT128`'s size is 6 bytes in gifticlib's type table (a `long double` takes
    /// 16): the arrays are allocated and decoded with 6 bytes per value, and the ASCII writer
    /// reads 16. Corrected: 16 bytes.
    pub const FLOAT128_SIZE: &str = "FS-82";
    /// The ASCII decoder's counters of a value's pieces (COMPLEX64, COMPLEX128, RGB24) persist
    /// from one array to the next, even of another file: an array whose last value is incomplete
    /// shifts the next one. Corrected: each `Data` element starts at a value's first piece.
    pub const ASCII_PIECE: &str = "FS-83";
    /// A metadata entry without a name (an `MD` with no `Name` text) is compared through a null
    /// pointer by `gifti_get_meta_value` and `gifti_add_to_meta`: the program crashes. Both
    /// modes: the entry does not match, with a notice.
    pub const META_NULL_NAME: &str = "FS-84";
    /// Character data (white space included) after a CDATA section or a child element inside a
    /// `Name`, `Value`, `Label`, `DataSpace` or `TransformedSpace`: gifticlib writes it through a
    /// null pointer (the program crashes). Both modes: the text is appended to the element's, as
    /// it is without the CDATA section, with a notice.
    pub const TEXT_AFTER_CDATA: &str = "FS-86";
    /// The `MatrixData` reader's messages at verbosity 6 and 7 show the `Data` element's buffer
    /// and its length instead of the matrix's own (memory never written when that buffer is
    /// new). Corrected: the matrix buffer's; reproduced: the `Data` buffer's, its never-written
    /// bytes from zeros, with a notice.
    pub const XFORM_MESSAGE_BUFFER: &str = "FS-87";
}

// NIfTI-1's codes (nifti1.h).
pub const DT_UNKNOWN: i32 = 0;
pub const NIFTI_TYPE_UINT8: i32 = 2;
pub const NIFTI_TYPE_INT16: i32 = 4;
pub const NIFTI_TYPE_INT32: i32 = 8;
pub const NIFTI_TYPE_FLOAT32: i32 = 16;
pub const NIFTI_TYPE_COMPLEX64: i32 = 32;
pub const NIFTI_TYPE_FLOAT64: i32 = 64;
pub const NIFTI_TYPE_RGB24: i32 = 128;
pub const NIFTI_TYPE_INT8: i32 = 256;
pub const NIFTI_TYPE_UINT16: i32 = 512;
pub const NIFTI_TYPE_UINT32: i32 = 768;
pub const NIFTI_TYPE_INT64: i32 = 1024;
pub const NIFTI_TYPE_UINT64: i32 = 1280;
pub const NIFTI_TYPE_FLOAT128: i32 = 1536;
pub const NIFTI_TYPE_COMPLEX128: i32 = 1792;
pub const NIFTI_TYPE_COMPLEX256: i32 = 2048;
pub const NIFTI_INTENT_NONE: i32 = 0;
pub const NIFTI_INTENT_POINTSET: i32 = 1008;
pub const NIFTI_INTENT_TRIANGLE: i32 = 1009;
pub const NIFTI_INTENT_TIME_SERIES: i32 = 2001;

pub const GIFTI_IND_ORD_UNDEF: i32 = 0;
pub const GIFTI_IND_ORD_ROW_MAJOR: i32 = 1;
pub const GIFTI_IND_ORD_COL_MAJOR: i32 = 2;
pub const GIFTI_IND_ORD_MAX: i32 = 2;
pub const GIFTI_ENCODING_UNDEF: i32 = 0;
pub const GIFTI_ENCODING_ASCII: i32 = 1;
pub const GIFTI_ENCODING_B64BIN: i32 = 2;
pub const GIFTI_ENCODING_B64GZ: i32 = 3;
pub const GIFTI_ENCODING_EXTBIN: i32 = 4;
pub const GIFTI_ENCODING_MAX: i32 = 4;
pub const GIFTI_ENDIAN_UNDEF: i32 = 0;
pub const GIFTI_ENDIAN_BIG: i32 = 1;
pub const GIFTI_ENDIAN_LITTLE: i32 = 2;
pub const GIFTI_ENDIAN_MAX: i32 = 2;
pub const GIFTI_B64_CHECK_UNDEF: i32 = 0;
pub const GIFTI_B64_CHECK_NONE: i32 = 1;
pub const GIFTI_B64_CHECK_DETECT: i32 = 2;
pub const GIFTI_B64_CHECK_COUNT: i32 = 3;
pub const GIFTI_B64_CHECK_SKIP: i32 = 4;
pub const GIFTI_B64_CHECK_SKIPNCOUNT: i32 = 5;
pub const GIFTI_B64_CHECK_MAX: i32 = 5;
pub const GIFTI_DARRAY_DIM_LEN: i32 = 6;
/// zlib's `Z_DEFAULT_COMPRESSION` (gifticlib is built with zlib).
pub const GZ_DEFAULT_COMPRESSION: i32 = -1;
pub const GIFTI_XML_VERSION: &str = "1.0";
pub const GIFTI_XML_ENCODING: &str = "UTF-8";
pub const GIFTI_XML_DTD_SOURCE: &str = "http://gifti.projects.nitrc.org/gifti.dtd";
pub const GXML_DEF_BSIZE: i32 = 32768;
pub const GXML_MIN_BSIZE: i32 = 2048;

/// gifticlib's version string (`gifticlib_version`).
pub const GIFTI_VERSION: &str = "gifti library version 1.12, 15 June, 2012";

pub const INDEX_ORDER_LIST: [&str; 3] = ["Undefined", "RowMajorOrder", "ColumnMajorOrder"];
pub const ENCODING_LIST: [&str; 5] = [
    "Undefined",
    "ASCII",
    "Base64Binary",
    "GZipBase64Binary",
    "ExternalFileBinary",
];
pub const ENDIAN_LIST: [&str; 3] = ["Undefined", "BigEndian", "LittleEndian"];

/// One of gifticlib's three string lists (`gifti_list_index2string`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum List {
    IndexOrder,
    Encoding,
    Endian,
}

impl List {
    fn items(self) -> &'static [&'static str] {
        match self {
            List::IndexOrder => &INDEX_ORDER_LIST,
            List::Encoding => &ENCODING_LIST,
            List::Endian => &ENDIAN_LIST,
        }
    }
}

/// `gifti_type_list`: code, bytes per value, bytes per swapped piece, name (FLOAT128's 6 bytes
/// as gifticlib's table has them: FS-82).
const TYPE_LIST: [(i32, i32, i32, &str); 16] = [
    (DT_UNKNOWN, 0, 0, "Undefined"),
    (NIFTI_TYPE_UINT8, 1, 0, "NIFTI_TYPE_UINT8"),
    (NIFTI_TYPE_INT16, 2, 2, "NIFTI_TYPE_INT16"),
    (NIFTI_TYPE_INT32, 4, 4, "NIFTI_TYPE_INT32"),
    (NIFTI_TYPE_FLOAT32, 4, 4, "NIFTI_TYPE_FLOAT32"),
    (NIFTI_TYPE_COMPLEX64, 8, 4, "NIFTI_TYPE_COMPLEX64"),
    (NIFTI_TYPE_FLOAT64, 8, 8, "NIFTI_TYPE_FLOAT64"),
    (NIFTI_TYPE_RGB24, 3, 0, "NIFTI_TYPE_RGB24"),
    (NIFTI_TYPE_INT8, 1, 0, "NIFTI_TYPE_INT8"),
    (NIFTI_TYPE_UINT16, 2, 2, "NIFTI_TYPE_UINT16"),
    (NIFTI_TYPE_UINT32, 4, 4, "NIFTI_TYPE_UINT32"),
    (NIFTI_TYPE_INT64, 8, 8, "NIFTI_TYPE_INT64"),
    (NIFTI_TYPE_UINT64, 8, 8, "NIFTI_TYPE_UINT64"),
    (NIFTI_TYPE_FLOAT128, 6, 16, "NIFTI_TYPE_FLOAT128"),
    (NIFTI_TYPE_COMPLEX128, 16, 8, "NIFTI_TYPE_COMPLEX128"),
    (NIFTI_TYPE_COMPLEX256, 32, 16, "NIFTI_TYPE_COMPLEX256"),
];

/// `gifti_intent_list`.
const INTENT_LIST: [(i32, &str); 40] = [
    (0, "NIFTI_INTENT_NONE"),
    (2, "NIFTI_INTENT_CORREL"),
    (3, "NIFTI_INTENT_TTEST"),
    (4, "NIFTI_INTENT_FTEST"),
    (5, "NIFTI_INTENT_ZSCORE"),
    (6, "NIFTI_INTENT_CHISQ"),
    (7, "NIFTI_INTENT_BETA"),
    (8, "NIFTI_INTENT_BINOM"),
    (9, "NIFTI_INTENT_GAMMA"),
    (10, "NIFTI_INTENT_POISSON"),
    (11, "NIFTI_INTENT_NORMAL"),
    (12, "NIFTI_INTENT_FTEST_NONC"),
    (13, "NIFTI_INTENT_CHISQ_NONC"),
    (14, "NIFTI_INTENT_LOGISTIC"),
    (15, "NIFTI_INTENT_LAPLACE"),
    (16, "NIFTI_INTENT_UNIFORM"),
    (17, "NIFTI_INTENT_TTEST_NONC"),
    (18, "NIFTI_INTENT_WEIBULL"),
    (19, "NIFTI_INTENT_CHI"),
    (20, "NIFTI_INTENT_INVGAUSS"),
    (21, "NIFTI_INTENT_EXTVAL"),
    (22, "NIFTI_INTENT_PVAL"),
    (23, "NIFTI_INTENT_LOGPVAL"),
    (24, "NIFTI_INTENT_LOG10PVAL"),
    (1001, "NIFTI_INTENT_ESTIMATE"),
    (1002, "NIFTI_INTENT_LABEL"),
    (1003, "NIFTI_INTENT_NEURONAME"),
    (1004, "NIFTI_INTENT_GENMATRIX"),
    (1005, "NIFTI_INTENT_SYMMATRIX"),
    (1006, "NIFTI_INTENT_DISPVECT"),
    (1007, "NIFTI_INTENT_VECTOR"),
    (1008, "NIFTI_INTENT_POINTSET"),
    (1009, "NIFTI_INTENT_TRIANGLE"),
    (1010, "NIFTI_INTENT_QUATERNION"),
    (1011, "NIFTI_INTENT_DIMLESS"),
    (2001, "NIFTI_INTENT_TIME_SERIES"),
    (2002, "NIFTI_INTENT_NODE_INDEX"),
    (2003, "NIFTI_INTENT_RGB_VECTOR"),
    (2004, "NIFTI_INTENT_RGBA_VECTOR"),
    (2005, "NIFTI_INTENT_SHAPE"),
];

/// A C string of the library (`char *`): `None` is the null pointer. The bytes never hold a 0.
pub type CStr = Option<Vec<u8>>;

/// `nvpairs` (and `giiMetaData`): parallel names and values.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NvPairs {
    pub name: Vec<CStr>,
    pub value: Vec<CStr>,
}

impl NvPairs {
    pub fn len(&self) -> usize {
        self.name.len()
    }
    pub fn is_empty(&self) -> bool {
        self.name.is_empty()
    }
}

/// `giiLabelTable`. `rgba` holds 4 values per label from the first label that had them; a label
/// that had none after that leaves a hole (`None`, memory gifticlib never wrote: FS-80).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LabelTable {
    pub key: Vec<i32>,
    pub label: Vec<CStr>,
    pub rgba: Option<Vec<Option<f32>>>,
}

impl LabelTable {
    pub fn len(&self) -> usize {
        self.key.len()
    }
    pub fn is_empty(&self) -> bool {
        self.key.is_empty()
    }
}

/// `giiCoordSystem`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CoordSystem {
    pub dataspace: CStr,
    pub xformspace: CStr,
    pub xform: [[f64; 4]; 4],
}

/// `giiDataArray`. `data` holds the values' bytes in little-endian order (`nvals * nbyper`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DataArray {
    pub intent: i32,
    pub datatype: i32,
    pub ind_ord: i32,
    pub num_dim: i32,
    pub dims: [i32; 6],
    pub encoding: i32,
    pub endian: i32,
    pub ext_fname: CStr,
    pub ext_offset: i64,
    pub meta: NvPairs,
    pub coordsys: Vec<Option<CoordSystem>>,
    pub data: Option<Vec<u8>>,
    pub nvals: i64,
    pub nbyper: i32,
    pub ex_atrs: NvPairs,
}

impl DataArray {
    pub fn num_cs(&self) -> i32 {
        self.coordsys.len() as i32
    }
}

/// `gifti_image`. `num_da` is the C struct's `numDA` (it equals `darray.len()` except while the
/// reader holds the `NumberOfDataArrays` attribute in it).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GiftiImage {
    pub num_da: i32,
    pub version: CStr,
    pub meta: NvPairs,
    pub labeltable: LabelTable,
    pub darray: Vec<Option<DataArray>>,
    pub swapped: i32,
    pub compressed: i32,
    pub ex_atrs: NvPairs,
}

/// The points where a program built on gifticlib changes it.
pub trait Dialect {
    /// The version check of [`Gifti::valid_gifti_image`]: the number of errors it adds.
    fn check_version(&self, gim: &GiftiImage, whine: bool, out: &mut dyn Streams) -> i32 {
        match &gim.version {
            Some(v) if v.as_slice() == GIFTI_XML_VERSION.as_bytes() => 0,
            v => {
                if whine {
                    out.err(&format!("** invalid: gim version = {}\n", show(v)));
                }
                1
            }
        }
    }
    /// The reader's check of a `CoordinateSystemTransformMatrix`'s array: its message, or `None`.
    fn coordsys_intent_message(&self, da_index: i32, intent: i32) -> Option<String> {
        (intent != NIFTI_INTENT_POINTSET).then(|| {
            format!(
                "** DA[{da_index}] has coordsys with intent {} (should be {})\n",
                intent_to_string(intent),
                intent_to_string(NIFTI_INTENT_POINTSET)
            )
        })
    }
}

/// gifticlib 1.12's own behaviour.
pub struct Gifticlib;
impl Dialect for Gifticlib {}

/// `G_CHECK_NULL_STR`: the string, or `NULL`.
pub fn show(s: &CStr) -> String {
    match s {
        Some(b) => String::from_utf8_lossy(b).into_owned(),
        None => "NULL".to_owned(),
    }
}

fn lossy(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

/// A string as the library copies it (`gifti_strdup`): up to its first 0.
pub fn cstr(s: &[u8]) -> Vec<u8> {
    s.iter().copied().take_while(|&b| b != 0).collect()
}

/// gifticlib's settings and persistent state: `G` (`gifti_io.c`) and `GXD`'s user variables
/// (`gifti_xml.c`), with the counters C keeps in function statics.
#[derive(Clone, Debug)]
pub struct Gifti {
    /// `G.verb`, `gifti_get_verb` / `gifti_set_verb`.
    pub verb: i32,
    /// `GXD.verb` (set from `G.verb` by every read and write).
    pub xml_verb: i32,
    pub dstore: i32,
    pub indent: i32,
    /// `GXD.buf_size`: the reader's buffer, which reading a `DataArray` changes for later reads.
    pub buf_size: i32,
    pub b64_check: i32,
    pub update_ok: i32,
    pub zlevel: i32,
    /// `append_to_data_ascii`'s and `append_to_xform`'s `static mod_prev`.
    pub(crate) mod_prev_data: i32,
    pub(crate) mod_prev_xform: i32,
    /// `decode_ascii`'s `static piece` for COMPLEX64, RGB24 and COMPLEX128 (FS-83).
    pub(crate) piece: [i32; 3],
    /// What the last read or write left in `GXD`'s other fields (shown at verbosity 4 and above
    /// by the next read's or write's `disp_gxml_data`).
    pub(crate) gxd: GxdState,
}

/// `GXD`'s fields a read or write leaves behind (its buffers are always freed by then).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GxdState {
    pub ele_da: i32,
    pub exp_da: i32,
    pub b64_errors: i32,
    pub errors: i32,
    pub skip: i32,
    pub depth: i32,
    pub dind: i64,
    pub clen: i64,
    pub doff: i64,
    pub zlen: i64,
    pub cdata: bool,
    pub gim: bool,
}

impl GxdState {
    /// The `show_all` lines of `disp_gxml_data` after `da_list` (pointers as placeholders).
    pub(crate) fn lines(&self, da_ind: i64) -> String {
        let p = |set: bool, name: &str| {
            if set {
                format!("<{name}>")
            } else {
                "(nil)".to_owned()
            }
        };
        format!(
            "   da_ind      : {da_ind}\n   eleDA       : {}\n   expDA       : {}\n   b64_errors  : {}\n   errors      : {}\n   skip        : {}\n   depth       : {}\n   dind        : {}\n   clen        : {}\n   doff        : {}\n   zlen        : {}\n   cdata       : {}\n   xdata       : (nil)\n   ddata       : (nil)\n   zdata       : (nil)\n   gim         : {}\n",
            self.ele_da,
            self.exp_da,
            self.b64_errors,
            self.errors,
            self.skip,
            self.depth,
            self.dind,
            self.clen,
            self.doff,
            self.zlen,
            p(self.cdata, "cdata"),
            p(self.gim, "gim")
        )
    }
}

impl Default for Gifti {
    fn default() -> Self {
        Gifti {
            verb: 1,
            xml_verb: 1,
            dstore: 1,
            indent: 3,
            buf_size: GXML_DEF_BSIZE,
            b64_check: GIFTI_B64_CHECK_SKIPNCOUNT,
            update_ok: 1,
            zlevel: GZ_DEFAULT_COMPRESSION,
            mod_prev_data: 0,
            mod_prev_xform: 0,
            piece: [0; 3],
            gxd: GxdState::default(),
        }
    }
}

/// `gifti_list_index2string` for one of the three lists.
pub fn list_index2string(list: List, index: i32, verb: i32, out: &mut dyn Streams) -> &'static str {
    let items = list.items();
    if index < 0 || index as usize >= items.len() {
        if verb > 0 {
            out.err(&format!(
                "** GLI2S: index {index} out of range {{0..{}}}\n",
                items.len() - 1
            ));
        }
        return "INDEX OUT OF RANGE";
    }
    items[index as usize]
}

/// `gifti_datatype2str`.
pub fn datatype2str(t: i32) -> &'static str {
    let c = (1..TYPE_LIST.len())
        .rev()
        .find(|&c| TYPE_LIST[c].0 == t)
        .unwrap_or(0);
    TYPE_LIST[c].3
}

/// `gifti_str2datatype`.
pub fn str2datatype(s: &[u8]) -> i32 {
    let c = (1..TYPE_LIST.len())
        .rev()
        .find(|&c| TYPE_LIST[c].3.as_bytes() == s)
        .unwrap_or(0);
    TYPE_LIST[c].0
}

/// `gifti_intent_to_string`.
pub fn intent_to_string(code: i32) -> &'static str {
    let c = (1..INTENT_LIST.len())
        .rev()
        .find(|&c| INTENT_LIST[c].0 == code)
        .unwrap_or(0);
    INTENT_LIST[c].1
}

/// `gifti_intent_from_string`.
pub fn intent_from_string(name: &[u8]) -> i32 {
    let c = (1..INTENT_LIST.len())
        .rev()
        .find(|&c| INTENT_LIST[c].1.as_bytes() == name)
        .unwrap_or(0);
    INTENT_LIST[c].0
}

/// `gifti_intent_is_valid`: true for every code (its search's result is never negative; a
/// finding kept as gifticlib has it, `docs/findings/gifti.md`).
pub fn intent_is_valid(_code: i32) -> bool {
    true
}

/// `gifti_get_this_endian` on the machines gifticlib's results are recorded on.
pub fn this_endian() -> i32 {
    GIFTI_ENDIAN_LITTLE
}

/// `gifti_clear_float_zeros`: the trailing zeros (and blanks) after a decimal point removed,
/// one digit kept after the point.
pub fn clear_float_zeros(s: &mut String) -> bool {
    let Some(dp) = s.find('.') else { return false };
    let b = s.as_bytes();
    let mut v = b.len() - 1;
    while v > dp + 1 && (b[v] == b' ' || b[v] == b'0') {
        v -= 1;
    }
    let changed = v < b.len() - 1;
    s.truncate(v + 1);
    changed
}

/// `atoi`/`atol` (glibc: `strtol(s, NULL, 10)`).
pub(crate) fn atol(s: &[u8]) -> i64 {
    larmorx_core::cscan::strtol10(s).0
}

impl Gifti {
    pub fn new() -> Gifti {
        Gifti::default()
    }

    /// `gifti_set_verb`.
    pub fn set_verb(&mut self, level: i32) {
        self.verb = level;
    }
    /// `gxml_set_verb`.
    pub fn xml_set_verb(&mut self, val: i32) {
        if val == -1 {
            self.xml_verb = 1;
        } else if val >= 0 {
            self.xml_verb = val;
        }
    }
    /// `gxml_set_dstore`.
    pub fn set_dstore(&mut self, val: i32) {
        self.dstore = (val != 0) as i32;
    }
    /// `gifti_set_indent` (`gxml_set_indent`): 1 on failure.
    pub fn set_indent(&mut self, val: i32) -> i32 {
        match val {
            -1 => self.indent = 3,
            v if v >= 0 => self.indent = v,
            _ => return 1,
        }
        0
    }
    /// `gifti_set_xml_buf_size`.
    pub fn set_xml_buf_size(&mut self, val: i32) -> i32 {
        match val {
            -1 => self.buf_size = GXML_DEF_BSIZE,
            v if v > 0 => self.buf_size = v,
            _ => return 1,
        }
        0
    }
    /// `gifti_set_b64_check`.
    pub fn set_b64_check(&mut self, val: i32) -> i32 {
        if val == -1 {
            self.b64_check = GIFTI_B64_CHECK_SKIPNCOUNT;
        } else if val > GIFTI_B64_CHECK_UNDEF && val <= GIFTI_B64_CHECK_MAX {
            self.b64_check = val;
        } else {
            return 1;
        }
        0
    }
    /// `gifti_set_update_ok`.
    pub fn set_update_ok(&mut self, val: i32) -> i32 {
        match val {
            -1 => self.update_ok = 1,
            v if v >= 0 => self.update_ok = v,
            _ => return 1,
        }
        0
    }
    /// `gifti_set_zlevel`.
    pub fn set_zlevel(&mut self, level: i32, out: &mut dyn Streams) -> i32 {
        if level != GZ_DEFAULT_COMPRESSION && !(0..=9).contains(&level) {
            out.err(&format!(
                "** invalid zlevel, must be {GZ_DEFAULT_COMPRESSION} (default) or {{0..9}}\n"
            ));
            return 1;
        }
        self.zlevel = level;
        0
    }
    /// `gifti_reset_user_vars`.
    pub fn reset_user_vars(&mut self) {
        self.xml_set_verb(-1);
        self.dstore = 1;
        self.indent = 3;
        self.buf_size = GXML_DEF_BSIZE;
        self.b64_check = GIFTI_B64_CHECK_SKIPNCOUNT;
        self.update_ok = 1;
        self.zlevel = GZ_DEFAULT_COMPRESSION;
    }

    /// `gifti_datatype_sizes`: (nbyper, swapsize), or `None` (with gifticlib's message) for an
    /// unknown type. FLOAT128 is 16 bytes in larmorx's corrected mode (FS-82).
    pub fn datatype_sizes(&self, t: i32, fix: bool, out: &mut dyn Streams) -> Option<(i32, i32)> {
        if let Some(c) = (1..TYPE_LIST.len()).rev().find(|&c| TYPE_LIST[c].0 == t) {
            let n = if t == NIFTI_TYPE_FLOAT128 && fix {
                16
            } else {
                TYPE_LIST[c].1
            };
            return Some((n, TYPE_LIST[c].2));
        }
        if self.verb > 0 {
            out.err(&format!("** GDS with bad datatype {t}\n"));
        }
        None
    }

    /// `gifti_valid_datatype`.
    pub fn valid_datatype(&self, t: i32, whine: bool, out: &mut dyn Streams) -> bool {
        if (1..TYPE_LIST.len()).any(|c| TYPE_LIST[c].0 == t) {
            return true;
        }
        if whine || self.verb > 3 {
            out.err(&format!("** invalid datatype value {t}\n"));
        }
        false
    }

    /// `gifti_valid_nbyper` (with FLOAT128's corrected size accepted in the corrected mode).
    pub fn valid_nbyper(&self, n: i32, whine: bool, fix: bool, out: &mut dyn Streams) -> bool {
        if (1..TYPE_LIST.len()).any(|c| TYPE_LIST[c].1 == n) || (fix && n == 16) {
            return true;
        }
        if whine || self.verb > 3 {
            out.err(&format!("** invalid nbyper value {n}\n"));
        }
        false
    }

    /// `gifti_valid_num_dim`.
    pub fn valid_num_dim(&self, num_dim: i32, whine: bool, out: &mut dyn Streams) -> bool {
        if num_dim <= 0 || num_dim > GIFTI_DARRAY_DIM_LEN {
            if self.verb > 3 || whine {
                out.err(&format!("** invalid num_dim = {num_dim}\n"));
            }
            return false;
        }
        true
    }

    /// `gifti_darray_nvals`.
    pub fn darray_nvals(&self, da: &DataArray, out: &mut dyn Streams) -> i64 {
        if !self.valid_num_dim(da.num_dim, false, out) {
            out.err(&format!(
                "** giiDataArray has illegal num_dim = {}\n",
                da.num_dim
            ));
            return 0;
        }
        let mut n: i64 = 1;
        for c in 0..da.num_dim as usize {
            n = n.wrapping_mul(da.dims[c] as i64);
        }
        if n <= 0 {
            self.disp_data_array(Some("** bad Dim list in "), Some(da), false, out);
            return 0;
        }
        n
    }

    /// `gifti_valid_dims`.
    pub fn valid_dims(
        &self,
        da: &DataArray,
        whine: bool,
        fix: bool,
        out: &mut dyn Streams,
    ) -> bool {
        if !self.valid_num_dim(da.num_dim, whine, out) {
            return false;
        }
        let mut vals: i64 = 1;
        for c in 0..da.num_dim as usize {
            if da.dims[c] <= 0 {
                if self.verb > 3 || whine {
                    out.err(&format!("** invalid dims[{c}] = {}\n", da.dims[c]));
                }
                return false;
            }
            vals = vals.wrapping_mul(da.dims[c] as i64);
        }
        if vals != da.nvals {
            if self.verb > 3 {
                out.err(&format!(
                    "** nvals = {} does not match {vals} for dims[{}]: ",
                    da.nvals, da.num_dim
                ));
                let dims: Vec<u8> = da.dims[..da.num_dim as usize]
                    .iter()
                    .flat_map(|v| v.to_le_bytes())
                    .collect();
                self.disp_raw_data(&dims, NIFTI_TYPE_INT32, da.num_dim, true, out, true);
            }
            return false;
        }
        let nbyper = self
            .datatype_sizes(da.datatype, fix, out)
            .map_or(0, |s| s.0);
        if nbyper != da.nbyper {
            out.err(&format!(
                "** nbyper {} not correct for type {}\n",
                da.nbyper,
                datatype2str(da.datatype)
            ));
            return false;
        }
        if da.num_dim > 1 && da.dims[da.num_dim as usize - 1] < 2 && whine {
            out.err(&format!(
                "** num_dim violation: num_dim = {}, yet dim[{}] = {}\n",
                da.num_dim,
                da.num_dim - 1,
                da.dims[da.num_dim as usize - 1]
            ));
        }
        true
    }

    /// `gifti_valid_nvpairs`.
    pub fn valid_nvpairs(&self, nvp: &NvPairs, whine: bool, out: &mut dyn Streams) -> bool {
        for c in 0..nvp.len() {
            let Some(name) = &nvp.name[c] else {
                if self.verb > 5 || whine {
                    out.err(&format!("** invalid nvpair, missing name @ {c}\n"));
                }
                return false;
            };
            if nvp.value[c].is_none() && self.verb > 3 {
                out.err(&format!(
                    "-- missing nvpair value[{c}], name {} (is OK)\n",
                    lossy(name)
                ));
            }
        }
        true
    }

    /// `gifti_valid_LabelTable` (holes in the colours read as 0: FS-80).
    pub fn valid_label_table(
        &self,
        t: &LabelTable,
        whine: bool,
        out: &mut dyn Streams,
        notices: &mut Notices,
    ) -> bool {
        for c in 0..t.len() {
            if t.label[c].is_none() {
                if self.verb > 3 || whine {
                    out.err(&format!("** invalid nvpair label[{c}]\n"));
                }
                return false;
            }
            if t.rgba.is_some() {
                let v = rgba_of(t, c, notices);
                // C's `rgba < 0.0 || rgba > 1.0`: a NaN passes
                if v.iter().any(|&x| x < 0.0 || x > 1.0) {
                    if self.verb > 3 || whine {
                        out.err(&format!("** RGBA values out of [0.0,1,0] at Label[{c}]\n"));
                    }
                    return false;
                }
            }
        }
        true
    }

    /// `gifti_valid_DataArray`.
    pub fn valid_data_array(
        &self,
        da: Option<&DataArray>,
        whine: bool,
        fix: bool,
        out: &mut dyn Streams,
    ) -> bool {
        let Some(da) = da else {
            if whine || self.verb > 2 {
                out.err("** invalid darray pointer\n");
            }
            return false;
        };
        let mut errs = 0;
        if !self.valid_datatype(da.datatype, whine, out) {
            errs += 1;
        }
        if da.ind_ord <= GIFTI_IND_ORD_UNDEF || da.ind_ord > GIFTI_IND_ORD_MAX {
            if whine || self.verb > 3 {
                out.err(&format!("** invalid darray ind_ord = {}\n", da.ind_ord));
            }
            errs += 1;
        }
        if !self.valid_num_dim(da.num_dim, whine, out) {
            errs += 1;
        }
        if !self.valid_dims(da, whine, fix, out) {
            errs += 1;
        }
        if da.encoding <= GIFTI_ENCODING_UNDEF || da.encoding > GIFTI_ENCODING_MAX {
            if whine || self.verb > 3 {
                out.err(&format!("** invalid darray encoding = {}\n", da.encoding));
            }
            errs += 1;
        }
        if da.endian <= GIFTI_ENDIAN_UNDEF || da.endian > GIFTI_ENDIAN_MAX {
            if whine || self.verb > 3 {
                out.err(&format!("** invalid darray endian = {}\n", da.endian));
            }
            errs += 1;
        }
        if !self.valid_nvpairs(&da.meta, whine, out) {
            errs += 1;
        }
        if da.nvals <= 0 {
            if whine || self.verb > 3 {
                out.err(&format!("** invalid darray nvals = {}\n", da.nvals as u32));
            }
            errs += 1;
        }
        if !self.valid_nbyper(da.nbyper, whine, fix, out) {
            errs += 1;
        }
        if !self.valid_nvpairs(&da.ex_atrs, whine, out) {
            errs += 1;
        }
        let nbyper = match self.datatype_sizes(da.datatype, fix, out) {
            Some(s) => s.0,
            None => {
                errs += 1;
                0
            }
        };
        if self.valid_nbyper(nbyper, false, fix, out) && nbyper != da.nbyper {
            if whine || self.verb > 3 {
                out.err(&format!(
                    "** nbyper {nbyper}, does not match type {}\n",
                    datatype2str(da.datatype)
                ));
            }
            errs += 1;
        }
        errs == 0
    }

    /// `gifti_valid_gifti_image`.
    pub fn valid_gifti_image(
        &self,
        gim: Option<&GiftiImage>,
        whine: bool,
        dialect: &dyn Dialect,
        fix: bool,
        out: &mut dyn Streams,
        notices: &mut Notices,
    ) -> bool {
        let Some(gim) = gim else {
            if whine {
                out.err("** invalid: gifti_image ptr is NULL\n");
            }
            return false;
        };
        if self.verb > 3 {
            out.err("-- checking for valid gifti_image...\n");
        }
        let mut errs = 0;
        if gim.num_da < 0 {
            if whine {
                out.err(&format!("** invalid: numDA = {}\n", gim.num_da));
            }
            errs += 1;
        }
        errs += dialect.check_version(gim, whine, out);
        if !self.valid_nvpairs(&gim.meta, whine, out) {
            errs += 1;
        }
        if !self.valid_label_table(&gim.labeltable, whine, out, notices) {
            errs += 1;
        }
        for c in 0..gim.num_da.max(0) as usize {
            if self.verb > 5 {
                out.err(&format!("-- checking DA[{c}]\n"));
            }
            let da = gim.darray.get(c).and_then(|d| d.as_ref());
            if !self.valid_data_array(da, whine, fix, out) {
                if self.verb > 3 {
                    out.err(&format!("-- DA[{c}] has errors\n"));
                }
                errs += 1;
            } else if self.verb > 4 {
                out.err(&format!("-- DA[{c}] is VALID\n"));
            }
        }
        if !self.valid_nvpairs(&gim.ex_atrs, whine, out) {
            errs += 1;
        }
        if self.verb > 2 {
            out.err(&format!("-- gifti_image: errors = {errs}"));
            out.err(if errs > 0 {
                " (INVALID)\n"
            } else {
                " (VALID)\n"
            });
        }
        errs == 0
    }

    /// `gifti_str2attr_gifti`: 0 when the attribute is the image's own.
    pub fn str2attr_gifti(
        &self,
        gim: &mut GiftiImage,
        attr: &[u8],
        val: &[u8],
        out: &mut dyn Streams,
    ) -> i32 {
        if self.verb > 2 {
            out.err(&format!(
                "++ setting GIFTI attr '{}' from '{}'\n",
                lossy(attr),
                lossy(val)
            ));
        }
        match attr {
            b"Version" => gim.version = Some(cstr(val)),
            b"NumberOfDataArrays" => {
                gim.num_da = atol(val) as i32;
                if gim.num_da < 0 {
                    out.err(&format!(
                        "** invalid NumberOfDataArrays attribute: {}\n",
                        lossy(val)
                    ));
                    gim.num_da = 0;
                    return 1;
                }
            }
            b"xmlns:xsi" | b"xsi:noNamespaceSchemaLocation" => {
                if self.verb > 1 {
                    out.err(&format!(
                        "-- have GIFTI attr, '{}'='{}'\n",
                        lossy(attr),
                        lossy(val)
                    ));
                }
                return 1;
            }
            _ => {
                if self.verb > 1 {
                    out.err(&format!(
                        "** unknown GIFTI attrib, '{}'='{}'\n",
                        lossy(attr),
                        lossy(val)
                    ));
                }
                return 1;
            }
        }
        0
    }

    /// `gifti_str2attr_darray`.
    pub fn str2attr_darray(
        &self,
        da: &mut DataArray,
        attr: &[u8],
        value: &[u8],
        out: &mut dyn Streams,
    ) -> i32 {
        if self.verb > 3 {
            out.err(&format!(
                "++ setting DA attr '{}'='{}'\n",
                lossy(attr),
                lossy(value)
            ));
        }
        let atoi = |v: &[u8]| atol(v) as i32;
        match attr {
            b"Intent" => da.intent = intent_from_string(value),
            b"DataType" => da.datatype = str2datatype(value),
            b"ArrayIndexingOrder" => {
                da.ind_ord = self.str2list(List::IndexOrder, value, "** bad index order, '", out)
            }
            b"Dimensionality" => da.num_dim = atoi(value),
            b"Dim0" => da.dims[0] = atoi(value),
            b"Dim1" => da.dims[1] = atoi(value),
            b"Dim2" => da.dims[2] = atoi(value),
            b"Dim3" => da.dims[3] = atoi(value),
            b"Dim4" => da.dims[4] = atoi(value),
            b"Dim5" => da.dims[5] = atoi(value),
            b"Encoding" => {
                da.encoding = self.str2list(List::Encoding, value, "** bad data encoding, '", out)
            }
            b"Endian" => da.endian = self.str2list(List::Endian, value, "** bad endian, '", out),
            b"ExternalFileName" => da.ext_fname = Some(cstr(value)),
            b"ExternalFileOffset" => da.ext_offset = atol(value),
            _ => {
                if self.verb > 1 {
                    out.err(&format!(
                        "** unknown giiDataArray attr, '{}'='{}'\n",
                        lossy(attr),
                        lossy(value)
                    ));
                }
                return 1;
            }
        }
        0
    }

    /// `gifti_str2ind_ord`, `gifti_str2encoding`, `gifti_str2endian`.
    fn str2list(&self, list: List, s: &[u8], mesg: &str, out: &mut dyn Streams) -> i32 {
        let items = list.items();
        let rv = (1..items.len())
            .rev()
            .find(|&i| items[i].as_bytes() == s)
            .unwrap_or(0) as i32;
        if rv <= 0 && self.verb > 1 {
            out.err(&format!("{mesg}{}'\n", lossy(s)));
        }
        rv
    }

    /// `gifti_set_DA_atrs` with `add_to_extras`: an unknown attribute goes into `ex_atrs`.
    pub fn set_da_atrs(
        &self,
        da: &mut DataArray,
        attr: &[(Vec<u8>, Vec<u8>)],
        add_to_extras: bool,
        fix: bool,
        out: &mut dyn Streams,
    ) -> i32 {
        if self.verb > 5 {
            out.err(&format!(
                "++ init darray attrs, len {}, ex_atrs = {}\n",
                2 * attr.len(),
                add_to_extras as i32
            ));
        }
        for (n, v) in attr {
            if self.str2attr_darray(da, n, v, out) != 0 {
                if add_to_extras {
                    self.add_to_nvpairs(&mut da.ex_atrs, n, v, out);
                } else {
                    if self.verb > 0 {
                        out.err(&format!(
                            "** set_darray_atrs, bad pair '{}'='{}'\n",
                            lossy(n),
                            lossy(v)
                        ));
                    }
                    return 1;
                }
            }
        }
        da.nvals = self.darray_nvals(da, out);
        da.nbyper = self
            .datatype_sizes(da.datatype, fix, out)
            .map_or(0, |s| s.0);
        0
    }

    /// `gifti_add_to_nvpairs`.
    pub fn add_to_nvpairs(
        &self,
        p: &mut NvPairs,
        name: &[u8],
        value: &[u8],
        out: &mut dyn Streams,
    ) -> i32 {
        if self.verb > 3 {
            out.err(&format!(
                "++ add_nvp [{}]: '{}', '{}'\n",
                p.len() + 1,
                lossy(name),
                lossy(value)
            ));
        }
        p.name.push(Some(cstr(name)));
        p.value.push(Some(cstr(value)));
        0
    }

    /// `gifti_add_to_meta`: 0 when added or replaced. An entry without a name never matches
    /// (gifticlib crashes there at verbosity 2 and below: FS-84).
    pub fn add_to_meta(
        &self,
        md: &mut NvPairs,
        name: &[u8],
        value: &[u8],
        replace: bool,
        out: &mut dyn Streams,
        notices: &mut Notices,
    ) -> i32 {
        if self.verb > 5 {
            out.err(&format!(
                "++ GA2M: name '{}', value '{}', replace = {}\n",
                lossy(name),
                lossy(value),
                replace as i32
            ));
        }
        let name = cstr(name);
        for c in 0..md.len() {
            let Some(n) = &md.name[c] else {
                if self.verb > 2 {
                    out.err(&format!("** G MD[{c}]: no name to check for replacement\n"));
                } else {
                    notices.add(&d::FS_84, "a metadata entry without a name skipped");
                }
                continue;
            };
            if *n == name {
                if md.value[c].is_none() && self.verb > 2 {
                    out.err(&format!("** G MD[{c}]: no value to replace\n"));
                    md.value[c] = Some(cstr(value));
                    return 0;
                }
                if replace {
                    if self.verb > 5 {
                        out.err("   (add via REPLACE)\n");
                    }
                    md.value[c] = Some(cstr(value));
                    return 0;
                }
                out.err(&format!(
                    "** G_add_to_meta: name '{}', already exists\n",
                    lossy(&name)
                ));
                return 1;
            }
        }
        if self.verb > 5 {
            out.err("   (adding new entry)\n");
        }
        md.name.push(Some(name));
        md.value.push(Some(cstr(value)));
        0
    }

    /// `gifti_get_meta_value`: the value of the first entry named `name` (an entry without a
    /// name never matches: FS-84).
    pub fn get_meta_value<'a>(
        &self,
        nvp: &'a NvPairs,
        name: &[u8],
        out: &mut dyn Streams,
        notices: &mut Notices,
    ) -> Option<&'a CStr> {
        if self.verb > 5 {
            out.err(&format!(
                "-- G_get_meta_value: looking for name = '{}'\n",
                lossy(name)
            ));
        }
        if nvp.is_empty() {
            if self.verb > 3 {
                out.err("-- G_get_meta_value: no name/value array\n");
            }
            return None;
        }
        for c in 0..nvp.len() {
            match &nvp.name[c] {
                None => notices.add(&d::FS_84, "a metadata entry without a name skipped"),
                Some(n) if n.as_slice() == name => {
                    if self.verb > 3 {
                        out.err(&format!(
                            "++ found meta '{}'='{}'\n",
                            lossy(n),
                            show(&nvp.value[c])
                        ));
                    }
                    return Some(&nvp.value[c]);
                }
                _ => {}
            }
        }
        None
    }

    /// `gifti_add_empty_CS`.
    pub fn add_empty_cs(&self, da: &mut DataArray, out: &mut dyn Streams) -> i32 {
        if self.verb > 3 {
            out.err(&format!("++ adding empty CS[{}]\n", da.num_cs()));
        }
        da.coordsys.push(Some(CoordSystem::default()));
        0
    }

    /// `gifti_add_empty_darray`.
    pub fn add_empty_darray(
        &self,
        gim: &mut GiftiImage,
        num_to_add: i32,
        out: &mut dyn Streams,
    ) -> i32 {
        let nnew = if num_to_add > 0 { num_to_add } else { 1 };
        if self.verb > 3 {
            out.err(&format!("++ alloc darray[{}] (+{nnew})\n", gim.num_da));
        }
        gim.darray.truncate(gim.num_da.max(0) as usize);
        for _ in 0..nnew {
            if self.verb > 5 {
                out.err("-- clearing DataArray\n");
            }
            gim.darray.push(Some(DataArray::default()));
            gim.num_da += 1;
        }
        0
    }

    /// `gifti_find_DA`: the `index`-th array of `intent`.
    pub fn find_da<'a>(
        &self,
        gim: &'a GiftiImage,
        intent: i32,
        index: i32,
        out: &mut dyn Streams,
    ) -> Option<&'a DataArray> {
        if index < 0 {
            out.err(&format!(
                "** find_DA: bad inputs (0x1, {intent}, {index})\n"
            ));
            return None;
        }
        gim.darray
            .iter()
            .take(gim.num_da.max(0) as usize)
            .flatten()
            .filter(|da| da.intent == intent)
            .nth(index as usize)
    }

    /// `gifti_gim_DA_size`.
    pub fn gim_da_size(&self, p: &GiftiImage, in_mb: bool, out: &mut dyn Streams) -> i64 {
        if p.num_da <= 0 || p.darray.is_empty() {
            return 0;
        }
        let mut bytes: i64 = 0;
        for c in 0..p.num_da as usize {
            let Some(Some(da)) = p.darray.get(c) else {
                continue;
            };
            if da.data.is_none() {
                continue;
            }
            if da.nvals <= 0 || da.nbyper <= 0 {
                out.err(&format!(
                    "** have data[{c}], but nvals = {}, nbyper = {}\n",
                    da.nvals, da.nbyper
                ));
                return 0;
            }
            bytes = bytes.wrapping_add(da.nvals.wrapping_mul(da.nbyper as i64));
        }
        if bytes <= 0 {
            return 0;
        }
        if in_mb {
            bytes = (bytes + (1 << 19)) >> 20;
        }
        bytes
    }

    /// `gifti_DA_rows_cols`: (rows, cols, success); on failure the first guess (`dims[0]`, 1).
    pub fn da_rows_cols(&self, da: &DataArray, out: &mut dyn Streams) -> (i64, i64, bool) {
        let rows = da.dims[0] as i64;
        if da.num_dim == 1 {
            return (rows, 1, true);
        }
        if da.ind_ord == GIFTI_IND_ORD_ROW_MAJOR {
            let cols = if rows != 0 { da.nvals / rows } else { 1 };
            return (rows, cols, true);
        }
        if !self.valid_num_dim(da.num_dim, true, out) {
            out.err("** cannot assign DA_rows_cols");
            return (rows, 1, false);
        }
        let rows = da.dims[da.num_dim as usize - 1] as i64;
        let cols = if rows > 0 { da.nvals / rows } else { 1 };
        (rows, cols, true)
    }

    /// `gifti_check_swap` on `data` (little-endian in memory): 1 when the bytes were swapped.
    pub fn check_swap(
        &self,
        data: &mut [u8],
        endian: i32,
        nsets: i64,
        swapsize: i32,
        out: &mut dyn Streams,
    ) -> i32 {
        if nsets < 0 || swapsize < 0 {
            out.err(&format!(
                "** check_swap: bad params (0x1,{nsets}, {swapsize})\n"
            ));
            return 0;
        } else if endian <= GIFTI_ENDIAN_UNDEF || endian > GIFTI_ENDIAN_MAX {
            out.err(&format!("** check_swap: invalid endian {endian}\n"));
            return 0;
        }
        if endian == this_endian() {
            if self.verb > 2 {
                out.err(&format!(
                    "-- darray no swap needed : {nsets} sets of {swapsize} bytes\n"
                ));
            }
            return 0;
        }
        if self.verb > 2 {
            out.err(&format!(
                "++ darray swap: {nsets} sets of {swapsize} bytes\n"
            ));
        }
        if swapsize >= 2 {
            let s = swapsize as usize;
            for set in data.chunks_exact_mut(s).take(nsets as usize) {
                set.reverse();
            }
        }
        1
    }

    /// `gifti_copy_DataArray`.
    pub fn copy_data_array(
        &self,
        orig: &DataArray,
        get_data: bool,
        out: &mut dyn Streams,
    ) -> DataArray {
        if self.verb > 5 {
            out.err("++ copying giiDataArray...\n");
        }
        let v = self.verb;
        let nvp = |p: &NvPairs, out: &mut dyn Streams| {
            if v > 6 {
                out.err(&format!("++ copy_nvp, length {}\n", p.len()));
            }
        };
        nvp(&orig.meta, out);
        if orig.num_cs() > 0 && v > 6 {
            for cs in &orig.coordsys {
                if cs.is_some() {
                    out.err("++ copy_CS\n");
                }
            }
        }
        let mut n = orig.clone();
        if !(orig.data.is_some() && get_data) {
            n.data = None;
        } else if v > 5 {
            out.err("++ copy_DA, adding data\n");
        }
        nvp(&orig.ex_atrs, out);
        n
    }

    // ---- displays ---------------------------------------------------------------------

    /// `gifti_disp_nvpairs`.
    pub fn disp_nvpairs(&self, mesg: Option<&str>, p: &NvPairs, out: &mut dyn Streams) {
        if let Some(m) = mesg {
            out.err(&format!("{m} "));
        }
        out.err(&format!("nvpairs struct, len = {} :\n", p.len()));
        for c in 0..p.len() {
            out.err(&format!(
                "    nvpair: '{}' = '{}'\n",
                show(&p.name[c]),
                show(&p.value[c])
            ));
        }
        if !p.is_empty() {
            out.err("\n");
        }
    }

    /// `gifti_disp_LabelTable`.
    pub fn disp_label_table(
        &self,
        mesg: Option<&str>,
        p: &LabelTable,
        out: &mut dyn Streams,
        notices: &mut Notices,
    ) {
        if let Some(m) = mesg {
            out.err(&format!("{m} "));
        }
        out.err(&format!("giiLabelTable struct, len = {} :\n", p.len()));
        for c in 0..p.len() {
            out.err(&format!("    key {}, ", p.key[c]));
            if p.rgba.is_some() {
                let v = rgba_of(p, c, notices);
                let f = |x: f32| cfmt::pad(&cfmt::f(x as f64, 3), 5);
                out.err(&format!(
                    "rgba ({}, {}, {}, {}), ",
                    f(v[0]),
                    f(v[1]),
                    f(v[2]),
                    f(v[3])
                ));
            }
            out.err(&format!("label '{}'\n", show(&p.label[c])));
        }
        if !p.is_empty() {
            out.err("\n");
        }
    }

    /// `gifti_disp_CoordSystem`.
    pub fn disp_coord_system(
        &self,
        mesg: Option<&str>,
        p: Option<&CoordSystem>,
        out: &mut dyn Streams,
    ) {
        if let Some(m) = mesg {
            out.err(&format!("{m} "));
        }
        let Some(p) = p else {
            out.err("disp: giiCoordSystem = NULL\n");
            return;
        };
        out.err(&format!(
            "giiCoordSystem struct\n    dataspace  = {}\n    xformspace = {}\n",
            show(&p.dataspace),
            show(&p.xformspace)
        ));
        for c1 in 0..4 {
            let mut s = format!("    xform[{c1}] :");
            for c2 in 0..4 {
                s += &format!("  {}", cfmt::f(p.xform[c1][c2], 6));
            }
            out.err(&(s + "\n"));
        }
    }

    /// `gifti_disp_DataArray`.
    pub fn disp_data_array(
        &self,
        mesg: Option<&str>,
        p: Option<&DataArray>,
        subs: bool,
        out: &mut dyn Streams,
    ) {
        out.err("--------------------------------------------------\n");
        if let Some(m) = mesg {
            out.err(&format!("{m} "));
        }
        let Some(p) = p else {
            out.err("disp: giiDataArray = NULL\n");
            return;
        };
        let mut s = String::from("giiDataArray struct\n");
        s += &format!(
            "    intent   {:4} = {}\n",
            p.intent,
            intent_to_string(p.intent)
        );
        s += &format!(
            "    datatype   {:2} = {}\n",
            p.datatype,
            datatype2str(p.datatype)
        );
        let io = list_index2string(List::IndexOrder, p.ind_ord, self.verb, out);
        s += &format!("    ind_ord    {:2} = {io}\n", p.ind_ord);
        s += &format!("    num_dim       = {}\n", p.num_dim);
        let dm = p.dims;
        s += &format!(
            "    dims          = {}, {}, {}, {}, {}, {}\n",
            dm[0], dm[1], dm[2], dm[3], dm[4], dm[5]
        );
        let en = list_index2string(List::Encoding, p.encoding, self.verb, out);
        s += &format!("    encoding   {:2} = {en}\n", p.encoding);
        let ed = list_index2string(List::Endian, p.endian, self.verb, out);
        s += &format!("    endian     {:2} = {ed}\n", p.endian);
        s += &format!("    ext_fname     = {}\n", show(&p.ext_fname));
        s += &format!("    ext_offset    = {}\n", p.ext_offset);
        out.err(&s);
        if subs {
            self.disp_nvpairs(Some("darray->meta"), &p.meta, out);
            for c in &p.coordsys {
                self.disp_coord_system(Some("darray->coordsys"), c.as_ref(), out);
            }
        }
        out.err(&format!(
            "    data       = {}\n    nvals      = {}\n    nbyper     = {}\n    numCS      = {}\n",
            if p.data.is_some() { "<set>" } else { "NULL" },
            p.nvals as u32,
            p.nbyper,
            p.num_cs()
        ));
        if subs {
            self.disp_nvpairs(Some("darray->ex_atrs"), &p.ex_atrs, out);
        }
        out.err("--------------------------------------------------\n");
    }

    /// The messages of `gifti_free_image` (at `G.verb` 3 and above), which frees `p`.
    pub fn free_image(&self, p: GiftiImage, out: &mut dyn Streams) {
        let v = self.verb;
        if v > 2 {
            out.err("-- freeing gifti_image\n");
        }
        self.free_nvpairs_messages(&p.meta, out);
        if v > 3 {
            out.err(&format!(
                "-- freeing {} giiLabelTable entries\n",
                p.labeltable.len()
            ));
        }
        self.free_darray_list_messages(&p.darray, p.num_da, out);
        self.free_nvpairs_messages(&p.ex_atrs, out);
    }

    fn free_nvpairs_messages(&self, p: &NvPairs, out: &mut dyn Streams) {
        if self.verb > 3 {
            out.err(&format!("-- freeing {} nvpairs\n", p.len()));
        }
    }

    /// `gifti_free_DataArray_list`'s messages: a list never allocated (no array was ever added)
    /// is C's null list; a null entry stops the loop.
    fn free_darray_list_messages(
        &self,
        list: &[Option<DataArray>],
        num_da: i32,
        out: &mut dyn Streams,
    ) {
        let v = self.verb;
        if list.is_empty() {
            if v > 3 {
                out.err("** GFDA: free NULL darray list\n");
            }
            return;
        }
        if v > 3 {
            out.err(&format!("-- freeing {num_da} giiDataArrays\n"));
        }
        if num_da < 0 {
            return;
        }
        for c in 0..num_da as usize {
            match list.get(c).and_then(|d| d.as_ref()) {
                None => {
                    if v > 3 {
                        out.err("** tried to free NULL darray ptr\n");
                    }
                    return;
                }
                Some(da) => {
                    if v > 3 {
                        out.err("-- freeing giiDataArray\n");
                    }
                    self.free_nvpairs_messages(&da.meta, out);
                    if v > 3 {
                        out.err("-- freeing giiCoordSystem list\n");
                        if da.num_cs() > 0 {
                            for cs in &da.coordsys {
                                if cs.is_some() {
                                    out.err("-- freeing giiCoordSystem\n");
                                }
                            }
                        }
                    }
                    self.free_nvpairs_messages(&da.ex_atrs, out);
                }
            }
        }
    }

    /// `gifti_disp_gifti_image`.
    pub fn disp_gifti_image(
        &self,
        mesg: Option<&str>,
        p: &GiftiImage,
        subs: bool,
        out: &mut dyn Streams,
        notices: &mut Notices,
    ) {
        out.err("==================================================\n");
        if let Some(m) = mesg {
            out.err(&format!("{m} "));
        }
        out.err(&format!(
            "gifti_image struct\n    version    = {}\n    numDA      = {}\n",
            show(&p.version),
            p.num_da
        ));
        if subs {
            self.disp_nvpairs(Some("gim->meta"), &p.meta, out);
            self.disp_label_table(Some("gim->labeltable"), &p.labeltable, out, notices);
            for c in 0..p.num_da.max(0) as usize {
                let da = p.darray.get(c).and_then(|d| d.as_ref());
                self.disp_data_array(Some(&format!("gim->darray[{c}]")), da, subs, out);
            }
        }
        out.err(&format!(
            "gifti_image struct\n    swapped    = {}\n    compressed = {}\n",
            p.swapped, p.compressed
        ));
        let mb = self.gim_da_size(p, true, out);
        out.err(&format!(" -- darray totals: {mb} MB\n"));
        if subs {
            self.disp_nvpairs(Some("gim->ex_atrs"), &p.ex_atrs, out);
        }
        out.err("==================================================\n");
    }

    /// `gifti_disp_raw_data` of `nvals` values (to standard error when `to_err`).
    pub fn disp_raw_data(
        &self,
        data: &[u8],
        t: i32,
        nvals: i32,
        newline: bool,
        out: &mut dyn Streams,
        to_err: bool,
    ) -> i32 {
        let size = self.datatype_sizes(t, true, out).map_or(0, |s| s.0) as usize;
        if size == 0 {
            out.err(&format!("** GDRD: cannot print with size 0, type {t}\n"));
            return 1;
        }
        let mut s = String::new();
        for c in 0..nvals.max(0) as usize {
            let v = &data[c * size..(c + 1) * size];
            let mut b = [0u8; 8];
            b[..size.min(8)].copy_from_slice(&v[..size.min(8)]);
            let item = match t {
                NIFTI_TYPE_INT8 => (v[0] as i8).to_string(),
                NIFTI_TYPE_INT16 => i16::from_le_bytes([b[0], b[1]]).to_string(),
                NIFTI_TYPE_INT32 => i32::from_le_bytes([b[0], b[1], b[2], b[3]]).to_string(),
                NIFTI_TYPE_INT64 => i64::from_le_bytes(b).to_string(),
                NIFTI_TYPE_UINT8 => v[0].to_string(),
                NIFTI_TYPE_UINT16 => u16::from_le_bytes([b[0], b[1]]).to_string(),
                NIFTI_TYPE_UINT32 => u32::from_le_bytes([b[0], b[1], b[2], b[3]]).to_string(),
                NIFTI_TYPE_UINT64 => u64::from_le_bytes(b).to_string(),
                NIFTI_TYPE_FLOAT32 | NIFTI_TYPE_FLOAT64 => {
                    let x = if t == NIFTI_TYPE_FLOAT32 {
                        f32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f64
                    } else {
                        f64::from_le_bytes(b)
                    };
                    let mut f = cfmt::f(x, 6);
                    clear_float_zeros(&mut f);
                    f
                }
                _ => {
                    out.err(&format!("** Gdisp_raw_data: invalid type {t}\n"));
                    return 1;
                }
            };
            s += &item;
            if (c as i32) < nvals - 1 {
                s.push(' ');
            }
        }
        if newline {
            s.push('\n');
        }
        if to_err {
            out.err(&s)
        } else {
            out.out(&s)
        }
        0
    }
}

/// A label's colours, the holes of FS-80 read as 0 (with a notice).
pub(crate) fn rgba_of(t: &LabelTable, c: usize, notices: &mut Notices) -> [f32; 4] {
    let mut v = [0.0f32; 4];
    let Some(rgba) = &t.rgba else { return v };
    let mut hole = false;
    for k in 0..4 {
        match rgba.get(4 * c + k).copied().flatten() {
            Some(x) => v[k] = x,
            None => hole = true,
        }
    }
    if hole {
        notices.add(
            &d::FS_80,
            format!("label {c}'s colours were never set: read as 0"),
        );
    }
    v
}
