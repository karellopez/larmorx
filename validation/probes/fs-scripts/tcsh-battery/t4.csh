#!/bin/tcsh -f
# SPDX-License-Identifier: Apache-2.0
echo "0=$0"
cat /nonexistent/build-stamp.txt | tee -a log.txt
echo "st $status"
set x = `cat /nonexistent/zz | head -n 1`
echo "x=($x) $#x"
