// SPDX-License-Identifier: Apache-2.0 AND libtiff
//! libtiff 3.6.1's tags (`tiff.h`), field bits (`tif_dir.h`), its table of known fields
//! (`tif_dirinfo.c`'s `tiffFieldInfo`, as FreeSurfer 8.2.0's build compiles it: IPTC and
//! Photoshop support on) and the codecs' tables (`tif_predict.c`, `tif_zip.c`, `tif_fax3.c`,
//! `tif_pixarlog.c`, `tif_luv.c`), with the lookups `_TIFFFindFieldInfo` (and its cache of the
//! last field found, one per thread as the C library keeps one per process),
//! `_TIFFMergeFieldInfo`, `_TIFFCreateAnonFieldInfo` and `TIFFDataWidth`.

use std::borrow::Cow;
use std::cell::RefCell;

/// The tags (`TIFFTAG_*`).
#[allow(missing_docs)]
pub mod tag {

    pub const SUBFILETYPE: u32 = 254;
    pub const OSUBFILETYPE: u32 = 255;
    pub const IMAGEWIDTH: u32 = 256;
    pub const IMAGELENGTH: u32 = 257;
    pub const BITSPERSAMPLE: u32 = 258;
    pub const COMPRESSION: u32 = 259;
    pub const PHOTOMETRIC: u32 = 262;
    pub const THRESHHOLDING: u32 = 263;
    pub const CELLWIDTH: u32 = 264;
    pub const CELLLENGTH: u32 = 265;
    pub const FILLORDER: u32 = 266;
    pub const DOCUMENTNAME: u32 = 269;
    pub const IMAGEDESCRIPTION: u32 = 270;
    pub const MAKE: u32 = 271;
    pub const MODEL: u32 = 272;
    pub const STRIPOFFSETS: u32 = 273;
    pub const ORIENTATION: u32 = 274;
    pub const SAMPLESPERPIXEL: u32 = 277;
    pub const ROWSPERSTRIP: u32 = 278;
    pub const STRIPBYTECOUNTS: u32 = 279;
    pub const MINSAMPLEVALUE: u32 = 280;
    pub const MAXSAMPLEVALUE: u32 = 281;
    pub const XRESOLUTION: u32 = 282;
    pub const YRESOLUTION: u32 = 283;
    pub const PLANARCONFIG: u32 = 284;
    pub const PAGENAME: u32 = 285;
    pub const XPOSITION: u32 = 286;
    pub const YPOSITION: u32 = 287;
    pub const FREEOFFSETS: u32 = 288;
    pub const FREEBYTECOUNTS: u32 = 289;
    pub const GRAYRESPONSEUNIT: u32 = 290;
    pub const GRAYRESPONSECURVE: u32 = 291;
    pub const GROUP3OPTIONS: u32 = 292;
    pub const GROUP4OPTIONS: u32 = 293;
    pub const RESOLUTIONUNIT: u32 = 296;
    pub const PAGENUMBER: u32 = 297;
    pub const COLORRESPONSEUNIT: u32 = 300;
    pub const TRANSFERFUNCTION: u32 = 301;
    pub const SOFTWARE: u32 = 305;
    pub const DATETIME: u32 = 306;
    pub const ARTIST: u32 = 315;
    pub const HOSTCOMPUTER: u32 = 316;
    pub const PREDICTOR: u32 = 317;
    pub const WHITEPOINT: u32 = 318;
    pub const PRIMARYCHROMATICITIES: u32 = 319;
    pub const COLORMAP: u32 = 320;
    pub const HALFTONEHINTS: u32 = 321;
    pub const TILEWIDTH: u32 = 322;
    pub const TILELENGTH: u32 = 323;
    pub const TILEOFFSETS: u32 = 324;
    pub const TILEBYTECOUNTS: u32 = 325;
    pub const BADFAXLINES: u32 = 326;
    pub const CLEANFAXDATA: u32 = 327;
    pub const CONSECUTIVEBADFAXLINES: u32 = 328;
    pub const SUBIFD: u32 = 330;
    pub const INKSET: u32 = 332;
    pub const INKNAMES: u32 = 333;
    pub const NUMBEROFINKS: u32 = 334;
    pub const DOTRANGE: u32 = 336;
    pub const TARGETPRINTER: u32 = 337;
    pub const EXTRASAMPLES: u32 = 338;
    pub const SAMPLEFORMAT: u32 = 339;
    pub const SMINSAMPLEVALUE: u32 = 340;
    pub const SMAXSAMPLEVALUE: u32 = 341;
    pub const YCBCRCOEFFICIENTS: u32 = 529;
    pub const YCBCRSUBSAMPLING: u32 = 530;
    pub const YCBCRPOSITIONING: u32 = 531;
    pub const REFERENCEBLACKWHITE: u32 = 532;
    pub const XMLPACKET: u32 = 700;
    pub const MATTEING: u32 = 32995;
    pub const DATATYPE: u32 = 32996;
    pub const IMAGEDEPTH: u32 = 32997;
    pub const TILEDEPTH: u32 = 32998;
    pub const PIXAR_IMAGEFULLWIDTH: u32 = 33300;
    pub const PIXAR_IMAGEFULLLENGTH: u32 = 33301;
    pub const PIXAR_TEXTUREFORMAT: u32 = 33302;
    pub const PIXAR_WRAPMODES: u32 = 33303;
    pub const PIXAR_FOVCOT: u32 = 33304;
    pub const PIXAR_MATRIX_WORLDTOSCREEN: u32 = 33305;
    pub const PIXAR_MATRIX_WORLDTOCAMERA: u32 = 33306;
    pub const COPYRIGHT: u32 = 33432;
    pub const RICHTIFFIPTC: u32 = 33723;
    pub const PHOTOSHOP: u32 = 34377;
    pub const ICCPROFILE: u32 = 34675;
    pub const FAXRECVPARAMS: u32 = 34908;
    pub const FAXSUBADDRESS: u32 = 34909;
    pub const FAXRECVTIME: u32 = 34910;
    pub const STONITS: u32 = 37439;
    pub const FAXMODE: u32 = 65536;
    pub const FAXFILLFUNC: u32 = 65540;
    pub const PIXARLOGDATAFMT: u32 = 65549;
    pub const ZIPQUALITY: u32 = 65557;
    pub const PIXARLOGQUALITY: u32 = 65558;
    pub const SGILOGDATAFMT: u32 = 65560;
    pub const SGILOGENCODE: u32 = 65561;
}

