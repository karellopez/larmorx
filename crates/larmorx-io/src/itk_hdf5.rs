// SPDX-License-Identifier: Apache-2.0
//! ITK's HDF5 transform files (`.h5`), read as ITK v5.4.5's `HDF5TransformIOTemplate` reads them
//! (`Modules/IO/TransformHDF5/src/itkHDF5TransformIO.cxx`), through `larmorx-hdf5`.
//!
//! The layout: `/TransformGroup/<i>` for i = 0 to (number of members of `/TransformGroup`) − 1,
//! each with a variable-length string `TransformType` and, unless it is a composite, the
//! datasets `TransformFixedParameters` and `TransformParameters` (or ITK 4's misspelt
//! `TranformFixedParameters` and `TranformParameters`). Parameters stored in 8 bytes are read
//! as double, any other floating-point type as float; fixed parameters likewise, widened to
//! double.
//!
//! When something cannot be read, the error carries what ANTs prints for it
//! ([`ReadFailure::ants_report`]): ITK's exception, with its addresses written as `0x0` and its
//! build directory as `<ITK source>`, and the HDF5 library's error stack abridged to its first
//! and last frames.
//!
//! **Upstream defects (D13).** ITK-12: ITK reads `TransformType` only as a variable-length
//! string; larmorx also reads a fixed-length string (as h5py writes `numpy.bytes_`), unless
//! `reproduce`. ITK-13: ITK's messages for a parameter dataset of the wrong rank name
//! "TransformType", and join the dataset's path to "in HDF5 File" without a blank; larmorx
//! names the dataset (`reproduce`: ITK's text). ITK-14: a parameter type wider than 8 bytes
//! (`long double`, IEEE quadruple) is read through `float`; larmorx reads it in the reader's
//! precision directly.

use std::path::Path;

use larmorx_core::Precision;
use larmorx_core::defects::{self, Notices};
use larmorx_hdf5::{Datatype, Error as H5Error, File, ObjectKind};

use crate::itk_transform::{ItkTransformParts, Parameters};

/// Where ITK raises an exception: its source file (under the ITK tree) and line, and the
/// class that raises it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItkSite {
    pub file: &'static str,
    pub line: u32,
    pub class: &'static str,
}

const HDF5_IO: &str = "Modules/IO/TransformHDF5/src/itkHDF5TransformIO.cxx";
const READER: &str = "Modules/IO/TransformBase/src/itkTransformFileReader.cxx";

/// `catch (const H5::Exception &)` in `Read()`: line 365.
const SITE_H5: ItkSite = ItkSite {
    file: HDF5_IO,
    line: 365,
    class: "HDF5TransformIOTemplate",
};
/// `ReadParameters`: wrong class (line 171), wrong rank (176).
const SITE_PARAM_TYPE: ItkSite = ItkSite {
    file: HDF5_IO,
    line: 171,
    class: "HDF5TransformIOTemplate",
};
const SITE_PARAM_DIMS: ItkSite = ItkSite {
    file: HDF5_IO,
    line: 176,
    class: "HDF5TransformIOTemplate",
};
/// `ReadFixedParameters`: wrong class (line 218), wrong rank (223).
const SITE_FIXED_TYPE: ItkSite = ItkSite {
    file: HDF5_IO,
    line: 218,
    class: "HDF5TransformIOTemplate",
};
const SITE_FIXED_DIMS: ItkSite = ItkSite {
    file: HDF5_IO,
    line: 223,
    class: "HDF5TransformIOTemplate",
};
/// `CreateTransform` (`itkTransformIOBase.cxx`): an unregistered class name.
const SITE_CREATE: ItkSite = ItkSite {
    file: "Modules/IO/TransformBase/src/itkTransformIOBase.cxx",
    line: 62,
    class: "HDF5TransformIOTemplate",
};
/// `TransformFileReaderTemplate::Update`: no IO can read the file (line 128), or the IO read
/// no transform (line 144).
const SITE_NO_IO: ItkSite = ItkSite {
    file: READER,
    line: 128,
    class: "TransformFileReaderTemplate",
};
const SITE_EMPTY: ItkSite = ItkSite {
    file: READER,
    line: 144,
    class: "TransformFileReaderTemplate",
};

