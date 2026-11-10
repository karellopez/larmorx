# SPDX-License-Identifier: Apache-2.0
"""Peak memory of one ReconstructFieldmap node: fMRIPrep's interface or larmorx's, in this
process (run each in its own process: ``python -P mem_recon.py <orig|lx> <node folder>``; the
peak resident size is printed at the end, from ``resource.getrusage``)."""

import ast
import resource
import sys
import tempfile
import warnings
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import check_bold_steps as cb

which, node = sys.argv[1], Path(sys.argv[2])
inp = cb.cw.node_inputs(node)
inputs = {
    "in_coeffs": [cb.host(node, p) for p in ast.literal_eval(inp["in_coeffs"])],
    "target_ref_file": cb.host(node, inp["target_ref_file"]),
    "fmap_ref_file": cb.host(node, inp["fmap_ref_file"]),
    "transforms": [cb.host(node, p) for p in ast.literal_eval(inp["transforms"])],
    "inverse": ast.literal_eval(inp["inverse"]),
}
base = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss
with tempfile.TemporaryDirectory() as tmp:
    if which == "orig":
        cb._recon_orig(inputs, Path(tmp))
    else:
        import larmorx as lx

        with warnings.catch_warnings():
            warnings.simplefilter("ignore")
            lx.transforms.reconstruct_fieldmap_node(**inputs, out_dir=tmp, n_threads=1)
peak = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss
print(f"{which}: peak {peak / 1024:.0f} MiB (after imports {base / 1024:.0f} MiB)")