/// The data types (`TIFFDataType`; `TIFF_ANY` is `TIFF_NOTYPE`).
pub const TIFF_NOTYPE: u16 = 0;
pub const TIFF_ANY: u16 = 0;
pub const TIFF_BYTE: u16 = 1;
pub const TIFF_ASCII: u16 = 2;
pub const TIFF_SHORT: u16 = 3;
pub const TIFF_LONG: u16 = 4;
pub const TIFF_RATIONAL: u16 = 5;
pub const TIFF_SBYTE: u16 = 6;
pub const TIFF_UNDEFINED: u16 = 7;
pub const TIFF_SSHORT: u16 = 8;
pub const TIFF_SLONG: u16 = 9;
pub const TIFF_SRATIONAL: u16 = 10;
pub const TIFF_FLOAT: u16 = 11;
pub const TIFF_DOUBLE: u16 = 12;
pub const TIFF_IFD: u16 = 13;

/// `TIFF_VARIABLE`, `TIFF_SPP`, `TIFF_VARIABLE2` (a field's read and write counts).
pub const TIFF_VARIABLE: i16 = -1;
pub const TIFF_SPP: i16 = -2;
pub const TIFF_VARIABLE2: i16 = -3;

/// The field bits (`FIELD_*`).
#[allow(missing_docs)]
pub mod bit {

