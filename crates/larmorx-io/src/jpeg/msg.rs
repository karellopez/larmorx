// SPDX-License-Identifier: Apache-2.0 AND IJG
//! IJG 6b's messages (`jerror.h`, its table in message-code order) and its standard error
//! manager (`jerror.c`'s `jpeg_std_error`: `emit_message`, `output_message`, `format_message`).

use larmorx_core::cfmt::Streams;

/// An error of the original (`ERREXIT`): its message was written to standard error, and the
/// original's `error_exit` ends the program there with status 1 (`EXIT_FAILURE`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fatal {
    /// The message code (`jerror.h`'s order: 0 is `JMSG_NOMESSAGE`).
    pub code: usize,
    /// The message as written, without its newline.
    pub message: String,
}

impl std::fmt::Display for Fatal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Fatal {}

#[rustfmt::skip]
pub(super) const TABLE: [&str; 124] = [
    "Bogus message code %d", // JMSG_NOMESSAGE
    "Sorry, there are legal restrictions on arithmetic coding", // JERR_ARITH_NOTIMPL
    "ALIGN_TYPE is wrong, please fix", // JERR_BAD_ALIGN_TYPE
    "MAX_ALLOC_CHUNK is wrong, please fix", // JERR_BAD_ALLOC_CHUNK
    "Bogus buffer control mode", // JERR_BAD_BUFFER_MODE
    "Invalid component ID %d in SOS", // JERR_BAD_COMPONENT_ID
    "DCT coefficient out of range", // JERR_BAD_DCT_COEF
    "IDCT output block size %d not supported", // JERR_BAD_DCTSIZE
    "Bogus Huffman table definition", // JERR_BAD_HUFF_TABLE
    "Bogus input colorspace", // JERR_BAD_IN_COLORSPACE
    "Bogus JPEG colorspace", // JERR_BAD_J_COLORSPACE
    "Bogus marker length", // JERR_BAD_LENGTH
    "Wrong JPEG library version: library is %d, caller expects %d", // JERR_BAD_LIB_VERSION
    "Sampling factors too large for interleaved scan", // JERR_BAD_MCU_SIZE
    "Invalid memory pool code %d", // JERR_BAD_POOL_ID
    "Unsupported JPEG data precision %d", // JERR_BAD_PRECISION
    "Invalid progressive parameters Ss=%d Se=%d Ah=%d Al=%d", // JERR_BAD_PROGRESSION
    "Invalid progressive parameters at scan script entry %d", // JERR_BAD_PROG_SCRIPT
    "Bogus sampling factors", // JERR_BAD_SAMPLING
    "Invalid scan script at entry %d", // JERR_BAD_SCAN_SCRIPT
    "Improper call to JPEG library in state %d", // JERR_BAD_STATE
    "JPEG parameter struct mismatch: library thinks size is %u, caller expects %u", // JERR_BAD_STRUCT_SIZE
    "Bogus virtual array access", // JERR_BAD_VIRTUAL_ACCESS
    "Buffer passed to JPEG library is too small", // JERR_BUFFER_SIZE
    "Suspension not allowed here", // JERR_CANT_SUSPEND
    "CCIR601 sampling not implemented yet", // JERR_CCIR601_NOTIMPL
    "Too many color components: %d, max %d", // JERR_COMPONENT_COUNT
    "Unsupported color conversion request", // JERR_CONVERSION_NOTIMPL
    "Bogus DAC index %d", // JERR_DAC_INDEX
    "Bogus DAC value 0x%x", // JERR_DAC_VALUE
    "Bogus DHT index %d", // JERR_DHT_INDEX
    "Bogus DQT index %d", // JERR_DQT_INDEX
    "Empty JPEG image (DNL not supported)", // JERR_EMPTY_IMAGE
    "Read from EMS failed", // JERR_EMS_READ
    "Write to EMS failed", // JERR_EMS_WRITE
    "Didn't expect more than one scan", // JERR_EOI_EXPECTED
    "Input file read error", // JERR_FILE_READ
    "Output file write error --- out of disk space?", // JERR_FILE_WRITE
    "Fractional sampling not implemented yet", // JERR_FRACT_SAMPLE_NOTIMPL
    "Huffman code size table overflow", // JERR_HUFF_CLEN_OVERFLOW
    "Missing Huffman code table entry", // JERR_HUFF_MISSING_CODE
    "Maximum supported image dimension is %u pixels", // JERR_IMAGE_TOO_BIG
    "Empty input file", // JERR_INPUT_EMPTY
    "Premature end of input file", // JERR_INPUT_EOF
    "Cannot transcode due to multiple use of quantization table %d", // JERR_MISMATCHED_QUANT_TABLE
    "Scan script does not transmit all data", // JERR_MISSING_DATA
    "Invalid color quantization mode change", // JERR_MODE_CHANGE
    "Not implemented yet", // JERR_NOTIMPL
    "Requested feature was omitted at compile time", // JERR_NOT_COMPILED
    "Backing store not supported", // JERR_NO_BACKING_STORE
    "Huffman table 0x%02x was not defined", // JERR_NO_HUFF_TABLE
    "JPEG datastream contains no image", // JERR_NO_IMAGE
    "Quantization table 0x%02x was not defined", // JERR_NO_QUANT_TABLE
    "Not a JPEG file: starts with 0x%02x 0x%02x", // JERR_NO_SOI
    "Insufficient memory (case %d)", // JERR_OUT_OF_MEMORY
    "Cannot quantize more than %d color components", // JERR_QUANT_COMPONENTS
    "Cannot quantize to fewer than %d colors", // JERR_QUANT_FEW_COLORS
    "Cannot quantize to more than %d colors", // JERR_QUANT_MANY_COLORS
    "Invalid JPEG file structure: two SOF markers", // JERR_SOF_DUPLICATE
    "Invalid JPEG file structure: missing SOS marker", // JERR_SOF_NO_SOS
    "Unsupported JPEG process: SOF type 0x%02x", // JERR_SOF_UNSUPPORTED
    "Invalid JPEG file structure: two SOI markers", // JERR_SOI_DUPLICATE
    "Invalid JPEG file structure: SOS before SOF", // JERR_SOS_NO_SOF
    "Failed to create temporary file %s", // JERR_TFILE_CREATE
    "Read failed on temporary file", // JERR_TFILE_READ
    "Seek failed on temporary file", // JERR_TFILE_SEEK
    "Write failed on temporary file --- out of disk space?", // JERR_TFILE_WRITE
    "Application transferred too few scanlines", // JERR_TOO_LITTLE_DATA
    "Unsupported marker type 0x%02x", // JERR_UNKNOWN_MARKER
    "Virtual array controller messed up", // JERR_VIRTUAL_BUG
    "Image too wide for this implementation", // JERR_WIDTH_OVERFLOW
    "Read from XMS failed", // JERR_XMS_READ
    "Write to XMS failed", // JERR_XMS_WRITE
    "Copyright (C) 1998, Thomas G. Lane", // JMSG_COPYRIGHT
    "6b  27-Mar-1998", // JMSG_VERSION
    "Caution: quantization tables are too coarse for baseline JPEG", // JTRC_16BIT_TABLES
    "Adobe APP14 marker: version %d, flags 0x%04x 0x%04x, transform %d", // JTRC_ADOBE
    "Unknown APP0 marker (not JFIF), length %u", // JTRC_APP0
    "Unknown APP14 marker (not Adobe), length %u", // JTRC_APP14
    "Define Arithmetic Table 0x%02x: 0x%02x", // JTRC_DAC
    "Define Huffman Table 0x%02x", // JTRC_DHT
    "Define Quantization Table %d  precision %d", // JTRC_DQT
    "Define Restart Interval %u", // JTRC_DRI
    "Freed EMS handle %u", // JTRC_EMS_CLOSE
    "Obtained EMS handle %u", // JTRC_EMS_OPEN
    "End Of Image", // JTRC_EOI
    "        %3d %3d %3d %3d %3d %3d %3d %3d", // JTRC_HUFFBITS
    "JFIF APP0 marker: version %d.%02d, density %dx%d  %d", // JTRC_JFIF
    "Warning: thumbnail image size does not match data length %u", // JTRC_JFIF_BADTHUMBNAILSIZE
    "JFIF extension marker: type 0x%02x, length %u", // JTRC_JFIF_EXTENSION
    "    with %d x %d thumbnail image", // JTRC_JFIF_THUMBNAIL
    "Miscellaneous marker 0x%02x, length %u", // JTRC_MISC_MARKER
    "Unexpected marker 0x%02x", // JTRC_PARMLESS_MARKER
    "        %4u %4u %4u %4u %4u %4u %4u %4u", // JTRC_QUANTVALS
    "Quantizing to %d = %d*%d*%d colors", // JTRC_QUANT_3_NCOLORS
    "Quantizing to %d colors", // JTRC_QUANT_NCOLORS
    "Selected %d colors for quantization", // JTRC_QUANT_SELECTED
    "At marker 0x%02x, recovery action %d", // JTRC_RECOVERY_ACTION
    "RST%d", // JTRC_RST
    "Smoothing not supported with nonstandard sampling ratios", // JTRC_SMOOTH_NOTIMPL
    "Start Of Frame 0x%02x: width=%u, height=%u, components=%d", // JTRC_SOF
    "    Component %d: %dhx%dv q=%d", // JTRC_SOF_COMPONENT
    "Start of Image", // JTRC_SOI
    "Start Of Scan: %d components", // JTRC_SOS
    "    Component %d: dc=%d ac=%d", // JTRC_SOS_COMPONENT
    "  Ss=%d, Se=%d, Ah=%d, Al=%d", // JTRC_SOS_PARAMS
    "Closed temporary file %s", // JTRC_TFILE_CLOSE
    "Opened temporary file %s", // JTRC_TFILE_OPEN
    "JFIF extension marker: JPEG-compressed thumbnail image, length %u", // JTRC_THUMB_JPEG
    "JFIF extension marker: palette thumbnail image, length %u", // JTRC_THUMB_PALETTE
    "JFIF extension marker: RGB thumbnail image, length %u", // JTRC_THUMB_RGB
    "Unrecognized component IDs %d %d %d, assuming YCbCr", // JTRC_UNKNOWN_IDS
    "Freed XMS handle %u", // JTRC_XMS_CLOSE
    "Obtained XMS handle %u", // JTRC_XMS_OPEN
    "Unknown Adobe color transform code %d", // JWRN_ADOBE_XFORM
    "Inconsistent progression sequence for component %d coefficient %d", // JWRN_BOGUS_PROGRESSION
    "Corrupt JPEG data: %u extraneous bytes before marker 0x%02x", // JWRN_EXTRANEOUS_DATA
    "Corrupt JPEG data: premature end of data segment", // JWRN_HIT_MARKER
    "Corrupt JPEG data: bad Huffman code", // JWRN_HUFF_BAD_CODE
    "Warning: unknown JFIF revision number %d.%02d", // JWRN_JFIF_MAJOR
    "Premature end of JPEG file", // JWRN_JPEG_EOF
    "Corrupt JPEG data: found marker 0x%02x instead of RST%d", // JWRN_MUST_RESYNC
    "Invalid SOS parameters for sequential JPEG", // JWRN_NOT_SEQUENTIAL
    "Application transferred too many scanlines", // JWRN_TOO_MUCH_DATA
];

