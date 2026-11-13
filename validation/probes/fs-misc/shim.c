/* SPDX-License-Identifier: Apache-2.0 */
/* The probe's call of miniz's inflate stream API (README.md), compiled twice: against miniz
 * 1.15 r4 as released (C) and against FreeSurfer 8.2.0's copy (C++, as nii_dicom_batch.cpp
 * includes it). MINIZ_FILE names the miniz.c to include. The dictionary is allocated zeroed,
 * so what miniz reads where it never wrote is deterministic (larmorx's reproduce: zeros). The
 * caller's output buffer has 16 bytes of slack past `room` (a write past it, FS-282, lands
 * there). */
#include MINIZ_FILE
#include <stdlib.h>
#include <string.h>

static void *zalloc_zero(void *op, size_t items, size_t size) {
  (void)op;
  return calloc(items, size);
}
static void zfree_plain(void *op, void *p) {
  (void)op;
  free(p);
}

#ifdef __cplusplus
extern "C"
#endif
int probe_inflate(const unsigned char *in, long in_len, int window_bits, int how, long room,
                  long piece, unsigned char *out, long *out_len, unsigned long long *total_in,
                  unsigned long long *adler) {
  mz_stream s;
  long op = 0, ip = 0;
  int ret = -99999;
  memset(&s, 0, sizeof s);
  s.zalloc = zalloc_zero;
  s.zfree = zfree_plain;
  if (mz_inflateInit2(&s, window_bits) != MZ_OK) return -99999;
  if (how == 0) {
    s.next_in = in;
    s.avail_in = (unsigned)in_len;
    s.next_out = out;
    s.avail_out = (unsigned)room;
    ret = mz_inflate(&s, MZ_FINISH);
    op = room - (long)s.avail_out;
  } else if (how == 1) {
    for (;;) {
      long end = op + piece < room ? op + piece : room;
      unsigned ai, ao;
      s.next_in = in + ip;
      s.avail_in = (unsigned)(in_len - ip);
      s.next_out = out + op;
      s.avail_out = (unsigned)(end - op);
      ai = s.avail_in;
      ao = s.avail_out;
      ret = mz_inflate(&s, MZ_SYNC_FLUSH);
      ip += ai - s.avail_in;
      op += ao - s.avail_out;
      if (ret != MZ_OK || (ai == s.avail_in && ao == s.avail_out)) break;
    }
  } else {
    for (;;) {
      int flush;
      unsigned ai, ao;
      s.next_in = in + ip;
      if (ip < in_len) {
        s.avail_in = 1;
        flush = MZ_NO_FLUSH;
      } else {
        s.avail_in = 0;
        flush = MZ_FINISH;
      }
      s.next_out = out + op;
      s.avail_out = (unsigned)(room - op);
      ai = s.avail_in;
      ao = s.avail_out;
      ret = mz_inflate(&s, flush);
      ip += ai - s.avail_in;
      op += ao - s.avail_out;
      if (ret != MZ_OK || flush == MZ_FINISH) break;
    }
  }
  *out_len = op;
  *total_in = s.total_in;
  *adler = s.adler;
  mz_inflateEnd(&s);
  return ret;
}

#ifdef __cplusplus
extern "C"
#endif
int probe_uncompress(const unsigned char *in, long in_len, unsigned char *out, long room,
                     long *out_len) {
  mz_ulong n = (mz_ulong)room;
  int r = mz_uncompress(out, &n, in, (mz_ulong)in_len);
  *out_len = (long)n;
  return r;
}
