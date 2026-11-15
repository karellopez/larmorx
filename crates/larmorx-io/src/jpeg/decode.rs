// SPDX-License-Identifier: Apache-2.0 AND IJG
//! IJG 6b's decompressor (this software is based in part on the work of the Independent JPEG
//! Group; `LICENSES/IJG.txt`): the source manager (`jdatasrc.c`), the marker reader
//! (`jdmarker.c`), the input controller (`jdinput.c`), the master's choices (`jdmaster.c`,
//! `jdapimin.c`, `jdapistd.c`), the Huffman decoders (`jdhuff.c`, `jdphuff.c`), the coefficient
//! controller with block smoothing (`jdcoefct.c`), the inverse DCT (`jddctmgr.c`, `jidctint.c`),
//! the upsampler (`jdsample.c`) and the colour converter (`jdcolor.c`).
//!
//! The row-group pointer lists of the main controller (`jdmainct.c`) become their effect: a
//! component row above the first or below the last real one is that first or last row. The
//! sample rows of three iMCU rows are kept per component (the one above, the one being output,
//! the one below); the coefficients of the whole image only when the file has several scans, as
//! IJG keeps them.

use larmorx_core::cfmt::Streams;
use larmorx_core::defects::{FS_813, Notices, UpstreamDefects};

use super::msg::*;

const DCTSIZE2: usize = 64;
const NUM_QUANT_TBLS: usize = 4;
const NUM_HUFF_TBLS: usize = 4;
const MAX_COMPS_IN_SCAN: usize = 4;
const MAX_COMPONENTS: i32 = 10;
const MAX_SAMP_FACTOR: i32 = 4;
const D_MAX_BLOCKS_IN_MCU: usize = 10;
const JPEG_MAX_DIMENSION: u32 = 65500;
const HUFF_LOOKAHEAD: i32 = 8;
const MIN_GET_BITS: i32 = 25;

/// `jpeg_natural_order` (`jutils.c`), with its 16 extra entries "for safety in decoder".
const NATURAL_ORDER: [usize; DCTSIZE2 + 16] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20,
    13, 6, 7, 14, 21, 28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59,
    52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63, 63, 63, 63, 63, 63, 63, 63, 63, 63, 63,
    63, 63, 63, 63, 63, 63,
];

const EXTEND_TEST: [i32; 16] = [
    0, 0x0001, 0x0002, 0x0004, 0x0008, 0x0010, 0x0020, 0x0040, 0x0080, 0x0100, 0x0200, 0x0400,
    0x0800, 0x1000, 0x2000, 0x4000,
];

/// `HUFF_EXTEND` with its tables (`(-1 << s) + 1` for s of 1 to 15).
fn huff_extend(x: i32, s: i32) -> i32 {
    if x < EXTEND_TEST[s as usize] {
        x + ((-1i32).wrapping_shl(s as u32) + 1)
    } else {
        x
    }
}

/// A JPEG colour space (`J_COLOR_SPACE`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorSpace {
    /// `JCS_UNKNOWN`.
    Unknown,
    /// `JCS_GRAYSCALE`.
    Grayscale,
    /// `JCS_RGB`.
    Rgb,
    /// `JCS_YCbCr`.
    YCbCr,
    /// `JCS_CMYK`.
    Cmyk,
    /// `JCS_YCCK`.
    Ycck,
}

/// What `jpeg_read_header` leaves in the decompress object.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Header {
    /// `image_width`, `image_height`.
    pub image_width: u32,
    pub image_height: u32,
    /// `num_components`.
    pub num_components: i32,
    /// `data_precision`.
    pub data_precision: i32,
    /// `jpeg_color_space` as `default_decompress_parms` guesses it.
    pub jpeg_color_space: ColorSpace,
    /// `progressive_mode`, `arith_code`.
    pub progressive: bool,
    pub arith_code: bool,
    /// Each component's identifier and sampling factors.
    pub components: Vec<(i32, i32, i32)>,
    /// The JFIF marker's fields (`saw_JFIF_marker`, version, `density_unit`, densities).
    pub saw_jfif_marker: bool,
    pub jfif_version: (u8, u8),
    pub density_unit: u8,
    pub x_density: u16,
    pub y_density: u16,
    /// The Adobe marker (`saw_Adobe_marker`, `Adobe_transform`).
    pub saw_adobe_marker: bool,
    pub adobe_transform: u8,
}

/// The decoder's settings.
#[derive(Clone, Copy, Debug, Default)]
pub struct Options {
    /// D13's switch (FS-813).
    pub upstream_defects: UpstreamDefects,
    /// The largest number of bytes one allocation of the coefficient buffer may take, the
    /// machine's memory as the caller models it: past it the decoder ends as IJG's memory manager
    /// does when `malloc` fails (`Insufficient memory (case 4)`). `None`: no limit.
    pub max_alloc: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ret {
    ReachedSos,
    ReachedEoi,
    RowCompleted,
    ScanCompleted,
}

/// `JHUFF_TBL`.
#[derive(Clone)]
struct HuffTbl {
    bits: [u8; 17],
    huffval: [u8; 256],
}

/// `d_derived_tbl`.
#[derive(Clone)]
struct Derived {
    maxcode: [i64; 18],
    valoffset: [i64; 17],
    look_nbits: [i32; 256],
    look_sym: [u8; 256],
    huffval: [u8; 256],
}

/// `jpeg_component_info`, the decompressor's fields.
#[derive(Clone)]
struct Comp {
    id: i32,
    index: usize,
    h: i32,
    v: i32,
    quant_tbl_no: i32,
    dc_tbl_no: i32,
    ac_tbl_no: i32,
    width_in_blocks: u32,
    height_in_blocks: u32,
    downsampled_width: u32,
    downsampled_height: u32,
    mcu_width: i32,
    mcu_height: i32,
    mcu_blocks: i32,
    last_col_width: i32,
    last_row_height: i32,
    /// `quant_table`: latched at the component's first scan.
    quant: Option<[u16; DCTSIZE2]>,
    /// `dct_table` (the slow integer IDCT's multipliers: the quantization values).
    dct_table: [i32; DCTSIZE2],
}

impl Default for Comp {
    fn default() -> Self {
        Comp {
            id: 0,
            index: 0,
            h: 0,
            v: 0,
            quant_tbl_no: 0,
            dc_tbl_no: 0,
            ac_tbl_no: 0,
            width_in_blocks: 0,
            height_in_blocks: 0,
            downsampled_width: 0,
            downsampled_height: 0,
            mcu_width: 0,
            mcu_height: 0,
            mcu_blocks: 0,
            last_col_width: 0,
            last_row_height: 0,
            quant: None,
            dct_table: [0; DCTSIZE2],
        }
    }
}

/// `jpeg_stdio_src` over a file's bytes: the whole file is one buffer; past its end each fill
/// warns (`Premature end of JPEG file`) and gives a fake EOI marker; an empty file is
/// `Empty input file`.
struct Src<'a> {
    data: &'a [u8],
    handed: bool,
    fake: bool,
    next: usize,
    start_of_file: bool,
}

const FAKE_EOI: [u8; 2] = [0xFF, 0xD9];

impl<'a> Src<'a> {
    fn buf(&self) -> &[u8] {
        if self.fake {
            &FAKE_EOI
        } else if self.handed {
            self.data
        } else {
            &[]
        }
    }
    fn bytes_in_buffer(&self) -> usize {
        self.buf().len() - self.next
    }
}

/// The decompress object.
struct D<'a, 's> {
    src: Src<'a>,
    err: ErrMgr,
    streams: &'s mut dyn Streams,
    opts: Options,
    notices: Notices,
    // marker reader
    saw_soi: bool,
    saw_sof: bool,
    discarded_bytes: u32,
    next_restart_num: i32,
    unread_marker: i32,
    // image
    image_width: u32,
    image_height: u32,
    data_precision: i32,
    num_components: i32,
    comps: Vec<Comp>,
    progressive: bool,
    arith: bool,
    restart_interval: u32,
    jpeg_color_space: ColorSpace,
    saw_jfif: bool,
    jfif_major: u8,
    jfif_minor: u8,
    density_unit: u8,
    x_density: u16,
    y_density: u16,
    saw_adobe: bool,
    adobe_transform: u8,
    quant_tbls: [Option<[u16; DCTSIZE2]>; NUM_QUANT_TBLS],
    dc_huff: [Option<HuffTbl>; NUM_HUFF_TBLS],
    ac_huff: [Option<HuffTbl>; NUM_HUFF_TBLS],
    // scan
    input_scan_number: i32,
    output_scan_number: i32,
    comps_in_scan: usize,
    cur_comp: [usize; MAX_COMPS_IN_SCAN],
    ss: i32,
    se: i32,
    ah: i32,
    al: i32,
    // input controller
    inheaders: bool,
    has_multiple_scans: bool,
    eoi_reached: bool,
    in_scan: bool,
    max_h: i32,
    max_v: i32,
    total_imcu_rows: u32,
    mcus_per_row: u32,
    blocks_in_mcu: usize,
    mcu_membership: [usize; D_MAX_BLOCKS_IN_MCU],
    coef_bits: Option<Vec<[i32; DCTSIZE2]>>,
    // entropy decoder
    get_buffer: u64,
    bits_left: i32,
    insufficient_data: bool,
    restarts_to_go: u32,
    last_dc_val: [i32; MAX_COMPS_IN_SCAN],
    eobrun: u32,
    dc_derived: [Option<Derived>; NUM_HUFF_TBLS],
    ac_derived: [Option<Derived>; NUM_HUFF_TBLS],
    cur_dc: [usize; D_MAX_BLOCKS_IN_MCU],
    cur_ac: [usize; D_MAX_BLOCKS_IN_MCU],
    phuff_ac: usize,
    // coefficient controller
    input_imcu_row: u32,
    mcu_rows_per_imcu_row: i32,
    /// The whole image's coefficients (several scans): each component's blocks from
    /// `coef_start[c]`, `coef_stride[c]` blocks per block row (rounded up to its sampling).
    coefs: Vec<[i16; DCTSIZE2]>,
    coef_start: Vec<usize>,
    coef_stride: Vec<usize>,
}

/// One block's slot for a decoder: the MCU buffer or the whole image's coefficients.
type Blocks = [[i16; DCTSIZE2]];

