// SPDX-License-Identifier: Apache-2.0
// fs-normalize's probe: FreeSurfer 8.2.0's mri_normalize's own functions (mri_normalize.cpp, its
// main renamed, compiled into this file) called case by case, linked with the unit oracle's
// libutils as the official program is (build.sh). CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS: it
// names FreeSurfer's functions (running it is a black-box run of FreeSurfer's code).
//
//     fsnormalize <cases>     one output line per case line (absolute path; see check.py)
//
// Every case runs in a child process in the folder <cases>.d (emptied first), its standard output
// and error to a scratch file; the parent prints `<result> | <text>`, the text C-escaped, or
// `EXIT <status> | <text>` / `CRASH <signal> | <text>`. Floats in and out are their bits in hex; a
// voxel token `v*k` stands for k voxels of the value v. Volumes the case's functions write are
// read back (MRIread) and printed after the result, with the FNV-1a of other files.

#define main mri_normalize_main
#include "mri_normalize.cpp"
#undef main

#include <dirent.h>
#include <fcntl.h>
#include <signal.h>
#include <sys/stat.h>
#include <sys/wait.h>
#include <unistd.h>

#include <algorithm>
#include <cerrno>
#include <cinttypes>
#include <cstdint>
#include <cstring>
#include <string>
#include <vector>

#include "label.h"
#include "mrisurf.h"
#include "utils.h"

