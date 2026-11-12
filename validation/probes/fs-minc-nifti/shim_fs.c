/* SPDX-License-Identifier: Apache-2.0 */
/* The probe's entry points against FreeSurfer 8.2.0's libminc (packages/minc, as the unit oracle
 * built it), through FreeSurfer's own header (read by the compiler only). */
#include "minc_volume_io.h"
int get_n_concated_transforms(General_transform *);
#include "shim_common.h"
