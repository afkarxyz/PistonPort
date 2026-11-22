use std::{
    env, fmt, fs, io,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Output},
};

#[derive(Clone, Debug)]
pub struct Paths {
    pub profile: PathBuf,
    runtime: PathBuf,
    output: PathBuf,
}

#[derive(Clone)]
pub struct SourceInspection {
    pub identity: String,
    pub cover: Vec<u8>,
}

impl Paths {
    pub fn discover() -> io::Result<Self> {
        let output = env::current_exe()?
            .parent()
            .ok_or_else(|| io::Error::other("Launcher directory missing"))?
            .to_owned();
        let profile = env::var_os("PISTONPORT_PROFILE")
            .filter(|path| !path.is_empty())
            .map(PathBuf::from)
            .or_else(|| env::var_os("LOCALAPPDATA").map(|p| PathBuf::from(p).join("PistonPort")))
            .ok_or_else(|| {
                io::Error::other("Set PISTONPORT_PROFILE to choose a profile folder.")
            })?;
        fs::create_dir_all(&profile)?;
        Ok(Self {
            profile: std::path::absolute(profile)?,
            runtime: output.join("PistonPort.Runtime"),
            output,
        })
    }

    fn python(&self, script: &str) -> Command {
        let mut command = Command::new(self.runtime.join("python/python.exe"));
        command
            .arg("-B")
            .arg(self.runtime.join("tools").join(script))
            .current_dir(&self.output);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            use windows_sys::Win32::System::Threading::CREATE_NO_WINDOW;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        command
    }

    pub fn inspect(&self, path: &str) -> Result<SourceInspection, String> {
        let mut command = self.python("boot.py");
        command.args(["--source-info", "--game", path]);
        let data = run(command)?;
        let line_end = data
            .iter()
            .position(|b| *b == b'\n')
            .ok_or("Invalid game metadata from the runtime.")?;
        let (title_id, version) = std::str::from_utf8(&data[..line_end])
            .ok()
            .and_then(|header| header.split_once('\t'))
            .ok_or("Invalid game metadata from the runtime.")?;
        let identity = if version.is_empty() {
            title_id.to_owned()
        } else {
            format!("{title_id}  ·  v{version}")
        };
        Ok(SourceInspection {
            identity,
            cover: data[line_end + 1..].to_vec(),
        })
    }

    pub fn inspect_trophies(&self, path: &str) -> Result<Vec<u8>, String> {
        let mut command = self.python("boot.py");
        command.args(["--inspect-trophies", "--game", path]);
        run(command)
    }

    pub fn save_action(&self, action: &str, value: &Path) -> Result<Vec<u8>, String> {
        let mut command = self.python("save_slots.py");
        command
            .arg("--profile")
            .arg(&self.profile)
            .arg(action)
            .arg(value);
        run(command)
    }

    pub fn export_save(&self, slot: &str, dest: &Path) -> Result<Vec<u8>, String> {
        let mut command = self.python("save_slots.py");
        command
            .arg("--profile")
            .arg(&self.profile)
            .arg("--export-save")
            .arg(slot)
            .arg(dest);
        run(command)
    }

    pub fn list_hardware(&self) -> HardwareInfo {
        let mut command = self.python("boot.py");
        command.arg("--list-hardware");
        let Ok(bytes) = run(command) else {
            return HardwareInfo::default();
        };
        let mut info = HardwareInfo::default();
        for line in String::from_utf8_lossy(&bytes).lines() {
            let mut parts = line.splitn(3, '\t');
            let (Some(kind), Some(id), Some(label)) = (parts.next(), parts.next(), parts.next())
            else {
                continue;
            };
            match kind {
                "GPU" => info.gpus.push((id.to_owned(), label.to_owned())),
                "MON" => info.monitors.push((id.to_owned(), label.to_owned())),
                _ => {}
            }
        }
        info
    }

