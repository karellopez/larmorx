/* SPDX-License-Identifier: Apache-2.0 */
/* The probe's platform header for CTN 3.0.6 on Linux x86-64 (the copy in CTN's tree is its
 * Windows NT one, which turns on its thread layer): a little-endian machine, no threads, as
 * FreeSurfer 8.2.0 builds its copy (-DLITTLE_ENDIAN_ARCHITECTURE -DLinux, no CTN_USE_THREADS). */
#ifndef DICOMPLATFORM_IS_IN
#define DICOMPLATFORM_IS_IN 1
#define LONGSIZE 64
#define INTSIZE 32
#define SHORTSIZE 16
#ifndef LITTLE_ENDIAN_ARCHITECTURE
#define LITTLE_ENDIAN_ARCHITECTURE
#endif
#endif
