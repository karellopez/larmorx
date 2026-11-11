larmorx container images (ghcr.io/karellopez/larmorx)
=====================================================

Two images per release, for linux/amd64 and linux/arm64, built from the release's own wheels:

  ghcr.io/karellopez/larmorx:<version>        larmorx with its [prep] dependencies,
                                              without the replica packages: commercial use
                                              allowed.
  ghcr.io/karellopez/larmorx:<version>-all    also larmorx-gpl (the GNU GPL version 3) and
                                              larmorx-nc (the FSL Licence: NON-COMMERCIAL USE
                                              ONLY). With larmorx-nc installed,
                                              implementation="auto" runs the FSL replicas, so
                                              FSL's terms cover the results of default calls.

"latest" and "latest-all" name the newest final release (never an alpha, beta or release
candidate).

Use
---
  apptainer pull docker://ghcr.io/karellopez/larmorx:<version>
  apptainer exec larmorx_<version>.sif larmorx --version
  docker run --rm -v "$PWD:/data" -w /data ghcr.io/karellopez/larmorx:<version> larmorx ...
  podman run --rm -v "$PWD:/data" -w /data ghcr.io/karellopez/larmorx:<version> larmorx ...

The programs: larmorx (and its short name lx), larmorprepx (a preview: not ready yet), and in the
-all image larmorx-nc and larmorx-gpl; Python 3.12 with `import larmorx`.

Under Docker the image runs as root; under Apptainer as you, with your home directory. Caches:
matplotlib's in /tmp/larmorx-matplotlib; TemplateFlow's templates are downloaded on first use
into $HOME/.cache/templateflow (TEMPLATEFLOW_HOME to choose another folder; with Docker, mount
a volume there to keep them).

In this image
-------------
  /opt/larmorx/pip-freeze.txt   every Python package installed, with its version
  /opt/larmorx/source/          (the -all image) larmorx-nc's and larmorx-gpl's source
                                distributions: the GNU GPL's Corresponding Source, and FSL's
                                original and amended source the FSL Licence asks for
  /usr/local/lib/python3.12/site-packages/larmorx*.dist-info/licenses/
                                each package's licence files (LICENSE, NOTICE, LICENSES/)

larmorx's own licence is Apache-2.0; the works it bundles and their licences are listed in its
NOTICE. The base image is Debian's python:3.12-slim (Debian 13, trixie) with its own licences.

Documentation: https://karellopez.github.io/larmorx_documentation/
Source:        https://github.com/karellopez/larmorx
