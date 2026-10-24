// SPDX-License-Identifier: LicenseRef-FSL AND Apache-2.0
// Copyright 2026 Karel Lopez Vilaret (the translation).
//
// fslpy 3.21.1's file-name semantics (imtest, remove_ext, imglob; fsl/utils/path.py,
// fsl/scripts/imtest.py, remove_ext.py, imglob.py) are Apache-2.0 (Copyright the University of
// Oxford, Paul McCarthy); the rest translates FSL's avwutils (fsl-avwutils 2209.3: fslroi.cc,
// fslmerge.cc, fslmaths.cc, fslstats.cc, fslval) under the FSL Licence.
//! The parts of FSL's other programs that the `bet` script runs, as the script uses them:
//! `fslval` (dimensions and voxel sizes), `fslroi` (one volume, one slice, a slab), `fslmerge -z`,
//! `fslmaths ... -dilF` and `-mas`, `fslstats -l <t> -C`, and fslpy's `imtest`, `remove_ext` and
//! `imglob`. Each reads and writes images through FSL's own layer ([`volume`]) with the program's
//! types: `fslroi` and `fslmerge` keep the input's type (`closestTemplatedType`), `fslmaths`
//! computes in `float` and writes `float`, `fslstats` reads `float`.

use std::path::{Path, PathBuf};

use crate::fsl::arma;
use crate::fsl::cstr;
use crate::fsl::filename;
use crate::fsl::nifti::{self, dt};
use crate::fsl::volume::{self, Elem, FslSettings, ImageError, Volume};

/// fslpy's image extensions, in its order.
pub const FSLPY_EXTS: [&str; 8] = [
    ".nii.gz", ".nii", ".img", ".hdr", ".img.gz", ".hdr.gz", ".mnc", ".mnc.gz",
];
const GROUPS: [[&str; 2]; 2] = [[".hdr", ".img"], [".hdr.gz", ".img.gz"]];

/// fslpy's `removeExt(path, exts)`: the first extension of the list the name ends with.
pub fn remove_ext(path: &str) -> String {
    for e in FSLPY_EXTS {
        if let Some(b) = path.strip_suffix(e) {
            return b.to_owned();
        }
    }
    path.to_owned()
}

fn get_ext(path: &str) -> &'static str {
    FSLPY_EXTS.into_iter().find(|e| path.ends_with(e)).unwrap_or("")
}

fn is_file(p: &str) -> bool {
    filename::is_regular_file(p)
}

fn exists(p: &str) -> bool {
    !p.is_empty() && Path::new(p).exists()
}

/// `os.path.realpath`: absolute, with the symbolic links of the existing part resolved.
fn realpath(p: &str) -> String {
    let path = Path::new(p);
    let abs = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(path)
    };
    // resolve the longest existing ancestor, keep the rest as written (normalized)
    let mut existing = abs.clone();
    let mut rest: Vec<std::ffi::OsString> = Vec::new();
    while !existing.exists() {
        match (existing.file_name().map(|s| s.to_owned()), existing.parent()) {
            (Some(name), Some(parent)) => {
                rest.push(name);
                existing = parent.to_path_buf();
            }
            _ => break,
        }
    }
    let mut out = std::fs::canonicalize(&existing).unwrap_or(existing);
    for name in rest.iter().rev() {
        if name == ".." {
            out.pop();
        } else if name != "." {
            out.push(name);
        }
    }
    out.to_string_lossy().into_owned()
}