/// An HDF5 library call that failed, as the HDF5 library reports it on standard error.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hdf5Failure {
    /// The API function (`H5Fopen`, `H5Gopen2`, `H5Dopen2`, `H5Dread`).
    pub api: &'static str,
    /// The cause, in HDF5's words where larmorx detects it as HDF5 does.
    pub cause: String,
}

impl Hdf5Failure {
    /// The first line of HDF5's error stack, then the API call and the cause.
    pub fn diag(&self) -> String {
        let what = match self.api {
            "H5Fopen" => "unable to open file",
            "H5Gopen2" => "unable to open group",
            "H5Dopen2" => "unable to open dataset",
            _ => "can't read data",
        };
        format!(
            "HDF5-DIAG: Error detected in HDF5 (1.12.1) thread 0:\n  #000: in {}(): {what}\n  #001: {}\n",
            self.api, self.cause
        )
    }
}

/// Why a transform file could not be read, and what ANTs prints for it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadFailure {
    /// One line for a library user.
    pub reason: String,
    /// What ANTs's `itk::ants::ReadTransform` writes on standard error, in full.
    pub ants_report: String,
}

impl std::fmt::Display for ReadFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.reason)
    }
}

/// ITK's `ExceptionObject::Print` of an `itkExceptionMacro` raised at `site`, as ANTs's reader
/// prints it, addresses written as `0x0` and the ITK tree as `<ITK source>`.
pub fn itk_exception(path: &str, site: ItkSite, description: &str) -> String {
    format!(
        "Transform reader for {path} caught an ITK exception:\n\nitk::ExceptionObject (0x0)\n\
         Location: \"unknown\" \nFile: <ITK source>/{}\nLine: {}\nDescription: ITK ERROR: {}(0x0): \
         {description}\n\n",
        site.file, site.line, site.class
    )
}

fn failure(
    path: &str,
    site: ItkSite,
    description: &str,
    hdf5: Option<&Hdf5Failure>,
    reason: String,
) -> ReadFailure {
    let mut ants_report = String::new();
    if let Some(h) = hdf5 {
        ants_report.push_str(&h.diag());
    }
    ants_report.push_str(&itk_exception(path, site, description));
    ReadFailure {
        reason,
        ants_report,
    }
}

/// The message of `TransformFileReaderTemplate::Update` when no transform IO can read `path`
/// (the IOs of the ANTs 2.6.5 build: HDF5, MATLAB and text, in double and float).
pub fn no_transform_io(path: &str, hdf5: Option<&Hdf5Failure>, why: &str) -> ReadFailure {
    let description = format!(
        "Could not create Transform IO object for reading file {path}\n  Tried to create one of \
         the following:\n    HDF5TransformIOTemplate\n    HDF5TransformIOTemplate\n    \
         MatlabTransformIOTemplate\n    MatlabTransformIOTemplate\n    TxtTransformIOTemplate\n    \
         TxtTransformIOTemplate\n  You probably failed to set a file suffix, or\n    set the \
         suffix to an unsupported type.\n"
    );
    failure(
        path,
        SITE_NO_IO,
        &description,
        hdf5,
        format!("{path}: not an ITK transform file ({why})"),
    )
}

/// `HDF5TransformIOTemplate::CanReadFile`: an HDF5 file with a `/TransformGroup` link. Returns
/// the open file, or why it cannot be read (with the HDF5 failure `H5Fopen` reports, if any).
pub fn can_read(path: &Path) -> Result<File, (String, Option<Hdf5Failure>)> {
    match File::is_hdf5(path) {
        Ok(true) => {}
        Ok(false) => return Err(("no HDF5 signature".into(), None)),
        Err(e) => return Err((e.to_string(), None)),
    }
    let file = match File::open(path) {
        Ok(f) => f,
        Err(e) => {
            let cause = cause_of(&e);
            return Err((
                format!("HDF5 file cannot be opened: {cause}"),
                Some(Hdf5Failure {
                    api: "H5Fopen",
                    cause,
                }),
            ));
        }
    };
    match file.exists("/TransformGroup") {
        Ok(true) => Ok(file),
        Ok(false) => Err(("no /TransformGroup in the HDF5 file".into(), None)),
        Err(e) => Err((e.to_string(), None)),
    }
}

