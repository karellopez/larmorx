# SPDX-License-Identifier: Apache-2.0
"""Where ``python -m larmorx``'s memory goes on a small image (the spec writer's memory part,
2026-10-30): the peak resident memory (``/usr/bin/time -v``) of the bare interpreter, of importing
the Rust extension, of importing the command line, of ``--version``, and of 3dAutomask on
``tests/parity/test_memory_peak.py``'s mean image (64 x 64 x 40) through the entry point, through
``_core.cli_main`` called directly, and through the standalone ``larmorx`` binary; AFNI 26.3.05's
3dAutomask for reference. One thread. Prints a Markdown table.

    PYTHONPATH=python:validation/src:tests/parity python -P \\
        validation/investigations/specs-memory/entry_memory.py
"""

from __future__ import annotations

import os
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]


def main() -> int:
    sys.path.insert(0, str(ROOT / "tests" / "parity"))
    import test_memory_peak as t
    from larmorx_validation.parity import afni_replica as ar

    py = sys.executable
    with tempfile.TemporaryDirectory(prefix="lx-entry-mem-") as tmp_name:
        tmp = Path(tmp_name)
        (tmp / "in").mkdir()
        paths = t._afni_inputs(tmp / "in")
        home = tmp / "home"
        home.mkdir()
        env = {
            **{k: v for k, v in os.environ.items() if not k.upper().startswith(("AFNI_", "NIML_"))},
            "OMP_NUM_THREADS": "1",
            "HOME": str(home),
            "AFNI_DONT_LOGFILE": "YES",
            "AFNI_NIFTI_NOEXT": "YES",
            "LARMORX_UPSTREAM_DEFECTS": "reproduce",
            "LARMORX_IMPLEMENTATION": "replica",
        }
        mean = paths["mean"]
        args = ["-apply_prefix", "masked.nii.gz", "-dilate", "1", "-prefix", "mask.nii.gz", mean]
        call = (
            "import sys, larmorx._core as c; "
            f"code, o, e = c.cli_main(['larmorx', 'afni', '3dAutomask', *{args!r}], [], None); "
            "sys.exit(code)"
        )
        runs = [
            ("the interpreter (`python -c pass`)", [py, "-c", "pass"]),
            ("import larmorx._core", [py, "-c", "import larmorx._core"]),
            ("import larmorx", [py, "-c", "import larmorx"]),
            (
                "import larmorx._core, sysconfig.get_path('scripts')",
                [py, "-c", "import larmorx._core, sysconfig; sysconfig.get_path('scripts')"],
            ),
            ("import larmorx._replica", [py, "-c", "import larmorx._replica"]),
            ("import larmorx.cli", [py, "-c", "import larmorx.cli"]),
            ("python -m larmorx --version", [py, "-m", "larmorx", "--version"]),
            ("3dAutomask, python -m larmorx", [py, "-m", "larmorx", "afni", "3dAutomask", *args]),
            ("3dAutomask, _core.cli_main directly", [py, "-c", call]),
            (
                "3dAutomask, the larmorx binary",
                [str(ROOT / "target" / "release" / "larmorx"), "afni", "3dAutomask", *args],
            ),
            ("3dAutomask, AFNI 26.3.05", [str(ar.afni_binary("3dAutomask", "26.3.05")), *args]),
        ]
        print("| Run | Peak (MiB) |\n|---|---|")
        for i, (label, cmd) in enumerate(runs):
            out = tmp / f"run{i}"
            out.mkdir()
            code, mib = t._peak(cmd, env, out)
            print(f"| {label} | {mib:.1f}{'' if code == 0 else f' (exit {code})'} |", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
