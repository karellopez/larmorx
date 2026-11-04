#!/usr/bin/env bash
x=1
/usr/bin/python3 -c "import os; os.abort()" a $x
echo next $?