fn cause_of(e: &H5Error) -> String {
    match e {
        H5Error::Corrupt(m) => m.clone(),
        H5Error::NotFound(p) => {
            let last = p.rsplit('/').next().unwrap_or(p);
            format!("object '{last}' doesn't exist")
        }
        other => other.to_string(),
    }
}

/// Reads the transforms of an ITK HDF5 file, as `HDF5TransformIOTemplate<T>::Read` does for
/// the computation type `precision` (`T`). The parts come in file order, the composite first.
pub fn read(
    path: &Path,
    precision: Precision,
    notices: &mut Notices,
) -> Result<Vec<ItkTransformParts>, ReadFailure> {
    let name = path.display().to_string();
    let file = can_read(path).map_err(|(why, h)| no_transform_io(&name, h.as_ref(), &why))?;
    read_open(&file, &name, precision, notices)
}

fn h5_fail(path: &str, api: &'static str, e: &H5Error, what: &str) -> ReadFailure {
    let detail = match api {
        "H5Gopen2" => "H5Gopen2 failed",
        "H5Dopen2" => "H5Dopen2 failed",
        "H5Dread-string" => "H5Dread failed for variable length string",
        _ => "H5Dread failed",
    };
    let api = if api == "H5Dread-string" {
        "H5Dread"
    } else {
        api
    };
    let h = Hdf5Failure {
        api,
        cause: cause_of(e),
    };
    let reason = format!("{path}: {what}: {}", h.cause);
    failure(path, SITE_H5, detail, Some(&h), reason)
}

/// Opens the group at `p` (`H5File::openGroup`).
fn open_group(file: &File, name: &str, p: &str) -> Result<u64, ReadFailure> {
    let addr = file
        .resolve(p)
        .map_err(|e| h5_fail(name, "H5Gopen2", &e, &format!("cannot open group {p}")))?;
    match file.kind(addr) {
        Ok(ObjectKind::Group) => Ok(addr),
        Ok(_) => Err(h5_fail(
            name,
            "H5Gopen2",
            &H5Error::Corrupt("not a group".into()),
            &format!("{p} is not a group"),
        )),
        Err(e) => Err(h5_fail(
            name,
            "H5Gopen2",
            &e,
            &format!("cannot open group {p}"),
        )),
    }
}

/// Opens the dataset at `p` (`H5File::openDataSet`).
fn open_dataset(
    file: &File,
    name: &str,
    p: &str,
) -> Result<larmorx_hdf5::DatasetInfo, ReadFailure> {
    let fail = |e: &H5Error| h5_fail(name, "H5Dopen2", e, &format!("cannot open dataset {p}"));
    let addr = file.resolve(p).map_err(|e| fail(&e))?;
    match file.kind(addr) {
        Ok(ObjectKind::Dataset) => file.dataset(addr).map_err(|e| fail(&e)),
        Ok(_) => Err(fail(&H5Error::Corrupt("not a dataset".into()))),
        Err(e) => Err(fail(&e)),
    }
}

fn read_open(
    file: &File,
    name: &str,
    precision: Precision,
    notices: &mut Notices,
) -> Result<Vec<ItkTransformParts>, ReadFailure> {
    let group = open_group(file, name, "/TransformGroup")?;
    let n = file
        .links(group)
        .map_err(|e| h5_fail(name, "H5Gopen2", &e, "cannot list /TransformGroup"))?
        .len();
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let tname = format!("/TransformGroup/{i}");
        open_group(file, name, &tname)?;
        let ttype = read_transform_type(file, name, &format!("{tname}/TransformType"), notices)?;
        let corrected = check_precision_name(name, &ttype, precision, notices)?;
        check_registered(name, &corrected)?;
        if ttype.contains("CompositeTransform") {
            out.push(ItkTransformParts {
                name: ttype,
                parameters: Parameters::F64(Vec::new()),
                fixed_parameters: Vec::new(),
            });
            continue;
        }
        let pick = |correct: &str, misspelt: &str| -> Result<String, ReadFailure> {
            let p = format!("{tname}/{correct}");
            match file.exists(&p) {
                Ok(true) => Ok(p),
                Ok(false) => Ok(format!("{tname}/{misspelt}")),
                Err(e) => Err(h5_fail(name, "H5Dopen2", &e, &format!("cannot open {p}"))),
            }
        };
        let fixed_path = pick("TransformFixedParameters", "TranformFixedParameters")?;
        let fixed = read_values(file, name, &fixed_path, true, Precision::Double, notices)?;
        let params_path = pick("TransformParameters", "TranformParameters")?;
        let params = read_values(file, name, &params_path, false, precision, notices)?;
        out.push(ItkTransformParts {
            name: ttype,
            parameters: params,
            fixed_parameters: fixed.to_f64().into_owned(),
        });
    }
    if out.is_empty() {
        let description =
            format!("Transform IO: HDF5TransformIOTemplate\n   failed to read file: {name}\n");
        return Err(failure(
            name,
            SITE_EMPTY,
            &description,
            None,
            format!("{name}: no transform in /TransformGroup"),
        ));
    }
    Ok(out)
}