impl<'a, 's> D<'a, 's> {
    fn new(data: &'a [u8], streams: &'s mut dyn Streams, opts: Options) -> Self {
        D {
            src: Src {
                data,
                handed: false,
                fake: false,
                next: 0,
                start_of_file: true,
            },
            err: ErrMgr::default(),
            streams,
            opts,
            notices: Notices::new(opts.upstream_defects),
            saw_soi: false,
            saw_sof: false,
            discarded_bytes: 0,
            next_restart_num: 0,
            unread_marker: 0,
            image_width: 0,
            image_height: 0,
            data_precision: 0,
            num_components: 0,
            comps: Vec::new(),
            progressive: false,
            arith: false,
            restart_interval: 0,
            jpeg_color_space: ColorSpace::Unknown,
            saw_jfif: false,
            jfif_major: 1,
            jfif_minor: 1,
            density_unit: 0,
            x_density: 1,
            y_density: 1,
            saw_adobe: false,
            adobe_transform: 0,
            quant_tbls: [None; NUM_QUANT_TBLS],
            dc_huff: [None, None, None, None],
            ac_huff: [None, None, None, None],
            input_scan_number: 0,
            output_scan_number: 0,
            comps_in_scan: 0,
            cur_comp: [0; MAX_COMPS_IN_SCAN],
            ss: 0,
            se: 0,
            ah: 0,
            al: 0,
            inheaders: true,
            has_multiple_scans: false,
            eoi_reached: false,
            in_scan: false,
            max_h: 1,
            max_v: 1,
            total_imcu_rows: 0,
            mcus_per_row: 0,
            blocks_in_mcu: 0,
            mcu_membership: [0; D_MAX_BLOCKS_IN_MCU],
            coef_bits: None,
            get_buffer: 0,
            bits_left: 0,
            insufficient_data: false,
            restarts_to_go: 0,
            last_dc_val: [0; MAX_COMPS_IN_SCAN],
            eobrun: 0,
            dc_derived: [None, None, None, None],
            ac_derived: [None, None, None, None],
            cur_dc: [0; D_MAX_BLOCKS_IN_MCU],
            cur_ac: [0; D_MAX_BLOCKS_IN_MCU],
            phuff_ac: 0,
            input_imcu_row: 0,
            mcu_rows_per_imcu_row: 0,
            coefs: Vec::new(),
            coef_start: Vec::new(),
            coef_stride: Vec::new(),
        }
    }

    fn fatal(&mut self, code: usize, p: &[i32]) -> Fatal {
        self.err.fatal(self.streams, code, p)
    }

    fn warn(&mut self, code: usize, p: &[i32]) {
        self.err.warn(self.streams, code, p);
    }

    // ---- jdatasrc.c ----

    /// `fill_input_buffer`.
    fn fill(&mut self) -> Result<(), Fatal> {
        if !self.src.handed {
            self.src.handed = true;
            if !self.src.data.is_empty() {
                self.src.next = 0;
                self.src.start_of_file = false;
                return Ok(());
            }
        }
        if self.src.start_of_file {
            return Err(self.fatal(JERR_INPUT_EMPTY, &[]));
        }
        self.warn(JWRN_JPEG_EOF, &[]);
        self.src.fake = true;
        self.src.next = 0;
        self.src.start_of_file = false;
        Ok(())
    }

    /// `INPUT_BYTE`.
    fn byte(&mut self) -> Result<i32, Fatal> {
        if self.src.bytes_in_buffer() == 0 {
            self.fill()?;
        }
        let v = self.src.buf()[self.src.next];
        self.src.next += 1;
        Ok(v as i32)
    }

    /// `INPUT_2BYTES`.
    fn two_bytes(&mut self) -> Result<i64, Fatal> {
        let hi = self.byte()? as i64;
        let lo = self.byte()? as i64;
        Ok((hi << 8) + lo)
    }

    /// `skip_input_data`.
    fn skip(&mut self, mut n: i64) -> Result<(), Fatal> {
        if n > 0 {
            while n > self.src.bytes_in_buffer() as i64 {
                n -= self.src.bytes_in_buffer() as i64;
                self.fill()?;
            }
            self.src.next += n as usize;
        }
        Ok(())
    }

    // ---- jdmarker.c ----

    fn get_soi(&mut self) -> Result<(), Fatal> {
        if self.saw_soi {
            return Err(self.fatal(JERR_SOI_DUPLICATE, &[]));
        }
        self.restart_interval = 0;
        self.jpeg_color_space = ColorSpace::Unknown;
        self.saw_jfif = false;
        self.jfif_major = 1;
        self.jfif_minor = 1;
        self.density_unit = 0;
        self.x_density = 1;
        self.y_density = 1;
        self.saw_adobe = false;
        self.adobe_transform = 0;
        self.saw_soi = true;
        Ok(())
    }

    fn get_sof(&mut self, is_prog: bool, is_arith: bool) -> Result<(), Fatal> {
        self.progressive = is_prog;
        self.arith = is_arith;
        let mut length = self.two_bytes()?;
        self.data_precision = self.byte()?;
        self.image_height = self.two_bytes()? as u32;
        self.image_width = self.two_bytes()? as u32;
        self.num_components = self.byte()?;
        length -= 8;
        if self.saw_sof {
            return Err(self.fatal(JERR_SOF_DUPLICATE, &[]));
        }
        if self.image_height == 0 || self.image_width == 0 || self.num_components <= 0 {
            return Err(self.fatal(JERR_EMPTY_IMAGE, &[]));
        }
        if length != (self.num_components * 3) as i64 {
            return Err(self.fatal(JERR_BAD_LENGTH, &[]));
        }
        if self.comps.is_empty() {
            self.comps = vec![Comp::default(); self.num_components as usize];
        }
        for ci in 0..self.num_components as usize {
            let id = self.byte()?;
            let c = self.byte()?;
            let q = self.byte()?;
            let comp = &mut self.comps[ci];
            comp.index = ci;
            comp.id = id;
            comp.h = (c >> 4) & 15;
            comp.v = c & 15;
            comp.quant_tbl_no = q;
        }
        self.saw_sof = true;
        Ok(())
    }

    fn get_sos(&mut self) -> Result<(), Fatal> {
        if !self.saw_sof {
            return Err(self.fatal(JERR_SOS_NO_SOF, &[]));
        }
        let length = self.two_bytes()?;
        let n = self.byte()?;
        if length != (n * 2 + 6) as i64 || n < 1 || n > MAX_COMPS_IN_SCAN as i32 {
            return Err(self.fatal(JERR_BAD_LENGTH, &[]));
        }
        self.comps_in_scan = n as usize;
        for i in 0..n as usize {
            let cc = self.byte()?;
            let c = self.byte()?;
            let Some(ci) = (0..self.num_components as usize).find(|&ci| self.comps[ci].id == cc)
            else {
                return Err(self.fatal(JERR_BAD_COMPONENT_ID, &[cc]));
            };
            if self.cur_comp[..i].contains(&ci) {
                // FS-813: a component twice in one scan: refused (rule B), or decoded as IJG 6b
                // decodes it
                let detail = format!(
                    "component {cc} named twice in scan {}{}",
                    self.input_scan_number + 1,
                    if self.notices.fix() {
                        ": the file is refused"
                    } else {
                        ""
                    }
                );
                self.notices.add(&FS_813, detail);
                if self.notices.fix() {
                    return Err(self.fatal(JERR_BAD_COMPONENT_ID, &[cc]));
                }
            }
            self.cur_comp[i] = ci;
            self.comps[ci].dc_tbl_no = (c >> 4) & 15;
            self.comps[ci].ac_tbl_no = c & 15;
        }
        self.ss = self.byte()?;
        self.se = self.byte()?;
        let c = self.byte()?;
        self.ah = (c >> 4) & 15;
        self.al = c & 15;
        self.next_restart_num = 0;
        self.input_scan_number += 1;
        Ok(())
    }

    fn get_dht(&mut self) -> Result<(), Fatal> {
        let mut length = self.two_bytes()? - 2;
        while length > 16 {
            let mut index = self.byte()?;
            let mut bits = [0u8; 17];
            let mut count = 0i64;
            for b in bits.iter_mut().skip(1) {
                *b = self.byte()? as u8;
                count += *b as i64;
            }
            length -= 1 + 16;
            if count > 256 || count > length {
                return Err(self.fatal(JERR_BAD_HUFF_TABLE, &[]));
            }
            let mut huffval = [0u8; 256];
            for h in huffval.iter_mut().take(count as usize) {
                *h = self.byte()? as u8;
            }
            length -= count;
            let ac = index & 0x10 != 0;
            if ac {
                index -= 0x10;
            }
            if !(0..NUM_HUFF_TBLS as i32).contains(&index) {
                return Err(self.fatal(JERR_DHT_INDEX, &[index]));
            }
            let t = Some(HuffTbl { bits, huffval });
            if ac {
                self.ac_huff[index as usize] = t;
            } else {
                self.dc_huff[index as usize] = t;
            }
        }
        if length != 0 {
            return Err(self.fatal(JERR_BAD_LENGTH, &[]));
        }
        Ok(())
    }

    fn get_dqt(&mut self) -> Result<(), Fatal> {
        let mut length = self.two_bytes()? - 2;
        while length > 0 {
            let mut n = self.byte()?;
            let prec = n >> 4;
            n &= 0x0F;
            if n >= NUM_QUANT_TBLS as i32 {
                return Err(self.fatal(JERR_DQT_INDEX, &[n]));
            }
            let mut q = self.quant_tbls[n as usize].unwrap_or([0; DCTSIZE2]);
            for &pos in NATURAL_ORDER.iter().take(DCTSIZE2) {
                let tmp = if prec != 0 {
                    self.two_bytes()?
                } else {
                    self.byte()? as i64
                };
                q[pos] = tmp as u16;
            }
            self.quant_tbls[n as usize] = Some(q);
            length -= DCTSIZE2 as i64 + 1;
            if prec != 0 {
                length -= DCTSIZE2 as i64;
            }
        }
        if length != 0 {
            return Err(self.fatal(JERR_BAD_LENGTH, &[]));
        }
        Ok(())
    }

    fn get_dri(&mut self) -> Result<(), Fatal> {
        let length = self.two_bytes()?;
        if length != 4 {
            return Err(self.fatal(JERR_BAD_LENGTH, &[]));
        }
        self.restart_interval = self.two_bytes()? as u32;
        Ok(())
    }

    /// `examine_app0`.
    fn examine_app0(&mut self, data: &[u8]) {
        if data.len() >= 14 && data[..5] == *b"JFIF\0" {
            self.saw_jfif = true;
            self.jfif_major = data[5];
            self.jfif_minor = data[6];
            self.density_unit = data[7];
            self.x_density = ((data[8] as u16) << 8) + data[9] as u16;
            self.y_density = ((data[10] as u16) << 8) + data[11] as u16;
            if self.jfif_major != 1 {
                let (a, b) = (self.jfif_major as i32, self.jfif_minor as i32);
                self.warn(JWRN_JFIF_MAJOR, &[a, b]);
            }
        }
    }

