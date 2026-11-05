// SPDX-License-Identifier: Apache-2.0
//! FSL's standard images, as the clean-room programs find them (`specs/bet.md` §5.4).
//!
//! `standard_space_roi` (and BET's `-S`, `-B`, `-A`, `-A2`) read three images of FSL 6.0.7.23's
//! data package `fsl-data_standard` 2208.0. They are FSL's data under the FSL Licence (the MNI152
//! template also carries the McConnell Brain Imaging Centre's notice): this package (Apache-2.0)
//! never ships them, and reads them at run time as input data (the user's decision of
//! 2026-11-04). Each image is looked for, in order:
//! 1. the directory the call names (`LARMORX_FSL_DATA`, Python's `fsl_data=`), laid out as FSL's
//!    `data` (`<dir>/standard/<image>` with any of FSL's image extensions); when it is given
//!    nothing else is searched;
//! 2. larmorx-nc's copy, when its program is found (`LARMORX_NC_BIN`, else `PATH`): the files
//!    its wheel installs at `<prefix>/share/larmorx-nc/fsl-data/standard/`, read as data only
//!    (nothing of larmorx-nc is run or linked), checked against the table's SHA-256;
//! 3. `$FSLDIR/data/standard/`;
//! 4. larmorx's data cache (`LARMORX_DATA_DIR`, else the platform's user cache directory),
//!    filled once by a download of FSL's package (`LARMORX_FSL_DATA_PACKAGE`: a local copy of
//!    the package or another address of it), checked against the package's and every image's
//!    SHA-256, of which only the three images are kept.
//!
//! The download runs the platform's `curl` (Windows 10 and later, macOS and Linux ship it; else
//! `wget`) as a child process: larmorx links no TLS library (pure-Rust dependencies only), and
//! the checksums, not the transport, make the files trustworthy.

use std::fmt;
use std::fs::File;
use std::io::{self, BufReader, Read, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::cli::Settings;

/// One of FSL's standard images: its name in `data/standard/` (without extension) and the
/// `.nii.gz` file's size and SHA-256 (FSL 6.0.7.23's `fsl-data_standard` 2208.0; `specs/bet.md`
/// §5.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StandardImage {
    pub name: &'static str,
    pub bytes: u64,
    pub sha256: &'static str,
}

/// FSL's 2 mm MNI152 T1 (int16, 91 × 109 × 91): standard_space_roi's default reference.
pub const MNI152_T1_2MM: StandardImage = StandardImage {
    name: "MNI152_T1_2mm",
    bytes: 1_406_977,
    sha256: "0585cd056bf5ccfb8bf97a5f6a66082d4e7caad525718fc11e40d80a827fcb92",
};

/// Its dilated brain mask (uint8): standard_space_roi `-b`.
pub const MNI152_T1_2MM_BRAIN_MASK_DIL: StandardImage = StandardImage {
    name: "MNI152_T1_2mm_brain_mask_dil",
    bytes: 11_132,
    sha256: "342d41e1c445d87a812ca288786a116306e96a5595a198e46ba7e7515d4f221c",
};

/// Its eye mask (uint8): `bet -S`.
pub const MNI152_T1_2MM_EYE_MASK: StandardImage = StandardImage {
    name: "MNI152_T1_2mm_eye_mask",
    bytes: 2_788,
    sha256: "5ce2da5f524da4562d2274192010c69e152b83166f214ac75dbcc5a778493ad5",
};

/// The three images, in the package's order of need.
pub const IMAGES: [StandardImage; 3] = [
    MNI152_T1_2MM,
    MNI152_T1_2MM_BRAIN_MASK_DIL,
    MNI152_T1_2MM_EYE_MASK,
];

/// FSL's conda package that holds them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Package {
    /// The package and its version, also the cache's directory name.
    pub name: &'static str,
    pub url: &'static str,
    pub bytes: u64,
    pub sha256: &'static str,
}

/// FSL 6.0.7.23's `fsl-data_standard` 2208.0 (`specs/bet.md` §5.4).
pub const PACKAGE: Package = Package {
    name: "fsl-data_standard-2208.0-0",
    url: "https://fsl.fmrib.ox.ac.uk/fsldownloads/fslconda/public/noarch/fsl-data_standard-2208.0-0.tar.bz2",
    bytes: 196_312_969,
    sha256: "2faefb4d366b6427cbf4e5c3e54c63c040ec9bcfe11b1f10848f89a1571b828a",
};

