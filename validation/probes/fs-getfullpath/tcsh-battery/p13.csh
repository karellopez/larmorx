#!/bin/tcsh -f
# SPDX-License-Identifier: Apache-2.0
if(! -e $argv[1]) then
  echo "no"
else
  echo "yes"
endif
