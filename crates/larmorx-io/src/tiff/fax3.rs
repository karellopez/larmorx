// SPDX-License-Identifier: Apache-2.0 AND libtiff
//! `tif_fax3.c`, `tif_fax3.h` and `mkg3states.c`: the CCITT decoders (Group 3 one- and
//! two-dimensional, Group 4, Modified Huffman RLE and RLE/W), their private tags and the state
//! tables `mkg3states` generates (`tif_fax3sm.c`, made here from the same code lists). The
//! decoder is derived, with permission, from Frank Cringle's viewfax (Copyright (C) 1990, 1995
//! Frank D. Cringle), as libtiff's is. libtiff 3.6.1's encoders are not ported: FreeSurfer
//! writes Deflate only.
//!
//! The decoders keep the runs of the current and the reference line in one block of
//! `2 * nruns + 3` entries, as libtiff 3.6.1 does; damaged data can make them read or write
//! outside it, or read entries never written (memory contents: FS-838). The port reads such
//! entries as 0 (libtiff 4 clears the block) and stops the row at the first access outside the
//! block (by default with libtiff 4's "Buffer overflow" error).

use std::sync::OnceLock;

use super::Tiff;
use super::fields::tag;
use super::next::Out;
use larmorx_core::cfmt::Streams;
use larmorx_core::defects::{FS_838, FS_839, FS_885};

#[cfg(test)]
const S_NULL: u8 = 0;
const S_PASS: u8 = 1;
const S_HORIZ: u8 = 2;
const S_V0: u8 = 3;
const S_VR: u8 = 4;
const S_VL: u8 = 5;
const S_EXT: u8 = 6;
const S_TERMW: u8 = 7;
const S_TERMB: u8 = 8;
const S_MAKEUPW: u8 = 9;
const S_MAKEUPB: u8 = 10;
const S_MAKEUP: u8 = 11;
const S_EOL: u8 = 12;

pub(crate) const FAXMODE_NORTC: i32 = 0x0001;
pub(crate) const FAXMODE_NOEOL: i32 = 0x0002;
pub(crate) const FAXMODE_BYTEALIGN: i32 = 0x0004;
pub(crate) const FAXMODE_WORDALIGN: i32 = 0x0008;
pub(crate) const FAXMODE_CLASSF: i32 = FAXMODE_NORTC;
const GROUP3OPT_2DENCODING: u32 = 0x1;

/// `TIFFFaxTabEnt`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TabEnt {
    pub state: u8,
    pub width: u8,
    pub param: u32,
}

/// `TIFFFaxMainTable`, `TIFFFaxWhiteTable`, `TIFFFaxBlackTable`.
pub struct Tables {
    pub main: [TabEnt; 128],
    pub white: Vec<TabEnt>,
    pub black: Vec<TabEnt>,
}