    /// `examine_app14`.
    fn examine_app14(&mut self, data: &[u8]) {
        if data.len() >= 12 && data[..5] == *b"Adobe" {
            self.saw_adobe = true;
            self.adobe_transform = data[11];
        }
    }

    /// `get_interesting_appn` (APP0 and APP14).
    fn get_interesting_appn(&mut self) -> Result<(), Fatal> {
        let mut length = self.two_bytes()? - 2;
        let numtoread = length.clamp(0, 14) as usize;
        let mut b = [0u8; 14];
        for x in b.iter_mut().take(numtoread) {
            *x = self.byte()? as u8;
        }
        length -= numtoread as i64;
        if self.unread_marker == 0xE0 {
            self.examine_app0(&b[..numtoread]);
        } else {
            self.examine_app14(&b[..numtoread]);
        }
        if length > 0 {
            self.skip(length)?;
        }
        Ok(())
    }

    /// `skip_variable`.
    fn skip_variable(&mut self) -> Result<(), Fatal> {
        let length = self.two_bytes()? - 2;
        if length > 0 {
            self.skip(length)?;
        }
        Ok(())
    }

    /// `next_marker`.
    fn next_marker(&mut self) -> Result<(), Fatal> {
        let mut c;
        loop {
            c = self.byte()?;
            while c != 0xFF {
                self.discarded_bytes = self.discarded_bytes.wrapping_add(1);
                c = self.byte()?;
            }
            loop {
                c = self.byte()?;
                if c != 0xFF {
                    break;
                }
            }
            if c != 0 {
                break;
            }
            self.discarded_bytes = self.discarded_bytes.wrapping_add(2);
        }
        if self.discarded_bytes != 0 {
            let d = self.discarded_bytes as i32;
            self.warn(JWRN_EXTRANEOUS_DATA, &[d, c]);
            self.discarded_bytes = 0;
        }
        self.unread_marker = c;
        Ok(())
    }

    /// `first_marker`.
    fn first_marker(&mut self) -> Result<(), Fatal> {
        let c = self.byte()?;
        let c2 = self.byte()?;
        if c != 0xFF || c2 != 0xD8 {
            return Err(self.fatal(JERR_NO_SOI, &[c, c2]));
        }
        self.unread_marker = c2;
        Ok(())
    }

    /// `read_markers`.
    fn read_markers(&mut self) -> Result<Ret, Fatal> {
        loop {
            if self.unread_marker == 0 {
                if !self.saw_soi {
                    self.first_marker()?;
                } else {
                    self.next_marker()?;
                }
            }
            match self.unread_marker {
                0xD8 => self.get_soi()?,
                0xC0 | 0xC1 => self.get_sof(false, false)?,
                0xC2 => self.get_sof(true, false)?,
                0xC9 => self.get_sof(false, true)?,
                0xCA => self.get_sof(true, true)?,
                0xC3 | 0xC5 | 0xC6 | 0xC7 | 0xC8 | 0xCB | 0xCD | 0xCE | 0xCF => {
                    let m = self.unread_marker;
                    return Err(self.fatal(JERR_SOF_UNSUPPORTED, &[m]));
                }
                0xDA => {
                    self.get_sos()?;
                    self.unread_marker = 0;
                    return Ok(Ret::ReachedSos);
                }
                0xD9 => {
                    self.unread_marker = 0;
                    return Ok(Ret::ReachedEoi);
                }
                // DAC: arithmetic coding is not compiled (get_dac is skip_variable)
                0xCC => self.skip_variable()?,
                0xC4 => self.get_dht()?,
                0xDB => self.get_dqt()?,
                0xDD => self.get_dri()?,
                0xE0 | 0xEE => self.get_interesting_appn()?,
                0xE1..=0xED | 0xEF | 0xFE => self.skip_variable()?,
                0xD0..=0xD7 | 0x01 => {}
                0xDC => self.skip_variable()?,
                m => return Err(self.fatal(JERR_UNKNOWN_MARKER, &[m])),
            }
            self.unread_marker = 0;
        }
    }

    /// `read_restart_marker`.
    fn read_restart_marker(&mut self) -> Result<(), Fatal> {
        if self.unread_marker == 0 {
            self.next_marker()?;
        }
        if self.unread_marker == 0xD0 + self.next_restart_num {
            self.unread_marker = 0;
        } else {
            self.resync_to_restart(self.next_restart_num)?;
        }
        self.next_restart_num = (self.next_restart_num + 1) & 7;
        Ok(())
    }

    /// `jpeg_resync_to_restart`.
    fn resync_to_restart(&mut self, desired: i32) -> Result<(), Fatal> {
        let mut marker = self.unread_marker;
        self.warn(JWRN_MUST_RESYNC, &[marker, desired]);
        loop {
            let action = if marker < 0xC0 {
                2
            } else if !(0xD0..=0xD7).contains(&marker) {
                3
            } else if marker == 0xD0 + ((desired + 1) & 7) || marker == 0xD0 + ((desired + 2) & 7) {
                3
            } else if marker == 0xD0 + ((desired - 1) & 7) || marker == 0xD0 + ((desired - 2) & 7) {
                2
            } else {
                1
            };
            match action {
                1 => {
                    self.unread_marker = 0;
                    return Ok(());
                }
                2 => {
                    self.next_marker()?;
                    marker = self.unread_marker;
                }
                _ => return Ok(()),
            }
        }
    }

    // ---- jdinput.c ----

    /// `initial_setup`.
    fn initial_setup(&mut self) -> Result<(), Fatal> {
        if self.image_height > JPEG_MAX_DIMENSION || self.image_width > JPEG_MAX_DIMENSION {
            return Err(self.fatal(JERR_IMAGE_TOO_BIG, &[JPEG_MAX_DIMENSION as i32]));
        }
        if self.data_precision != 8 {
            let p = self.data_precision;
            return Err(self.fatal(JERR_BAD_PRECISION, &[p]));
        }
        if self.num_components > MAX_COMPONENTS {
            let n = self.num_components;
            return Err(self.fatal(JERR_COMPONENT_COUNT, &[n, MAX_COMPONENTS]));
        }
        self.max_h = 1;
        self.max_v = 1;
        for ci in 0..self.num_components as usize {
            let (h, v) = (self.comps[ci].h, self.comps[ci].v);
            if h <= 0 || h > MAX_SAMP_FACTOR || v <= 0 || v > MAX_SAMP_FACTOR {
                return Err(self.fatal(JERR_BAD_SAMPLING, &[]));
            }
            self.max_h = self.max_h.max(h);
            self.max_v = self.max_v.max(v);
        }
        let (w, hh) = (self.image_width as u64, self.image_height as u64);
        let (mh, mv) = (self.max_h as u64, self.max_v as u64);
        for c in self.comps.iter_mut() {
            c.width_in_blocks = div_round_up(w * c.h as u64, mh * 8) as u32;
            c.height_in_blocks = div_round_up(hh * c.v as u64, mv * 8) as u32;
            c.downsampled_width = div_round_up(w * c.h as u64, mh) as u32;
            c.downsampled_height = div_round_up(hh * c.v as u64, mv) as u32;
            c.quant = None;
        }
        self.total_imcu_rows = div_round_up(hh, mv * 8) as u32;
        self.has_multiple_scans =
            (self.comps_in_scan as i32) < self.num_components || self.progressive;
        Ok(())
    }

    /// `per_scan_setup`.
    fn per_scan_setup(&mut self) -> Result<(), Fatal> {
        if self.comps_in_scan == 1 {
            let c = &mut self.comps[self.cur_comp[0]];
            self.mcus_per_row = c.width_in_blocks;
            c.mcu_width = 1;
            c.mcu_height = 1;
            c.mcu_blocks = 1;
            c.last_col_width = 1;
            let mut tmp = (c.height_in_blocks % c.v as u32) as i32;
            if tmp == 0 {
                tmp = c.v;
            }
            c.last_row_height = tmp;
            self.blocks_in_mcu = 1;
            self.mcu_membership[0] = 0;
        } else {
            self.mcus_per_row = div_round_up(self.image_width as u64, self.max_h as u64 * 8) as u32;
            self.blocks_in_mcu = 0;
            for ci in 0..self.comps_in_scan {
                let c = &mut self.comps[self.cur_comp[ci]];
                c.mcu_width = c.h;
                c.mcu_height = c.v;
                c.mcu_blocks = c.h * c.v;
                let mut tmp = (c.width_in_blocks % c.mcu_width as u32) as i32;
                if tmp == 0 {
                    tmp = c.mcu_width;
                }
                c.last_col_width = tmp;
                let mut tmp = (c.height_in_blocks % c.mcu_height as u32) as i32;
                if tmp == 0 {
                    tmp = c.mcu_height;
                }
                c.last_row_height = tmp;
                let mcublks = c.mcu_blocks as usize;
                if self.blocks_in_mcu + mcublks > D_MAX_BLOCKS_IN_MCU {
                    return Err(self.fatal(JERR_BAD_MCU_SIZE, &[]));
                }
                for _ in 0..mcublks {
                    self.mcu_membership[self.blocks_in_mcu] = ci;
                    self.blocks_in_mcu += 1;
                }
            }
        }
        Ok(())
    }

    /// `latch_quant_tables`.
    fn latch_quant_tables(&mut self) -> Result<(), Fatal> {
        for ci in 0..self.comps_in_scan {
            let k = self.cur_comp[ci];
            if self.comps[k].quant.is_some() {
                continue;
            }
            let q = self.comps[k].quant_tbl_no;
            let tbl = if (0..NUM_QUANT_TBLS as i32).contains(&q) {
                self.quant_tbls[q as usize]
            } else {
                None
            };
            let Some(t) = tbl else {
                return Err(self.fatal(JERR_NO_QUANT_TABLE, &[q]));
            };
            self.comps[k].quant = Some(t);
        }
        Ok(())
    }

    /// `start_input_pass`.
    fn start_input_pass(&mut self) -> Result<(), Fatal> {
        self.per_scan_setup()?;
        self.latch_quant_tables()?;
        if self.progressive {
            self.start_pass_phuff()?;
        } else {
            self.start_pass_huff()?;
        }
        self.input_imcu_row = 0;
        self.start_imcu_row();
        self.in_scan = true;
        Ok(())
    }

    /// `consume_markers`.
    fn consume_markers(&mut self) -> Result<Ret, Fatal> {
        if self.eoi_reached {
            return Ok(Ret::ReachedEoi);
        }
        let val = self.read_markers()?;
        match val {
            Ret::ReachedSos => {
                if self.inheaders {
                    self.initial_setup()?;
                    self.inheaders = false;
                } else {
                    if !self.has_multiple_scans {
                        return Err(self.fatal(JERR_EOI_EXPECTED, &[]));
                    }
                    self.start_input_pass()?;
                }
            }
            Ret::ReachedEoi => {
                self.eoi_reached = true;
                if self.inheaders {
                    if self.saw_sof {
                        return Err(self.fatal(JERR_SOF_NO_SOS, &[]));
                    }
                } else if self.output_scan_number > self.input_scan_number {
                    self.output_scan_number = self.input_scan_number;
                }
            }
            _ => {}
        }
        Ok(val)
    }

