/* SPDX-License-Identifier: Apache-2.0 */
/* The probe's entry point over CTN's DICOM object library, compiled twice: against CTN 3.0.6 built
 * from its pinned source (libup.so) and against FreeSurfer 8.2.0's copy as the unit oracle built it
 * (libfs.so). The driver calls it in a forked child (an original may crash), so the library's
 * global state (the condition stack) is fresh for every case. It runs a script of operations on one
 * object handle (open with options, close, the condition stack's snapshot, pop, dump, top) and
 * writes typed records (src/dump.rs reads the same format): the objects are walked through the
 * library's private structures (dcmprivate.h), every field the reader sets, every element's bytes. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "dicom.h"
#include "condition.h"
#include "lst.h"
#include "dicom_objects.h"
#include "dcmprivate.h"

typedef struct {
    unsigned char *p;
    long len, cap;
} gb;

static void put(gb *b, const void *v, long n) {
    if (b->len + n <= b->cap) memcpy(b->p + b->len, v, n);
    b->len += n;
}
static void put_u32(gb *b, unsigned v) { put(b, &v, 4); }
static void put_u64(gb *b, unsigned long long v) { put(b, &v, 8); }
static void put_c(gb *b, char c) { put(b, &c, 1); }
static void put_s(gb *b, const char *s) {
    put_u32(b, (unsigned) strlen(s));
    put(b, s, (long) strlen(s));
}

static void dump_object(gb *b, PRIVATE_OBJECT *o);

static void dump_element(gb *b, PRV_ELEMENT_ITEM *it) {
    DCM_ELEMENT *e = &it->element;
    put_c(b, 'E');
    put_u32(b, e->tag);
    put_u32(b, (unsigned) e->representation);
    put_u64(b, (unsigned long long) e->multiplicity);
    put_u32(b, e->length);
    put_s(b, e->description);
    put_u32(b, (unsigned) it->byteOrder);
    put_u64(b, (unsigned long long) it->dataOffset);
    put_u64(b, (unsigned long long) it->currentOffset);
    put_u64(b, (unsigned long long) it->allocatedDataLength);
    put_u64(b, (unsigned long long) it->originalDataLength);
    put_u64(b, (unsigned long long) it->paddedDataLength);
    put_u32(b, (unsigned) it->fragmentFlag);
    if (e->representation == DCM_SQ) {
        LST_HEAD *l = e->d.sq;
        DCM_SEQUENCE_ITEM *s;
        put_c(b, 'Q');
        if (l == NULL) {
            put_u32(b, 0xffffffffu);
            return;
        }
        put_u32(b, (unsigned) LST_Count(&l));
        s = (DCM_SEQUENCE_ITEM *) LST_Head(&l);
        if (s) (void) LST_Position(&l, s);
        while (s != NULL) {
            dump_object(b, (PRIVATE_OBJECT *) s->object);
            s = (DCM_SEQUENCE_ITEM *) LST_Next(&l);
        }
    } else if (e->d.ot == NULL) {
        put_c(b, 'Z');
    } else if (e->length == DCM_UNSPECIFIEDLENGTH) {
        put_c(b, 'U');
    } else {
        put_c(b, 'D');
        put_u32(b, e->length);
        put(b, e->d.ot, (long) e->length);
    }
}

static void dump_object(gb *b, PRIVATE_OBJECT *o) {
    PRV_GROUP_ITEM *g;
    put_c(b, 'J');
    if (o == NULL) {
        put_c(b, '0');
        return;
    }
    put_c(b, '1');
    put_s(b, o->keyType);
    put_u32(b, (unsigned) o->objectType);
    put_u32(b, (unsigned) o->accessMethod);
    put_u32(b, (unsigned) o->deleteFlag);
    put_u32(b, (unsigned) o->groupLengthFlag);
    put_u64(b, (unsigned long long) o->objectSize);
    put_u64(b, (unsigned long long) o->offset);
    put_u64(b, (unsigned long long) o->pixelSize);
    put_u64(b, (unsigned long long) o->pixelOffset);
    put_u32(b, o->pixelBitsAllocated);
    put_u32(b, o->pixelRepresentation);
    put_u32(b, o->fd >= 0 ? 1u : 0u);
    put_s(b, o->fileName);
    put_u32(b, (unsigned) o->preambleFlag);
    put(b, o->preamble, DCM_PREAMBLELENGTH);
    put_u64(b, (unsigned long long) o->dataOptions);
    put_u64(b, (unsigned long long) o->metaHeaderLength);
    put_u32(b, (unsigned) o->longVRAttributes);
    put_s(b, o->waveformDataVR);
    put_u32(b, (unsigned) LST_Count(&o->groupList));
    g = (PRV_GROUP_ITEM *) LST_Head(&o->groupList);
    if (g) (void) LST_Position(&o->groupList, g);
    while (g != NULL) {
        PRV_ELEMENT_ITEM *it;
        put_c(b, 'G');
        put_u32(b, g->group);
        put_u64(b, (unsigned long long) g->baseLength);
        put_u32(b, (unsigned) g->longVRAttributes);
        put_u32(b, (unsigned) LST_Count(&g->elementList));
        it = (PRV_ELEMENT_ITEM *) LST_Head(&g->elementList);
        if (it) (void) LST_Position(&g->elementList, it);
        while (it != NULL) {
            dump_element(b, it);
            it = (PRV_ELEMENT_ITEM *) LST_Next(&g->elementList);
        }
        g = (PRV_GROUP_ITEM *) LST_Next(&o->groupList);
    }
}

/* the condition stack, bottom first (as COND_DumpConditions prints it) */
static unsigned long st_code[128];
static char st_text[128][260];
static int st_n;
static CTNBOOLEAN collect(CONDITION c, const char *t) {
    if (st_n < 128) {
        st_code[st_n] = c;
        strncpy(st_text[st_n], t, 259);
        st_text[st_n][259] = '\0';
        st_n++;
    }
    return 1;
}
static void snapshot(gb *b) {
    int i;
    st_n = 0;
    (void) COND_ExtractConditions(collect);
    put_c(b, 'S');
    put_u32(b, (unsigned) st_n);
    for (i = st_n - 1; i >= 0; i--) {
        put_u64(b, (unsigned long long) st_code[i]);
        put_s(b, st_text[i]);
    }
}