    pub fn crash_report(&self) -> Result<String, String> {
        let mut command = self.python("boot.py");
        command.arg("--crash-report").arg("--profile").arg(&self.profile);
        let bytes = run(command)?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    pub fn check_update(&self, current_version: &str) -> Result<UpdateInfo, String> {
        let mut command = self.python("boot.py");
        command.args(["--check-update", "--current-version", current_version]);
        let bytes = run(command)?;
        let text = String::from_utf8_lossy(&bytes);
        let line = text
            .lines()
            .find(|l| l.starts_with("UPDATE\t"))
            .ok_or_else(|| "Unexpected update response from runtime.".to_owned())?;
        let mut parts = line.splitn(5, '\t');
        let _ = parts.next();
        let available = parts.next() == Some("1");
        let tag = parts.next().unwrap_or_default().trim().to_owned();
        let url = parts.next().unwrap_or_default().trim().to_owned();
        let title = parts.next().unwrap_or(&tag).trim().to_owned();
        if tag.is_empty() {
            return Err("Missing release tag from GitHub Releases.".into());
        }
        Ok(UpdateInfo {
            available,
            tag,
            url,
            title,
        })
    }

    pub fn open_log(&self) -> Result<(), String> {
        let log = self.profile.join("native-last.log");
        if !log.is_file() {
            return Err("No game log exists yet. Launch the game once first.".into());
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            use windows_sys::Win32::System::Threading::CREATE_NO_WINDOW;
            Command::new("cmd")
                .args(["/C", "start", ""])
                .arg(&log)
                .creation_flags(CREATE_NO_WINDOW)
                .spawn()
                .map_err(|e| format!("Could not open game log: {e}"))?;
            Ok(())
        }
        #[cfg(not(windows))]
        {
            Err(format!("Game log: {}", log.display()))
        }
    }

    pub fn migrate_retired_save_selection(&self) -> Result<(), String> {
        let selected = read(&self.profile.join("selected-save.txt"));
        if matches!(selected.as_str(), "builtin-smokey" | "builtin-complete") {
            self.save_action("--select", Path::new(&selected))?;
        }
        Ok(())
    }

    pub fn play(&self, game: &str) -> Result<Vec<u8>, String> {
        let mut command = self.python("boot.py");
        command
            .args(["--game", game, "--profile"])
            .arg(&self.profile);
        run(command)
    }
}

fn run(mut command: Command) -> Result<Vec<u8>, String> {
    let Output {
        status,
        stdout,
        stderr,
    } = command.output().map_err(|error| {
        format!("Could not start the bundled runtime. Extract the entire PistonPort ZIP. {error}")
    })?;
    if status.success() {
        return Ok(stdout);
    }
    let detail = String::from_utf8_lossy(&stderr);
    let detail = detail.trim();
    Err(if detail.is_empty() {
        format!("The runtime exited with {status}.")
    } else {
        detail.to_owned()
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Slot {
    pub id: String,
    pub label: String,
}

impl Slot {
    fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
        }
    }

    pub fn is_builtin(&self) -> bool {
        self.id.starts_with("builtin-")
    }
}

impl fmt::Display for Slot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.label)
    }
}

pub fn slots(profile: &Path) -> (Vec<Slot>, Slot) {
    let active = read(&profile.join("selected-save.txt"));
    let active = if active.is_empty() {
        "default"
    } else {
        &active
    };
    let mut choices = vec![
        Slot::new("default", "My Progress"),
        Slot::new("builtin-complete-v2", "100% Complete (Built-In)"),
    ];
    if let Ok(entries) = fs::read_dir(profile.join("save-slots")) {
        let mut imports: Vec<_> = entries.flatten().collect();
        imports.sort_by_key(|e| (e.metadata().and_then(|m| m.created()).ok(), e.file_name()));
        for entry in imports {
            let id = entry.file_name();
            let Some(id) = id.to_str() else { continue };
            if id.len() != 32
                || !id
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            {
                continue;
            }
            let mut label = read(&entry.path().join("name.txt"));
            if label.is_empty() {
                continue;
            }
            if choices.iter().any(|slot| slot.label == label) {
                label = format!("{} ({})", label, &id[..6]);
            }
            choices.push(Slot::new(id, &label));
        }
    }
    let selected = choices
        .iter()
        .find(|slot| slot.id == active)
        .cloned()
        .unwrap_or_else(|| Slot::new(active, "Missing Save Slot"));
    (choices, selected)
}

pub fn read(path: &Path) -> String {
    fs::read_to_string(path)
        .unwrap_or_default()
        .trim_matches('\u{feff}')
        .trim()
        .to_owned()
}

