// SPDX-License-Identifier: Apache-2.0
//! The probe's commands of part 11b (`specs/freesurfer-volume.md` §62 to §65; the commands and
//! the encodings, VF among them: `validation/investigations/freesurfer-volume-probe/README.md`).

use larmorx_fs_core::files::path_of;
use larmorx_fs_core::maths::Returned;
use larmorx_fs_core::maths::matrix::Matrix;
use larmorx_fs_core::mri::{Mri, VolGeom};
use larmorx_fs_core::volume::ltaconv::{ComposeInto, Composed};
use larmorx_fs_core::volume::ltaread::TransformInputs;
use larmorx_fs_core::volume::ltawrite::TransformOutputs;
use larmorx_fs_core::volume::vlist::VoxelList;
use larmorx_fs_core::{Process, Stop};

use super::UNSETD;
use super::args::{Line, hexbytes};

/// A folder made with mode 0700 (the probe's `mkdir`; elsewhere the default mode).
fn make_dir(name: &[u8]) -> std::io::Result<()> {
    let path = path_of(name);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new().mode(0o700).create(path)
    }
    #[cfg(not(unix))]
    {
        std::fs::create_dir(path)
    }
}

use super::bspline::getlta;
use super::part11a::getvl;
use super::{D, getw, paint, putlta, putres};

/// A voxel list (printed) by the commands of §65: `max_vox nvox type`, `self` (the list's volume
/// is the call's, or the list has none and the call none), then the first `len` voxels (at most
/// what was allocated).
fn putvl(l: &mut Line, vl: Option<&VoxelList>, len: usize) {
    let Some(vl) = vl else {
        l.out("null");
        return;
    };
    l.puti(i64::from(vl.max_vox));
    l.puti(i64::from(vl.nvox));
    l.puti(i64::from(vl.kind));
    l.out("self");
    for i in 0..len.min(vl.alloc_len()) {
        l.puti(i64::from(vl.xi[i]));
        l.puti(i64::from(vl.yi[i]));
        l.puti(i64::from(vl.zi[i]));
        l.puti(i64::from(vl.fi[i]));
        l.putf(vl.xd[i]);
        l.putf(vl.yd[i]);
        l.putf(vl.zd[i]);
        l.putf(vl.vsrc[i]);
        l.putf(vl.vdst[i]);
    }
}

/// The matrix a call returned, `given` first when it is the given one.
fn putm_res(l: &mut Line, r: &Returned<Matrix>, given: Option<&Matrix>) {
    match r {
        Returned::Nothing => l.putm(None),
        Returned::New(m) => l.putm(Some(m)),
        Returned::Output => {
            l.out("given");
            l.putm(given);
        }
    }
}

/// VF, a voxel list with its fields: VL, or `f <alloc> <nvox> <type> <mask> <k>` and `k` voxels'
/// fields (`VLSTalloc(alloc)`, VL's patterns, the voxels as given, the slope arrays of the mask
/// with entry i `i + 0.75`, then `nvox` and `type` set); with the allocation.
fn getvf(p: &mut Process, l: &mut Line) -> Result<(Option<VoxelList>, i32), Stop> {
    if !l.take("f") {
        return getvl(p, l);
    }
    let (n, nv, kind, mask, k) = (l.geti(), l.geti(), l.geti(), l.geti(), l.geti());
    let (Some(mut vl), _) = pattern_list(p, n)? else {
        unreachable!("a list is made")
    };
    for i in 0..k.max(0) as usize {
        let (xi, yi, zi, fi) = (l.geti(), l.geti(), l.geti(), l.geti());
        let (xd, yd, zd, vs, vd) = (l.getf(), l.getf(), l.getf(), l.getf(), l.getf());
        if i >= n.max(0) as usize {
            continue;
        }
        vl.xi[i] = xi;
        vl.yi[i] = yi;
        vl.zi[i] = zi;
        vl.fi[i] = fi;
        vl.xd[i] = xd;
        vl.yd[i] = yd;
        vl.zd[i] = zd;
        vl.vsrc[i] = vs;
        vl.vdst[i] = vd;
    }
    let len = if n > 0 { n as usize } else { 1 };
    for (b, slot) in [&mut vl.mx, &mut vl.my, &mut vl.mz, &mut vl.t]
        .into_iter()
        .enumerate()
    {
        if mask & (1 << b) == 0 {
            continue;
        }
        let mut a = vec![0.0f32; len];
        for (i, x) in a.iter_mut().enumerate().take(n.max(0) as usize) {
            *x = i as f32 + 0.75;
        }
        *slot = Some(a);
    }
    vl.nvox = nv;
    vl.kind = kind;
    Ok((Some(vl), n))
}

