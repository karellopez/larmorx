# SPDX-License-Identifier: Apache-2.0
"""Clean-room MRI tools: ``lx.mri``.

Named by function rather than by the tool they are compatible with (PLAN.md §3.3). They were
written from behaviour specs, papers, documentation and black-box runs of the original
programs, without their source code, and are Apache-2.0. The same tools run from the command
line as ``larmorx mri <tool> ...``.

- :func:`hmc`: head-motion correction, compatible with FSL's ``mcflirt`` (its options, its
  ``.mat``/``.par``/``.rms`` files and its numbers).
"""

from larmorx.mri.hmc import COSTS, INTERPOLATIONS, HmcResult, hmc, read_mats

__all__ = ["COSTS", "INTERPOLATIONS", "HmcResult", "hmc", "read_mats"]