pub fn write(path: &Path, content: &str) -> io::Result<()> {
    let directory = path
        .parent()
        .ok_or_else(|| io::Error::other("Missing profile folder"))?;
    let mut staged = tempfile::NamedTempFile::new_in(directory)?;
    staged.write_all(content.as_bytes())?;
    staged.persist(path).map_err(|e| e.error)?;
    Ok(())
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HardwareInfo {
    pub gpus: Vec<(String, String)>,
    pub monitors: Vec<(String, String)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateInfo {
    pub available: bool,
    pub tag: String,
    pub url: String,
    pub title: String,
}

pub fn open_url(url: &str) -> Result<(), String> {
    if !url.starts_with("https://github.com/")
        && !url.starts_with("https://afkarxyz.gumroad.com/")
    {
        return Err("Refusing to open untrusted URL.".into());
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        use windows_sys::Win32::System::Threading::CREATE_NO_WINDOW;
        Command::new("rundll32.exe")
            .args(["url.dll,FileProtocolHandler", url])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| format!("Could not open browser: {e}"))?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        Err(format!("Release page: {url}"))
    }
}

pub fn copy_to_clipboard(text: &str) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        use std::process::Stdio;
        use windows_sys::Win32::System::Threading::CREATE_NO_WINDOW;
        let mut child = Command::new("clip.exe")
            .stdin(Stdio::piped())
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| format!("Could not access clipboard: {e}"))?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(text.as_bytes())
                .map_err(|e| format!("Could not write crash report to clipboard: {e}"))?;
        }
        let status = child
            .wait()
            .map_err(|e| format!("Clipboard command failed: {e}"))?;
        if status.success() {
            Ok(())
        } else {
            Err("Clipboard command exited with an error.".into())
        }
    }
    #[cfg(not(windows))]
    {
        let _ = text;
        Err("Clipboard copy is only available on Windows.".into())
    }
}

pub const MODES: &[Choice] = &[
    Choice("windowed", "Windowed"),
    Choice("borderless", "Borderless"),
    Choice("fullscreen", "Fullscreen"),
];
pub const SIZES: &[Choice] = &[
    Choice("720p", "1280 × 720"),
    Choice("1280x800", "1280 × 800 (16:10)"),
    Choice("900p", "1600 × 900"),
    Choice("native", "1920 × 1080"),
    Choice("2560x1080", "2560 × 1080 (21:9)"),
    Choice("1440p", "2560 × 1440"),
    Choice("3440x1440", "3440 × 1440 (21:9)"),
    Choice("3840x1600", "3840 × 1600 (21:9)"),
    Choice("5120x1440", "5120 × 1440 (32:9)"),
    Choice("4K", "3840 × 2160"),
];
pub const SHADOW_SIZES: &[Choice] = &[
    Choice("1024", "Low (1024)"),
    Choice("2048", "Default (2048)"),
    Choice("3072", "High (3072)"),
    Choice("4096", "Ultra (4096)"),
];
pub const ANISOTROPIES: &[Choice] = &[
    Choice("0", "Game Default"),
    Choice("2", "2x"),
    Choice("4", "4x"),
    Choice("8", "8x"),
    Choice("16", "16x"),
];
pub const COMPAT_MODES: &[Choice] = &[
    Choice("auto", "Auto"),
    Choice("safe", "Safe (AMD / Handheld)"),
    Choice("standard", "Standard"),
];
pub const FOV_SCALES: &[Choice] = &[
    Choice("100", "Default (100%)"),
    Choice("110", "Wide (+10%)"),
    Choice("120", "Extra Wide (+20%)"),
    Choice("130", "Ultra Wide (+30%)"),
];
pub const GLYPH_STYLES: &[Choice] = &[
    Choice("auto", "Auto"),
    Choice("off", "PlayStation"),
    Choice("xbox", "Xbox"),
    Choice("nintendo", "Nintendo"),
    Choice("keyboard", "Keyboard"),
];
pub const RATES: &[Choice] = &[
    Choice("30", "30 FPS"),
    Choice("60", "60 FPS"),
    Choice("100", "100 FPS"),
    Choice("120", "120 FPS"),
    Choice("144", "144 FPS"),
    Choice("180", "180 FPS"),
    Choice("240", "240 FPS"),
    Choice("0", "Unlimited"),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preset {
    Handheld,
    Balanced,
    HighRefresh,
    Ultra,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Choice(pub &'static str, pub &'static str);

impl fmt::Display for Choice {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.1)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    pub mode: Choice,
    pub size: String,
    pub shadow: Choice,
    pub anisotropy: Choice,
    pub motion_blur: bool,
    pub bloom: bool,
    pub fov: Choice,
    pub skip_intro: bool,
    pub high_lod: bool,
    pub fsr1: bool,
    pub rcas: bool,
    pub rcas_strength: u16,
    pub async_shaders: bool,
    pub vibration: u16,
    pub glyph_style: Choice,
    pub gpu_id: String,
    pub monitor: String,
    pub compat: Choice,
    pub rate: Choice,
    pub vsync: bool,
    pub fps: bool,
    pub keyboard_hints: bool,
    pub volume: u16,
}

fn ini_field(line: &str) -> Option<(&str, &str)> {
    let (key, value) = line.split_once('=')?;
    let key = key.trim();
    (!key.starts_with('#')).then_some((key, value.trim()))
}

fn config_file(profile: &Path) -> PathBuf {
    let primary = profile.join("pistonport.ini");
    if primary.is_file() {
        primary
    } else {
        profile.join("spartanport.ini")
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            mode: MODES[0],
            size: "native".into(),
            shadow: SHADOW_SIZES[1],
            anisotropy: ANISOTROPIES[4],
            motion_blur: false,
            bloom: true,
            fov: FOV_SCALES[0],
            skip_intro: false,
            high_lod: false,
            fsr1: false,
            rcas: false,
            rcas_strength: 75,
            async_shaders: true,
            vibration: 100,
            glyph_style: GLYPH_STYLES[0],
            gpu_id: "auto".into(),
            monitor: String::new(),
            compat: COMPAT_MODES[0],
            rate: RATES[1],
            vsync: true,
            fps: true,
            keyboard_hints: false,
            volume: 100,
        }
    }
}

