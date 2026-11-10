# SPDX-License-Identifier: Apache-2.0
"""The permanent cases of relay 27's random testing of the field maps' part B (the difftest tools
``ndimage-skimage``, ``niworkflows-helpers`` and ``sdcflows-estimation``): each difference the
first probes found (``r27/try_cases.py``'s drawn cases, the same recipes as ``difftest --case
TOOL:SEED:N``), written as ``validation/difftest/cases/<tool>/<id>.json`` in the form
``difftest --promote`` writes, so the tool's parity suite runs it from then on (smoke tier).

    PYTHONPATH=<repo>/python:<repo>/validation/src python -P make_permanent.py [--check]

``--check`` runs each case through its suite's runner and prints its checks (all must pass in
the newest environment, the suites' own; in the shared one ``reorient-rotation-with-two-candidates``
differs as numpy's LAPACK 3.12.0 picks the other axis, until relay 28: the runner now follows the
environment's LAPACK release, as nibabel does; ALL OK in both). A case whose
file exists is checked as committed, never written again (relay 28: the draws changed in
e88326b6, so the seeds of ``brain-masker-no-positive-value`` and ``brain-masker-padding-0`` now
draw other calls).
"""

from __future__ import annotations

import argparse
import json
import sys

from larmorx_validation.difftest import core, inputs, tools

