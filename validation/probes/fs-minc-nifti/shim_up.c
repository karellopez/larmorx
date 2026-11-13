/* SPDX-License-Identifier: Apache-2.0 */
/* The probe's entry points against MINC 1.5.1's own volume_io (the release tarball's files, built
 * by build.sh with GCC 8.4.0 and FreeSurfer's flags). */
#define UPSTREAM 1
#include <internal_volume_io.h>
#include "shim_common.h"