    pub const IGNORE: u16 = 0;
    pub const IMAGEDIMENSIONS: u16 = 1;
    pub const TILEDIMENSIONS: u16 = 2;
    pub const RESOLUTION: u16 = 3;
    pub const POSITION: u16 = 4;
    pub const SUBFILETYPE: u16 = 5;
    pub const BITSPERSAMPLE: u16 = 6;
    pub const COMPRESSION: u16 = 7;
    pub const PHOTOMETRIC: u16 = 8;
    pub const THRESHHOLDING: u16 = 9;
    pub const FILLORDER: u16 = 10;
    pub const DOCUMENTNAME: u16 = 11;
    pub const IMAGEDESCRIPTION: u16 = 12;
    pub const MAKE: u16 = 13;
    pub const MODEL: u16 = 14;
    pub const ORIENTATION: u16 = 15;
    pub const SAMPLESPERPIXEL: u16 = 16;
    pub const ROWSPERSTRIP: u16 = 17;
    pub const MINSAMPLEVALUE: u16 = 18;
    pub const MAXSAMPLEVALUE: u16 = 19;
    pub const PLANARCONFIG: u16 = 20;
    pub const PAGENAME: u16 = 21;
    pub const RESOLUTIONUNIT: u16 = 22;
    pub const PAGENUMBER: u16 = 23;
    pub const STRIPBYTECOUNTS: u16 = 24;
    pub const STRIPOFFSETS: u16 = 25;
    pub const COLORMAP: u16 = 26;
    pub const ARTIST: u16 = 27;
    pub const DATETIME: u16 = 28;
    pub const HOSTCOMPUTER: u16 = 29;
    pub const EXTRASAMPLES: u16 = 31;
    pub const SAMPLEFORMAT: u16 = 32;
    pub const SMINSAMPLEVALUE: u16 = 33;
    pub const SMAXSAMPLEVALUE: u16 = 34;
    pub const IMAGEDEPTH: u16 = 35;
    pub const TILEDEPTH: u16 = 36;
    pub const HALFTONEHINTS: u16 = 37;
    pub const YCBCRCOEFFICIENTS: u16 = 38;
    pub const YCBCRSUBSAMPLING: u16 = 39;
    pub const YCBCRPOSITIONING: u16 = 40;
    pub const REFBLACKWHITE: u16 = 41;
    pub const WHITEPOINT: u16 = 42;
    pub const PRIMARYCHROMAS: u16 = 43;
    pub const TRANSFERFUNCTION: u16 = 44;
    pub const INKSET: u16 = 45;
    pub const INKNAMES: u16 = 46;
    pub const DOTRANGE: u16 = 47;
    pub const TARGETPRINTER: u16 = 48;
    pub const SUBIFD: u16 = 49;
    pub const NUMBEROFINKS: u16 = 50;
    pub const ICCPROFILE: u16 = 51;
    pub const PHOTOSHOP: u16 = 52;
    pub const RICHTIFFIPTC: u16 = 53;
    pub const STONITS: u16 = 54;
    pub const IMAGEFULLWIDTH: u16 = 55;
    pub const IMAGEFULLLENGTH: u16 = 56;
    pub const TEXTUREFORMAT: u16 = 57;
    pub const WRAPMODES: u16 = 58;
    pub const FOVCOT: u16 = 59;
    pub const MATRIX_WORLDTOSCREEN: u16 = 60;
    pub const MATRIX_WORLDTOCAMERA: u16 = 61;
    pub const COPYRIGHT: u16 = 62;
    pub const XMLPACKET: u16 = 63;
    pub const CUSTOM: u16 = 65;
    pub const CODEC: u16 = 66;
    pub const PREDICTOR: u16 = 66;
    pub const BADFAXLINES: u16 = 66;
    pub const CLEANFAXDATA: u16 = 67;
    pub const BADFAXRUN: u16 = 68;
    pub const RECVPARAMS: u16 = 69;
    pub const SUBADDRESS: u16 = 70;
    pub const RECVTIME: u16 = 71;
    pub const OPTIONS: u16 = 72;
    pub const PSEUDO: u16 = 0;
}

/// `TIFFFieldInfo`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldInfo {
    pub tag: u32,
    pub readcount: i16,
    pub writecount: i16,
    pub ty: u16,
    pub bit: u16,
    pub oktochange: bool,
    pub passcount: bool,
    pub name: Cow<'static, str>,
}

#[allow(clippy::too_many_arguments)]
const fn fi(
    tag: u32,
    readcount: i16,
    writecount: i16,
    ty: u16,
    bit: u16,
    oktochange: bool,
    passcount: bool,
    name: &'static str,
) -> FieldInfo {
    FieldInfo {
        tag,
        readcount,
        writecount,
        ty,
        bit,
        oktochange,
        passcount,
        name: Cow::Borrowed(name),
    }
}

use tag::*;