namespace probe {

std::vector<std::string> toks;
size_t pos;
std::string out;
std::string scratch, workdir;

[[noreturn]] void fail(const char *what)
{
  fprintf(stderr, "fsnormalize: %s\n", what);
  exit(2);
}

const char *next()
{
  if (pos >= toks.size()) fail("missing token");
  return toks[pos++].c_str();
}
int geti() { return (int)strtol(next(), NULL, 10); }
float getf()
{
  uint32_t u = (uint32_t)strtoul(next(), NULL, 16);
  float f;
  memcpy(&f, &u, 4);
  return f;
}
void oi(long long v) { out += std::to_string(v) + " "; }
void of(float f)
{
  uint32_t u;
  memcpy(&u, &f, 4);
  char b[16];
  snprintf(b, sizeof b, "%08x ", u);
  out += b;
}
void od(double d)
{
  uint64_t u;
  memcpy(&u, &d, 8);
  char b[24];
  snprintf(b, sizeof b, "%016" PRIx64 " ", u);
  out += b;
}
void os(const char *s) { out += std::string(s ? s : "NULL") + " "; }

// a volume: w h d type nframes xsize ysize zsize, then its stored voxels
MRI *get_vol()
{
  int w = geti(), h = geti(), d = geti(), type = geti(), nf = geti();
  MRI *m = MRIallocSequence(w, h, d, type, nf);
  m->xsize = getf();
  m->ysize = getf();
  m->zsize = getf();
  size_t n = (size_t)w * h * d * nf, i = 0;
  unsigned char *b = (unsigned char *)m->chunk;
  while (i < n) {
    std::string t = next();
    size_t star = t.find('*');
    size_t k = star == std::string::npos ? 1 : strtoul(t.c_str() + star + 1, NULL, 10);
    std::string v = t.substr(0, star);
    for (size_t j = 0; j < k && i < n; j++, i++) {
      switch (type) {
        case MRI_UCHAR: b[i] = (unsigned char)strtol(v.c_str(), NULL, 10); break;
        case MRI_SHORT: { short s = (short)strtol(v.c_str(), NULL, 10); memcpy(b + 2 * i, &s, 2); break; }
        case MRI_INT: { int s = (int)strtol(v.c_str(), NULL, 10); memcpy(b + 4 * i, &s, 4); break; }
        case MRI_FLOAT: { uint32_t u = (uint32_t)strtoul(v.c_str(), NULL, 16); memcpy(b + 4 * i, &u, 4); break; }
        default: fail("a voxel type the cases do not write");
      }
    }
  }
  return m;
}
MRI *get_opt() { return geti() ? get_vol() : NULL; }

// a volume with a geometry: get_vol's tokens, then the direction cosines x_r .. z_s and the
// centre c_r c_a c_s (float bits)
MRI *get_volg()
{
  MRI *m = get_vol();
  float *f[12] = {&m->x_r, &m->x_a, &m->x_s, &m->y_r, &m->y_a, &m->y_s,
                  &m->z_r, &m->z_a, &m->z_s, &m->c_r, &m->c_a, &m->c_s};
  for (float *p : f) *p = getf();
  m->ras_good_flag = 1;
  MRIreInitCache(m);
  return m;
}

void put_vol(const MRI *m)
{
  if (!m) { out += "NULL "; return; }
  oi(m->width); oi(m->height); oi(m->depth); oi(m->type); oi(m->nframes);
  od(m->outside_val);
  of(m->xsize);
  size_t n = (size_t)m->width * m->height * m->depth * m->nframes;
  const unsigned char *b = (const unsigned char *)m->chunk;
  std::string prev;
  size_t run = 0;
  auto flush = [&]() {
    if (!run) return;
    out += prev;
    if (run > 1) out += "*" + std::to_string(run);
    out += " ";
  };
  for (size_t i = 0; i < n; i++) {
    char t[32];
    switch (m->type) {
      case MRI_UCHAR: snprintf(t, sizeof t, "%d", b[i]); break;
      case MRI_SHORT: { short s; memcpy(&s, b + 2 * i, 2); snprintf(t, sizeof t, "%d", s); break; }
      case MRI_INT: { int s; memcpy(&s, b + 4 * i, 4); snprintf(t, sizeof t, "%d", s); break; }
      case MRI_FLOAT: { uint32_t u; memcpy(&u, b + 4 * i, 4); snprintf(t, sizeof t, "%08x", u); break; }
      default: fail("a voxel type the cases do not print");
    }
    if (run && prev == t) {
      run++;
    } else {
      flush();
      prev = t;
      run = 1;
    }
  }
  flush();
}

// hex bytes into a file of the work folder
void put_file(const char *name)
{
  const char *h = next();
  FILE *f = fopen(name, "wb");
  if (!f) fail("cannot write a case file");
  for (const char *p = h; p[0] && p[1]; p += 2) {
    unsigned v;
    sscanf(p, "%2x", &v);
    fputc((int)v, f);
  }
  fclose(f);
}

uint64_t fnv(const std::string &s)
{
  uint64_t h = 0xcbf29ce484222325ULL;
  for (unsigned char c : s) h = (h ^ c) * 0x100000001b3ULL;
  return h;
}

std::string slurp(const std::string &path)
{
  std::string r;
  if (FILE *f = fopen(path.c_str(), "rb")) {
    int ch;
    while ((ch = fgetc(f)) != EOF) r += (char)ch;
    fclose(f);
  }
  return r;
}

// the files the case wrote (not the ones it was given: names starting with `in.`), sorted: a
// volume read back and printed, any other file's FNV-1a
void put_written()
{
  std::vector<std::string> names;
  if (DIR *d = opendir(".")) {
    while (struct dirent *e = readdir(d)) {
      std::string n = e->d_name;
      if (n == "." || n == ".." || n.compare(0, 3, "in.") == 0) continue;
      names.push_back(n);
    }
    closedir(d);
  }
  std::sort(names.begin(), names.end());
  for (const std::string &n : names) {
    out += "F " + n + " ";
    if (n.size() > 4 && n.compare(n.size() - 4, 4, ".mgz") == 0) {
      int gd = Gdiag;
      Gdiag = 0;
      MRI *m = MRIread(n.c_str());
      Gdiag = gd;
      put_vol(m);
      if (m) MRIfree(&m);
    } else {
      char b[24];
      snprintf(b, sizeof b, "%016" PRIx64 " ", fnv(slurp(n)));
      out += b;
    }
  }
}

void clear_dir(const std::string &dir)
{
  if (DIR *d = opendir(dir.c_str())) {
    while (struct dirent *e = readdir(d)) {
      std::string n = e->d_name;
      if (n != "." && n != "..") unlink((dir + "/" + n).c_str());
    }
    closedir(d);
  }
}

void run_case()
{
  std::string cmd = next();
  if (cmd == "cbias") {
    // cbias <src> <dst>: compute_bias
    MRI *src = get_vol(), *dst = get_vol();
    put_vol(compute_bias(src, dst, NULL));
  } else if (cmd == "wmo" || cmd == "wmom") {
    // wmo|wmom <src> <ctrl> <intensity_below> <Gdiag>: MRIremoveWMOutliers,
    // MRIremoveWMOutliersAndRetainMedialSurface (the control volume in place, as main calls them)
    MRI *src = get_vol(), *ctrl = get_vol();
    int ib = geti();
    Gdiag = (unsigned long)strtoul(next(), NULL, 10);
    if (cmd == "wmo")
      MRIremoveWMOutliers(src, ctrl, ctrl, ib);
    else
      MRIremoveWMOutliersAndRetainMedialSurface(src, ctrl, ctrl, ib);
    Gdiag = 0;
    put_vol(ctrl);
    put_written();
  } else if (cmd == "snw") {
    // snw <ctrl with geometry> <aseg with geometry>: remove_nonwm_voxels(ctrl, aseg, ctrl)
    MRI *ctrl = get_volg(), *aseg = get_volg();
    remove_nonwm_voxels(ctrl, aseg, ctrl);
    put_vol(ctrl);
  } else if (cmd == "tcp" || cmd == "rcor") {
    // tcp <ctrl> <intensity> <thresh> <Gx Gy Gz>: threshold_control_points;
    // rcor <ctrl> <mri> <fmin> <fmax> <Gx Gy Gz>: remove_controlpoints_outside_range
    MRI *ctrl = get_vol(), *v = get_vol();
    float a = getf(), b = cmd == "rcor" ? getf() : 0;
    Gx = geti(); Gy = geti(); Gz = geti();
    if (cmd == "tcp")
      threshold_control_points(ctrl, v, a);
    else
      remove_controlpoints_outside_range(ctrl, v, a, b);
    put_vol(ctrl);
  } else if (cmd == "t2o") {
    // t2o <ctrl with geometry> <dist with geometry> <src with geometry>: remove_surface_outliers_T2
    MRI *ctrl = get_volg(), *dist = get_volg(), *src = get_volg();
    remove_surface_outliers_T2(ctrl, dist, src, ctrl);
    put_vol(ctrl);
    put_written();
  } else if (cmd == "aip") {
    // aip <ctrl with geometry> <vals> <intensity_above> <intensity_below> <surface hex> <surface
    // hex> <aseg?> <Gdiag>: add_interior_points (the control volume in place)
    MRI *ctrl = get_volg(), *vals = get_vol();
    float ia = getf(), ib = getf();
    put_file("in.lh.s0");
    put_file("in.lh.s1");
    MRI *aseg = get_opt();
    Gdiag = (unsigned long)strtoul(next(), NULL, 10);
    setenv("SURFER_SIDEDOOR", "1", 1);
    MRIS *s0 = MRISread("in.lh.s0"), *s1 = MRISread("in.lh.s1");
    if (!s0 || !s1) { out += "NOSURF "; return; }
    add_interior_points(ctrl, vals, ia, ib, s0, s1, aseg, ctrl);
    Gdiag = 0;
    put_vol(ctrl);
    put_written();
  } else if (cmd == "opt") {
    // opt <label hex | -> <n> <arg>...: main's option loop (get_option) and its argument count
    // check, then the file statics
    std::string lab = next();
    if (lab != "-") {
      pos--;
      put_file("in.l.label");
    }
    int n = geti();
    static std::vector<std::string> keep;
    keep.assign(1, "mri_normalize");
    for (int i = 0; i < n; i++) keep.push_back(next());
    std::vector<char *> av;
    for (std::string &s : keep) av.push_back(&s[0]);
    av.push_back(NULL);
    av.push_back(NULL);
    int argc = (int)keep.size();
    char **argv = av.data();
    mni.max_gradient = MAX_GRADIENT;
    for (; argc > 1 && ISOPTION(*argv[1]); argc--, argv++) {
      int nargs = get_option(argc, argv);
      argc -= nargs;
      argv += nargs;
    }
    if (argc < 3) usage_exit(0);
    oi(argc); os(argv[1]); os(argv[2]);
    oi(contrast); oi(gca != NULL); oi(xform ? xform->type : -1); oi(brain_distance); oi(conform);
    oi(gentle_flag); oi(noskull); oi(nosnr); od(min_dist); os(checknorm_fname); of(check_min);
    of(check_max); os(renorm_fname); oi(renorm_val); of(bias_sigma); os(mask_fname);
    os(interior_fname1); os(interior_fname2); of(mask_sigma); of(mask_thresh);
    os(mask_orig_fname); of(mask_orig_thresh); oi(scan_type); oi(prune); of(mni.max_gradient);
    oi(verbose); oi(num_3d_iter); of(intensity_above); of(intensity_below);
    os(control_point_fname); os(long_control_volume_fname); os(long_bias_volume_fname);
    os(aseg_fname); os(control_volume_fname); os(bias_volume_fname); oi(read_flag);
    oi(long_flag); oi(no1d); oi(file_only); oi(nsurfs);
    for (int i = 0; i < nsurfs; i++) {
      os(surface_fnames[i]);
      os(surface_xform_fnames[i]);
      oi(surface_xforms[i]->type);
    }
    of(grad_thresh); oi(control_point_label ? control_point_label->n_points : -1);
    os(output_control_points_vol); oi(DoGentleCPFile); oi(nonmax_suppress); oi(erode);
    oi(Gx); oi(Gy); oi(Gz); oi(Gvx); oi(Gvy); oi(Gvz);
    // a draw only after -seed (unseeded, FreeSurfer seeds from the clock: FS-04)
    bool seeded = false;
    for (size_t i = 1; i < keep.size(); i++) seeded = seeded || strcasecmp(keep[i].c_str(), "-seed") == 0;
    if (seeded) od(randomNumber(0.0, 1.0));
  } else {
    fail("unknown command");
  }
}

const char *MARK = "\n@@RESULT ";

}  // namespace probe