/// fslpy's `imtest`: whether a name refers to one image (a single file, or a complete
/// `.hdr`/`.img` pair).
pub fn imtest(path: &str) -> bool {
    let prefix = realpath(&remove_ext(path));
    // addExt(prefix, exts, mustExist=True, fileGroups=GROUPS, unambiguous=True)
    let all: Vec<String> = if !get_ext(&prefix).is_empty() {
        vec![prefix.clone()]
    } else {
        FSLPY_EXTS.iter().map(|e| format!("{prefix}{e}")).collect()
    };
    let all: Vec<String> = all.into_iter().filter(|p| is_file(p)).collect();
    let path = match all.len() {
        0 => return false,
        1 => all[0].clone(),
        _ => {
            let mut suffixes: Vec<&str> = all.iter().map(|p| get_ext(p)).collect();
            suffixes.sort_unstable();
            let matches: Vec<usize> = GROUPS
                .iter()
                .enumerate()
                .filter(|(_, g)| {
                    let mut g = g.to_vec();
                    g.sort_unstable();
                    g == suffixes
                })
                .map(|(i, _)| i)
                .collect();
            if matches.len() != 1 {
                return false;
            }
            format!("{prefix}{}", GROUPS[matches[0]][0])
        }
    };
    let ext = get_ext(&path);
    let base = &path[..path.len() - ext.len()];
    let (mut partial, mut full) = (0, 0);
    for g in GROUPS {
        if !ext.is_empty() && !g.contains(&ext) {
            continue;
        }
        let e: Vec<bool> = g.iter().map(|s| exists(&format!("{base}{s}"))).collect();
        if e.iter().any(|&b| b) {
            partial += 1;
        }
        if e.iter().all(|&b| b) {
            full += 1;
        }
    }
    if partial == 0 {
        return true;
    }
    if full > 1 {
        return false;
    }
    if full == 1 && partial <= 1 {
        return true;
    }
    partial == 0
}

/// fslpy's `imglob` (prefix output) of the files `paths` (already expanded by the shell):
/// the image names without extension, sorted, each once.
pub fn imglob(paths: &[String]) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    for p in paths {
        let prefix = remove_ext(p);
        let cands: Vec<String> = if !get_ext(&prefix).is_empty() {
            vec![prefix.clone()]
        } else {
            FSLPY_EXTS.iter().map(|e| format!("{prefix}{e}")).collect()
        };
        for c in cands {
            if is_file(&c) {
                found.push(c);
            }
        }
    }
    let mut prefixes: Vec<String> = found.iter().map(|f| remove_ext(f)).collect();
    prefixes.sort();
    prefixes.dedup();
    prefixes
}

/// The files a shell's `<prefix>*` matches (bash's pathname expansion of an unquoted word with
/// one trailing star), sorted as bash sorts them in the C locale; the word itself when nothing
/// matches.
pub fn shell_glob_prefix(prefix: &str) -> Vec<String> {
    let path = Path::new(prefix);
    let (dir, stem) = match (path.parent(), path.file_name()) {
        (Some(d), Some(s)) if !d.as_os_str().is_empty() => (d.to_path_buf(), s.to_string_lossy().into_owned()),
        (_, Some(s)) => (PathBuf::from("."), s.to_string_lossy().into_owned()),
        _ => (PathBuf::from("."), String::new()),
    };
    let mut out: Vec<String> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with(&stem) && !(name.starts_with('.') && !stem.starts_with('.')) {
                let shown = if path.parent().is_some_and(|d| !d.as_os_str().is_empty()) {
                    format!("{}/{name}", dir.to_string_lossy())
                } else {
                    name
                };
                out.push(shown);
            }
        }
    }
    out.sort();
    if out.is_empty() {
        out.push(format!("{prefix}*"));
    }
    out
}

/// What an avwutils program printed and its exit status, when it failed.
#[derive(Debug, Clone)]
pub struct ToolFailure {
    pub err: String,
    pub code: u8,
}

fn failed(e: ImageError) -> ToolFailure {
    // The programs catch std::exception (print what(), return 1 from main via fallthrough)
    // or newimage's Exception (exit(EXIT_FAILURE)); either way exit status 1 after the
    // Image Exception lines.
    let mut err = e.printed;
    if e.exit.is_none() && !e.what.is_empty() && e.type_name == "std::runtime_error" {
        err.push_str(&e.what);
        err.push('\n');
    }
    ToolFailure {
        err,
        code: e.exit.unwrap_or(1),
    }
}

