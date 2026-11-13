#!/bin/sh
# SPDX-License-Identifier: Apache-2.0
# Runs <program> <command> with <input file> as its standard input, so that the oracle-output cache
# (which passes no standard input) keys the program and the input by their content: both are
# arguments naming existing files. Alone in its folder, the cache's identity of this script is
# this file only. Usage: run_stdin.sh <program> <command> <input file>
exec "$1" "$2" < "$3"