/// The message codes (`jerror.h`'s enumeration), every one kept with its text.
#[allow(dead_code)]
mod codes {
    pub(crate) const JMSG_NOMESSAGE: usize = 0;
    pub(crate) const JERR_ARITH_NOTIMPL: usize = 1;
    pub(crate) const JERR_BAD_ALIGN_TYPE: usize = 2;
    pub(crate) const JERR_BAD_ALLOC_CHUNK: usize = 3;
    pub(crate) const JERR_BAD_BUFFER_MODE: usize = 4;
    pub(crate) const JERR_BAD_COMPONENT_ID: usize = 5;
    pub(crate) const JERR_BAD_DCT_COEF: usize = 6;
    pub(crate) const JERR_BAD_DCTSIZE: usize = 7;
    pub(crate) const JERR_BAD_HUFF_TABLE: usize = 8;
    pub(crate) const JERR_BAD_IN_COLORSPACE: usize = 9;
    pub(crate) const JERR_BAD_J_COLORSPACE: usize = 10;
    pub(crate) const JERR_BAD_LENGTH: usize = 11;
    pub(crate) const JERR_BAD_LIB_VERSION: usize = 12;
    pub(crate) const JERR_BAD_MCU_SIZE: usize = 13;
    pub(crate) const JERR_BAD_POOL_ID: usize = 14;
    pub(crate) const JERR_BAD_PRECISION: usize = 15;
    pub(crate) const JERR_BAD_PROGRESSION: usize = 16;
    pub(crate) const JERR_BAD_PROG_SCRIPT: usize = 17;
    pub(crate) const JERR_BAD_SAMPLING: usize = 18;
    pub(crate) const JERR_BAD_SCAN_SCRIPT: usize = 19;
    pub(crate) const JERR_BAD_STATE: usize = 20;
    pub(crate) const JERR_BAD_STRUCT_SIZE: usize = 21;
    pub(crate) const JERR_BAD_VIRTUAL_ACCESS: usize = 22;
    pub(crate) const JERR_BUFFER_SIZE: usize = 23;
    pub(crate) const JERR_CANT_SUSPEND: usize = 24;
    pub(crate) const JERR_CCIR601_NOTIMPL: usize = 25;
    pub(crate) const JERR_COMPONENT_COUNT: usize = 26;
    pub(crate) const JERR_CONVERSION_NOTIMPL: usize = 27;
    pub(crate) const JERR_DAC_INDEX: usize = 28;
    pub(crate) const JERR_DAC_VALUE: usize = 29;
    pub(crate) const JERR_DHT_INDEX: usize = 30;
    pub(crate) const JERR_DQT_INDEX: usize = 31;
    pub(crate) const JERR_EMPTY_IMAGE: usize = 32;
    pub(crate) const JERR_EMS_READ: usize = 33;
    pub(crate) const JERR_EMS_WRITE: usize = 34;
    pub(crate) const JERR_EOI_EXPECTED: usize = 35;
    pub(crate) const JERR_FILE_READ: usize = 36;
    pub(crate) const JERR_FILE_WRITE: usize = 37;
    pub(crate) const JERR_FRACT_SAMPLE_NOTIMPL: usize = 38;
    pub(crate) const JERR_HUFF_CLEN_OVERFLOW: usize = 39;
    pub(crate) const JERR_HUFF_MISSING_CODE: usize = 40;
    pub(crate) const JERR_IMAGE_TOO_BIG: usize = 41;
    pub(crate) const JERR_INPUT_EMPTY: usize = 42;
    pub(crate) const JERR_INPUT_EOF: usize = 43;
    pub(crate) const JERR_MISMATCHED_QUANT_TABLE: usize = 44;
    pub(crate) const JERR_MISSING_DATA: usize = 45;
    pub(crate) const JERR_MODE_CHANGE: usize = 46;
    pub(crate) const JERR_NOTIMPL: usize = 47;
    pub(crate) const JERR_NOT_COMPILED: usize = 48;
    pub(crate) const JERR_NO_BACKING_STORE: usize = 49;
    pub(crate) const JERR_NO_HUFF_TABLE: usize = 50;
    pub(crate) const JERR_NO_IMAGE: usize = 51;
    pub(crate) const JERR_NO_QUANT_TABLE: usize = 52;
    pub(crate) const JERR_NO_SOI: usize = 53;
    pub(crate) const JERR_OUT_OF_MEMORY: usize = 54;
    pub(crate) const JERR_QUANT_COMPONENTS: usize = 55;
    pub(crate) const JERR_QUANT_FEW_COLORS: usize = 56;
    pub(crate) const JERR_QUANT_MANY_COLORS: usize = 57;
    pub(crate) const JERR_SOF_DUPLICATE: usize = 58;
    pub(crate) const JERR_SOF_NO_SOS: usize = 59;
    pub(crate) const JERR_SOF_UNSUPPORTED: usize = 60;
    pub(crate) const JERR_SOI_DUPLICATE: usize = 61;
    pub(crate) const JERR_SOS_NO_SOF: usize = 62;
    pub(crate) const JERR_TFILE_CREATE: usize = 63;
    pub(crate) const JERR_TFILE_READ: usize = 64;
    pub(crate) const JERR_TFILE_SEEK: usize = 65;
    pub(crate) const JERR_TFILE_WRITE: usize = 66;
    pub(crate) const JERR_TOO_LITTLE_DATA: usize = 67;
    pub(crate) const JERR_UNKNOWN_MARKER: usize = 68;
    pub(crate) const JERR_VIRTUAL_BUG: usize = 69;
    pub(crate) const JERR_WIDTH_OVERFLOW: usize = 70;
    pub(crate) const JERR_XMS_READ: usize = 71;
    pub(crate) const JERR_XMS_WRITE: usize = 72;
    pub(crate) const JMSG_COPYRIGHT: usize = 73;
    pub(crate) const JMSG_VERSION: usize = 74;
    pub(crate) const JTRC_16BIT_TABLES: usize = 75;
    pub(crate) const JTRC_ADOBE: usize = 76;
    pub(crate) const JTRC_APP0: usize = 77;
    pub(crate) const JTRC_APP14: usize = 78;
    pub(crate) const JTRC_DAC: usize = 79;
    pub(crate) const JTRC_DHT: usize = 80;
    pub(crate) const JTRC_DQT: usize = 81;
    pub(crate) const JTRC_DRI: usize = 82;
    pub(crate) const JTRC_EMS_CLOSE: usize = 83;
    pub(crate) const JTRC_EMS_OPEN: usize = 84;
    pub(crate) const JTRC_EOI: usize = 85;
    pub(crate) const JTRC_HUFFBITS: usize = 86;
    pub(crate) const JTRC_JFIF: usize = 87;
    pub(crate) const JTRC_JFIF_BADTHUMBNAILSIZE: usize = 88;
    pub(crate) const JTRC_JFIF_EXTENSION: usize = 89;
    pub(crate) const JTRC_JFIF_THUMBNAIL: usize = 90;
    pub(crate) const JTRC_MISC_MARKER: usize = 91;
    pub(crate) const JTRC_PARMLESS_MARKER: usize = 92;
    pub(crate) const JTRC_QUANTVALS: usize = 93;
    pub(crate) const JTRC_QUANT_3_NCOLORS: usize = 94;
    pub(crate) const JTRC_QUANT_NCOLORS: usize = 95;
    pub(crate) const JTRC_QUANT_SELECTED: usize = 96;
    pub(crate) const JTRC_RECOVERY_ACTION: usize = 97;
    pub(crate) const JTRC_RST: usize = 98;
    pub(crate) const JTRC_SMOOTH_NOTIMPL: usize = 99;
    pub(crate) const JTRC_SOF: usize = 100;
    pub(crate) const JTRC_SOF_COMPONENT: usize = 101;
    pub(crate) const JTRC_SOI: usize = 102;
    pub(crate) const JTRC_SOS: usize = 103;
    pub(crate) const JTRC_SOS_COMPONENT: usize = 104;
    pub(crate) const JTRC_SOS_PARAMS: usize = 105;
    pub(crate) const JTRC_TFILE_CLOSE: usize = 106;
    pub(crate) const JTRC_TFILE_OPEN: usize = 107;
    pub(crate) const JTRC_THUMB_JPEG: usize = 108;
    pub(crate) const JTRC_THUMB_PALETTE: usize = 109;
    pub(crate) const JTRC_THUMB_RGB: usize = 110;
    pub(crate) const JTRC_UNKNOWN_IDS: usize = 111;
    pub(crate) const JTRC_XMS_CLOSE: usize = 112;
    pub(crate) const JTRC_XMS_OPEN: usize = 113;
    pub(crate) const JWRN_ADOBE_XFORM: usize = 114;
    pub(crate) const JWRN_BOGUS_PROGRESSION: usize = 115;
    pub(crate) const JWRN_EXTRANEOUS_DATA: usize = 116;
    pub(crate) const JWRN_HIT_MARKER: usize = 117;
    pub(crate) const JWRN_HUFF_BAD_CODE: usize = 118;
    pub(crate) const JWRN_JFIF_MAJOR: usize = 119;
    pub(crate) const JWRN_JPEG_EOF: usize = 120;
    pub(crate) const JWRN_MUST_RESYNC: usize = 121;
    pub(crate) const JWRN_NOT_SEQUENTIAL: usize = 122;
    pub(crate) const JWRN_TOO_MUCH_DATA: usize = 123;
}
pub(super) use codes::*;