/// `tiffFieldInfo`.
pub static MAIN: [FieldInfo; 96] = [

    fi(SUBFILETYPE, 1, 1, TIFF_LONG, bit::SUBFILETYPE, true, false, "SubfileType"),
    fi(SUBFILETYPE, 1, 1, TIFF_SHORT, bit::SUBFILETYPE, true, false, "SubfileType"),
    fi(OSUBFILETYPE, 1, 1, TIFF_SHORT, bit::SUBFILETYPE, true, false, "OldSubfileType"),
    fi(IMAGEWIDTH, 1, 1, TIFF_LONG, bit::IMAGEDIMENSIONS, false, false, "ImageWidth"),
    fi(IMAGEWIDTH, 1, 1, TIFF_SHORT, bit::IMAGEDIMENSIONS, false, false, "ImageWidth"),
    fi(IMAGELENGTH, 1, 1, TIFF_LONG, bit::IMAGEDIMENSIONS, true, false, "ImageLength"),
    fi(IMAGELENGTH, 1, 1, TIFF_SHORT, bit::IMAGEDIMENSIONS, true, false, "ImageLength"),
    fi(BITSPERSAMPLE, -1, -1, TIFF_LONG, bit::BITSPERSAMPLE, false, false, "BitsPerSample"),
    fi(BITSPERSAMPLE, -1, -1, TIFF_SHORT, bit::BITSPERSAMPLE, false, false, "BitsPerSample"),
    fi(COMPRESSION, -1, 1, TIFF_LONG, bit::COMPRESSION, false, false, "Compression"),
    fi(COMPRESSION, -1, 1, TIFF_SHORT, bit::COMPRESSION, false, false, "Compression"),
    fi(PHOTOMETRIC, 1, 1, TIFF_LONG, bit::PHOTOMETRIC, false, false, "PhotometricInterpretation"),
    fi(PHOTOMETRIC, 1, 1, TIFF_SHORT, bit::PHOTOMETRIC, false, false, "PhotometricInterpretation"),
    fi(THRESHHOLDING, 1, 1, TIFF_SHORT, bit::THRESHHOLDING, true, false, "Threshholding"),
    fi(CELLWIDTH, 1, 1, TIFF_SHORT, bit::IGNORE, true, false, "CellWidth"),
    fi(CELLLENGTH, 1, 1, TIFF_SHORT, bit::IGNORE, true, false, "CellLength"),
    fi(FILLORDER, 1, 1, TIFF_SHORT, bit::FILLORDER, false, false, "FillOrder"),
    fi(DOCUMENTNAME, -1, -1, TIFF_ASCII, bit::DOCUMENTNAME, true, false, "DocumentName"),
    fi(IMAGEDESCRIPTION, -1, -1, TIFF_ASCII, bit::IMAGEDESCRIPTION, true, false, "ImageDescription"),
    fi(MAKE, -1, -1, TIFF_ASCII, bit::MAKE, true, false, "Make"),
    fi(MODEL, -1, -1, TIFF_ASCII, bit::MODEL, true, false, "Model"),
    fi(STRIPOFFSETS, -1, -1, TIFF_LONG, bit::STRIPOFFSETS, false, false, "StripOffsets"),
    fi(STRIPOFFSETS, -1, -1, TIFF_SHORT, bit::STRIPOFFSETS, false, false, "StripOffsets"),
    fi(ORIENTATION, 1, 1, TIFF_SHORT, bit::ORIENTATION, false, false, "Orientation"),
    fi(SAMPLESPERPIXEL, 1, 1, TIFF_SHORT, bit::SAMPLESPERPIXEL, false, false, "SamplesPerPixel"),
    fi(ROWSPERSTRIP, 1, 1, TIFF_LONG, bit::ROWSPERSTRIP, false, false, "RowsPerStrip"),
    fi(ROWSPERSTRIP, 1, 1, TIFF_SHORT, bit::ROWSPERSTRIP, false, false, "RowsPerStrip"),
    fi(STRIPBYTECOUNTS, -1, -1, TIFF_LONG, bit::STRIPBYTECOUNTS, false, false, "StripByteCounts"),
    fi(STRIPBYTECOUNTS, -1, -1, TIFF_SHORT, bit::STRIPBYTECOUNTS, false, false, "StripByteCounts"),
    fi(MINSAMPLEVALUE, -2, -1, TIFF_SHORT, bit::MINSAMPLEVALUE, true, false, "MinSampleValue"),
    fi(MAXSAMPLEVALUE, -2, -1, TIFF_SHORT, bit::MAXSAMPLEVALUE, true, false, "MaxSampleValue"),
    fi(XRESOLUTION, 1, 1, TIFF_RATIONAL, bit::RESOLUTION, false, false, "XResolution"),
    fi(YRESOLUTION, 1, 1, TIFF_RATIONAL, bit::RESOLUTION, false, false, "YResolution"),
    fi(PLANARCONFIG, 1, 1, TIFF_SHORT, bit::PLANARCONFIG, false, false, "PlanarConfiguration"),
    fi(PAGENAME, -1, -1, TIFF_ASCII, bit::PAGENAME, true, false, "PageName"),
    fi(XPOSITION, 1, 1, TIFF_RATIONAL, bit::POSITION, true, false, "XPosition"),
    fi(YPOSITION, 1, 1, TIFF_RATIONAL, bit::POSITION, true, false, "YPosition"),
    fi(FREEOFFSETS, -1, -1, TIFF_LONG, bit::IGNORE, false, false, "FreeOffsets"),
    fi(FREEBYTECOUNTS, -1, -1, TIFF_LONG, bit::IGNORE, false, false, "FreeByteCounts"),
    fi(GRAYRESPONSEUNIT, 1, 1, TIFF_SHORT, bit::IGNORE, true, false, "GrayResponseUnit"),
    fi(GRAYRESPONSECURVE, -1, -1, TIFF_SHORT, bit::IGNORE, true, false, "GrayResponseCurve"),
    fi(RESOLUTIONUNIT, 1, 1, TIFF_SHORT, bit::RESOLUTIONUNIT, false, false, "ResolutionUnit"),
    fi(PAGENUMBER, 2, 2, TIFF_SHORT, bit::PAGENUMBER, true, false, "PageNumber"),
    fi(COLORRESPONSEUNIT, 1, 1, TIFF_SHORT, bit::IGNORE, true, false, "ColorResponseUnit"),
    fi(TRANSFERFUNCTION, -1, -1, TIFF_SHORT, bit::TRANSFERFUNCTION, true, false, "TransferFunction"),
    fi(SOFTWARE, -1, -1, TIFF_ASCII, bit::CUSTOM, true, false, "Software"),
    fi(DATETIME, 20, 20, TIFF_ASCII, bit::DATETIME, true, false, "DateTime"),
    fi(ARTIST, -1, -1, TIFF_ASCII, bit::ARTIST, true, false, "Artist"),
    fi(HOSTCOMPUTER, -1, -1, TIFF_ASCII, bit::HOSTCOMPUTER, true, false, "HostComputer"),
    fi(WHITEPOINT, 2, 2, TIFF_RATIONAL, bit::WHITEPOINT, true, false, "WhitePoint"),
    fi(PRIMARYCHROMATICITIES, 6, 6, TIFF_RATIONAL, bit::PRIMARYCHROMAS, true, false, "PrimaryChromaticities"),
    fi(COLORMAP, -1, -1, TIFF_SHORT, bit::COLORMAP, true, false, "ColorMap"),
    fi(HALFTONEHINTS, 2, 2, TIFF_SHORT, bit::HALFTONEHINTS, true, false, "HalftoneHints"),
    fi(TILEWIDTH, 1, 1, TIFF_LONG, bit::TILEDIMENSIONS, false, false, "TileWidth"),
    fi(TILEWIDTH, 1, 1, TIFF_SHORT, bit::TILEDIMENSIONS, false, false, "TileWidth"),
    fi(TILELENGTH, 1, 1, TIFF_LONG, bit::TILEDIMENSIONS, false, false, "TileLength"),
    fi(TILELENGTH, 1, 1, TIFF_SHORT, bit::TILEDIMENSIONS, false, false, "TileLength"),
    fi(TILEOFFSETS, -1, 1, TIFF_LONG, bit::STRIPOFFSETS, false, false, "TileOffsets"),
    fi(TILEBYTECOUNTS, -1, 1, TIFF_LONG, bit::STRIPBYTECOUNTS, false, false, "TileByteCounts"),
    fi(TILEBYTECOUNTS, -1, 1, TIFF_SHORT, bit::STRIPBYTECOUNTS, false, false, "TileByteCounts"),
    fi(SUBIFD, -1, -1, TIFF_LONG, bit::SUBIFD, true, true, "SubIFD"),
    fi(INKSET, 1, 1, TIFF_SHORT, bit::INKSET, false, false, "InkSet"),
    fi(INKNAMES, -1, -1, TIFF_ASCII, bit::INKNAMES, true, true, "InkNames"),
    fi(NUMBEROFINKS, 1, 1, TIFF_SHORT, bit::NUMBEROFINKS, true, false, "NumberOfInks"),
    fi(DOTRANGE, 2, 2, TIFF_SHORT, bit::DOTRANGE, false, false, "DotRange"),
    fi(DOTRANGE, 2, 2, TIFF_BYTE, bit::DOTRANGE, false, false, "DotRange"),
    fi(TARGETPRINTER, -1, -1, TIFF_ASCII, bit::TARGETPRINTER, true, false, "TargetPrinter"),
    fi(EXTRASAMPLES, -1, -1, TIFF_SHORT, bit::EXTRASAMPLES, false, false, "ExtraSamples"),
    fi(EXTRASAMPLES, -1, -1, TIFF_BYTE, bit::EXTRASAMPLES, false, false, "ExtraSamples"),
    fi(SAMPLEFORMAT, -1, -1, TIFF_SHORT, bit::SAMPLEFORMAT, false, false, "SampleFormat"),
    fi(SMINSAMPLEVALUE, -2, -1, TIFF_ANY, bit::SMINSAMPLEVALUE, true, false, "SMinSampleValue"),
    fi(SMAXSAMPLEVALUE, -2, -1, TIFF_ANY, bit::SMAXSAMPLEVALUE, true, false, "SMaxSampleValue"),
    fi(YCBCRCOEFFICIENTS, 3, 3, TIFF_RATIONAL, bit::YCBCRCOEFFICIENTS, false, false, "YCbCrCoefficients"),
    fi(YCBCRSUBSAMPLING, 2, 2, TIFF_SHORT, bit::YCBCRSUBSAMPLING, false, false, "YCbCrSubsampling"),
    fi(YCBCRPOSITIONING, 1, 1, TIFF_SHORT, bit::YCBCRPOSITIONING, false, false, "YCbCrPositioning"),
    fi(REFERENCEBLACKWHITE, 6, 6, TIFF_RATIONAL, bit::REFBLACKWHITE, true, false, "ReferenceBlackWhite"),
    fi(REFERENCEBLACKWHITE, 6, 6, TIFF_LONG, bit::REFBLACKWHITE, true, false, "ReferenceBlackWhite"),
    fi(XMLPACKET, -1, -3, TIFF_UNDEFINED, bit::XMLPACKET, false, true, "XMLPacket"),
    fi(MATTEING, 1, 1, TIFF_SHORT, bit::EXTRASAMPLES, false, false, "Matteing"),
    fi(DATATYPE, -2, -1, TIFF_SHORT, bit::SAMPLEFORMAT, false, false, "DataType"),
    fi(IMAGEDEPTH, 1, 1, TIFF_LONG, bit::IMAGEDEPTH, false, false, "ImageDepth"),
    fi(IMAGEDEPTH, 1, 1, TIFF_SHORT, bit::IMAGEDEPTH, false, false, "ImageDepth"),
    fi(TILEDEPTH, 1, 1, TIFF_LONG, bit::TILEDEPTH, false, false, "TileDepth"),
    fi(TILEDEPTH, 1, 1, TIFF_SHORT, bit::TILEDEPTH, false, false, "TileDepth"),
    fi(PIXAR_IMAGEFULLWIDTH, 1, 1, TIFF_LONG, bit::IMAGEFULLWIDTH, true, false, "ImageFullWidth"),
    fi(PIXAR_IMAGEFULLLENGTH, 1, 1, TIFF_LONG, bit::IMAGEFULLLENGTH, true, false, "ImageFullLength"),
    fi(PIXAR_TEXTUREFORMAT, -1, -1, TIFF_ASCII, bit::TEXTUREFORMAT, true, false, "TextureFormat"),
    fi(PIXAR_WRAPMODES, -1, -1, TIFF_ASCII, bit::WRAPMODES, true, false, "TextureWrapModes"),
    fi(PIXAR_FOVCOT, 1, 1, TIFF_FLOAT, bit::FOVCOT, true, false, "FieldOfViewCotan"),
    fi(PIXAR_MATRIX_WORLDTOSCREEN, 16, 16, TIFF_FLOAT, bit::MATRIX_WORLDTOSCREEN, true, false, "MatrixWorldToScreen"),
    fi(PIXAR_MATRIX_WORLDTOCAMERA, 16, 16, TIFF_FLOAT, bit::MATRIX_WORLDTOCAMERA, true, false, "MatrixWorldToCamera"),
    fi(COPYRIGHT, -1, -1, TIFF_ASCII, bit::COPYRIGHT, true, false, "Copyright"),
    fi(RICHTIFFIPTC, -1, -1, TIFF_LONG, bit::RICHTIFFIPTC, false, true, "RichTIFFIPTC"),
    fi(PHOTOSHOP, -1, -3, TIFF_BYTE, bit::PHOTOSHOP, false, true, "Photoshop"),
    fi(ICCPROFILE, -1, -3, TIFF_UNDEFINED, bit::ICCPROFILE, false, true, "ICC Profile"),
    fi(STONITS, 1, 1, TIFF_DOUBLE, bit::STONITS, false, false, "StoNits"),
];