// mkg3states' code lists: the code right-justified, lsb-first, and (run length << 4) + width.
const PASS: &[(u16, u16)] = &[(0x0008, 4)];
const HORIZ: &[(u16, u16)] = &[(0x0004, 3)];
const V0: &[(u16, u16)] = &[(0x0001, 1)];
const VR: &[(u16, u16)] = &[
    (0x0006, (1 << 4) + 3),
    (0x0030, (2 << 4) + 6),
    (0x0060, (3 << 4) + 7),
];
const VL: &[(u16, u16)] = &[
    (0x0002, (1 << 4) + 3),
    (0x0010, (2 << 4) + 6),
    (0x0020, (3 << 4) + 7),
];
const EXT: &[(u16, u16)] = &[(0x0040, 7)];
const EOLV: &[(u16, u16)] = &[(0x0000, 7)];
#[rustfmt::skip]
const MAKEUPW: &[(u16, u16)] = &[
    (0x001b, 1029), (0x0009, 2053), (0x003a, 3078), (0x0076, 4103), (0x006c, 5128),
    (0x00ec, 6152), (0x0026, 7176), (0x00a6, 8200), (0x0016, 9224), (0x00e6, 10248),
    (0x0066, 11273), (0x0166, 12297), (0x0096, 13321), (0x0196, 14345), (0x0056, 15369),
    (0x0156, 16393), (0x00d6, 17417), (0x01d6, 18441), (0x0036, 19465), (0x0136, 20489),
    (0x00b6, 21513), (0x01b6, 22537), (0x0032, 23561), (0x0132, 24585), (0x00b2, 25609),
    (0x0006, 26630), (0x01b2, 27657),
];
#[rustfmt::skip]
const MAKEUPB: &[(u16, u16)] = &[
    (0x03c0, 1034), (0x0130, 2060), (0x0930, 3084), (0x0da0, 4108), (0x0cc0, 5132),
    (0x02c0, 6156), (0x0ac0, 7180), (0x06c0, 8205), (0x16c0, 9229), (0x0a40, 10253),
    (0x1a40, 11277), (0x0640, 12301), (0x1640, 13325), (0x09c0, 14349), (0x19c0, 15373),
    (0x05c0, 16397), (0x15c0, 17421), (0x0dc0, 18445), (0x1dc0, 19469), (0x0940, 20493),
    (0x1940, 21517), (0x0540, 22541), (0x1540, 23565), (0x0b40, 24589), (0x1b40, 25613),
    (0x04c0, 26637), (0x14c0, 27661),
];
#[rustfmt::skip]
const MAKEUP: &[(u16, u16)] = &[
    (0x0080, 28683), (0x0180, 29707), (0x0580, 30731), (0x0480, 31756), (0x0c80, 32780),
    (0x0280, 33804), (0x0a80, 34828), (0x0680, 35852), (0x0e80, 36876), (0x0380, 37900),
    (0x0b80, 38924), (0x0780, 39948), (0x0f80, 40972),
];
#[rustfmt::skip]
const TERMW: &[(u16, u16)] = &[
    (0x00ac, 8), (0x0038, 22), (0x000e, 36), (0x0001, 52), (0x000d, 68), (0x0003, 84),
    (0x0007, 100), (0x000f, 116), (0x0019, 133), (0x0005, 149), (0x001c, 165), (0x0002, 181),
    (0x0004, 198), (0x0030, 214), (0x000b, 230), (0x002b, 246), (0x0015, 262), (0x0035, 278),
    (0x0072, 295), (0x0018, 311), (0x0008, 327), (0x0074, 343), (0x0060, 359), (0x0010, 375),
    (0x000a, 391), (0x006a, 407), (0x0064, 423), (0x0012, 439), (0x000c, 455), (0x0040, 472),
    (0x00c0, 488), (0x0058, 504), (0x00d8, 520), (0x0048, 536), (0x00c8, 552), (0x0028, 568),
    (0x00a8, 584), (0x0068, 600), (0x00e8, 616), (0x0014, 632), (0x0094, 648), (0x0054, 664),
    (0x00d4, 680), (0x0034, 696), (0x00b4, 712), (0x0020, 728), (0x00a0, 744), (0x0050, 760),
    (0x00d0, 776), (0x004a, 792), (0x00ca, 808), (0x002a, 824), (0x00aa, 840), (0x0024, 856),
    (0x00a4, 872), (0x001a, 888), (0x009a, 904), (0x005a, 920), (0x00da, 936), (0x0052, 952),
    (0x00d2, 968), (0x004c, 984), (0x00cc, 1000), (0x002c, 1016),
];
#[rustfmt::skip]
const TERMB: &[(u16, u16)] = &[
    (0x03b0, 10), (0x0002, 19), (0x0003, 34), (0x0001, 50), (0x0006, 67), (0x000c, 84),
    (0x0004, 100), (0x0018, 117), (0x0028, 134), (0x0008, 150), (0x0010, 167), (0x0050, 183),
    (0x0070, 199), (0x0020, 216), (0x00e0, 232), (0x0030, 249), (0x03a0, 266), (0x0060, 282),
    (0x0040, 298), (0x0730, 315), (0x00b0, 331), (0x01b0, 347), (0x0760, 363), (0x00a0, 379),
    (0x0740, 395), (0x00c0, 411), (0x0530, 428), (0x0d30, 444), (0x0330, 460), (0x0b30, 476),
    (0x0160, 492), (0x0960, 508), (0x0560, 524), (0x0d60, 540), (0x04b0, 556), (0x0cb0, 572),
    (0x02b0, 588), (0x0ab0, 604), (0x06b0, 620), (0x0eb0, 636), (0x0360, 652), (0x0b60, 668),
    (0x05b0, 684), (0x0db0, 700), (0x02a0, 716), (0x0aa0, 732), (0x06a0, 748), (0x0ea0, 764),
    (0x0260, 780), (0x0a60, 796), (0x04a0, 812), (0x0ca0, 828), (0x0240, 844), (0x0ec0, 860),
    (0x01c0, 876), (0x0e40, 892), (0x0140, 908), (0x01a0, 924), (0x09a0, 940), (0x0d40, 956),
    (0x0340, 972), (0x05a0, 988), (0x0660, 1004), (0x0e60, 1020),
];
const EOLH: &[(u16, u16)] = &[(0x0000, 11)];

/// The code lists, for the probe's generator: `(code lsb-first, width, run)`.
pub fn code_list(which: &str) -> Vec<(u16, u8, u32)> {
    let l: &[(u16, u16)] = match which {
        "pass" => PASS,
        "horiz" => HORIZ,
        "v0" => V0,
        "vr" => VR,
        "vl" => VL,
        "ext" => EXT,
        "eolv" => EOLV,
        "makeupw" => MAKEUPW,
        "makeupb" => MAKEUPB,
        "makeup" => MAKEUP,
        "termw" => TERMW,
        "termb" => TERMB,
        _ => EOLH,
    };
    l.iter()
        .map(|&(c, v)| (c, (v & 15) as u8, u32::from(v >> 4)))
        .collect()
}

/// mkg3states' `FillTable`.
fn fill_table(t: &mut [TabEnt], size: u32, p: &[(u16, u16)], state: u8) {
    let limit = 1u32 << size;
    for &(code, val) in p {
        let width = u32::from(val & 15);
        let param = u32::from(val >> 4);
        let incr = 1u32 << width;
        let mut c = u32::from(code);
        while c < limit {
            t[c as usize] = TabEnt {
                state,
                width: width as u8,
                param,
            };
            c += incr;
        }
    }
}

/// The three tables, as `mkg3states` writes them into `tif_fax3sm.c`.
pub fn tables() -> &'static Tables {
    static T: OnceLock<Tables> = OnceLock::new();
    T.get_or_init(|| {
        let mut main = [TabEnt::default(); 128];
        let mut white = vec![TabEnt::default(); 4096];
        let mut black = vec![TabEnt::default(); 8192];
        fill_table(&mut main, 7, PASS, S_PASS);
        fill_table(&mut main, 7, HORIZ, S_HORIZ);
        fill_table(&mut main, 7, V0, S_V0);
        fill_table(&mut main, 7, VR, S_VR);
        fill_table(&mut main, 7, VL, S_VL);
        fill_table(&mut main, 7, EXT, S_EXT);
        fill_table(&mut main, 7, EOLV, S_EOL);
        fill_table(&mut white, 12, MAKEUPW, S_MAKEUPW);
        fill_table(&mut white, 12, MAKEUP, S_MAKEUP);
        fill_table(&mut white, 12, TERMW, S_TERMW);
        fill_table(&mut white, 12, EOLH, S_EOL);
        fill_table(&mut black, 13, MAKEUPB, S_MAKEUPB);
        fill_table(&mut black, 13, MAKEUP, S_MAKEUP);
        fill_table(&mut black, 13, TERMB, S_TERMB);
        fill_table(&mut black, 13, EOLH, S_EOL);
        Tables { main, white, black }
    })
}

