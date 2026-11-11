#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Relay 3b: a rootless Docker daemon for the container rehearsal (container_rehearsal.sh), with
# no administrator rights: Docker's rootless extras (dockerd-rootless.sh, rootlesskit,
# slirp4netns) and the account's subordinate ids (/etc/subuid, /etc/subgid). Its images and
# containers live in <state>/data, its sockets in $XDG_RUNTIME_DIR/larmorx-rehearsal-docker (a
# socket's path must stay under 108 characters); nothing of the system's Docker is touched.
#
# Usage: rootless_docker.sh start <state> [<GiB>]   (in a memory box of <GiB>, 32 by default)
#        rootless_docker.sh stop <state>
#        rootless_docker.sh env <state>              (prints the client's DOCKER_HOST and DOCKER_CONFIG)
# The daemon's log: <state>/dockerd.log. Its client: `eval "$(rootless_docker.sh env <state>)"`.
set -u
[ $# -ge 2 ] || { sed -n '3,13p' "$0"; exit 2; }
HERE=$(cd "$(dirname "$0")" && pwd)
ACTION=$1
mkdir -p "$2" && STATE=$(cd "$2" && pwd)
: "${XDG_RUNTIME_DIR:?XDG_RUNTIME_DIR must be set}"
RUN=$XDG_RUNTIME_DIR/larmorx-rehearsal-docker
SOCK=$RUN/docker.sock
case $ACTION in
env)
  echo "export DOCKER_HOST=unix://$SOCK DOCKER_CONFIG=$STATE/config"
  ;;
start)
  [ -S "$SOCK" ] && DOCKER_HOST=unix://$SOCK docker info >/dev/null 2>&1 && {
    echo "rootless_docker: already running ($SOCK)"; exit 0; }
  mkdir -p "$RUN" "$STATE/data" "$STATE/config"
  export DOCKERD_ROOTLESS_ROOTLESSKIT_STATE_DIR=$RUN/rootlesskit
  # setsid: the daemon outlives this script; the box (a systemd user scope) holds its memory
  setsid nohup "$HERE/../../../scripts/orchestration/boxed.sh" "${3:-32}" dockerd-rootless.sh \
    --data-root "$STATE/data" --exec-root "$RUN/exec" --pidfile "$RUN/docker.pid" \
    --host "unix://$SOCK" >"$STATE/dockerd.log" 2>&1 </dev/null &
  for _ in $(seq 1 60); do
    DOCKER_HOST=unix://$SOCK docker info >/dev/null 2>&1 && {
      echo "rootless_docker: running ($SOCK)"; exit 0; }
    sleep 1
  done
  echo "rootless_docker: the daemon did not start; its log:" >&2
  tail -30 "$STATE/dockerd.log" >&2
  exit 1
  ;;
stop)
  if [ -f "$RUN/docker.pid" ]; then
    kill "$(cat "$RUN/docker.pid")" 2>/dev/null
    for _ in $(seq 1 30); do [ -S "$SOCK" ] || break; sleep 1; done
  fi
  # rootlesskit's own process (the parent of dockerd's namespace)
  if [ -f "$RUN/rootlesskit/child_pid" ]; then kill "$(cat "$RUN/rootlesskit/child_pid")" 2>/dev/null; fi
  echo "rootless_docker: stopped"
  ;;
*)
  sed -n '3,13p' "$0"; exit 2
  ;;
esac