/// `predictFieldInfo`.
pub static PREDICT: [FieldInfo; 1] = [fi(
    PREDICTOR,
    1,
    1,
    TIFF_SHORT,
    bit::PREDICTOR,
    false,
    false,
    "Predictor",
)];
/// `zipFieldInfo`.
pub static ZIP: [FieldInfo; 1] = [fi(ZIPQUALITY, 0, 0, TIFF_ANY, bit::PSEUDO, true, false, "")];
/// `faxFieldInfo`.
pub static FAX: [FieldInfo; 10] = [
    fi(FAXMODE, 0, 0, TIFF_ANY, bit::PSEUDO, false, false, "FaxMode"),
    fi(FAXFILLFUNC, 0, 0, TIFF_ANY, bit::PSEUDO, false, false, "FaxFillFunc"),
    fi(BADFAXLINES, 1, 1, TIFF_LONG, bit::BADFAXLINES, true, false, "BadFaxLines"),
    fi(BADFAXLINES, 1, 1, TIFF_SHORT, bit::BADFAXLINES, true, false, "BadFaxLines"),
    fi(CLEANFAXDATA, 1, 1, TIFF_SHORT, bit::CLEANFAXDATA, true, false, "CleanFaxData"),
    fi(CONSECUTIVEBADFAXLINES, 1, 1, TIFF_LONG, bit::BADFAXRUN, true, false, "ConsecutiveBadFaxLines"),
    fi(CONSECUTIVEBADFAXLINES, 1, 1, TIFF_SHORT, bit::BADFAXRUN, true, false, "ConsecutiveBadFaxLines"),
    fi(FAXRECVPARAMS, 1, 1, TIFF_LONG, bit::RECVPARAMS, true, false, "FaxRecvParams"),
    fi(FAXSUBADDRESS, -1, -1, TIFF_ASCII, bit::SUBADDRESS, true, false, "FaxSubAddress"),
    fi(FAXRECVTIME, 1, 1, TIFF_LONG, bit::RECVTIME, true, false, "FaxRecvTime"),
];
/// `fax3FieldInfo`.
pub static FAX3: [FieldInfo; 1] =
    [fi(GROUP3OPTIONS, 1, 1, TIFF_LONG, bit::OPTIONS, false, false, "Group3Options")];
