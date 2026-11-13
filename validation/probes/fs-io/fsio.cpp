// SPDX-License-Identifier: Apache-2.0
// fs.io's probe: FreeSurfer 8.2.0's file helpers (fio.cpp's readers and writers of one number on
// C streams and znzlib files, its name helpers and file tests, TSV::read and TSV::write;
// bfileio.cpp's byte swaps; MRIio_old.cpp's lcalloc) called case by case, linked with the unit
// oracle's libutils as the official programs are (build.sh). CLOSED TO FREESURFER CLEAN-ROOM
// IMPLEMENTERS: it names FreeSurfer's functions (running it is a black-box run of FreeSurfer's
// code).
//
//     fsio <cases>     one output line per case line (absolute path; see check.py)
//
// Every case runs in a child process, in a fixture directory made beside the cases (a file `f`,
// a directory `d` with a file `d/i`, an unreadable file `nr`, a read-only directory `ro`, links
// `ln` -> f, `dln` -> d, `bad` -> nowhere); its standard output and error go to a scratch file;
// the parent prints `<result> | <text>`, the text C-escaped (\xHH for bytes outside printable
// ASCII and for the backslash), or `EXIT <status> | <text>` / `CRASH <signal> | <text>` when
// FreeSurfer's code ended the child (each child is limited to 1 GiB of address space, 64 MiB of
// output and 5 seconds; text beyond 1 MiB is shown as `BIGTEXT <bytes>`). Numbers read are printed as their bytes in hex; strings as `N` (NULL) or `x<hex>`.

#include <fcntl.h>
#include <ftw.h>
#include <signal.h>
#include <sys/resource.h>
#include <sys/stat.h>
#include <sys/wait.h>
#include <unistd.h>

#include <cerrno>
#include <cinttypes>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <string>
#include <vector>

#include "bfileio.h"
#include "colortab.h"
#include "fstagsio.h"
#include "matrix.h"
#include "mri.h"
#include "tags.h"
#include "diag.h"
#include "error.h"
#include "fio.h"
#include "MRIio_old.h"
#include "znzlib.h"

const char *Progname = "fsio";

