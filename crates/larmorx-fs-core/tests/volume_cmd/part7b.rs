// SPDX-License-Identifier: Apache-2.0
//! The probe's commands of fs.volume's part 7b (`specs/freesurfer-volume.md` §34, §35; the
//! commands and encodings, the image I among them:
//! `validation/investigations/freesurfer-volume-probe/README.md`).

use larmorx_fs_core::mri::Region;
use larmorx_fs_core::volume::image::{Image, ImageOut, image_alloc};
use larmorx_fs_core::{Process, Stop};

use super::args::{Line, bad, hexbytes, strtol};
use super::{TABLE, UNSET, UNSETD, dst, getdst, getreg, getw, paint, putres, putv};

/// I, an image: `null`, or `rows cols format frames`, `xsize ysize`, a fill, then the header's
/// `orows ocols rows cols num_frame` (-1 keeps the allocation's).
fn getimg(l: &mut Line) -> Option<Image> {
    if l.take("null") {
        return None;
    }
    let (rows, cols, fmt, nf) = (l.geti(), l.geti(), l.geti(), l.geti());
    let mut i = image_alloc(rows, cols, fmt, nf);
    i.xsize = l.getf();
    i.ysize = l.getf();
    let fill = l.tok().to_owned();
    if let Some(p) = i.pixels.as_mut() {
        if let Some(seed) = fill.strip_prefix('t') {
            let seed: u64 = seed
                .chars()
                .take_while(char::is_ascii_digit)
                .collect::<String>()
                .parse()
                .unwrap_or(0);
            for k in 0..p.len() / 4 {
                let x = TABLE[((7 * k as u64 + seed) % 34) as usize];
                p[4 * k..4 * k + 4].copy_from_slice(&x.to_le_bytes());
            }
        } else {
            let seed = strtol(&fill);
            if seed >= 0 {
                for (k, b) in p.iter_mut().enumerate() {
                    *b = ((seed as u64).wrapping_add(101 * k as u64) & 0xff) as u8;
                }
            }
        }
    }
    for x in [
        &mut i.orows,
        &mut i.ocols,
        &mut i.rows,
        &mut i.cols,
        &mut i.num_frame,
    ] {
        let v = l.geti();
        if v != -1 {
            *x = v;
        }
    }
    Some(i)
}

/// A printed image: `orows ocols rows cols format num_frame xsize ysize`, then its pixels.
fn putimg(l: &mut Line, i: &Image) {
    for v in [
        i.orows,
        i.ocols,
        i.rows,
        i.cols,
        i.pixel_format,
        i.num_frame,
    ] {
        l.puti(i64::from(v));
    }
    l.putf(i.xsize);
    l.putf(i.ysize);
    match &i.pixels {
        Some(p) => l.out(hexbytes(p)),
        None => l.out("null"),
    }
}

/// The result of a call that may return the given image, a new one or none.
fn putimg_res(l: &mut Line, r: ImageOut, given: Option<&Image>) {
    match r {
        ImageOut::Nothing => l.out("null"),
        ImageOut::Given => {
            l.out("given");
            putimg(l, given.expect("the given image"));
        }
        ImageOut::New(i) => {
            l.out("new");
            putimg(l, &i);
        }
    }
}

