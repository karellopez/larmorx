# SPDX-License-Identifier: Apache-2.0
<#
.SYNOPSIS
A published larmorx release checked on this Windows machine as a user installs it: no
administrator rights, nothing left behind.

.DESCRIPTION
Everything happens in one folder of the user's profile, %USERPROFILE%\larmorx-check-<version>,
which is removed at the end (-Keep leaves it): a Python of uv's (python-build-standalone,
installed into that folder, with no link in %USERPROFILE%\.local\bin and no registry entry; or
the one -Python names, or the py launcher's), pip's, uv's, Matplotlib's and TemplateFlow's caches, and the run of check_install.py (beside this script, or
taken from GitHub at the tag v<version>) with that Python: a fresh venv,
pip install "larmorx[all]==<version>" from PyPI, pip check, the smoke checks of the installed
packages and of the standalone .exe programs from the GitHub Release (every program's version,
no C runtime DLL in any of them, the machine type), and the golden tests. The report is printed;
its last line is CHECK-INSTALL ok or CHECK-INSTALL FAILED, and the exit status 0 or 1.

-FindLinks and -Archives (folders; relative ones from this script's folder), -SourceRef (a
commit, branch or tag of GitHub's archive) and -Target (the archives' target triple) check files
that are not published yet. -Tries (default 20, 30 seconds apart): how often pip install is tried
while PyPI's CDN catches up.

.EXAMPLE
powershell -NoProfile -ExecutionPolicy Bypass -File check_install_windows.ps1 -Version 0.1.0rc1
#>
param(
  [Parameter(Mandatory = $true)][string]$Version,
  [string]$PythonVersion = '3.12',
  [string]$Python,
  [string]$FindLinks,
  [string]$SourceRef,
  [string]$Archives,
  [switch]$NoArchives,
  [string]$Target,
  [int]$Tries = 20,
  [switch]$Keep
)
$ErrorActionPreference = 'Continue'
$here = $PSScriptRoot
$work = Join-Path $env:USERPROFILE "larmorx-check-$Version"
function Resolve-Here([string]$p) { if ([System.IO.Path]::IsPathRooted($p)) { $p } else { Join-Path $here $p } }
New-Item -ItemType Directory -Force -Path $work | Out-Null
foreach ($d in 'python', 'python-bin', 'uv-cache', 'pip-cache', 'matplotlib', 'templateflow', 'tmp') {
  New-Item -ItemType Directory -Force -Path (Join-Path $work $d) | Out-Null
}
# every cache and temporary file of this check inside the work folder
$env:UV_PYTHON_INSTALL_DIR = Join-Path $work 'python'
$env:UV_CACHE_DIR = Join-Path $work 'uv-cache'
$env:UV_PYTHON_PREFERENCE = 'only-managed'
# uv would also put a python3.12.exe into %USERPROFILE%\.local\bin and register the Python in
# HKCU\Software\Python\Astral (PEP 514): neither (and its links in this folder should a uv not
# know the two switches); the end of this script removes any registry entry that points here
$env:UV_PYTHON_INSTALL_BIN = '0'
$env:UV_PYTHON_INSTALL_REGISTRY = '0'
$env:UV_PYTHON_BIN_DIR = Join-Path $work 'python-bin'
$env:PIP_CACHE_DIR = Join-Path $work 'pip-cache'
$env:MPLCONFIGDIR = Join-Path $work 'matplotlib'
$env:TEMPLATEFLOW_HOME = Join-Path $work 'templateflow'
$env:TEMP = Join-Path $work 'tmp'
$env:TMP = $env:TEMP
$env:PYTHONUNBUFFERED = '1'
$env:PYTHONUTF8 = '1'
"check_install_windows.ps1: larmorx $Version in $work"
$os = Get-CimInstance Win32_OperatingSystem
"Windows: $($os.Caption) $($os.Version) (build $($os.BuildNumber)), $env:PROCESSOR_ARCHITECTURE"

$py = $null
if ($Python) {
  $py = $Python
} elseif (Get-Command uv -ErrorAction SilentlyContinue) {
  "== uv python install $PythonVersion (into $env:UV_PYTHON_INSTALL_DIR)"
  & uv python install $PythonVersion
  if ($LASTEXITCODE -eq 0) { $py = (& uv python find $PythonVersion | Select-Object -First 1) }
} elseif (Get-Command py -ErrorAction SilentlyContinue) {
  $py = (& py "-$PythonVersion" -c "import sys; print(sys.executable)" | Select-Object -First 1)
}
$code = 1
if (-not $py) {
  "No Python ${PythonVersion}: give -Python <python.exe>, or install uv or Python (python.org's installer, 'for me only', needs no administrator rights)."
  "CHECK-INSTALL FAILED: no Python"
} else {
  "python: $py"
  $checker = Join-Path $here 'check_install.py'
  if (-not (Test-Path $checker)) {
    $ref = if ($SourceRef) { $SourceRef } else { "v$Version" }
    $checker = Join-Path $work 'check_install.py'
    Invoke-WebRequest -UseBasicParsing -OutFile $checker `
      -Uri "https://raw.githubusercontent.com/karellopez/larmorx/$ref/scripts/release/check_install.py"
  }
  $arguments = @('-P', $checker, '--version', $Version, '--work', (Join-Path $work 'run'), '--tries', $Tries)
  if ($FindLinks) { $arguments += @('--find-links', (Resolve-Here $FindLinks)) }
  if ($SourceRef) { $arguments += @('--source-ref', $SourceRef) }
  if ($Archives) { $arguments += @('--archives', (Resolve-Here $Archives)) }
  if ($NoArchives) { $arguments += '--no-archives' }
  if ($Target) { $arguments += @('--target', $Target) }
  & $py @arguments
  $code = $LASTEXITCODE
}
Set-Location $env:USERPROFILE
foreach ($key in Get-ChildItem 'HKCU:\Software\Python\*' -ErrorAction SilentlyContinue) {
  foreach ($entry in Get-ChildItem $key.PSPath -ErrorAction SilentlyContinue) {
    $installed = Get-ItemProperty (Join-Path $entry.PSPath 'InstallPath') -ErrorAction SilentlyContinue
    if ($installed -and $installed.'(default)' -like "$work\*") {
      Remove-Item $entry.PSPath -Recurse
      "removed the registry entry $($key.PSChildName)\$($entry.PSChildName) (a Python of this check)"
    }
  }
}
if ($Keep) {
  "kept: $work"
} else {
  Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue
  if (Test-Path -LiteralPath $work) { "NOT removed (in use?): $work" } else { "removed: $work" }
}
exit $code
