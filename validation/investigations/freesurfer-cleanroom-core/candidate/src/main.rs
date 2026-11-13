// SPDX-License-Identifier: Apache-2.0
//! The candidate of `specs/freesurfer-core.md` §20.1: larmorx's clean room of FreeSurfer's core
//! services (`larmorx-fs-core`) behind the commands and print forms of the black-box probe
//! `fs_core_probe` (`validation/investigations/freesurfer-core-probe/README.md`), so that
//! `compare.py compare` runs both under the same commands. `LARMORX_UPSTREAM_DEFECTS` chooses
//! the mode; in `fix` the notices of the defects that changed a result are printed on standard
//! error at the end.

use std::io::Write;
use std::process::ExitCode;

use larmorx_core::defects::UpstreamDefects;
use larmorx_fs_core::clib::{self, Stol};
use larmorx_fs_core::numbers::{self, ListStats};
use larmorx_fs_core::{
    Environment, Process, Stop, cmdargs, files, fsenv, romp, strings, swap, temp, version,
};

// ---- encodings (README.md, "Encodings") -------------------------------------------------------

fn is_null(t: &str) -> bool {
    t == "null"
}

fn unhex(t: &str) -> Vec<u8> {
    if !t.starts_with('x') {
        eprintln!("probe: a string must start with x: {t}");
        std::process::exit(2);
    }
    let b = t.as_bytes();
    let mut out = Vec::new();
    let mut i = 1;
    while i + 1 < b.len() {
        let s = std::str::from_utf8(&b[i..i + 2]).unwrap_or("0");
        out.push(u8::from_str_radix(s, 16).unwrap_or(0));
        i += 2;
    }
    out
}

/// A C string's bytes: up to the first NUL.
fn cs(b: &[u8]) -> &[u8] {
    strings::c_str(b)
}

fn hex(s: &[u8]) -> String {
    let mut o = String::from("x");
    for b in cs(s) {
        o.push_str(&format!("{b:02x}"));
    }
    o
}

fn hexn(s: &[u8]) -> String {
    let mut o = String::from("x");
    for b in s {
        o.push_str(&format!("{b:02x}"));
    }
    o
}

fn f32a(t: &str) -> f32 {
    f32::from_bits(u32::from_str_radix(t, 16).unwrap_or(0))
}

fn f64a(t: &str) -> f64 {
    f64::from_bits(u64::from_str_radix(t, 16).unwrap_or(0))
}

fn sf(f: f32) -> String {
    format!("{:08x}", f.to_bits())
}

fn sd(d: f64) -> String {
    format!("{:016x}", d.to_bits())
}

/// C's `atoi`/`atol` of a token.
fn atol(t: &str) -> i64 {
    clib::strtol(t.as_bytes())
}

fn atoi(t: &str) -> i32 {
    clib::atoi(t.as_bytes())
}

fn strtoul16(t: &str) -> u64 {
    let s = t.trim_start_matches("0x");
    let digits: String = s.chars().take_while(char::is_ascii_hexdigit).collect();
    u64::from_str_radix(&digits, 16).unwrap_or(0)
}

/// The probe's guard (`FS_CORE_PROBE_GUARD`), unused by the clean room (it never reads outside a
/// list) but parsed as the probe parses it.
fn split(line: &str) -> Vec<&str> {
    line.split(' ').filter(|t| !t.is_empty()).collect()
}

// ---- batch commands ----------------------------------------------------------------------------