/// `dtype(filename)` then `closestTemplatedType`: the type avwutils programs read a file as.
pub fn template_type(name: &str) -> Result<i16, ToolFailure> {
    let mut printed = String::new();
    let path = filename::return_validimagefilename(name, &mut printed).map_err(|w| ToolFailure {
        err: format!("{printed}{w}\n"),
        code: 1,
    })?;
    let h = nifti::load_header(&path).map_err(|e| ToolFailure {
        err: format!("{}\n", e.0),
        code: 1,
    })?;
    let t = if h.scl_slope != 1.0 || h.scl_inter != 0.0 {
        if h.scl_slope == 0.0 || h.datatype == dt::DOUBLE {
            h.datatype
        } else {
            dt::FLOAT
        }
    } else {
        h.datatype
    };
    Ok(match t {
        dt::UNSIGNED_CHAR | dt::INT8 => dt::UNSIGNED_CHAR,
        dt::SIGNED_SHORT => dt::SIGNED_SHORT,
        dt::SIGNED_INT | dt::UINT16 => dt::SIGNED_INT,
        dt::FLOAT | dt::UINT32 | dt::INT64 | dt::UINT64 => dt::FLOAT,
        dt::DOUBLE | dt::FLOAT128 => dt::DOUBLE,
        dt::COMPLEX => {
            return Err(ToolFailure {
                err: "COMPLEX not supported as an independent type\n".into(),
                code: 255,
            });
        }
        other => {
            return Err(ToolFailure {
                err: format!("Datatype {other} is NOT supported - please check your image\n"),
                code: 255,
            });
        }
    })
}

/// `fslval <name> <dimN|pixdimN>`: the header field as fslhd prints it, with a trailing blank
/// (the script's `sed 's/-//g'` applied by the caller).
pub fn fslval(name: &str, key: &str) -> Result<String, ToolFailure> {
    let mut printed = String::new();
    let path = filename::return_validimagefilename(name, &mut printed).map_err(|w| ToolFailure {
        err: format!("{printed}{w}\n"),
        code: 1,
    })?;
    let h = nifti::load_header(&path).map_err(|e| ToolFailure {
        err: format!("{}\n", e.0),
        code: 1,
    })?;
    let v = if let Some(k) = key.strip_prefix("pixdim") {
        let k: usize = k.parse().unwrap_or(0);
        cstr::fmt_f(h.pixdim[k.min(7)], 6)
    } else if let Some(k) = key.strip_prefix("dim") {
        let k: usize = k.parse().unwrap_or(0);
        h.dim[k.min(7)].to_string()
    } else {
        String::new()
    };
    Ok(format!("{v} "))
}

/// `volume::ROI(minx, miny, minz, mint, maxx, maxy, maxz, maxt)`.
fn roi<T: Elem>(v: &Volume<T>, min: [i64; 4], max: [i64; 4]) -> Volume<T> {
    let n = [
        max[0] - min[0] + 1,
        max[1] - min[1] + 1,
        max[2] - min[2] + 1,
        max[3] - min[3] + 1,
    ];
    let mut r = Volume::<T>::new_4d(n[0], n[1], n[2], n[3]);
    let nvox = v.nvoxels();
    for t in min[3]..=max[3] {
        for z in min[2]..=max[2] {
            for y in min[1]..=max[1] {
                for x in min[0]..=max[0] {
                    let val = if v.in_bounds(x, y, z) && t >= 0 && t < v.tsize() {
                        v.data[t as usize * nvox + v.index(x, y, z)]
                    } else {
                        T::default()
                    };
                    let ri = (((t - min[3]) * n[2] + (z - min[2])) * n[1] + (y - min[1])) * n[0]
                        + (x - min[0]);
                    r.data[ri as usize] = val;
                }
            }
        }
    }
    r.copy_properties_from(v);
    let mut roi2vol = arma::IDENTITY;
    roi2vol[3][0] = min[0] as f64;
    roi2vol[3][1] = min[1] as f64;
    roi2vol[3][2] = min[2] as f64;
    if v.sform_code != nifti::XFORM_UNKNOWN {
        r.sform = arma::mul(&v.sform, &roi2vol);
    }
    if v.qform_code != nifti::XFORM_UNKNOWN {
        r.qform = arma::mul(&v.qform, &roi2vol);
    }
    r
}

fn save<T: Elem>(v: &Volume<T>, name: &str, fsl: &FslSettings) -> Result<(), ToolFailure> {
    match volume::save_volume(v, name, fsl, false) {
        Ok(_) => Ok(()),
        Err(e) => Err(failed(e)),
    }
}

