#!/bin/tcsh -f
# SPDX-License-Identifier: Apache-2.0
set d = ( $argv:q )
if(! -e $d) then
  echo "no"
else
  echo "yes"
endif
