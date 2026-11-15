// SPDX-License-Identifier: Apache-2.0 AND IJG
//! JPEG files read and written as the Independent JPEG Group's library, release 6b of
//! 27-Mar-1998, reads and writes them: a Rust translation of its decompressor (every path a file
//! can drive) and of the compressor's path for one 8-bit grey component with `jpeg_set_defaults`.
//!
//! This software is based in part on the work of the Independent JPEG Group. IJG 6b's README,
//! with its copyright and no-warranty notice, is `LICENSES/IJG.txt`; copyright (C) 1991-1998,
//! Thomas G. Lane. The changes to the original files: the C library's modules are translated
//! into Rust (the decompressor's `jdapimin.c`, `jdapistd.c`, `jdatasrc.c`, `jdinput.c`,
//! `jdmaster.c`, `jdmarker.c`, `jdhuff.c`, `jdphuff.c`, `jdcoefct.c`, `jdmainct.c`,
//! `jddctmgr.c`, `jidctint.c`, `jdsample.c`, `jdcolor.c`, `jerror.c`, `jerror.h`, `jutils.c`;
//! the compressor's `jcapimin.c`, `jcapistd.c`, `jcparam.c`, `jcmaster.c`, `jcmarker.c`,
//! `jcprepct.c`, `jcsample.c`, `jccolor.c`, `jccoefct.c`, `jcdctmgr.c`, `jfdctint.c`,
//! `jchuff.c`, `jdatadst.c`), restructured where the structure decides no value or message (the
//! memory manager, suspension, the method tables, the row-group pointer lists); the options a
//! caller of the original could set and that no caller here sets (scaling, quantization, other
//! DCT methods, raw data, buffered-image mode) are left out; one correction (FS-813) is on by
//! default. `PROVENANCE.md` lists the files and what each became.
//!
//! - [`read_header`]: `jpeg_read_header(cinfo, TRUE)` on a file (`jpeg_stdio_src`): the image's
//!   size, components and colour space.
//! - [`decode_rgb`]: the decompressor with `out_color_space = JCS_RGB` and every other parameter
//!   at its default (`jpeg_read_header`, `jpeg_start_decompress`, `jpeg_read_scanlines` one row
//!   at a time, `jpeg_finish_decompress`): baseline, extended and progressive Huffman files,
//!   grey, YCbCr and RGB, any sampling factors IJG 6b accepts, restart markers, damaged and
//!   truncated data with IJG's recovery; the slow-but-accurate integer inverse DCT, fancy
//!   upsampling, block smoothing of incomplete progressive files.
//! - [`encode_gray`]: the compressor for one 8-bit grey component after `jpeg_set_defaults`
//!   (quality 75, the standard Huffman tables, the slow-but-accurate integer forward DCT, a JFIF
//!   header), written through `jpeg_stdio_dest`'s 4,096-byte buffer.
//!
//! Messages are IJG's standard error manager's (`jpeg_std_error`): the first warning of an
//! object is written to standard error, later warnings are counted only, trace messages are not
//! written (trace level 0); an error writes its message and the function returns [`Fatal`]: the
//! original's `error_exit` then ends the program with status 1 (`EXIT_FAILURE`), which is the
//! caller's to do. Each message is followed by a newline.
//!
//! The behaviour is proved by the differential probe `validation/probes/fs-jpeg` against IJG 6b
//! built from its release and FreeSurfer 8.2.0's copy of it (its `packages/jpeg`, IJG 6b but one
//! added `#include` in `jerror.c`).

mod decode;
mod encode;
mod msg;
#[cfg(test)]
mod tests;

pub use decode::{ColorSpace, Header, Options, decode_rgb, read_header};
pub use encode::encode_gray;
pub use msg::Fatal;

/// IJG 6b's version string (`JVERSION`).
pub const JVERSION: &str = "6b  27-Mar-1998";

#[doc(hidden)]
pub mod cases;