/// `fax4FieldInfo`.
pub static FAX4: [FieldInfo; 1] =
    [fi(GROUP4OPTIONS, 1, 1, TIFF_LONG, bit::OPTIONS, false, false, "Group4Options")];
/// `pixarlogFieldInfo`.
pub static PIXARLOG: [FieldInfo; 2] = [
    fi(PIXARLOGDATAFMT, 0, 0, TIFF_ANY, bit::PSEUDO, false, false, ""),
    fi(PIXARLOGQUALITY, 0, 0, TIFF_ANY, bit::PSEUDO, false, false, ""),
];
/// `LogLuvFieldInfo`.
pub static LOGLUV: [FieldInfo; 2] = [
    fi(SGILOGDATAFMT, 0, 0, TIFF_SHORT, bit::PSEUDO, true, false, "SGILogDataFmt"),
    fi(SGILOGENCODE, 0, 0, TIFF_SHORT, bit::PSEUDO, true, false, "SGILogEncode"),
];

/// One of the static tables.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Table {
    Main,
    Predict,
    Zip,
    Fax,
    Fax3,
    Fax4,
    PixarLog,
    LogLuv,
}

impl Table {
    pub fn entries(self) -> &'static [FieldInfo] {
        match self {
            Table::Main => &MAIN,
            Table::Predict => &PREDICT,
            Table::Zip => &ZIP,
            Table::Fax => &FAX,
            Table::Fax3 => &FAX3,
            Table::Fax4 => &FAX4,
            Table::PixarLog => &PIXARLOG,
            Table::LogLuv => &LOGLUV,
        }
    }
}

