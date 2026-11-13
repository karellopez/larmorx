// SPDX-License-Identifier: Apache-2.0
//! The histogram commands (README.md, "Commands", spec §5).

use larmorx_fs_core::maths::histo::{Histogram, Histogram2d};
use larmorx_fs_core::maths::{Output, Returned};
use larmorx_fs_core::{Process, Stop};

use crate::args::{Line, Out};

fn slurp(path: &str) -> Option<Vec<u8>> {
    std::fs::read(path).ok()
}

fn putfile(l: &mut Line, path: &str) {
    match slurp(path) {
        Some(b) => l.out(crate::hexbytes(&b)),
        None => l.out("nofile"),
    }
}

/// A one-input command's output resolved: the result and the output's state for printing.
fn one(
    p: &mut Process,
    l: &mut Line,
    mut h: Histogram,
    o: Out<Option<Histogram>>,
    f: impl FnOnce(&mut Process, &mut Histogram, Output<'_, Histogram>) -> Result<Returned<Histogram>, Stop>,
) -> Result<(), Stop> {
    match o {
        Out::New => {
            let r = f(p, &mut h, Output::New)?;
            l.puth_res(&r, None);
            l.puth(Some(&h));
        }
        Out::First | Out::Second => {
            let r = f(p, &mut h, Output::First)?;
            l.puth_res(&r, Some(&h));
        }
        Out::Obj(None) => {
            let r = f(p, &mut h, Output::New)?;
            l.puth_res(&r, None);
            l.puth(Some(&h));
        }
        Out::Obj(Some(mut d)) => {
            let r = f(p, &mut h, Output::Given(&mut d))?;
            l.puth_res(&r, Some(&d));
            l.puth(Some(&h));
        }
    }
    Ok(())
}

pub fn call(p: &mut Process, l: &mut Line, c: &str) -> Result<bool, Stop> {
    match c {
        "halloc" => {
            let n = l.geti();
            let h = p.histo_alloc(n)?;
            l.puth(Some(&h));
        }
        "hrealloc" => {
            let mut h = l.geth(p)?;
            let n = l.geti();
            let r = p.histo_realloc(h.as_mut(), n)?;
            l.puth_res(&r, h.as_ref());
        }
        "hfree" => {
            let mut h = l.geth(p)?;
            let v = p.histo_free(&mut h);
            l.puti(i64::from(v));
            l.out(if h.is_some() { "set" } else { "none" });
        }
        "hcopy" | "hclear" | "hclearcounts" | "hpdf" | "habs" | "hcdf" | "hrcdf" | "hcumsum" => {
            let h = l.geth(p)?.expect("a histogram");
            let o = l.geth_out(p)?;
            let f = match c {
                "hcopy" => Process::histo_copy,
                "hclear" => Process::histo_clear,
                "hclearcounts" => Process::histo_clear_counts,
                "hpdf" => Process::histo_make_pdf,
                "habs" => Process::histo_abs,
                "hcdf" => Process::histo_make_cdf,
                "hrcdf" => Process::histo_make_reverse_cdf,
                _ => Process::histo_cumsum,
            };
            one(p, l, h, o, f)?;
        }
        "hinit" => {
            let o = l.geth_out(p)?;
            let n = l.geti();
            let (lo, hi) = (l.getd(), l.getd());
            match o {
                Out::Obj(Some(mut d)) => {
                    let r = p.histo_init(Some(&mut d), n, lo, hi)?;
                    l.puth_res(&r, Some(&d));
                }
                _ => {
                    let r = p.histo_init(None, n, lo, hi)?;
                    l.puth_res(&r, None);
                }
            }
        }
        "hbins" => {
            let n = l.geti();
            let (lo, hi) = (l.getd(), l.getd());
            let h = p.histo_bins(n, lo, hi)?;
            l.puth(Some(&h));
        }
        "hadd" | "hsub" => {
            let mut a = l.geth(p)?.expect("a histogram");
            let mut b = l.geth(p)?;
            let o = l.geth_out(p)?;
            let f = if c == "hadd" { Process::histo_add } else { Process::histo_subtract };
            match o {
                Out::New | Out::Obj(None) => {
                    let r = f(p, &mut a, b.as_mut(), Output::New)?;
                    l.puth_res(&r, None);
                    l.puth(Some(&a));
                    // the probe prints each input that is not the output: none for no h2 and
                    // no output (both are C's NULL)
                    if b.is_some() {
                        l.puth(b.as_ref());
                    }
                }
                Out::First => {
                    let r = f(p, &mut a, b.as_mut(), Output::First)?;
                    l.puth_res(&r, Some(&a));
                    l.puth(b.as_ref());
                }
                Out::Second => {
                    let r = f(p, &mut a, b.as_mut(), Output::Second)?;
                    l.puth_res(&r, b.as_ref());
                    l.puth(Some(&a));
                }
                Out::Obj(Some(mut d)) => {
                    let r = f(p, &mut a, b.as_mut(), Output::Given(&mut d))?;
                    l.puth_res(&r, Some(&d));
                    l.puth(Some(&a));
                    l.puth(b.as_ref());
                }
            }
        }
        "hscale" => {
            let h = l.geth(p)?.expect("a histogram");
            let o = l.geth_out(p)?;
            let (s, off) = (l.getf(), l.getf());
            let r = |p: &mut Process, h: &mut Histogram, o: Output<'_, Histogram>| p.histo_linear_scale(h, o, s, off);
            one_no_input(p, l, h, o, r)?;
        }
        "hclearbins" => {
            let h = l.geth(p)?.expect("a histogram");
            let o = l.geth_out(p)?;
            let (lo, hi) = (l.geti(), l.geti());
            one_no_input(p, l, h, o, |p, h, o| p.histo_clear_bins(h, o, lo, hi))?;
        }
        "hsoap" => {
            let h = l.geth(p)?.expect("a histogram");
            let o = l.geth_out(p)?;
            let n = l.geti();
            one_no_input(p, l, h, o, |p, h, o| p.histo_soap_bubble_zeros(h, o, n))?;
        }
        "hsmooth" => {
            let h = l.geth(p)?.expect("a histogram");
            let o = l.geth_out(p)?;
            let s = l.getf();
            one_no_input(p, l, h, o, |p, h, o| p.histo_smooth(h, o, s))?;
        }
        "hsumnorm" | "hfill" | "hclearzero" => {
            let mut h = l.geth(p)?.expect("a histogram");
            let v = match c {
                "hsumnorm" => p.histo_sum_norm(&mut h),
                "hfill" => p.histo_fill_holes(&mut h),
                _ => p.histo_clear_zero_bin(&mut h),
            };
            l.puti(i64::from(v));
            l.puth(Some(&h));
        }
        "hlastpeak" | "hfirstpeak" => {
            let h = l.geth(p)?.expect("a histogram");
            let w = l.geti();
            let pct = l.getf();
            let v = if c == "hlastpeak" {
                p.histo_find_last_peak(&h, w, pct)
            } else {
                p.histo_find_first_peak(&h, w, pct)
            };
            l.puti(i64::from(v));
        }
        "hvalley" => {
            let h = l.geth(p)?.expect("a histogram");
            let (w, i0, i1) = (l.geti(), l.geti(), l.geti());
            let v = p.histo_find_valley(&h, w, i0, i1);
            l.puti(i64::from(v));
        }
        "hlastpeakin" => {
            let h = l.geth(p)?.expect("a histogram");
            let w = l.geti();
            let pct = l.getf();
            let (i0, i1) = (l.geti(), l.geti());
            let v = p.histo_find_last_peak_in_region(&h, w, pct, i0, i1);
            l.puti(i64::from(v));
        }
        "hhighest" => {
            let h = l.geth(p)?.expect("a histogram");
            let (b0, b1) = (l.geti(), l.geti());
            let v = p.histo_find_highest_peak_in_region(&h, b0, b1);
            l.puti(i64::from(v));
        }
        "hprevvalley" | "hnextvalley" => {
            let h = l.geth(p)?.expect("a histogram");
            let b0 = l.geti();
            let v = if c == "hprevvalley" {
                p.histo_find_previous_valley(&h, b0)
            } else {
                p.histo_find_next_valley(&h, b0)
            };
            l.puti(i64::from(v));
        }
        "hnextpeak" | "hprevpeak" => {
            let h = l.geth(p)?.expect("a histogram");
            let (b0, w) = (l.geti(), l.geti());
            let v = if c == "hnextpeak" {
                p.histo_find_next_peak(&h, b0, w)
            } else {
                p.histo_find_previous_peak(&h, b0, w)
            };
            l.puti(i64::from(v));
        }
        "hstartpeak" | "hendpeak" => {
            let h = l.geth(p)?.expect("a histogram");
            let b0 = l.geti();
            let pc = l.getf();
            let v = if c == "hstartpeak" {
                p.histo_find_start_of_peak(&h, b0, pc)
            } else {
                p.histo_find_end_of_peak(&h, b0, pc)
            };
            l.puti(i64::from(v));
        }
        "hcurrentpeak" => {
            let h = l.geth(p)?.expect("a histogram");
            let (b0, w) = (l.geti(), l.geti());
            let pc = l.getf();
            let v = p.histo_find_current_peak(&h, b0, w, pc);
            l.puti(i64::from(v));
        }
        "hfindbin" => {
            let h = l.geth(p)?.expect("a histogram");
            let v = l.getf();
            let b = p.histo_find_bin(&h, v);
            l.puti(i64::from(b));
            let c = p.histo_get_count(&h, v);
            l.putd(c);
            let b = p.histo_find_bin_with_count(&h, v);
            l.puti(i64::from(b));
        }
        "hvaltobin" => {
            let h = l.geth(p)?.expect("a histogram");
            let v = l.getd();
            let b = p.histo_val_to_bin(&h, v);
            l.puti(i64::from(b));
        }
        "hcount" => {
            let mut h = l.geth(p)?.expect("a histogram");
            let n = l.geti();
            let v: Vec<f64> = (0..n.max(0)).map(|_| l.getd()).collect();
            let r = p.histo_count(&mut h, &v);
            l.puti(i64::from(r));
            l.puth(Some(&h));
        }
        "haddsample" => {
            let mut h = l.geth(p)?.expect("a histogram");
            let n = l.geti();
            let v: Vec<f32> = (0..n.max(0)).map(|_| l.getf()).collect();
            let (lo, hi) = (l.getf(), l.getf());
            for x in v {
                let b = p.histo_add_sample(&mut h, x, lo, hi);
                l.puti(i64::from(b));
            }
            l.puth(Some(&h));
        }
        "htotal" => {
            let h = l.geth(p)?.expect("a histogram");
            let (b0, b1) = (l.geti(), l.geti());
            let t = p.histo_total_in_region(&h, b0, b1);
            l.putf(t);
            let t = p.histo_total(&h);
            l.putf(t);
        }
        "hthresh" => {
            let m = l.geth(p)?.expect("a histogram");
            let h = l.geth(p)?.expect("a histogram");
            let t = l.getf();
            let v = p.histo_thresh_sum(&m, &h, t);
            l.putf(v);
        }
        "hmedian" => {
            let h = l.geth(p)?.expect("a histogram");
            let v = p.histo_find_median(&h);
            l.putd(v);
        }
        "hentropy" => {
            let h = l.geth(p)?.expect("a histogram");
            let v = p.histo_get_entropy(&h);
            l.putd(v);
        }
        "hcorrelate" => {
            let a = l.geth(p)?.expect("a histogram");
            let b = l.geth(p)?.expect("a histogram");
            let v = p.histo_correlate(&a, &b)?;
            l.putd(v);
        }
        "hlinfit" => {
            let a = l.geth(p)?.expect("a histogram");
            let b = l.geth(p)?.expect("a histogram");
            let (a0, a1, b0, b1) = (l.getd(), l.getd(), l.getd(), l.getd());
            let (best, at) = p.histo_find_linear_fit(&a, &b, a0, a1, b0, b1);
            l.putd(best);
            let (pa, pb) = at.unwrap_or((-7.0, -7.0));
            l.putf(pa);
            l.putf(pb);
        }
        "hfwhm" => {
            let h = l.geth(p)?.expect("a histogram");
            let peak = l.geti();
            let v = p.histo_compute_fwhm(&h, peak);
            l.putf(v);
        }
        "hrobust" => {
            let mut h = l.geth(p)?.expect("a histogram");
            let q = l.getd();
            let r = p.histo_robust_gaussian_fit(&mut h, q)?;
            l.puti(i64::from(r.status));
            l.putd(r.offset.unwrap_or(-7.0));
            l.putd(r.sigma.unwrap_or(-7.0));
            l.puth(Some(&h));
            for f in ["ha.plt", "hc.plt", "h.plt", "hs.plt", "hz.plt", "hp.plt"] {
                putfile(l, f);
                let _ = std::fs::remove_file(f);
            }
        }
        "hplot" | "hwrite" => {
            let h = l.geth(p)?.expect("a histogram");
            let name = l.getx();
            let path = String::from_utf8_lossy(&name).into_owned();
            let _ = std::fs::remove_file(&path);
            let v = if c == "hplot" {
                p.histo_plot(&h, &name)
            } else {
                p.histo_write_txt(&h, &name)
            };
            l.puti(i64::from(v));
            putfile(l, &path);
            let _ = std::fs::remove_file(&path);
        }
        "h2d" => {
            let (n1, n2) = (l.geti(), l.geti());
            let mut h = Histogram2d::new(n1, n2);
            h.bin_size1 = l.getf();
            h.bin_size2 = l.getf();
            h.min1 = l.getf();
            h.min2 = l.getf();
            for i in 0..n1.max(0) as usize {
                h.bins1[i] = l.getf();
            }
            for i in 0..n2.max(0) as usize {
                h.bins2[i] = l.getf();
            }
            for i in 0..n1.max(0) as usize {
                for j in 0..n2.max(0) as usize {
                    h.counts[i][j] = l.getf();
                }
            }
            let (v1, v2) = (l.getf(), l.getf());
            let b1 = p.histo2d_find_bin1(&h, v1);
            l.puti(i64::from(b1));
            let b2 = p.histo2d_find_bin2(&h, v2);
            l.puti(i64::from(b2));
            let c = p.histo2d_get_count(&h, v1, v2);
            l.putd(c);
        }
        _ => return Ok(false),
    }
    Ok(true)
}

/// A one-input command that prints the result and the output only (the probe's `hscale`,
/// `hclearbins`, `hsoap`, `hsmooth`).
fn one_no_input(
    p: &mut Process,
    l: &mut Line,
    mut h: Histogram,
    o: Out<Option<Histogram>>,
    f: impl FnOnce(&mut Process, &mut Histogram, Output<'_, Histogram>) -> Result<Returned<Histogram>, Stop>,
) -> Result<(), Stop> {
    match o {
        Out::New | Out::Obj(None) => {
            let r = f(p, &mut h, Output::New)?;
            l.puth_res(&r, None);
        }
        Out::First | Out::Second => {
            let r = f(p, &mut h, Output::First)?;
            l.puth_res(&r, Some(&h));
        }
        Out::Obj(Some(mut d)) => {
            let r = f(p, &mut h, Output::Given(&mut d))?;
            l.puth_res(&r, Some(&d));
        }
    }
    Ok(())
}