    /// `consume_input` in the multi-scan states: the scan's data, or markers between scans.
    fn consume_input(&mut self) -> Result<Ret, Fatal> {
        if self.in_scan {
            self.consume_data()
        } else {
            self.consume_markers()
        }
    }

    /// `default_decompress_parms`'s colour space.
    fn default_decompress_parms(&mut self) {
        self.jpeg_color_space = match self.num_components {
            1 => ColorSpace::Grayscale,
            3 => {
                if self.saw_jfif {
                    ColorSpace::YCbCr
                } else if self.saw_adobe {
                    match self.adobe_transform {
                        0 => ColorSpace::Rgb,
                        1 => ColorSpace::YCbCr,
                        t => {
                            self.warn(JWRN_ADOBE_XFORM, &[t as i32]);
                            ColorSpace::YCbCr
                        }
                    }
                } else {
                    let ids = (self.comps[0].id, self.comps[1].id, self.comps[2].id);
                    if ids == (82, 71, 66) {
                        ColorSpace::Rgb
                    } else {
                        ColorSpace::YCbCr
                    }
                }
            }
            4 => {
                if self.saw_adobe {
                    match self.adobe_transform {
                        0 => ColorSpace::Cmyk,
                        2 => ColorSpace::Ycck,
                        t => {
                            self.warn(JWRN_ADOBE_XFORM, &[t as i32]);
                            ColorSpace::Ycck
                        }
                    }
                } else {
                    ColorSpace::Cmyk
                }
            }
            _ => ColorSpace::Unknown,
        };
    }

    /// `jpeg_read_header(cinfo, TRUE)`.
    fn read_header(&mut self) -> Result<(), Fatal> {
        // reset_input_controller (with reset_error_mgr and reset_marker_reader), init_source
        self.err.reset();
        match self.consume_markers()? {
            Ret::ReachedSos => {
                self.default_decompress_parms();
                Ok(())
            }
            _ => Err(self.fatal(JERR_NO_IMAGE, &[])),
        }
    }

    fn header(&self) -> Header {
        Header {
            image_width: self.image_width,
            image_height: self.image_height,
            num_components: self.num_components,
            data_precision: self.data_precision,
            jpeg_color_space: self.jpeg_color_space,
            progressive: self.progressive,
            arith_code: self.arith,
            components: self.comps.iter().map(|c| (c.id, c.h, c.v)).collect(),
            saw_jfif_marker: self.saw_jfif,
            jfif_version: (self.jfif_major, self.jfif_minor),
            density_unit: self.density_unit,
            x_density: self.x_density,
            y_density: self.y_density,
            saw_adobe_marker: self.saw_adobe,
            adobe_transform: self.adobe_transform,
        }
    }

    // ---- jdhuff.c ----

    /// `jpeg_make_d_derived_tbl`.
    fn make_derived(&mut self, is_dc: bool, tblno: i32) -> Result<Derived, Fatal> {
        if !(0..NUM_HUFF_TBLS as i32).contains(&tblno) {
            return Err(self.fatal(JERR_NO_HUFF_TABLE, &[tblno]));
        }
        let tbl = if is_dc { &self.dc_huff } else { &self.ac_huff }[tblno as usize].clone();
        let Some(htbl) = tbl else {
            return Err(self.fatal(JERR_NO_HUFF_TABLE, &[tblno]));
        };
        let mut huffsize = [0u8; 257];
        let mut huffcode = [0u32; 257];
        let mut p = 0usize;
        for l in 1..=16u8 {
            let i = htbl.bits[l as usize] as usize;
            if p + i > 256 {
                return Err(self.fatal(JERR_BAD_HUFF_TABLE, &[]));
            }
            for _ in 0..i {
                huffsize[p] = l;
                p += 1;
            }
        }
        huffsize[p] = 0;
        let numsymbols = p;
        let mut code: u32 = 0;
        let mut si = huffsize[0] as u32;
        p = 0;
        while huffsize[p] != 0 {
            while huffsize[p] as u32 == si {
                huffcode[p] = code;
                p += 1;
                code = code.wrapping_add(1);
            }
            if code as i64 >= (1i64 << si) {
                return Err(self.fatal(JERR_BAD_HUFF_TABLE, &[]));
            }
            code <<= 1;
            si += 1;
        }
        let mut d = Derived {
            maxcode: [0; 18],
            valoffset: [0; 17],
            look_nbits: [0; 256],
            look_sym: [0; 256],
            huffval: htbl.huffval,
        };
        p = 0;
        for l in 1..=16usize {
            if htbl.bits[l] != 0 {
                d.valoffset[l] = p as i64 - huffcode[p] as i64;
                p += htbl.bits[l] as usize;
                d.maxcode[l] = huffcode[p - 1] as i64;
            } else {
                d.maxcode[l] = -1;
            }
        }
        d.maxcode[17] = 0xFFFFF;
        p = 0;
        for l in 1..=HUFF_LOOKAHEAD as usize {
            for _ in 1..=htbl.bits[l] as usize {
                let mut lookbits = (huffcode[p] << (HUFF_LOOKAHEAD as usize - l)) as usize;
                for _ in 0..(1usize << (HUFF_LOOKAHEAD as usize - l)) {
                    d.look_nbits[lookbits] = l as i32;
                    d.look_sym[lookbits] = htbl.huffval[p];
                    lookbits += 1;
                }
                p += 1;
            }
        }
        if is_dc {
            for &sym in htbl.huffval.iter().take(numsymbols) {
                if sym > 15 {
                    return Err(self.fatal(JERR_BAD_HUFF_TABLE, &[]));
                }
            }
        }
        Ok(d)
    }

    /// `start_pass_huff_decoder`.
    fn start_pass_huff(&mut self) -> Result<(), Fatal> {
        if self.ss != 0 || self.se != 63 || self.ah != 0 || self.al != 0 {
            self.warn(JWRN_NOT_SEQUENTIAL, &[]);
        }
        for ci in 0..self.comps_in_scan {
            let k = self.cur_comp[ci];
            let (dctbl, actbl) = (self.comps[k].dc_tbl_no, self.comps[k].ac_tbl_no);
            let d = self.make_derived(true, dctbl)?;
            self.dc_derived[dctbl as usize] = Some(d);
            let a = self.make_derived(false, actbl)?;
            self.ac_derived[actbl as usize] = Some(a);
            self.last_dc_val[ci] = 0;
        }
        for blkn in 0..self.blocks_in_mcu {
            let k = self.cur_comp[self.mcu_membership[blkn]];
            self.cur_dc[blkn] = self.comps[k].dc_tbl_no as usize;
            self.cur_ac[blkn] = self.comps[k].ac_tbl_no as usize;
        }
        self.bits_left = 0;
        self.get_buffer = 0;
        self.insufficient_data = false;
        self.restarts_to_go = self.restart_interval;
        Ok(())
    }

    /// `jpeg_fill_bit_buffer`.
    fn fill_bit_buffer(&mut self, nbits: i32) -> Result<(), Fatal> {
        if self.unread_marker == 0 {
            while self.bits_left < MIN_GET_BITS {
                let mut c = self.byte()?;
                if c == 0xFF {
                    loop {
                        c = self.byte()?;
                        if c != 0xFF {
                            break;
                        }
                    }
                    if c == 0 {
                        c = 0xFF;
                    } else {
                        self.unread_marker = c;
                        return self.no_more_bytes(nbits);
                    }
                }
                self.get_buffer = (self.get_buffer << 8) | c as u64;
                self.bits_left += 8;
            }
            Ok(())
        } else {
            self.no_more_bytes(nbits)
        }
    }

    fn no_more_bytes(&mut self, nbits: i32) -> Result<(), Fatal> {
        if nbits > self.bits_left {
            if !self.insufficient_data {
                self.warn(JWRN_HIT_MARKER, &[]);
                self.insufficient_data = true;
            }
            self.get_buffer <<= MIN_GET_BITS - self.bits_left;
            self.bits_left = MIN_GET_BITS;
        }
        Ok(())
    }

    /// `CHECK_BIT_BUFFER`.
    fn check_bits(&mut self, nbits: i32) -> Result<(), Fatal> {
        if self.bits_left < nbits {
            self.fill_bit_buffer(nbits)?;
        }
        Ok(())
    }

    /// `GET_BITS`.
    fn get_bits(&mut self, nbits: i32) -> i32 {
        self.bits_left -= nbits;
        ((self.get_buffer >> self.bits_left) as u32 as i32) & ((1i32 << nbits) - 1)
    }

    /// `HUFF_DECODE` with `jpeg_huff_decode`.
    fn huff_decode(&mut self, tbl: &Derived) -> Result<i32, Fatal> {
        if self.bits_left < HUFF_LOOKAHEAD {
            self.fill_bit_buffer(0)?;
            if self.bits_left < HUFF_LOOKAHEAD {
                return self.huff_decode_slow(tbl, 1);
            }
        }
        let look = ((self.get_buffer >> (self.bits_left - HUFF_LOOKAHEAD)) as u32 as i32)
            & ((1 << HUFF_LOOKAHEAD) - 1);
        let nb = tbl.look_nbits[look as usize];
        if nb != 0 {
            self.bits_left -= nb;
            Ok(tbl.look_sym[look as usize] as i32)
        } else {
            self.huff_decode_slow(tbl, HUFF_LOOKAHEAD + 1)
        }
    }

    /// `jpeg_huff_decode`.
    fn huff_decode_slow(&mut self, tbl: &Derived, min_bits: i32) -> Result<i32, Fatal> {
        let mut l = min_bits;
        self.check_bits(l)?;
        let mut code = self.get_bits(l) as i64;
        while code > tbl.maxcode[l as usize] {
            code <<= 1;
            self.check_bits(1)?;
            code |= self.get_bits(1) as i64;
            l += 1;
        }
        if l > 16 {
            self.warn(JWRN_HUFF_BAD_CODE, &[]);
            return Ok(0);
        }
        Ok(tbl.huffval[(code + tbl.valoffset[l as usize]) as usize] as i32)
    }

    /// `process_restart` (both Huffman decoders).
    fn process_restart(&mut self) -> Result<(), Fatal> {
        self.discarded_bytes = self
            .discarded_bytes
            .wrapping_add((self.bits_left / 8) as u32);
        self.bits_left = 0;
        self.read_restart_marker()?;
        for ci in 0..self.comps_in_scan {
            self.last_dc_val[ci] = 0;
        }
        self.eobrun = 0;
        self.restarts_to_go = self.restart_interval;
        if self.unread_marker == 0 {
            self.insufficient_data = false;
        }
        Ok(())
    }

