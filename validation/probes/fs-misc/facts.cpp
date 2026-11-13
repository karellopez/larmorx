// SPDX-License-Identifier: Apache-2.0
// The probe's other originals (README.md): NrrdIO's and Cephes' constants as the unit oracle's
// libnrrdio.a and libcephes.a hold them, FreeSurfer 8.2.0's FNV headers (include/fnvhash.h,
// include/fnv_hash.h) and libtiff's handlers (the unit oracle's libtiff.a, whose tif_error.c,
// tif_warning.c and tif_unix.c are libtiff 3.6.1's).
//   facts consts            the bits, one per line
//   facts fnv  < records    records: u32 length (little-endian) + bytes; prints the two hashes
//   facts tiff < lines      lines: module<TAB>message ("-" for no module); TIFFError then
//                           TIFFWarning of each, "%s" with the message
#include <cstdio>
#include <cstring>
#include <cstdint>
#include <vector>
#include "fnvhash.h"
#include "fnv_hash.h"

extern "C" {
extern const unsigned int airFloatQNaN, airFloatSNaN, airFloatPosInf, airFloatNegInf;
extern double PI;
void TIFFError(const char *, const char *, ...);
void TIFFWarning(const char *, const char *, ...);
}

int main(int argc, char **argv) {
  if (argc < 2) return 2;
  if (!strcmp(argv[1], "consts")) {
    uint64_t pi;
    memcpy(&pi, &PI, 8);
    printf("airFloatQNaN %08x\nairFloatSNaN %08x\nairFloatPosInf %08x\nairFloatNegInf %08x\n",
           airFloatQNaN, airFloatSNaN, airFloatPosInf, airFloatNegInf);
    printf("cephes PI %016llx\n", (unsigned long long)pi);
    return 0;
  }
  if (!strcmp(argv[1], "fnv")) {
    unsigned char len4[4];
    while (fread(len4, 1, 4, stdin) == 4) {
      uint32_t n = len4[0] | (len4[1] << 8) | (len4[2] << 16) | ((uint32_t)len4[3] << 24);
      std::vector<unsigned char> b(n + 1);
      if (fread(b.data(), 1, n, stdin) != n) return 3;
      FnvHash h;
      h.add(b.data(), n);
      unsigned long f = fnv_add(fnv_init(), b.data(), n);
      printf("%lu %lu\n", (unsigned long)h.value, f);
    }
    return 0;
  }
  if (!strcmp(argv[1], "tiff")) {
    char line[8192];
    while (fgets(line, sizeof line, stdin)) {
      size_t n = strlen(line);
      if (n && line[n - 1] == '\n') line[--n] = 0;
      char *tab = strchr(line, '\t');
      if (!tab) continue;
      *tab = 0;
      const char *module = strcmp(line, "-") ? line : NULL;
      TIFFError(module, "%s", tab + 1);
      TIFFWarning(module, "%s", tab + 1);
    }
    return 0;
  }
  return 2;
}