/* Runs `ops` (nops pairs: code, argument) on one object handle; returns the dump's length. Codes:
 * 1 DCM_OpenFile(name, arg) and the object; 2 DCM_CloseObject and the handle's state after it;
 * 3 the stack's snapshot; 4 COND_PopCondition(arg); 5 COND_DumpConditions (standard error);
 * 6 COND_TopCondition with a buffer of arg bytes. */
long fdp_run(const char *name, const long *ops, int nops, unsigned char *out, long cap) {
    gb b = {out, 0, cap};
    DCM_OBJECT *object = NULL;
    int i;
    for (i = 0; i < nops; i++) {
        long op = ops[2 * i], arg = ops[2 * i + 1];
        CONDITION c;
        switch (op) {
        case 1:
            c = DCM_OpenFile(name, (unsigned long) arg, &object);
            fflush(stdout);
            fflush(stderr);
            put_c(&b, 'O');
            put_u64(&b, (unsigned long long) c);
            dump_object(&b, (PRIVATE_OBJECT *) object);
            break;
        case 2:
            c = DCM_CloseObject(&object);
            put_c(&b, 'X');
            put_u64(&b, (unsigned long long) c);
            put_c(&b, object == NULL ? '0' : '1');
            break;
        case 3:
            snapshot(&b);
            break;
        case 4:
            c = COND_PopCondition((CTNBOOLEAN) arg);
            put_c(&b, 'P');
            put_u64(&b, (unsigned long long) c);
            break;
        case 5:
            COND_DumpConditions();
            fflush(stderr);
            put_c(&b, 'D');
            break;
        case 6: {
            char text[1024];
            CONDITION code = 0;
            if (arg < 1 || arg > 1024) arg = 1024;
            c = COND_TopCondition(&code, text, (unsigned long) arg);
            put_c(&b, 'T');
            put_u64(&b, (unsigned long long) c);
            put_u64(&b, (unsigned long long) code);
            put_s(&b, text);
            break;
        }
        default:
            break;
        }
    }
    fflush(stdout);
    fflush(stderr);
    return b.len;
}

/* the size of an element item (the slack of its allocation decides whether a 4-byte write into a
 * short group length stays inside it) */
long fdp_sizes(void) { return (long) sizeof(PRV_ELEMENT_ITEM); }