/// A pointer of the registered-field array: an entry of a static table, or one of the
/// handle's anonymous fields (`_TIFFCreateAnonFieldInfo`, by its index in the handle's list).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldRef {
    Static(Table, u16),
    Anon(u32),
}

/// `TIFFDataWidth(type)`.
pub fn data_width(ty: u16) -> i32 {
    match ty {
        0 | 1 | 2 | 6 | 7 => 1,
        3 | 8 => 2,
        4 | 9 | 11 | 13 => 4,
        5 | 10 | 12 => 8,
        _ => 0,
    }
}

/// The C library's `static const TIFFFieldInfo *last` of `_TIFFFindFieldInfo`, one per thread:
/// a static entry, a copy of an anonymous field while its handle keeps it (`alive`), or an
/// anonymous field freed since (its memory reused: never matched again).
#[derive(Clone, Debug)]
pub(crate) enum Last {
    None,
    Static(Table, u16),
    Anon { owner: u64, index: u32, info: FieldInfo },
    Freed,
}

thread_local! {
    pub(crate) static LAST: RefCell<Last> = const { RefCell::new(Last::None) };
    static SERIAL: RefCell<u64> = const { RefCell::new(0) };
}

/// Forgets the cache of the last field found (a new process's state).
pub(crate) fn reset_last() {
    LAST.with(|l| *l.borrow_mut() = Last::None);
}

/// A new handle's serial number (for the anonymous fields the cache may hold).
pub(crate) fn next_serial() -> u64 {
    SERIAL.with(|s| {
        let mut s = s.borrow_mut();
        *s += 1;
        *s
    })
}