impl Settings {
    pub fn apply_preset(&mut self, preset: Preset) {
        match preset {
            Preset::Handheld => {
                self.mode = MODES[1];
                self.size = "720p".into();
                self.shadow = SHADOW_SIZES[0];
                self.anisotropy = ANISOTROPIES[3];
                self.rate = RATES[1];
                self.fsr1 = true;
                self.rcas = true;
                self.rcas_strength = 75;
                self.async_shaders = true;
                self.motion_blur = false;
                self.high_lod = false;
                self.compat = COMPAT_MODES[1];
            }
            Preset::Balanced => {
                self.size = "native".into();
                self.shadow = SHADOW_SIZES[1];
                self.anisotropy = ANISOTROPIES[4];
                self.rate = RATES[1];
                self.fsr1 = false;
                self.rcas = false;
                self.async_shaders = true;
                self.motion_blur = false;
                self.high_lod = false;
                self.compat = COMPAT_MODES[0];
            }
            Preset::HighRefresh => {
                self.size = "1440p".into();
                self.shadow = SHADOW_SIZES[2];
                self.anisotropy = ANISOTROPIES[4];
                self.rate = RATES[3];
                self.fsr1 = false;
                self.rcas = false;
                self.async_shaders = true;
                self.motion_blur = false;
                self.high_lod = true;
                self.compat = COMPAT_MODES[0];
            }
            Preset::Ultra => {
                self.size = "4K".into();
                self.shadow = SHADOW_SIZES[3];
                self.anisotropy = ANISOTROPIES[4];
                self.rate = RATES[3];
                self.fsr1 = false;
                self.rcas = false;
                self.async_shaders = true;
                self.motion_blur = false;
                self.high_lod = true;
                self.compat = COMPAT_MODES[0];
            }
        }
    }

    pub fn load(profile: &Path) -> Self {
        let mut settings = Self::default();
        for (key, value) in read(&config_file(profile)).lines().filter_map(ini_field) {
            match key {
                "display_mode" => settings.mode = find_choice(MODES, value, MODES[0]),
                "render_resolution" => settings.size = value.to_owned(),
                "shadow_resolution" => {
                    settings.shadow = find_choice(SHADOW_SIZES, value, SHADOW_SIZES[1])
                }
                "anisotropy" => {
                    settings.anisotropy = find_choice(ANISOTROPIES, value, ANISOTROPIES[4])
                }
                "motion_blur" => settings.motion_blur = value == "1",
                "bloom" => settings.bloom = value != "0",
                "fov_scale" => settings.fov = find_choice(FOV_SCALES, value, FOV_SCALES[0]),
                "skip_intro" => settings.skip_intro = value == "1",
                "high_lod" => settings.high_lod = value == "1",
                "fsr1" => settings.fsr1 = value == "1",
                "rcas" => settings.rcas = value == "1",
                "rcas_strength" => {
                    settings.rcas_strength = value.parse::<i32>().unwrap_or(75).clamp(0, 100) as u16
                }
                "async_shaders" => settings.async_shaders = value != "0",
                "vibration" => {
                    settings.vibration = value.parse::<i32>().unwrap_or(100).clamp(0, 100) as u16
                }
                "glyph_style" => {
                    settings.glyph_style = find_choice(GLYPH_STYLES, value, GLYPH_STYLES[0])
                }
                "gpu_id" => {
                    settings.gpu_id = if value.chars().all(|c| c.is_ascii_digit()) && !value.is_empty() {
                        value.to_owned()
                    } else {
                        "auto".into()
                    }
                }
                "monitor" => {
                    settings.monitor = if value.chars().all(|c| c.is_ascii_digit()) {
                        value.to_owned()
                    } else {
                        String::new()
                    }
                }
                "compat_mode" => {
                    settings.compat = find_choice(COMPAT_MODES, value, COMPAT_MODES[0])
                }
                "fps_limit" => settings.rate = find_choice(RATES, value, RATES[1]),
                "vsync" => settings.vsync = value != "0",
                "show_fps" => settings.fps = value != "0",
                "show_keyboard_hints" => settings.keyboard_hints = value == "1",
                "master_volume" => {
                    settings.volume = value.parse::<i32>().unwrap_or(100).clamp(0, 200) as u16
                }
                _ => {}
            }
        }
        settings
    }

