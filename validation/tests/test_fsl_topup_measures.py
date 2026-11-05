# SPDX-License-Identifier: Apache-2.0
"""The topup suite's metrics (``larmorx_validation.parity.fsl_topup``) on images NIfTI stores
with fewer than three axes."""

from __future__ import annotations

import numpy as np
from larmorx_validation.parity import fsl_topup as suite


def test_a_one_voxel_image_is_measured_on_the_masks_grid() -> None:
    # A 1 x 1 x 1 image's Jacobian (nibabel's shape (1,), dim[0] = 1) and the mask of the input's
    # mean (1, 1, 1): the same grid, measured (random testing's topup-trial-00027, shrunk)
    mask = np.ones((1, 1, 1), bool)
    jac = np.ones(1)
    got = suite._field_measures("jac_01", jac, jac.copy(), mask, scale=0.01)
    assert got, "no measure"
    assert not any(m.name.endswith("the same grid") for m in got)
    assert all(m.value == 0 for m in got)
    got = suite._intensity_measures("mag", np.full((1, 1), 2.0), np.full((1, 1), 2.0), mask)
    assert got and not any(m.name.endswith("the same grid") for m in got)


def test_images_of_other_grids_still_differ() -> None:
    mask = np.ones((2, 2, 1), bool)
    got = suite._field_measures("jac_01", np.ones(2), np.ones(2), mask)
    assert [m.name for m in got] == ["jac_01: the same grid"]
    assert suite._field_measures("f", np.ones((2, 2)), np.ones((2, 2)), None)[0].value == np.inf


def test_a_nifti2_quaternion_a_float_rounding_above_one_is_read(tmp_path) -> None:
    # FSL writes the float quaternion's doubles: b, c, d whose squares sum to a little more than
    # 1 (topup-trial-04796); nibabel's NIfTI-2 (double) tolerance refuses them, its float one not
    import nibabel as nib

    hdr = nib.Nifti2Header()
    hdr.set_data_shape((2, 2, 2))
    hdr.set_data_dtype(np.float32)
    hdr["qform_code"], hdr["sform_code"] = 1, 0
    s = float(np.nextafter(np.float32(np.sqrt(0.5)), np.float32(1)))
    hdr["quatern_b"], hdr["quatern_c"], hdr["quatern_d"] = 0.0, s, s
    assert s * s * 2 > 1
    path = tmp_path / "q.nii"
    path.write_bytes(hdr.binaryblock + b"\0" * 4 + np.zeros(8, np.float32).tobytes())
    try:
        nib.load(str(path)).get_qform()
        refused = False
    except ValueError:
        refused = True
    assert refused, "nibabel's double tolerance no longer refuses it"
    img = suite._load(path)
    assert img.get_qform().shape == (4, 4)
