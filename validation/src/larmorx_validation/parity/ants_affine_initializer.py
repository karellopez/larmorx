# SPDX-License-Identifier: Apache-2.0
"""Parity of ``larmorx ants antsAffineInitializer`` with ANTs 2.6.5's ``antsAffineInitializer``.

The comparison is the antsAI suite's (:mod:`larmorx_validation.parity.ants_ai`): ANTs as its
own binary with one thread, larmorx in-process with 4 threads and ANTs' defects reproduced;
the exit status, both streams and every file written (the transform byte for byte); ANTs with
CORE-MATH's correctly rounded functions where the first comparison differs; everything ANTs
printed before an abort or a crash; ANTs' thread band; a fix pass.

The program takes positional arguments only and prints nothing but its usage, the
principal-axes determinant (` bad det`), the moments' ` zero image` errors and what ITK
throws. The real case is niworkflows' call (``RobustMNINormalization`` without an initial
transform: ``antsAffineInitializer 3 fixed moving transform.mat 15 0.1 0 10``) on the MNI
template and fMRIPrep's preprocessed T1w of the ds000005 oracle run, and (full tier) fMRIPrep's
own call in each oracle run's template registration, on stand-ins for the masked images nipype
did not keep (the transform it wrote was not kept either, so there is nothing of fMRIPrep's to
compare it with).
"""

from __future__ import annotations

from larmorx_validation.parity import ants_ai
from larmorx_validation.parity import ants_registration as reg
from larmorx_validation.parity.ants_ai import AiCase
from larmorx_validation.parity.ants_programs import ants_version
from larmorx_validation.parity.harness import Case, Suite

PROGRAM = "antsAffineInitializer"
_TIERS = ("smoke", "standard", "full")

F, M, K = reg.F, reg.M, reg.K
F2, M2 = reg.F2, reg.M2
F4, M4 = reg.F4, reg.M4
FO, MO = reg.FO, reg.MO
Z = "{gen:reg-zero}"
T1W = "{oracle:ds000005-sub-01-noreconall/out/sub-01/anat/sub-01_desc-preproc_T1w.nii.gz}"
#: The fMRIPrep oracle runs whose template registration ran ``antsAffineInitializer``.
RUNS = (
    "ds000005-sub-01-noreconall",
    "ds001600-sub-1-noreconall",
    "ds005040-sub-002-noreconall",
    "ds005454-sub-16-noreconall",
    "ds005752-sub-ON01016-noreconall",
)


def case(cid: str, category: str, description: str, *args: str, **kw) -> AiCase:
    return AiCase(cid, category, description, tuple(args), program=PROGRAM, **kw)