/// `TransformType`: one variable-length string, read into a 1-element memory space.
fn read_transform_type(
    file: &File,
    name: &str,
    p: &str,
    notices: &mut Notices,
) -> Result<String, ReadFailure> {
    let info = open_dataset(file, name, p)?;
    let string_fail = |cause: &str| {
        h5_fail(
            name,
            "H5Dread-string",
            &H5Error::Conversion(cause.into()),
            &format!("cannot read {p}"),
        )
    };
    let fixed = match &info.dtype {
        Datatype::VarLen { string: true, .. } => false,
        Datatype::String { .. } => true,
        _ => return Err(string_fail("no appropriate function for conversion path")),
    };
    if fixed && notices.reproduce() {
        return Err(string_fail("no appropriate function for conversion path"));
    }
    if info.len() != 1 {
        return Err(string_fail(
            "src and dest dataspaces have different number of elements selected",
        ));
    }
    let values = file
        .read_strings(&info)
        .map_err(|e| h5_fail(name, "H5Dread-string", &e, &format!("cannot read {p}")))?;
    let Some(bytes) = values.into_iter().next().flatten() else {
        // ITK assigns the null pointer HDF5 returns to a std::string, which crashes.
        return Err(ReadFailure {
            reason: format!("{name}: {p} is a null string (ANTs crashes on it)"),
            ants_report: format!(
                "larmorx: {name}: {p} is a null string; ANTs crashes reading it\n"
            ),
        });
    };
    // A C string: it ends at its first NUL.
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    let text = String::from_utf8_lossy(&bytes[..end]).into_owned();
    if fixed {
        notices.add(
            &defects::ITK_12,
            format!("{p} is a fixed-length string, which ITK cannot read: read as {text:?}"),
        );
    }
    Ok(text)
}

/// `CorrectTransformPrecisionType`: a name without the reader's precision must contain the
/// other one, which `std::string::replace` swaps in; without either, `replace` throws.
fn check_precision_name(
    name: &str,
    ttype: &str,
    precision: Precision,
    notices: &mut Notices,
) -> Result<String, ReadFailure> {
    let (own, other) = match precision {
        Precision::Double => ("double", "float"),
        Precision::Float => ("float", "double"),
    };
    if ttype.contains(own) {
        return Ok(ttype.to_string());
    }
    if let Some(at) = ttype.find(other) {
        let mut corrected = ttype.to_string();
        corrected.replace_range(at..at + other.len(), own);
        return Ok(corrected);
    }
    let reason = format!("{name}: transform type {ttype:?} names no precision (float or double)");
    let ants_report = if notices.reproduce() {
        format!(
            "Transform reader for {name} caught an exception:\nbasic_string::replace: __pos (which \
             is 18446744073709551615) > this->size() (which is {})\n",
            ttype.len()
        )
    } else {
        format!(
            "Transform reader for {name} caught an exception:\nthe transform type {ttype:?} names \
             no precision (float or double)\n"
        )
    };
    Err(ReadFailure {
        reason,
        ants_report,
    })
}

