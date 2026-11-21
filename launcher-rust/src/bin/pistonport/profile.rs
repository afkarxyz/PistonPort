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
        Slot::new("builtin-complete-v2", "All Skill Checks (Built-In)"),
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

pub const MODES: &[Choice] = &[
    Choice("windowed", "Windowed"),
    Choice("borderless", "Borderless"),
    Choice("fullscreen", "Fullscreen"),
];
pub const SIZES: &[Choice] = &[
    Choice("720p", "1280 × 720"),
    Choice("900p", "1600 × 900"),
    Choice("native", "1920 × 1080"),
    Choice("1440p", "2560 × 1440"),
    Choice("4K", "3840 × 2160"),
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
pub struct Choice(pub &'static str, pub &'static str);

impl fmt::Display for Choice {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.1)
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct Settings {
    pub mode: Choice,
    pub size: String,
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

impl Settings {
    pub fn load(profile: &Path) -> Self {
        let mut settings = Self {
            mode: MODES[0],
            size: "native".into(),
            rate: RATES[1],
            vsync: true,
            fps: true,
            keyboard_hints: false,
            volume: 100,
        };
        for (key, value) in read(&config_file(profile)).lines().filter_map(ini_field) {
            match key {
                "display_mode" => settings.mode = find_choice(MODES, value, MODES[0]),
                "render_resolution" => settings.size = value.to_owned(),
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