/// Which decoder (`tif_decoderow` and friends).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FaxKind {
    D1,
    D2,
    G4,
    Rle,
}

/// `Fax3CodecState` (the decoder's part).
#[derive(Clone, Debug, Default)]
pub(crate) struct Fax {
    pub mode: i32,
    pub rowbytes: u32,
    pub rowpixels: u32,
    pub cleanfaxdata: u16,
    pub badfaxrun: u32,
    pub badfaxlines: u32,
    pub groupoptions: u32,
    pub recvparams: u32,
    pub subaddress: Option<Vec<u8>>,
    pub recvtime: u32,
    /// The bit-reversal table is `TIFFBitRevTable` (fill order not LSB-to-MSB).
    rev: bool,
    data: u32,
    bit: i32,
    eolcnt: i32,
    /// The run block (`runs`) up to its highest entry used, which entries were written, and
    /// its allocated size; `curruns` and `refruns` as indices into it.
    block: Vec<u32>,
    written: Vec<bool>,
    nalloc: u64,
    has_runs: bool,
    curruns: i64,
    refruns: Option<i64>,
}

/// An access outside the run block: the row stops.
struct Ovf;

enum Msg {
    Err(&'static str, String),
    Warn(&'static str, String),
}

/// The decoder's registers (`DECLARE_STATE`).
struct Dec<'a, 'b> {
    raw: &'a [u8],
    raw_defined: usize,
    sp: &'a mut Fax,
    out: Out<'b>,
    module: &'static str,
    name: &'a str,
    row: u32,
    fix: bool,
    rev: bool,
    a0: i32,
    lastx: i32,
    acc: u32,
    avail: i32,
    runlength: i32,
    cp: i64,
    ep: i64,
    pa: i64,
    thisrun: i64,
    eolcnt: i32,
    pb: i64,
    b1: i32,
    msgs: Vec<Msg>,
    undefined: bool,
    outside: bool,
    past_raw: bool,
    /// libtiff 3.6.1's word alignment of an RLE/W row went elsewhere than the next 16-bit
    /// boundary (FS-839).
    misaligned: bool,
}

#[inline]
fn wadd(a: i32, b: u32) -> i32 {
    (a as u32).wrapping_add(b) as i32
}

impl Dec<'_, '_> {
    // ---- the block ----
    fn read(&mut self, at: i64) -> Result<u32, Ovf> {
        if at < 0 || at as u64 >= self.sp.nalloc {
            self.outside = true;
            return Err(Ovf);
        }
        let i = at as usize;
        if i < self.sp.block.len() && self.sp.written[i] {
            Ok(self.sp.block[i])
        } else {
            if !self.fix {
                self.undefined = true;
            }
            Ok(0)
        }
    }

    fn write(&mut self, at: i64, v: u32) -> Result<(), Ovf> {
        if at < 0 || at as u64 >= self.sp.nalloc {
            self.outside = true;
            return Err(Ovf);
        }
        let i = at as usize;
        if i >= self.sp.block.len() {
            self.sp.block.resize(i + 1, 0);
            self.sp.written.resize(i + 1, false);
        }
        self.sp.block[i] = v;
        self.sp.written[i] = true;
        Ok(())
    }

    // ---- the bits ----
    fn byte(&mut self) -> u32 {
        let i = self.cp as usize;
        self.cp += 1;
        let b = if i < self.raw_defined && i < self.raw.len() {
            self.raw[i]
        } else {
            self.past_raw = true;
            0
        };
        u32::from(if self.rev { b.reverse_bits() } else { b })
    }

    /// `NeedBits8(n, eoflab)`: false at the label.
    fn need8(&mut self, n: i32) -> bool {
        if self.avail < n {
            if self.cp >= self.ep {
                if self.avail == 0 {
                    return false;
                }
                self.avail = n;
            } else {
                let b = self.byte();
                self.acc |= b.wrapping_shl(self.avail as u32);
                self.avail += 8;
            }
        }
        true
    }

    /// `NeedBits16(n, eoflab)`.
    fn need16(&mut self, n: i32) -> bool {
        if self.avail < n {
            if self.cp >= self.ep {
                if self.avail == 0 {
                    return false;
                }
                self.avail = n;
            } else {
                let b = self.byte();
                self.acc |= b.wrapping_shl(self.avail as u32);
                self.avail += 8;
                if self.avail < n {
                    if self.cp >= self.ep {
                        self.avail = n;
                    } else {
                        let b = self.byte();
                        self.acc |= b.wrapping_shl(self.avail as u32);
                        self.avail += 8;
                    }
                }
            }
        }
        true
    }

    #[inline]
    fn get(&self, n: i32) -> u32 {
        self.acc & ((1u32 << n) - 1)
    }

    #[inline]
    fn clr(&mut self, n: i32) {
        self.avail -= n;
        self.acc = self.acc.wrapping_shr(n as u32);
    }

    fn lookup8(&mut self, wid: i32, t: &[TabEnt]) -> Option<TabEnt> {
        if !self.need8(wid) {
            return None;
        }
        let e = t[self.get(wid) as usize];
        self.clr(i32::from(e.width));
        Some(e)
    }

    fn lookup16(&mut self, wid: i32, t: &[TabEnt]) -> Option<TabEnt> {
        if !self.need16(wid) {
            return None;
        }
        let e = t[self.get(wid) as usize];
        self.clr(i32::from(e.width));
        Some(e)
    }

    // ---- messages ----
    fn unexpected(&mut self) {
        let m = format!(
            "{}: Bad code word at scanline {} (x {})",
            self.name, self.row as i32, self.a0 as u32
        );
        self.msgs.push(Msg::Err(self.module, m));
    }

    fn extension(&mut self) {
        let m = format!(
            "{}: Uncompressed data (not supported) at scanline {} (x {})",
            self.name, self.row as i32, self.a0 as u32
        );
        self.msgs.push(Msg::Err(self.module, m));
    }

    fn premature_eof(&mut self) {
        let m = format!(
            "{}: Premature EOF at scanline {} (x {})",
            self.name, self.row as i32, self.a0 as u32
        );
        self.msgs.push(Msg::Warn(self.module, m));
    }

    fn badlength(&mut self) {
        let (a0, lastx) = (self.a0 as u32, self.lastx as u32);
        let m = format!(
            "{}: {} at scanline {} (got {a0}, expected {lastx})",
            self.name,
            if a0 < lastx {
                "Premature EOL"
            } else {
                "Line length mismatch"
            },
            self.row as i32
        );
        self.msgs.push(Msg::Warn(self.module, m));
    }

    // ---- the macros ----
    fn setval(&mut self, x: u32) -> Result<(), Ovf> {
        let v = (self.runlength as u32).wrapping_add(x);
        self.write(self.pa, v)?;
        self.pa += 1;
        self.a0 = wadd(self.a0, x);
        self.runlength = 0;
        Ok(())
    }

    /// `SYNC_EOL(eoflab)`: false at the label.
    fn sync_eol(&mut self) -> bool {
        if self.eolcnt == 0 {
            loop {
                if !self.need16(11) {
                    return false;
                }
                if self.get(11) == 0 {
                    break;
                }
                self.clr(1);
            }
        }
        loop {
            if !self.need8(8) {
                return false;
            }
            if self.get(8) != 0 {
                break;
            }
            self.clr(8);
        }
        while self.get(1) == 0 {
            self.clr(1);
        }
        self.clr(1);
        self.eolcnt = 0;
        true
    }

    /// `CLEANUP_RUNS()`.
    fn cleanup_runs(&mut self) -> Result<(), Ovf> {
        if self.runlength != 0 {
            self.setval(0)?;
        }
        if self.a0 != self.lastx {
            self.badlength();
            while self.a0 > self.lastx && self.pa > self.thisrun {
                self.pa -= 1;
                let v = self.read(self.pa)?;
                self.a0 = (self.a0 as u32).wrapping_sub(v) as i32;
            }
            if self.a0 < self.lastx {
                if self.a0 < 0 {
                    self.a0 = 0;
                }
                if (self.pa - self.thisrun) & 1 != 0 {
                    self.setval(0)?;
                }
                self.setval(self.lastx.wrapping_sub(self.a0) as u32)?;
            } else if self.a0 > self.lastx {
                self.setval(self.lastx as u32)?;
                self.setval(0)?;
            }
        }
        Ok(())
    }

    /// One colour's codes up to its terminating code (the white or black half of `EXPAND1D` and
    /// of the horizontal mode): `Some(true)` done, `Some(false)` a code of another kind (with
    /// `state` its state), `None` the end of the data.
    fn run_codes(&mut self, black: bool) -> Result<Option<(bool, u8)>, Ovf> {
        let t = tables();
        let (wid, tab, term, makeup) = if black {
            (13, &t.black[..], S_TERMB, S_MAKEUPB)
        } else {
            (12, &t.white[..], S_TERMW, S_MAKEUPW)
        };
        loop {
            let Some(e) = self.lookup16(wid, tab) else {
                return Ok(None);
            };
            if e.state == term {
                self.setval(e.param)?;
                return Ok(Some((true, e.state)));
            } else if e.state == makeup || e.state == S_MAKEUP {
                self.a0 = wadd(self.a0, e.param);
                self.runlength = wadd(self.runlength, e.param);
            } else {
                return Ok(Some((false, e.state)));
            }
        }
    }

    /// `EXPAND1D(eoflab)`: false at the label (after its message and `CLEANUP_RUNS`).
    fn expand1d(&mut self) -> Result<bool, Ovf> {
        loop {
            for black in [false, true] {
                match self.run_codes(black)? {
                    None => {
                        self.premature_eof();
                        self.cleanup_runs()?;
                        return Ok(false);
                    }
                    Some((true, _)) => {}
                    Some((false, S_EOL)) => {
                        self.eolcnt = 1;
                        self.cleanup_runs()?;
                        return Ok(true);
                    }
                    Some((false, _)) => {
                        self.unexpected();
                        self.cleanup_runs()?;
                        return Ok(true);
                    }
                }
                if self.a0 >= self.lastx {
                    self.cleanup_runs()?;
                    return Ok(true);
                }
            }
            if self.read(self.pa - 1)? == 0 && self.read(self.pa - 2)? == 0 {
                self.pa -= 2;
            }
        }
    }

    /// `CHECK_b1`.
    fn check_b1(&mut self) -> Result<(), Ovf> {
        if self.pa != self.thisrun {
            while self.b1 <= self.a0 && self.b1 < self.lastx {
                let v = self.read(self.pb)?.wrapping_add(self.read(self.pb + 1)?);
                self.b1 = wadd(self.b1, v);
                self.pb += 2;
            }
        }
        Ok(())
    }

    /// `EXPAND2D(eoflab)`: false at the label (after its message and `CLEANUP_RUNS`).
    fn expand2d(&mut self) -> Result<bool, Ovf> {
        let t = tables();
        enum End {
            Eol,
            Eof,
        }
        let end = 'sw: {
            while self.a0 < self.lastx {
                let Some(e) = self.lookup8(7, &t.main) else {
                    break 'sw End::Eof;
                };
                match e.state {
                    S_PASS => {
                        self.check_b1()?;
                        let v = self.read(self.pb)?;
                        self.pb += 1;
                        self.b1 = wadd(self.b1, v);
                        self.runlength = self.runlength.wrapping_add(self.b1.wrapping_sub(self.a0));
                        self.a0 = self.b1;
                        let v = self.read(self.pb)?;
                        self.pb += 1;
                        self.b1 = wadd(self.b1, v);
                    }
                    S_HORIZ => {
                        let order = if (self.pa - self.thisrun) & 1 != 0 {
                            [true, false]
                        } else {
                            [false, true]
                        };
                        for black in order {
                            match self.run_codes(black)? {
                                None => break 'sw End::Eof,
                                Some((true, _)) => {}
                                Some((false, _)) => {
                                    self.unexpected();
                                    break 'sw End::Eol;
                                }
                            }
                        }
                        self.check_b1()?;
                    }
                    S_V0 => {
                        self.check_b1()?;
                        self.setval(self.b1.wrapping_sub(self.a0) as u32)?;
                        let v = self.read(self.pb)?;
                        self.pb += 1;
                        self.b1 = wadd(self.b1, v);
                    }
                    S_VR => {
                        self.check_b1()?;
                        self.setval((self.b1.wrapping_sub(self.a0) as u32).wrapping_add(e.param))?;
                        let v = self.read(self.pb)?;
                        self.pb += 1;
                        self.b1 = wadd(self.b1, v);
                    }
                    S_VL => {
                        self.check_b1()?;
                        self.setval((self.b1.wrapping_sub(self.a0) as u32).wrapping_sub(e.param))?;
                        self.pb -= 1;
                        let v = self.read(self.pb)?;
                        self.b1 = (self.b1 as u32).wrapping_sub(v) as i32;
                    }
                    S_EXT => {
                        self.write(self.pa, self.lastx.wrapping_sub(self.a0) as u32)?;
                        self.pa += 1;
                        self.extension();
                        break 'sw End::Eol;
                    }
                    S_EOL => {
                        self.write(self.pa, self.lastx.wrapping_sub(self.a0) as u32)?;
                        self.pa += 1;
                        if !self.need8(4) {
                            break 'sw End::Eof;
                        }
                        if self.get(4) != 0 {
                            self.unexpected();
                        }
                        self.clr(4);
                        self.eolcnt = 1;
                        break 'sw End::Eol;
                    }
                    _ => {
                        self.unexpected();
                        break 'sw End::Eol;
                    }
                }
            }
            if self.runlength != 0 {
                if self.runlength.wrapping_add(self.a0) < self.lastx {
                    // expect a final V0
                    if !self.need8(1) {
                        break 'sw End::Eof;
                    }
                    if self.get(1) == 0 {
                        self.unexpected();
                        break 'sw End::Eol;
                    }
                    self.clr(1);
                }
                self.setval(0)?;
            }
            End::Eol
        };
        if let End::Eof = end {
            self.premature_eof();
            self.cleanup_runs()?;
            return Ok(false);
        }
        self.cleanup_runs()?;
        Ok(true)
    }

    /// `_TIFFFax3fillruns(buf, thisrun, pa, lastx)` into the row at `row`.
    fn fill(&mut self, row: i64) -> Result<(), Ovf> {
        let runs = self.thisrun;
        let mut erun = self.pa;
        if (erun - runs) & 1 != 0 {
            self.write(erun, 0)?;
            erun += 1;
        }
        let lastx = self.lastx as u32;
        let mut x: u32 = 0;
        let mut r = runs;
        while r < erun {
            for (k, black) in [(0i64, false), (1, true)] {
                let mut run = self.read(r + k)?;
                if x.wrapping_add(run) > lastx || run > lastx {
                    run = lastx.wrapping_sub(x);
                    self.write(r + k, run)?;
                }
                if run != 0 {
                    paint(&mut self.out, row, x, run, black);
                    x = x.wrapping_add(run);
                }
            }
            r += 2;
        }
        Ok(())
    }
}

/// The bits `[x, x + run)` of the row (most significant bit first) set to 1 (`black`) or 0,
/// the others untouched, as the fill's masks do.
fn paint(out: &mut Out, row: i64, x: u32, run: u32, black: bool) {
    let mut pos = u64::from(x);
    let end = pos + u64::from(run);
    while pos < end {
        let byte = row + (pos >> 3) as i64;
        let bx = (pos & 7) as u32;
        let n = (8 - bx).min((end - pos) as u32);
        let mask = ((0xffu32 >> bx) & !(0xffu32 >> (bx + n))) as u8;
        if n == 8 {
            out.set(byte, if black { 0xff } else { 0 });
        } else {
            let b = out.get(byte);
            out.set(byte, if black { b | mask } else { b & !mask });
        }
        pos += u64::from(n);
    }
}

impl Tiff {
    /// `InitCCITTFax3` and the scheme's own init (`TIFFInitCCITTFax3`, `TIFFInitCCITTFax4`,
    /// `TIFFInitCCITTRLE`, `TIFFInitCCITTRLEW`).
    pub(crate) fn fax_init(&mut self, scheme: u16, s: &mut dyn Streams) -> bool {
        use super::codec::{Cleanup, Decode, PreDecode, SetupDecode};
        use super::compression as c;
        use super::dir::{Arg, Layer};
        use super::fields::Table;
        self.codec.fax = Some(Box::default());
        self.fields.merge(Table::Fax);
        let k = &mut self.codec;
        k.layers.insert(0, Layer::Fax);
        self.flags |= super::TIFF_NOBITREV;
        k.setupdecode = SetupDecode::Fax;
        k.predecode = PreDecode::Fax;
        let d = match scheme {
            c::CCITTFAX4 => Decode::Fax(FaxKind::G4),
            c::CCITTRLE | c::CCITTRLEW => Decode::Fax(FaxKind::Rle),
            _ => Decode::Fax(FaxKind::D1),
        };
        k.decoderow = d;
        k.decodestrip = d;
        k.decodetile = d;
        k.cleanup = Cleanup::Fax;
        let mode = match scheme {
            c::CCITTFAX3 => {
                self.fields.merge(Table::Fax3);
                FAXMODE_CLASSF
            }
            c::CCITTFAX4 => {
                self.fields.merge(Table::Fax4);
                FAXMODE_NORTC
            }
            c::CCITTRLE => FAXMODE_NORTC | FAXMODE_NOEOL | FAXMODE_BYTEALIGN,
            _ => FAXMODE_NORTC | FAXMODE_NOEOL | FAXMODE_WORDALIGN,
        };
        self.set_field_arg(tag::FAXMODE, Arg::Int(mode as u32), s)
    }

