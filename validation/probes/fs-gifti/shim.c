/* SPDX-License-Identifier: Apache-2.0 */
/* The probe's entry points over gifticlib's and expat's public API, compiled twice: against
 * gifticlib 1.12, expat 2.0.1 and nifticlib 2.0.0 (libup.so) and against FreeSurfer 8.2.0's copies
 * (libfs.so). The driver calls them in a forked child (an original may crash or never return), so
 * the libraries' global state is fresh for every case. Every entry point writes a dump of typed
 * records (the formats are src/dump.rs's) and flushes the C streams before it returns. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "gifti_io.h"

typedef struct {
    unsigned char *p;
    long len, cap;
} gb;

static void put(gb *b, const void *v, long n) {
    if (b->len + n <= b->cap) memcpy(b->p + b->len, v, n);
    b->len += n;
}
static void put_i(gb *b, int v) { put(b, &v, 4); }
static void put_l(gb *b, long long v) { put(b, &v, 8); }
static void put_c(gb *b, char c) { put(b, &c, 1); }
/* a string: its length (-1 for NULL), then its bytes */
static void put_s(gb *b, const char *s) {
    if (!s) {
        put_i(b, -1);
        return;
    }
    put_i(b, (int) strlen(s));
    put(b, s, (long) strlen(s));
}
static void put_n(gb *b, const char *s, int n) {
    put_i(b, n);
    put(b, s, n);
}

/* ---------------------------------------------------------------- the parser's events */

static XML_Parser g_parser;

static void put_model(gb *b, XML_Content *m) {
    unsigned i;
    put_i(b, (int) m->type);
    put_i(b, (int) m->quant);
    put_s(b, m->name);
    put_i(b, (int) m->numchildren);
    for (i = 0; i < m->numchildren; i++) put_model(b, &m->children[i]);
}

static void XMLCALL x_start(void *u, const char *name, const char **atts) {
    gb *b = (gb *) u;
    int n = 0;
    while (atts[2 * n]) n++;
    put_c(b, 'S');
    put_s(b, name);
    put_i(b, n);
    for (int i = 0; i < 2 * n; i++) put_s(b, atts[i]);
}
static void XMLCALL x_end(void *u, const char *name) {
    put_c((gb *) u, 'E');
    put_s((gb *) u, name);
}
static void XMLCALL x_char(void *u, const char *s, int len) {
    put_c((gb *) u, 'C');
    put_n((gb *) u, s, len);
}
static void XMLCALL x_pi(void *u, const char *target, const char *data) {
    put_c((gb *) u, 'P');
    put_s((gb *) u, target);
    put_s((gb *) u, data);
}
static void XMLCALL x_comment(void *u, const char *s) {
    put_c((gb *) u, 'M');
    put_s((gb *) u, s);
}
static void XMLCALL x_cdata_start(void *u) { put_c((gb *) u, 'A'); }
static void XMLCALL x_cdata_end(void *u) { put_c((gb *) u, 'B'); }
static void XMLCALL x_default(void *u, const char *s, int len) {
    put_c((gb *) u, 'D');
    put_n((gb *) u, s, len);
}
static void XMLCALL x_xmldecl(void *u, const char *ver, const char *enc, int standalone) {
    put_c((gb *) u, 'X');
    put_s((gb *) u, ver);
    put_s((gb *) u, enc);
    put_i((gb *) u, standalone);
}
static void XMLCALL x_start_doctype(void *u, const char *name, const char *sysid,
                                    const char *pubid, int has_subset) {
    put_c((gb *) u, 'T');
    put_s((gb *) u, name);
    put_s((gb *) u, sysid);
    put_s((gb *) u, pubid);
    put_i((gb *) u, has_subset);
}
static void XMLCALL x_end_doctype(void *u) { put_c((gb *) u, 'U'); }
static void XMLCALL x_elem_decl(void *u, const char *name, XML_Content *model) {
    put_c((gb *) u, 'L');
    put_s((gb *) u, name);
    put_model((gb *) u, model);
    XML_FreeContentModel(g_parser, model);
}

/* Parses data in pieces of the given sizes (the last call final), with gifticlib's twelve
 * handlers; an error ends the dump with 'R', the code and XML_GetCurrentLineNumber, as gifticlib
 * prints them; the end of every piece is 'K'. */
long gp_xml(const char *data, long len, const int *pieces, int npieces, unsigned char *out,
            long cap) {
    gb b = {out, 0, cap};
    long off = 0;
    int i;
    (void) len;
    g_parser = XML_ParserCreate(NULL);
    XML_SetUserData(g_parser, &b);
    XML_SetStartElementHandler(g_parser, x_start);
    XML_SetEndElementHandler(g_parser, x_end);
    XML_SetCharacterDataHandler(g_parser, x_char);
    XML_SetProcessingInstructionHandler(g_parser, x_pi);
    XML_SetCommentHandler(g_parser, x_comment);
    XML_SetStartCdataSectionHandler(g_parser, x_cdata_start);
    XML_SetEndCdataSectionHandler(g_parser, x_cdata_end);
    XML_SetDefaultHandler(g_parser, x_default);
    XML_SetXmlDeclHandler(g_parser, x_xmldecl);
    XML_SetStartDoctypeDeclHandler(g_parser, x_start_doctype);
    XML_SetEndDoctypeDeclHandler(g_parser, x_end_doctype);
    XML_SetElementDeclHandler(g_parser, x_elem_decl);
    for (i = 0; i < npieces; i++) {
        if (XML_Parse(g_parser, data + off, pieces[i], i == npieces - 1) == XML_STATUS_ERROR) {
            put_c(&b, 'R');
            put_i(&b, (int) XML_GetErrorCode(g_parser));
            put_l(&b, (long long) XML_GetCurrentLineNumber(g_parser));
            break;
        }
        off += pieces[i];
        put_c(&b, 'K');
    }
    XML_ParserFree(g_parser);
    fflush(stdout);
    fflush(stderr);
    return b.len;
}

