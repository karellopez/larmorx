/* SPDX-License-Identifier: Apache-2.0 */
/* Investigation shim (not part of larmorx's library): time() and clock() return constants, so that ITK's
 * Mersenne twister, seeded by default from a hash of time() and clock(), always starts from
 * the same seed. Used to confirm that the run-to-run variation of antsRegistration with
 * -r <displacement field> before a linear stage comes from that clock seed (ANTS-72). */
#include <time.h>

time_t time(time_t *t)
{
  if (t)
    *t = 1700000000;
  return 1700000000;
}

clock_t clock(void)
{
  return 12345;
}
