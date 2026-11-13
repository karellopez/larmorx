/* SPDX-License-Identifier: Apache-2.0 */
/* nifticlib's file names and znzlib for the probe, built twice: against nifticlib 2.0.0 and against
 * FreeSurfer 8.2.0's copy (each with its own headers, read by the compiler only). */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "nifti1_io.h"
#include "znzlib.h"

/* Defined by both libraries, not declared by every header. */
int nifti_fileexists(const char *);
int znzeof(znzFile);
int znzflush(znzFile);

static void flush_all(void) {
    fflush(stdout);
    fflush(stderr);
}

int fm_is_nifti_file(const char *name) {
    int r = is_nifti_file(name);
    flush_all();
    return r;
}

/* hdr|base|ext offset|gz|valid|exists|complete, as text in out; returns its length. */
int fm_nifti_names(const char *name, char *out, int cap) {
    /* one call after the other (an argument list's order is the compiler's) */
    char *hdr = nifti_findhdrname(name);
    char *base = nifti_makebasename(name);
    char *ext = nifti_find_file_extension(name);
    int gz = nifti_is_gzfile(name);
    int valid = nifti_validfilename(name);
    int exists = nifti_fileexists(name);
    int complete = nifti_is_complete_filename(name);
    int n = snprintf(out, cap, "hdr=%s|base=%s|ext=%ld|gz=%d|valid=%d|exists=%d|complete=%d",
                     hdr ? hdr : "(NULL)", base ? base : "(NULL)", ext ? (long) (ext - name) : -1L,
                     gz, valid, exists, complete);
    free(hdr);
    free(base);
    flush_all();
    return n;
}

static void put64(unsigned char *out, int cap, int *len, long long v) {
    if (*len + 8 <= cap) memcpy(out + *len, &v, 8);
    *len += 8;
}

/* Runs the operations ops[0..nops) on znzopen(path, mode, comp): each op a code byte and its
 * arguments (little-endian), each result 8 bytes (and a read's bytes after its count):
 *   'r' u32 size u32 nmemb   znzread       'w' u32 size u32 nmemb, bytes   znzwrite
 *   's' i64 offset u8 whence znzseek       't' znztell   'e' znzeof   'g' znzgetc
 *   'p' u8 c znzputc   'f' znzflush   'R' znzrewind
 * The first result is 1 when the file opened (0: nothing else runs); the last, znzclose's. */
int fm_znz_script(const char *path, const char *mode, int comp, const unsigned char *ops, int nops,
                  unsigned char *out, int cap, int *outlen) {
    znzFile f = znzopen(path, mode, comp);
    int i = 0, len = 0;
    put64(out, cap, &len, znz_isnull(f) ? 0 : 1);
    if (znz_isnull(f)) {
        *outlen = len;
        flush_all();
        return 0;
    }
    while (i < nops) {
        unsigned char op = ops[i++];
        unsigned int a, b;
        long long off;
        switch (op) {
        case 'r': {
            memcpy(&a, ops + i, 4);
            memcpy(&b, ops + i + 4, 4);
            i += 8;
            unsigned char *buf = calloc((size_t) a * b + 1, 1);
            long long got = (long long) znzread(buf, a, b, f);
            put64(out, cap, &len, got);
            if (len + (long long) a * b <= cap) memcpy(out + len, buf, (size_t) a * b);
            len += a * b;
            free(buf);
            break;
        }
        case 'w':
            memcpy(&a, ops + i, 4);
            memcpy(&b, ops + i + 4, 4);
            i += 8;
            put64(out, cap, &len, (long long) znzwrite(ops + i, a, b, f));
            i += a * b;
            break;
        case 's':
            memcpy(&off, ops + i, 8);
            put64(out, cap, &len, (long long) znzseek(f, (long) off, ops[i + 8]));
            i += 9;
            break;
        case 't': put64(out, cap, &len, (long long) znztell(f)); break;
        case 'e': put64(out, cap, &len, znzeof(f) ? 1 : 0); break;
        case 'g': put64(out, cap, &len, znzgetc(f)); break;
        case 'p': put64(out, cap, &len, znzputc(ops[i++], f)); break;
        case 'f': put64(out, cap, &len, znzflush(f)); break;
        case 'R': put64(out, cap, &len, znzrewind(f)); break;
        default: break;
        }
    }
    put64(out, cap, &len, znzclose(f));
    *outlen = len;
    flush_all();
    return 1;
}