/// The handle's registered fields (`tif_fieldinfo`, `tif_nfields`) and its anonymous ones.
#[derive(Clone, Debug)]
pub struct Fields {
    pub(crate) sorted: Vec<FieldRef>,
    pub(crate) anon: Vec<FieldInfo>,
    pub(crate) serial: u64,
}

/// `tagCompare`: by tag, then by type, larger first.
fn tag_compare(a: &FieldInfo, b: &FieldInfo) -> std::cmp::Ordering {
    if a.tag != b.tag {
        a.tag.cmp(&b.tag)
    } else {
        b.ty.cmp(&a.ty)
    }
}

impl Fields {
    pub(crate) fn new(serial: u64) -> Fields {
        Fields {
            sorted: Vec::new(),
            anon: Vec::new(),
            serial,
        }
    }

    pub fn info(&self, r: FieldRef) -> &FieldInfo {
        match r {
            FieldRef::Static(t, i) => &t.entries()[i as usize],
            FieldRef::Anon(i) => &self.anon[i as usize],
        }
    }

    /// The entry after `r` in memory (`fip++`): the next one of its static table, or `None`
    /// past the end of the table or past an anonymous field (memory the C library reads
    /// anyway).
    pub(crate) fn next_in_memory(r: FieldRef) -> Option<FieldRef> {
        match r {
            FieldRef::Static(t, i) if (i as usize) + 1 < t.entries().len() => {
                Some(FieldRef::Static(t, i + 1))
            }
            _ => None,
        }
    }

    /// `_TIFFMergeFieldInfo(tif, info, n)` for a static table: appended, then the whole array
    /// sorted (glibc's `qsort` sorts this array by merging, which keeps equal entries in order;
    /// equal entries are the same static entry merged twice).
    pub(crate) fn merge(&mut self, t: Table) {
        for i in 0..t.entries().len() {
            self.sorted.push(FieldRef::Static(t, i as u16));
        }
        self.sort();
    }

    fn sort(&mut self) {
        let mut v = std::mem::take(&mut self.sorted);
        v.sort_by(|&a, &b| tag_compare(self.info(a), self.info(b)));
        self.sorted = v;
    }

    /// `_TIFFCreateAnonFieldInfo` then `_TIFFMergeFieldInfo(tif, fld, 1)`.
    pub(crate) fn merge_anon(&mut self, tag: u32, ty: u16) -> FieldRef {
        let info = FieldInfo {
            tag,
            readcount: TIFF_VARIABLE,
            writecount: TIFF_VARIABLE,
            ty,
            bit: bit::CUSTOM,
            oktochange: true,
            passcount: true,
            name: Cow::Owned(format!("Tag {}", tag as i32)),
        };
        self.anon.push(info);
        let r = FieldRef::Anon((self.anon.len() - 1) as u32);
        self.sorted.push(r);
        self.sort();
        r
    }

    /// `_TIFFSetupFieldInfo`: the anonymous fields freed (the cache forgets them), the main
    /// table merged again.
    pub(crate) fn setup(&mut self) {
        self.free_anon();
        self.sorted.clear();
        self.merge(Table::Main);
    }

    /// The anonymous fields freed (`_TIFFSetupFieldInfo`, `TIFFClose`).
    pub(crate) fn free_anon(&mut self) {
        let serial = self.serial;
        LAST.with(|l| {
            let mut l = l.borrow_mut();
            if matches!(&*l, Last::Anon { owner, .. } if *owner == serial) {
                *l = Last::Freed;
            }
        });
        self.anon.clear();
    }

    /// `_TIFFFindFieldInfo(tif, tag, TIFF_ANY)`: the cached field if its tag matches, else the
    /// first registered field with the tag (then cached).
    pub fn find(&self, tag: u32) -> Option<FieldInfo> {
        let hit = LAST.with(|l| match &*l.borrow() {
            Last::Static(t, i) if t.entries()[*i as usize].tag == tag => {
                Some(t.entries()[*i as usize].clone())
            }
            Last::Anon { info, .. } if info.tag == tag => Some(info.clone()),
            _ => None,
        });
        if hit.is_some() {
            return hit;
        }
        let r = *self.sorted.iter().find(|&&r| self.info(r).tag == tag)?;
        let info = self.info(r).clone();
        LAST.with(|l| {
            *l.borrow_mut() = match r {
                FieldRef::Static(t, i) => Last::Static(t, i),
                FieldRef::Anon(i) => Last::Anon {
                    owner: self.serial,
                    index: i,
                    info: info.clone(),
                },
            }
        });
        Some(info)
    }
}