fn batch_call(p: &mut Process, a: &[&str]) -> Result<String, Stop> {
    let c = a[0];
    let arg = |i: usize| a.get(i).copied().unwrap_or("");
    Ok(match c {
        "swapshort" => (swap::swap_i16(atol(arg(1)) as i16)).to_string(),
        "swapushort" => (swap::swap_u16(atol(arg(1)) as u16)).to_string(),
        "swapint" | "swaplong32" => (swap::swap_i32(atol(arg(1)) as i32)).to_string(),
        "swaplong64" => p.swap_i64(atol(arg(1))).to_string(),
        "swapfloat" => format!("{:08x}", swap::swap_f32_bits(f32a(arg(1)).to_bits())),
        "swapdouble" => format!("{:016x}", swap::swap_f64_bits(f64a(arg(1)).to_bits())),
        "byteswapbuf" => {
            let b = unhex(arg(3));
            let mut v = b.clone();
            v.resize(b.len() + 16, 0);
            let r = p.byte_swap_buf(&mut v, atol(arg(2)), atoi(arg(1)));
            format!("{r} {}", hexn(&v[..b.len()]))
        }
        "arch486" => swap::arch486().to_string(),
        "isflag" => i32::from(cmdargs::is_flag(cs(&unhex(arg(1))))).to_string(),
        "singledash" => i32::from(cmdargs::is_single_dash(cs(&unhex(arg(1))))).to_string(),
        "stringmatch" => {
            if is_null(arg(1)) && is_null(arg(2)) {
                return Ok("skip".into());
            }
            let s1 = (!is_null(arg(1))).then(|| unhex(arg(1)));
            let s2 = (!is_null(arg(2))).then(|| unhex(arg(2)));
            let m = cmdargs::string_match(s1.as_deref().map(cs), s2.as_deref().map(cs));
            i32::from(m).to_string()
        }
        "nthisarg" => {
            let bufs: Vec<Vec<u8>> = a[2..].iter().map(|t| unhex(t)).collect();
            let args: Vec<&[u8]> = bufs.iter().map(|b| cs(b)).collect();
            i32::from(cmdargs::nth_is_arg(&args, i64::from(atoi(arg(1))))).to_string()
        }
        "fgetl" => {
            let content = unhex(arg(2));
            let n = atoi(arg(1)).max(0) as usize;
            let mut r = std::io::Cursor::new(content);
            let mut out = String::new();
            while let Some(line) = strings::fgetl(&mut r, n) {
                out.push_str(&hex(&line));
                out.push(' ');
            }
            out + "end"
        }
        "strupper" | "strlower" | "str_toupper" => {
            let mut b = cs(&unhex(arg(1))).to_vec();
            if c == "strlower" {
                strings::str_lower(&mut b);
            } else {
                strings::str_upper(&mut b);
            }
            hex(&b)
        }
        "filenameonly" => hex(&strings::file_name_only(cs(&unhex(arg(1))))),
        "filename" => {
            let mut b = cs(&unhex(arg(1))).to_vec();
            let at = strings::file_name_in_place(&mut b);
            format!("{} {}", hex(&b[at..]), hex(&b))
        }
        "filenamepath" => hex(&strings::file_name_path(cs(&unhex(arg(1))))),
        "removeext" => hex(&strings::file_name_remove_extension(cs(&unhex(arg(1))))),
        "extension" => {
            let s = unhex(arg(1));
            hex(&p.file_name_extension(cs(&s)))
        }
        "fileexists" => format!("{} 1", i32::from(files::file_exists(cs(&unhex(arg(1)))))),
        "fromwildcard" => hex(&clib::glob::file_name_from_wildcard(cs(&unhex(arg(1))))),
        "glob" => {
            let (r, v) = clib::glob::glob(cs(&unhex(arg(1))));
            let mut o = r.to_string();
            for x in v {
                o.push(' ');
                o.push_str(&hex(&x));
            }
            o
        }
        "stricmp" => {
            let (x, y) = (unhex(arg(1)), unhex(arg(2)));
            p.stricmp(cs(&x), cs(&y)).to_string()
        }
        "itemsinstring" => strings::items_in_string(cs(&unhex(arg(1)))).to_string(),
        "countitems" => strings::count_items_in_string(cs(&unhex(arg(1)))).to_string(),
        "nthitem" => {
            let s = unhex(arg(2));
            match p.nth_item(cs(&s), atoi(arg(1))) {
                Some(v) => hex(&v),
                None => "null".into(),
            }
        }
        "deblank" => hex(&strings::deblank(cs(&unhex(arg(1))))),
        "deltaangle" => sf(numbers::delta_angle(f32a(arg(1)), f32a(arg(2)))),
        "devisinf" => numbers::dev_isinf(f32a(arg(1))).to_string(),
        "devisnan" => i32::from(numbers::dev_isnan(f32a(arg(1)))).to_string(),
        "devfinite" => i32::from(numbers::dev_finite(f32a(arg(1)))).to_string(),
        "nint" => p.nint(f64a(arg(1))).to_string(),
        "nint2" => p.nint2(f64a(arg(1))).to_string(),
        "sum2stddev" => sd(numbers::sum2stddev(
            f64a(arg(1)),
            f64a(arg(2)),
            atoi(arg(3)),
        )),
        "cmpints" => numbers::compare_ints(atoi(arg(1)), atoi(arg(2))).to_string(),
        "cmpfloats" => numbers::compare_floats(f32a(arg(1)), f32a(arg(2))).to_string(),
        "nunique" | "unique" | "mostfrequent" => {
            let mut v: Vec<i32> = a[1..].iter().map(|t| atoi(t)).collect();
            let mut o = match c {
                "nunique" => p.nunique_int_list(&mut v).to_string(),
                "unique" => {
                    let u = p.unique_int_list(&mut v);
                    let mut o = u.len().to_string();
                    for x in u {
                        o.push_str(&format!(" {x}"));
                    }
                    o
                }
                _ => {
                    let (m, n) = p.most_frequent_int_list(&mut v);
                    format!("{m} {n}")
                }
            };
            o.push_str(" |");
            for x in &v {
                o.push_str(&format!(" {x}"));
            }
            o
        }
        "kth" | "qselect" | "median" => {
            let first = if c == "median" { 1 } else { 2 };
            let mut v: Vec<f32> = a[first..].iter().map(|t| f32a(t)).collect();
            let r = match c {
                "kth" => numbers::kth_smallest(&mut v, atoi(arg(1)) as usize),
                "qselect" => numbers::quick_select(&mut v, atoi(arg(1)) as usize),
                _ => p.median(&mut v),
            };
            let mut o = sf(r) + " |";
            for x in &v {
                o.push(' ');
                o.push_str(&sf(*x));
            }
            o
        }
        "atan2f" => sf(p.fast_approx_atan2f(f32a(arg(1)), f32a(arg(2)))),
        "dliststats" => {
            let n = atoi(arg(1));
            let none = a.len() > 2 && is_null(a[2]);
            let mut v: Vec<f64> = if none {
                Vec::new()
            } else {
                a[2..].iter().map(|t| f64a(t)).collect()
            };
            v.push(0.0);
            let r = p.dlist_stats(if none { None } else { Some(&v) }, n);
            p.flush();
            match r {
                ListStats::None => "null".into(),
                ListStats::Stats(st) => st.iter().map(|x| sd(*x)).collect::<Vec<_>>().join(" "),
            }
        }
        "findfirst" => {
            let v: Vec<i32> = a[2..].iter().map(|t| atoi(t)).collect();
            strings::find_first_match(&v, atoi(arg(1))).to_string()
        }
        "argv2cmdline" => {
            let bufs: Vec<Vec<u8>> = a[1..].iter().map(|t| unhex(t)).collect();
            let args: Vec<&[u8]> = bufs.iter().map(|b| cs(b)).collect();
            hex(&strings::argv_to_cmdline(&args))
        }
        "checkfinite" => {
            p.errno = 0;
            let place = unhex(arg(1));
            p.check_finite(cs(&place), f64a(arg(2))).to_string()
        }
        "rompsum" => {
            let vals: Vec<f64> = a[2..].iter().map(|t| f64a(t)).collect();
            let n = romp::thread_count(p.env.get("OMP_NUM_THREADS"));
            sd(romp::reproducible_sum(f64a(arg(1)), &vals, n))
        }
        "rompparts" => {
            let parts = romp::pieces(i64::from(atoi(arg(1))), i64::from(atoi(arg(2))));
            let mut o = parts.len().to_string();
            for (lo, hi) in parts {
                o.push_str(&format!(" {lo}:{hi}"));
            }
            o
        }
        "rompparallel" => romp::Level::ALL
            .iter()
            .map(|l| i32::from(l.parallel()).to_string())
            .collect::<Vec<_>>()
            .join(" "),
        "strerror" => hex(clib::strerror(atoi(arg(1))).as_bytes()),
        "atoi" => clib::atoi(cs(&unhex(arg(1)))).to_string(),
        "atof" => sd(clib::atof(cs(&unhex(arg(1))))),
        "scanlx" => {
            let (r, v) = clib::scan_0x_lx(cs(&unhex(arg(1))));
            format!("{r} {}", v.unwrap_or(0))
        }
        "scand" => {
            let (r, v) = clib::scan_d(cs(&unhex(arg(1))));
            match v {
                Some(v) if r == 1 => format!("{r} {v}"),
                _ => format!("{r} -"),
            }
        }
        "gmtime" | "localtime" => {
            let t = atol(arg(1));
            let tm = if c == "gmtime" {
                p.gmtime(t)
            } else {
                p.localtime(t)
            };
            match tm {
                Some(m) => format!(
                    "{} {} {} {} {} {} {} {} {} {} {}",
                    m.year,
                    m.mon,
                    m.mday,
                    m.hour,
                    m.min,
                    m.sec,
                    m.wday,
                    m.yday,
                    m.isdst,
                    m.gmtoff,
                    hex(&m.zone)
                ),
                None => "null".into(),
            }
        }
        "strftimec" => match p.local_c_string(atol(arg(1))) {
            Some(s) => hex(&s),
            None => "null".into(),
        },
        "stol" => match clib::stol(cs(&unhex(arg(1)))) {
            Stol::Value(v) => v.to_string(),
            Stol::InvalidArgument => "invalid_argument".into(),
            Stol::OutOfRange => "out_of_range".into(),
        },
        _ => {
            eprintln!("probe: unknown batch command {c}");
            return Err(Stop::Exit(2));
        }
    })
}

