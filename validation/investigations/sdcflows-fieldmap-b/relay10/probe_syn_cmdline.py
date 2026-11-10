# SPDX-License-Identifier: Apache-2.0
"""The SyN node's command line from larmorx's values against fMRIPrep's ``command.txt``, token by token
(the probe that found the settings file overriding ``transform_parameters``).

    python -P probe_syn_cmdline.py <fmap_preproc_wf/wf_* folder of a SyN run>  (from the repository root)
"""

import json
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, "validation/investigations/sdcflows-fieldmap-b")
import check_syn_preprocessing as csp
import check_workflows as cw

from larmorx import _nipype
from larmorx import sdcflows as sd

wf = Path(sys.argv[1])
n = wf / "syn"
vp_in = wf / "vox_params"
ro = sd.get_readout_time(
    __import__("ast").literal_eval(csp.inputs(wf / "readout_time")["metadata"]),
    csp.hosts(wf / "clip_epi", "in_file"),
)
rd = sd.warp_dir(
    csp.hosts(wf / "warp_dir", "moving_image"),
    csp.hosts(wf / "warp_dir", "fixed_image"),
    ro.pe_direction,
)
cfg = json.loads(sd.registration_settings("sd_syn.json").read_text())
vp = sd.mm2vox(
    csp.hosts(vp_in, "moving_image"), csp.hosts(vp_in, "fixed_image"), ro.pe_direction, cfg
)
reg = _nipype.registration(
    cwd=tempfile.mkdtemp(),
    from_file=sd.registration_settings("sd_syn.json"),
    output_warped_image=False,
    output_inverse_warped_image=False,
    restrict_deformation=rd,
    fixed_image=csp.hosts(n, "fixed_image"),
    fixed_image_masks=csp.hosts(n, "fixed_image_masks"),
    moving_image=csp.hosts(n, "moving_image"),
    moving_image_masks=csp.hosts(n, "moving_image_masks"),
    transform_parameters=vp,
    num_threads=4,
)
ref = (n / "command.txt").read_text().strip()
run = cw.run_root(n)
ref = ref.replace("/work/", str(run / "work") + "/")
a, b = ref.split(), reg.cmdline.split()
for i, (x, y) in enumerate(zip(a, b, strict=False)):
    if x != y:
        print(i, repr(x), repr(y))
print(len(a), len(b))