    /// `decode_mcu` (sequential): `idx[blkn]` is the slot of each block of the MCU in `blocks`.
    fn decode_mcu(&mut self, blocks: &mut Blocks, idx: &[usize]) -> Result<(), Fatal> {
        if self.restart_interval != 0 && self.restarts_to_go == 0 {
            self.process_restart()?;
        }
        if !self.insufficient_data {
            for blkn in 0..self.blocks_in_mcu {
                let dctbl = self.dc_derived[self.cur_dc[blkn]].take().expect("table");
                let actbl = self.ac_derived[self.cur_ac[blkn]].take().expect("table");
                let r = self.decode_block(blocks, idx[blkn], blkn, &dctbl, &actbl);
                self.dc_derived[self.cur_dc[blkn]] = Some(dctbl);
                self.ac_derived[self.cur_ac[blkn]] = Some(actbl);
                r?;
            }
        }
        self.restarts_to_go = self.restarts_to_go.wrapping_sub(1);
        Ok(())
    }

    fn decode_block(
        &mut self,
        blocks: &mut Blocks,
        slot: usize,
        blkn: usize,
        dctbl: &Derived,
        actbl: &Derived,
    ) -> Result<(), Fatal> {
        let mut s = self.huff_decode(dctbl)?;
        if s != 0 {
            self.check_bits(s)?;
            let r = self.get_bits(s);
            s = huff_extend(r, s);
        }
        let ci = self.mcu_membership[blkn];
        s = s.wrapping_add(self.last_dc_val[ci]);
        self.last_dc_val[ci] = s;
        blocks[slot][0] = s as i16;
        let mut k = 1usize;
        while k < DCTSIZE2 {
            let mut s = self.huff_decode(actbl)?;
            let r = s >> 4;
            s &= 15;
            if s != 0 {
                k += r as usize;
                self.check_bits(s)?;
                let r = self.get_bits(s);
                s = huff_extend(r, s);
                blocks[slot][NATURAL_ORDER[k]] = s as i16;
            } else {
                if r != 15 {
                    break;
                }
                k += 15;
            }
            k += 1;
        }
        Ok(())
    }

    // ---- jdphuff.c ----

    /// `start_pass_phuff_decoder`.
    fn start_pass_phuff(&mut self) -> Result<(), Fatal> {
        let is_dc_band = self.ss == 0;
        let mut bad = false;
        if is_dc_band {
            if self.se != 0 {
                bad = true;
            }
        } else {
            if self.ss > self.se || self.se >= DCTSIZE2 as i32 {
                bad = true;
            }
            if self.comps_in_scan != 1 {
                bad = true;
            }
        }
        if self.ah != 0 && self.al != self.ah - 1 {
            bad = true;
        }
        if self.al > 13 {
            bad = true;
        }
        if bad {
            let p = [self.ss, self.se, self.ah, self.al];
            return Err(self.fatal(JERR_BAD_PROGRESSION, &p));
        }
        for ci in 0..self.comps_in_scan {
            let cindex = self.comps[self.cur_comp[ci]].index;
            let mut warns = Vec::new();
            {
                let cb = &mut self.coef_bits.as_mut().expect("coef_bits")[cindex];
                if !is_dc_band && cb[0] < 0 {
                    warns.push((cindex as i32, 0));
                }
                for coefi in self.ss..=self.se {
                    let expected = cb[coefi as usize].max(0);
                    if self.ah != expected {
                        warns.push((cindex as i32, coefi));
                    }
                    cb[coefi as usize] = self.al;
                }
            }
            for (a, b) in warns {
                self.warn(JWRN_BOGUS_PROGRESSION, &[a, b]);
            }
        }
        for ci in 0..self.comps_in_scan {
            let k = self.cur_comp[ci];
            if is_dc_band {
                if self.ah == 0 {
                    let tbl = self.comps[k].dc_tbl_no;
                    let d = self.make_derived(true, tbl)?;
                    self.dc_derived[tbl as usize] = Some(d);
                }
            } else {
                let tbl = self.comps[k].ac_tbl_no;
                let d = self.make_derived(false, tbl)?;
                self.ac_derived[tbl as usize] = Some(d);
                self.phuff_ac = tbl as usize;
            }
            self.last_dc_val[ci] = 0;
        }
        self.bits_left = 0;
        self.get_buffer = 0;
        self.insufficient_data = false;
        self.eobrun = 0;
        self.restarts_to_go = self.restart_interval;
        Ok(())
    }

    /// The progressive decoders' `decode_mcu` (the one `start_pass_phuff` chose).
    fn decode_mcu_prog(&mut self, blocks: &mut Blocks, idx: &[usize]) -> Result<(), Fatal> {
        if self.restart_interval != 0 && self.restarts_to_go == 0 {
            self.process_restart()?;
        }
        if self.ss == 0 {
            if self.ah == 0 {
                self.dc_first(blocks, idx)?;
            } else {
                self.dc_refine(blocks, idx)?;
            }
        } else if self.ah == 0 {
            self.ac_first(blocks, idx)?;
        } else {
            self.ac_refine(blocks, idx)?;
        }
        self.restarts_to_go = self.restarts_to_go.wrapping_sub(1);
        Ok(())
    }

    fn dc_first(&mut self, blocks: &mut Blocks, idx: &[usize]) -> Result<(), Fatal> {
        if self.insufficient_data {
            return Ok(());
        }
        for blkn in 0..self.blocks_in_mcu {
            let ci = self.mcu_membership[blkn];
            let tbl_no = self.comps[self.cur_comp[ci]].dc_tbl_no as usize;
            let tbl = self.dc_derived[tbl_no].take().expect("table");
            let r = self.huff_decode(&tbl);
            self.dc_derived[tbl_no] = Some(tbl);
            let mut s = r?;
            if s != 0 {
                self.check_bits(s)?;
                let r = self.get_bits(s);
                s = huff_extend(r, s);
            }
            s = s.wrapping_add(self.last_dc_val[ci]);
            self.last_dc_val[ci] = s;
            blocks[idx[blkn]][0] = s.wrapping_shl(self.al as u32) as i16;
        }
        Ok(())
    }

    fn ac_first(&mut self, blocks: &mut Blocks, idx: &[usize]) -> Result<(), Fatal> {
        if self.insufficient_data {
            return Ok(());
        }
        let mut eobrun = self.eobrun;
        if eobrun > 0 {
            eobrun -= 1;
        } else {
            let tbl = self.ac_derived[self.phuff_ac].take().expect("table");
            let r = self.ac_first_block(blocks, idx[0], &tbl, &mut eobrun);
            self.ac_derived[self.phuff_ac] = Some(tbl);
            r?;
        }
        self.eobrun = eobrun;
        Ok(())
    }

    fn ac_first_block(
        &mut self,
        blocks: &mut Blocks,
        slot: usize,
        tbl: &Derived,
        eobrun: &mut u32,
    ) -> Result<(), Fatal> {
        let (se, al) = (self.se, self.al);
        let mut k = self.ss;
        while k <= se {
            let mut s = self.huff_decode(tbl)?;
            let mut r = s >> 4;
            s &= 15;
            if s != 0 {
                k += r;
                self.check_bits(s)?;
                let r = self.get_bits(s);
                s = huff_extend(r, s);
                blocks[slot][NATURAL_ORDER[k as usize]] = s.wrapping_shl(al as u32) as i16;
            } else if r == 15 {
                k += 15;
            } else {
                *eobrun = 1u32 << r;
                if r != 0 {
                    self.check_bits(r)?;
                    r = self.get_bits(r);
                    *eobrun = eobrun.wrapping_add(r as u32);
                }
                *eobrun = eobrun.wrapping_sub(1);
                break;
            }
            k += 1;
        }
        Ok(())
    }

    fn dc_refine(&mut self, blocks: &mut Blocks, idx: &[usize]) -> Result<(), Fatal> {
        let p1 = 1i32 << self.al;
        for &slot in idx.iter().take(self.blocks_in_mcu) {
            self.check_bits(1)?;
            if self.get_bits(1) != 0 {
                blocks[slot][0] = ((blocks[slot][0] as i32) | p1) as i16;
            }
        }
        Ok(())
    }

    fn ac_refine(&mut self, blocks: &mut Blocks, idx: &[usize]) -> Result<(), Fatal> {
        if self.insufficient_data {
            return Ok(());
        }
        let tbl = self.ac_derived[self.phuff_ac].take().expect("table");
        let mut newnz = [0usize; DCTSIZE2];
        let mut num_newnz = 0usize;
        let slot = idx[0];
        let r = self.ac_refine_block(blocks, slot, &tbl, &mut newnz, &mut num_newnz);
        self.ac_derived[self.phuff_ac] = Some(tbl);
        match r {
            Ok(eobrun) => {
                self.eobrun = eobrun;
                Ok(())
            }
            Err(e) => {
                // undoit: the new nonzero coefficients are zeroed again (only reached when a
                // read fails, which a file's data never makes it do here)
                while num_newnz > 0 {
                    num_newnz -= 1;
                    blocks[slot][newnz[num_newnz]] = 0;
                }
                Err(e)
            }
        }
    }

    fn ac_refine_block(
        &mut self,
        blocks: &mut Blocks,
        slot: usize,
        tbl: &Derived,
        newnz: &mut [usize; DCTSIZE2],
        num_newnz: &mut usize,
    ) -> Result<u32, Fatal> {
        let se = self.se;
        let p1 = 1i32 << self.al;
        let m1 = (-1i32).wrapping_shl(self.al as u32);
        let mut eobrun = self.eobrun;
        let mut k = self.ss;
        if eobrun == 0 {
            while k <= se {
                let mut s = self.huff_decode(tbl)?;
                let mut r = s >> 4;
                s &= 15;
                if s != 0 {
                    if s != 1 {
                        self.warn(JWRN_HUFF_BAD_CODE, &[]);
                    }
                    self.check_bits(1)?;
                    s = if self.get_bits(1) != 0 { p1 } else { m1 };
                } else if r != 15 {
                    eobrun = 1u32 << r;
                    if r != 0 {
                        self.check_bits(r)?;
                        r = self.get_bits(r);
                        eobrun = eobrun.wrapping_add(r as u32);
                    }
                    break;
                }
                loop {
                    let pos = NATURAL_ORDER[k as usize];
                    let coef = blocks[slot][pos] as i32;
                    if coef != 0 {
                        self.check_bits(1)?;
                        if self.get_bits(1) != 0 && (coef & p1) == 0 {
                            let v = if coef >= 0 { coef + p1 } else { coef + m1 };
                            blocks[slot][pos] = v as i16;
                        }
                    } else {
                        r -= 1;
                        if r < 0 {
                            break;
                        }
                    }
                    k += 1;
                    if k > se {
                        break;
                    }
                }
                if s != 0 {
                    let pos = NATURAL_ORDER[k as usize];
                    blocks[slot][pos] = s as i16;
                    newnz[*num_newnz] = pos;
                    *num_newnz += 1;
                }
                k += 1;
            }
        }
        if eobrun > 0 {
            while k <= se {
                let pos = NATURAL_ORDER[k as usize];
                let coef = blocks[slot][pos] as i32;
                if coef != 0 {
                    self.check_bits(1)?;
                    if self.get_bits(1) != 0 && (coef & p1) == 0 {
                        let v = if coef >= 0 { coef + p1 } else { coef + m1 };
                        blocks[slot][pos] = v as i16;
                    }
                }
                k += 1;
            }
            eobrun -= 1;
        }
        Ok(eobrun)
    }