fn batch(p: &mut Process, path: &str) -> Result<(), Stop> {
    let data = std::fs::read(path).map_err(|_| {
        eprintln!("probe: cannot open {path}");
        Stop::Exit(2)
    })?;
    for line in data.split(|&c| c == b'\n') {
        let line = String::from_utf8_lossy(line);
        let t = split(&line);
        if t.is_empty() {
            continue;
        }
        let r = batch_call(p, &t)?;
        p.put_out(r.as_bytes());
        p.put_out(b"\n");
        p.flush();
    }
    Ok(())
}

// ---- run commands ------------------------------------------------------------------------------

fn print_globals(p: &mut Process) {
    let s = format!(
        "Gdiag={:x} Gdiag_no={} IMAGE_SIZE={} Gx={} Gy={} Gz={} Gvx={} Gvy={} Gvz={} Gprofile={}\n",
        p.gdiag,
        p.gdiag_no,
        p.image_size,
        p.gvoxel[0],
        p.gvoxel[1],
        p.gvoxel[2],
        p.gvertex[0],
        p.gvertex[1],
        p.gvertex[2],
        p.gprofile
    );
    p.put_out(s.as_bytes());
}

fn run(p: &mut Process, c: &str, a: &[&str]) -> Result<(), Stop> {
    let arg = |i: usize| a.get(i).copied().unwrap_or("");
    match c {
        "batch" => return batch(p, if a.is_empty() { "-" } else { a[0] }),
        "single" => {
            let r = batch_call(p, a)?;
            p.put_out(format!("{r}\n").as_bytes());
        }
        "init" => {
            let r = p.diag_init();
            p.put_out(format!("return={r:x}\n").as_bytes());
            print_globals(p);
        }
        "errorprintf" | "errorexit" => {
            let (e, code) = (atoi(arg(0)), atoi(arg(1)));
            let msg = unhex(arg(2));
            let (mut thr, mut twice) = (false, false);
            let mut i = 3;
            while i < a.len() {
                if a[i] == "log" && i + 1 < a.len() {
                    i += 1;
                    let l = unhex(a[i]);
                    p.set_error_log(cs(&l));
                } else if a[i] == "throw" {
                    thr = true;
                } else if a[i] == "twice" {
                    twice = true;
                }
                i += 1;
            }
            p.set_throw_exceptions(thr);
            p.errno = e;
            if c == "errorprintf" {
                p.error_printf(code, cs(&msg));
                if twice {
                    p.error_printf(code, cs(&msg));
                }
                let s = format!("Gerror={} errno={}\n", p.gerror, p.errno);
                p.put_out(s.as_bytes());
                return Ok(());
            }
            match p.error_exit(code, cs(&msg)) {
                Ok(()) => {
                    let s = format!("returned Gerror={}\n", p.gerror);
                    p.put_out(s.as_bytes());
                }
                Err(Stop::Thrown(t)) => {
                    let s = format!("caught {t} Gerror={}\n", p.gerror);
                    p.put_out(s.as_bytes());
                }
                Err(e) => return Err(e),
            }
        }
        "fatal" => {
            p.set_throw_exceptions(a.len() > 2 && a[2] == "throw");
            let msg = unhex(arg(1));
            match p.fatal(atoi(arg(0)), &msg) {
                Ok(()) => p.put_out(b"returned\n"),
                Err(Stop::Thrown(t)) => p.put_out(format!("caught {t}\n").as_bytes()),
                Err(e) => return Err(e),
            }
        }
        "error" => {
            if a.len() > 2 && a[1] == "log" {
                let l = unhex(a[2]);
                p.set_error_log(cs(&l));
            }
            p.error(&unhex(arg(0)));
        }
        "warning" => p.warning(&unhex(arg(0))),
        "debug" => {
            p.gdiag = strtoul16(arg(0));
            p.debug(&unhex(arg(1)));
        }
        "argnerr" => {
            let f = unhex(arg(0));
            return Err(p.cmd_arg_n_err(cs(&f), atoi(arg(1))));
        }
        "usageexit" => return Err(p.cmd_usage_exit()),
        "assert" => {
            let (f, t) = (unhex(arg(0)), unhex(arg(2)));
            return Err(p.assert_failed(cs(&f), atoi(arg(1)), cs(&t)));
        }
        "diagprintf" => {
            p.gdiag = strtoul16(arg(0));
            let m = unhex(arg(2));
            let r = p.diag_printf(strtoul16(arg(1)), cs(&m));
            p.put_out(format!("\nreturn={r}\n").as_bytes());
        }
        "heartbeat" => {
            p.gdiag = strtoul16(arg(0));
            for t in &a[1..] {
                p.diag_heartbeat(f32a(t));
            }
        }
        "pctdone" => {
            p.gdiag = strtoul16(arg(0));
            let n = atoi(arg(1));
            for t in a.iter().skip(2) {
                p.diag_show_pct_done(f32a(t), n);
            }
        }
        "random" => {
            for s in a {
                if let Some(v) = s.strip_prefix("seed=") {
                    p.set_random_seed(atol(v));
                } else if let Some(v) = s.strip_prefix("draw=") {
                    let (lo, hi) = v.split_once(',').unwrap_or((v, ""));
                    let r = p.random_number(f64a(lo), f64a(hi))?;
                    p.put_out(format!("{}\n", sd(r)).as_bytes());
                } else if let Some(v) = s.strip_prefix("perm=") {
                    let n = atoi(v);
                    let perm = p.compute_permutation(n)?;
                    let mut o = String::from("perm");
                    for x in perm {
                        o.push_str(&format!(" {x}"));
                    }
                    p.put_out(format!("{o}\n").as_bytes());
                } else if *s == "state" {
                    let o = format!("seed={} calls={}\n", p.random_seed(), p.random_calls());
                    p.put_out(o.as_bytes());
                }
                p.flush();
            }
        }
        "logic" => {
            let file = unhex(arg(0));
            let (line, n) = (atoi(arg(1)), atoi(arg(2)));
            let mut o = String::new();
            for _ in 0..n {
                let r = p.spend_time_checking_for_logic_problem(cs(&file), line)?;
                o.push(if r { '1' } else { '0' });
            }
            p.put_out(format!("{o}\n").as_bytes());
        }
        "cope" => {
            let fixer = (!is_null(arg(0))).then(|| unhex(arg(0)));
            let (msg, file, func) = (unhex(arg(1)), unhex(arg(2)), unhex(arg(4)));
            let (line, n) = (atoi(arg(3)), atoi(arg(5)));
            for _ in 0..n {
                let (r, rep) = p.cope_with_logic_problem(
                    fixer.as_deref().map(cs),
                    cs(&msg),
                    cs(&file),
                    line,
                    cs(&func),
                );
                p.put_out(
                    format!("response={} reported={}\n", r as i32, i32::from(rep)).as_bytes(),
                );
                p.flush();
            }
        }
        "xdbg" => {
            for s in a {
                if let Some(v) = s.strip_prefix("push=") {
                    let (t, n) = v.split_once(',').unwrap_or((v, "x"));
                    let (t, n) = (unhex(t), unhex(n));
                    p.xdbg_push(cs(&t), cs(&n));
                } else if *s == "pop" {
                    p.xdbg_pop();
                } else if let Some(v) = s.strip_prefix("enter=") {
                    let t = unhex(v);
                    p.debug_enter_function(cs(&t));
                } else if *s == "exit" {
                    p.debug_exit_function();
                } else if let Some(v) = s.strip_prefix("desc=") {
                    let t = unhex(v);
                    p.xdbg_set_stack_desc(cs(&t));
                } else if let Some(v) = s.strip_prefix("note=") {
                    let t = unhex(v);
                    p.xdbg_set_current_note(cs(&t));
                } else if *s == "cur" {
                    let o = format!(
                        "{} {} {}\n",
                        hex(&p.xdbg_current_function()),
                        hex(p.xdbg_current_note()),
                        hex(p.xdbg_stack_desc())
                    );
                    p.put_out(o.as_bytes());
                }
                p.flush();
            }
        }
        "version" | "allinfo" => {
            let prog = unhex(arg(0));
            let argv: Vec<Vec<u8>> = a[1..].iter().map(|t| cs(&unhex(t)).to_vec()).collect();
            if c == "allinfo" {
                let args: Vec<&[u8]> = argv.iter().map(Vec::as_slice).collect();
                let mut l = p.all_info(&args, cs(&prog));
                l.push(b'\n');
                p.put_out(&l);
                return Ok(());
            }
            let (n, left) = p.handle_version_option(&argv, cs(&prog));
            let mut o = format!("n={n}");
            for x in &left {
                o.push(' ');
                o.push_str(&hex(x));
            }
            p.put_out(format!("{o}\n").as_bytes());
        }
        "versionstrings" => {
            let o = format!("{}\n{}\n", version::VERSION, version::BUILD_STAMP);
            p.put_out(o.as_bytes());
        }
        "curstamp" => {
            let mut s = p.cur_time_stamp();
            s.push(b'\n');
            p.put_out(&s);
        }
        "filestamp" => {
            let f = unhex(arg(0));
            let mut s = p
                .file_time_stamp(cs(&f))
                .unwrap_or_else(|| b"NULL".to_vec());
            s.push(b'\n');
            p.put_out(&s);
        }
        "user" => {
            let mut s = p.user_name();
            s.push(b'\n');
            p.put_out(&s);
        }
        "datetime" => {
            let mut s = p.current_date_time(atoi(arg(0)) != 0);
            s.push(b'\n');
            p.put_out(&s);
        }
        "fsenv" => match p.fsenv_getenv(&mut fsenv::OpenOnly) {
            None => p.put_out(b"\nNULL\n"),
            Some(e) => {
                let h = |v: Option<&Vec<u8>>| v.map_or_else(|| "null".to_string(), |x| hex(x));
                let mut o = format!(
                    "\nFREESURFER_HOME={}\nSUBJECTS_DIR={}\nuser={}\ncwd={}\nsysname={}\nmachine={}\nhostname={}\ndate=",
                    hex(&e.freesurfer_home),
                    h(e.subjects_dir.as_ref()),
                    h(e.user.as_ref()),
                    hex(&e.cwd),
                    hex(&e.sysname),
                    hex(&e.machine),
                    hex(&e.hostname)
                )
                .into_bytes();
                o.extend_from_slice(cs(&e.date));
                o.extend_from_slice(
                    format!("\ndesired_bvec_space={}\nctab=read\n", e.desired_bvec_space)
                        .as_bytes(),
                );
                p.put_out(&o);
            }
        },
        "getsubjectsdir" => {
            let v = p.fsenv_get_subjects_dir();
            let o = v.map_or_else(|| "null".to_string(), |x| hex(&x));
            p.put_out(format!("{o}\n").as_bytes());
        }
        "setsubjectsdir" => {
            let d = unhex(arg(0));
            p.fsenv_set_subjects_dir(cs(&d));
            let o = p
                .env
                .get("SUBJECTS_DIR")
                .map_or_else(|| "null".to_string(), hex);
            p.put_out(format!("{o}\n").as_bytes());
        }
        "tempdir" | "tempfile" | "mktempdir" => {
            let r = match c {
                "tempdir" => p.base_temp_dir()?,
                "tempfile" => {
                    let s = if a.is_empty() {
                        Vec::new()
                    } else {
                        unhex(arg(0))
                    };
                    p.make_temp_file(cs(&s))?
                }
                _ => p.make_temp_dir()?,
            };
            let mut o = r;
            o.push(b'\n');
            p.put_out(&o);
        }
        "dirwritable" => {
            let d = unhex(arg(0));
            let d = cs(&d);
            let o = format!(
                "{} {}\n",
                i32::from(temp::directory_exists(d)),
                i32::from(temp::directory_is_writable(d))
            );
            p.put_out(o.as_bytes());
        }
        "memused" => {
            let m = p.memory_used();
            p.put_out(format!("\nmemused={m}\n").as_bytes());
        }
        "memusage" => {
            let mut u = [-1i32; 5];
            p.mem_usage(&mut u);
            let peak = p.vm_peak();
            let o = format!("{} {} {} {} {} peak={peak}\n", u[0], u[1], u[2], u[3], u[4]);
            p.put_out(o.as_bytes());
        }
        "rusage" => {
            let pre = unhex(arg(0));
            let r = p.print_rusage(cs(&pre));
            p.put_out(format!("return={r}\n").as_bytes());
        }
        "writerusage" => {
            let (pre, f) = (unhex(arg(0)), unhex(arg(1)));
            let r = p.write_rusage(cs(&pre), cs(&f));
            p.put_out(format!("return={r}\n").as_bytes());
        }
        "maxthreads" => {
            let n = romp::thread_count(p.env.get("OMP_NUM_THREADS"));
            p.put_out(format!("{n}\n").as_bytes());
        }
        "threadnum" => {
            let n = (atoi(arg(0)).max(1) as usize).min(romp::MAX_THREADS);
            p.put_out(format!("max thread number {}\n", n - 1).as_bytes());
        }
        "hips" => {
            let r = p.hips_unsupported();
            p.hips_unsupported();
            p.put_out(format!("{}\n", sd(f64::from(r))).as_bytes());
        }
        _ => {
            eprintln!("probe: unknown command {c}");
            return Err(Stop::Exit(2));
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: fs_core_probe batch <file> | <command> ... (README.md)");
        return ExitCode::from(2);
    }
    let mode = UpstreamDefects::from_env().unwrap_or_default();
    let out = Box::new(std::io::BufWriter::new(std::io::stdout()));
    let err = Box::new(std::io::stderr());
    let mut p = Process::new(Environment::from_process(), mode, out, err);
    let rest: Vec<&str> = args[2..].iter().map(String::as_str).collect();
    let status = match run(&mut p, &args[1], &rest) {
        Ok(()) => 0,
        Err(s) => s.status(),
    };
    p.flush();
    if mode.fix() {
        let notices = p.take_notices();
        notices.print(&mut std::io::stderr());
    }
    let _ = std::io::stdout().flush();
    ExitCode::from(status as u8)
}
