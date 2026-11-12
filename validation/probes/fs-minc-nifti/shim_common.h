/* SPDX-License-Identifier: Apache-2.0 */
/* The probe's entry points over volume_io's API (included after the library's own header: FreeSurfer
 * 8.2.0's packages/minc for shim_fs.c, MINC 1.5.1's volume_io for shim_up.c). Every function flushes
 * the C streams before it returns, so the driver reads the messages of that call alone. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef struct {
    unsigned char *p;
    int len, cap;
} fm_buf;

static void put(fm_buf *b, const void *v, int n) {
    if (b->len + n <= b->cap) memcpy(b->p + b->len, v, n);
    b->len += n;
}
static void put_i(fm_buf *b, int v) { put(b, &v, 4); }
static void put_d(fm_buf *b, double v) { put(b, &v, 8); }
static void put_t(fm_buf *b, Transform *t) {
    int i, j;
    for (i = 0; i < 4; i++)
        for (j = 0; j < 4; j++) put_d(b, Transform_elem(*t, i, j));
}

/* type, inverse flag, then the data: LINEAR: the stored forward and inverse matrices (row by row);
 * THIN_PLATE_SPLINE: n_dimensions, n_points, the points, the displacements; CONCATENATED: n, each. */
static void dump(fm_buf *b, General_transform *t) {
    int i, j;
    put_i(b, (int) t->type);
    put_i(b, (int) t->inverse_flag);
    switch (t->type) {
    case LINEAR:
        put_t(b, t->linear_transform);
        put_t(b, t->inverse_linear_transform);
        break;
#ifdef UPSTREAM
    case THIN_PLATE_SPLINE:
        put_i(b, t->n_dimensions);
        put_i(b, t->n_points);
        for (i = 0; i < t->n_points; i++)
            for (j = 0; j < t->n_dimensions; j++) put_d(b, t->points[i][j]);
        for (i = 0; i < t->n_points + t->n_dimensions + 1; i++)
            for (j = 0; j < t->n_dimensions; j++) put_d(b, t->displacements[i][j]);
        break;
#endif
    case CONCATENATED_TRANSFORM:
        put_i(b, t->n_transforms);
        for (i = 0; i < t->n_transforms; i++) dump(b, &t->transforms[i]);
        break;
    default:
        break;
    }
}

static void flush_all(void) {
    fflush(stdout);
    fflush(stderr);
}

/* input_transform_file(name); on success the transform's dump in out (its length in *len). */
int fm_input_transform_file(const char *name, unsigned char *out, int cap, int *len) {
    General_transform t;
    fm_buf b = {out, 0, cap};
    int status;
    memset(&t, 0, sizeof t);
    status = (int) input_transform_file((char *) name, &t);
    if (status == 0) dump(&b, &t);
    *len = b.len;
    flush_all();
    return status;
}

/* Reads two files, concat_general_transforms(a, b) (each inverted first when asked), the dump. */
int fm_concat_files(const char *a, const char *b, int invert_a, int invert_b, unsigned char *out,
                    int cap, int *len) {
    General_transform ta, tb, tc;
    fm_buf buf = {out, 0, cap};
    memset(&ta, 0, sizeof ta);
    memset(&tb, 0, sizeof tb);
    memset(&tc, 0, sizeof tc);
    if (input_transform_file((char *) a, &ta) != 0 || input_transform_file((char *) b, &tb) != 0) {
        flush_all();
        return 1;
    }
    if (invert_a) ta.inverse_flag = !ta.inverse_flag;
    if (invert_b) tb.inverse_flag = !tb.inverse_flag;
    concat_general_transforms(&ta, &tb, &tc);
    dump(&buf, &tc);
    *len = buf.len;
    flush_all();
    return 0;
}

/* compute_transform_inverse on a 4 x 4 (row by row); returns its BOOLEAN. */
int fm_compute_transform_inverse(const double *m, double *inv) {
    Transform t, ti;
    int i, j, ok;
    for (i = 0; i < 4; i++)
        for (j = 0; j < 4; j++) Transform_elem(t, i, j) = m[4 * i + j];
    ok = (int) compute_transform_inverse(&t, &ti);
    for (i = 0; i < 4; i++)
        for (j = 0; j < 4; j++) inv[4 * i + j] = Transform_elem(ti, i, j);
    flush_all();
    return ok;
}

void fm_concat_transforms(const double *a, const double *b, double *r) {
    Transform ta, tb, tr;
    int i, j;
    for (i = 0; i < 4; i++)
        for (j = 0; j < 4; j++) {
            Transform_elem(ta, i, j) = a[4 * i + j];
            Transform_elem(tb, i, j) = b[4 * i + j];
        }
    concat_transforms(&tr, &ta, &tb);
    for (i = 0; i < 4; i++)
        for (j = 0; j < 4; j++) r[4 * i + j] = Transform_elem(tr, i, j);
}

void fm_transform_point(const double *a, double x, double y, double z, double *out) {
    Transform ta;
    int i, j;
    for (i = 0; i < 4; i++)
        for (j = 0; j < 4; j++) Transform_elem(ta, i, j) = a[4 * i + j];
    transform_point(&ta, x, y, z, &out[0], &out[1], &out[2]);
}

/* expand_filename into out (NUL-terminated); returns its length (-1: it returned NULL). */
int fm_expand_filename(const char *name, char *out, int cap) {
    char *e = expand_filename((char *) name);
    int n;
    if (e == NULL) {
        flush_all();
        return -1;
    }
    n = (int) strlen(e);
    if (n < cap) memcpy(out, e, n + 1);
    flush_all();
    return n;
}

#ifdef UPSTREAM
/* input_transform_file(in), then output_transform_file(out, comments); returns the status. */
int fm_rewrite(const char *in, const char *out, const char *comments) {
    General_transform t;
    int status;
    memset(&t, 0, sizeof t);
    status = (int) input_transform_file((char *) in, &t);
    if (status == 0) status = (int) output_transform_file((char *) out, (char *) comments, &t);
    flush_all();
    return status;
}

/* general_transform_point (inverse: general_inverse_transform_point) of a file's transform. */
int fm_general_point(const char *in, int inverse, double x, double y, double z, double *out) {
    General_transform t;
    memset(&t, 0, sizeof t);
    if (input_transform_file((char *) in, &t) != 0) {
        flush_all();
        return 1;
    }
    if (inverse)
        general_inverse_transform_point(&t, x, y, z, &out[0], &out[1], &out[2]);
    else
        general_transform_point(&t, x, y, z, &out[0], &out[1], &out[2]);
    flush_all();
    return 0;
}
#endif
