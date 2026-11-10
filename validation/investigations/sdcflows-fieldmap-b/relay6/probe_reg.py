# SPDX-License-Identifier: Apache-2.0
"""larmorx's antsRegistration vs ANTs 2.6.5 (one thread, CORE-MATH) on coeff2epi's call."""

import os
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import check_bold_fmap as cb
from larmorx_validation.parity.ants_programs import _run_ants

from larmorx import _nipype
from larmorx.sdcflows import binary_dilation, registration_settings

wf = cb.fmapreg_nodes(["ds002422"])[0]
inp = cb.coeff2epi_inputs(wf)
tmp = Path(tempfile.mkdtemp(dir=sys.argv[2]))
_, dt = binary_dilation(inp["target_mask"], 5, out_dir=tmp / "dt")
_, df = binary_dilation(inp["fmap_mask"], 5, out_dir=tmp / "df")
for mode, threads in (("reproduce", 1), ("reproduce", 4), ("fix", 1)):
    d = tmp / f"lx-{mode}-{threads}"
    cmd = _nipype.registration(
        cwd=d,
        from_file=registration_settings("fmap-any_registration.json"),
        moving_image=[inp["target_ref"]],
        fixed_image=[inp["fmap_ref"]],
        moving_image_masks=[dt],
        fixed_image_masks=[df],
        output_warped_image=False,
        output_inverse_warped_image=False,
    )
    _nipype.run(cmd, n_threads=threads, upstream_defects=mode)
    print(mode, threads, (d / "transform0GenericAffine.mat").read_bytes().hex())
for name, rounded in (("ants", True), ("ants-glibc", False)):
    o = tmp / name
    o.mkdir()
    cwd = os.getcwd()
    os.chdir(o)
    try:
        code, out, err = _run_ants(
            "antsRegistration", cmd.argv(), threads=1, correctly_rounded=rounded, never="probe"
        )
    finally:
        os.chdir(cwd)
    print(name, code, (o / "transform0GenericAffine.mat").read_bytes().hex(), err[-300:])
