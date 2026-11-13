// SPDX-License-Identifier: Apache-2.0
//! The probe's encodings (`validation/investigations/freesurfer-mri-probe/README.md`,
//! "Encodings"): a line's arguments and its printed result.

use larmorx_fs_core::maths::matrix::{MATRIX_REAL, Matrix};
use larmorx_fs_core::mri::geom::fixed_name;
use larmorx_fs_core::mri::xform::{MincTransform, TransformKind};
use larmorx_fs_core::mri::{ColorTable, ColorTableEntry, Mri, VolGeom};
use larmorx_fs_core::{Process, Stop};

/// One line's arguments and result.
pub struct Line {
    toks: Vec<String>,
    pos: usize,
    res: Vec<String>,
}

/// C's `strtol(s, NULL, 10)`.
pub fn strtol(s: &str) -> i64 {
    let b = s.as_bytes();
    let mut i = 0;
    let neg = match b.first() {
        Some(b'-') => {
            i = 1;
            true
        }
        Some(b'+') => {
            i = 1;
            false
        }
        _ => false,
    };
    let mut v: i64 = 0;
    while i < b.len() && b[i].is_ascii_digit() {
        v = v.saturating_mul(10).saturating_add(i64::from(b[i] - b'0'));
        i += 1;
    }
    if neg { -v } else { v }
}

/// C's `strtoull(s, NULL, 16)`.
pub fn strtoul16(s: &str) -> u64 {
    let d: String = s.chars().take_while(char::is_ascii_hexdigit).collect();
    u64::from_str_radix(&d, 16).unwrap_or(u64::MAX)
}

/// The probe's string encoding: `x` and the bytes' hex digits.
pub fn hexbytes(b: &[u8]) -> String {
    let mut o = String::with_capacity(1 + 2 * b.len());
    o.push('x');
    for x in b {
        o.push_str(&format!("{x:02x}"));
    }
    o
}

/// The probe's exit on a malformed line.
pub fn bad(what: &str) -> ! {
    eprintln!("probe: {what}");
    std::process::exit(3);
}

impl Line {
    pub fn new(toks: Vec<String>) -> Line {
        Line {
            toks,
            pos: 0,
            res: Vec::new(),
        }
    }

    pub fn tok(&mut self) -> &str {
        if self.pos >= self.toks.len() {
            bad("missing argument");
        }
        self.pos += 1;
        &self.toks[self.pos - 1]
    }

    pub fn peek(&self, s: &str) -> bool {
        self.toks.get(self.pos).is_some_and(|t| t == s)
    }

