[CmdletBinding()]
param([string]$Source, [string]$Build, [string]$ToolBin, [string]$LlvmBin, [string]$Destination, [string]$Game)
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$work = if ($env:PISTONPORT_WORK) { $env:PISTONPORT_WORK } else { Join-Path $repo 'out/work' }
if (!$Source) { $Source = $repo }
if (!$Build) { $Build = Join-Path $Source 'out/gpu' }
if (!$ToolBin) { $ToolBin = 'C:/msys64/clang64/bin' }
if (!$LlvmBin) { $LlvmBin = $ToolBin }
if (!$Destination) { $Destination = Join-Path $work 'ps4/piston-runtime' }
if (!$Game) { $Game = if ($env:PISTONPORT_GAME) { $env:PISTONPORT_GAME } else { Join-Path $work 'ps4/CUSA07027' } }
if ((Resolve-Path $Source).Path -ne $repo) {
    $runtimeFiles = @(
        'packaging/windows/spartanport-probe.rc',
        'gpu/CMakeLists.txt',
        'gpu/shim/pistonport_hint_icons.cmake',
        'gpu/shim/spartanportgpu.cpp',
        'gpu/shim/spartanport_overlay.cpp',
        'gpu/shim/spartanport_settings.cpp',
        'gpu/shim/spartanport_settings.h',
        'gpu/spartanportgpu.h',
        'gpu/tools/overlay_preview.cpp',
        'launcher-rust/assets/pads/dualshock4/leftstick.png',
        'src/runtime_pad.c',
        'tests/test_pad_map.c'
    )
    $padIcons = 'launcher-rust/assets/pads/icons/playstation'
    $runtimeFiles += Get-ChildItem -LiteralPath "$repo/$padIcons" -File -Filter '*.png' |
        ForEach-Object { "$padIcons/$($_.Name)" }
    $options = $runtimeFiles | ForEach-Object { "--exclude=$_" }
    foreach ($name in 'pistonport-runtime.patch','pistonport-pkg-runtime.patch','pistonport-window-icon.patch','pistonport-trophy-overlay.patch') {
        $patch = Join-Path $PSScriptRoot $name
        # Sync these files directly so previously patched source checkouts can update.
        git -C $Source apply @options --reverse --check --quiet $patch
        if ($LASTEXITCODE -ne 0) {
            git -C $Source apply @options $patch
            if ($LASTEXITCODE) { throw "Could not apply $name." }
        }
    }
    Copy-Item -LiteralPath "$repo/src/piston_pkg_bridge.h" -Destination "$Source/src/piston_pkg_bridge.h"
    foreach ($file in $runtimeFiles) {
        if (!(Test-Path -LiteralPath "$Source/$file") -or
            (Get-FileHash "$repo/$file").Hash -ne (Get-FileHash "$Source/$file").Hash) {
            Copy-Item -LiteralPath "$repo/$file" -Destination "$Source/$file"
        }
    }
}
$icon = Join-Path $Source 'launcher/piston-game.ico'
$previous = if (Test-Path -LiteralPath $icon) { (Get-FileHash -LiteralPath $icon).Hash }
& python "$PSScriptRoot\make_icon.py" --game-icon (Join-Path $Game 'sce_sys/icon0.png') $icon
if ($LASTEXITCODE) { throw 'Game icon conversion failed.' }
if ((Get-FileHash -LiteralPath $icon).Hash -ne $previous) {
    (Get-Item "$Source\packaging\windows\spartanport-probe.rc").LastWriteTime = Get-Date
}
$oldPath = $env:Path
try {
    $env:Path = "$ToolBin;$oldPath"
    & "$ToolBin\cmake.exe" --build $Build --target spartanport-probe
    if ($LASTEXITCODE) { throw 'Runtime build failed.' }
    $null = New-Item -ItemType Directory -Force $Destination
    $cargoTarget = Join-Path $Build 'pkg-reader'
    cargo build --locked --release --manifest-path "$repo/pistonport/pkg/Cargo.toml" --target-dir $cargoTarget
    if ($LASTEXITCODE) { throw 'PKG reader build failed.' }
    Copy-Item -LiteralPath "$cargoTarget/release/piston_pkg.dll" -Destination $Destination
    $hostFile = Join-Path $Destination 'PistonPort.Game.exe'
    Copy-Item -LiteralPath "$Build\spartanport-probe.exe" -Destination $hostFile
    $queue = New-Object 'System.Collections.Generic.Queue[string]'
    $seen = @{}
    $queue.Enqueue($hostFile)
    while ($queue.Count) {
        $file = $queue.Dequeue()
        $imports = & "$LlvmBin\llvm-readobj.exe" --coff-imports $file
        if ($LASTEXITCODE) { throw "Could not read dependencies for $file" }
        foreach ($line in $imports) {
            if ($line -notmatch '^\s*Name: ([^\\/]+\.dll)\s*$') { continue }
            $name = $matches[1]
            $library = Join-Path $ToolBin $name
            if ($seen.ContainsKey($name) -or !(Test-Path -LiteralPath $library)) { continue }
            $seen[$name] = $true
            $copy = Join-Path $Destination $name
            Copy-Item -LiteralPath $library -Destination $copy
            $queue.Enqueue($copy)
        }
    }
} finally { $env:Path = $oldPath }