/// The directory a call names (laid out as FSL's `data`): when set, the only place searched.
pub const DATA_ENV: &str = "LARMORX_FSL_DATA";
/// larmorx's data cache (default: the platform's user cache directory, `larmorx` in it).
pub const CACHE_ENV: &str = "LARMORX_DATA_DIR";
/// Where the package is fetched from instead of FSL's address: a local file (a copy made on
/// another machine, for offline machines and tests) or another URL.
pub const PACKAGE_ENV: &str = "LARMORX_FSL_DATA_PACKAGE";
/// The replica's program (larmorx-nc), whose installed copy of the images is read as data.
pub const NC_ENV: &str = "LARMORX_NC_BIN";

/// What a search reads besides the settings: the table of images and the package (FSL's, or a
/// test's).
#[derive(Clone, Copy, Debug)]
pub struct Catalog<'a> {
    pub images: &'a [StandardImage],
    pub package: Package,
}

/// FSL's catalog.
pub const FSL: Catalog<'static> = Catalog {
    images: &IMAGES,
    package: PACKAGE,
};

/// Where an image was found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    /// The image's name without extension, as FSL's scripts name it (`<dir>/standard/<image>`).
    pub stem: String,
    /// The file.
    pub file: PathBuf,
    /// Which place of the search (`LARMORX_FSL_DATA`, `larmorx-nc`, `FSLDIR`, `cache`).
    pub place: &'static str,
}

/// Why an image is not available: every place looked, and what went wrong where something did
/// (a damaged file, a download that failed).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Missing {
    pub image: &'static str,
    pub looked: Vec<String>,
    pub problem: Option<String>,
}

impl fmt::Display for Missing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "FSL's standard image {} was not found (looked in: {})",
            self.image,
            self.looked.join("; ")
        )?;
        if let Some(p) = &self.problem {
            write!(f, "; {p}")?;
        }
        write!(
            f,
            ". It is FSL's data (FSL Licence, non-commercial use). Provide it with \
             {DATA_ENV}=<a directory laid out as $FSLDIR/data> (Python: fsl_data=), an FSL \
             installation's FSLDIR, the larmorx-nc package, or `larmorx fetch fsl-standard` on \
             a connected machine ({PACKAGE_ENV}=<a copy of {}.tar.bz2> offline)",
            PACKAGE.name
        )
    }
}

/// FSL's image extensions, in the order FSL's names are completed (`specs/bet.md` §3.1).
const EXTENSIONS: [&str; 8] = [
    ".nii.gz", ".nii", ".hdr", ".hdr.gz", ".nii.zst", ".hdr.zst", ".nii.bz2", ".hdr.bz2",
];

/// The file of `<stem>` with one of FSL's extensions, if any.
fn with_extension(stem: &Path) -> Option<PathBuf> {
    EXTENSIONS.iter().find_map(|e| {
        let mut s = stem.as_os_str().to_owned();
        s.push(e);
        let p = PathBuf::from(s);
        p.is_file().then_some(p)
    })
}

fn stem_text(dir: &Path, image: &StandardImage) -> String {
    dir.join("standard")
        .join(image.name)
        .to_string_lossy()
        .into_owned()
}