    // ---- jdcoefct.c ----

    /// `start_iMCU_row`.
    fn start_imcu_row(&mut self) {
        self.mcu_rows_per_imcu_row = if self.comps_in_scan > 1 {
            1
        } else {
            let c = &self.comps[self.cur_comp[0]];
            if self.input_imcu_row < self.total_imcu_rows - 1 {
                c.v
            } else {
                c.last_row_height
            }
        };
    }

    /// `consume_data`: one iMCU row of the scan into the whole image's coefficients.
    fn consume_data(&mut self) -> Result<Ret, Fatal> {
        let mut coefs = std::mem::take(&mut self.coefs);
        let r = self.consume_data_rows(&mut coefs);
        self.coefs = coefs;
        r
    }

    fn consume_data_rows(&mut self, coefs: &mut Blocks) -> Result<Ret, Fatal> {
        let mut idx = [0usize; D_MAX_BLOCKS_IN_MCU];
        for yoffset in 0..self.mcu_rows_per_imcu_row {
            for mcu_col in 0..self.mcus_per_row {
                let mut blkn = 0;
                for ci in 0..self.comps_in_scan {
                    let c = &self.comps[self.cur_comp[ci]];
                    let start_col = mcu_col as usize * c.mcu_width as usize;
                    let base = self.coef_start[c.index];
                    let stride = self.coef_stride[c.index];
                    for yindex in 0..c.mcu_height {
                        let row = self.input_imcu_row as usize * c.v as usize
                            + (yindex + yoffset) as usize;
                        for xindex in 0..c.mcu_width as usize {
                            idx[blkn] = base + row * stride + start_col + xindex;
                            blkn += 1;
                        }
                    }
                }
                if self.progressive {
                    self.decode_mcu_prog(coefs, &idx)?;
                } else {
                    self.decode_mcu(coefs, &idx)?;
                }
            }
        }
        self.input_imcu_row += 1;
        if self.input_imcu_row < self.total_imcu_rows {
            self.start_imcu_row();
            return Ok(Ret::RowCompleted);
        }
        self.in_scan = false;
        Ok(Ret::ScanCompleted)
    }
}

fn div_round_up(a: u64, b: u64) -> u64 {
    a.div_ceil(b)
}

fn round_up(a: u64, b: u64) -> u64 {
    a.div_ceil(b) * b
}

/// `jpeg_idct_islow` on one block into `out` (8 rows of `stride` samples from `col`), with the
/// range limit `jdmaster.c`'s `prepare_range_limit_table` builds.
fn idct_islow(coef: &[i16; DCTSIZE2], quant: &[i32; DCTSIZE2], out: &mut [u8], stride: usize) {
    const CONST_BITS: i64 = 13;
    const PASS1_BITS: i64 = 2;
    const F0298: i64 = 2446;
    const F0390: i64 = 3196;
    const F0541: i64 = 4433;
    const F0765: i64 = 6270;
    const F0899: i64 = 7373;
    const F1175: i64 = 9633;
    const F1501: i64 = 12299;
    const F1847: i64 = 15137;
    const F1961: i64 = 16069;
    const F2053: i64 = 16819;
    const F2562: i64 = 20995;
    const F3072: i64 = 25172;
    let deq = |i: usize| (coef[i] as i32).wrapping_mul(quant[i]);
    let descale = |x: i64, n: i64| (x + (1i64 << (n - 1))) >> n;
    let mut ws = [0i32; DCTSIZE2];
    for col in 0..8 {
        if (1..8).all(|r| coef[r * 8 + col] == 0) {
            let dcval = deq(col).wrapping_shl(PASS1_BITS as u32);
            for r in 0..8 {
                ws[r * 8 + col] = dcval;
            }
            continue;
        }
        let z2 = deq(16 + col) as i64;
        let z3 = deq(48 + col) as i64;
        let z1 = (z2 + z3) * F0541;
        let tmp2 = z1 + z3 * -F1847;
        let tmp3 = z1 + z2 * F0765;
        let z2 = deq(col) as i64;
        let z3 = deq(32 + col) as i64;
        let tmp0 = (z2 + z3) << CONST_BITS;
        let tmp1 = (z2 - z3) << CONST_BITS;
        let tmp10 = tmp0 + tmp3;
        let tmp13 = tmp0 - tmp3;
        let tmp11 = tmp1 + tmp2;
        let tmp12 = tmp1 - tmp2;
        let mut t0 = deq(56 + col) as i64;
        let mut t1 = deq(40 + col) as i64;
        let mut t2 = deq(24 + col) as i64;
        let mut t3 = deq(8 + col) as i64;
        let z1 = t0 + t3;
        let z2 = t1 + t2;
        let z3 = t0 + t2;
        let z4 = t1 + t3;
        let z5 = (z3 + z4) * F1175;
        t0 *= F0298;
        t1 *= F2053;
        t2 *= F3072;
        t3 *= F1501;
        let z1 = z1 * -F0899;
        let z2 = z2 * -F2562;
        let z3 = z3 * -F1961 + z5;
        let z4 = z4 * -F0390 + z5;
        t0 += z1 + z3;
        t1 += z2 + z4;
        t2 += z2 + z3;
        t3 += z1 + z4;
        let n = CONST_BITS - PASS1_BITS;
        ws[col] = descale(tmp10 + t3, n) as i32;
        ws[56 + col] = descale(tmp10 - t3, n) as i32;
        ws[8 + col] = descale(tmp11 + t2, n) as i32;
        ws[48 + col] = descale(tmp11 - t2, n) as i32;
        ws[16 + col] = descale(tmp12 + t1, n) as i32;
        ws[40 + col] = descale(tmp12 - t1, n) as i32;
        ws[24 + col] = descale(tmp13 + t0, n) as i32;
        ws[32 + col] = descale(tmp13 - t0, n) as i32;
    }
    let n = CONST_BITS + PASS1_BITS + 3;
    for row in 0..8 {
        let w = &ws[row * 8..row * 8 + 8];
        let o = &mut out[row * stride..row * stride + 8];
        if w[1..].iter().all(|&x| x == 0) {
            let v = range_limit(descale(w[0] as i64, PASS1_BITS + 3) as i32);
            o.fill(v);
            continue;
        }
        let z2 = w[2] as i64;
        let z3 = w[6] as i64;
        let z1 = (z2 + z3) * F0541;
        let tmp2 = z1 + z3 * -F1847;
        let tmp3 = z1 + z2 * F0765;
        let tmp0 = (w[0] as i64 + w[4] as i64) << CONST_BITS;
        let tmp1 = (w[0] as i64 - w[4] as i64) << CONST_BITS;
        let tmp10 = tmp0 + tmp3;
        let tmp13 = tmp0 - tmp3;
        let tmp11 = tmp1 + tmp2;
        let tmp12 = tmp1 - tmp2;
        let mut t0 = w[7] as i64;
        let mut t1 = w[5] as i64;
        let mut t2 = w[3] as i64;
        let mut t3 = w[1] as i64;
        let z1 = t0 + t3;
        let z2 = t1 + t2;
        let z3 = t0 + t2;
        let z4 = t1 + t3;
        let z5 = (z3 + z4) * F1175;
        t0 *= F0298;
        t1 *= F2053;
        t2 *= F3072;
        t3 *= F1501;
        let z1 = z1 * -F0899;
        let z2 = z2 * -F2562;
        let z3 = z3 * -F1961 + z5;
        let z4 = z4 * -F0390 + z5;
        t0 += z1 + z3;
        t1 += z2 + z4;
        t2 += z2 + z3;
        t3 += z1 + z4;
        o[0] = range_limit(descale(tmp10 + t3, n) as i32);
        o[7] = range_limit(descale(tmp10 - t3, n) as i32);
        o[1] = range_limit(descale(tmp11 + t2, n) as i32);
        o[6] = range_limit(descale(tmp11 - t2, n) as i32);
        o[2] = range_limit(descale(tmp12 + t1, n) as i32);
        o[5] = range_limit(descale(tmp12 - t1, n) as i32);
        o[3] = range_limit(descale(tmp13 + t0, n) as i32);
        o[4] = range_limit(descale(tmp13 - t0, n) as i32);
    }
}

/// `IDCT_range_limit(cinfo)[x & RANGE_MASK]`: the table's entries from CENTERJSAMPLE on: 128 to
/// 255, then 255 (to 511), then 0 (to 895), then 0 to 127.
fn range_limit(x: i32) -> u8 {
    let t = ((x & 1023) + 512) & 1023; // x's low ten bits as -512..511, shifted by 512
    (t - 512 + 128).clamp(0, 255) as u8
}

/// Which upsampler `jinit_upsampler` chose for a component.
#[derive(Clone, Copy)]
enum Up {
    Full,
    H2v1Fancy,
    H2v1,
    H2v2Fancy,
    H2v2,
    Int(usize, usize),
}

/// The sample rows of one component: three iMCU rows' worth, as a ring.
struct Plane {
    width: usize,
    rows_per_imcu: usize,
    data: Vec<u8>,
}

impl Plane {
    fn row(&self, r: usize) -> &[u8] {
        let k = r % (3 * self.rows_per_imcu);
        &self.data[k * self.width..(k + 1) * self.width]
    }
    fn imcu_rows_mut(&mut self, imcu: usize) -> &mut [u8] {
        let k = (imcu % 3) * self.rows_per_imcu;
        &mut self.data[k * self.width..(k + self.rows_per_imcu) * self.width]
    }
}

/// `build_ycc_rgb_table`.
struct YccTables {
    cr_r: [i32; 256],
    cb_b: [i32; 256],
    cr_g: [i64; 256],
    cb_g: [i64; 256],
}

