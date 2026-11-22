# PistonPort

PistonPort runs *Cars 3: Driven to Win* for PS4 on Windows x64. This port supports CUSA07027 version 1.00. The supported eboot.bin SHA-256 is 56d27cc83b7fe34921fb07413dd7c8fb255daf357a3f81c2d882969f001ff654.

## Playing

Extract the Windows ZIP and open `PistonPort.exe` with `PistonPort.Runtime` beside it. Select a playable PS4 PKG or an extracted folder containing `eboot.bin` on the welcome screen. Click **Play** in Garage to prepare the native image and start the game. Use **Settings → Source & Updates** to change the location or check for new releases afterward. [Player guide](launcher/QUICKSTART.md)

The standalone launcher uses `%LOCALAPPDATA%\PistonPort`. Set `PISTONPORT_PROFILE` to select another location. Pass `PistonPort.exe --play` (or `--launch`) to jump straight into the game from Steam Big Picture, Playnite, or ROG Ally Armoury Crate SE without opening the launcher window. In the launcher, XInput controllers can switch tabs with **LB/RB**, launch with **A/Start** on Garage, and go back with **B**.

## Graphics, Ultrawide & Handheld Support

* **Quick Presets**: One-click **Handheld (720p)**, **Balanced (1080p)**, **High Refresh (1440p)**, and **Ultra (4K)** presets in **Settings → Display & Graphics**.
* **Internal Render Resolution & Ultrawide HUD**: True internal resolution patching (`720p`, `1280×800` 16:10 Steam Deck, `900p`, `1080p`, `2560×1080` 21:9, `1440p`, `3440×1440` 21:9, `3840×1600` 21:9, `5120×1440` 32:9, and `4K`) in Windowed, Borderless, and Fullscreen modes, with Hor+ 3D aspect-ratio scaling, automatic GPU Data heap expansion, and a **centered 16:9 Scaleform HUD** so menus and button prompts stay 1:1 proportional instead of stretching on ultrawide displays.
* **Shadow Resolution & LOD**: Shadow map scaling (`1024`, `2048`, `3072`, `4096`) and a **Force High LOD** toggle to reduce pop-in.
* **Motion Blur & Bloom Toggles**: Fixes Octane's high-FPS shutter multiplier (`42.0 / dt_ms`) that caused 4× over-blurred cars at 120 FPS (Off by default), plus a **Bloom** toggle (`On / Off`) that zeroes Octane's bloom weights to remove glow artifacts on moving cars.
* **Anisotropic Filtering, FSR 1 + RCAS, Async Shaders**: Configurable AF (`Game Default`–`16x`, default `16x`), FSR 1 upscaling, RCAS sharpening (`0%`–`100%`), and async shader compilation.
* **Camera FOV & Skip Intro Logos**: Adjustable gameplay camera FOV (`100%`–`130%`) and an optional **Skip Intro Logos** toggle.
* **AMD / Handheld Compatibility**: Uses the signed Windows System32 Vulkan loader (no bundled `vulkan-1.dll`), automatically enables `Safe (AMD / Handheld)` paths + commit-on-map memory allocation on AMD-only / ≤ 20 GiB RAM handhelds (such as ASUS ROG Ally Z1 Extreme), and lets you choose the Vulkan **GPU Adapter** and **Monitor** directly in **Settings → Upscaling & Hardware**.
* **Diagnostics & Updates**: Built-in GitHub release checker in **Settings → Source & Updates**, Rune toast notifications for status/errors with an inline **Copy Crash Report** action on unexpected game exits, plus **Open Log** and **Copy Report** tools.

## Save Slots

In **Saves**, select **Use My Progress** to start or continue your own game. **Import Save…** creates a separate slot from a decrypted `CUSA07027` ZIP or its `P0` file, and **Export…** backs up any slot into a re-importable ZIP archive. The built-in **All Skill Checks** save includes Smokey and 136/136 Skill Checks. It creates a writable copy on first use.

The built-in save also includes completed skill counters, gold event and cup results, and 22 local PS4 trophies in PistonPort's rethemed **F8 / R3+R2** in-game overlay.

