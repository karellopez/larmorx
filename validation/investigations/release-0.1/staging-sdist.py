# SPDX-License-Identifier: Apache-2.0
"""Relay-1 experiment: can a companion's sdist be made when its pyproject.toml stands at the root of
a staging copy of the repository (so that maturin's sdist root contains both workspaces)?

Usage: python staging-sdist.py <checkout> <nc|gpl> <out dir>
Copies the tracked tree (no .git, no target) to <out>/stage-<pkg>, writes the companion's
pyproject.toml at its root with every path prefixed by crates-<pkg>/, and runs `maturin sdist`.
"""

import re
import shutil
import subprocess
import sys
from pathlib import Path

src, pkg, out = Path(sys.argv[1]), sys.argv[2], Path(sys.argv[3])
maturin = shutil.which("maturin") or "maturin"
stage = out / f"stage-{pkg}"
shutil.rmtree(stage, ignore_errors=True)
shutil.copytree(src, stage, ignore=shutil.ignore_patterns(".git", "target", "__pycache__", "*.so"))
sub = f"crates-{pkg}"
text = (stage / sub / "pyproject.toml").read_text()
# readme, licence files, manifest path and wheel data, relative to the new root
text = text.replace('readme = "README.md"', f'readme = "{sub}/README.md"')
text = re.sub(
    r"(license-files = \[)(.*?)(\])",
    lambda m: (
        m.group(1)
        + re.sub(r'"([^"]+)"', lambda q: f'"{sub}/{q.group(1)}"', m.group(2))
        + m.group(3)
    ),
    text,
    flags=re.S,
)
text = text.replace('manifest-path = "', f'manifest-path = "{sub}/')
text = text.replace('data = "wheel-data"', f'data = "{sub}/wheel-data"')
(stage / "pyproject.toml").write_text(text)
(stage / sub / "pyproject.toml").unlink()
r = subprocess.run(
    [str(maturin), "sdist", "-o", str(out / "sdist-staged")],
    cwd=stage,
    capture_output=True,
    text=True,
)
print(r.stdout[-600:], r.stderr[-1500:])
sys.exit(r.returncode)