namespace {

std::vector<std::string> toks;
size_t pos;
std::string out;
std::string dat;  // the data file's absolute path

[[noreturn]] void fail(const char *what)
{
  fprintf(stderr, "fsio: %s\n", what);
  exit(2);
}

const char *next()
{
  if (pos >= toks.size()) fail("missing token");
  return toks[pos++].c_str();
}
long long geti() { return strtoll(next(), NULL, 10); }
uint32_t getx32() { return (uint32_t)strtoul(next(), NULL, 16); }

std::vector<unsigned char> unhex(const char *s)
{
  std::vector<unsigned char> b;
  if (strcmp(s, "-") == 0) return b;
  for (size_t i = 0; s[i] && s[i + 1]; i += 2) {
    char t[3] = {s[i], s[i + 1], 0};
    b.push_back((unsigned char)strtoul(t, NULL, 16));
  }
  return b;
}
// `N` or `x<hex>`; `isnull` set for N
std::string getstr(bool *isnull)
{
  const char *t = next();
  *isnull = (t[0] == 'N');
  std::string s;
  if (*isnull) return s;
  std::vector<unsigned char> b = unhex(t[1] ? t + 1 : "-");
  s.assign(b.begin(), b.end());
  return s;
}

void oi(long long v) { out += std::to_string(v) + " "; }
void ox(const void *p, size_t n)
{
  // the value's bytes most significant first (x86-64 memory is little-endian)
  const unsigned char *b = (const unsigned char *)p;
  char t[4];
  for (size_t i = n; i-- > 0;) {
    snprintf(t, sizeof t, "%02x", b[i]);
    out += t;
  }
  out += " ";
}
void obytes(const std::vector<unsigned char> &b)
{
  char t[4];
  if (b.empty()) out += "-";
  for (unsigned char c : b) {
    snprintf(t, sizeof t, "%02x", c);
    out += t;
  }
  out += " ";
}
void ostr(const char *s)
{
  if (!s) { out += "N "; return; }
  out += "x";
  obytes(std::vector<unsigned char>(s, s + strlen(s)));
  if (!*s) { out.pop_back(); out.pop_back(); out += " "; }
}

void write_file(const std::vector<unsigned char> &b)
{
  FILE *fp = fopen(dat.c_str(), "wb");
  if (!fp) fail("cannot write the data file");
  if (!b.empty()) fwrite(b.data(), 1, b.size(), fp);
  fclose(fp);
}
std::vector<unsigned char> read_file(const std::string &name)
{
  std::vector<unsigned char> b;
  FILE *fp = fopen(name.c_str(), "rb");
  if (!fp) return b;
  int c;
  while ((c = fgetc(fp)) != EOF) b.push_back((unsigned char)c);
  fclose(fp);
  return b;
}
// the decompressed bytes of a znz file
std::vector<unsigned char> read_znz(const std::string &name, int gz)
{
  std::vector<unsigned char> b;
  znzFile z = znzopen(name.c_str(), "rb", gz);
  if (znz_isnull(z)) return b;
  unsigned char c;
  while (znzread(&c, 1, 1, z) == 1) b.push_back(c);
  znzclose(z);
  return b;
}


// ---- piece 2: tags, FStagsIO, VOL_GEOM, binary colour tables ----
float getf() { uint32_t u = getx32(); float f; memcpy(&f, &u, 4); return f; }
double getd() { uint64_t u = strtoull(next(), NULL, 16); double d; memcpy(&d, &u, 8); return d; }
void of(float f) { ox(&f, 4); }
void od(double d) { ox(&d, 8); }
// the bytes of a char[STRLEN] buffer up to its last non-zero byte
void obuf(const char *b)
{
  size_t n = STRLEN;
  while (n > 0 && b[n - 1] == 0) n--;
  out += "x";
  obytes(std::vector<unsigned char>(b, b + n));
  if (n == 0) { out.pop_back(); out.pop_back(); out += " "; }
}
// a C string of at most STRLEN bytes
void ocs(const char *s)
{
  size_t n = strnlen(s, STRLEN);
  out += "x";
  obytes(std::vector<unsigned char>(s, s + n));
  if (n == 0) { out.pop_back(); out.pop_back(); out += " "; }
}
// two names strcpy'd one after the other into a buffer
void names_into(char *buf)
{
  bool n1, n2;
  std::string a = getstr(&n1), b = getstr(&n2);
  strcpy(buf, a.c_str());
  strcpy(buf, b.c_str());
}
void read_vg(VOL_GEOM &v)
{
  v.valid = (int)geti(); v.width = (int)geti(); v.height = (int)geti(); v.depth = (int)geti();
  float *f[] = {&v.xsize, &v.ysize, &v.zsize, &v.x_r, &v.x_a, &v.x_s, &v.y_r, &v.y_a, &v.y_s,
                &v.z_r, &v.z_a, &v.z_s, &v.c_r, &v.c_a, &v.c_s, &v.s_r, &v.s_a, &v.s_s};
  for (float *p : f) *p = getf();
  names_into(v.fname);
}
void print_vg(const VOL_GEOM &v)
{
  oi(v.valid); oi(v.width); oi(v.height); oi(v.depth);
  const float f[] = {v.xsize, v.ysize, v.zsize, v.x_r, v.x_a, v.x_s, v.y_r, v.y_a, v.y_s,
                     v.z_r, v.z_a, v.z_s, v.c_r, v.c_a, v.c_s, v.s_r, v.s_a, v.s_s};
  for (float x : f) of(x);
  obuf(v.fname);
}
MATRIX *read_mat()
{
  int k = (int)geti();
  if (k == 0) return NULL;
  MATRIX *m = MatrixAlloc(k, k, MATRIX_REAL);
  for (int r = 1; r <= k; r++)
    for (int c = 1; c <= k; c++) m->rptr[r][c] = getf();
  return m;
}
void print_mat(const MATRIX *m)
{
  if (!m) { out += "N "; return; }
  oi(m->rows);
  for (int r = 1; r <= m->rows; r++)
    for (int c = 1; c <= m->cols; c++) of(m->rptr[r][c]);
}
COLOR_TABLE *read_ct()
{
  COLOR_TABLE *ct = (COLOR_TABLE *)calloc(1, sizeof(COLOR_TABLE));
  ct->version = (int)geti();
  ct->nentries = (int)geti();
  bool n;
  std::string fn = getstr(&n);
  strcpy(ct->fname, fn.c_str());
  ct->entries = (COLOR_TABLE_ENTRY **)calloc(ct->nentries > 0 ? ct->nentries : 1, sizeof(CTE *));
  for (int i = 0; i < ct->nentries; i++) {
    if (geti() == 0) continue;
    CTE *e = (CTE *)calloc(1, sizeof(CTE));
    std::string nm = getstr(&n);
    strcpy(e->name, nm.c_str());
    e->ri = (int)geti(); e->gi = (int)geti(); e->bi = (int)geti(); e->ai = (int)geti();
    ct->entries[i] = e;
  }
  return ct;
}
void print_ct(const COLOR_TABLE *ct)
{
  if (!ct) { out += "N "; return; }
  oi(ct->nentries); oi(ct->version); ocs(ct->fname);
  // the non-empty entries, each after its index (a table can have millions of empty places)
  for (int i = 0; i < ct->nentries; i++) {
    const CTE *e = ct->entries[i];
    if (!e) continue;
    out += "@" + std::to_string(i) + " ";
    ocs(e->name); oi(e->ri); oi(e->gi); oi(e->bi); oi(e->ai);
    of(e->rf); of(e->gf); of(e->bf); of(e->af);
  }
}
void read_frame(MRI_FRAME *f)
{
  f->type = (int)geti();
  f->TE = getf(); f->TR = getf(); f->flip = getf(); f->TI = getf(); f->TD = getf();
  f->TM = getd();
  f->sequence_type = (int)geti();
  f->echo_spacing = getf(); f->echo_train_len = getf();
  for (int i = 0; i < 3; i++) f->read_dir[i] = getf();
  for (int i = 0; i < 3; i++) f->pe_dir[i] = getf();
  for (int i = 0; i < 3; i++) f->slice_dir[i] = getf();
  f->label = (int)geti();
  names_into(f->name);
  f->dof = (int)geti();
  if (f->m_ras2vox) MatrixFree(&f->m_ras2vox);
  f->m_ras2vox = read_mat();
  f->thresh = getf();
  f->units = (int)geti();
  f->DX = getd(); f->DY = getd(); f->DZ = getd(); f->DR = getd(); f->DP = getd(); f->DS = getd();
  f->bvalue = getd();
  long *ramp[] = {&f->D1_ramp, &f->D2_ramp, &f->D3_ramp, &f->D4_ramp};
  long *flat[] = {&f->D1_flat, &f->D2_flat, &f->D3_flat, &f->D4_flat};
  double *amp[] = {&f->D1_amp, &f->D2_amp, &f->D3_amp, &f->D4_amp};
  for (int i = 0; i < 4; i++) { *ramp[i] = strtoll(next(), NULL, 10); *flat[i] = strtoll(next(), NULL, 10); *amp[i] = getd(); }
}
void print_frame(const MRI_FRAME *f)
{
  oi(f->type); of(f->TE); of(f->TR); of(f->flip); of(f->TI); of(f->TD); od(f->TM);
  oi(f->sequence_type); of(f->echo_spacing); of(f->echo_train_len);
  for (int i = 0; i < 3; i++) of(f->read_dir[i]);
  for (int i = 0; i < 3; i++) of(f->pe_dir[i]);
  for (int i = 0; i < 3; i++) of(f->slice_dir[i]);
  oi(f->label); obuf(f->name); oi(f->dof); print_mat(f->m_ras2vox); of(f->thresh); oi(f->units);
  od(f->DX); od(f->DY); od(f->DZ); od(f->DR); od(f->DP); od(f->DS); od(f->bvalue);
  const long ramp[] = {f->D1_ramp, f->D2_ramp, f->D3_ramp, f->D4_ramp};
  const long flat[] = {f->D1_flat, f->D2_flat, f->D3_flat, f->D4_flat};
  const double amp[] = {f->D1_amp, f->D2_amp, f->D3_amp, f->D4_amp};
  for (int i = 0; i < 4; i++) { oi(ramp[i]); oi(flat[i]); od(amp[i]); }
}
// the data file in mode 0 (plain), 1 (compressed) or 2 (plain, opened as compressed)
znzFile data_znz(int mode, const std::vector<unsigned char> &b)
{
  if (mode == 1) {
    znzFile w = znzopen(dat.c_str(), "wb", 1);
    if (znz_isnull(w)) fail("cannot write the data file");
    if (!b.empty()) znzwrite((void *)b.data(), 1, b.size(), w);
    znzclose(w);
  } else {
    write_file(b);
  }
  znzFile z = znzopen(dat.c_str(), "rb", mode != 0);
  if (znz_isnull(z)) fail("cannot open the data file");
  return z;
}
int ***labels_alloc(int x0, int y0, int z0)
{
  int ***l = (int ***)calloc(x0 > 0 ? x0 : 1, sizeof(int **));
  for (int x = 0; x < x0; x++) {
    l[x] = (int **)calloc(y0 > 0 ? y0 : 1, sizeof(int *));
    for (int y = 0; y < y0; y++) {
      l[x][y] = (int *)calloc(z0 > 0 ? z0 : 1, sizeof(int));
      for (int z = 0; z < z0; z++) l[x][y][z] = -1;
    }
  }
  return l;
}
bool more() { return pos < toks.size(); }

void run_piece2(const std::string &cmd)
{
  if (cmd == "vgw") {
    int gz = (int)geti(), nifti = (int)geti(), shearless = (int)geti();
    VOL_GEOM v;
    read_vg(v);
    znzFile z = znzopen(dat.c_str(), "wb", gz);
    v.write(z, nifti, shearless);
    znzclose(z);
    obytes(read_znz(dat, gz));
  } else if (cmd == "vgr") {
    int mode = (int)geti(), nifti = (int)geti(), shearless = (int)geti();
    Gdiag_no = (int)geti();
    VOL_GEOM v;
    read_vg(v);
    znzFile z = data_znz(mode, unhex(next()));
    v.read(z, nifti, shearless);
    print_vg(v);
    oi(znzeof(z) != 0);
    oi(znztell(z));
    znzclose(z);
  } else if (cmd == "fsw") {
    std::string op = next();
    int gz = (int)geti(), nifti = (int)geti();
    Gdiag = (int)geti();
    znzFile z = znzopen(dat.c_str(), "wb", gz);
    FStagsIO io(z, nifti);
    int r = 0;
    if (op == "tag") {
      int tag = (int)geti();
      std::vector<unsigned char> d = unhex(next());
      long long dlen = geti();
      d.resize(d.size() + 8, 0);
      r = io.write_tag(tag, d.data(), dlen);
    } else if (op == "mat") {
      int tag = (int)geti();
      MATRIX *m = read_mat();
      r = io.write_matrix(m, tag);
    } else if (op == "ctab") {
      r = io.write_old_colortable(read_ct());
    } else if (op == "frames") {
      int nf = (int)geti();
      MRI *mri = MRIallocHeader(1, 1, 1, MRI_UCHAR, nf);
      for (int i = 0; i < nf; i++) read_frame(&mri->frames[i]);
      r = io.write_mri_frames(mri);
    } else if (op == "geom") {
      int shearless = (int)geti();
      VOL_GEOM a, b;
      read_vg(a);
      read_vg(b);
      r = io.write_gcamorph_geom(&a, &b, shearless);
    } else if (op == "meta") {
      int a = (int)geti(), b = (int)geti();
      r = io.write_gcamorph_meta(a, b, getd());
    } else if (op == "labels") {
      int x0 = (int)geti(), y0 = (int)geti(), z0 = (int)geti();
      int ***l = labels_alloc(x0, y0, z0);
      for (int x = 0; x < x0; x++)
        for (int y = 0; y < y0; y++)
          for (int zz = 0; zz < z0; zz++) l[x][y][zz] = (int)geti();
      r = io.write_gcamorph_labels(x0, y0, z0, l);
    } else if (op == "dof") {
      r = io.write_dof((int)geti());
    } else if (op == "scan") {
      MRI *mri = MRIallocHeader(1, 1, 1, MRI_UCHAR, 1);
      mri->te = getf(); mri->ti = getf(); mri->flip_angle = getd(); mri->FieldStrength = getf();
      bool n;
      std::string pe = getstr(&n);
      if (!n) mri->pedir = strdup(pe.c_str());
      r = io.write_scan_parameters(mri);
    } else if (op == "ras" || op == "rasx") {
      VOL_GEOM v;
      read_vg(v);
      r = op == "ras" ? io.write_ras_xform(&v) : io.write_ras_xform_extra(&v);
    } else if (op == "end") {
      r = io.write_endtag();
    } else fail("unknown writer");
    znzclose(z);
    oi(r);
    obytes(read_znz(dat, gz));
  } else if (cmd == "fsr") {
    int mode = (int)geti(), nifti = (int)geti();
    Gdiag = (int)geti();
    Gdiag_no = (int)geti();
    znzFile z = data_znz(mode, unhex(next()));
    FStagsIO io(z, nifti);
    while (more()) {
      std::string op = next();
      out += op + ": ";
      if (op == "t") {
        int zl = (int)geti();
        long long len = -7777;
        oi(io.read_tagid_len(&len, zl));
        oi(len);
      } else if (op == "d") {
        long long n = geti();
        std::vector<unsigned char> b((size_t)n + 1, 0xee);
        oi(io.read_data(b.data(), n));
        b.resize((size_t)n);
        obytes(b);
      } else if (op == "m") {
        errno = 0;
        MATRIX *m = io.read_matrix();
        int e = errno;
        print_mat(m);
        oi(e);
      } else if (op == "c") {
        print_ct(io.read_old_colortable());
      } else if (op == "f") {
        int nf = (int)geti();
        long long len = geti();
        MRI *mri = MRIallocHeader(1, 1, 1, MRI_UCHAR, nf);
        oi(io.read_mri_frames(mri, len));
        for (int i = 0; i < nf; i++) print_frame(&mri->frames[i]);
      } else if (op == "g") {
        int shearless = (int)geti();
        VOL_GEOM a, b;
        read_vg(a);
        read_vg(b);
        oi(io.read_gcamorph_geom(&a, &b, shearless));
        print_vg(a);
        print_vg(b);
      } else if (op == "e") {
        int a = 0, b = 0;
        double c = 0;
        oi(io.read_gcamorph_meta(&a, &b, &c));
        oi(a); oi(b); od(c);
      } else if (op == "l") {
        int x0 = (int)geti(), y0 = (int)geti(), z0 = (int)geti();
        int ***l = labels_alloc(x0, y0, z0);
        oi(io.read_gcamorph_labels(x0, y0, z0, l));
        for (int x = 0; x < x0; x++)
          for (int y = 0; y < y0; y++)
            for (int zz = 0; zz < z0; zz++) oi(l[x][y][zz]);
      } else if (op == "o") {
        int d = 0;
        oi(io.read_dof(&d));
        oi(d);
      } else if (op == "s") {
        long long dlen = geti();
        MRI *mri = MRIallocHeader(1, 1, 1, MRI_UCHAR, 1);
        oi(io.read_scan_parameters(mri, dlen));
        of(mri->te); of(mri->ti); od(mri->flip_angle); of(mri->FieldStrength);
        if (mri->pedir) ocs(mri->pedir); else out += "N ";
      } else if (op == "r" || op == "x") {
        VOL_GEOM v;  // C++ leaves these four unset (memory contents): the probe sets them
        v.valid = v.width = v.height = v.depth = 0;
        oi(op == "r" ? io.read_ras_xform(&v) : io.read_ras_xform_extra(&v));
        print_vg(v);
      } else if (op == "k") {
        int tag = (int)geti();
        long long len = geti();
        oi(io.skip_tag(tag, len));
      } else fail("unknown reader");
    }
    oi(znzeof(z) != 0);
    oi(znztell(z));
    znzclose(z);
  } else if (cmd == "fsl") {
    std::string op = next();
    long long r = 0;
    if (op == "tag") {
      int tag = (int)geti(); long long len = geti(); int n = (int)geti(), a = (int)geti();
      r = FStagsIO::getlen_tag(tag, len, n, a);
    } else if (op == "mat") {
      int n = (int)geti(), a = (int)geti();
      r = FStagsIO::getlen_matrix(n, a);
    } else if (op == "ctab") {
      int n = (int)geti(), a = (int)geti();
      r = FStagsIO::getlen_old_colortable(read_ct(), n, a);
    } else if (op == "frames") {
      int nf = (int)geti(), n = (int)geti(), a = (int)geti();
      MRI *mri = MRIallocHeader(1, 1, 1, MRI_UCHAR, nf);
      r = FStagsIO::getlen_mri_frames(mri, n, a);
    } else if (op == "geom") {
      bool n1, n2;
      std::string s = getstr(&n1), t = getstr(&n2);
      int n = (int)geti(), a = (int)geti(), sh = (int)geti();
      r = FStagsIO::getlen_gcamorph_geom(s.c_str(), t.c_str(), n, a, sh);
    } else if (op == "meta") {
      r = FStagsIO::getlen_gcamorph_meta((int)geti());
    } else if (op == "labels") {
      int x = (int)geti(), y = (int)geti(), zz = (int)geti(), l = (int)geti(), n = (int)geti(), a = (int)geti();
      r = FStagsIO::getlen_gcamorph_labels(x, y, zz, l, n, a);
    } else if (op == "dof") {
      r = FStagsIO::getlen_dof(0, (int)geti());
    } else if (op == "scan") {
      int a = (int)geti();
      bool n;
      std::string pe = getstr(&n);
      MRI *mri = MRIallocHeader(1, 1, 1, MRI_UCHAR, 1);
      if (!n) mri->pedir = strdup(pe.c_str());
      r = FStagsIO::getlen_scan_parameters(mri, a);
    } else if (op == "ras") {
      VOL_GEOM v;
      r = FStagsIO::getlen_ras_xform(&v, (int)geti());
    } else if (op == "rasx") {
      VOL_GEOM v;
      r = FStagsIO::getlen_ras_xform_extra(&v, (int)geti());
    } else if (op == "end") {
      r = FStagsIO::getlen_endtag((int)geti());
    } else if (op == "sizeof") {
      r = (long long)sizeof(MRI_FRAME) * 1000000 + (long long)sizeof(CTE);
    } else fail("unknown length");
    oi(r);
  } else if (cmd == "ctw") {
    int stream = (int)geti(), gz = (int)geti();
    Gdiag = (int)geti();
    COLOR_TABLE *ct = read_ct();
    int r;
    if (stream == 0) {
      FILE *fp = fopen(dat.c_str(), "wb");
      r = CTABwriteIntoBinary(ct, fp);
      fclose(fp);
      oi(r);
      obytes(read_file(dat));
    } else {
      znzFile z = znzopen(dat.c_str(), "wb", gz);
      r = znzCTABwriteIntoBinary(ct, z);
      znzclose(z);
      oi(r);
      obytes(read_znz(dat, gz));
    }
  } else if (cmd == "ctr") {
    int stream = (int)geti(), mode = (int)geti();
    Gdiag = (int)geti();
    Gdiag_no = (int)geti();
    std::vector<unsigned char> b = unhex(next());
    if (stream == 0) {
      write_file(b);
      FILE *fp = fopen(dat.c_str(), "rb");
      print_ct(CTABreadFromBinary(fp));
      oi(feof(fp) != 0);
      oi(ftell(fp));
      fclose(fp);
    } else {
      znzFile z = data_znz(mode, b);
      print_ct(znzCTABreadFromBinary(z));
      oi(znzeof(z) != 0);
      oi(znztell(z));
      znzclose(z);
    }
  } else if (cmd == "tw" || cmd == "ztw") {
    bool zn = cmd == "ztw";
    int gz = zn ? (int)geti() : 0;
    FILE *fp = NULL;
    znzFile z = NULL;
    if (zn) z = znzopen(dat.c_str(), "wb", gz); else fp = fopen(dat.c_str(), "wb");
    while (more()) {
      std::string op = next();
      out += op + ": ";
      if (op == "s") {
        int tag = (int)geti(); long long len = geti(), here = -7777;
        oi(zn ? znzTAGwriteStart(z, tag, &here, len) : TAGwriteStart(fp, tag, &here, len));
        oi(here);
      } else if (op == "w") {
        int tag = (int)geti();
        std::vector<unsigned char> d = unhex(next());
        long long len = geti();
        d.resize(d.size() + 8, 0);
        oi(TAGwrite(fp, tag, d.data(), len));
      } else if (op == "m") {
        int tag = (int)geti();
        MATRIX *m = read_mat();
        oi(zn ? znzWriteMatrix(z, m, tag) : TAGwriteMatrix(fp, m, tag));
      } else if (op == "e") {
        oi(zn ? znzTAGwriteEnd(z, 0) : TAGwriteEnd(fp, 0));
      } else fail("unknown tag writer");
    }
    if (zn) znzclose(z); else fclose(fp);
    obytes(zn ? read_znz(dat, gz) : read_file(dat));
  } else if (cmd == "tr" || cmd == "ztr") {
    bool zn = cmd == "ztr";
    int mode = zn ? (int)geti() : 0;
    Gdiag_no = (int)geti();
    std::vector<unsigned char> b = unhex(next());
    FILE *fp = NULL;
    znzFile z = NULL;
    if (zn) z = data_znz(mode, b);
    else { write_file(b); fp = fopen(dat.c_str(), "rb"); }
    while (more()) {
      std::string op = next();
      out += op + ": ";
      if (op == "t") {
        long long len = -7777;
        if (zn) { int zl = (int)geti(); oi(znzTAGreadStart(z, &len, zl)); }
        else oi(TAGreadStart(fp, &len));
        oi(len);
      } else if (op == "k") {
        int tag = (int)geti();
        long long len = geti();
        oi(TAGskip(fp, tag, len));
      } else if (op == "d") {
        long long n = geti();
        std::vector<unsigned char> d((size_t)n + 1, 0xee);
        oi(TAGread(fp, d.data(), n));
        d.resize((size_t)n);
        obytes(d);
      } else if (op == "m") {
        errno = 0;
        MATRIX *m = zn ? znzReadMatrix(z) : TAGreadMatrix(fp);
        int e = errno;
        print_mat(m);
        oi(e);
      } else fail("unknown tag reader");
    }
    if (zn) { oi(znzeof(z) != 0); oi(znztell(z)); znzclose(z); }
    else { oi(feof(fp) != 0); oi(ftell(fp)); fclose(fp); }
  } else if (cmd == "cmdl") {
    std::vector<std::string> args;
    while (more()) { bool n; args.push_back(getstr(&n)); }
    std::vector<char *> argv;
    for (std::string &a : args) argv.push_back((char *)a.c_str());
    static char line[1 << 16];
    oi(TAGmakeCommandLineString((int)argv.size(), argv.data(), line));
    out += "x";
    obytes(std::vector<unsigned char>(line, line + strlen(line)));
    if (!line[0]) { out.pop_back(); out.pop_back(); out += " "; }
  } else {
    fail("unknown command");
  }
}

void run_case()
{
  std::string cmd = next();
  if (cmd == "rd") {
    std::string fn = next();
    Gdiag_no = (int)geti();
    int count = (int)geti();
    int32_t init = (int32_t)getx32();
    write_file(unhex(next()));
    FILE *fp = fopen(dat.c_str(), "rb");
    if (!fp) fail("cannot open the data file");
    for (int k = 0; k < count; k++) {
      if (fn == "r1") { int v = 0x5a5a5a5a; oi(fread1(&v, fp)); ox(&v, 4); }
      else if (fn == "r2") { int v = 0x5a5a5a5a; oi(fread2(&v, fp)); ox(&v, 4); }
      else if (fn == "r3") { int v = 0x5a5a5a5a; oi(fread3(&v, fp)); ox(&v, 4); }
      else if (fn == "rd") { double d = freadDouble(fp); ox(&d, 8); }
      else if (fn == "ri") { int i = freadInt(fp); ox(&i, 4); }
      else if (fn == "rl") { long long l = freadLong(fp); ox(&l, 8); }
      else if (fn == "rix") { int i = init; oi(freadIntEx(&i, fp)); ox(&i, 4); }
      else if (fn == "rf") { float f = freadFloat(fp); ox(&f, 4); }
      else fail("unknown reader");
    }
    oi(feof(fp) != 0);
    fclose(fp);
  } else if (cmd == "zr") {
    std::string fn = next();
    Gdiag_no = (int)geti();
    int mode = (int)geti();
    int count = (int)geti();
    uint32_t init = getx32();
    std::vector<unsigned char> b = unhex(next());
    if (mode == 1) {
      znzFile w = znzopen(dat.c_str(), "wb", 1);
      if (znz_isnull(w)) fail("cannot write the data file");
      if (!b.empty()) znzwrite(b.data(), 1, b.size(), w);
      znzclose(w);
    } else {
      write_file(b);
    }
    znzFile z = znzopen(dat.c_str(), "rb", mode != 0);
    if (znz_isnull(z)) fail("cannot open the data file");
    for (int k = 0; k < count; k++) {
      if (fn == "z1") { int v = 0x5a5a5a5a; oi(znzread1(&v, z)); ox(&v, 4); }
      else if (fn == "zs") { short s = znzreadShort(z); ox(&s, 2); }
      else if (fn == "zi") { int i = znzreadInt(z); ox(&i, 4); }
      else if (fn == "zl") { long long l = znzreadLong(z); ox(&l, 8); }
      else if (fn == "zf") { float f = znzreadFloat(z); ox(&f, 4); }
      else if (fn == "zd") { double d = znzreadDouble(z); ox(&d, 8); }
      else if (fn == "zix") { int i = (int)init; oi(znzreadIntEx(&i, z)); ox(&i, 4); }
      else if (fn == "zfx") { float f; memcpy(&f, &init, 4); oi(znzreadFloatEx(&f, z)); ox(&f, 4); }
      else fail("unknown reader");
    }
    oi(znzeof(z) != 0);
    znzclose(z);
  } else if (cmd == "wr") {
    std::string fn = next();
    const char *v = next();
    FILE *fp = fopen(dat.c_str(), "wb");
    if (!fp) fail("cannot write the data file");
    int r;
    if (fn == "w2") r = fwrite2((int)strtoll(v, NULL, 10), fp);
    else if (fn == "w3") r = fwrite3((int)strtoll(v, NULL, 10), fp);
    else if (fn == "wi") r = fwriteInt((int)strtoll(v, NULL, 10), fp);
    else if (fn == "wl") r = fwriteLong(strtoll(v, NULL, 10), fp);
    else if (fn == "wf") { uint32_t u = (uint32_t)strtoul(v, NULL, 16); float f; memcpy(&f, &u, 4); r = fwriteFloat(f, fp); }
    else fail("unknown writer");
    fclose(fp);
    oi(r);
    obytes(read_file(dat));
  } else if (cmd == "zw") {
    std::string fn = next();
    int gz = (int)geti();
    const char *v = next();
    znzFile z = znzopen(dat.c_str(), "wb", gz);
    if (znz_isnull(z)) fail("cannot write the data file");
    int r;
    if (fn == "zws") r = znzwriteShort((short)strtoll(v, NULL, 10), z);
    else if (fn == "zwus") r = znzwriteUShort((unsigned short)strtoll(v, NULL, 10), z);
    else if (fn == "zwi") r = znzwriteInt((int)strtoll(v, NULL, 10), z);
    else if (fn == "zwl") r = znzwriteLong(strtoll(v, NULL, 10), z);
    else if (fn == "zwf") { uint32_t u = (uint32_t)strtoul(v, NULL, 16); float f; memcpy(&f, &u, 4); r = znzwriteFloat(f, z); }
    else if (fn == "zwd") { uint64_t u = strtoull(v, NULL, 16); double d; memcpy(&d, &u, 8); r = znzwriteDouble(d, z); }
    else fail("unknown writer");
    znzclose(z);
    oi(r);
    obytes(read_znz(dat, gz));
  } else if (cmd == "dirname") {
    bool n;
    std::string s = getstr(&n);
    char *r = fio_dirname(n ? NULL : s.c_str());
    ostr(r);
    free(r);
  } else if (cmd == "basename") {
    bool n, ne;
    std::string s = getstr(&n);
    std::string e = getstr(&ne);
    char *r = fio_basename(n ? NULL : s.c_str(), ne ? NULL : e.c_str());
    ostr(r);
    free(r);
  } else if (cmd == "ext") {
    bool n;
    std::string s = getstr(&n);
    char *r = fio_extension(n ? NULL : s.c_str());
    ostr(r);
    free(r);
  } else if (cmd == "fsx") {
    std::string fn = next();
    bool n;
    std::string s = getstr(&n);
    int r, e;
    errno = 0;
    if (fn == "exists") { r = fio_FileExistsReadable(s.c_str()); e = errno; }
    else if (fn == "isdir") { r = fio_IsDirectory(s.c_str()); e = errno; }
    else if (fn == "dirw0" || fn == "dirw1") {
      int f = fn == "dirw1";
      r = fio_DirIsWritable(s.c_str(), f);
      e = errno;
      std::string junk = f ? s + ".junk.54_-_sdfklj" : s + "/.junk.54_-_sdfklj";
      struct stat st;
      int left = stat(junk.c_str(), &st) == 0;
      oi(r);
      oi(left);
      r = -1;
    } else fail("unknown test");
    if (r != -1) oi(r);
    oi(e);
  } else if (cmd == "tsv") {
    int debug = (int)geti();
    bool nofmt;
    std::string fmt = getstr(&nofmt);
    const char *content = next();
    unlink("t.tsv");
    if (strcmp(content, "N") != 0) {
      std::vector<unsigned char> b = unhex(content);
      FILE *fp = fopen("t.tsv", "wb");
      if (!fp) fail("cannot write t.tsv");
      if (!b.empty()) fwrite(b.data(), 1, b.size(), fp);
      fclose(fp);
    }
    TSV t;
    t.debug = debug;
    errno = 0;
    oi(t.read("t.tsv"));
    int e = errno;
    oi(t.ncols());
    for (const std::string &h : t.headers) {
      out += "x";
      obytes(std::vector<unsigned char>(h.begin(), h.end()));
      if (h.empty()) { out.pop_back(); out.pop_back(); out += " "; }
    }
    oi(t.nrows());
    for (const std::vector<double> &row : t.data) {
      oi((long long)row.size());
      for (double v : row) ox(&v, 8);
    }
    oi(e);
    if (!nofmt) {
      oi(t.write("o.tsv", fmt));
      obytes(read_file("o.tsv"));
      unlink("o.tsv");
    }
    unlink("t.tsv");
  } else if (cmd == "swap") {
    std::string fn = next();
    long count = (long)geti();
    int buflen = (int)geti();
    std::vector<unsigned char> b = unhex(next());
    // room after the buffer, so that FreeSurfer's writes past it stay in the probe's memory
    std::vector<unsigned char> room(b);
    room.resize(b.size() + 64, 0xee);
    if (fn == "s2") byteswapbufshort(room.data(), count);
    else if (fn == "s4") byteswapbuffloat(room.data(), count);
    else if (fn == "s8") byteswapbufdouble(room.data(), count);
    else fail("unknown swap");
    room.resize(buflen);
    obytes(room);
  } else if (cmd == "calloc") {
    unsigned long long nmemb = strtoull(next(), NULL, 10);
    unsigned long long size = strtoull(next(), NULL, 10);
    char *p = lcalloc((size_t)nmemb, (size_t)size);
    unsigned long long n = nmemb * size;
    int nonzero = 0;
    for (unsigned long long i = 0; i < n && i < 64; i++) nonzero |= p[i] != 0;
    out += "ok ";
    oi(nonzero);
    free(p);
  } else {
    run_piece2(cmd);
  }
}

std::string escape(const std::string &s)
{
  std::string r;
  char b[8];
  for (unsigned char c : s) {
    if (c < 0x20 || c > 0x7e || c == '\\') {
      snprintf(b, sizeof b, "\\x%02x", c);
      r += b;
    } else {
      r += (char)c;
    }
  }
  return r;
}

const char *MARK = "\n@@RESULT ";

int rm_entry(const char *p, const struct stat *, int, struct FTW *)
{
  chmod(p, 0700);
  return remove(p);
}

void make_fixture(const std::string &fix)
{
  nftw(fix.c_str(), rm_entry, 16, FTW_DEPTH | FTW_PHYS);
  if (mkdir(fix.c_str(), 0700)) fail("cannot make the fixture");
  auto file = [&](const std::string &n, const char *text) {
    FILE *f = fopen((fix + "/" + n).c_str(), "w");
    if (!f) fail("cannot write a fixture file");
    fputs(text, f);
    fclose(f);
  };
  file("f", "abc\n");
  mkdir((fix + "/d").c_str(), 0700);
  file("d/i", "x");
  file("nr", "secret");
  chmod((fix + "/nr").c_str(), 0);
  mkdir((fix + "/ro").c_str(), 0500);
  if (symlink("f", (fix + "/ln").c_str()) || symlink("d", (fix + "/dln").c_str()) ||
      symlink("nowhere", (fix + "/bad").c_str()))
    fail("cannot make the fixture's links");
}

}  // namespace

