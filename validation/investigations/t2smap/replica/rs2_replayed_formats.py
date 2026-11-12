# SPDX-License-Identifier: Apache-2.0
"""The replica (``reproduce``) against tedana 26.0.3's files on replayed random-testing cases
whose inputs are ANALYZE 7.5 pairs or MGH/MGZ images (relay RS2, 2026-11-12): every file tedana
wrote, image headers and data decompressed, bit for bit (``dataset_description.json`` and
``t2smap_call.sh`` apart), and the replica's exit status where tedana wrote nothing.

The cases are folders ``<work>/f<n>/{inputs,tedana/out}`` as
``../../t2smap-original/replay.py <finding> --keep <work>/f<n>`` leaves them::

    python -P rs2_replayed_formats.py <work> <larmorx-gpl> <scratch dir> [f<n> ...]
"""

from __future__ import annotations

import gzip
import os
import shlex
import shutil
import subprocess
import sys
from pathlib import Path

FORMATS = (".hdr", ".img", ".mgh", ".mgz", ".hdr.gz", ".img.gz")


def files_of(out: Path) -> dict[str, bytes]:
    got = {}
    for p in sorted(out.rglob("*")):
        if p.is_file() and not p.name.endswith(("dataset_description.json", "t2smap_call.sh")):
            data = p.read_bytes()
            got[str(p.relative_to(out))] = gzip.decompress(data) if p.suffix == ".gz" else data
    return got


def main() -> int:
    work, replica, scratch = Path(sys.argv[1]), sys.argv[2], Path(sys.argv[3])
    only = set(sys.argv[4:])
    env = {k: v for k, v in os.environ.items() if not k.startswith(("LARMORX_", "OMP_"))}
    env.update(COLUMNS="80", TQDM_DISABLE="1", LARMORX_UPSTREAM_DEFECTS="reproduce")
    n = bad = 0
    for case in sorted(work.glob("f*")):
        if only and case.name not in only:
            continue
        call = case / "tedana/out/t2smap_call.sh"
        if not call.is_file():
            continue
        words = shlex.split(call.read_text(encoding="utf-8"))[1:]
        if not any(w.lower().endswith(FORMATS) for w in words):
            continue
        n += 1
        if "--out-dir" in words:
            i = words.index("--out-dir")
            del words[i : i + 2]
        run = scratch / case.name
        shutil.rmtree(run, ignore_errors=True)
        run.mkdir(parents=True)
        p = subprocess.run(
            [replica, "tedana", "t2smap", *words, "--out-dir", "out"],
            cwd=run,
            env=env,
            capture_output=True,
            text=True,
            timeout=900,
        )
        t_out = case / "tedana/out"
        completed = any(t_out.glob("*.json"))
        tf, rf = files_of(t_out), files_of(run / "out") if (run / "out").is_dir() else {}
        if not completed:
            # tedana failed: the replica must fail too (its message is the suite's to compare)
            ok = p.returncode != 0
            what = (
                f"tedana failed; replica exit {p.returncode}: {p.stderr.strip().splitlines()[-1:]}"
            )
        else:
            diff = sorted(k for k in set(tf) | set(rf) if tf.get(k) != rf.get(k))
            ok = p.returncode == 0 and not diff
            what = f"exit {p.returncode}; files differing: {diff[:6]}"
            if p.returncode != 0:
                what += f" ({p.stderr.strip().splitlines()[-1:]})"
        bad += not ok
        forms = sorted({Path(w).suffix.lower() for w in words if w.lower().endswith(FORMATS)})
        print(f"{'ok  ' if ok else 'DIFF'} {case.name} {forms}: {what}", flush=True)
        if ok:
            shutil.rmtree(run, ignore_errors=True)
    print(f"{n} cases, {bad} differ")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