    /// `Fax3VSetField` for the tags it handles: `Some(status)`, or `None` for its parent.
    pub(crate) fn fax_vsetfield(&mut self, t: u32, a: &super::dir::Arg) -> Option<bool> {
        use super::dir::Arg;
        let sp = self.codec.fax.as_mut()?;
        match t {
            tag::FAXMODE => {
                sp.mode = a.int() as i32;
                return Some(true);
            }
            tag::FAXFILLFUNC => return Some(true),
            tag::GROUP3OPTIONS | tag::GROUP4OPTIONS => sp.groupoptions = a.int(),
            tag::BADFAXLINES => sp.badfaxlines = a.int(),
            tag::CLEANFAXDATA => sp.cleanfaxdata = a.int() as u16,
            tag::CONSECUTIVEBADFAXLINES => sp.badfaxrun = a.int(),
            tag::FAXRECVPARAMS => sp.recvparams = a.int(),
            tag::FAXSUBADDRESS => {
                let v = match a {
                    Arg::Str(b) => b.to_vec(),
                    // FS-885: the tag came before Compression and was registered as an unknown
                    // tag that passes its count; libtiff 3.6.1 hands the count to this method
                    // where the string's address goes (a crash)
                    Arg::Count(_, super::dir::Data::Bytes(b)) => {
                        b.iter().copied().take_while(|&c| c != 0).collect()
                    }
                    _ => Vec::new(),
                };
                let counted = matches!(a, Arg::Count(..));
                sp.subaddress = Some(v);
                if counted {
                    let name = self.name.clone();
                    self.notices.add(
                        &FS_885,
                        format!(
                            "{name}: a FaxSubAddress before the Compression field, read as an \
                             unknown tag that passes its count: libtiff 3.6.1 takes the count \
                             for the string's address (a crash); set from the string"
                        ),
                    );
                }
            }
            tag::FAXRECVTIME => sp.recvtime = a.int(),
            _ => return None,
        }
        if let Some(f) = self.fields.find(t) {
            self.dir.set_bit(f.bit);
        }
        self.flags |= super::TIFF_DIRTYDIRECT;
        Some(true)
    }

