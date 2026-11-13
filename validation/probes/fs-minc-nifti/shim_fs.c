/* SPDX-License-Identifier: Apache-2.0 */
/* The probe's entry points against FreeSurfer 8.2.0's libminc (packages/minc, as the unit oracle
 * built it), through FreeSurfer's own header (read by the compiler only). */
#include "minc_volume_io.h"
/* Defined in FreeSurfer's library, not declared by its header. */
int get_n_concated_transforms(General_transform *);
char *expand_filename(const char *);
int compute_transform_inverse(Transform *, Transform *);
#include "shim_common.h"
