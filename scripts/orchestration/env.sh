# SPDX-License-Identifier: Apache-2.0
# Common environment of the orchestration scripts; sourced, not run.
# LARMORX_WORKSPACE: the workspace holding larmorx/, .venv/, oracles/ (default: the parent of this repository).
# LARMORX_VENV: the Python environment with larmorx's development and validation dependencies.
_here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
export LARMORX_REPO=${LARMORX_REPO:-$(cd "$_here/../.." && pwd)}
# In a git worktree (<repo>/.claude/worktrees/<name>) the workspace is the main checkout's parent, not the worktree's.
_common=$(git -C "$LARMORX_REPO" rev-parse --path-format=absolute --git-common-dir 2>/dev/null)
_main=${_common:+$(dirname "$_common")}
export LARMORX_WORKSPACE=${LARMORX_WORKSPACE:-$(cd "${_main:-$LARMORX_REPO}/.." && pwd)}
export LARMORX_VENV=${LARMORX_VENV:-$LARMORX_WORKSPACE/.venv}
# Cargo first; FSL kept off the path (its programs shadow nothing of ours, but the rule is explicit).
export PATH=$LARMORX_VENV/bin:$HOME/.cargo/bin:$(echo "$PATH" | tr ':' '\n' | grep -v /fsl | paste -sd:)
export PYO3_PYTHON=$LARMORX_VENV/bin/python
# numpy's OpenBLAS on its Haswell kernels, the reference larmorx reproduces (decision log 2026-10-11):
# on an AVX-512 CPU it would pick SkylakeX, whose 4x4 inverses differ (docs/findings/numpy-blas.md),
# and the parity suites comparing with numpy, nibabel and nitransforms would not reproduce their records.
# fMRIPrep's oracle runs use `apptainer --cleanenv`, so it does not reach them (machine-move-proof.md).
export OPENBLAS_CORETYPE=${OPENBLAS_CORETYPE:-Haswell}
