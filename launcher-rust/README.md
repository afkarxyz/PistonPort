# PistonPort Launcher (Rust / Rune)

Native Windows x64 launcher for PistonPort, built with Rust and [Rune](https://github.com/anomalyco/rune).

- Manifest: `pistonport/Cargo.toml`
- Source: `src/bin/pistonport/`
- Controller assets: `assets/pads/`

## Building

Install Rust (x64 MSVC toolchain) and the Windows SDK, then run from this directory:

```powershell
cd pistonport
cargo build --release
```

Or use `pistonport/launcher/BuildLauncher.ps1` from the repository root.