    /// `Fax3VGetField` for the tags it handles.
    pub(crate) fn fax_vgetfield(&self, t: u32) -> Option<super::Value> {
        use super::Value;
        let sp = self.codec.fax.as_ref()?;
        Some(match t {
            tag::FAXMODE => Value::I32(sp.mode),
            tag::FAXFILLFUNC => Value::Present,
            tag::GROUP3OPTIONS | tag::GROUP4OPTIONS => Value::U32(sp.groupoptions),
            tag::BADFAXLINES => Value::U32(sp.badfaxlines),
            tag::CLEANFAXDATA => Value::U16(sp.cleanfaxdata),
            tag::CONSECUTIVEBADFAXLINES => Value::U32(sp.badfaxrun),
            tag::FAXRECVPARAMS => Value::U32(sp.recvparams),
            tag::FAXSUBADDRESS => Value::Present,
            tag::FAXRECVTIME => Value::U32(sp.recvtime),
            _ => return None,
        })
    }

    /// `Fax3SetupState` (decoding).
    pub(crate) fn fax_setup(&mut self, s: &mut dyn Streams) -> bool {
        use super::codec::Decode;
        if self.dir.bitspersample != 1 {
            self.error(
                Some(&self.name.clone()),
                "Bits/sample must be 1 for Group 3/4 encoding/decoding",
                s,
            );
            return false;
        }
        let (rowbytes, rowpixels) = if self.is_tiled() {
            (self.tile_row_size(), self.dir.tilewidth)
        } else {
            (self.scanline_size(), self.dir.imagewidth)
        };
        let fax4 = self.dir.compression == super::compression::CCITTFAX4;
        let name = self.name.clone();
        let sp = self.codec.fax.as_mut().expect("fax state");
        sp.rowbytes = rowbytes as u32;
        sp.rowpixels = rowpixels;
        let needs_ref = sp.groupoptions & GROUP3OPT_2DENCODING != 0 || fax4;
        let roundup = rowpixels.wrapping_add(31) / 32 * 32;
        let nruns = if needs_ref {
            roundup.wrapping_mul(2)
        } else {
            rowpixels
        };
        let entries = nruns.wrapping_mul(2).wrapping_add(3);
        if !self.can_alloc(u64::from(entries) * 4) {
            let m = format!("{name}: No space for Group 3/4 run arrays");
            self.error(Some("Fax3SetupState"), &m, s);
            return false;
        }
        let true_entries = 2 * u64::from(if needs_ref {
            2 * (u64::from(rowpixels).div_ceil(32) * 32)
        } else {
            u64::from(rowpixels)
        }) + 3;
        if u64::from(entries) != true_entries {
            self.notices.add(
                &FS_838,
                format!(
                    "the CCITT run arrays of {name} ({true_entries} entries) are allocated with \
                     their size cut to 32 bits ({entries} entries): libtiff 3.6.1 writes past \
                     them (memory past the block)"
                ),
            );
        }
        let sp = self.codec.fax.as_mut().expect("fax state");
        sp.block.clear();
        sp.written.clear();
        sp.nalloc = u64::from(entries);
        sp.has_runs = true;
        sp.curruns = 0;
        sp.refruns = needs_ref.then_some(i64::from(nruns >> 1));
        if sp.groupoptions & GROUP3OPT_2DENCODING != 0 {
            let k = &mut self.codec;
            k.decoderow = Decode::Fax(FaxKind::D2);
            k.decodestrip = Decode::Fax(FaxKind::D2);
            k.decodetile = Decode::Fax(FaxKind::D2);
        }
        if needs_ref && (rowbytes < 0 || !self.can_alloc(rowbytes as u64)) {
            let m = format!("{name}: No space for Group 3/4 reference line");
            self.error(Some("Fax3SetupState"), &m, s);
            return false;
        }
        true
    }

