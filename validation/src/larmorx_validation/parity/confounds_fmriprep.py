# SPDX-License-Identifier: Apache-2.0
"""The parity suite ``confounds-fmriprep``: the confounds' part's check scripts of its kind, run as
cases (:mod:`larmorx_validation.parity.confounds_scripts` says how)."""

from larmorx_validation.parity.confounds_scripts import make_suite
from larmorx_validation.parity.harness import Suite


def suite() -> Suite:
    return make_suite("confounds-fmriprep")