    pub fn take(&mut self, s: &str) -> bool {
        if self.peek(s) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    pub fn geti(&mut self) -> i32 {
        strtol(self.tok()) as i32
    }

    pub fn getf(&mut self) -> f32 {
        f32::from_bits(strtoul16(self.tok()) as u32)
    }

    pub fn getd(&mut self) -> f64 {
        f64::from_bits(strtoul16(self.tok()))
    }

    pub fn getx(&mut self) -> Vec<u8> {
        let t = self.tok().to_owned();
        if !t.starts_with('x') {
            bad("a string must start with x");
        }
        let b = t.as_bytes();
        let mut s = Vec::new();
        let mut i = 1;
        while i + 1 < b.len() {
            s.push(u8::from_str_radix(&t[i..i + 2], 16).unwrap_or(0));
            i += 2;
        }
        s
    }

    // ---- results

    pub fn out(&mut self, s: impl Into<String>) {
        self.res.push(s.into());
    }

    pub fn puti(&mut self, v: i64) {
        self.out(v.to_string());
    }

    pub fn putf(&mut self, f: f32) {
        self.out(format!("{:08x}", f.to_bits()));
    }

    pub fn putd(&mut self, d: f64) {
        self.out(format!("{:016x}", d.to_bits()));
    }

    /// A C string (up to its first NUL), or `null`.
    pub fn puts(&mut self, s: Option<&[u8]>) {
        match s {
            None => self.out("null"),
            Some(s) => {
                let end = s.iter().position(|&b| b == 0).unwrap_or(s.len());
                self.out(hexbytes(&s[..end]));
            }
        }
    }

    pub fn result(&self) -> String {
        self.res.join(" ")
    }

    // ---- matrices: null, or rows cols and the floats row by row

    pub fn getm(&mut self, p: &mut Process) -> Result<Option<Matrix>, Stop> {
        if self.take("null") {
            return Ok(None);
        }
        let rows = self.geti();
        let cols = self.geti();
        let Some(mut m) = p.matrix_alloc(rows, cols, MATRIX_REAL)? else {
            bad("a matrix argument the allocator refused");
        };
        for r in 1..=rows {
            for c in 1..=cols {
                let v = self.getf();
                m.set(r, c, v);
            }
        }
        Ok(Some(m))
    }

    pub fn putm(&mut self, m: Option<&Matrix>) {
        let Some(m) = m else {
            self.out("null");
            return;
        };
        self.puti(i64::from(m.rows));
        self.puti(i64::from(m.cols));
        for r in 1..=m.rows {
            for c in 1..=m.cols {
                self.putf(m.at(r, c));
            }
        }
    }

    // ---- geometries

    pub fn getg(&mut self, g: &mut VolGeom, with_valid: bool) {
        if with_valid {
            g.valid = self.geti();
        }
        g.width = self.geti();
        g.height = self.geti();
        g.depth = self.geti();
        g.xsize = self.getf();
        g.ysize = self.getf();
        g.zsize = self.getf();
        g.ras_good_flag = self.geti();
        for v in [&mut g.x, &mut g.y, &mut g.z, &mut g.c, &mut g.s] {
            for k in 0..3 {
                v[k] = self.getf();
            }
        }
        let n = self.getx();
        g.fname = fixed_name(&n, 1024);
    }

    pub fn putg(&mut self, g: &VolGeom, with_valid: bool) {
        if with_valid {
            self.puti(i64::from(g.valid));
        }
        self.puti(i64::from(g.width));
        self.puti(i64::from(g.height));
        self.puti(i64::from(g.depth));
        self.putf(g.xsize);
        self.putf(g.ysize);
        self.putf(g.zsize);
        self.puti(i64::from(g.ras_good_flag));
        for v in [g.x, g.y, g.z, g.c, g.s] {
            for x in v {
                self.putf(x);
            }
        }
        self.puts(Some(&g.fname));
        match &g.i_to_r {
            Some(a) => {
                self.out("cache");
                for x in a {
                    self.putf(*x);
                }
            }
            None => self.out("null"),
        }
        self.putm(g.r_to_i.as_ref());
        self.putm(g.register_mat.as_ref());
    }

    /// G, a cache flag and a registration matrix (Gf).
    pub fn getg_full(&mut self, p: &mut Process, g: &mut VolGeom) -> Result<(), Stop> {
        self.getg(g, true);
        if self.geti() != 0 {
            p.reinit_cache(g)?;
        }
        g.register_mat = self.getm(p)?;
        Ok(())
    }

    // ---- volumes

    pub fn getv(&mut self, p: &mut Process) -> Result<Mri, Stop> {
        let (w, h, d, kind, nf, hdr) = (
            self.geti(),
            self.geti(),
            self.geti(),
            self.geti(),
            self.geti(),
            self.geti(),
        );
        p.mri_alloc(w, h, d, kind, nf, hdr == 0)
    }

    /// V then G.
    pub fn getvg(&mut self, p: &mut Process) -> Result<Mri, Stop> {
        let mut m = self.getv(p)?;
        self.getg(&mut m.geom, true);
        Ok(m)
    }

    pub fn putvox(&mut self, m: &Mri) {
        match &m.voxels {
            None => self.out("novox"),
            Some(v) => self.out(hexbytes(v)),
        }
    }

    pub fn puthdr(&mut self, m: &Mri) {
        let g = &m.geom;
        for v in [g.width, g.height, g.depth, m.nframes, m.kind] {
            self.puti(i64::from(v));
        }
        for n in [
            m.bytes_per_vox,
            m.bytes_total,
            m.vox_per_row,
            m.vox_per_slice,
            m.vox_per_vol,
            m.vox_total,
        ] {
            self.puti(n as i64);
        }
        self.puti(i64::from(m.imnr0));
        self.puti(i64::from(m.imnr1));
        for f in [
            m.xstart, m.ystart, m.zstart, m.xend, m.yend, m.zend, m.fov, m.thick, m.ps,
        ] {
            self.putf(f);
        }
        self.puti(i64::from(m.scale));
        for v in [m.roi.dx, m.roi.dy, m.roi.dz] {
            self.puti(i64::from(v));
        }
        for v in [
            m.version,
            m.intent,
            m.dof,
            m.brightness,
            m.yinvert,
            m.ptype,
        ] {
            self.puti(i64::from(v));
        }
        self.putd(m.outside_val);
        self.putd(m.flip_angle);
        for f in [m.tr, m.te, m.ti, m.location, m.field_strength] {
            self.putf(f);
        }
        self.puts(m.pedir.as_deref());
        self.puti(i64::from(m.bvec_space));
        self.putm(m.bvals.as_ref());
        self.putm(m.bvecs.as_ref());
        self.puti(i64::from(m.warp_field_format));
        self.puti(i64::from(m.gcamorph_spacing));
        self.putd(m.gcamorph_exp_k);
        self.puti(m.cmdlines.len() as i64);
        for c in &m.cmdlines {
            self.puts(Some(c));
        }
        for s in [
            &m.subject_name,
            &m.path_to_t1,
            &m.fname_format,
            &m.gdf_image_stem,
            &m.transform_fname,
        ] {
            self.puts(Some(s));
        }
        self.putm(m.auto_align.as_ref());
        self.putm(m.orig_ras2vox.as_ref());
        self.puti(i64::from(m.free_transform));
        self.puti(i64::from(m.transform.is_some()));
        self.puti(i64::from(m.ct.is_some()));
        self.putg(&m.geom, false);
    }

    pub fn putidx(&mut self, m: &Mri) {
        if !m.index {
            self.out("noindex");
            return;
        }
        for k in 0..3 {
            let e = m.geom.extent(k);
            for i in [-500, -1, 0, 1, e - 1, e, e + 499] {
                self.puti(i64::from(m.index_value(k, i)));
            }
        }
    }

    pub fn putframes(&mut self, m: &Mri) {
        let Some(frames) = &m.frames else {
            self.out("noframes");
            return;
        };
        for x in frames.iter().take(m.nframes.max(0) as usize) {
            self.puti(i64::from(x.kind));
            for v in [x.te, x.tr, x.flip, x.ti, x.td] {
                self.putf(v);
            }
            self.puti(i64::from(x.sequence_type));
            self.putf(x.echo_spacing);
            self.putf(x.echo_train_len);
            for v in x.read_dir.iter().chain(&x.pe_dir).chain(&x.slice_dir) {
                self.putf(*v);
            }
            self.puti(i64::from(x.label));
            self.puts(Some(&x.name));
            self.puti(i64::from(x.dof));
            self.putm(x.m_ras2vox.as_ref());
            self.putf(x.thresh);
            self.puti(i64::from(x.units));
            for v in x.d {
                self.putd(v);
            }
            self.puti(i64::from(x.diffusion_type));
            for v in x.ramp_flat {
                self.puti(v);
            }
            for v in x.amp {
                self.putd(v);
            }
        }
    }

    pub fn put_volume(&mut self, m: &Mri) {
        self.puthdr(m);
        self.putidx(m);
        self.putframes(m);
        self.putvox(m);
    }

    // ---- colour tables

    pub fn getct(&mut self, inner: bool) -> Option<ColorTable> {
        if self.take("null") {
            return None;
        }
        let n = self.geti();
        let mut ct = ColorTable {
            fname: fixed_name(&self.getx(), 1024),
            version: self.geti(),
            idbase: self.geti(),
            schema: fixed_name(&self.getx(), 1024),
            ..ColorTable::default()
        };
        for _ in 0..n.max(0) {
            if self.geti() == 0 {
                ct.entries.push(None);
                continue;
            }
            let e = ColorTableEntry {
                name: fixed_name(&self.getx(), 1024),
                rgbai: [self.geti(), self.geti(), self.geti(), self.geti()],
                rgbaf: [self.getf(), self.getf(), self.getf(), self.getf()],
                tissue_type: self.geti(),
                count: self.geti(),
            };
            ct.entries.push(Some(e));
        }
        ct.tissue = if inner {
            self.take("null");
            None
        } else {
            self.getct(true).map(Box::new)
        };
        Some(ct)
    }

    pub fn putct(&mut self, ct: Option<&ColorTable>, counts: bool) {
        let Some(ct) = ct else {
            self.out("null");
            return;
        };
        self.puti(ct.entries.len() as i64);
        self.puts(Some(&ct.fname));
        self.puti(i64::from(ct.version));
        self.puti(i64::from(ct.idbase));
        self.puts(Some(&ct.schema));
        for e in &ct.entries {
            let Some(e) = e else {
                self.out("0");
                continue;
            };
            self.out("1");
            self.puts(Some(&e.name));
            for v in e.rgbai {
                self.puti(i64::from(v));
            }
            for v in e.rgbaf {
                self.putf(v);
            }
            self.puti(i64::from(e.tissue_type));
            if counts {
                self.puti(i64::from(e.count));
            }
        }
        self.putct(ct.tissue.as_deref(), counts);
    }

    // ---- header fields (H)

    pub fn seth(&mut self, p: &mut Process, m: &mut Mri) -> Result<(), Stop> {
        m.version = self.geti();
        m.intent = self.geti();
        m.dof = self.geti();
        m.mean = self.getd();
        m.geom.xsize = self.getf();
        m.geom.ysize = self.getf();
        m.geom.zsize = self.getf();
        m.imnr0 = self.geti();
        m.imnr1 = self.geti();
        m.ptype = self.geti();
        m.fov = self.getf();
        m.thick = self.getf();
        m.ps = self.getf();
        m.location = self.getf();
        m.xstart = self.getf();
        m.xend = self.getf();
        m.ystart = self.getf();
        m.yend = self.getf();
        m.zstart = self.getf();
        m.zend = self.getf();
        m.flip_angle = self.getd();
        m.tr = self.getf();
        m.te = self.getf();
        m.ti = self.getf();
        m.geom.fname = fixed_name(&self.getx(), 1024);
        for v in [&mut m.geom.x, &mut m.geom.y, &mut m.geom.z, &mut m.geom.c] {
            for k in 0..3 {
                v[k] = self.getf();
            }
        }
        m.geom.ras_good_flag = self.geti();
        m.brightness = self.geti();
        m.subject_name = fixed_name(&self.getx(), 1024);
        m.path_to_t1 = fixed_name(&self.getx(), 1024);
        m.fname_format = fixed_name(&self.getx(), 1024);
        m.gdf_image_stem = fixed_name(&self.getx(), 1024);
        m.transform_fname = fixed_name(&self.getx(), 1024);
        let n = self.geti();
        m.cmdlines.clear();
        for _ in 0..n.max(0) {
            let s = self.getx();
            m.cmdlines.push(s);
        }
        let (reg, aa, orig) = (self.getm(p)?, self.getm(p)?, self.getm(p)?);
        if reg.is_some() {
            m.geom.register_mat = reg;
        }
        if aa.is_some() {
            m.auto_align = aa;
        }
        if orig.is_some() {
            m.orig_ras2vox = orig;
        }
        if let Some(ct) = self.getct(false) {
            m.ct = Some(ct);
        }
        m.geom.valid = self.geti();
        m.geom.s = [self.getf(), self.getf(), self.getf()];
        m.scale = self.geti();
        m.yinvert = self.geti();
        m.outside_val = self.getd();
        m.field_strength = self.getf();
        Ok(())
    }

    // ---- MINC transforms (X)

    pub fn getxfm(&mut self, m: &mut Mri) {
        if self.take("none") {
            return;
        }
        let kind = self.geti();
        let inv = self.geti();
        let mut linear = [0.0f64; 16];
        let mut inverse = [0.0f64; 16];
        for v in linear.iter_mut().chain(inverse.iter_mut()) {
            *v = self.getd();
        }
        m.transform = Some(MincTransform {
            kind: TransformKind::of_code(kind),
            inverse_flag: inv != 0,
            linear,
            inverse,
        });
        m.free_transform = 1;
    }

    pub fn putxfm(&mut self, m: &Mri) {
        self.puti(i64::from(m.free_transform));
        match &m.transform {
            Some(t) => {
                for v in t.effective().iter().chain(t.effective_inverse()) {
                    self.putd(*v);
                }
            }
            None => {
                self.out("null");
                self.out("null");
            }
        }
        if m.free_transform != 0
            && let Some(t) = &m.transform
        {
            self.puti(i64::from(t.kind.code()));
            self.puti(i64::from(t.inverse_flag));
        }
    }

    // ---- fill and the box's volume

    pub fn fill(m: &mut Mri, seed: i32) {
        if seed < 0 {
            return;
        }
        if let Some(v) = m.voxels.as_mut() {
            for (i, b) in v.iter_mut().enumerate() {
                *b = ((seed as u64).wrapping_add(101u64.wrapping_mul(i as u64)) & 0xff) as u8;
            }
        }
    }

    /// B: V, a fill seed, then n settings `c r s f value` (`MRIsetVoxVal`, mode 0).
    pub fn getbv(&mut self, p: &mut Process) -> Result<Mri, Stop> {
        let mut m = self.getv(p)?;
        let seed = self.geti();
        Line::fill(&mut m, seed);
        let n = self.geti();
        for _ in 0..n.max(0) {
            let (c, r, s, f) = (self.geti(), self.geti(), self.geti(), self.geti());
            let v = self.getf();
            p.mri_set_vox_val(&mut m, c, r, s, f, v, 0)?;
        }
        Ok(m)
    }
}