    /// `Fax3PreDecode`.
    pub(crate) fn fax_pre_decode(&mut self) -> bool {
        let fillorder = self.dir.fillorder;
        let sp = self.codec.fax.as_mut().expect("fax state");
        sp.bit = 0;
        sp.data = 0;
        sp.eolcnt = 0;
        sp.rev = fillorder != super::values::FILLORDER_LSB2MSB;
        if let Some(r) = sp.refruns {
            let rowpixels = sp.rowpixels;
            for (k, v) in [(0, rowpixels), (1, 0)] {
                let i = (r + k) as usize;
                if (i as u64) < sp.nalloc {
                    if i >= sp.block.len() {
                        sp.block.resize(i + 1, 0);
                        sp.written.resize(i + 1, false);
                    }
                    sp.block[i] = v;
                    sp.written[i] = true;
                }
            }
        }
        true
    }

    /// `Fax3Decode1D`, `Fax3Decode2D`, `Fax4Decode`, `Fax3DecodeRLE`.
    pub(crate) fn fax_decode(
        &mut self,
        kind: FaxKind,
        buf: &mut [u8],
        occ0: i32,
        s: &mut dyn Streams,
    ) -> i32 {
        let module = match kind {
            FaxKind::D1 => "Fax3Decode1D",
            FaxKind::D2 => "Fax3Decode2D",
            FaxKind::G4 => "Fax4Decode",
            FaxKind::Rle => "Fax3DecodeRLE",
        };
        let mut sp = *self.codec.fax.take().expect("fax state");
        let raw = std::mem::take(&mut self.rawdata);
        let name = self.name.clone();
        let fix = self.fix();
        let rowbytes = sp.rowbytes;
        let mode = sp.mode;
        let cp0 = self.rawcp as i64;
        let mut d = Dec {
            raw: &raw,
            raw_defined: self.raw_defined,
            out: Out { buf, past: false },
            module,
            name: &name,
            row: self.row,
            fix,
            rev: sp.rev,
            a0: 0,
            lastx: sp.rowpixels as i32,
            acc: sp.data,
            avail: sp.bit,
            runlength: 0,
            cp: cp0,
            ep: cp0 + i64::from(self.rawcc),
            pa: 0,
            thisrun: sp.curruns,
            eolcnt: sp.eolcnt,
            pb: 0,
            b1: 0,
            msgs: Vec::new(),
            undefined: false,
            outside: false,
            past_raw: false,
            misaligned: false,
            sp: &mut sp,
        };
        let mut hang = false;
        let r = (|| -> Result<i32, Ovf> {
            let mut occ = occ0;
            let mut row: i64 = 0;
            d.thisrun = d.sp.curruns;
            while occ > 0 {
                if rowbytes == 0 {
                    hang = true;
                    return Ok(-1);
                }
                if row > 0 && row >= d.out.buf.len() as i64 {
                    // every row from here is past the caller's buffer
                    d.out.past = true;
                    return Ok(-1);
                }
                d.a0 = 0;
                d.runlength = 0;
                if matches!(kind, FaxKind::D2 | FaxKind::G4) {
                    d.thisrun = d.sp.curruns;
                }
                d.pa = d.thisrun;
                match kind {
                    FaxKind::D1 => {
                        if !d.sync_eol() {
                            d.cleanup_runs()?;
                            d.fill(row)?;
                            return Ok(-1);
                        }
                        if !d.expand1d()? {
                            d.fill(row)?;
                            return Ok(-1);
                        }
                        d.fill(row)?;
                    }
                    FaxKind::D2 => {
                        if !d.sync_eol() || !d.need8(1) {
                            d.cleanup_runs()?;
                            d.fill(row)?;
                            return Ok(-1);
                        }
                        let is1d = d.get(1) != 0;
                        d.clr(1);
                        d.pb = d.sp.refruns.ok_or(Ovf)?;
                        d.b1 = d.read(d.pb)? as i32;
                        d.pb += 1;
                        let ok = if is1d { d.expand1d()? } else { d.expand2d()? };
                        if !ok {
                            d.fill(row)?;
                            return Ok(-1);
                        }
                        d.fill(row)?;
                        d.setval(0)?;
                        let sp = &mut *d.sp;
                        if let Some(rf) = sp.refruns {
                            sp.refruns = Some(sp.curruns);
                            sp.curruns = rf;
                        }
                    }
                    FaxKind::G4 => {
                        d.pb = d.sp.refruns.ok_or(Ovf)?;
                        d.b1 = d.read(d.pb)? as i32;
                        d.pb += 1;
                        let ok = d.expand2d()?;
                        if !ok || d.eolcnt != 0 {
                            let _ = d.need16(13);
                            d.clr(13);
                            d.fill(row)?;
                            return Ok(-1);
                        }
                        d.fill(row)?;
                        d.setval(0)?;
                        let sp = &mut *d.sp;
                        if let Some(rf) = sp.refruns {
                            sp.refruns = Some(sp.curruns);
                            sp.curruns = rf;
                        }
                    }
                    FaxKind::Rle => {
                        if !d.expand1d()? {
                            d.fill(row)?;
                            return Ok(-1);
                        }
                        d.fill(row)?;
                        if mode & FAXMODE_BYTEALIGN != 0 {
                            let n = d.avail - (d.avail & !7);
                            d.clr(n);
                        } else if mode & FAXMODE_WORDALIGN != 0 {
                            // the next row starts at the next 16-bit boundary of the strip
                            // (the raw buffer itself is aligned: `isAligned(cp, uint16)`)
                            let pos = d.cp * 8 - i64::from(d.avail);
                            let right = (16 - pos.rem_euclid(16)) % 16;
                            let n = d.avail - (d.avail & !15);
                            let skip = i64::from(d.avail - n) == 0 && (d.cp & 1 != 0);
                            let theirs = pos + i64::from(n) + if skip { 8 } else { 0 };
                            if theirs != pos + right {
                                d.misaligned = true;
                            }
                            if fix {
                                let r = right as i32;
                                if r <= d.avail {
                                    d.clr(r);
                                } else {
                                    let extra = r - d.avail;
                                    d.clr(d.avail);
                                    d.cp += i64::from(extra / 8);
                                }
                            } else {
                                d.clr(n);
                                if skip {
                                    d.cp += 1;
                                }
                            }
                        }
                    }
                }
                row += i64::from(rowbytes);
                occ = occ.wrapping_sub(rowbytes as i32);
            }
            Ok(1)
        })();
        let r = match r {
            Ok(v) => v,
            Err(Ovf) => -1,
        };
        // UNCACHE_STATE
        let (cp, acc, avail, eolcnt) = (d.cp, d.acc, d.avail, d.eolcnt);
        let msgs = std::mem::take(&mut d.msgs);
        let (undefined, outside, past_raw, past, misaligned) =
            (d.undefined, d.outside, d.past_raw, d.out.past, d.misaligned);
        let row = d.row;
        drop(d);
        sp.bit = avail;
        sp.data = acc;
        sp.eolcnt = eolcnt;
        self.rawcc = self.rawcc.wrapping_sub((cp - cp0) as i32);
        self.rawcp = cp.max(0) as usize;
        self.rawdata = raw;
        self.codec.fax = Some(Box::new(sp));
        for m in msgs {
            match m {
                Msg::Err(module, m) => self.error(Some(module), &m, s),
                Msg::Warn(module, m) => self.warning(Some(module), &m, s),
            }
        }
        if outside {
            self.notices.add(
                &FS_838,
                format!(
                    "decoding {name}: the CCITT decoder's runs go outside its run arrays \
                     (libtiff 3.6.1 reads or writes the memory around them: a crash or memory \
                     contents); the row stopped there"
                ),
            );
            if fix {
                let what = if self.is_tiled() { "tile" } else { "strip" };
                let n = if self.is_tiled() {
                    self.curtile
                } else {
                    self.curstrip
                };
                let m = format!("Buffer overflow at line {row} of {what} {n}");
                self.error(Some(module), &m, s);
            }
        }
        if undefined {
            self.notices.add(
                &FS_838,
                format!(
                    "decoding {name}: the CCITT decoder read entries of its run arrays never \
                     written (memory contents in libtiff 3.6.1); read as 0"
                ),
            );
        }
        if misaligned {
            self.notices.add(
                &FS_839,
                format!(
                    "decoding {name}: an RLE/W row ended with the decoder holding bits of the next \
                     16-bit word; libtiff 3.6.1 drops them and starts the next row elsewhere \
                     than the next word boundary{}",
                    if fix {
                        " (corrected: the next row starts at the next word boundary)"
                    } else {
                        ""
                    }
                ),
            );
        }
        if hang {
            self.notices.add(
                &FS_838,
                format!(
                    "decoding {name}: a CCITT row size of 0 bytes for a buffer of {occ0}: \
                     libtiff 3.6.1 never returns (a hang); nothing decoded"
                ),
            );
        }
        if past_raw {
            self.past_raw_notice();
        }
        if past {
            self.past_buffer_notice("CCITT");
        }
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The tables equal FreeSurfer 8.2.0's `packages/tiff/tif_fax3sm.c` (and the one
    /// libtiff 3.6.1's `mkg3states -c const` writes): FNV-1a over `state, width, param`
    /// (little-endian) of the 128 + 4096 + 8192 entries, computed from that file by
    /// `validation/probes/fs-tiff/fax3sm_hash.py`.
    #[test]
    fn tables_equal_tif_fax3sm() {
        let t = tables();
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        for e in t.main.iter().chain(&t.white).chain(&t.black) {
            for b in [e.state, e.width].into_iter().chain(e.param.to_le_bytes()) {
                h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
            }
        }
        assert_eq!(h, TABLES_HASH);
        assert!(t.white.iter().any(|e| e.state == S_NULL));
    }

    const TABLES_HASH: u64 = 0xdc50_3109_c88f_d801;
}
