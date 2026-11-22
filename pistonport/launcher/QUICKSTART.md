# PistonPort for Windows x64

1. Extract the entire ZIP to a folder, keeping `PistonPort.exe` beside `PistonPort.Runtime`.
2. Open `PistonPort.exe`. On the welcome screen, use **Choose PKG…** for a playable PS4 package or **Choose Folder…** for a dump containing `eboot.bin`. The launcher saves the source and opens Garage. Click **Play** to start. Supported version is **Cars 3: Driven to Win, CUSA07027, 01.00**.
3. PKG mode streams track, character and audio assets from the selected file. Keep the PKG at that location while playing. First launch temporarily stages the executable and three modules to prepare a roughly 28 MiB native image in your profile. Later launches verify and reuse it. Session metadata is removed after a normal exit. Both sources use the same save profile.

**Settings → Source & Updates** shows the selected game's PS4 Title ID and app version, lets you change the source after setup, and checks GitHub for newer PistonPort releases.

## Playing

Open **Saves** and select **Use My Progress** to start or continue a game with your own progress. Use **Import Save…** for a decrypted CUSA07027 ZIP or the **P0** file in its save folder, and **Export…** beside any slot to back up your progress and trophies as a re-importable ZIP. **All Skill Checks** is the built-in save with Smokey unlocked and 136/136 Skill Checks. The launcher creates a writable copy on first use.

The built-in save includes completed skill counters, gold event and cup results, and all 22 offline trophies. Press **F8** (or **R3 + R2** on a controller) in-game to open the Trophy & Notifications overlay with the original English names and grades read from the game.

Open **Trophies** in the launcher to see each trophy's name, tier, and status. When you import a save, PistonPort restores local trophies from the game's saved achievement flags and statistics. To restore trophies for a previously imported save, switch away from that slot and select it again.

Choose **Events → Race → Fireball Beach Sprint** after the tutorial. **F** accelerates, **A/D** steers and **E** boosts. **Space** selects and **Left Shift** goes back. The Controls tab lists the keyboard, DualShock, and Xbox inputs side by side. Change the mapping in the DualShock column to see the equivalent Xbox input. Picking an occupied button swaps the two bindings. Select **Save Controls** to apply the mapping on your next launch.

## Graphics, Ultrawide & Handheld Tips

* **Quick Presets**: Pick **Handheld (720p)**, **Balanced (1080p)**, **High Refresh (1440p)**, or **Ultra (4K)** at the top of **Settings → Display & Graphics**, then click **Save Settings**.
* **Resolution, Ultrawide & Shadows**: Choose from `720p`, `1280×800` (16:10), `900p`, `1080p`, `2560×1080` / `3440×1440` / `3840×1600` (21:9 Hor+ with centered 16:9 HUD), `5120×1440` (32:9 Hor+ with centered 16:9 HUD), or `4K`, plus **Shadow Resolution** (`1024`–`4096`), **Anisotropic Filtering** (`Game Default`–`16x`), **Camera FOV** (`100%`–`130%`), **Force High LOD**, and **Skip Intro Logos**.
* **High-FPS Motion Blur & Bloom**: **Motion Blur** is Off by default for crisp high-FPS racing (turning it On uses the fixed shutter scale without 120 FPS over-blur). If you see bloom/glow artifacts on moving cars, toggle **Bloom** to **Off**.
* **FSR 1 + RCAS & Async Shaders**: Found under **Settings → Upscaling & Hardware**; recommended for handhelds rendering at `720p`/`900p` on a `1080p` display.
* **ROG Ally / AMD Handhelds**: Leave **GPU Compatibility** on **Auto** (or pick **Safe (AMD / Handheld)**). You can also launch straight into the game from Steam Big Picture / Playnite / Armoury Crate SE with `PistonPort.exe --play`.
* **Controller & Audio**: Under **Settings → Gameplay & Audio**, configure **Button Prompts** (Auto / PlayStation / Xbox / Nintendo / Keyboard), **Controller Vibration** (`0%`–`100%`), **Master Volume**, and **Show Keyboard Hints**.
* **Troubleshooting**: Click **Open Log** or **Copy Report** in **Settings → Source & Updates** (or **Copy Crash Report** in the toast notification if the game exits unexpectedly) to grab your system info and `native-last.log`.

Frame limits are **30, 60, 100, 120, 144, 180, 240**, and **Unlimited**. The default is 60. To measure maximum throughput, select Unlimited and turn off VSync. **Show Frame Rate** displays the counter in-game.

Saves, settings, first-launch images and `native-last.log` are stored in `%LOCALAPPDATA%\PistonPort` on a standalone installation. Set `PISTONPORT_PROFILE` to choose another profile location. Keep a backup of your saves when moving between PCs.

Requires Windows x64, a compatible x86-64 CPU and a Vulkan-capable graphics driver. The launcher uses Rust and Rune. The embedded Python handles save imports, game preparation and launching the native executable.

## Credits

PistonPort uses the SpartanPort loader and shadPS4 Vulkan video core. The controller glyphs come from SpartanPort's input assets and are credited in `PistonPort.Runtime/Licenses`. The launcher's Piston Cup icon was generated with ImageGen from the supplied trophy reference. The Garage reads `sce_sys/pic1.png` from the selected game, and the game executable embeds `sce_sys/icon0.png` from the local build dump. Cars 3 and the game artwork belong to their respective owners. The launcher uses Lucide for action and navigation icons. [Lucide icons](https://github.com/lucide-icons/lucide/tree/c1a02a04d4f3f7049d44567d62b8b06cff0b9103)

The PistonPort repository contains the build sources and patches. The runtime is based on SpartanPort and the fiber implementation on shadPS4. [Build instructions](../README.md) · [SpartanPort](https://github.com/FelipePrado0/SpartanPort/tree/6c97aa8bf38a049558879a6f1ca9ba4e01b9b80c) · [shadPS4](https://github.com/shadps4-emu/shadPS4/tree/11557f2eb60b3b00712c8887c30852de7615d6df)
