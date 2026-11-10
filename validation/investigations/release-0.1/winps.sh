#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Run a PowerShell script file on win-laptop (login shell cmd.exe; the script sent encoded).
# Usage: winps.sh <script.ps1>
enc=$(iconv -t UTF-16LE < "$1" | base64 -w0)
exec timeout 120 ssh -o BatchMode=yes -o ConnectTimeout=8 win-laptop "powershell -NoProfile -NonInteractive -EncodedCommand $enc"
