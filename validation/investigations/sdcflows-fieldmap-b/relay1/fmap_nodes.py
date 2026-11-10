# SPDX-License-Identifier: Apache-2.0
# ruff: noqa: F401, F841, B007, SIM115 (relay 1's probes, kept as they ran)
"""Inventory of the field-map nodes fMRIPrep 25.2.6 ran in every oracle run (part B of the field maps).

Reads each node's ``_report/report.rst`` (nipype's own text report: no pickles) under
``fmap_preproc_wf`` and the BOLD workflows' field-map nodes, and prints per run: the estimator
workflows, every node with its interface, duration and (for selected nodes) key inputs.

    python -I fmap_nodes.py [run-substring ...] [--inputs]
"""

import os
import re
import sys
from collections import defaultdict

ROOT = os.path.join(os.environ["LARMORX_WORKSPACE"], "oracles/fmriprep-25.2.6")
BOLD_FMAP_NODES = (
    "boldref_fmap",
    "fmap_select",
    "fmapref_buffer",
    "fmapreg_wf",
    "ds_fmapreg_wf",
    "distortion_params",
    "unwarp_boldref",
    "fmap_recon",
    "itk_mat2txt",
    "enhance_boldref_wf",
    "skullstrip_bold_wf",
    "bold_native_wf",
    "boldref_fmap",
)
KEYS = (
    "in_file",
    "in_files",
    "in_data",
    "metadata",
    "pe_dir",
    "pe_dirs_fsl",
    "readout_times",
    "config",
    "encoding_direction",
    "reference",
    "target_orientation",
    "max_vols_per_pe",
    "mc_method",
    "t_mask",
    "num_threads",
    "affine_tolerance",
    "zero_based_avg",
    "hmc",
    "to_ras",
    "p_min",
    "p_max",
    "dimension",
    "n_iterations",
    "shrink_factor",
    "bspline_fitting_distance",
    "convergence_threshold",
    "fallback",
    "use_estimate",
    "in_coeffs",
    "target_ref_file",
    "fmap_ref_file",
    "transforms",
    "inverse",
    "radius",
    "from_file",
    "fixed_image",
    "moving_image",
    "fixed_image_masks",
    "moving_image_masks",
    "jacobian",
    "ro_time",
    "num_threads",
    "ref_file",
    "fieldmap",
    "nthreads",
    "axis",
    "units",
    "in_coeff",
    "fmap_ref",
    "args",
)

want_inputs = "--inputs" in sys.argv
runs = [a for a in sys.argv[1:] if not a.startswith("--")]


def parse_report(path):
    text = open(path, encoding="utf-8", errors="replace").read()
    m = re.search(r"Node: .*?\((\w+)\)\s*\n", text)
    iface = m.group(1) if m else "?"
    first = text.splitlines()[0] if text else ""
    m2 = re.search(r"\(([^()]*)\)+\s*$", first)
    dur = re.search(r"\* duration : ([0-9.]+)", text)
    host = re.search(r"\* hostname : (\S+)", text)
    exec_in = text.split("Execution Inputs", 1)[-1].split("Execution Outputs", 1)[0]
    vals = {}
    for line in exec_in.splitlines():
        mm = re.match(r"\* (\w+) : (.*)", line)
        if mm and mm.group(2) != "<undefined>":
            vals[mm.group(1)] = mm.group(2)
    return (
        first.replace("Node: ", ""),
        (float(dur.group(1)) if dur else None),
        (host.group(1) if host else None),
        vals,
    )


for run in sorted(os.listdir(ROOT)):
    if runs and not any(r in run for r in runs):
        continue
    wf = os.path.join(ROOT, run, "work", "fmriprep_25_2_wf")
    if not os.path.isdir(wf):
        continue
    rows = []
    for dp, dns, fns in os.walk(wf):
        rel = dp[len(wf) + 1 :]
        if "_report" in dns and os.path.isfile(os.path.join(dp, "_report", "report.rst")):
            in_fmap = "fmap_preproc_wf" in rel or "syn_preprocessing" in rel
            in_bold = any(f"/{n}" in "/" + rel for n in BOLD_FMAP_NODES)
            if in_fmap or in_bold:
                name, dur, host, vals = parse_report(os.path.join(dp, "_report", "report.rst"))
                rows.append((rel, name, dur, host, vals))
    if not rows:
        continue
    print("==", run)
    hosts = {r[3] for r in rows if r[3]}
    print("   hosts:", ", ".join(sorted(hosts)))
    for rel, name, dur, host, vals in sorted(rows):
        short = re.sub(r"^sub_\w+?_wf/", "", rel)
        iface = name.rsplit("(", 1)[-1].rstrip(")").strip() if "(" in name else ""
        d = f"{dur:.1f}s" if dur is not None else "-"
        print(f"   {short}  [{iface}] {d}")
        if want_inputs:
            for k in KEYS:
                if k in vals:
                    v = vals[k]
                    v = re.sub(r"/work/fmriprep_25_2_wf/[^ ,\]\']*/", ".../", v)
                    print(f"        {k} = {v[:300]}")