/// `VLSTalloc(n)` with VL's patterns (`nvox` left as allocated).
fn pattern_list(p: &mut Process, n: i32) -> Result<(Option<VoxelList>, i32), Stop> {
    let mut vl = p.vlst_alloc(n)?;
    for i in 0..n.max(0) as usize {
        let f = i as f32;
        vl.xi[i] = 1000 + i as i32;
        vl.yi[i] = 2000 + i as i32;
        vl.zi[i] = 3000 + i as i32;
        vl.fi[i] = 4000 + i as i32;
        vl.xd[i] = f + 0.5;
        vl.yd[i] = f + 0.25;
        vl.zd[i] = f + 0.125;
        vl.vsrc[i] = -1.0 - f;
        vl.vdst[i] = -2.0 - f;
    }
    Ok((Some(vl), n))
}

/// The slope arrays of a list (whether each is present), to tell given arrays from new ones.
type Slopes = [Option<Vec<f32>>; 4];

fn slopes_of(vl: Option<&VoxelList>) -> Slopes {
    match vl {
        Some(v) => [v.mx.clone(), v.my.clone(), v.mz.clone(), v.t.clone()],
        None => [None, None, None, None],
    }
}

/// A voxel list (printed) and its slope arrays: for each of `mx`, `my`, `mz`, `t` `null`, or
/// `given` and its first `given_len` entries when it is the array the list was given with (still
/// the same array: `kept` says), else `new` and its first `new_len`.
fn putvf(
    l: &mut Line,
    vl: &VoxelList,
    len: usize,
    kept: [bool; 4],
    given_len: usize,
    new_len: usize,
) {
    putvl(l, Some(vl), len);
    for (b, a) in [&vl.mx, &vl.my, &vl.mz, &vl.t].into_iter().enumerate() {
        let Some(a) = a else {
            l.out("null");
            continue;
        };
        l.out(if kept[b] { "given" } else { "new" });
        let n = if kept[b] { given_len } else { new_len };
        for &x in a.iter().take(n) {
            l.putf(x);
        }
    }
}

/// The golden cases' writer target: the file in memory; an empty name, or one in the folder
/// `no/`, does not open (`ENOENT`).
#[derive(Default)]
struct MemOut {
    file: Option<Vec<u8>>,
}

impl TransformOutputs for MemOut {
    fn create(&mut self, name: &[u8]) -> Result<(), i32> {
        if name.is_empty() || name.starts_with(b"no/") {
            return Err(2);
        }
        self.file = Some(Vec::new());
        Ok(())
    }

    fn write(&mut self, bytes: &[u8]) {
        if let Some(f) = self.file.as_mut() {
            f.extend_from_slice(bytes);
        }
    }
}

/// The subjects' folders and files of a `devxfm` line, for the file system or in memory (the
/// golden cases' `devxfmmem`).
#[derive(Default)]
struct Subjects {
    dirs: Vec<Vec<u8>>,
    files: Vec<(Vec<u8>, Vec<u8>)>,
}

impl TransformInputs for Subjects {
    fn read_file(&mut self, name: &[u8]) -> Result<Vec<u8>, i32> {
        match self.files.iter().find(|(n, _)| n.as_slice() == name) {
            Some((_, b)) => Ok(b.clone()),
            None => Err(2),
        }
    }

