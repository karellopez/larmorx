#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Relay 3b: the release workflow's container steps (.github/workflows/release.yml: the jobs
# `container` and `container-push`, both through scripts/release/container.py) run here, outside
# GitHub, for linux/amd64, with no administrator rights:
#   1. a rootless Docker daemon (rootless_docker.sh) with buildx's docker-container builder
#      (BuildKit pinned by digest, as on the runners);
#   2. `container.py stage`, `build` and `check`: both images built from the given Linux wheels
#      and sdists, saved as OCI archives, loaded into Docker; inside each: its labels, smoke.py as
#      root and as an unprivileged user on a read-only file system, the default command, the
#      packages, the companions' sources, the golden tests and tests/python/test_smoke.py;
#   3. Apptainer: each OCI archive turned into a SIF image (`apptainer build ... oci-archive:`),
#      then `apptainer exec` of `larmorx --version`, smoke.py and the golden tests in it (the
#      test group in a venv over the image's packages);
#   4. the push: `container.py index` (amd64 only here: arm64 is built on GitHub's arm64 runner)
#      and `container.py push --latest` into a local registry (the distribution registry, run by
#      Apptainer on 127.0.0.1) with skopeo 1.13.3 (Ubuntu 24.04's, the runner's; its own image,
#      run by Apptainer); each tag read back; then `apptainer pull docker://...` of the pushed
#      tags and `larmorx --version` (and the companions' in the -all image) in what came back.
# What only GitHub can show: the arm64 images, the runners' Docker, GHCR's login and its first
# push (the package's visibility: D17), the provenance attestations.
#
# Usage: container_rehearsal.sh <checkout> <wheels> <sdists> <out>
#   <wheels>: the folder of the three packages' Linux x86_64 wheels of the checkout's version;
#   <sdists>: the folder of larmorx-nc's and larmorx-gpl's sdists; <out>: a new folder.
# Optional: PYTHON (a Python 3.12 or newer; default python3), PORT (the registry's, 5055),
# KEEP_DOCKER=1 (leave the daemon running). Run it in a memory box:
# scripts/orchestration/boxed.sh 32 bash ... (the daemon has a box of its own).
set -u
[ $# -ge 4 ] || { sed -n '3,32p' "$0"; exit 2; }
W=$(cd "$1" && pwd)
WHEELS=$(cd "$2" && pwd)
SDISTS=$(cd "$3" && pwd)
mkdir -p "$4" && OUT=$(cd "$4" && pwd)
HERE=$(cd "$(dirname "$0")" && pwd)
PY=${PYTHON:-python3}
PORT=${PORT:-5055}
REGISTRY_IMAGE=docker://docker.io/library/registry:3.0.0@sha256:6c5666b861f3505b116bb9aa9b25175e71210414bd010d92035ff64018f9457e
SKOPEO_IMAGE=docker://quay.io/skopeo/stable:v1.13.3@sha256:4853591bd1d2a13c6fd1ede57fa234503d08a6f7a0e0f455dcffd5c920fc8fba
export APPTAINER_CACHEDIR=$OUT/apptainer-cache APPTAINER_TMPDIR=$OUT/apptainer-tmp
mkdir -p "$APPTAINER_CACHEDIR" "$APPTAINER_TMPDIR"
VERSION=$("$PY" -P "$W/scripts/release/set_version.py" --print)
SHA=$(git -C "$W" rev-parse HEAD)
EPOCH=$(git -C "$W" log -1 --format=%ct HEAD)
CONTAINER=("$PY" -P "$W/scripts/release/container.py")
FAILED=()
step() { # step <name> <command...>: run it, note a failure
  local name=$1; shift
  echo "=== $name"
  if "$@"; then echo "=== $name: ok"; else echo "=== $name: FAILED"; FAILED+=("$name"); fi
}
expect() { # expect <text> <command...>: the command's standard output is exactly <text>
  local want=$1; shift
  local got
  got=$("$@") || { echo "exit status $? from $*"; return 1; }
  [ "$got" = "$want" ] || { echo "printed '$got', expected '$want'"; return 1; }
  echo "$got"
}
echo "container rehearsal of larmorx $VERSION at $SHA"

# 1. the daemon
bash "$HERE/rootless_docker.sh" start "$OUT/docker" 32 || exit 1
eval "$(bash "$HERE/rootless_docker.sh" env "$OUT/docker")"
docker version --format 'Docker {{.Server.Version}} (rootless)'
docker buildx version

# 2. the workflow's container job
step stage "${CONTAINER[@]}" stage --version "$VERSION" --arch amd64 --wheels "$WHEELS" \
  --sdists "$SDISTS" --out "$OUT/context"
step build "${CONTAINER[@]}" build --version "$VERSION" --arch amd64 --context "$OUT/context" \
  --out "$OUT/images-amd64" --revision "$SHA" --epoch "$EPOCH"
step check "${CONTAINER[@]}" check --version "$VERSION" --out "$OUT/images-amd64" --checkout "$W"
ls -l "$OUT/images-amd64"

# 3. Apptainer, from the very archives the push would send
mkdir -p "$OUT/sif" "$OUT/venvs"
for image in larmorx larmorx-all; do
  archive=$OUT/images-amd64/$image-$VERSION-amd64.oci.tar
  sif=$OUT/sif/$image.sif
  step "apptainer build $image" apptainer build --force "$sif" "oci-archive:$archive"
  step "apptainer exec $image: larmorx --version" \
    expect "larmorx $VERSION" apptainer exec --containall "$sif" larmorx --version
  if [ "$image" = larmorx-all ]; then
    step "apptainer exec $image: larmorx-nc --version" \
      expect "larmorx-nc $VERSION" apptainer exec --containall "$sif" larmorx-nc --version
    step "apptainer exec $image: larmorx-gpl --version" \
      expect "larmorx-gpl $VERSION" apptainer exec --containall "$sif" larmorx-gpl --version
  fi
  step "apptainer exec $image: smoke.py" apptainer exec --containall \
    --bind "$W/scripts/release:/opt/release-checks:ro" "$sif" \
    python -P /opt/release-checks/smoke.py --version "$VERSION" --installed --image "$image"
  mkdir -p "$OUT/venvs/$image"
  step "apptainer exec $image: the golden tests" apptainer exec --containall \
    --bind "$W:/src:ro" --bind "$OUT/venvs/$image:/venv" --pwd /src "$sif" sh -c '
      set -e
      python -m venv --system-site-packages /venv/v
      /venv/v/bin/python -m pip install -q --upgrade pip
      /venv/v/bin/python -m pip install -q --only-binary :all: --group test
      /venv/v/bin/python -m pytest -p no:cacheprovider -q tests/golden'
done

# 4. the workflow's container-push job, into a local registry
step index "${CONTAINER[@]}" index --version "$VERSION" --images "$OUT/images-amd64" \
  --out "$OUT/layout" --arches amd64
mkdir -p "$OUT/registry" "$OUT/bin"
apptainer exec --containall --env "REGISTRY_HTTP_ADDR=127.0.0.1:$PORT" \
  --bind "$OUT/registry:/var/lib/registry" "$REGISTRY_IMAGE" \
  /bin/registry serve /etc/distribution/config.yml >"$OUT/registry.log" 2>&1 &
REGISTRY_PID=$!
for _ in $(seq 1 60); do curl -fs "http://127.0.0.1:$PORT/v2/" >/dev/null && break; sleep 1; done
cat >"$OUT/bin/skopeo" <<EOF
#!/bin/bash
exec apptainer exec --bind "$OUT" "$SKOPEO_IMAGE" skopeo "\$@"
EOF
chmod +x "$OUT/bin/skopeo"
"$OUT/bin/skopeo" --version
REPO=127.0.0.1:$PORT/karellopez/larmorx
step push env PATH="$OUT/bin:$PATH" "${CONTAINER[@]}" push --layout "$OUT/layout" \
  --repository "$REPO" --latest --no-tls-verify
"$OUT/bin/skopeo" inspect --raw --tls-verify=false "docker://$REPO:$VERSION" >"$OUT/pushed-index.json"
echo "the pushed index: $(cat "$OUT/pushed-index.json")"
for tag in "$VERSION" "$VERSION-all" latest latest-all; do
  rm -f "$OUT/sif/pulled-$tag.sif"
  step "apptainer pull $tag" apptainer pull --no-https "$OUT/sif/pulled-$tag.sif" "docker://$REPO:$tag"
  step "pulled $tag: larmorx --version" \
    expect "larmorx $VERSION" apptainer exec --containall "$OUT/sif/pulled-$tag.sif" larmorx --version
done
step "pulled $VERSION-all: larmorx-nc --version" expect "larmorx-nc $VERSION" \
  apptainer exec --containall "$OUT/sif/pulled-$VERSION-all.sif" larmorx-nc --version
step "pulled $VERSION: no larmorx-nc" expect "absent" apptainer exec --containall \
  "$OUT/sif/pulled-$VERSION.sif" sh -c 'command -v larmorx-nc || echo absent'
kill "$REGISTRY_PID" 2>/dev/null
wait "$REGISTRY_PID" 2>/dev/null
[ "${KEEP_DOCKER:-0}" = 1 ] || bash "$HERE/rootless_docker.sh" stop "$OUT/docker"
du -sh "$OUT/images-amd64"/*.oci.tar "$OUT/sif"/larmorx*.sif
if [ ${#FAILED[@]} -eq 0 ]; then
  echo "CONTAINER-REHEARSAL ok"
else
  echo "CONTAINER-REHEARSAL FAILED: ${FAILED[*]}"
  exit 1
fi
