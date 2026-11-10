# SPDX-License-Identifier: Apache-2.0
"""The parity suite ``sdcflows-estimation``: the field maps' part B's check scripts of its kind, run as cases
(:mod:`larmorx_validation.parity.sdcflows_scripts` says how)."""

from larmorx_validation.parity.harness import Suite
from larmorx_validation.parity.sdcflows_scripts import make_suite


def suite() -> Suite:
    return make_suite("sdcflows-estimation")