/// `fslroi <in> <out> <args>` with 2 (t) or 6 (x, y, z) numbers, in the input's type.
pub fn fslroi(input: &str, output: &str, args: &[&str], fsl: &FslSettings) -> Result<(), ToolFailure> {
    match template_type(input)? {
        dt::UNSIGNED_CHAR => fslroi_t::<i8>(input, output, args, fsl),
        dt::SIGNED_SHORT => fslroi_t::<i16>(input, output, args, fsl),
        dt::SIGNED_INT => fslroi_t::<i32>(input, output, args, fsl),
        dt::DOUBLE => fslroi_t::<f64>(input, output, args, fsl),
        _ => fslroi_t::<f32>(input, output, args, fsl),
    }
}

fn fslroi_t<T: Elem>(input: &str, output: &str, args: &[&str], fsl: &FslSettings) -> Result<(), ToolFailure> {
    let v: Volume<T> = volume::read_volume4d(input, fsl).map_err(failed)?;
    let a: Vec<i64> = args.iter().map(|s| i64::from(cstr::atoi(s))).collect();
    let (mut min, mut max) = ([0i64; 4], [0i64; 4]);
    if a.len() == 2 {
        min = [0, 0, 0, a[0]];
        max = [v.xsize() - 1, v.ysize() - 1, v.zsize() - 1, a[1] + a[0] - 1];
    } else if a.len() == 6 {
        min = [a[0], a[2], a[4], 0];
        max = [a[1] + a[0] - 1, a[3] + a[2] - 1, a[5] + a[4] - 1, v.tsize() - 1];
    }
    for k in 0..3 {
        if max[k] == min[k] - 2 {
            max[k] = v.dims[k] - 1;
        }
    }
    if max[3] == min[3] - 2 {
        max[3] = v.tsize() - 1;
    }
    if max[0] < min[0] || max[1] < min[1] || max[2] < min[2] || max[3] < min[3] {
        return Err(ToolFailure {
            err: "Image Exception : #21 :: Invalid ROI dimensions\nInvalid ROI dimensions\n".into(),
            code: 1,
        });
    }
    let m = v.niftivox2newimagevox_mat();
    let lo = arma::mul_vec(&m, &[min[0] as f64, min[1] as f64, min[2] as f64, 1.0]);
    let hi = arma::mul_vec(&m, &[max[0] as f64, max[1] as f64, max[2] as f64, 1.0]);
    let mut mn = [0i64; 3];
    let mut mx = [0i64; 3];
    for k in 0..3 {
        mn[k] = i64::from(volume::misc_round(lo[k]));
        mx[k] = i64::from(volume::misc_round(hi[k]));
        if mn[k] > mx[k] {
            std::mem::swap(&mut mn[k], &mut mx[k]);
        }
    }
    let r = roi(&v, [mn[0], mn[1], mn[2], min[3]], [mx[0], mx[1], mx[2], max[3]]);
    save(&r, output, fsl)
}

/// `fslmerge -z <out> <inputs...>`, in the first input's type.
pub fn fslmerge_z(output: &str, inputs: &[String], fsl: &FslSettings) -> Result<(), ToolFailure> {
    let first = inputs.first().map_or("", String::as_str);
    match template_type(first)? {
        dt::UNSIGNED_CHAR => fslmerge_z_t::<i8>(output, inputs, fsl),
        dt::SIGNED_SHORT => fslmerge_z_t::<i16>(output, inputs, fsl),
        dt::SIGNED_INT => fslmerge_z_t::<i32>(output, inputs, fsl),
        dt::DOUBLE => fslmerge_z_t::<f64>(output, inputs, fsl),
        _ => fslmerge_z_t::<f32>(output, inputs, fsl),
    }
}

fn header_of(name: &str) -> Result<nifti::NiftiHeader, ToolFailure> {
    let mut printed = String::new();
    let path = filename::return_validimagefilename(name, &mut printed).map_err(|w| ToolFailure {
        err: format!("{printed}{w}\n"),
        code: 1,
    })?;
    nifti::load_header(&path).map_err(|e| ToolFailure {
        err: format!("{}\n", e.0),
        code: 1,
    })
}