impl YccTables {
    fn new() -> Self {
        let fix = |x: f64| (x * 65536.0 + 0.5) as i64;
        let one_half = 1i64 << 15;
        let mut t = YccTables {
            cr_r: [0; 256],
            cb_b: [0; 256],
            cr_g: [0; 256],
            cb_g: [0; 256],
        };
        for i in 0..256usize {
            let x = i as i64 - 128;
            t.cr_r[i] = ((fix(1.40200) * x + one_half) >> 16) as i32;
            t.cb_b[i] = ((fix(1.77200) * x + one_half) >> 16) as i32;
            t.cr_g[i] = -fix(0.71414) * x;
            t.cb_g[i] = -fix(0.34414) * x + one_half;
        }
        t
    }
}

/// The colour conversion `jinit_color_deconverter` chose (for `JCS_RGB` output).
enum Convert {
    Ycc(Box<YccTables>),
    Gray,
    Null,
}

/// The output side: planes, upsamplers and converter.
struct Output {
    planes: Vec<Plane>,
    ups: Vec<Up>,
    convert: Convert,
    width: usize,
    height: usize,
    max_v: usize,
    /// Each component's `downsampled_width`, `downsampled_height`, `v_samp_factor`.
    dims: Vec<(usize, usize, usize)>,
    up_rows: Vec<Vec<u8>>,
    rgb: Vec<u8>,
}

impl Output {
    /// The output rows of iMCU row `imcu` (all its rows below the image's height).
    fn emit(&mut self, imcu: usize, out: &mut dyn FnMut(&[u8])) {
        let first = imcu * self.max_v * 8;
        let last = ((imcu + 1) * self.max_v * 8).min(self.height);
        for y in first..last {
            for c in 0..self.planes.len() {
                let (dw, dh, _) = self.dims[c];
                let plane = &self.planes[c];
                let buf = &mut self.up_rows[c];
                match self.ups[c] {
                    Up::Full => buf[..self.width].copy_from_slice(&plane.row(y)[..self.width]),
                    Up::H2v1 => {
                        let row = plane.row(y);
                        for x in 0..self.width {
                            buf[x] = row[x / 2];
                        }
                    }
                    Up::H2v2 => {
                        let row = plane.row(y / 2);
                        for x in 0..self.width {
                            buf[x] = row[x / 2];
                        }
                    }
                    Up::Int(he, ve) => {
                        let row = plane.row(y / ve);
                        for x in 0..self.width {
                            buf[x] = row[x / he];
                        }
                    }
                    Up::H2v1Fancy => {
                        let row = plane.row(y);
                        h2v1_fancy(row, dw, buf);
                    }
                    Up::H2v2Fancy => {
                        let r = y / 2;
                        let nb = if y % 2 == 0 {
                            r.saturating_sub(1)
                        } else {
                            (r + 1).min(dh - 1)
                        };
                        h2v2_fancy(plane.row(r), plane.row(nb), dw, buf);
                    }
                }
            }
            let w = self.width;
            match &self.convert {
                Convert::Ycc(t) => {
                    for x in 0..w {
                        let y0 = self.up_rows[0][x] as i32;
                        let cb = self.up_rows[1][x] as usize;
                        let cr = self.up_rows[2][x] as usize;
                        self.rgb[3 * x] = clamp_sample(y0 + t.cr_r[cr]);
                        self.rgb[3 * x + 1] =
                            clamp_sample(y0 + ((t.cb_g[cb] + t.cr_g[cr]) >> 16) as i32);
                        self.rgb[3 * x + 2] = clamp_sample(y0 + t.cb_b[cb]);
                    }
                }
                Convert::Gray => {
                    for x in 0..w {
                        let g = self.up_rows[0][x];
                        self.rgb[3 * x..3 * x + 3].fill(g);
                    }
                }
                Convert::Null => {
                    for x in 0..w {
                        for c in 0..3 {
                            self.rgb[3 * x + c] = self.up_rows[c][x];
                        }
                    }
                }
            }
            out(&self.rgb);
        }
    }
}

/// `range_limit` over the colour converter's sums (within -256 to 639: a clamp).
fn clamp_sample(x: i32) -> u8 {
    x.clamp(0, 255) as u8
}

/// `h2v1_fancy_upsample` on one row (`dw` > 2).
fn h2v1_fancy(inp: &[u8], dw: usize, out: &mut [u8]) {
    let g = |i: usize| inp[i] as i32;
    out[0] = inp[0];
    out[1] = ((g(0) * 3 + g(1) + 2) >> 2) as u8;
    for c in 1..dw - 1 {
        let v = g(c) * 3;
        out[2 * c] = ((v + g(c - 1) + 1) >> 2) as u8;
        out[2 * c + 1] = ((v + g(c + 1) + 2) >> 2) as u8;
    }
    let c = dw - 1;
    out[2 * c] = ((g(c) * 3 + g(c - 1) + 1) >> 2) as u8;
    out[2 * c + 1] = inp[c];
}

/// `h2v2_fancy_upsample` on one output row from its nearer and farther input rows (`dw` > 2).
fn h2v2_fancy(in0: &[u8], in1: &[u8], dw: usize, out: &mut [u8]) {
    let cs = |i: usize| in0[i] as i32 * 3 + in1[i] as i32;
    let mut this = cs(0);
    let mut next = cs(1);
    out[0] = ((this * 4 + 8) >> 4) as u8;
    out[1] = ((this * 3 + next + 7) >> 4) as u8;
    let mut last = this;
    this = next;
    for c in 1..dw - 1 {
        next = cs(c + 1);
        out[2 * c] = ((this * 3 + last + 8) >> 4) as u8;
        out[2 * c + 1] = ((this * 3 + next + 7) >> 4) as u8;
        last = this;
        this = next;
    }
    let c = dw - 1;
    out[2 * c] = ((this * 3 + last + 8) >> 4) as u8;
    out[2 * c + 1] = ((this * 4 + 7) >> 4) as u8;
}

/// `jpeg_read_header(cinfo, TRUE)` on a file's bytes (`jpeg_stdio_src`), then
/// `jpeg_destroy_decompress`: FreeSurfer's `JPEGReadHeader`.
pub fn read_header(data: &[u8], streams: &mut dyn Streams, opts: Options) -> Result<Header, Fatal> {
    let mut d = D::new(data, streams, opts);
    d.read_header()?;
    Ok(d.header())
}

/// The decompressor with `out_color_space = JCS_RGB` (FreeSurfer's `JPEGReadImage`): the header,
/// `jpeg_start_decompress`, each output row in order to `row` (`output_width` RGB triples),
/// `jpeg_finish_decompress`. The notices of FS-813 go to `notices`.
pub fn decode_rgb(
    data: &[u8],
    streams: &mut dyn Streams,
    opts: Options,
    notices: &mut Notices,
    row: &mut dyn FnMut(&[u8]),
) -> Result<Header, Fatal> {
    let mut d = D::new(data, streams, opts);
    let r = decompress(&mut d, row);
    notices
        .extend(std::mem::replace(&mut d.notices, Notices::new(opts.upstream_defects)).into_vec());
    r?;
    Ok(d.header())
}

fn decompress(d: &mut D, row: &mut dyn FnMut(&[u8])) -> Result<(), Fatal> {
    d.read_header()?;
    // jpeg_start_decompress -> master_selection (out_color_space JCS_RGB)
    let w = d.image_width as usize;
    let h = d.image_height as usize;
    // jinit_color_deconverter
    let ncomp = d.num_components;
    let bad_space = match d.jpeg_color_space {
        ColorSpace::Grayscale => ncomp != 1,
        ColorSpace::Rgb | ColorSpace::YCbCr => ncomp != 3,
        ColorSpace::Cmyk | ColorSpace::Ycck => ncomp != 4,
        ColorSpace::Unknown => ncomp < 1,
    };
    if bad_space {
        return Err(d.fatal(JERR_BAD_J_COLORSPACE, &[]));
    }
    let convert = match d.jpeg_color_space {
        ColorSpace::YCbCr => Convert::Ycc(Box::new(YccTables::new())),
        ColorSpace::Grayscale => Convert::Gray,
        ColorSpace::Rgb => Convert::Null,
        _ => return Err(d.fatal(JERR_CONVERSION_NOTIMPL, &[])),
    };
    // jinit_upsampler
    let mut ups = Vec::new();
    for c in &d.comps {
        let (hi, vi) = (c.h, c.v);
        let (ho, vo) = (d.max_h, d.max_v);
        let fancy_ok = c.downsampled_width > 2;
        let u = if hi == ho && vi == vo {
            Up::Full
        } else if hi * 2 == ho && vi == vo {
            if fancy_ok { Up::H2v1Fancy } else { Up::H2v1 }
        } else if hi * 2 == ho && vi * 2 == vo {
            if fancy_ok { Up::H2v2Fancy } else { Up::H2v2 }
        } else if ho % hi == 0 && vo % vi == 0 {
            Up::Int((ho / hi) as usize, (vo / vi) as usize)
        } else {
            return Err(d.fatal(JERR_FRACT_SAMPLE_NOTIMPL, &[]));
        };
        ups.push(u);
    }
    if d.arith {
        return Err(d.fatal(JERR_ARITH_NOTIMPL, &[]));
    }
    if d.progressive {
        d.coef_bits = Some(vec![[-1i32; DCTSIZE2]; ncomp as usize]);
    }
    // jinit_d_coef_controller: the whole image's coefficients when there are several scans
    if d.has_multiple_scans {
        let mut total: u64 = 0;
        for c in &d.comps {
            let stride = round_up(c.width_in_blocks as u64, c.h as u64);
            let rows = round_up(c.height_in_blocks as u64, c.v as u64);
            d.coef_start.push(total as usize);
            d.coef_stride.push(stride as usize);
            total += stride * rows;
        }
        let bytes = total * 128;
        let too_big = d.opts.max_alloc.is_some_and(|m| bytes > m);
        let mut v: Vec<[i16; DCTSIZE2]> = Vec::new();
        if too_big || v.try_reserve_exact(total as usize).is_err() {
            return Err(d.fatal(JERR_OUT_OF_MEMORY, &[4]));
        }
        v.resize(total as usize, [0; DCTSIZE2]);
        d.coefs = v;
    }
    let mut output = Output {
        planes: d
            .comps
            .iter()
            .map(|c| {
                let width = c.width_in_blocks as usize * 8;
                let rows_per_imcu = c.v as usize * 8;
                Plane {
                    width,
                    rows_per_imcu,
                    data: vec![0; width * rows_per_imcu * 3],
                }
            })
            .collect(),
        ups,
        convert,
        width: w,
        height: h,
        max_v: d.max_v as usize,
        dims: d
            .comps
            .iter()
            .map(|c| {
                (
                    c.downsampled_width as usize,
                    c.downsampled_height as usize,
                    c.v as usize,
                )
            })
            .collect(),
        up_rows: vec![vec![0; round_up(w as u64, d.max_h as u64) as usize + 16]; ncomp as usize],
        rgb: vec![0; 3 * w],
    };
    // start_input_pass for the first scan
    d.start_input_pass()?;
    let total = d.total_imcu_rows as usize;
    if d.has_multiple_scans {
        // jpeg_start_decompress's loop: every scan into the coefficients
        loop {
            if d.consume_input()? == Ret::ReachedEoi {
                break;
            }
        }
        d.output_scan_number = d.input_scan_number;
        // prepare_for_output_pass: jddctmgr's start_pass, the coefficient controller's choice
        build_dct_tables(d);
        let smooth = smoothing_ok(d);
        for r in 0..total {
            if smooth.is_some() {
                idct_smooth_row(d, &mut output, r, smooth.as_ref().expect("latch"));
            } else {
                idct_row(d, &mut output, r);
            }
            if r > 0 {
                output.emit(r - 1, row);
            }
        }
        output.emit(total - 1, row);
    } else {
        build_dct_tables(d);
        let mut mcu = vec![[0i16; DCTSIZE2]; D_MAX_BLOCKS_IN_MCU];
        let ident: Vec<usize> = (0..D_MAX_BLOCKS_IN_MCU).collect();
        let missing = missing_components(d);
        for r in 0..total {
            for (c, plane) in output.planes.iter_mut().enumerate() {
                if missing[c] {
                    // FS-813 (reproduce): the component no scan names: its rows are the
                    // original's memory contents; zeros here
                    plane.imcu_rows_mut(r).fill(0);
                }
            }
            decompress_onepass(d, &mut output, r, &mut mcu, &ident)?;
            if r > 0 {
                output.emit(r - 1, row);
            }
        }
        output.emit(total - 1, row);
    }
    // jpeg_finish_decompress: the markers up to EOI
    while !d.eoi_reached {
        d.consume_input()?;
    }
    d.err.reset();
    Ok(())
}