def all_cases() -> list[AiCase]:
    out = "{out}/t.mat"
    return [
        case("messages/no-arguments", "messages", "No arguments: the usage, exit 0", tier="smoke"),
        case("messages/dimension-only", "messages", "One argument: the usage, exit 0", "3"),
        case(
            "messages/two-arguments",
            "messages",
            "Two arguments: std::string of a null pointer (ANTS-57)",
            "3",
            F,
            expect="abort",
        ),
        case(
            "messages/three-arguments",
            "messages",
            "Three arguments (ANTS-57)",
            "3",
            F,
            M,
            expect="abort",
        ),
        case(
            "messages/dimension-text",
            "messages",
            "A dimension std::stoi cannot read",
            "x",
            F,
            M,
            out,
            expect="abort",
        ),
        case(
            "messages/dimension-range",
            "messages",
            "A dimension beyond int's range",
            "99999999999",
            F,
            M,
            out,
            expect="abort",
        ),
        case(
            "messages/dimension-5", "messages", "Dimension 5 runs as 2D (ANTS-56)", "5", F2, M2, out
        ),
        case(
            "messages/dimension-1", "messages", "Dimension 1 runs as 2D (ANTS-56)", "1", F2, M2, out
        ),
        case(
            "messages/principal-axes-text",
            "messages",
            "A principal-axes flag std::stoi cannot read",
            "3",
            F,
            M,
            out,
            "30",
            "0.1",
            "x",
            expect="abort",
        ),
        case(
            "messages/missing-fixed",
            "messages",
            "A fixed image that does not exist",
            "3",
            "{missing:fixed.nii.gz}",
            M,
            out,
            expect="abort",
        ),
        case(
            "messages/missing-moving",
            "messages",
            "A moving image that does not exist",
            "3",
            F,
            "{missing:moving.nii.gz}",
            out,
            expect="abort",
        ),
        case(
            "messages/zero-fixed",
            "messages",
            "A fixed image of zeros",
            "3",
            Z,
            M,
            out,
            expect="abort",
        ),
        case(
            "messages/zero-moving",
            "messages",
            "A moving image of zeros",
            "3",
            F,
            Z,
            out,
            expect="abort",
        ),
        case(
            "messages/unknown-extension",
            "messages",
            "An output name no writer takes",
            "3",
            F,
            M,
            "{out}/t.xyz",
            "30",
            "0.2",
            expect="abort",
        ),
        case(
            "messages/no-directory",
            "messages",
            "An output in a directory that does not exist",
            "3",
            F,
            M,
            "{out}/none/t.mat",
            "30",
            "0.2",
            expect="abort",
        ),
        case(
            "messages/search-factor-zero",
            "messages",
            "A search factor of 0: ANTs' loop never ends",
            "3",
            F,
            M,
            out,
            "0",
            "0.1",
            expect="hang",
        ),
        case(
            "messages/search-factor-text",
            "messages",
            "A search factor atof reads as 0: ANTs' loop never ends",
            "3",
            F,
            M,
            out,
            "abc",
            "0.1",
            expect="hang",
        ),
        case(
            "messages/search-factor-nan",
            "messages",
            "A search factor that is not a number: no start point",
            "3",
            F,
            M,
            out,
            "nan",
            "0.1",
            expect="abort",
        ),
        case(
            "messages/radian-fraction-nan",
            "messages",
            "A radian fraction that is not a number: no start point",
            "3",
            F,
            M,
            out,
            "30",
            "nan",
            expect="abort",
        ),
        case(
            "search/defaults",
            "search",
            "3D, every default (10 degrees, pi/4)",
            "3",
            F,
            M,
            out,
            tier="smoke",
            band=True,
        ),
        case(
            "search/niworkflows",
            "search",
            "niworkflows' arguments (15 0.1 0 10)",
            "3",
            F,
            M,
            out,
            "15.000000",
            "0.100000",
            "0",
            "10",
        ),
        case(
            "search/principal-axes",
            "search",
            "From the principal axes",
            "3",
            F,
            M,
            out,
            "15",
            "0.1",
            "1",
            "10",
            band=True,
        ),
        case(
            "search/principal-axes-mask",
            "search",
            "From the principal axes, a fixed mask",
            "3",
            F,
            M,
            out,
            "15",
            "0.1",
            "1",
            "10",
            K,
        ),
        case(
            "search/missing-mask",
            "search",
            "A mask that cannot be read is no mask",
            "3",
            F,
            M,
            out,
            "15",
            "0.1",
            "0",
            "10",
            "{missing:mask.nii.gz}",
        ),
        case(
            "search/fraction-above-one",
            "search",
            "A radian fraction of 5 (clamped to 1)",
            "3",
            F,
            M,
            out,
            "40",
            "5",
            "0",
        ),
        case(
            "search/fraction-below-minimum",
            "search",
            "A radian fraction of 0.001 (clamped to 0.01)",
            "3",
            F,
            M,
            out,
            "5",
            "0.001",
            "0",
        ),
        case(
            "search/local-search-text",
            "search",
            "A local-search argument that is not read (ANTS-54; corrected: an error)",
            "3",
            F,
            M,
            out,
            "30",
            "0.1",
            "0",
            "abc",
        ),
        case(
            "search/local-search-zero",
            "search",
            "A local search of 0 iterations (ANTS-54; corrected: start points only evaluated)",
            "3",
            F,
            M,
            out,
            "30",
            "0.1",
            "0",
            "0",
        ),
        case(
            "search/local-search-negative",
            "search",
            "A negative local search (ANTS-54; corrected: an error)",
            "3",
            F,
            M,
            out,
            "30",
            "0.1",
            "0",
            "-3",
        ),
        case(
            "search/local-search-20",
            "search",
            "A local search of 20 iterations, ANTs' own (only ITK-21 can differ)",
            "3",
            F,
            M,
            out,
            "30",
            "0.1",
            "1",
            "20",
        ),
        case(
            "search/principal-axes-2",
            "search",
            "A principal-axes flag of 2 (true)",
            "3",
            F,
            M,
            out,
            "30",
            "0.1",
            "2",
        ),
        case(
            "search/dimension-text-prefix",
            "search",
            "A dimension of 3x (std::stoi reads 3)",
            "3x",
            F,
            M,
            out,
            "30",
            "0.2",
        ),
        case(
            "search/2d",
            "search",
            "2D from the principal axes",
            "2",
            F2,
            M2,
            out,
            "20",
            "0.5",
            "1",
            band=True,
        ),
        case("search/2d-defaults", "search", "2D, every default", "2", F2, M2, out),
        case("search/4d", "search", "4D: the initial alignment only", "4", F4, M4, out),
        case(
            "search/4d-principal-axes",
            "search",
            "4D from the principal axes",
            "4",
            F4,
            M4,
            out,
            "10",
            "0.1",
            "1",
        ),
        case(
            "search/oblique",
            "search",
            "Oblique grids, from the principal axes",
            "3",
            FO,
            MO,
            out,
            "30",
            "0.2",
            "1",
        ),
        case(
            "search/txt", "search", "A text transform file", "3", F, M, "{out}/t.txt", "30", "0.2"
        ),
        case("search/h5", "search", "An HDF5 transform file", "3", F, M, "{out}/t.h5", "30", "0.2"),
        case(
            "search/extra-arguments",
            "search",
            "Arguments after the mask are ignored",
            "3",
            F,
            M,
            out,
            "30",
            "0.2",
            "0",
            "5",
            K,
            "extra1",
            "extra2",
        ),
        case(
            "real/mni-2mm",
            "real",
            "niworkflows' arguments on the 2 mm MNI template and fMRIPrep's preprocessed T1w "
            "(ds000005)",
            "3",
            "{tf:tpl-MNI152NLin2009cAsym/tpl-MNI152NLin2009cAsym_res-02_T1w.nii.gz}",
            T1W,
            out,
            "15.000000",
            "0.100000",
            "0",
            "10",
            band=True,
        ),
        *[
            case(
                f"real/niworkflows/{run.split('-noreconall')[0]}",
                "real",
                "fMRIPrep's call in RobustMNINormalization (no initial transform): the masked "
                "1 mm template and the run's masked T1w",
                "3",
                "{nw-fixed}",
                f"{{nw-moving:{run}}}",
                "{out}/transform.mat",
                "15.000000",
                "0.100000",
                "0",
                "10",
                tier="full",
            )
            for run in RUNS
        ],
    ]