    pub fn save(&self, profile: &Path) -> io::Result<()> {
        let mut preserve = read(&config_file(profile));
        let values = [
            ("display_mode", self.mode.0.to_owned()),
            ("render_resolution", self.size.clone()),
            ("shadow_resolution", self.shadow.0.to_owned()),
            ("anisotropy", self.anisotropy.0.to_owned()),
            ("motion_blur", u8::from(self.motion_blur).to_string()),
            ("bloom", u8::from(self.bloom).to_string()),
            ("fov_scale", self.fov.0.to_owned()),
            ("skip_intro", u8::from(self.skip_intro).to_string()),
            ("high_lod", u8::from(self.high_lod).to_string()),
            ("fsr1", u8::from(self.fsr1).to_string()),
            ("rcas", u8::from(self.rcas).to_string()),
            ("rcas_strength", self.rcas_strength.to_string()),
            ("async_shaders", u8::from(self.async_shaders).to_string()),
            ("vibration", self.vibration.to_string()),
            ("glyph_style", self.glyph_style.0.to_owned()),
            ("gpu_id", self.gpu_id.clone()),
            ("monitor", self.monitor.clone()),
            ("compat_mode", self.compat.0.to_owned()),
            ("fps_limit", self.rate.0.to_owned()),
            ("vsync", u8::from(self.vsync).to_string()),
            ("show_fps", u8::from(self.fps).to_string()),
            (
                "show_keyboard_hints",
                u8::from(self.keyboard_hints).to_string(),
            ),
            ("master_volume", self.volume.to_string()),
        ];
        preserve = preserve
            .lines()
            .filter(|line| {
                !ini_field(line).is_some_and(|(key, _)| values.iter().any(|(name, _)| *name == key))
            })
            .collect::<Vec<_>>()
            .join("\n");
        if !preserve.is_empty() {
            preserve.push('\n');
        }
        for (key, value) in values {
            preserve.push_str(&format!("{key}={value}\n"));
        }
        write(&profile.join("pistonport.ini"), &preserve)
    }
}

fn find_choice(choices: &[Choice], key: &str, fallback: Choice) -> Choice {
    choices
        .iter()
        .find(|item| item.0 == key)
        .copied()
        .unwrap_or(fallback)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_defaults_and_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let defaults = Settings::load(dir.path());
        assert_eq!(defaults.size, "native");
        assert_eq!(defaults.shadow.0, "2048");
        assert_eq!(defaults.anisotropy.0, "16");
        assert!(!defaults.motion_blur);
        assert_eq!(defaults.fov.0, "100");
        assert!(!defaults.skip_intro);
        assert!(!defaults.high_lod);
        assert!(defaults.async_shaders);
        assert_eq!(defaults.vibration, 100);
        assert_eq!(defaults.glyph_style.0, "auto");
        assert_eq!(defaults.gpu_id, "auto");
        assert_eq!(defaults.compat.0, "auto");

        let mut custom = defaults.clone();
        custom.apply_preset(Preset::Handheld);
        assert_eq!(custom.size, "720p");
        assert!(custom.fsr1);
        assert!(custom.rcas);
        assert_eq!(custom.compat.0, "safe");

        custom.mode = MODES[1];
        custom.size = "3440x1440".into();
        custom.shadow = SHADOW_SIZES[3];
        custom.anisotropy = ANISOTROPIES[2];
        custom.motion_blur = true;
        custom.fov = FOV_SCALES[2];
        custom.skip_intro = true;
        custom.high_lod = true;
        custom.vibration = 60;
        custom.glyph_style = GLYPH_STYLES[2];
        custom.gpu_id = "0".into();
        custom.monitor = "1".into();
        custom.compat = COMPAT_MODES[2];
        custom.rate = RATES[3];
        custom.save(dir.path()).unwrap();

        let loaded = Settings::load(dir.path());
        assert_eq!(loaded, custom);
    }
}
