/* SPDX-License-Identifier: Apache-2.0 */
/* expat 2.0.1's configuration for the probe's upstream build: the three settings FreeSurfer 8.2.0's
 * copy of expat is built with (its packages/expat/expat_config.h), and the C library's functions. */
#define XML_CONTEXT_BYTES 1024
#define XML_DTD 1
#define XML_NS 1
#define HAVE_MEMMOVE 1
#define HAVE_BCOPY 1
#define HAVE_UNISTD_H 1
#define HAVE_STDLIB_H 1
#define HAVE_STRING_H 1
#define BYTEORDER 1234