/// The SHA-256 of a file, in hex.
pub fn sha256_file(path: &Path) -> io::Result<String> {
    let mut f = File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(hex(&h.finalize()))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// A file checked against the table: `Ok` when its SHA-256 is the image's.
fn checked(path: &Path, image: &StandardImage) -> Result<(), String> {
    match sha256_file(path) {
        Ok(h) if h == image.sha256 => Ok(()),
        Ok(h) => Err(format!(
            "{} is damaged (SHA-256 {h}, FSL's file has {})",
            path.display(),
            image.sha256
        )),
        Err(e) => Err(format!("{} cannot be read: {e}", path.display())),
    }
}

/// The `larmorx-nc` program: `LARMORX_NC_BIN`, else the first on the settings' `PATH`.
fn nc_program(settings: &Settings) -> Option<PathBuf> {
    if let Some(p) = settings.get(NC_ENV).filter(|p| !p.is_empty()) {
        return Some(PathBuf::from(p));
    }
    let exe = if cfg!(windows) {
        "larmorx-nc.exe"
    } else {
        "larmorx-nc"
    };
    let path = settings.get("PATH")?;
    std::env::split_paths(path)
        .map(|d| d.join(exe))
        .find(|p| p.is_file())
}

/// The directories where larmorx-nc's wheel installs its data beside its program: the
/// program's directory's parent (`<prefix>/bin`, `<prefix>\Scripts`), and one level higher
/// (Windows' per-user scheme, `<userbase>\Python3XY\Scripts`).
fn nc_data_dirs(program: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut d = program.parent().and_then(Path::parent);
    for _ in 0..2 {
        let Some(p) = d else { break };
        out.push(p.join("share").join("larmorx-nc").join("fsl-data"));
        d = p.parent();
    }
    out
}

/// larmorx's data cache: `LARMORX_DATA_DIR`, else the platform's user cache directory
/// (`$XDG_CACHE_HOME` or `~/.cache` on Linux, `~/Library/Caches` on macOS, `%LOCALAPPDATA%`
/// on Windows) with `larmorx` in it.
pub fn cache_dir(settings: &Settings) -> Option<PathBuf> {
    let var = |k: &str| settings.get(k).filter(|v| !v.is_empty()).map(PathBuf::from);
    if let Some(d) = var(CACHE_ENV) {
        return Some(d);
    }
    if cfg!(windows) {
        return var("LOCALAPPDATA").map(|d| d.join("larmorx").join("cache"));
    }
    if cfg!(target_os = "macos") {
        return var("HOME").map(|d| d.join("Library").join("Caches").join("larmorx"));
    }
    var("XDG_CACHE_HOME")
        .or_else(|| var("HOME").map(|d| d.join(".cache")))
        .map(|d| d.join("larmorx"))
}

/// The cache's directory of `package`: `<cache>/fsl/<package>`, laid out as FSL's `data`.
pub fn package_dir(cache: &Path, package: &Package) -> PathBuf {
    cache.join("fsl").join(package.name)
}

/// Finds `image` (§5.4's order); `log` gets the download's notice.
pub fn find(
    settings: &Settings,
    catalog: &Catalog<'_>,
    image: &StandardImage,
    log: &mut dyn Write,
) -> Result<Found, Missing> {
    let mut looked = Vec::new();
    let missing = |looked: Vec<String>, problem: Option<String>| Missing {
        image: image.name,
        looked,
        problem,
    };
    // 1. the named directory, alone
    if let Some(d) = settings.get(DATA_ENV).filter(|d| !d.is_empty()) {
        let dir = settings.path(d);
        let stem = dir.join("standard").join(image.name);
        if let Some(file) = with_extension(&stem) {
            return Ok(Found {
                stem: stem_text(&dir, image),
                file,
                place: DATA_ENV,
            });
        }
        looked.push(format!("{DATA_ENV}={}", stem.display()));
        return Err(missing(looked, None));
    }
    // 2. larmorx-nc's copy, read as data
    match nc_program(settings) {
        Some(program) => {
            for dir in nc_data_dirs(&program) {
                let file = dir.join("standard").join(format!("{}.nii.gz", image.name));
                if file.is_file() {
                    return match checked(&file, image) {
                        Ok(()) => Ok(Found {
                            stem: stem_text(&dir, image),
                            file,
                            place: "larmorx-nc",
                        }),
                        Err(p) => {
                            looked.push(format!("larmorx-nc's copy {}", file.display()));
                            Err(missing(looked, Some(p)))
                        }
                    };
                }
                looked.push(format!("larmorx-nc's copy {}", file.display()));
            }
        }
        None => looked.push("larmorx-nc's copy (its program not found)".to_owned()),
    }
    // 3. FSLDIR
    match settings.get("FSLDIR").filter(|d| !d.is_empty()) {
        Some(d) => {
            let dir = settings.path(d).join("data");
            let stem = dir.join("standard").join(image.name);
            if let Some(file) = with_extension(&stem) {
                return Ok(Found {
                    stem: stem_text(&dir, image),
                    file,
                    place: "FSLDIR",
                });
            }
            looked.push(format!("$FSLDIR/data/standard/{}", image.name));
        }
        None => looked.push("$FSLDIR (not set)".to_owned()),
    }
    // 4. the cache, filled by the download
    let Some(cache) = cache_dir(settings) else {
        looked.push(format!(
            "larmorx's data cache (no {CACHE_ENV}, no home directory)"
        ));
        return Err(missing(looked, None));
    };
    let dir = package_dir(&cache, &catalog.package);
    let file = dir.join("standard").join(format!("{}.nii.gz", image.name));
    looked.push(format!("larmorx's data cache {}", file.display()));
    if !file.is_file() {
        let source = settings
            .get(PACKAGE_ENV)
            .filter(|s| !s.is_empty())
            .map_or_else(|| catalog.package.url.to_owned(), str::to_owned);
        if let Err(p) = fetch(settings, catalog, &source, &cache, log) {
            return Err(missing(looked, Some(p)));
        }
    }
    match checked(&file, image) {
        Ok(()) => Ok(Found {
            stem: stem_text(&dir, image),
            file,
            place: "cache",
        }),
        Err(p) => Err(missing(looked, Some(p))),
    }
}

/// Fills the cache with the catalog's images from the package at `source` (a local file, a
/// `file://` URL, or an `http(s)://` URL fetched by `curl` or `wget`): the package checked
/// against its size and SHA-256, its members `data/standard/<image>.nii.gz` against theirs, only
/// those kept (with a `MANIFEST.tsv`). Returns the cache's package directory.
pub fn fetch(
    settings: &Settings,
    catalog: &Catalog<'_>,
    source: &str,
    cache: &Path,
    log: &mut dyn Write,
) -> Result<PathBuf, String> {
    let dir = package_dir(cache, &catalog.package);
    let standard = dir.join("standard");
    std::fs::create_dir_all(&standard)
        .map_err(|e| format!("the data cache {} cannot be made: {e}", standard.display()))?;
    let local = source.strip_prefix("file://").map_or_else(
        || (!source.contains("://")).then(|| settings.path(source)),
        |p| Some(PathBuf::from(p)),
    );
    let (package_file, temporary) = match local {
        Some(p) => (p, false),
        None => {
            let _ = writeln!(
                log,
                "larmorx: downloading FSL's data package {} ({} MB) from {source}, once, into {}: \
                 FSL's standard images are FSL's data, under the FSL Licence (non-commercial use \
                 only; the MNI152 template also under its MNI notice)",
                catalog.package.name,
                catalog.package.bytes / 1_000_000,
                dir.display()
            );
            let tmp = dir.join(format!(".download-{}.tar.bz2", std::process::id()));
            download(source, &tmp)?;
            (tmp, true)
        }
    };
    let result = extract(catalog, &package_file, &dir);
    if temporary {
        let _ = std::fs::remove_file(&package_file);
    }
    result.map(|()| dir)
}

/// Runs `curl` (else `wget`) to save `url` at `dest`.
fn download(url: &str, dest: &Path) -> Result<(), String> {
    let attempts: [(&str, Vec<String>); 2] = [
        (
            "curl",
            vec![
                "-fsSL".into(),
                "--retry".into(),
                "3".into(),
                "-o".into(),
                dest.to_string_lossy().into_owned(),
                url.into(),
            ],
        ),
        (
            "wget",
            vec![
                "-q".into(),
                "-O".into(),
                dest.to_string_lossy().into_owned(),
                url.into(),
            ],
        ),
    ];
    let mut tried = Vec::new();
    for (program, args) in attempts {
        match std::process::Command::new(program)
            .args(&args)
            .stdin(std::process::Stdio::null())
            .status()
        {
            Ok(s) if s.success() => return Ok(()),
            Ok(s) => {
                let _ = std::fs::remove_file(dest);
                return Err(format!("{program} could not download {url} ({s})"));
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => tried.push(program),
            Err(e) => return Err(format!("{program} could not be run: {e}")),
        }
    }
    Err(format!(
        "no downloader found ({} are needed to fetch {url})",
        tried.join(" or ")
    ))
}

/// Checks the package file and keeps its standard images in `dir`.
fn extract(catalog: &Catalog<'_>, package: &Path, dir: &Path) -> Result<(), String> {
    let meta = std::fs::metadata(package)
        .map_err(|e| format!("the package {} cannot be read: {e}", package.display()))?;
    if meta.len() != catalog.package.bytes {
        return Err(format!(
            "the package {} has {} bytes, FSL's has {}: not FSL's {}",
            package.display(),
            meta.len(),
            catalog.package.bytes,
            catalog.package.name
        ));
    }
    let h = sha256_file(package).map_err(|e| format!("{}: {e}", package.display()))?;
    if h != catalog.package.sha256 {
        return Err(format!(
            "the package {} is damaged or another file (SHA-256 {h}, FSL's has {})",
            package.display(),
            catalog.package.sha256
        ));
    }
    let file = File::open(package).map_err(|e| format!("{}: {e}", package.display()))?;
    let mut tar = bzip2::read::MultiBzDecoder::new(BufReader::new(file));
    let mut wanted: Vec<&StandardImage> = catalog.images.iter().collect();
    let mut kept = Vec::new();
    while !wanted.is_empty() {
        let Some((name, size)) =
            next_member(&mut tar).map_err(|e| format!("{}: {e}", package.display()))?
        else {
            break;
        };
        let at = wanted
            .iter()
            .position(|i| name == format!("data/standard/{}.nii.gz", i.name));
        let padded = size.div_ceil(512) * 512;
        let Some(at) = at else {
            io::copy(&mut (&mut tar).take(padded), &mut io::sink())
                .map_err(|e| format!("{}: {e}", package.display()))?;
            continue;
        };
        let image = wanted.remove(at);
        let mut bytes = vec![0u8; size as usize];
        tar.read_exact(&mut bytes)
            .map_err(|e| format!("{}: {e}", package.display()))?;
        io::copy(&mut (&mut tar).take(padded - size), &mut io::sink())
            .map_err(|e| format!("{}: {e}", package.display()))?;
        let h = hex(&Sha256::digest(&bytes));
        if h != image.sha256 || bytes.len() as u64 != image.bytes {
            return Err(format!(
                "the package's {name} is not FSL's file (SHA-256 {h}, FSL's has {})",
                image.sha256
            ));
        }
        let dest = dir.join("standard").join(format!("{}.nii.gz", image.name));
        let tmp =
            dir.join("standard")
                .join(format!(".{}.{}.partial", image.name, std::process::id()));
        std::fs::write(&tmp, &bytes).map_err(|e| format!("{}: {e}", tmp.display()))?;
        std::fs::rename(&tmp, &dest).map_err(|e| format!("{}: {e}", dest.display()))?;
        kept.push(*image);
    }
    if let Some(first) = wanted.first() {
        return Err(format!(
            "the package {} holds no data/standard/{}.nii.gz",
            package.display(),
            first.name
        ));
    }
    let mut manifest = String::from("file\tbytes\tsha256\n");
    for i in &kept {
        manifest.push_str(&format!(
            "standard/{}.nii.gz\t{}\t{}\n",
            i.name, i.bytes, i.sha256
        ));
    }
    manifest.push_str(&format!(
        "# from {} ({} bytes, SHA-256 {}): FSL's data, FSL Licence (non-commercial use)\n",
        catalog.package.name, catalog.package.bytes, catalog.package.sha256
    ));
    std::fs::write(dir.join("MANIFEST.tsv"), manifest)
        .map_err(|e| format!("{}: {e}", dir.display()))?;
    Ok(())
}

/// The next regular file of a tar stream: its name (ustar's prefix joined, a pax `path` or a
/// GNU long name applied) and size; the stream left at its data. `None` at the archive's end.
fn next_member(r: &mut dyn Read) -> io::Result<Option<(String, u64)>> {
    let mut long: Option<String> = None;
    loop {
        let mut h = [0u8; 512];
        if !read_block(r, &mut h)? || h.iter().all(|&b| b == 0) {
            return Ok(None);
        }
        let size = octal(&h[124..136]);
        let kind = h[156];
        let padded = size.div_ceil(512) * 512;
        let field = |a: &[u8]| -> String {
            let end = a.iter().position(|&b| b == 0).unwrap_or(a.len());
            String::from_utf8_lossy(&a[..end]).into_owned()
        };
        match kind {
            // pax extended header: its `path` applies to the next member; GNU long name
            b'x' | b'L' => {
                let mut data = vec![0u8; padded as usize];
                r.read_exact(&mut data)?;
                let data = &data[..size as usize];
                if kind == b'L' {
                    long = Some(field(data));
                } else {
                    long = pax_path(data).or(long);
                }
            }
            b'0' | 0 | b'7' => {
                let name = long.take().unwrap_or_else(|| {
                    let (name, prefix) = (field(&h[0..100]), field(&h[345..500]));
                    if &h[257..262] == b"ustar" && !prefix.is_empty() {
                        format!("{prefix}/{name}")
                    } else {
                        name
                    }
                });
                return Ok(Some((name.trim_start_matches("./").to_owned(), size)));
            }
            _ => {
                // directories, links, global headers: skipped with their data
                io::copy(&mut r.take(padded), &mut io::sink())?;
                long = None;
            }
        }
    }
}

/// Reads one 512-byte block; `false` at a clean end of the stream.
fn read_block(r: &mut dyn Read, h: &mut [u8; 512]) -> io::Result<bool> {
    let mut got = 0;
    while got < h.len() {
        let n = r.read(&mut h[got..])?;
        if n == 0 {
            return if got == 0 {
                Ok(false)
            } else {
                Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "a tar header cut short",
                ))
            };
        }
        got += n;
    }
    Ok(true)
}

