#!/usr/bin/env bash
# Recreate the larmorx workspace on a new machine.
#
# The workspace root is the directory that contains this repo (larmorx/). Upstream
# repositories from upstream.tsv are shallow-cloned at their pinned commits next to it
# (or under reference_src/ for licence-restricted sources).
#
# Usage:
#   larmorx/scripts/bootstrap_workspace.sh [options]
#
# Options:
#   --workspace DIR   workspace root (default: parent directory of this repo)
#   --only a,b,c      clone only these upstream names
#   --skip a,b,c      skip these upstream names (e.g. --skip freesurfer,afni to save ~1.1 GB)
#   --jobs N          parallel clones (default: 4)
#   --venv DIR        also install evaluation packages (fmriprep from the local clone,
#                     antspyx, SimpleITK, pipdeptree) into this existing virtualenv
#   --install-rust    install the Rust toolchain with rustup (user-level, ~/.cargo) if missing
#   -h, --help        show this help
set -euo pipefail

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WS_DIR="$(dirname "$REPO_DIR")"
LOCK="$REPO_DIR/upstream.tsv"
ONLY=""
SKIP=""
JOBS=4
VENV=""
INSTALL_RUST=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --workspace) WS_DIR="$(mkdir -p "$2" && cd "$2" && pwd)"; shift 2 ;;
    --only) ONLY=",$2,"; shift 2 ;;
    --skip) SKIP=",$2,"; shift 2 ;;
    --jobs) JOBS="$2"; shift 2 ;;
    --venv) VENV="$2"; shift 2 ;;
    --install-rust) INSTALL_RUST=1; shift ;;
    -h|--help) sed -n '2,20p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "unknown option: $1" >&2; exit 2 ;;
  esac
done

command -v git >/dev/null || { echo "git is required" >&2; exit 1; }
echo "workspace: $WS_DIR"
echo "repo:      $REPO_DIR"

# Clone one repository at an exact commit (shallow). Idempotent: skips if already at that commit.
clone_one() {
  local name="$1" url="$2" sha="$3" dest="$4"
  if [[ -d "$dest/.git" ]] && [[ "$(git -C "$dest" rev-parse HEAD 2>/dev/null)" == "$sha" ]]; then
    echo "  = $name (already at ${sha:0:9})"
    return 0
  fi
  if [[ -e "$dest" && ! -d "$dest/.git" ]]; then
    echo "  ! $name: $dest exists and is not a git repo; skipping" >&2
    return 1
  fi
  mkdir -p "$dest"
  git -C "$dest" init -q
  git -C "$dest" remote get-url origin >/dev/null 2>&1 || git -C "$dest" remote add origin "$url"
  if git -C "$dest" fetch -q --depth 1 origin "$sha" && git -C "$dest" checkout -q --detach FETCH_HEAD; then
    echo "  + $name @ ${sha:0:9}"
  else
    echo "  ! $name: failed to fetch $sha from $url" >&2
    return 1
  fi
}

echo "cloning upstream references (jobs=$JOBS)..."
fail=0
pids=()
while IFS=$'\t' read -r name url sha licence location; do
  [[ -z "${name// }" || "$name" == \#* ]] && continue
  [[ -n "$ONLY" && "$ONLY" != *",$name,"* ]] && continue
  [[ -n "$SKIP" && "$SKIP" == *",$name,"* ]] && continue
  case "$location" in
    ws) dest="$WS_DIR/$name" ;;
    tags) dest="$WS_DIR/tags/$name" ;;
    reference_src) dest="$WS_DIR/reference_src/$name" ;;
    *) echo "  ! $name: unknown location '$location'" >&2; fail=1; continue ;;
  esac
  clone_one "$name" "$url" "$sha" "$dest" &
  pids+=("$!")
  while [[ "$(jobs -rp | wc -l)" -ge "$JOBS" ]]; do sleep 0.5; done
done < "$LOCK"
for pid in "${pids[@]}"; do wait "$pid" || fail=1; done

if [[ -d "$WS_DIR/reference_src" ]]; then
  cat > "$WS_DIR/reference_src/DO_NOT_READ.md" <<'EOF'
# Licence-restricted sources

These repositories are licence-restricted:

- workbench/  (GPL-2.0-or-later)
- MSM_HOCR/   (non-commercial; the optimiser is patented: do not implement it)

They may be read to understand behaviour (CLAUDE.md rule 1, decided 2026-10-09). larmorx's
code must be an original implementation, never a translation of theirs, even renamed.
Record the behaviour in specs/<tool>.md and validate against the upstream binaries. See
larmorx/CLAUDE.md and PLAN.md §12.
EOF
fi

# Workspace-level CLAUDE.md so Claude Code opened at the workspace root loads the repo context.
if [[ ! -f "$WS_DIR/CLAUDE.md" ]]; then
  cat > "$WS_DIR/CLAUDE.md" <<'EOF'
# Workspace root

This directory holds the larmorx repo plus read-only upstream reference clones.
The project context lives in the repo:

@larmorx/CLAUDE.md
EOF
  echo "wrote $WS_DIR/CLAUDE.md"
fi

if [[ "$INSTALL_RUST" == 1 ]]; then
  if command -v rustup >/dev/null || [[ -x "$HOME/.cargo/bin/rustup" ]]; then
    echo "rustup already installed"
  else
    echo "installing Rust (rustup, user-level)..."
    command -v curl >/dev/null || { echo "curl is required for --install-rust" >&2; exit 1; }
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile default
  fi
  "$HOME/.cargo/bin/rustup" component add clippy rustfmt >/dev/null
  "$HOME/.cargo/bin/rustc" --version
fi

if [[ -n "$VENV" ]]; then
  PY="$VENV/bin/python"
  [[ -x "$PY" ]] || { echo "no python at $PY (create the venv first)" >&2; exit 1; }
  echo "installing evaluation packages into $VENV..."
  "$PY" -m pip install -q --upgrade pip
  "$PY" -m pip install -q pipdeptree antspyx SimpleITK
  if [[ -d "$WS_DIR/fmriprep" ]]; then
    # Note: fmriprep's pyproject pulls smriprep from git master, not the pinned clone.
    "$PY" -m pip install -q "$WS_DIR/fmriprep"
  fi
  "$PY" -c "import ants, SimpleITK, fmriprep; print('ok: antspyx', ants.__version__, '| fmriprep', fmriprep.__version__)" || true
fi

echo
echo "environment:"
echo "  os:     $(uname -sm)"
if [[ -n "$VENV" ]]; then
  echo "  python: $("$VENV/bin/python" --version 2>&1) ($VENV)"
else
  echo "  python: $(command -v python3 >/dev/null && python3 --version 2>&1 || echo missing)"
fi
# rustup installs into ~/.cargo/bin, which may not be on PATH in this shell yet.
RUSTC="$(command -v rustc || echo "$HOME/.cargo/bin/rustc")"
echo "  rustc:  $([[ -x "$RUSTC" ]] && "$RUSTC" --version || echo 'missing (use --install-rust)')"
echo "  docker: $(command -v docker >/dev/null && docker --version || echo 'missing (needed for fMRIPrep/FreeSurfer oracles)')"
echo "  cores:  $(getconf _NPROCESSORS_ONLN 2>/dev/null || echo ?)"
[[ "$fail" == 0 ]] || { echo "some clones failed (see above)" >&2; exit 1; }
echo "done."