def cases_for(tier: str) -> list[Case]:
    rank = _TIERS.index(tier)
    return [
        Case(c.id, c.category, c.description, c)
        for c in all_cases()
        if _TIERS.index(c.tier) <= rank
    ]


def suite() -> Suite:
    return Suite(
        name="ants-affine-initializer",
        title="antsAffineInitializer",
        tool="`larmorx ants antsAffineInitializer` / `lx.ants.affine_initializer` (crates "
        "larmorx-ants, larmorx-registration)",
        reference=f"antsAffineInitializer from {ants_version()}, the binary built by "
        "`scripts/build_ants_oracle.sh`, run with one thread",
        thresholds=[
            ("Exit status", "identical"),
            ("Standard output and error", "identical (larmorx's D13 notices aside)"),
            ("Files", "the transform byte for byte (HDF5: every dataset)"),
            (
                "glibc's maths",
                "where ANTs as built differs, ANTs with CORE-MATH preloaded must be identical",
            ),
            ("Aborts and crashes", "everything ANTs printed before it stopped; larmorx exits 1"),
            (
                "Thread band",
                f"reported: ANTs at {', '.join(map(str, ants_ai.BAND_THREADS))} threads against 1",
            ),
        ],
        cases=cases_for,
        run_case=ants_ai.run_case,
        packages=("nibabel", "scipy"),
        highlights=ants_ai.highlights,
        notes="Inputs named `gen:<name>` are built by the suites (seeded phantoms).",
    )


__all__ = ["all_cases", "suite"]