/// A tar number field: octal digits (blanks and NULs around them), or GNU's base-256 form.
fn octal(field: &[u8]) -> u64 {
    if field.first().is_some_and(|&b| b & 0x80 != 0) {
        return field[1..]
            .iter()
            .fold(u64::from(field[0] & 0x7f), |a, &b| (a << 8) | u64::from(b));
    }
    field
        .iter()
        .skip_while(|&&b| b == b' ' || b == 0)
        .take_while(|&&b| (b'0'..=b'7').contains(&b))
        .fold(0, |a, &b| a * 8 + u64::from(b - b'0'))
}

/// The `path` record of a pax header (`<length> path=<name>\n`).
fn pax_path(data: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(data);
    text.lines().find_map(|line| {
        let (_, rec) = line.split_once(' ')?;
        rec.strip_prefix("path=").map(str::to_owned)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn settings(vars: &[(&str, &str)]) -> Settings {
        Settings {
            vars: vars
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect::<BTreeMap<_, _>>(),
            dir: None,
            stdin: None,
        }
    }

    /// A ustar member header.
    fn header(name: &str, size: usize, kind: u8) -> [u8; 512] {
        let mut h = [0u8; 512];
        h[..name.len()].copy_from_slice(name.as_bytes());
        h[100..107].copy_from_slice(b"0000644");
        h[124..135].copy_from_slice(format!("{size:011o}").as_bytes());
        h[156] = kind;
        h[257..263].copy_from_slice(b"ustar\0");
        h[263..265].copy_from_slice(b"00");
        // the checksum: the header's bytes with the field as blanks
        h[148..156].copy_from_slice(b"        ");
        let sum: u32 = h.iter().map(|&b| u32::from(b)).sum();
        h[148..155].copy_from_slice(format!("{sum:06o}\0").as_bytes());
        h
    }

    /// A bzip2-compressed tar of `members` (a pax header before the second one, a directory
    /// and another file between them).
    fn package(members: &[(&str, &[u8])]) -> Vec<u8> {
        let mut tar = Vec::new();
        tar.extend_from_slice(&header("data/", 0, b'5'));
        tar.extend_from_slice(&header("info/index.json", 3, b'0'));
        tar.extend_from_slice(b"{}\n");
        tar.resize(tar.len().div_ceil(512) * 512, 0);
        for (k, (name, data)) in members.iter().enumerate() {
            if k == 1 {
                let rec = format!("path={name}\n");
                let rec = format!("{} {rec}", rec.len() + 3);
                tar.extend_from_slice(&header("PaxHeader", rec.len(), b'x'));
                tar.extend_from_slice(rec.as_bytes());
                tar.resize(tar.len().div_ceil(512) * 512, 0);
                tar.extend_from_slice(&header("ignored-name", data.len(), b'0'));
            } else {
                tar.extend_from_slice(&header(name, data.len(), b'0'));
            }
            tar.extend_from_slice(data);
            tar.resize(tar.len().div_ceil(512) * 512, 0);
        }
        tar.extend_from_slice(&[0u8; 1024]);
        let mut enc = bzip2::write::BzEncoder::new(Vec::new(), bzip2::Compression::fast());
        enc.write_all(&tar).unwrap();
        enc.finish().unwrap()
    }

    fn image_of(name: &'static str, bytes: &[u8]) -> StandardImage {
        StandardImage {
            name,
            bytes: bytes.len() as u64,
            sha256: Box::leak(hex(&Sha256::digest(bytes)).into_boxed_str()),
        }
    }

    struct Fixture {
        _tmp: tempfile::TempDir,
        root: PathBuf,
        images: Vec<StandardImage>,
        package: Package,
        file: PathBuf,
    }

    fn fixture() -> Fixture {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().to_path_buf();
        let a: &[u8] = b"the reference's bytes";
        let b: &[u8] = b"the mask's bytes";
        let images = vec![image_of("REF", a), image_of("MASK", b)];
        let bytes = package(&[
            ("data/standard/REF.nii.gz", a),
            ("data/standard/MASK.nii.gz", b),
        ]);
        let file = root.join("pkg.tar.bz2");
        std::fs::write(&file, &bytes).unwrap();
        let package = Package {
            name: "test-package-1",
            url: "https://example.invalid/test-package-1.tar.bz2",
            bytes: bytes.len() as u64,
            sha256: Box::leak(hex(&Sha256::digest(&bytes)).into_boxed_str()),
        };
        Fixture {
            _tmp: tmp,
            root,
            images,
            package,
            file,
        }
    }

    #[test]
    fn the_named_directory_alone() {
        let f = fixture();
        let named = f.root.join("named");
        std::fs::create_dir_all(named.join("standard")).unwrap();
        std::fs::write(named.join("standard/REF.nii"), b"any bytes").unwrap();
        let fsldir = f.root.join("fsl");
        std::fs::create_dir_all(fsldir.join("data/standard")).unwrap();
        std::fs::write(fsldir.join("data/standard/MASK.nii.gz"), b"x").unwrap();
        let cat = Catalog {
            images: &f.images,
            package: f.package,
        };
        let s = settings(&[
            (DATA_ENV, named.to_str().unwrap()),
            ("FSLDIR", fsldir.to_str().unwrap()),
        ]);
        let mut log = Vec::new();
        let got = find(&s, &cat, &f.images[0], &mut log).unwrap();
        assert_eq!(got.place, DATA_ENV);
        assert!(got.file.ends_with("standard/REF.nii"));
        assert!(got.stem.ends_with("standard/REF"));
        // nothing else is searched when it is given: FSLDIR's MASK not taken
        let e = find(&s, &cat, &f.images[1], &mut log).unwrap_err();
        assert_eq!(e.looked.len(), 1, "{e}");
        assert!(
            e.to_string()
                .contains("FSL's standard image MASK was not found")
        );
    }

    #[test]
    fn larmorx_nc_then_fsldir_then_the_cache() {
        let f = fixture();
        let cat = Catalog {
            images: &f.images,
            package: f.package,
        };
        // larmorx-nc's program and its copy (checked: MASK damaged)
        let prefix = f.root.join("venv");
        std::fs::create_dir_all(prefix.join("bin")).unwrap();
        let program = prefix.join("bin/larmorx-nc");
        std::fs::write(&program, b"").unwrap();
        let data = prefix.join("share/larmorx-nc/fsl-data/standard");
        std::fs::create_dir_all(&data).unwrap();
        std::fs::write(data.join("REF.nii.gz"), b"the reference's bytes").unwrap();
        std::fs::write(data.join("MASK.nii.gz"), b"another file").unwrap();
        let s = settings(&[(NC_ENV, program.to_str().unwrap())]);
        let mut log = Vec::new();
        let got = find(&s, &cat, &f.images[0], &mut log).unwrap();
        assert_eq!(got.place, "larmorx-nc");
        let e = find(&s, &cat, &f.images[1], &mut log).unwrap_err();
        assert!(e.problem.as_deref().unwrap().contains("is damaged"), "{e}");
        // FSLDIR next (no larmorx-nc): used as it is
        let fsldir = f.root.join("fsl");
        std::fs::create_dir_all(fsldir.join("data/standard")).unwrap();
        std::fs::write(fsldir.join("data/standard/MASK.nii.gz"), b"FSL's own").unwrap();
        let s = settings(&[("FSLDIR", fsldir.to_str().unwrap()), ("PATH", "")]);
        let got = find(&s, &cat, &f.images[1], &mut log).unwrap();
        assert_eq!(got.place, "FSLDIR");
        // the cache, pre-filled: no download
        let cache = f.root.join("cache");
        let dir = package_dir(&cache, &f.package).join("standard");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("REF.nii.gz"), b"the reference's bytes").unwrap();
        let s = settings(&[
            (CACHE_ENV, cache.to_str().unwrap()),
            (PACKAGE_ENV, "/nonexistent/package.tar.bz2"),
        ]);
        let got = find(&s, &cat, &f.images[0], &mut log).unwrap();
        assert_eq!(got.place, "cache");
        assert!(log.is_empty());
        // a damaged cached file is refused
        std::fs::write(dir.join("REF.nii.gz"), b"the reference's byteZ").unwrap();
        let e = find(&s, &cat, &f.images[0], &mut log).unwrap_err();
        assert!(e.problem.as_deref().unwrap().contains("is damaged"), "{e}");
        // the package missing: every place named
        let e = find(&s, &cat, &f.images[1], &mut log).unwrap_err();
        let text = e.to_string();
        assert!(
            text.contains("larmorx-nc's copy (its program not found)"),
            "{text}"
        );
        assert!(text.contains("$FSLDIR (not set)"), "{text}");
        assert!(text.contains("larmorx's data cache"), "{text}");
    }

    #[test]
    fn the_download_from_a_local_copy() {
        let f = fixture();
        let cat = Catalog {
            images: &f.images,
            package: f.package,
        };
        let cache = f.root.join("cache");
        let s = settings(&[
            (CACHE_ENV, cache.to_str().unwrap()),
            (PACKAGE_ENV, f.file.to_str().unwrap()),
        ]);
        let mut log = Vec::new();
        let got = find(&s, &cat, &f.images[1], &mut log).unwrap();
        assert_eq!(got.place, "cache");
        let dir = package_dir(&cache, &f.package);
        assert_eq!(
            std::fs::read(dir.join("standard/MASK.nii.gz")).unwrap(),
            b"the mask's bytes"
        );
        assert_eq!(
            std::fs::read(dir.join("standard/REF.nii.gz")).unwrap(),
            b"the reference's bytes"
        );
        // only the images kept, with their manifest
        let mut names: Vec<_> = std::fs::read_dir(dir.join("standard"))
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        assert_eq!(names, ["MASK.nii.gz", "REF.nii.gz"]);
        let manifest = std::fs::read_to_string(dir.join("MANIFEST.tsv")).unwrap();
        assert!(manifest.contains("standard/REF.nii.gz\t21\t"), "{manifest}");
        // a file:// address reads the same copy
        let cache2 = f.root.join("cache2");
        let url = format!("file://{}", f.file.display());
        let dir2 = fetch(&s, &cat, &url, &cache2, &mut log).unwrap();
        assert!(dir2.join("standard/REF.nii.gz").is_file());
    }

    #[test]
    fn a_wrong_package_is_refused() {
        let f = fixture();
        let cat = Catalog {
            images: &f.images,
            package: f.package,
        };
        let mut log = Vec::new();
        let s = settings(&[]);
        // one byte changed: the package's checksum refuses it
        let mut bytes = std::fs::read(&f.file).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 1;
        let bad = f.root.join("bad.tar.bz2");
        std::fs::write(&bad, &bytes).unwrap();
        let e = fetch(
            &s,
            &cat,
            bad.to_str().unwrap(),
            &f.root.join("c1"),
            &mut log,
        )
        .unwrap_err();
        assert!(e.contains("is damaged or another file"), "{e}");
        // another size
        let short = f.root.join("short.tar.bz2");
        std::fs::write(&short, &bytes[..10]).unwrap();
        let e = fetch(
            &s,
            &cat,
            short.to_str().unwrap(),
            &f.root.join("c2"),
            &mut log,
        )
        .unwrap_err();
        assert!(e.contains("not FSL's"), "{e}");
        // a package whose member is not the image's file
        let other = package(&[
            ("data/standard/REF.nii.gz", b"another reference"),
            ("data/standard/MASK.nii.gz", b"the mask's bytes"),
        ]);
        let p = f.root.join("other.tar.bz2");
        std::fs::write(&p, &other).unwrap();
        let cat2 = Catalog {
            images: &f.images,
            package: Package {
                bytes: other.len() as u64,
                sha256: Box::leak(hex(&Sha256::digest(&other)).into_boxed_str()),
                ..f.package
            },
        };
        let e = fetch(&s, &cat2, p.to_str().unwrap(), &f.root.join("c3"), &mut log).unwrap_err();
        assert!(e.contains("is not FSL's file"), "{e}");
        assert!(
            !package_dir(&f.root.join("c3"), &cat2.package)
                .join("standard/REF.nii.gz")
                .exists()
        );
    }

    #[test]
    fn tar_numbers() {
        assert_eq!(octal(b"00000000017\0"), 15);
        assert_eq!(octal(b"  17 \0"), 15);
        assert_eq!(octal(&[0x80, 0, 0, 1, 0]), 256);
        assert_eq!(
            pax_path(b"30 path=data/standard/x.nii.gz\n").as_deref(),
            Some("data/standard/x.nii.gz")
        );
    }

    #[test]
    fn fsl_s_table() {
        assert_eq!(IMAGES.len(), 3);
        for i in IMAGES {
            assert_eq!(i.sha256.len(), 64);
        }
        assert!(PACKAGE.url.ends_with(&format!("{}.tar.bz2", PACKAGE.name)));
    }
}