int main(int argc, char *argv[])
{
  using namespace probe;
  if (argc != 2) {
    fprintf(stderr, "usage: fsnormalize <cases>\n");
    return 2;
  }
  if (argv[1][0] != '/') fail("the cases must be given by an absolute path");
  scratch = std::string(argv[1]) + ".out";
  workdir = std::string(argv[1]) + ".d";
  Progname = "mri_normalize";
  setenv("SURFER_SIDEDOOR", "1", 1);  // the licence key check (none in the replica)
  FILE *fp = fopen(argv[1], "r");
  if (!fp) fail("cannot open the cases");
  std::vector<std::string> lines;
  static char buf[1 << 22];
  while (fgets(buf, sizeof buf, fp)) lines.push_back(buf);
  fclose(fp);
  mkdir(workdir.c_str(), 0700);
  for (const std::string &line : lines) {
    if (line.size() >= sizeof buf - 1) fail("a case longer than the buffer");
    fflush(stdout);
    clear_dir(workdir);
    pid_t pid = fork();
    if (pid < 0) fail("fork");
    if (pid == 0) {
      int fd = open(scratch.c_str(), O_WRONLY | O_CREAT | O_TRUNC, 0600);
      if (fd < 0) _exit(3);
      dup2(fd, 1);
      dup2(fd, 2);
      close(fd);
      setvbuf(stdout, NULL, _IONBF, 0);
      if (chdir(workdir.c_str()) != 0) _exit(4);
      alarm(60);
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
    std::string text = slurp(scratch);
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
    std::string esc;
    char b[8];
    for (unsigned char c : text) {
      if (c < 0x20 || c > 0x7e || c == '\\') {
        snprintf(b, sizeof b, "\\x%02x", c);
        esc += b;
      } else {
        esc += (char)c;
      }
    }
    printf("%s | %s\n", result.c_str(), esc.c_str());
  }
  clear_dir(workdir);
  rmdir(workdir.c_str());
  unlink(scratch.c_str());
  return 0;
}
