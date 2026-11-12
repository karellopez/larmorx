# SPDX-License-Identifier: Apache-2.0
"""fMRIPrep's own t2smap files against tedana 25.0.1 run here in fMRIPrep 25.2.6's image (relay
RS2, 2026-11-12), on fMRIPrep's multi-echo calls of the oracle runs made again with OpenBLAS's
Haswell kernels (2026-11-09: ds000210's two tasks, ds000258): the values of ``T2starmap.nii.gz``
and ``desc-optcom_bold.nii.gz`` that differ, with numpy's AVX-512 kernels off (the replica
suite's and the evidence module's setting) and on (the CPU's default, as fMRIPrep's own run had
them: its RUN_INFO sets only ``OPENBLAS_CORETYPE=Haswell``), both through the oracle-output
cache::

    PYTHONPATH=<worktree>/python:<worktree>/validation/src python -P rs2_fmriprep_t25.py
"""

from __future__ import annotations

import shutil
import sys
import tempfile
from pathlib import Path

from larmorx_validation.parity import tedana_t2smap_replica as tt


def run_t25(args: list[str], cwd: Path, binds: list[Path], avx512: bool) -> tt.Run:
    if not avx512:
        return tt.run_fmriprep_tedana(args, cwd, binds)
    from larmorx_validation import oracle_cache

    image = tt.workspace() / tt.FMRIPREP_IMAGE
    inner = {
        "OPENBLAS_CORETYPE": "Haswell",
        "OPENBLAS_NUM_THREADS": "1",
        "COLUMNS": "80",
        "TQDM_DISABLE": "1",
    }
    cmd = ["apptainer", "exec", "--cleanenv", "--no-home", "--pwd", str(cwd)]
    for b in sorted({str(cwd), *(str(b) for b in binds)}):
        cmd += ["--bind", f"{b}:{b}"]
    for k, v in inner.items():
        cmd += ["--env", f"{k}={v}"]
    full = [*args, "--out-dir", "out"]
    p = oracle_cache.run_apptainer(
        image,
        "t2smap",
        full,
        inner,
        [*cmd, str(image), "t2smap", *full],
        cwd=cwd,
        outputs=[cwd],
        policy=oracle_cache.Policy(threads=1, family="python", volatile=tt.VOLATILE),
        runner="tedana-fmriprep",
        timeout=3600,
    )
    return tt.Run(-9 if p.returncode is None else p.returncode, p.stdout, p.stderr, cwd)


def main() -> int:
    stages = tt._investigation("check_stages")
    for name, args in stages.real_cases():
        if not name.startswith(("ds000210", "ds000258")) or "loglin" in name:
            continue
        node = tt._node_dir(name)
        binds = sorted({Path(a).parent for a in args if a.endswith(".nii.gz")})
        for avx in (False, True):
            d = Path(tempfile.mkdtemp(prefix="lx-t25_"))
            try:
                r = run_t25(args, d, binds, avx)
                diffs = tt.value_differences(node, r.out, tt.FMRIPREP_FILES)
                total, detail = tt._summarise(diffs)
                print(
                    f"{name}: tedana 25.0.1, numpy's AVX-512 kernels {'on' if avx else 'off'} "
                    f"(exit {r.code}): {total} values differ from fMRIPrep's own files; {detail}",
                    flush=True,
                )
            finally:
                shutil.rmtree(d, ignore_errors=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
