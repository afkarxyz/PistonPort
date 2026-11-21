[CmdletBinding()]
param(
    [string]$Game,
    [string]$SpartanSource,
    [string]$ShadSource,
    [string]$LlvmBin,
    [string]$OutputDir
)
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$work = if ($env:PISTONPORT_WORK) { $env:PISTONPORT_WORK } else { Join-Path $repo 'out/work' }
if (!$Game) { $Game = $env:PISTONPORT_GAME }
if (!$Game) { throw 'Specify -Game or set PISTONPORT_GAME to your extracted dump.' }
if (!$SpartanSource) { $SpartanSource = $repo }
if (!$ShadSource) { $ShadSource = Join-Path $work 'ps4\shad-source' }
if (!$LlvmBin) { $LlvmBin = Join-Path $work 'tools\llvm-22.1.8\bin' }
if (!$OutputDir) { $OutputDir = Join-Path $work 'ps4\native' }

function Test-Revision([string]$Directory, [string]$Commit) {
    $actual = git -C $Directory rev-parse HEAD
    if ($LASTEXITCODE -ne 0 -or $actual -ne $Commit) { throw "Expected checkout $Commit in $Directory." }
}
git -C $SpartanSource merge-base --is-ancestor '6c97aa8bf38a049558879a6f1ca9ba4e01b9b80c' HEAD
if ($LASTEXITCODE) { throw 'SpartanPort base revision is missing from this checkout.' }
Test-Revision $ShadSource '11557f2eb60b3b00712c8887c30852de7615d6df'
$previousPath = $env:PYTHONPATH
$localLibraries = Join-Path $work 'ps4\python-libs'
try {
    if (Test-Path -LiteralPath $localLibraries) { $env:PYTHONPATH = "$localLibraries;$previousPath" }
    python -B (Join-Path $PSScriptRoot 'prepare.py') $Game $SpartanSource $LlvmBin $OutputDir $ShadSource
    if ($LASTEXITCODE -ne 0) { throw "Cars 3 image preparation failed ($LASTEXITCODE)." }
} finally {
    $env:PYTHONPATH = $previousPath
}