fn fslmerge_z_t<T: Elem>(output: &str, inputs: &[String], fsl: &FslSettings) -> Result<(), ToolFailure> {
    let h0 = header_of(&inputs[0])?;
    let new_tr = h0.pixdim[4] as f32;
    let x = h0.dim[1].max(1);
    let y = h0.dim[2].max(1);
    let mut z = h0.dim[3].max(1);
    let t = h0.dim[4].max(1);
    for name in &inputs[1..] {
        z += header_of(name)?.dim[3].max(1);
    }
    let mut out = Volume::<T>::new_4d(x, y, z, t);
    let first: Volume<T> = volume::read_volume4d(&inputs[0], fsl).map_err(failed)?;
    out.copy_properties_from(&first);
    let mut zoff = 0;
    for (k, name) in inputs.iter().enumerate() {
        let v: Volume<T> = if k == 0 {
            first.clone()
        } else {
            volume::read_volume4d(name, fsl).map_err(failed)?
        };
        if v.xsize() != x || v.ysize() != y || v.tsize() != t {
            return Err(ToolFailure {
                err: format!(
                    "Error in size-match along non-concatenated dimension for input file: {name}\n"
                ),
                code: 1,
            });
        }
        let nvox_in = v.nvoxels();
        let nvox_out = out.nvoxels();
        for tt in 0..t {
            for kz in 0..v.zsize() {
                for j in 0..y {
                    for i in 0..x {
                        let src = tt as usize * nvox_in + v.index(i, j, kz);
                        let dst = tt as usize * nvox_out + out.index(i, j, kz + zoff);
                        out.data[dst] = v.data[src];
                    }
                }
            }
        }
        zoff += v.zsize();
    }
    out.tr = new_tr.abs();
    save(&out, output, fsl)
}

/// `morphfilter(v, box_kernel(3,3,3), "dilate")` on each volume: the maximum of the 3x3x3
/// neighbourhood inside the image.
fn dilate_f(v: &Volume<f32>) -> Volume<f32> {
    let mut r = v.clone();
    let nvox = v.nvoxels();
    let (nx, ny, nz) = (v.xsize(), v.ysize(), v.zsize());
    let nvol = v.data.len() / nvox.max(1);
    for t in 0..nvol {
        let base = t * nvox;
        for z in 0..nz {
            for y in 0..ny {
                for x in 0..nx {
                    let mut best = f64::NEG_INFINITY;
                    let mut any = false;
                    for mz in (z - 1).max(0)..=(z + 1).min(nz - 1) {
                        for my in (y - 1).max(0)..=(y + 1).min(ny - 1) {
                            for mx in (x - 1).max(0)..=(x + 1).min(nx - 1) {
                                let val = f64::from(v.data[base + v.index(mx, my, mz)]);
                                // NEWMAT's Maximum: the first value, then larger ones
                                if !any || val > best {
                                    best = val;
                                    any = true;
                                }
                            }
                        }
                    }
                    r.data[base + v.index(x, y, z)] = best as f32;
                }
            }
        }
    }
    r
}

/// `fslmaths <in> -dilF <out>`: float output, display range 0 0.
pub fn fslmaths_dilf(input: &str, output: &str, fsl: &FslSettings) -> Result<(), ToolFailure> {
    let v: Volume<f32> = volume::read_volume4d(input, fsl).map_err(failed)?;
    let mut r = dilate_f(&v);
    r.display_max = 0.0;
    r.display_min = 0.0;
    save(&r, output, fsl)
}

/// `fslmaths <in> -mas <mask> <out>`: the input times the mask binarised (above 0, below its
/// maximum plus one), in float; float output, display range 0 0.
pub fn fslmaths_mas(input: &str, mask: &str, output: &str, fsl: &FslSettings) -> Result<(), ToolFailure> {
    let mut v: Volume<f32> = volume::read_volume4d(input, fsl).map_err(failed)?;
    let mut m: Volume<f32> = volume::read_volume4d(mask, fsl).map_err(failed)?;
    let mut err = String::new();
    if v.tsize() == 1 && v.tsize() < m.tsize() {
        let one = v.data.clone();
        v.data = (0..m.tsize()).flat_map(|_| one.iter().copied()).collect();
        v.dims[3] = m.tsize();
    }
    let hi = m.max() + 1.0;
    for x in m.data.iter_mut() {
        *x = if *x > 0.0 && *x < hi { 1.0 } else { 0.0 };
    }
    if v.dimensionality() < m.dimensionality() || v.xsize() != m.xsize() || v.ysize() != m.ysize() || v.zsize() != m.zsize() {
        err.push_str("Image Exception : #3 :: Attempted to multiply images of different sizes\n");
        err.push_str("Attempted to multiply images of different sizes\n");
        return Err(ToolFailure { err, code: 1 });
    }
    let n = m.data.len();
    for (i, x) in v.data.iter_mut().enumerate() {
        *x *= m.data[i % n];
    }
    v.display_max = 0.0;
    v.display_min = 0.0;
    save(&v, output, fsl)
}

