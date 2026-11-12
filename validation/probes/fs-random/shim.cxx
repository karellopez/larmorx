// SPDX-License-Identifier: Apache-2.0
// C entry points to ITK 4.13.2's vnl_random (its own vnl_random.cxx, compiled beside this file by
// build.sh with GCC 8.4.0 and FreeSurfer 8.2.0's flags), for the Rust driver of
// validation/probes/fs-random. Written for this probe.
#include "vnl/vnl_random.h"

extern "C" {
void *vr_new(unsigned long seed) { return new vnl_random(seed); }
void vr_free(void *r) { delete static_cast<vnl_random *>(r); }
void vr_reseed(void *r, unsigned long seed) { static_cast<vnl_random *>(r)->reseed(seed); }
void vr_reseed_array(void *r, unsigned long *seed) { static_cast<vnl_random *>(r)->reseed(seed); }
void vr_restart(void *r) { static_cast<vnl_random *>(r)->restart(); }
unsigned long vr_lrand32(void *r) { return static_cast<vnl_random *>(r)->lrand32(); }
int vr_lrand32_range(void *r, int a, int b) { return static_cast<vnl_random *>(r)->lrand32(a, b); }
int vr_lrand32_range_count(void *r, int a, int b, int *count) {
  return static_cast<vnl_random *>(r)->lrand32(a, b, *count);
}
double vr_drand32(void *r, double a, double b) { return static_cast<vnl_random *>(r)->drand32(a, b); }
double vr_drand64(void *r, double a, double b) { return static_cast<vnl_random *>(r)->drand64(a, b); }
double vr_normal(void *r) { return static_cast<vnl_random *>(r)->normal(); }
double vr_normal64(void *r) { return static_cast<vnl_random *>(r)->normal64(); }
// FreeSurfer's use: (float) drand64(0.0, 1.0) (OpenRan1), the cast as GCC compiles it.
float vr_drand64_float(void *r) { return (float)static_cast<vnl_random *>(r)->drand64(0.0, 1.0); }
}
