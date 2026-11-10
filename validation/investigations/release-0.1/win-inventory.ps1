$ErrorActionPreference = 'Continue'
$os = Get-CimInstance Win32_OperatingSystem
"OS: $($os.Caption) $($os.Version) build $($os.BuildNumber) arch $($os.OSArchitecture)"
"CPU arch env: $env:PROCESSOR_ARCHITECTURE"
$admin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
"Elevated admin token: $admin"
"Admin group member: " + [bool](whoami /groups | Select-String 'S-1-5-32-544')
foreach ($d in 'vcruntime140.dll','vcruntime140_1.dll','msvcp140.dll','ucrtbase.dll') {
  "System32\$d present: " + (Test-Path "$env:WINDIR\System32\$d")
}
"VC++ redist (registry): " + ((Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\*' -ErrorAction SilentlyContinue | ForEach-Object { "$($_.PSChildName)=$($_.Version)" }) -join ', ')
"python on PATH: " + ((Get-Command python, py -ErrorAction SilentlyContinue | ForEach-Object { $_.Source }) -join '; ')
"uv: " + ((Get-Command uv -ErrorAction SilentlyContinue).Source)
"uv python dir: " + (Test-Path "$env:APPDATA\uv\python")
"cargo: " + ((Get-Command cargo -ErrorAction SilentlyContinue).Source)
"git: " + ((Get-Command git -ErrorAction SilentlyContinue).Source)
"Sandbox exe present: " + (Test-Path "$env:WINDIR\System32\WindowsSandbox.exe")
"Virtualization in firmware: " + ((Get-CimInstance Win32_Processor | Select-Object -First 1).VirtualizationFirmwareEnabled)
"Hypervisor present: " + ((Get-CimInstance Win32_ComputerSystem).HypervisorPresent)
"Free space C: " + [math]::Round((Get-PSDrive C).Free / 1GB, 1) + " GB"
"Home: $env:USERPROFILE"
