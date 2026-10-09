#!/usr/bin/env bash
# Install Docker Engine on Ubuntu from Docker's official apt repository, so that the
# fMRIPrep and FreeSurfer oracles (PLAN.md §11.1) can run on this machine.
#
# Usage (needs sudo):
#   sudo larmorx/scripts/setup_docker_ubuntu.sh [user]
#
# [user] is added to the "docker" group (default: the user who invoked sudo). Log out and in
# again, or run `newgrp docker`, before using docker without sudo.
set -euo pipefail

[[ "$(id -u)" == 0 ]] || { echo "run with sudo" >&2; exit 1; }
TARGET_USER="${1:-${SUDO_USER:-}}"
. /etc/os-release
[[ "${ID:-}" == ubuntu ]] || { echo "this script supports Ubuntu only (found: ${ID:-unknown})" >&2; exit 1; }

if command -v docker >/dev/null; then
  echo "docker already installed: $(docker --version)"
else
  apt-get update
  apt-get install -y ca-certificates curl
  install -m 0755 -d /etc/apt/keyrings
  curl -fsSL https://download.docker.com/linux/ubuntu/gpg -o /etc/apt/keyrings/docker.asc
  chmod a+r /etc/apt/keyrings/docker.asc
  echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.asc] https://download.docker.com/linux/ubuntu ${UBUNTU_CODENAME:-$VERSION_CODENAME} stable" \
    > /etc/apt/sources.list.d/docker.list
  apt-get update
  apt-get install -y docker-ce docker-ce-cli containerd.io docker-buildx-plugin docker-compose-plugin
fi

systemctl enable --now docker
if [[ -n "$TARGET_USER" ]]; then
  usermod -aG docker "$TARGET_USER"
  echo "added $TARGET_USER to the docker group (log out and in again to use it)"
fi
docker run --rm hello-world >/dev/null && echo "docker works: $(docker --version)"