/* ---------------------------------------------------------------- gifticlib */

static void put_nvp(gb *b, const nvpairs *p) {
    int i;
    put_i(b, p->length);
    for (i = 0; i < p->length; i++) {
        put_s(b, p->name ? p->name[i] : NULL);
        put_s(b, p->value ? p->value[i] : NULL);
    }
}

static void put_gim(gb *b, gifti_image *g) {
    int i, c, k;
    put_c(b, 'G');
    put_i(b, g->numDA);
    put_s(b, g->version);
    put_nvp(b, &g->meta);
    put_i(b, g->labeltable.length);
    for (i = 0; i < g->labeltable.length; i++) {
        put_i(b, g->labeltable.key ? g->labeltable.key[i] : 0);
        put_s(b, g->labeltable.label ? g->labeltable.label[i] : NULL);
    }
    put_i(b, g->labeltable.rgba != NULL);
    if (g->labeltable.rgba) put(b, g->labeltable.rgba, 16L * g->labeltable.length);
    for (i = 0; i < g->numDA; i++) {
        giiDataArray *da = g->darray ? g->darray[i] : NULL;
        put_i(b, da != NULL);
        if (!da) continue;
        put_i(b, da->intent);
        put_i(b, da->datatype);
        put_i(b, da->ind_ord);
        put_i(b, da->num_dim);
        for (k = 0; k < 6; k++) put_i(b, da->dims[k]);
        put_i(b, da->encoding);
        put_i(b, da->endian);
        put_s(b, da->ext_fname);
        put_l(b, da->ext_offset);
        put_nvp(b, &da->meta);
        put_i(b, da->numCS);
        for (c = 0; c < da->numCS; c++) {
            giiCoordSystem *cs = da->coordsys ? da->coordsys[c] : NULL;
            put_i(b, cs != NULL);
            if (!cs) continue;
            put_s(b, cs->dataspace);
            put_s(b, cs->xformspace);
            put(b, cs->xform, sizeof cs->xform);
        }
        put_l(b, da->nvals);
        put_i(b, da->nbyper);
        put_i(b, da->data != NULL);
        if (da->data) {
            long long n = da->nvals * da->nbyper;
            put_l(b, n);
            put(b, da->data, (long) n);
        }
        put_nvp(b, &da->ex_atrs);
    }
    put_i(b, g->swapped);
    put_i(b, g->compressed);
    put_nvp(b, &g->ex_atrs);
}

/* set: verb, b64_check, xml_buf_size, update_ok, zlevel, indent.
 * Reads each of the nfiles names (NUL-separated in names) in turn (dalen > 0: gifti_read_da_list),
 * then for each image: its dump, gifti_valid_gifti_image(gim, 1), and (disp) its display; with a
 * write name, the image is written (wenc > 0: every DataArray's encoding set to wenc first; wext:
 * the external file name for ExternalFileBinary) and the status recorded; 'N' for no image. */
long gp_read(const char *names, int nfiles, int read_data, const int *set, const int *dalist,
             int dalen, int disp, const char *wname, int wdata, int wenc, const char *wext,
             unsigned char *out, long cap) {
    gb b = {out, 0, cap};
    int f, i;
    gifti_set_verb(set[0]);
    gifti_set_b64_check(set[1]);
    gifti_set_xml_buf_size(set[2]);
    gifti_set_update_ok(set[3]);
    gifti_set_zlevel(set[4]);
    gifti_set_indent(set[5]);
    for (f = 0; f < nfiles; f++) {
        gifti_image *gim;
        if (dalen > 0)
            gim = gifti_read_da_list(names, read_data, dalist, dalen);
        else
            gim = gifti_read_image(names, read_data);
        fflush(stderr);
        if (!gim) {
            put_c(&b, 'N');
        } else {
            put_gim(&b, gim);
            put_i(&b, gifti_valid_gifti_image(gim, 1));
            if (disp) gifti_disp_gifti_image("probe", gim, 1);
            if (wname && f == nfiles - 1) {
                if (wenc > 0)
                    for (i = 0; i < gim->numDA; i++)
                        if (gim->darray[i]) {
                            gim->darray[i]->encoding = wenc;
                            if (wenc == GIFTI_ENCODING_EXTBIN && wext) {
                                if (gim->darray[i]->ext_fname) free(gim->darray[i]->ext_fname);
                                gim->darray[i]->ext_fname = gifti_strdup(wext);
                                gim->darray[i]->ext_offset = 0;
                            }
                        }
                put_c(&b, 'W');
                put_i(&b, gifti_write_image(gim, wname, wdata));
            }
            gifti_free_image(gim);
        }
        fflush(stdout);
        fflush(stderr);
        names += strlen(names) + 1;
    }
    fflush(stdout);
    fflush(stderr);
    return b.len;
}

/* sscanf(s, "%f", &f): 1 and the float's bits, or 0 (FreeSurfer's version check reads with it). */
int gp_scanf_f32(const char *s, unsigned int *bits) {
    float v = 0.0f;
    int n = sscanf(s, "%f", &v);
    memcpy(bits, &v, 4);
    return n;
}
