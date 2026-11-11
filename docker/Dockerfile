# SPDX-License-Identifier: Apache-2.0
# The container images of a larmorx release, ghcr.io/karellopez/larmorx, for linux/amd64 and
# linux/arm64. .github/workflows/release.yml builds them on each architecture's own runner from
# the release's own Linux wheels (scripts/release/container.py: the build context, the build,
# the checks inside, the push after PyPI took the files); RELEASING.md has the maintainer's steps.
#
#   --target larmorx      -> ghcr.io/karellopez/larmorx:<version>
#       larmorx with its [prep] dependencies, without the replica packages: commercial use allowed.
#   --target larmorx-all  -> ghcr.io/karellopez/larmorx:<version>-all
#       also larmorx-gpl (the GNU GPL version 3) and larmorx-nc (the FSL Licence: NON-COMMERCIAL
#       USE ONLY), with their source distributions in /opt/larmorx/source/. With larmorx-nc
#       installed, implementation="auto" runs the FSL replicas: FSL's terms then cover the results
#       of the default calls. That is why the two images are separate.
#
# The build context (container.py stages it): wheels/ (this architecture's wheels of the three
# packages), source/ (the companions' sdists), README.txt (docker/README.txt). Build arguments:
# VERSION (PEP 440), REVISION (the commit), CREATED (the commit's time, RFC 3339).
#
# No ENTRYPOINT, so `docker run <image> larmorx ...` and `apptainer exec <image> larmorx ...`
# both run the command they are given; the default command prints larmorx's help. The image
# runs as root under Docker and as the calling user under Apptainer: everything is readable by
# all, and the caches go to /tmp or the user's home.

# python:3.12.16-slim-trixie (Debian 13), the multi-architecture index pinned by its digest.
ARG BASE=docker.io/library/python:3.12.16-slim-trixie@sha256:5e2bdb1b0d542d5039cffb8181c65930093949194a65c365dfb1b0fe13a4fa5b

FROM ${BASE} AS larmorx
ARG BASE
ARG VERSION
ARG REVISION
ARG CREATED
ENV PYTHONDONTWRITEBYTECODE=1 \
    PYTHONUNBUFFERED=1 \
    PIP_NO_CACHE_DIR=1 \
    PIP_DISABLE_PIP_VERSION_CHECK=1 \
    PIP_ROOT_USER_ACTION=ignore \
    MPLCONFIGDIR=/tmp/larmorx-matplotlib
# The release's own wheel; its [prep] dependencies (numpy, matplotlib, nilearn, pandas, jinja2,
# templateflow and theirs) from PyPI, as `pip install "larmorx[prep]"` gets them: wheels, and a
# pure-Python sdist built where a package has no wheel (docopt, for pybids' num2words; with
# --only-binary :all: pip would fall back to pybids 0.18.1). /opt/larmorx/pip-freeze.txt lists
# what is installed.
RUN --mount=type=bind,source=wheels,target=/wheels \
    set -eu; \
    test -n "$VERSION"; \
    wheel=$(echo /wheels/larmorx-"$VERSION"-*.whl); \
    test -f "$wheel"; \
    python -m pip install "$wheel[prep]"; \
    python -m pip check; \
    mkdir -p /opt/larmorx; \
    python -m pip list --format=freeze > /opt/larmorx/pip-freeze.txt; \
    larmorx --version
COPY README.txt /opt/larmorx/README.txt
LABEL org.opencontainers.image.title="larmorx" \
      org.opencontainers.image.description="larmorx: neuroimaging tools reimplemented in Rust, with Python bindings and the original programs' command lines (without the replica packages: commercial use allowed)" \
      org.opencontainers.image.version="${VERSION}" \
      org.opencontainers.image.revision="${REVISION}" \
      org.opencontainers.image.created="${CREATED}" \
      org.opencontainers.image.source="https://github.com/karellopez/larmorx" \
      org.opencontainers.image.url="https://karellopez.github.io/larmorx_documentation/" \
      org.opencontainers.image.documentation="https://karellopez.github.io/larmorx_documentation/" \
      org.opencontainers.image.licenses="Apache-2.0" \
      org.opencontainers.image.base.name="${BASE}"
CMD ["larmorx", "--help"]

FROM larmorx AS larmorx-all
ARG VERSION
# The companions' wheels of this release, no other download; their sdists beside them (the GNU
# GPL's Corresponding Source, and the FSL Licence's original and amended source).
RUN --mount=type=bind,source=wheels,target=/wheels \
    set -eu; \
    nc=$(echo /wheels/larmorx_nc-"$VERSION"-*.whl); \
    gpl=$(echo /wheels/larmorx_gpl-"$VERSION"-*.whl); \
    test -f "$nc" && test -f "$gpl"; \
    python -m pip install --no-index --no-deps "$nc" "$gpl"; \
    python -m pip check; \
    python -m pip list --format=freeze > /opt/larmorx/pip-freeze.txt; \
    larmorx-nc --version; \
    larmorx-gpl --version
COPY source/ /opt/larmorx/source/
LABEL org.opencontainers.image.title="larmorx-all" \
      org.opencontainers.image.description="larmorx with larmorx-gpl (GNU GPL v3) and larmorx-nc (the FSL Licence: NON-COMMERCIAL USE ONLY; with it implementation=\"auto\" runs the FSL replicas)" \
      org.opencontainers.image.licenses="Apache-2.0 AND GPL-3.0-only AND LicenseRef-FSL"