/// The components a single scan does not name (FS-813 in reproduce mode only).
fn missing_components(d: &D) -> Vec<bool> {
    let mut m = vec![true; d.comps.len()];
    for &k in &d.cur_comp[..d.comps_in_scan] {
        m[k] = false;
    }
    m
}

/// `jddctmgr.c`'s `start_pass`: each component's multipliers from its latched quantization
/// table (zeros for a component no scan named).
fn build_dct_tables(d: &mut D) {
    for c in d.comps.iter_mut() {
        if let Some(q) = c.quant {
            for i in 0..DCTSIZE2 {
                c.dct_table[i] = q[i] as i32;
            }
        }
    }
}

/// `decompress_onepass`: one iMCU row decoded and transformed into the planes.
fn decompress_onepass(
    d: &mut D,
    output: &mut Output,
    imcu: usize,
    mcu: &mut [[i16; DCTSIZE2]],
    ident: &[usize],
) -> Result<(), Fatal> {
    let last_mcu_col = d.mcus_per_row - 1;
    let last_imcu_row = d.total_imcu_rows - 1;
    for yoffset in 0..d.mcu_rows_per_imcu_row {
        for mcu_col in 0..=last_mcu_col {
            for b in mcu.iter_mut().take(d.blocks_in_mcu) {
                *b = [0; DCTSIZE2];
            }
            d.decode_mcu(mcu, ident)?;
            let mut blkn = 0usize;
            for ci in 0..d.comps_in_scan {
                let c = &d.comps[d.cur_comp[ci]];
                let useful_width = if mcu_col < last_mcu_col {
                    c.mcu_width
                } else {
                    c.last_col_width
                };
                let plane = &mut output.planes[c.index];
                let width = plane.width;
                let rows = plane.imcu_rows_mut(imcu);
                let start_col = mcu_col as usize * c.mcu_width as usize * 8;
                for yindex in 0..c.mcu_height {
                    if d.input_imcu_row < last_imcu_row || yoffset + yindex < c.last_row_height {
                        let r0 = ((yoffset + yindex) * 8) as usize;
                        for xindex in 0..useful_width as usize {
                            let col = start_col + xindex * 8;
                            idct_islow(
                                &mcu[blkn + xindex],
                                &c.dct_table,
                                &mut rows[r0 * width + col..],
                                width,
                            );
                        }
                    }
                    blkn += c.mcu_width as usize;
                }
            }
        }
    }
    d.input_imcu_row += 1;
    if d.input_imcu_row < d.total_imcu_rows {
        d.start_imcu_row();
    } else {
        d.in_scan = false;
    }
    Ok(())
}

/// `decompress_data`: one output iMCU row from the whole image's coefficients.
fn idct_row(d: &D, output: &mut Output, imcu: usize) {
    let last = d.total_imcu_rows as usize - 1;
    for c in &d.comps {
        let v = c.v as usize;
        let block_rows = if imcu < last {
            v
        } else {
            let b = (c.height_in_blocks as usize) % v;
            if b == 0 { v } else { b }
        };
        let base = d.coef_start[c.index];
        let stride = d.coef_stride[c.index];
        let plane = &mut output.planes[c.index];
        let width = plane.width;
        let rows = plane.imcu_rows_mut(imcu);
        for br in 0..block_rows {
            let brow = imcu * v + br;
            for bn in 0..c.width_in_blocks as usize {
                let blk = &d.coefs[base + brow * stride + bn];
                idct_islow(
                    blk,
                    &c.dct_table,
                    &mut rows[br * 8 * width + bn * 8..],
                    width,
                );
            }
        }
    }
}

/// `smoothing_ok`: the latched `coef_bits[1..=5]` of each component, or `None` when block
/// smoothing is not used.
fn smoothing_ok(d: &D) -> Option<Vec<[i32; 6]>> {
    let coef_bits = d.coef_bits.as_ref()?;
    if !d.progressive {
        return None;
    }
    let mut useful = false;
    let mut latch = Vec::new();
    for c in &d.comps {
        let q = c.quant?;
        if q[0] == 0 || q[1] == 0 || q[8] == 0 || q[16] == 0 || q[9] == 0 || q[2] == 0 {
            return None;
        }
        let cb = &coef_bits[c.index];
        if cb[0] < 0 {
            return None;
        }
        let mut l = [0i32; 6];
        for coefi in 1..=5 {
            l[coefi] = cb[coefi];
            if cb[coefi] != 0 {
                useful = true;
            }
        }
        latch.push(l);
    }
    if useful { Some(latch) } else { None }
}

/// `decompress_smooth_data`: one output iMCU row with the low AC coefficients that are not yet
/// known estimated from the neighbouring blocks' DC values.
fn idct_smooth_row(d: &D, output: &mut Output, imcu: usize, latch: &[[i32; 6]]) {
    let last_imcu = d.total_imcu_rows as usize - 1;
    for (ci, c) in d.comps.iter().enumerate() {
        let v = c.v as usize;
        let (block_rows, last_row) = if imcu < last_imcu {
            (v, false)
        } else {
            let b = (c.height_in_blocks as usize) % v;
            (if b == 0 { v } else { b }, true)
        };
        let first_row = imcu == 0;
        let base = d.coef_start[c.index];
        let stride = d.coef_stride[c.index];
        let coef_bits = &latch[ci];
        let q = c.quant.expect("latched");
        let (q00, q01, q10, q20, q11, q02) = (
            q[0] as i64,
            q[1] as i64,
            q[8] as i64,
            q[16] as i64,
            q[9] as i64,
            q[2] as i64,
        );
        let plane = &mut output.planes[c.index];
        let width = plane.width;
        let rows = plane.imcu_rows_mut(imcu);
        for br in 0..block_rows {
            let brow = imcu * v + br;
            let prev = if first_row && br == 0 { brow } else { brow - 1 };
            let next = if last_row && br == block_rows - 1 {
                brow
            } else {
                brow + 1
            };
            let at = |r: usize, b: usize| &d.coefs[base + r * stride + b];
            let mut dc1 = at(prev, 0)[0] as i32;
            let mut dc2 = dc1;
            let mut dc3 = dc1;
            let mut dc4 = at(brow, 0)[0] as i32;
            let mut dc5 = dc4;
            let mut dc6 = dc4;
            let mut dc7 = at(next, 0)[0] as i32;
            let mut dc8 = dc7;
            let mut dc9 = dc7;
            let last_col = c.width_in_blocks as usize - 1;
            for bn in 0..=last_col {
                let mut ws = *at(brow, bn);
                if bn < last_col {
                    dc3 = at(prev, bn + 1)[0] as i32;
                    dc6 = at(brow, bn + 1)[0] as i32;
                    dc9 = at(next, bn + 1)[0] as i32;
                }
                let predict = |al: i32, num: i64, qv: i64| -> i16 {
                    let mut pred;
                    if num >= 0 {
                        pred = (((qv << 7) + num) / (qv << 8)) as i32;
                        if al > 0 && pred >= (1 << al) {
                            pred = (1 << al) - 1;
                        }
                    } else {
                        pred = (((qv << 7) - num) / (qv << 8)) as i32;
                        if al > 0 && pred >= (1 << al) {
                            pred = (1 << al) - 1;
                        }
                        pred = pred.wrapping_neg();
                    }
                    pred as i16
                };
                let (d1, d2, d3, d4, d5, d6, d7, d8, d9) = (
                    dc1 as i64, dc2 as i64, dc3 as i64, dc4 as i64, dc5 as i64, dc6 as i64,
                    dc7 as i64, dc8 as i64, dc9 as i64,
                );
                let al = coef_bits[1];
                if al != 0 && ws[1] == 0 {
                    ws[1] = predict(al, 36 * q00 * (d4 - d6), q01);
                }
                let al = coef_bits[2];
                if al != 0 && ws[8] == 0 {
                    ws[8] = predict(al, 36 * q00 * (d2 - d8), q10);
                }
                let al = coef_bits[3];
                if al != 0 && ws[16] == 0 {
                    ws[16] = predict(al, 9 * q00 * (d2 + d8 - 2 * d5), q20);
                }
                let al = coef_bits[4];
                if al != 0 && ws[9] == 0 {
                    ws[9] = predict(al, 5 * q00 * (d1 - d3 - d7 + d9), q11);
                }
                let al = coef_bits[5];
                if al != 0 && ws[2] == 0 {
                    ws[2] = predict(al, 9 * q00 * (d4 + d6 - 2 * d5), q02);
                }
                idct_islow(
                    &ws,
                    &c.dct_table,
                    &mut rows[br * 8 * width + bn * 8..],
                    width,
                );
                dc1 = dc2;
                dc2 = dc3;
                dc4 = dc5;
                dc5 = dc6;
                dc7 = dc8;
                dc8 = dc9;
            }
        }
    }
}
