# SPDX-License-Identifier: Apache-2.0
# The installation page's PowerShell check of a download against SHA256SUMS (relay 5b, 2026-11-11):
# True for the file, False after changing it; its temporary folder removed (Test-Path then False).
# Run on the Windows helper with ../winps.sh <this file>.
$d = Join-Path $env:TEMP "lx-5b-sumtest2"
New-Item -ItemType Directory -Force $d | Out-Null
Set-Location $d
Set-Content -NoNewline -Path "larmorx-0.1.0rc1-x86_64-pc-windows-msvc.zip" -Value "a`n"
Set-Content -NoNewline -Path "larmorx-nc-0.1.0rc1-x86_64-pc-windows-msvc.zip" -Value "b`n"
$h1 = (Get-FileHash "larmorx-0.1.0rc1-x86_64-pc-windows-msvc.zip").Hash.ToLower()
$h2 = (Get-FileHash "larmorx-nc-0.1.0rc1-x86_64-pc-windows-msvc.zip").Hash.ToLower()
[IO.File]::WriteAllText("$d\SHA256SUMS", "$h2  larmorx-nc-0.1.0rc1-x86_64-pc-windows-msvc.zip`n$h1  larmorx-0.1.0rc1-x86_64-pc-windows-msvc.zip`n")
$f = "larmorx-0.1.0rc1-x86_64-pc-windows-msvc.zip"
$sum = (Select-String -Path SHA256SUMS -SimpleMatch "  $f").Line.Split(" ")[0]
(Get-FileHash $f).Hash -eq $sum
Set-Content -NoNewline -Path $f -Value "x"
(Get-FileHash $f).Hash -eq $sum
Set-Location $env:TEMP
Remove-Item -Recurse -Force $d
Test-Path $d
