[CmdletBinding()]
param([string]$OutputFile)
$ErrorActionPreference = 'Stop'
$repo = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
if (!$OutputFile) { $OutputFile = Join-Path $repo 'dist\PistonPort\PistonPort.exe' }
$target = Join-Path $repo 'launcher-rust\target'
& cargo +1.99.1 build --locked --release --bin pistonport --target-dir $target --manifest-path (Join-Path $repo 'launcher-rust\pistonport\Cargo.toml')
if ($LASTEXITCODE) { throw 'PistonPort launcher compilation failed.' }
$null = New-Item -ItemType Directory -Force ([IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($OutputFile)))
Copy-Item -LiteralPath (Join-Path $target 'release\pistonport.exe') -Destination $OutputFile