    fn volume_geometry(&mut self, _p: &mut Process, name: &[u8]) -> Result<Option<VolGeom>, i32> {
        if self.files.iter().any(|(n, _)| n.as_slice() == name) {
            Ok(None)
        } else {
            Err(2)
        }
    }

    fn is_dir(&mut self, name: &[u8]) -> bool {
        self.dirs.iter().any(|d| d.as_slice() == name)
    }

    fn opens(&mut self, name: &[u8]) -> bool {
        self.files.iter().any(|(n, _)| n.as_slice() == name)
    }
}

/// M, a volume or none: `null`, or W P.
fn getwn(p: &mut Process, l: &mut Line) -> Result<Option<Mri>, Stop> {
    if l.take("null") {
        return Ok(None);
    }
    let mut m = getw(p, l)?;
    paint(p, l, &mut m)?;
    Ok(Some(m))
}

/// A file or folder removed, a folder with everything in it.
pub(super) fn rmtree(path: &std::path::Path) {
    match std::fs::symlink_metadata(path) {
        Ok(m) if m.is_dir() => {
            let _ = std::fs::remove_dir_all(path);
        }
        Ok(_) => {
            let _ = std::fs::remove_file(path);
        }
        Err(_) => {}
    }
}

/// One call of part 11b's commands; `false` when the command is not one of them.
pub(super) fn call(p: &mut Process, l: &mut Line, c: &str) -> Result<bool, Stop> {
    match c {
        // -- §62
        "ltawr" => {
            let (k, n) = (l.geti(), l.geti());
            let dirs: Vec<Vec<u8>> = (0..n.max(0)).map(|_| l.getx()).collect();
            let lta = getlta(p, l)?;
            let name = l.getx();
            for d in &dirs {
                let _ = make_dir(d);
            }
            let path = path_of(&name);
            let _ = std::fs::remove_file(&path);
            p.flush();
            p.errno = 0;
            let r = if k != 0 {
                p.lta_write(&lta, &name)
            } else {
                p.lta_write_ex(&lta, &name)
            };
            p.flush();
            let r = match r {
                Ok(r) => r,
                Err(s) => {
                    let _ = std::fs::remove_file(&path);
                    for d in dirs.iter().rev() {
                        rmtree(&path_of(d));
                    }
                    return Err(s);
                }
            };
            l.puti(i64::from(r));
            match std::fs::metadata(&path) {
                Ok(m) if m.is_file() => {
                    let b = std::fs::read(&path).unwrap_or_default();
                    l.out(hexbytes(&b));
                }
                _ => l.out("nofile"),
            }
            let _ = std::fs::remove_file(&path);
            for d in dirs.iter().rev() {
                rmtree(&path_of(d));
            }
        }
        "ltawrmem" => {
            // the golden cases' `ltawr`: `k LTA name`, the file in memory (a name that is empty or
            // in the folder `no/` does not open: `ENOENT`)
            let _k = l.geti();
            let lta = getlta(p, l)?;
            let name = l.getx();
            let mut out = MemOut::default();
            p.flush();
            p.errno = 0;
            let r = p.lta_write_to(&lta, &name, &mut out)?;
            p.flush();
            l.puti(i64::from(r));
            match out.file {
                Some(b) => l.out(hexbytes(&b)),
                None => l.out("nofile"),
            }
        }
        // -- §63
        "ltapt" => {
            let lta = getlta(p, l)?;
            let (x, y, z) = (l.getf(), l.getf(), l.getf());
            let mut m = l.getm(p)?;
            let e0 = l.geti();
            p.flush();
            p.errno = e0;
            let r = p.lta_transform_at_point(&lta, x, y, z, m.as_mut())?;
            let e = p.errno;
            p.flush();
            putm_res(l, &r, m.as_ref());
            l.puti(i64::from(e));
        }
        "ltaxf" | "ltaixf" => {
            let mut a = getw(p, l)?;
            paint(p, l, &mut a)?;
            let mut d = if l.take("new") {
                D::New
            } else {
                D::W(Box::new(getw(p, l)?))
            };
            let mut lta = getlta(p, l)?;
            let interp = l.geti();
            p.flush();
            let dm = match &mut d {
                D::W(m) => Some(m.as_mut()),
                _ => None,
            };
            let r = if c == "ltaixf" {
                p.lta_inverse_transform_interp(&mut a, dm, &mut lta, interp)?
            } else if interp < 0 {
                p.lta_transform(&mut a, dm, &mut lta)?
            } else {
                p.lta_transform_interp(&mut a, dm, &mut lta, interp)?
            };
            p.flush();
            putres(l, r, &a, None, &d);
            putlta(l, Some(&lta));
        }
        // -- §67
        "ltav2r" | "ltar2v" => {
            let mut lta = getlta(p, l)?;
            let s = getwn(p, l)?;
            let d = getwn(p, l)?;
            p.flush();
            let r = if c == "ltav2r" {
                p.lta_voxel_to_ras_xform(&mut lta, s.as_ref(), d.as_ref())?
            } else {
                p.lta_ras_to_voxel_xform(&mut lta, s.as_ref(), d.as_ref())?
            };
            p.flush();
            l.puti(i64::from(r));
            putlta(l, Some(&lta));
        }
        "ltacmp" => {
            let mut src = getlta(p, l)?;
            let left = l.getm(p)?;
            let right = l.getm(p)?;
            let k = l.geti();
            let mut other = if k == 1 { Some(getlta(p, l)?) } else { None };
            p.flush();
            let into = match (k, other.as_mut()) {
                (1, Some(o)) => ComposeInto::Given(o),
                (2, _) => ComposeInto::Itself,
                _ => ComposeInto::New,
            };
            let r = p.lta_compose(&mut src, left.as_ref(), right.as_ref(), into)?;
            p.flush();
            match r {
                Composed::New(n) => {
                    l.out("new");
                    putlta(l, Some(&n));
                    if let Some(o) = &other {
                        putlta(l, Some(o));
                    }
                    putlta(l, Some(&src));
                }
                Composed::Given if k == 2 => {
                    l.out("self");
                    putlta(l, Some(&src));
                }
                Composed::Given => {
                    l.out("given");
                    putlta(l, other.as_ref());
                    putlta(l, Some(&src));
                }
            }
        }
        "ltacor" => {
            let mut lta = getlta(p, l)?;
            p.flush();
            let r = p.lta_voxel_transform_to_coronal_ras_transform(&mut lta)?;
            p.flush();
            l.puti(i64::from(r));
            putlta(l, Some(&lta));
        }
        "fixmni" => {
            let (x, y, z) = (l.getf(), l.getf(), l.getf());
            let (r, v) = p.fix_mni_tal(x, y, z)?;
            l.puti(i64::from(r));
            for a in v {
                l.putf(a);
            }
        }
        "ltamod" => {
            let mut lta = getlta(p, l)?;
            let s = getwn(p, l)?;
            let d = getwn(p, l)?;
            let t = l.getd();
            p.flush();
            let r = p.lta_modify_src_dst_geom(&mut lta, s.as_ref(), d.as_ref(), t)?;
            p.flush();
            l.puti(i64::from(r));
            putlta(l, Some(&lta));
        }
        "devxfm" | "devxfmmem" => {
            // the subjects' folder `subjects`: mk 0 none, 1 subjects/<subject>, 2 also its
            // mri/transforms, the original volume and the transform file (README.md)
            let mk = l.geti();
            let subj = l.getx();
            let sd = if l.take("null") { None } else { Some(l.getx()) };
            let name = if l.take("null") { None } else { Some(l.getx()) };
            let ok = l.geti();
            let ob = if ok == 1 || ok == 2 { l.getx() } else { Vec::new() };
            let xb = if l.take("nofile") { None } else { Some(l.getx()) };
            let mut x = l.getm(p)?;
            let mut base = b"subjects/".to_vec();
            base.extend_from_slice(&subj);
            let sub = |s: &[u8]| {
                let mut b = base.clone();
                b.extend_from_slice(s);
                b
            };
            let mut m = Subjects::default();
            if mk >= 1 {
                m.dirs.push(b"subjects".to_vec());
                m.dirs.push(base.clone());
            }
            if mk >= 2 {
                m.dirs.push(sub(b"/mri"));
                m.dirs.push(sub(b"/mri/transforms"));
                match ok {
                    1 => m.files.push((sub(b"/mri/orig.mgz"), ob.clone())),
                    2 => m.files.push((sub(b"/mri/orig.mgh"), ob.clone())),
                    3 => m.dirs.push(sub(b"/mri/orig")),
                    _ => {}
                }
                if let Some(xb) = &xb {
                    let mut t = b"/mri/transforms/".to_vec();
                    t.extend_from_slice(name.as_deref().unwrap_or(b"talairach.xfm"));
                    m.files.push((sub(&t), xb.clone()));
                }
            }
            let disk = c == "devxfm";
            let root = path_of(b"subjects");
            if disk {
                rmtree(&root);
                for d in &m.dirs {
                    let _ = make_dir(d);
                }
                for (f, b) in &m.files {
                    let _ = std::fs::write(path_of(f), b);
                }
            }
            p.flush();
            p.errno = 0;
            let r = if disk {
                p.devolve_xfm_with_subjects_dir(&subj, x.as_mut(), name.as_deref(), sd.as_deref())
            } else {
                p.devolve_xfm_from(&subj, x.as_mut(), name.as_deref(), sd.as_deref(), &mut m)
            };
            p.flush();
            if disk {
                rmtree(&root);
            }
            let r = r?;
            putm_res(l, &r, x.as_ref());
        }
        "ltadvg" => {
            let mut g = VolGeom::default();
            l.getg(&mut g, true);
            p.flush();
            let r = p.lta_dump_vol_geom(&g);
            p.flush();
            l.puti(i64::from(r));
        }
        // -- §65
        "vlcre" => {
            let mut a = getw(p, l)?;
            paint(p, l, &mut a)?;
            let (lo, hi) = (l.getf(), l.getf());
            let (vl, alloc) = getvf(p, l)?;
            let (skip, border) = (l.geti(), l.geti());
            let given = vl.is_some();
            let before = slopes_of(vl.as_ref());
            p.flush();
            let res = p.vlst_create(&a, lo, hi, vl, skip, border)?;
            p.flush();
            l.out(if given { "given" } else { "new" });
            let mut len = res.nvox.max(0) as usize;
            if given && alloc.max(0) as usize > len {
                len = alloc as usize;
            }
            let kept = [0, 1, 2, 3].map(|b| before[b].is_some());
            putvf(l, &res, len, kept, alloc.max(0) as usize, 0);
        }
        "vlsort" => {
            let (src, a_s) = getvf(p, l)?;
            let (dst, a_d) = if l.take("null") {
                (None, 0)
            } else {
                getvf(p, l)?
            };
            let src = src.expect("a source list");
            p.flush();
            let (res, given) = p.vlst_sort(&src, dst)?;
            p.flush();
            l.out(if given { "given" } else { "new" });
            let n = if given { a_d.max(0) as usize } else { res.nvox.max(0) as usize };
            putvl(l, Some(&res), n);
            putvl(l, Some(&src), a_s.max(0) as usize);
        }
        "vlfree" => {
            let mut vl = if l.take("null") { None } else { getvf(p, l)?.0 };
            let r = p.vlst_free(&mut vl);
            l.puti(i64::from(r));
            l.out(if vl.is_some() { "set" } else { "null" });
        }
        "vlstat" => {
            let (vl, _) = getvf(p, l)?;
            let mut vl = vl.expect("a list");
            let m = if l.take("null") {
                None
            } else {
                let mut m = getw(p, l)?;
                paint(p, l, &mut m)?;
                Some(m)
            };
            p.flush();
            p.vlst_compute_stats(&mut vl, m.as_ref())?;
            p.flush();
            l.putd(vl.mean);
            l.putd(vl.std);
        }
        "vlcopy" => {
            let k = l.geti();
            let (src, a_s) = getvf(p, l)?;
            let (dst, a_d) = if l.take("null") {
                (None, 0)
            } else {
                getvf(p, l)?
            };
            let (start, num) = (l.geti(), l.geti());
            let src = src.expect("a source list");
            let before = slopes_of(dst.as_ref());
            p.flush();
            let (res, given) = p.vlst_copy_any(&src, dst, start, num, k != 0)?;
            p.flush();
            l.out(if given { "given" } else { "new" });
            let len = if given {
                a_d.max(0) as usize
            } else if k != 0 {
                start.wrapping_add(num).max(0) as usize
            } else {
                num.max(0) as usize
            };
            // a slope array the copy replaced is new; one it left is the given one
            let replaced = slopes_of(Some(&src)).map(|s| s.is_some());
            let kept = [0, 1, 2, 3].map(|b| before[b].is_some() && !replaced[b]);
            putvf(l, &res, len, kept, a_d.max(0) as usize, num.max(0) as usize);
            putvl(l, Some(&src), a_s.max(0) as usize);
        }
        "vladd" => {
            let k = l.geti();
            let (vl, a) = getvf(p, l)?;
            let mut vl = vl.expect("a list");
            let (x, y, z) = (l.geti(), l.geti(), l.geti());
            let (xd, yd, zd, vs, vd) = (l.getf(), l.getf(), l.getf(), l.getf(), l.getf());
            p.flush();
            let r = match k {
                0 => p.vlst_add(&mut vl, x, y, z, xd, yd, zd)?,
                1 => p.vlst_add_unique(&mut vl, x, y, z, xd, yd, zd)?,
                2 => p.vlst_add_with_value(&mut vl, x, y, z, xd, yd, zd, vs, vd)?,
                _ => p.vlst_in_list(&vl, x, y, z)?,
            };
            p.flush();
            l.puti(i64::from(r));
            putvl(l, Some(&vl), a.max(0) as usize);
        }
        // -- §64
        "angrm" => {
            let a = [l.getd(), l.getd(), l.getd()];
            let m = p.mri_angles2_rot_mat(a)?;
            l.putm(Some(&m));
        }
        "affp2m" => {
            let mut a = [0.0f64; 12];
            for v in a.iter_mut() {
                *v = l.getd();
            }
            let mut m = l.getm(p)?;
            p.flush();
            let r = p.tranform_affine_params2_matrix(&a, m.as_mut())?;
            p.flush();
            putm_res(l, &r, m.as_ref());
        }
        "affm2p" => {
            let m = l.getm(p)?;
            let mut a = [UNSETD; 12];
            p.flush();
            let r = p.tranform_extract_affine_params(m.as_ref(), &mut a)?;
            p.flush();
            l.out(if r { "given" } else { "null" });
            for v in a {
                l.putd(v);
            }
        }
        "ltarot" => {
            let mut lta = getlta(p, l)?;
            p.flush();
            let r = p.lta_mat2_rot_mat(&mut lta)?;
            p.flush();
            l.puti(i64::from(r));
            putlta(l, Some(&lta));
        }
        "rmsmj" => {
            let a = l.getm(p)?;
            let b = l.getm(p)?;
            let r = l.getd();
            let (Some(a), Some(b)) = (a, b) else {
                super::args::bad("rmsmj needs two matrices")
            };
            p.flush();
            let v = p.rms_reg_diff_mj(&a, &b, r)?;
            p.flush();
            l.putd(v);
        }
        _ => return Ok(false),
    }
    Ok(true)
}