/// ITK's transform factory: the class names of the ANTs 2.6.5 build (the ones larmorx can use
/// are a subset; others are refused later with larmorx's own message).
fn check_registered(name: &str, ttype: &str) -> Result<(), ReadFailure> {
    if REGISTERED.contains(&ttype) {
        return Ok(());
    }
    let mut description = format!(
        "Could not create an instance of \"{ttype}\"\nThe usual cause of this error is not \
         registering the transform with TransformFactory\nCurrently registered Transforms: \n"
    );
    for r in REGISTERED {
        description.push_str(&format!("\t\"{r}\"\n"));
    }
    Err(failure(
        name,
        SITE_CREATE,
        &description,
        None,
        format!("{name}: unknown transform type {ttype:?}"),
    ))
}

/// The transforms registered with ITK's `TransformFactory` in the ANTs 2.6.5 oracle, in the
/// order `GetClassOverrideWithNames` lists them (filled in from the oracle's message).
pub const REGISTERED: &[&str] = &crate::itk_transform_names::REGISTERED;

/// A parameter dataset (`ReadParameters`, `ReadFixedParameters`).
fn read_values(
    file: &File,
    name: &str,
    p: &str,
    fixed: bool,
    precision: Precision,
    notices: &mut Notices,
) -> Result<Parameters, ReadFailure> {
    let info = open_dataset(file, name, p)?;
    let Datatype::Float(ft) = &info.dtype else {
        let site = if fixed {
            SITE_FIXED_TYPE
        } else {
            SITE_PARAM_TYPE
        };
        // ITK-13: "<path>in HDF5 File", without a blank.
        let description = if notices.reproduce() {
            format!("Wrong data type for {p}in HDF5 File")
        } else {
            format!("Wrong data type for {p} in HDF5 File")
        };
        return Err(failure(
            name,
            site,
            &description,
            None,
            format!("{name}: {p} is not floating point"),
        ));
    };
    if info.rank() != 1 {
        let site = if fixed {
            SITE_FIXED_DIMS
        } else {
            SITE_PARAM_DIMS
        };
        let description = if notices.reproduce() {
            "Wrong # of dims for TransformType in HDF5 File".to_string()
        } else {
            format!("Wrong # of dims for {p} in HDF5 File")
        };
        return Err(failure(
            name,
            site,
            &description,
            None,
            format!("{name}: {p} has {} dimensions, not 1", info.rank()),
        ));
    }
    // ITK: 8 bytes are read as double, anything else as float.
    if ft.size == 8 {
        return Ok(Parameters::F64(read_floats(
            file, name, p, &info, true, notices,
        )?));
    }
    // ITK-14: a type wider than 8 bytes goes through float; larmorx reads it in the reader's
    // precision (fixed parameters are always double).
    let wants_double = fixed || precision == Precision::Double;
    if ft.size > 8 && wants_double {
        let direct = read_floats(file, name, p, &info, true, notices)?;
        let through_float = read_floats(file, name, p, &info, false, notices)?;
        let differs = direct
            .iter()
            .zip(&through_float)
            .any(|(a, b)| a.to_bits() != b.to_bits());
        if differs {
            notices.add(
                &defects::ITK_14,
                format!(
                    "{p} holds {}-byte floats, which ITK reads as float",
                    ft.size
                ),
            );
        }
        if notices.fix() {
            return Ok(Parameters::F64(direct));
        }
        return Ok(Parameters::F32(
            through_float.into_iter().map(|x| x as f32).collect(),
        ));
    }
    let v = read_floats(file, name, p, &info, false, notices)?;
    Ok(Parameters::F32(v.into_iter().map(|x| x as f32).collect()))
}

/// A float dataset as `H5Dread` converts it to native double or float (widened to f64).
fn read_floats(
    file: &File,
    name: &str,
    p: &str,
    info: &larmorx_hdf5::DatasetInfo,
    as_double: bool,
    notices: &mut Notices,
) -> Result<Vec<f64>, ReadFailure> {
    let (v, state_changed) = file
        .read_floats(info, as_double, 0, notices.reproduce())
        .map_err(|e| h5_fail(name, "H5Dread", &e, &format!("cannot read {p}")))?;
    if state_changed {
        notices.add(
            &defects::HDF_01,
            format!("{p}: values converted from a non-native floating-point format"),
        );
    }
    Ok(v)
}
