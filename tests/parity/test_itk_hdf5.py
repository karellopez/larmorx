# SPDX-License-Identifier: Apache-2.0
"""ITK's HDF5 transform files: larmorx's reader and writer against ITK 5.4.5 (the ANTs 2.6.5
oracle), h5py and nitransforms (see validation/). Skipped where the oracle is not built."""

import os

import pytest

pytest.importorskip("larmorx_testdata")
pytest.importorskip("h5py")
pytest.importorskip("nitransforms")
parity = pytest.importorskip("larmorx_validation.parity")
from larmorx_validation.parity import ants_programs, itk_hdf5  # noqa: E402

if ants_programs.ants_bin() is None:
    pytest.skip("the ANTs 2.6.5 oracle is not available", allow_module_level=True)

TIER = os.environ.get("LARMORX_PARITY_TIER", "smoke")
SUITE = itk_hdf5.suite()


@pytest.mark.parametrize("case", SUITE.cases(TIER), ids=lambda c: c.id)
def test_itk_hdf5_parity(case):
    result = parity.run_one(SUITE, case)
    problems = [f"{c.name}: {c.detail}" for c in result.failed_checks]
    assert result.status in parity.OK_STATUSES, "\n".join([result.reason, *problems])