/// One of part 7b's commands, or `false` when `c` is none.
pub(super) fn call(p: &mut Process, l: &mut Line, c: &str) -> Result<bool, Stop> {
    match c {
        "toimgv" | "toimg" => {
            let mut a = getw(p, l)?;
            paint(p, l, &mut a)?;
            let mut img = getimg(l);
            let slice = l.geti();
            let r = if c == "toimgv" {
                let (view, frame) = (l.geti(), l.geti());
                p.mri_to_image_view(&a, img.as_mut(), slice, view, frame)?
            } else {
                p.mri_to_image(&a, img.as_mut(), slice)?
            };
            putimg_res(l, r, img.as_ref());
        }
        "img2mri" => {
            let Some(mut img) = getimg(l) else {
                bad("img2mri needs an image")
            };
            let m = p.image_to_mri(&mut img)?;
            l.putf(img.xsize);
            l.putf(img.ysize);
            putv(l, m.as_ref());
        }
        "smpar" => {
            let mut a = getw(p, l)?;
            paint(p, l, &mut a)?;
            let k = l.geti();
            let r = p.mri_smooth_parcellation(&a, k)?;
            putv(l, r.as_ref());
        }
        "erasebp" => {
            let mut a = getw(p, l)?;
            let b = l.geti();
            let r = p.mri_erase_border_planes(&mut a, b)?;
            l.puti(i64::from(r));
            putv(l, Some(&a));
        }
        "nbhd" => {
            let mut a = getw(p, l)?;
            paint(p, l, &mut a)?;
            let n = l.geti();
            for _ in 0..n {
                let (k, ws) = (l.geti(), l.geti());
                let q = (l.geti(), l.geti(), l.geti());
                let fr = l.geti();
                let th = l.getf();
                let v = match k {
                    0 => p.mri_max_in_nbhd(&a, ws, q, fr)?,
                    1 => p.mri_count_nonzero_in_nbhd(&a, ws, q)?,
                    2 => p.mri_count_thresh_in_nbhd(&a, ws, q, th)?,
                    _ => p.mri_are_nonzero_in_nbhd(&a, ws, q)?,
                };
                l.puti(i64::from(v));
            }
        }
        "nearnz" => {
            let mut a = getw(p, l)?;
            paint(p, l, &mut a)?;
            let n = l.geti();
            for _ in 0..n {
                let ws = l.geti();
                let q = (l.getd(), l.getd(), l.getd());
                let wp = l.geti();
                let mut place = [-7i32; 3];
                let v = p.mri_find_nearest_nonzero_location(
                    &a,
                    ws,
                    q,
                    (wp != 0).then_some(&mut place),
                )?;
                l.putf(v);
                for x in place {
                    l.puti(i64::from(x));
                }
            }
        }
        "lmeans" => {
            let k = l.geti();
            let a = getw(p, l)?;
            let mut lab = getw(p, l)?;
            paint(p, l, &mut lab)?;
            let label = l.geti();
            let q = (l.geti(), l.geti(), l.geti());
            let wh = l.geti();
            let want = l.geti() != 0;
            let mut sd = UNSETD;
            match k {
                0 => {
                    let v = p.mri_mean_in_label_multispectral(&a, &lab, label)?;
                    l.putm(Some(&v));
                }
                1 => {
                    let v = p.mri_mean_in_label(&a, &lab, label)?;
                    l.putd(v);
                }
                2 => {
                    let v =
                        p.mri_mean_and_std_in_label(&a, &lab, label, want.then_some(&mut sd))?;
                    l.putd(v);
                    l.putd(sd);
                }
                3 => {
                    let v = p.mri_mean_in_label_in_region(
                        &a,
                        &lab,
                        label,
                        q,
                        wh,
                        want.then_some(&mut sd),
                    )?;
                    l.putd(v);
                    l.putd(sd);
                }
                _ => {
                    let v = p.mri_max_in_label_in_region(&a, &lab, label, q, wh)?;
                    l.putd(v);
                }
            }
        }
        "mkpos" => {
            let mut a = getw(p, l)?;
            let mut d = getdst(p, l, false)?;
            let r = p.mri_make_positive(&mut a, dst(&mut d))?;
            putres(l, r, &a, None, &d);
        }
        "mlbv" => {
            let mut a = getw(p, l)?;
            paint(p, l, &mut a)?;
            let mut d = getdst(p, l, false)?;
            let (label, mark, six) = (l.geti(), l.geti(), l.geti());
            let r = p.mri_mark_label_border_voxels(&mut a, dst(&mut d), label, mark, six != 0)?;
            putres(l, r, &a, None, &d);
        }
        "lnbhd" => {
            let mut lab = getw(p, l)?;
            paint(p, l, &mut lab)?;
            let vals = if l.take("null") {
                None
            } else {
                Some(getw(p, l)?)
            };
            let q = (l.geti(), l.geti(), l.geti());
            let (wh, ml, wm) = (l.geti(), l.geti(), l.geti());
            let n = ml.max(0) as usize;
            let mut counts = vec![-7i32; n];
            let mut means = vec![UNSET; n];
            let r = p.mri_compute_label_nbhd(
                &lab,
                vals.as_ref(),
                q,
                &mut counts,
                (wm != 0).then_some(&mut means[..]),
                wh,
            )?;
            l.puti(i64::from(r));
            for c in &counts {
                l.puti(i64::from(*c));
            }
            if wm != 0 {
                for m in &means {
                    l.putf(*m);
                }
            }
        }
        "linbhd" => {
            let mut a = getw(p, l)?;
            paint(p, l, &mut a)?;
            let n = l.geti();
            for _ in 0..n {
                let k = l.geti();
                let q = (l.geti(), l.geti(), l.geti());
                let (wh, label, which) = (l.geti(), l.geti(), l.geti());
                let v = if k == 0 {
                    p.mri_labels_in_nbhd(&a, q, wh, label)?
                } else {
                    p.mri_labels_in_planar_nbhd(&a, q, wh, label, which)?
                };
                l.puti(i64::from(v));
            }
        }
        "lbbox" => {
            let mut a = getw(p, l)?;
            paint(p, l, &mut a)?;
            let label = l.geti();
            let mut b = Region {
                x: -7,
                y: -7,
                z: -7,
                dx: -7,
                dy: -7,
                dz: -7,
            };
            let r = p.mri_label_bounding_box(&a, label, &mut b)?;
            l.puti(i64::from(r));
            for v in [b.x, b.y, b.z, b.dx, b.dy, b.dz] {
                l.puti(i64::from(v));
            }
        }
        "fillbox" => {
            let mut a = getw(p, l)?;
            let Some(b) = getreg(l) else {
                bad("fillbox needs a box")
            };
            let v = l.getf();
            let r = p.mri_fill_box(&mut a, &b, v)?;
            l.puti(i64::from(r));
            putv(l, Some(&a));
        }
        "mskz" => {
            let mut a = getw(p, l)?;
            let mut m = getw(p, l)?;
            paint(p, l, &mut m)?;
            let mut d = getdst(p, l, true)?;
            let r = p.mri_mask_zero(&mut a, &mut m, dst(&mut d))?;
            putres(l, r, &a, Some(&m), &d);
        }
        "clipwm" => {
            let mut t = getw(p, l)?;
            let mut w = getw(p, l)?;
            paint(p, l, &mut w)?;
            let r = p.mri_clip_bright_wm(&mut t, &w)?;
            l.puti(i64::from(r));
            putv(l, Some(&t));
        }
        _ => return Ok(false),
    }
    Ok(true)
}