## Frame pacing

Cars 3 requests one refresh per frame on VideoOut initialization and restoration. Its second GPU query poll takes the existing timeout path if the counters are still pending after 500 microseconds. The simulation timestep comes from Octane.

Choose 30, 60, 100, 120, 144, 180, 240 or Unlimited in Settings. Unlimited can present a ready frame between guest vblanks, with the PS4 vblank clock at 60 Hz. VSync and hardware throughput can still constrain output. The default is 60 FPS with VSync on.

| Fireball Beach Sprint | Previous 30 FPS | 60 FPS | Unlimited, VSync off |
| --- | ---: | ---: | ---: |
| Wall time (s) | 30.030 | 65.028 | 35.051 |
| Game time (s) | 30.034 | 65.008 | 35.050 |
| Engine frames | 901 | 3,901 | 6,741 |
| Frames per real second | 30.00 | 59.99 | 192.32 |

Measured on a Core i9-13900K and RTX 4070 SUPER with 1920×1080 output. The 60 FPS build recorded 60.0 FPS in all 13 five-second windows and a worst frame of 17.2 ms. A matched full-throttle run from the start stripe to a trackside flag took about 5.02 seconds at 30 FPS and 5.10 seconds at 60 FPS. SPARTAN_FRAME_STATS=1 enables frame-pacing reports in native-last.log. [60 FPS capture](../docs/images/pistonport/Fireball-Beach-Native-60FPS.png) · [60 FPS samples](benchmarks/Fireball-Beach-Native-60FPS.csv) · [30 FPS samples](benchmarks/Fireball-Beach-Native-FPS.csv)

## Building

1. Extract the PKG into a separate folder with pkgextract. The working dump used extractor revision 2fc629ae49fc265e46dac35afbf0269cccb2cc60.
2. Use Python with pyelftools 0.32, LLVM 22.1.8, the pinned [SpartanPort checkout](https://github.com/FelipePrado0/SpartanPort/tree/6c97aa8bf38a049558879a6f1ca9ba4e01b9b80c) and [shadPS4 checkout](https://github.com/shadps4-emu/shadPS4/tree/11557f2eb60b3b00712c8887c30852de7615d6df). Build.ps1 compiles the freestanding module and prepares a reference image. Use -Game, -SpartanSource, -ShadSource, -LlvmBin and -OutputDir for custom paths.
3. Configure a CLANG64 build following SpartanPort's Windows instructions. Run launcher/BuildRuntime.ps1 for the runtime and its dependencies. It uses Python with Pillow to create the Windows game icon from `sce_sys/icon0.png` in the dump. Pass `-Game` if your dump is outside the default build directory. Run launcher/BuildLauncher.ps1 to compile the Rune launcher with Rust 1.99.1 for x64 MSVC and the Windows SDK. The launcher manifest is `launcher-rust/pistonport/Cargo.toml`.
4. From pistonport/, run python launcher/package_windows.py. Set PISTONPORT_GAME to the extracted game folder. PISTONPORT_WORK selects the directory of build inputs (default: out/work in this repository). The Windows host build also requires Cargo and an x64 MSVC Rust toolchain for the read-only orbis-pkg/orbis-pfs reader. Its packaging inputs include the CPython 3.13.9 Windows x64 embeddable distribution in ps4/piston-package/python under PISTONPORT_WORK, pyelftools, the compiled port module and the source dumps' hashes. Each installation links its game image locally.

## Implementation and credits

The local image links libc.prx, libSceFios2.prx and libSceNpToolkit2.prx from the dump. The port adapts shadPS4 fibers to SpartanPort per-thread storage. Its growing event flag pool serves the Octane workers' file requests, replacing the 256-entry table that previously stalled Fireball Beach. The runtime also supports local PlayGo installation, offline user state and save metadata.

The launcher uses the selected game's artwork and the controller glyphs from SpartanPort. The app icon is an ImageGen-generated winged Piston Cup. [Icon and visual direction](launcher/Artwork.md) · [Controller art license](../launcher-rust/assets/pads/LICENSE.txt)
