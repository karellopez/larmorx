# SPDX-License-Identifier: Apache-2.0
"""The permanent cases of the confounds' random-testing probes and of its trial's notice verdicts
(relay 3a-iii): each a drawn case (tool, seed, case number) of ``difftest/tools/confounds.py`` that
showed a larmorx mistake (a difference, or a notice where nothing changed), now fixed, written with ``difftest.permanent.promote`` as a trial's finding would be
(``validation/difftest/cases/<tool>/<id>.json``; the suites run them in the smoke tier).

    PYTHONPATH=<repo>/python:<repo>/validation/src python -P promote_probe_cases.py
"""

import json
import tempfile
from pathlib import Path

from larmorx_validation.difftest import core, permanent, tools

#: (tool, seed, n, id, failed check, what was wrong)
CASES = (
    (
        "confounds-nipype",
        "probe3",
        485,
        "regress-poly-fortran-rows",
        "regress_poly: every output bit-identical",
        "regress_poly of Fortran-ordered rows (387 x 69 float32, degree 0): numpy's "
        "pinv(X).dot(data.T) takes gemv on a C-ordered data.T, whose sums run in another order; "
        "larmorx summed as for C-ordered rows (16,284 of 26,703 residuals one or two ulps off). "
        "Fixed: regress_poly_layout follows the rows' memory order.",
    ),
    (
        "confounds-nipype",
        "probe",
        148,
        "tcompcor-refused-degree-text",
        "compcor: every output bit-identical",
        "TCompCor with regress_poly_degree=0: traits' TraitError text ('The "
        "'regress_poly_degree' trait of a TCompCorInputSpec instance must be an integer >= 1, ...') "
        "where larmorx raised its own message. Fixed: every refused CompCor input raises traits' "
        "or nipype's text.",
    ),
    (
        "confounds-fmriprep",
        "probe",
        140,
        "rename-acompcor-text-retained",
        "rename_acompcor: every output bit-identical",
        "RenameACompCor with a 'retained' column of text: pandas' KeyError prints the values' "
        "Index with line breaks at width 80, larmorx in one line. Fixed: _index_repr follows "
        "pandas' format_object_summary.",
    ),
    (
        "confounds-nipype",
        "confounds-trial",
        105,
        "dvars-constant-series-notices",
        "D13 fix pass",
        "compute_dvars of an all-zero series with remove_zerovariance=False (the trial's notice "
        "verdicts): PIPE-50's and PIPE-46's corrections leave every output NaN, as nipype's, yet "
        "their notices fired. Fixed: named only where the corrected series differ from nipype's.",
    ),
    (
        "confounds-nipype",
        "confounds-trial",
        12587,
        "cosine-filter-glibc07-exact",
        "D13 fix pass",
        "cosine_filter whose glibc and correctly rounded cosines differ only before the product "
        "with sqrt(2 / T) rounds them to one value (the trial's notice verdicts): GLIBC-07 fired "
        "with the basis unchanged. Fixed: the witness compares the basis values.",
    ),
    (
        "confounds-fmriprep",
        "confounds-trial",
        1234,
        "spikes-pipe67-after-filling",
        "D13 fix pass",
        "SpikeRegressors with lags [1, 2] and minimum_contiguous 5 (the trial's notice "
        "verdicts): the filling of short runs made both lag rules flag the same volumes, yet "
        "PIPE-67 fired. Fixed: PIPE-67 is named where the flagged volumes differ after the "
        "filling.",
    ),
    (
        "confounds-nipype",
        "confounds-trial",
        42559,
        "compcor-pipe44-later-masks-add-nothing",
        "D13 fix pass",
        "ACompCor whose second mask gives no component and whose later masks give none either "
        "(the trial's notice verdicts): the default goes on with them (PIPE-44) to the same "
        "result, yet named PIPE-44. Fixed: named where a later mask adds components or changes "
        "the pre-filter's basis.",
    ),
)


def main() -> None:
    with tempfile.TemporaryDirectory() as d:
        for tool, seed, n, cid, check, what in CASES:
            recipe = core.make_recipe(tools.get(tool), seed, n)
            finding = {
                "tool": tool,
                "implementation": "port",
                "seed": seed,
                "n": n,
                "signature": [check],
                "detail": what,
                "recipe": recipe,
            }
            f = Path(d) / f"{cid}.json"
            f.write_text(json.dumps(finding), encoding="utf-8")
            out = permanent.promote(f, cid, description=what, status="fixed")
            print(out)


if __name__ == "__main__":
    main()