int main(int argc, char *argv[])
{
  if (argc != 2) {
    fprintf(stderr, "usage: fsio <cases>\n");
    return 2;
  }
  if (argv[1][0] != '/') fail("the cases must be given by an absolute path");
  std::string scratch = std::string(argv[1]) + ".out";
  dat = std::string(argv[1]) + ".dat";
  std::string fix = std::string(argv[1]) + ".fix";
  make_fixture(fix);
  FILE *fp = fopen(argv[1], "r");
  if (!fp) fail("cannot open the cases");
  std::vector<std::string> lines;
  static char buf[1 << 20];
  while (fgets(buf, sizeof buf, fp)) lines.push_back(buf);
  fclose(fp);
  for (const std::string &line : lines) {
    if (line.size() >= sizeof buf) fail("a case longer than the buffer");
    fflush(stdout);
    pid_t pid = fork();
    if (pid < 0) fail("fork");
    if (pid == 0) {
      int fd = open(scratch.c_str(), O_WRONLY | O_CREAT | O_TRUNC, 0600);
      if (fd < 0) _exit(3);
      dup2(fd, 1);
      dup2(fd, 2);
      close(fd);
      setvbuf(stdout, NULL, _IONBF, 0);
      if (chdir(fix.c_str())) _exit(4);
      // a case that reads for ever (TSV::read, FS-172) stops at 1 GiB of memory or 5 seconds
      struct rlimit rl = {1ul << 30, 1ul << 30};
      setrlimit(RLIMIT_AS, &rl);
      // and at most 64 MiB of messages (a reader that prints for each of millions of entries)
      struct rlimit fl = {64ul << 20, 64ul << 20};
      setrlimit(RLIMIT_FSIZE, &fl);
      alarm(5);
      memcpy(buf, line.c_str(), line.size() + 1);
      toks.clear();
      pos = 0;
      for (char *t = strtok(buf, " \t\n"); t; t = strtok(NULL, " \t\n")) toks.push_back(t);
      if (!toks.empty()) {
        out.clear();
        Gerror = NO_ERROR;
        errno = 0;
        run_case();
        if (!out.empty() && out.back() == ' ') out.pop_back();
        fputs(MARK, stdout);
        fputs(out.c_str(), stdout);
      }
      fflush(stdout);
      _exit(0);
    }
    int status = 0;
    waitpid(pid, &status, 0);
    std::string text;
    if (FILE *r = fopen(scratch.c_str(), "rb")) {
      int ch;
      while ((ch = fgetc(r)) != EOF) text += (char)ch;
      fclose(r);
    }
    std::string result;
    size_t at = text.rfind(MARK);
    bool clean = WIFEXITED(status) && WEXITSTATUS(status) == 0 && at != std::string::npos;
    if (clean) {
      result = text.substr(at + strlen(MARK));
      text = text.substr(0, at);
    } else if (WIFSIGNALED(status)) {
      result = "CRASH " + std::to_string(WTERMSIG(status));
    } else {
      result = "EXIT " + std::to_string(WEXITSTATUS(status));
    }
    // messages beyond 1 MiB are shown by their length (the port does the same)
    if (text.size() > (1u << 20)) text = "BIGTEXT " + std::to_string(text.size());
    printf("%s | %s\n", result.c_str(), escape(text).c_str());
  }
  unlink(scratch.c_str());
  unlink(dat.c_str());
  nftw(fix.c_str(), rm_entry, 16, FTW_DEPTH | FTW_PHYS);
  return 0;
}