/// `fslstats <in> -l <lthr> -C`: the centre of gravity in NIfTI voxel coordinates, as it prints
/// it (fixed, 6 decimals, each followed by a blank).
pub fn fslstats_cog(input: &str, lthr: &str, fsl: &FslSettings) -> Result<String, ToolFailure> {
    let mut v: Volume<f32> = volume::read_volume4d(input, fsl).map_err(failed)?;
    let l = cstr::atof(lthr) as f32;
    let u = f32::MAX;
    for x in v.data.iter_mut() {
        let keep = *x > l && *x < u;
        *x *= if keep { 1.0 } else { 0.0 };
    }
    // calc_cog
    let vmin = v.min();
    let nvox = v.nvoxels();
    let mut nlim = (nvox as f64).sqrt() as i64;
    if nlim < 1000 {
        nlim = 1000;
    }
    let (mut c1, mut c2, mut c3) = (0.0f64, 0.0f64, 0.0f64);
    let (mut total, mut vx, mut vy, mut vz, mut tot) = (0.0f64, 0.0, 0.0, 0.0, 0.0);
    let mut n: i64 = 0;
    for z in 0..v.zsize() {
        for y in 0..v.ysize() {
            for x in 0..v.xsize() {
                let val = f64::from(v.value(x, y, z) - vmin);
                vx += val * x as f64;
                vy += val * y as f64;
                vz += val * z as f64;
                tot += val;
                n += 1;
                if n > nlim {
                    n = 0;
                    total += tot;
                    c1 += vx;
                    c2 += vy;
                    c3 += vz;
                    tot = 0.0;
                    vx = 0.0;
                    vy = 0.0;
                    vz = 0.0;
                }
            }
        }
    }
    total += tot;
    c1 += vx;
    c2 += vy;
    c3 += vz;
    let mut warn = String::new();
    if total.abs() < 1e-5 {
        warn.push_str("WARNING::in calculating COG, total = 0.0\n");
        total = 1.0;
    }
    c1 /= total;
    c2 /= total;
    c3 /= total;
    let m = v.niftivox2newimagevox_mat();
    let c = arma::solve_vec(&m, &[c1, c2, c3, 1.0]).unwrap_or([c1, c2, c3, 1.0]);
    let _ = warn;
    Ok(format!(
        "{} {} {} ",
        cstr::fmt_f(c[0], 6),
        cstr::fmt_f(c[1], 6),
        cstr::fmt_f(c[2], 6)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fslpy_names() {
        assert_eq!(remove_ext("a/b.nii.gz"), "a/b");
        assert_eq!(remove_ext("b.mnc.gz"), "b");
        assert_eq!(remove_ext("b.hdr"), "b");
        assert_eq!(remove_ext("b.txt"), "b.txt");
        let d = tempfile::tempdir().unwrap();
        let p = |n: &str| d.path().join(n).to_string_lossy().into_owned();
        std::fs::write(p("x.nii.gz"), b"").unwrap();
        assert!(imtest(&p("x")));
        assert!(imtest(&p("x.nii.gz")));
        std::fs::write(p("x.nii"), b"").unwrap();
        assert!(!imtest(&p("x")), "ambiguous");
        std::fs::write(p("y.hdr"), b"").unwrap();
        assert!(!imtest(&p("y")), "incomplete pair");
        std::fs::write(p("y.img"), b"").unwrap();
        assert!(imtest(&p("y")));
        assert_eq!(imglob(&[p("y.hdr"), p("y.img"), p("x.nii")]), vec![p("x"), p("y")]);
    }
}