#: (tool, seed, case number, id, status, defect, rule, description)
CASES = [
    (
        "ndimage-skimage",
        "try1",
        120,
        "binary-erosion-int8-mask-overwritten",
        "register",
        "SCI-31",
        "A",
        "an iterated binary_erosion with an int8 mask: SciPy writes its scratch values into the "
        "caller's mask (SCI-31), so each side gets its own arguments and they are compared afterwards "
        "(the first probe gave larmorx the mask SciPy had overwritten)",
    ),
    (
        "ndimage-skimage",
        "relay27-rw",
        15279,
        "random-walker-cg-mg-without-zeros",
        "fixed",
        None,
        None,
        "random_walker(mode='cg_mg') with no label 0: scikit-image returns the labels before it "
        "names the mode it replaces; larmorx warned about pyamg first (found as try3's case 380, "
        "a float64 one, compared only without numpy's AVX-512 exp; this float32 one everywhere)",
    ),
    (
        "niworkflows-helpers",
        "try1",
        35,
        "denoise-image-lists-noise-image",
        "register",
        "PIPE-38",
        "C",
        "nipype's DenoiseImage lists noise_image (in_noise.nii.gz) without save_noise, as it lists "
        "every name_source output; larmorx's formatter left it out (PIPE-38: kept as nipype's)",
    ),
    (
        "sdcflows-estimation",
        "try1",
        15,
        "brain-masker-no-positive-value",
        "fixed",
        None,
        None,
        "brain_masker of an image without a positive value: numpy's IndexError of the empty "
        "array's percentile (larmorx raised it as ValueError)",
    ),
    (
        "sdcflows-estimation",
        "try1",
        19,
        "readout-time-string-trait-error",
        "fixed",
        None,
        None,
        "GetReadoutTime of a TotalReadoutTime given as a string: traits' TraitError of the output "
        "spec, with its message (larmorx raised a TypeError of its own words)",
    ),
    (
        "sdcflows-estimation",
        "try1",
        30,
        "readout-time-invalid-direction",
        "fixed",
        None,
        None,
        "GetReadoutTime of an invalid PhaseEncodingDirection ('j+'): traits' TraitError of the "
        "output spec (larmorx returned the direction); nipype checks the outputs in a hash-ordered "
        "set (PIPE-39), larmorx and the comparison in their declared order",
    ),
    (
        "sdcflows-estimation",
        "try1",
        59,
        "uniform-grid-4d-off-the-grid",
        "fixed",
        None,
        None,
        "UniformGrid of a 4-D image on another grid: nitransforms' map_coordinates raises SciPy's "
        "RuntimeError('invalid shape for coordinate array'); larmorx refused with NotImplementedError",
    ),
    (
        "sdcflows-estimation",
        "try1",
        104,
        "brain-masker-padding-0",
        "register",
        "SDC-46",
        "A",
        "brain_masker with padding=0: sdcflows crops its probabilities with [..., 0:-0, ...] and "
        "writes empty probability and mask images, the brain unmasked (SDC-46)",
    ),
    (
        "sdcflows-estimation",
        "fmapb-try1",
        11,
        "epi-mask-watershed-notice-without-change",
        "fixed",
        None,
        None,
        "epi_mask whose watershed labels differ between the modes (SCI-13, SCI-14) only where the "
        "brain's dilated label is the same: no notice (larmorx named the defects without a change)",
    ),
    (
        "niworkflows-helpers",
        "fmapb-try1",
        114,
        "intra-modal-merge-ras-mask",
        "register",
        "PIPE-40",
        "A",
        "IntraModalMerge with to_ras, grand_mean_scaling and in_mask: niworkflows reorients the "
        "series but not the mask (PIPE-40; its indexing fails anyway: PIPE-18); larmorx reorients "
        "the mask with the series and names it",
    ),
    (
        "sdcflows-estimation",
        "try1",
        134,
        "match-histogram-message-dtype",
        "fixed",
        None,
        None,
        "match_histogram of an unsigned image with a float reference (SKI-02 reproduced): numpy's "
        "message names nibabel's dtype('<f4'), which larmorx now keeps for unscaled data",
    ),
    (
        "ndimage-skimage",
        "fmapb-try2",
        1211,
        "binary-opening-step-notice-without-change",
        "fixed",
        None,
        None,
        "binary_opening with a uint16 mask (SCI-10's cast) whose erosion step differs between the "
        "modes but whose opening does not: no notice (larmorx named the step's defect)",
    ),
    (
        "sdcflows-estimation",
        "fmapb-try2",
        725,
        "reorient-rotation-with-two-candidates",
        "fixed",
        None,
        None,
        "ReorientImageAndMetadata of an affine whose rotation has two equal candidates: nibabel's "
        "io_orientation (numpy's SVD) decides the axis; larmorx used a platform-independent SVD "
        "(another axis, other directions and dim_info)",
    ),
]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true")
    args = ap.parse_args()
    bad = 0
    for tool, seed, n, cid, status, defect, rule, desc in CASES:
        t = tools.get(tool)
        recipe = core.make_recipe(t, seed, n)
        doc = {
            "id": cid,
            "tool": tool,
            "suite": t.suite,
            "implementations": ["port"],
            "description": desc,
            "found": {"seed": seed, "n": n, "signature": [f"{recipe['extra']['call']}: every output bit-identical"],
                      "detail": "relay 27's first probe (r27/try_cases.py)", "shrink_moves": []},
            "status": status,
            "defect": defect,
            "rule": rule,
            "tier": "smoke",
            "expect": "pass",
            "reason": "",
            "recipe": recipe,
        }  # fmt: skip
        d = inputs.CASES / tool
        d.mkdir(parents=True, exist_ok=True)
        # Relay 28: an existing case is never written again (e88326b6 changed the draws, so two
        # of the seeds below now draw other calls; the committed files keep the probes' recipes)
        if not (d / f"{cid}.json").exists():
            (d / f"{cid}.json").write_text(
                json.dumps(doc, indent=1, sort_keys=True) + "\n", encoding="utf-8"
            )
        if args.check:
            from larmorx_validation.difftest import permanent
            from larmorx_validation.parity.harness import CheckList, Outcome

            case = next(c for c in permanent.cases(tool, "smoke") if c.id == f"difftest/{cid}")
            checks = CheckList()
            try:
                t.run("port")(case, checks)
            except Outcome as o:
                bad += 1
                print(f"{tool}/{cid}: {o}")
                continue
            failed = [c for c in checks.checks if not c.passed]
            bad += bool(failed)
            print(f"{tool}/{cid}: {len(checks.checks)} checks, {len(failed)} failed"
                  + "".join(f"\n   {c.name}: {c.detail[:300]}" for c in failed))  # fmt: skip
    print("ALL OK" if not bad else f"FAIL {bad}")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
