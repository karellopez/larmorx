# SPDX-License-Identifier: Apache-2.0
"""The oracle-output cache: every run of an original program, computed once.

The output of an original program for the same build, inputs, arguments and settings never
changes, so the parity suites keep it in a content-addressed cache
(``<workspace>/oracles/cache``, or ``LARMORX_ORACLE_CACHE``) and later runs re-run only larmorx
against it. Runs whose result the original itself varies are never cached (:mod:`.policy`).
The whole design, what is in the key and why, and the commands: ``docs/validation/oracle-cache.md``.

- :func:`run` (and :func:`run_apptainer`) runs an original program, or replays it;
- :class:`Policy` says whether a run may be cached;
- ``LARMORX_ORACLE_CACHE_MODE``: ``use`` (the default), ``refresh`` or ``off``
  (``python -m larmorx_validation parity --oracle-cache``);
- ``python -m larmorx_validation oracle-cache verify --sample N`` re-runs cached entries.
"""

from larmorx_validation.oracle_cache.core import (
    ENV_MODE,
    MODES,
    Completed,
    Tally,
    begin_case,
    end_case,
    mode,
    run,
    run_apptainer,
    using,
)
from larmorx_validation.oracle_cache.policy import (
    CONFIRMATIONS,
    VARIES_ABOVE_ONE_THREAD,
    VARYING_DEFECTS,
    Policy,
    ants_seed_never,
    decide,
    itk_threads,
    never_for,
)
from larmorx_validation.oracle_cache.store import ENV_LOCATION, Store, location

__all__ = [
    "CONFIRMATIONS",
    "ENV_LOCATION",
    "ENV_MODE",
    "MODES",
    "VARIES_ABOVE_ONE_THREAD",
    "VARYING_DEFECTS",
    "Completed",
    "Policy",
    "Store",
    "Tally",
    "ants_seed_never",
    "begin_case",
    "decide",
    "end_case",
    "itk_threads",
    "location",
    "mode",
    "never_for",
    "run",
    "run_apptainer",
    "using",
]
