# SPDX-License-Identifier: Apache-2.0
"""Write docs/validation/oracles.md and oracle-runs.md from the oracles' BUILD_INFO.txt and RUN_INFO.txt.

Usage: python -P scripts/orchestration/oracle_info.py   (from the checkout; LARMORX_WORKSPACE overrides
the workspace, default the checkout's parent). Machine paths are written as <workspace> and ~; every
oracle program gets its SHA-256, so a build on another machine can be compared with the one the
validation records were made with. The FreeSurfer licence is never read or copied.
"""

import glob
import hashlib
import os
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
W = Path(os.environ.get("LARMORX_WORKSPACE", REPO.parent))
ORACLES = W / "oracles"
HOME = str(Path.home())


def norm(t: str) -> str:
    return t.replace(str(W), "<workspace>").replace(HOME, "~")


def sha(p: str) -> str:
    h = hashlib.sha256()
    with open(p, "rb") as f:
        for b in iter(lambda: f.read(1 << 20), b""):
            h.update(b)
    return h.hexdigest()


BUILDS = [
    ("ants-2.6.5", "ANTs 2.6.5 on ITK 5.4.5, GCC 14.3 (the oracle)"),
    ("afni-25.2.09", "AFNI 25.2.09"),
    ("core-math-preload", "CORE-MATH preload (correctly rounded maths for the originals)"),
    (
        "ants-2.6.5-gcc11-O3",
        "ANTs 2.6.5, GCC 11.4 -O3 (kept as evidence only; miscompiles float roundings)",
    ),
]


def oracles() -> str:
    out = [
        "# The oracles: how the original programs were built",
        "",
        'The builds larmorx is validated against (CLAUDE.md: "Validation against the oracles named here only"), each',
        "made by a committed script from a pinned release (`scripts/build_ants_oracle.sh`, `build_afni_oracle.sh`,",
        "`build_crmath_preload.sh`). This page copies each build's `BUILD_INFO.txt` verbatim (machine paths shown as",
        "`<workspace>`) and lists the SHA-256 of every program, so a build on another machine can be checked against",
        "the one the records were made with. Regenerate it with `scripts/orchestration/oracle_info.py` after a rebuild.",
        "",
    ]
    for d, title in BUILDS:
        b = ORACLES / d
        if not (b / "BUILD_INFO.txt").exists():
            continue
        out += [
            f"## {title}",
            "",
            f"`oracles/{d}/BUILD_INFO.txt`:",
            "",
            "```text",
            norm((b / "BUILD_INFO.txt").read_text()).rstrip(),
            "```",
            "",
        ]
        files = sorted(
            p
            for p in glob.glob(str(b / "bin" / "*")) + glob.glob(str(b / "lib*" / "*.so*"))
            if os.path.isfile(p) and not os.path.islink(p)
        )
        if not files:
            files = sorted(
                p
                for p in glob.glob(str(b / "*.so*")) + glob.glob(str(b / "*" / "*.so*"))
                if os.path.isfile(p)
            )
        if files:
            out += (
                ["| File | SHA-256 |", "|---|---|"]
                + [f"| `{os.path.relpath(p, b)}` | `{sha(p)}` |" for p in files]
                + [""]
            )
    return "\n".join(out)


def runs() -> str:
    out = [
        "# The fMRIPrep oracle runs",
        "",
        "fMRIPrep 25.2.6 run on OpenNeuro datasets through Apptainer, as the user (`scripts/run_fmriprep_oracle.sh`).",
        "Their outputs and working directories (`oracles/fmriprep-25.2.6/<run>/`) are the reference for the stage-by-stage",
        "comparisons and the real-data cases of the parity suites; they are not committed (image data). Each run's",
        "`RUN_INFO.txt` is copied here verbatim (machine paths shown as `<workspace>` and `~`): the image digest, the",
        "arguments, seeds and threads. The FreeSurfer licence file is mounted read-only, never copied.",
        "",
    ]
    for r in sorted(glob.glob(str(ORACLES / "fmriprep-25.2.6" / "*" / "RUN_INFO.txt"))):
        out += [
            f"## {Path(r).parent.name}",
            "",
            "```text",
            norm(Path(r).read_text()).rstrip(),
            "```",
            "",
        ]
    return "\n".join(out)


if __name__ == "__main__":
    (REPO / "docs/validation/oracles.md").write_text(oracles())
    (REPO / "docs/validation/oracle-runs.md").write_text(runs())
    print("docs/validation/oracles.md, docs/validation/oracle-runs.md written")