/// `format_message`: the text of `code` (or `JMSG_NOMESSAGE`'s with the code) formatted as C's
/// `sprintf` formats it with the eight integer parameters (no message of the table that can be
/// written here takes the string parameter).
pub(super) fn format(code: usize, p: &[i32]) -> String {
    let mut parms = [0i32; 8];
    for (d, s) in parms.iter_mut().zip(p) {
        *d = *s;
    }
    let text = if code > 0 && code < TABLE.len() {
        TABLE[code]
    } else {
        parms[0] = code as i32;
        TABLE[0]
    };
    let b = text.as_bytes();
    let mut out = String::new();
    let mut i = 0;
    let mut arg = 0;
    while i < b.len() {
        if b[i] != b'%' {
            out.push(b[i] as char);
            i += 1;
            continue;
        }
        i += 1;
        if i < b.len() && b[i] == b'%' {
            out.push('%');
            i += 1;
            continue;
        }
        let mut zero = false;
        while i < b.len() && b[i] == b'0' {
            zero = true;
            i += 1;
        }
        let mut width = 0usize;
        while i < b.len() && b[i].is_ascii_digit() {
            width = width * 10 + (b[i] - b'0') as usize;
            i += 1;
        }
        let conv = b.get(i).copied().unwrap_or(b'd');
        i += 1;
        let v = parms.get(arg).copied().unwrap_or(0);
        arg += 1;
        let s = match conv {
            b'u' => (v as u32).to_string(),
            b'x' => format!("{:x}", v as u32),
            _ => v.to_string(),
        };
        if s.len() < width {
            let fill = width - s.len();
            if zero {
                if let Some(rest) = s.strip_prefix('-') {
                    out.push('-');
                    out.extend(std::iter::repeat_n('0', fill));
                    out.push_str(rest);
                } else {
                    out.extend(std::iter::repeat_n('0', fill));
                    out.push_str(&s);
                }
            } else {
                out.extend(std::iter::repeat_n(' ', fill));
                out.push_str(&s);
            }
        } else {
            out.push_str(&s);
        }
    }
    out
}

/// The standard error manager of one decompress or compress object.
#[derive(Default)]
pub(super) struct ErrMgr {
    /// `num_warnings`.
    pub(super) num_warnings: u32,
}

impl ErrMgr {
    /// `WARNMS*`: `emit_message(-1)`: the first warning is written, every one counted.
    pub(super) fn warn(&mut self, streams: &mut dyn Streams, code: usize, p: &[i32]) {
        if self.num_warnings == 0 {
            let mut m = format(code, p);
            m.push('\n');
            streams.err(&m);
        }
        self.num_warnings += 1;
    }

    /// `ERREXIT*`: `error_exit`'s `output_message`; the caller returns the [`Fatal`].
    pub(super) fn fatal(&mut self, streams: &mut dyn Streams, code: usize, p: &[i32]) -> Fatal {
        let message = format(code, p);
        let mut m = message.clone();
        m.push('\n');
        streams.err(&m);
        Fatal { code, message }
    }

    /// `reset_error_mgr`.
    pub(super) fn reset(&mut self) {
        self.num_warnings = 0;
    }
}
