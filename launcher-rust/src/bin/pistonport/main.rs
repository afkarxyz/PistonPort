#![cfg_attr(windows, windows_subsystem = "windows")]

mod icons;
mod profile;
mod trophies;
#[allow(dead_code)]
mod trophy_data;
mod view;

use std::{
    cell::RefCell,
    fs,
    path::PathBuf,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use profile::{
    BackdropInfo, Choice, HardwareInfo, Paths, Preset, Settings, Slot, SourceInspection, UpdateInfo,
};
use rfd::FileDialog;
use rune::{
    AppHandle, AppSpec, Image, ImageCache, TextEdit, Tx, Waker, WindowHandle, WindowOptions,
};
use trophies::{Trophy, TrophyPage};
use trophy_data::read_unlocked_trophies;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Page {
    Welcome,
    Garage,
    Saves,
    Settings,
    Controls,
    Trophies,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SettingsTab {
    Graphics,
    Upscaling,
    Gameplay,
    System,
}

#[derive(Clone, Copy)]
enum Picker {
    Package,
    Folder,
    Save,
}

#[derive(Clone, Copy)]
enum GamepadAction {
    PrevTab,
    NextTab,
    Confirm,
    Back,
}

#[derive(Clone)]
enum Message {
    Page(Page),
    SettingsTab(SettingsTab),
    Pick(Picker),
    Picked(Picker, Option<PathBuf>),
    Inspected(String, Result<GameSource, String>, bool),
    HardwareLoaded(HardwareInfo),
    CheckUpdate,
    UpdateChecked(Result<UpdateInfo, String>, bool),
    InstallUpdate(String, String),
    UpdateInstalled(Result<(), String>),
    OpenCoffee,
    Preset(Preset),
    Mode(Choice),
    Size(String),
    Shadow(Choice),
    Anisotropy(Choice),
    MotionBlur(bool),
    Bloom(bool),
    Fov(Choice),
    SkipIntro(bool),
    HighLod(bool),
    Fsr1(bool),
    Rcas(bool),
    RcasStrength(u16),
    AsyncShaders(bool),
    Vibration(u16),
    Gpu(String),
    Monitor(String),
    Compat(Choice),
    Rate(Choice),
    Vsync(bool),
    Fps(bool),
    KeyboardHints(bool),
    Volume(u16),
    Map(usize, Pad),
    Reset,
    ResetSettings,
    Save,
    Commit(String, Result<Option<GameSource>, String>, bool),
    SelectSlot(Slot),
    ExportSave(Slot),
    ExportPicked(Slot, Option<PathBuf>),
    ExportDone(Result<PathBuf, String>),
    SaveDone(Result<Vec<u8>, String>, bool),
    TrophiesLoaded(String, Result<Vec<Trophy>, String>),
    BackdropsReady(String, Vec<BackdropInfo>),
    AutoCycleBackdrop(u64),
    BackdropLoaded(String, Result<Image, String>),
    OpenLog,
    CopyCrashReport,
    ReportCopied(Result<(), String>),
    DismissToast(u64),
    Gamepad(GamepadAction),
    Play,
    StopGame,
    GameDone(Result<Vec<u8>, String>),
    Minimize,
    Close,
}

#[derive(Clone, Debug)]
pub struct ToastNotice {
    pub id: u64,
    pub title: String,
    pub detail: Option<String>,
    pub error: bool,
    pub crash_action: bool,
}

#[derive(Clone)]
struct GameSource {
    identity: String,
}

fn load_game_source(paths: &Paths, source: &str) -> Result<GameSource, String> {
    let SourceInspection { identity, .. } = paths.inspect(source)?;
    Ok(GameSource { identity })
}

struct App {
    paths: Paths,
    handle: AppHandle<Self>,
    native_window: Rc<RefCell<Option<WindowHandle>>>,
    page: Page,
    settings_tab: SettingsTab,
    settings: Settings,
    hardware: HardwareInfo,
    update_info: Option<Result<UpdateInfo, String>>,
    checking_update: bool,
    installing_update: bool,
    playing_flag: Arc<AtomicBool>,
    source: TextEdit,
    saved_source: String,
    game_identity: Option<(String, String)>,
    assets: Option<view::Artwork>,
    backdrops: Vec<BackdropInfo>,
    backdrop_index: usize,
    backdrop_seq: u64,
    backdrop_timer_gen: u64,
    backdrop_loading: bool,
    pad: [Pad; 15],
    slots: Vec<Slot>,
    selected: Slot,
    trophies: TrophyPage,
    menu: Option<String>,
    toast: Option<ToastNotice>,
    toast_seq: u64,
    busy: bool,
    playing: bool,
    stopping: bool,
    status: String,
    error: bool,
}

impl App {
    fn open(
        paths: Paths,
        handle: AppHandle<Self>,
        native_window: Rc<RefCell<Option<WindowHandle>>>,
    ) -> Self {
        let migration_error = paths.migrate_retired_save_selection().err();
        let (slots, selected) = profile::slots(&paths.profile);
        let source = profile::read(&paths.profile.join("game-folder.txt"));
        let has_source = !source.is_empty();
        let toast = std::env::var("PISTONPORT_SMOKE_TOAST")
            .ok()
            .filter(|v| !v.is_empty())
            .map(|raw| {
                let (title, detail) = match raw.split_once('|') {
                    Some((t, d)) => (t.to_owned(), Some(d.to_owned())),
                    None => (raw, None),
                };
                ToastNotice {
                    id: 1,
                    title,
                    detail,
                    error: true,
                    crash_action: true,
                }
            })
            .or_else(|| {
                migration_error.map(|detail| ToastNotice {
                    id: 1,
                    title: "Save Migration Notice".into(),
                    detail: Some(detail),
                    error: true,
                    crash_action: false,
                })
            });
        Self {
            settings: Settings::load(&paths.profile),
            hardware: HardwareInfo::default(),
            update_info: None,
            checking_update: false,
            installing_update: false,
            playing_flag: Arc::new(AtomicBool::new(false)),
            pad: load_bindings(&paths.profile.join("controller.map")),
            page: Page::Welcome,
            settings_tab: SettingsTab::Graphics,
            source: TextEdit::new(&source),
            saved_source: source.clone(),
            game_identity: None,
            assets: None,
            backdrops: Vec::new(),
            backdrop_index: 0,
            backdrop_seq: 0,
            backdrop_timer_gen: 0,
            backdrop_loading: false,
            handle,
            native_window,
            paths,
            slots,
            selected,
            trophies: TrophyPage::default(),
            menu: None,
            toast,
            toast_seq: 1,
            busy: has_source,
            playing: false,
            stopping: false,
            status: if has_source {
                "Loading Cars 3: Driven to Win…".into()
            } else {
                "Ready to Play".into()
            },
            error: false,
        }
    }

    fn setup(&mut self, wake: Waker) {
        let cache = Arc::new(ImageCache::new(wake));
        self.assets = Some(view::Artwork::load(cache));
        let paths = self.paths.clone();
        self.background(move || Message::HardwareLoaded(paths.list_hardware()));
        self.checking_update = true;
        let update_paths = self.paths.clone();
        self.background(move || {
            Message::UpdateChecked(update_paths.check_update(env!("CARGO_PKG_VERSION")), false)
        });
        #[cfg(windows)]
        start_gamepad_poller(self.handle.clone(), self.playing_flag.clone());
        if !self.source.text.is_empty() {
            self.busy = true;
            self.report("Loading Cars 3: Driven to Win…", false);
            self.inspect(self.source.text.clone(), false);
        }
    }

    fn background(&mut self, work: impl FnOnce() -> Message + Send + 'static) {
        let handle = self.handle.clone();
        if let Err(error) = rune::task::spawn("PistonPort worker", move || {
            let message = work();
            handle.update(move |state, tx| state.update(message, tx));
        }) {
            self.busy = false;
            self.playing = false;
            self.report(format!("Could not start a launcher task: {error}"), true);
        }
    }

    fn report(&mut self, message: impl Into<String>, error: bool) {
        let text = message.into();
        if error {
            self.push_toast("Operation Failed", Some(text), true, false);
            if self.status.is_empty() || self.busy {
                self.status = "Ready to Play".into();
            }
            self.error = false;
        } else {
            self.status = text;
            self.error = false;
        }
    }

    fn push_toast(
        &mut self,
        title: impl Into<String>,
        detail: Option<String>,
        error: bool,
        crash_action: bool,
    ) {
        self.toast_seq += 1;
        let id = self.toast_seq;
        self.toast = Some(ToastNotice {
            id,
            title: title.into(),
            detail,
            error,
            crash_action,
        });
        let secs = if crash_action {
            10
        } else if error {
            6
        } else {
            4
        };
        let handle = self.handle.clone();
        let _ = rune::task::spawn("PistonPort toast timer", move || {
            std::thread::sleep(std::time::Duration::from_secs(secs));
            handle.update(move |state, tx| state.update(Message::DismissToast(id), tx));
        });
    }

    fn dirty(&mut self) {
        self.report("Unsaved Changes", false);
    }

    fn refresh_trophy_progress(&mut self) {
        self.trophies.unlocked.clear();
        if !self.slots.contains(&self.selected) {
            return;
        }
        let root = if self.selected.id == "default" {
            self.paths.profile.clone()
        } else {
            self.paths
                .profile
                .join("save-slots")
                .join(&self.selected.id)
        };
        self.trophies.unlocked = read_unlocked_trophies(&root.join("trophies.dat"));
    }

    fn open_trophies(&mut self) {
        self.refresh_trophy_progress();
        let source = self.source.text.trim().to_owned();
        if source == self.trophies.source && !matches!(self.trophies.catalog, Some(Err(_))) {
            return;
        }
        self.trophies.source = source.clone();
        self.trophies.catalog = None;
        if source.is_empty() {
            return;
        }
        let Some(assets) = &self.assets else {
            return;
        };
        let images = assets.cache.clone();
        let paths = self.paths.clone();
        self.background(move || {
            let catalog = paths
                .inspect_trophies(&source)
                .and_then(|bytes| trophies::parse_catalog(&bytes, &images));
            Message::TrophiesLoaded(source, catalog)
        });
    }

    fn set_source_info(&mut self, source: &str, inspection: Option<GameSource>) {
        if let Some(inspection) = inspection {
            self.game_identity = Some((source.to_owned(), inspection.identity));
        } else {
            self.game_identity = None;
        }
        self.backdrop_index = 0;
        self.backdrop_loading = false;
        if let Some(assets) = &mut self.assets {
            assets.cover = None;
        }
        if self.game_identity.is_some() {
            let paths = self.paths.clone();
            let src = source.to_owned();
            self.background(move || {
                let list = paths.list_backdrops(&src);
                Message::BackdropsReady(src, list)
            });
            self.open_trophies();
        } else {
            self.backdrops.clear();
        }
    }

    fn random_backdrop_index(&self, exclude: usize) -> usize {
        let count = self.backdrops.len();
        if count <= 1 {
            return count;
        }
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos() as usize ^ (d.as_secs() as usize).wrapping_mul(2654435761))
            .unwrap_or(1);
        let mut pick = (nanos % count) + 1;
        if pick == exclude {
            pick = (pick % count) + 1;
        }
        pick
    }

    fn schedule_backdrop_timer(&mut self) {
        self.backdrop_timer_gen += 1;
        let token = self.backdrop_timer_gen;
        let handle = self.handle.clone();
        let secs: u64 = std::env::var("PISTONPORT_BACKDROP_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .filter(|&s| s >= 1)
            .unwrap_or(3);
        let _ = rune::task::spawn("PistonPort backdrop timer", move || {
            std::thread::sleep(std::time::Duration::from_secs(secs));
            handle.update(move |state, tx| state.update(Message::AutoCycleBackdrop(token), tx));
        });
    }

    fn apply_backdrop_index(&mut self, index: usize) {
        let count = self.backdrops.len();
        if count == 0 {
            return;
        }
        let idx = ((index.saturating_sub(1)) % count) + 1;
        self.backdrop_index = idx;
        let Some(entry) = self.backdrops.get(idx - 1).cloned() else {
            return;
        };
        let Some(cache) = self.assets.as_ref().map(|a| a.cache.clone()) else {
            return;
        };
        let source = self.saved_source.clone();
        if source.is_empty() {
            return;
        }
        self.backdrop_loading = true;
        let paths = self.paths.clone();
        self.background(move || {
            let id = entry.id.clone();
            let result = paths.load_backdrop(&source, &entry).and_then(|bytes| {
                let img = cache.load(bytes);
                if img.wait().is_ok() {
                    Ok(img)
                } else {
                    Err("Could not decode backdrop image.".to_owned())
                }
            });
            Message::BackdropLoaded(id, result)
        });
    }

    fn inspect(&mut self, path: String, picked: bool) {
        let paths = self.paths.clone();
        let first_selection = picked && self.page == Page::Welcome;
        self.background(move || {
            let result = load_game_source(&paths, &path).and_then(|inspection| {
                if first_selection {
                    profile::write(&paths.profile.join("game-folder.txt"), &path)
                        .map_err(|error| format!("Could not save the game source: {error}"))?;
                }
                Ok(inspection)
            });
            Message::Inspected(path, result, picked)
        });
    }

    fn choose(&mut self, kind: Picker) {
        self.menu = None;
        self.busy = true;
        if self.page == Page::Welcome {
            self.status.clear();
            self.error = false;
        }
        let directory = PathBuf::from(self.source.text.trim());
        self.background(move || {
            let mut dialog = FileDialog::new();
            if directory.is_dir() {
                dialog = dialog.set_directory(&directory);
            } else if let Some(parent) = directory.parent().filter(|p| p.is_dir()) {
                dialog = dialog.set_directory(parent);
            }
            let picked = match kind {
                Picker::Package => dialog
                    .set_title("Choose Your Cars 3 PS4 Game Package")
                    .add_filter("PlayStation 4 Game Package", &["pkg"])
                    .pick_file(),
                Picker::Folder => dialog
                    .set_title("Select the Extracted PS4 Game Folder")
                    .pick_folder(),
                Picker::Save => dialog
                    .set_title("Load a Cars 3 Save Into a New Slot")
                    .add_filter("Cars 3 Save ZIP", &["zip"])
                    .add_filter("All Files (Includes P0)", &["*"])
                    .pick_file(),
            };
            Message::Picked(kind, picked)
        });
    }

    fn save_action(&mut self, command: &'static str, path: PathBuf, is_import: bool) {
        self.busy = true;
        let paths = self.paths.clone();
        self.background(move || Message::SaveDone(paths.save_action(command, &path), is_import));
    }

    fn commit(&mut self, play: bool) {
        let source = self.source.text.trim().to_owned();
        if play && source.is_empty() {
            self.page = Page::Settings;
            self.choose(Picker::Package);
            return;
        }
        self.menu = None;
        self.busy = true;
        self.report(
            if play {
                "Checking Game Source…"
            } else {
                "Saving Settings…"
            },
            false,
        );
        let changed_source = source != self.saved_source;
        let paths = self.paths.clone();
        let settings = self.settings.clone();
        let pad = self.pad;
        self.background(move || {
            let result = (|| {
                let inspection = if !source.is_empty() && (changed_source || play) {
                    Some(load_game_source(&paths, &source)?)
                } else {
                    None
                };
                settings.save(&paths.profile).map_err(|e| e.to_string())?;
                let map = bindings_value(&pad);
                let map_file = paths.profile.join("controller.map");
                if profile::read(&map_file) != map {
                    profile::write(&map_file, &map).map_err(|e| e.to_string())?;
                }
                if changed_source || !paths.profile.join("game-folder.txt").exists() {
                    profile::write(&paths.profile.join("game-folder.txt"), &source)
                        .map_err(|e| e.to_string())?;
                }
                Ok(inspection)
            })();
            Message::Commit(source, result, play)
        });
    }

    fn update(&mut self, message: Message, tx: &mut Tx) {
        match message {
            Message::Page(page) if self.page != Page::Welcome => {
                self.page = page;
                self.menu = None;
                if page == Page::Trophies {
                    self.open_trophies();
                }
            }
            Message::SettingsTab(tab) => {
                self.settings_tab = tab;
                self.menu = None;
            }
            Message::Pick(kind) if !self.busy => self.choose(kind),
            Message::Picked(kind, picked) => {
                self.busy = false;
                if let Some(path) = picked {
                    if matches!(kind, Picker::Save) {
                        self.save_action("--import-save", path, true);
                    } else {
                        self.busy = true;
                        self.report("Checking Game Source…", false);
                        self.inspect(path.to_string_lossy().into_owned(), true);
                    }
                }
            }
            Message::Inspected(path, result, picked) => {
                if self.page == Page::Welcome {
                    self.busy = false;
                    match result {
                        Ok(inspection) => {
                            self.source = TextEdit::new(&path);
                            self.saved_source = path.clone();
                            self.set_source_info(&path, Some(inspection));
                            self.page = Page::Garage;
                            if let Ok(start) = std::env::var("PISTONPORT_START_PAGE") {
                                match start.as_str() {
                                    "settings" | "settings:graphics" => {
                                        self.page = Page::Settings;
                                        self.settings_tab = SettingsTab::Graphics;
                                    }
                                    "settings:upscaling" => {
                                        self.page = Page::Settings;
                                        self.settings_tab = SettingsTab::Upscaling;
                                    }
                                    "settings:gameplay" => {
                                        self.page = Page::Settings;
                                        self.settings_tab = SettingsTab::Gameplay;
                                    }
                                    "settings:system" => {
                                        self.page = Page::Settings;
                                        self.settings_tab = SettingsTab::System;
                                    }
                                    "saves" => self.page = Page::Saves,
                                    "controls" => self.page = Page::Controls,
                                    "trophies" => {
                                        self.page = Page::Trophies;
                                        self.open_trophies();
                                    }
                                    _ => {}
                                }
                            }
                            self.report("Ready to Play", false);
                            if let Ok(test_toast) = std::env::var("PISTONPORT_TEST_TOAST")
                                && !test_toast.is_empty()
                            {
                                self.push_toast(
                                    "Game Exited With an Error",
                                    Some(test_toast),
                                    true,
                                    true,
                                );
                            }
                        }
                        Err(error) => {
                            self.push_toast("Invalid Game Source", Some(error), true, false);
                        }
                    }
                } else if picked {
                    self.busy = false;
                    match result {
                        Ok(inspection) => {
                            self.source = TextEdit::new(&path);
                            self.set_source_info(&path, Some(inspection));
                            self.dirty();
                            if self.page == Page::Trophies {
                                self.open_trophies();
                            }
                        }
                        Err(error) => {
                            tx.animated();
                            self.push_toast("Invalid Game Source", Some(error), true, false);
                        }
                    }
                } else if self.source.text == path
                    && let Ok(inspection) = result
                {
                    self.set_source_info(&path, Some(inspection));
                }
            }
            Message::HardwareLoaded(info) => {
                self.hardware = info;
            }
            Message::CheckUpdate if !self.checking_update => {
                self.checking_update = true;
                self.report("Checking GitHub Releases…", false);
                let paths = self.paths.clone();
                self.background(move || {
                    Message::UpdateChecked(paths.check_update(env!("CARGO_PKG_VERSION")), true)
                });
            }
            Message::UpdateChecked(result, manual) => {
                self.checking_update = false;
                if manual {
                    self.report("Ready to Play", false);
                }
                if let Ok(info) = &result {
                    if info.available {
                        tx.animated();
                        if manual {
                            self.push_toast(
                                format!("Update Available: {}", info.tag),
                                Some(info.title.clone()),
                                false,
                                false,
                            );
                        }
                    } else if manual {
                        tx.animated();
                        let msg = format!("PistonPort is Up to Date (v{})", env!("CARGO_PKG_VERSION"));
                        self.push_toast(msg, Some(format!("Latest release: {}", info.tag)), false, false);
                    }
                } else if let (true, Err(error)) = (manual, &result) {
                    tx.animated();
                    self.push_toast("Update Check Unavailable", Some(error.clone()), true, false);
                }
                self.update_info = Some(result);
            }
            Message::InstallUpdate(asset_url, fallback_url) if !self.busy && !self.installing_update => {
                if asset_url.is_empty() {
                    if let Err(error) = profile::open_url(&fallback_url) {
                        tx.animated();
                        self.push_toast("Could Not Open Browser", Some(error), true, false);
                    }
                    return;
                }
                self.installing_update = true;
                self.busy = true;
                tx.animated();
                self.report("Downloading Update…", false);
                self.push_toast(
                    "Downloading Update…",
                    Some("PistonPort will close, extract the new version, and restart automatically.".into()),
                    false,
                    false,
                );
                let paths = self.paths.clone();
                self.background(move || Message::UpdateInstalled(paths.install_update(&asset_url)));
            }
            Message::UpdateInstalled(result) => {
                self.installing_update = false;
                self.busy = false;
                match result {
                    Ok(()) => {
                        std::process::exit(0);
                    }
                    Err(error) => {
                        tx.animated();
                        self.report("Ready to Play", false);
                        self.push_toast("Update Failed", Some(error), true, false);
                    }
                }
            }
            Message::OpenCoffee => {
                if let Err(error) = profile::open_url("https://afkarxyz.gumroad.com/coffee") {
                    tx.animated();
                    self.push_toast("Could Not Open Browser", Some(error), true, false);
                }
            }
            Message::Preset(preset) if !self.busy => {
                self.settings.apply_preset(preset);
                let label = match preset {
                    Preset::Handheld => "Applied Handheld Preset (Unsaved)",
                    Preset::Balanced => "Applied Balanced Preset (Unsaved)",
                    Preset::HighRefresh => "Applied High Refresh Preset (Unsaved)",
                    Preset::Ultra => "Applied Ultra Preset (Unsaved)",
                };
                self.report(label, false);
            }
            Message::Mode(value) if !self.busy => {
                self.settings.mode = value;
                self.dirty();
            }
            Message::Size(value) if !self.busy => {
                self.settings.size = value;
                self.dirty();
            }
            Message::Shadow(value) if !self.busy => {
                self.settings.shadow = value;
                self.dirty();
            }
            Message::Anisotropy(value) if !self.busy => {
                self.settings.anisotropy = value;
                self.dirty();
            }
            Message::MotionBlur(value) if !self.busy => {
                self.settings.motion_blur = value;
                self.dirty();
            }
            Message::Bloom(value) if !self.busy => {
                self.settings.bloom = value;
                self.dirty();
            }
            Message::Fov(value) if !self.busy => {
                self.settings.fov = value;
                self.dirty();
            }
            Message::SkipIntro(value) if !self.busy => {
                self.settings.skip_intro = value;
                self.dirty();
            }
            Message::HighLod(value) if !self.busy => {
                self.settings.high_lod = value;
                self.dirty();
            }
            Message::Fsr1(value) if !self.busy => {
                self.settings.fsr1 = value;
                self.dirty();
            }
            Message::Rcas(value) if !self.busy => {
                self.settings.rcas = value;
                self.dirty();
            }
            Message::RcasStrength(value) if !self.busy => {
                self.settings.rcas_strength = value;
                self.dirty();
            }
            Message::AsyncShaders(value) if !self.busy => {
                self.settings.async_shaders = value;
                self.dirty();
            }
            Message::Vibration(value) if !self.busy => {
                self.settings.vibration = value;
                self.dirty();
            }
            Message::Gpu(value) if !self.busy => {
                self.settings.gpu_id = value;
                self.dirty();
            }
            Message::Monitor(value) if !self.busy => {
                self.settings.monitor = value;
                self.dirty();
            }
            Message::Compat(value) if !self.busy => {
                self.settings.compat = value;
                self.dirty();
            }
            Message::Rate(value) if !self.busy => {
                self.settings.rate = value;
                self.dirty();
            }
            Message::Vsync(value) if !self.busy => {
                self.settings.vsync = value;
                self.dirty();
            }
            Message::Fps(value) if !self.busy => {
                self.settings.fps = value;
                self.dirty();
            }
            Message::KeyboardHints(value) if !self.busy => {
                self.settings.keyboard_hints = value;
                self.dirty();
            }
            Message::Volume(value) if !self.busy => {
                self.settings.volume = value;
                self.dirty();
            }
            Message::Map(index, pad) if !self.busy => {
                if let Some(previous) = self.pad.iter().position(|mapped| *mapped == pad) {
                    self.pad.swap(index, previous);
                } else {
                    self.pad[index] = pad;
                }
                self.dirty();
            }
            Message::Reset if !self.busy => {
                self.pad = DEFAULT_PAD;
                self.dirty();
            }
            Message::ResetSettings if !self.busy => {
                self.settings = Settings::default();
                self.report("Default Settings Restored (Unsaved)", false);
                self.push_toast(
                    "Default Settings Restored",
                    Some("Click Save Settings to persist the default configuration.".into()),
                    false,
                    false,
                );
            }
            Message::Save if !self.busy => self.commit(false),
            Message::Play if !self.busy => self.commit(true),
            Message::Commit(source, result, play) => {
                self.busy = false;
                match result {
                    Err(error) => {
                        tx.animated();
                        self.push_toast("Could Not Save Settings", Some(error), true, false);
                    }
                    Ok(inspection) => {
                        if source != self.saved_source || play {
                            self.set_source_info(&source, inspection);
                        }
                        self.source = TextEdit::new(&source);
                        self.saved_source = source.clone();
                        self.report("Ready to Play", false);
                        if !play {
                            tx.animated();
                            self.push_toast("Settings Saved", None, false, false);
                        }
                        if play {
                            self.busy = true;
                            self.playing = true;
                            self.playing_flag.store(true, Ordering::Relaxed);
                            self.report("Game Running", false);
                            let paths = self.paths.clone();
                            self.minimize(tx);
                            self.background(move || Message::GameDone(paths.play(&source)));
                        } else if source.is_empty() {
                            self.page = Page::Welcome;
                            self.status.clear();
                        }
                    }
                }
            }
            Message::SelectSlot(slot) if !self.busy && slot.id != self.selected.id => {
                self.save_action("--select", PathBuf::from(slot.id), false);
            }
            Message::ExportSave(slot) if !self.busy => {
                self.menu = None;
                self.busy = true;
                let default_name = format!(
                    "PistonPort-{}.zip",
                    slot.label
                        .chars()
                        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
                        .collect::<String>()
                        .trim_matches('-')
                );
                self.background(move || {
                    let picked = FileDialog::new()
                        .set_title("Export Cars 3 Save Archive")
                        .set_file_name(&default_name)
                        .add_filter("Cars 3 Save ZIP", &["zip"])
                        .save_file();
                    Message::ExportPicked(slot, picked)
                });
            }
            Message::ExportPicked(slot, picked) => {
                if let Some(dest) = picked {
                    let paths = self.paths.clone();
                    self.report("Exporting Save…", false);
                    self.background(move || {
                        let result = paths.export_save(&slot.id, &dest).map(|_| dest);
                        Message::ExportDone(result)
                    });
                } else {
                    self.busy = false;
                }
            }
            Message::ExportDone(result) => {
                self.busy = false;
                tx.animated();
                match result {
                    Ok(path) => {
                        let name = path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("ZIP archive")
                            .to_owned();
                        self.report("Ready to Play", false);
                        self.push_toast("Save Exported", Some(name), false, false);
                    }
                    Err(error) => {
                        self.push_toast("Could Not Export Save", Some(error), true, false);
                    }
                }
            }
            Message::SaveDone(result, imported) => {
                self.busy = false;
                tx.animated();
                (self.slots, self.selected) = profile::slots(&self.paths.profile);
                self.refresh_trophy_progress();
                match result {
                    Ok(_) => {
                        let msg = if imported {
                            "Imported Save Selected"
                        } else {
                            "Save Slot Selected"
                        };
                        self.report("Ready to Play", false);
                        self.push_toast(
                            msg,
                            Some(self.selected.label.clone()),
                            false,
                            false,
                        );
                    }
                    Err(error) => {
                        self.push_toast("Save Slot Error", Some(error), true, false);
                    }
                }
            }
            Message::TrophiesLoaded(source, result) if source == self.trophies.source => {
                self.trophies.catalog = Some(result);
            }
            Message::OpenLog => {
                if let Err(error) = self.paths.open_log() {
                    tx.animated();
                    self.push_toast("Could Not Open Log", Some(error), true, false);
                }
            }
            Message::CopyCrashReport if !self.busy => {
                self.busy = true;
                self.report("Preparing Crash Report…", false);
                let paths = self.paths.clone();
                self.background(move || {
                    let result = paths
                        .crash_report()
                        .and_then(|report| profile::copy_to_clipboard(&report));
                    Message::ReportCopied(result)
                });
            }
            Message::ReportCopied(result) => {
                self.busy = false;
                tx.animated();
                match result {
                    Ok(()) => {
                        self.report("Ready to Play", false);
                        self.push_toast(
                            "Crash Report Copied",
                            Some("System specs, settings, and log details are on your clipboard.".into()),
                            false,
                            false,
                        );
                    }
                    Err(error) => {
                        self.push_toast("Could Not Copy Crash Report", Some(error), true, false);
                    }
                }
            }
            Message::DismissToast(id) => {
                if self.toast.as_ref().is_some_and(|t| t.id == id) {
                    tx.animated();
                    self.toast = None;
                }
            }
            Message::BackdropsReady(source, list) => {
                if source == self.saved_source {
                    self.backdrops = list;
                    if !self.backdrops.is_empty() {
                        let start = std::env::var("PISTONPORT_SMOKE_BACKDROP")
                            .ok()
                            .filter(|v| !v.is_empty())
                            .and_then(|wanted| self.backdrops.iter().position(|b| b.id == wanted))
                            .map(|pos| pos + 1)
                            .unwrap_or_else(|| self.random_backdrop_index(0));
                        self.apply_backdrop_index(start);
                    }
                }
            }
            Message::AutoCycleBackdrop(token) => {
                if token != self.backdrop_timer_gen || self.backdrops.is_empty() {
                    return;
                }
                if self.page == Page::Garage
                    && !self.playing
                    && !self.busy
                    && !self.backdrop_loading
                {
                    let next = self.random_backdrop_index(self.backdrop_index);
                    self.apply_backdrop_index(next);
                } else {
                    self.schedule_backdrop_timer();
                }
            }
            Message::BackdropLoaded(id, result) => {
                let matches_current = self.backdrop_index > 0
                    && self
                        .backdrops
                        .get(self.backdrop_index - 1)
                        .is_some_and(|b| b.id == id);
                if matches_current {
                    self.backdrop_loading = false;
                    match result {
                        Ok(image) => {
                            tx.animated();
                            self.backdrop_seq += 1;
                            if let Some(assets) = &mut self.assets {
                                assets.cover = Some(image);
                            }
                        }
                        Err(error) => {
                            self.push_toast("Backdrop Error", Some(error), true, false);
                        }
                    }
                    self.schedule_backdrop_timer();
                }
            }
            Message::Gamepad(action) if !self.busy && self.page != Page::Welcome => {
                let order = [
                    Page::Garage,
                    Page::Settings,
                    Page::Controls,
                    Page::Saves,
                    Page::Trophies,
                ];
                match action {
                    GamepadAction::PrevTab | GamepadAction::NextTab => {
                        if let Some(pos) = order.iter().position(|p| *p == self.page) {
                            let next = if matches!(action, GamepadAction::NextTab) {
                                (pos + 1) % order.len()
                            } else {
                                (pos + order.len() - 1) % order.len()
                            };
                            self.update(Message::Page(order[next]), tx);
                        }
                    }
                    GamepadAction::Confirm => {
                        if self.page == Page::Garage {
                            self.commit(true);
                        } else if matches!(self.page, Page::Settings | Page::Controls) {
                            self.commit(false);
                        }
                    }
                    GamepadAction::Back => {
                        if self.menu.is_some() {
                            self.menu = None;
                        } else if self.page != Page::Garage {
                            self.update(Message::Page(Page::Garage), tx);
                        }
                    }
                }
            }
            Message::StopGame if self.playing && !self.stopping => {
                self.stopping = true;
                self.report("Stopping Game…", false);
                let _ = rune::task::spawn("PistonPort stop game", || {
                    Paths::stop_game();
                });
            }
            Message::GameDone(result) => {
                self.busy = false;
                self.playing = false;
                let was_stopping = std::mem::take(&mut self.stopping);
                self.playing_flag.store(false, Ordering::Relaxed);
                self.settings = Settings::load(&self.paths.profile);
                self.pad = load_bindings(&self.paths.profile.join("controller.map"));
                (self.slots, self.selected) = profile::slots(&self.paths.profile);
                self.refresh_trophy_progress();
                if was_stopping {
                    self.report("Ready to Play", false);
                } else {
                    match result {
                        Ok(_) => self.report("Ready to Play", false),
                        Err(error) => {
                            tx.animated();
                            self.report("Ready to Play", false);
                            self.push_toast("Game Exited With an Error", Some(error), true, true);
                        }
                    }
                }
                self.handle.open();
            }
            Message::Minimize => self.minimize(tx),
            Message::Close => tx.close_window(),
            _ => {}
        }
    }

    fn minimize(&self, tx: &mut Tx) {
        #[cfg(windows)]
        if let Some(window) = self.native_window.borrow().as_ref()
            && matches!(
                window.with_raw_handle(|handle| {
                    use rune::raw_window_handle::RawWindowHandle;
                    use windows_sys::Win32::UI::WindowsAndMessaging::{SW_MINIMIZE, ShowWindow};
                    if let RawWindowHandle::Win32(handle) = handle.as_raw() {
                        // Rune skips minimization for borderless windows.
                        unsafe {
                            ShowWindow(handle.hwnd.get() as _, SW_MINIMIZE);
                        }
                        true
                    } else {
                        false
                    }
                }),
                Ok(true)
            )
        {
            return;
        }
        tx.minimize();
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Pad {
    virtual_button: &'static str,
    physical: &'static str,
    label: &'static str,
    action: &'static str,
    key: &'static str,
}

impl std::fmt::Display for Pad {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label)
    }
}

const fn pad(
    virtual_button: &'static str,
    physical: &'static str,
    label: &'static str,
    action: &'static str,
    key: &'static str,
) -> Pad {
    Pad {
        virtual_button,
        physical,
        label,
        action,
        key,
    }
}

const DEFAULT_PAD: [Pad; 15] = [
    pad("r2", "righttrigger", "R2", "Accelerate", "F"),
    pad("l2", "lefttrigger", "L2", "Brake / Reverse", "R"),
    pad("square", "x", "Square", "Boost", "E"),
    pad("cross", "a", "Cross", "Select", "Space"),
    pad("circle", "b", "Circle", "Back", "Left Shift"),
    pad("options", "start", "Options", "Pause", "Enter"),
    pad("triangle", "y", "Triangle", "Triangle", "Q"),
    pad("l1", "leftshoulder", "L1", "Left Bumper", "1"),
    pad("r1", "rightshoulder", "R1", "Right Bumper", "3"),
    pad("l3", "leftstick", "L3", "Left Stick Click", "Z"),
    pad("r3", "rightstick", "R3", "Right Stick Click", "C"),
    pad("up", "dpup", "Up", "D-Pad Up", "I"),
    pad("down", "dpdown", "Down", "D-Pad Down", "K"),
    pad("left", "dpleft", "Left", "D-Pad Left", "J"),
    pad("right", "dpright", "Right", "D-Pad Right", "L"),
];

fn load_bindings(file: &std::path::Path) -> [Pad; 15] {
    let mut mapping = DEFAULT_PAD;
    for (key, physical) in profile::read(file)
        .split(',')
        .filter_map(|s| s.split_once('='))
    {
        if let (Some(index), Some(assigned)) = (
            DEFAULT_PAD.iter().position(|pad| pad.virtual_button == key),
            DEFAULT_PAD.iter().find(|pad| pad.physical == physical),
        ) {
            mapping[index] = *assigned;
        }
    }
    mapping
}

fn bindings_value(mapping: &[Pad; 15]) -> String {
    DEFAULT_PAD
        .iter()
        .zip(mapping)
        .filter(|(default, assigned)| default.physical != assigned.physical)
        .map(|(default, assigned)| format!("{}={}", default.virtual_button, assigned.physical))
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(windows)]
fn center_window(window: &rune::WindowHandle) {
    use rune::raw_window_handle::RawWindowHandle;
    use windows_sys::Win32::{
        Foundation::RECT,
        Graphics::Gdi::{
            GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow,
        },
        UI::WindowsAndMessaging::{
            GetWindowRect, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER, SetWindowPos,
        },
    };

    let _ = window.with_raw_handle(|handle| {
        let RawWindowHandle::Win32(handle) = handle.as_raw() else {
            return;
        };
        let hwnd = handle.hwnd.get() as _;
        let mut frame = RECT::default();
        let mut display = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..MONITORINFO::default()
        };
        // The borrowed HWND belongs to Rune's hidden window and is live in this callback.
        unsafe {
            #[link(name = "dwmapi")]
            unsafe extern "system" {
                fn DwmSetWindowAttribute(
                    hwnd: *mut std::ffi::c_void,
                    dw_attribute: u32,
                    pv_attribute: *const std::ffi::c_void,
                    cb_attribute: u32,
                ) -> i32;
            }
            const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;
            const DWMWCP_ROUND: u32 = 2;
            let pref = DWMWCP_ROUND;
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_WINDOW_CORNER_PREFERENCE,
                (&raw const pref).cast(),
                std::mem::size_of::<u32>() as u32,
            );
            let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
            if GetWindowRect(hwnd, &mut frame) == 0 || GetMonitorInfoW(monitor, &mut display) == 0 {
                return;
            }
            let area = display.rcMonitor;
            let x = area.left + ((area.right - area.left) - (frame.right - frame.left)) / 2;
            let y = area.top + ((area.bottom - area.top) - (frame.bottom - frame.top)) / 2;
            SetWindowPos(
                hwnd,
                std::ptr::null_mut(),
                x,
                y,
                0,
                0,
                SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOZORDER,
            );
        }
    });
}

#[cfg(windows)]
fn start_gamepad_poller(handle: AppHandle<App>, playing: Arc<AtomicBool>) {
    use std::ffi::c_void;
    use std::time::Duration;

    #[repr(C)]
    #[derive(Default)]
    struct XInputGamepad {
        buttons: u16,
        left_trigger: u8,
        right_trigger: u8,
        thumb_lx: i16,
        thumb_ly: i16,
        thumb_rx: i16,
        thumb_ry: i16,
    }

    #[repr(C)]
    #[derive(Default)]
    struct XInputState {
        packet_number: u32,
        gamepad: XInputGamepad,
    }

    unsafe extern "system" {
        fn LoadLibraryA(name: *const u8) -> *mut c_void;
        fn GetProcAddress(module: *mut c_void, name: *const u8) -> *const c_void;
    }

    type GetStateFn = unsafe extern "system" fn(u32, *mut XInputState) -> u32;

    let _ = std::thread::Builder::new()
        .name("PistonPort gamepad".into())
        .spawn(move || {
            let get_state: GetStateFn = unsafe {
                let mut module = LoadLibraryA(c"xinput1_4.dll".as_ptr().cast());
                if module.is_null() {
                    module = LoadLibraryA(c"xinput9_1_0.dll".as_ptr().cast());
                }
                if module.is_null() {
                    return;
                }
                let symbol = GetProcAddress(module, c"XInputGetState".as_ptr().cast());
                if symbol.is_null() {
                    return;
                }
                std::mem::transmute(symbol)
            };
            let mut prev = [0u16; 4];
            loop {
                std::thread::sleep(Duration::from_millis(50));
                if playing.load(Ordering::Relaxed) {
                    prev = [0u16; 4];
                    continue;
                }
                for (slot, previous) in prev.iter_mut().enumerate() {
                    let mut state = XInputState::default();
                    let status = unsafe { get_state(slot as u32, &mut state) };
                    if status != 0 {
                        *previous = 0;
                        continue;
                    }
                    let buttons = state.gamepad.buttons;
                    let pressed = buttons & !*previous;
                    *previous = buttons;
                    let action = if pressed & 0x0100 != 0 {
                        Some(GamepadAction::PrevTab)
                    } else if pressed & 0x0200 != 0 {
                        Some(GamepadAction::NextTab)
                    } else if pressed & (0x1000 | 0x0010) != 0 {
                        Some(GamepadAction::Confirm)
                    } else if pressed & 0x2000 != 0 {
                        Some(GamepadAction::Back)
                    } else {
                        None
                    };
                    if let Some(action) = action {
                        handle.update(move |app, tx| app.update(Message::Gamepad(action), tx));
                    }
                }
            }
        });
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let paths = Paths::discover()?;
    let direct_play = std::env::args()
        .skip(1)
        .any(|arg| arg == "--play" || arg == "--launch");
    if direct_play {
        let source = profile::read(&paths.profile.join("game-folder.txt"));
        if !source.is_empty() {
            let _ = paths.migrate_retired_save_selection();
            return match paths.play(&source) {
                Ok(_) => Ok(()),
                Err(error) => Err(error.into()),
            };
        }
    }
    let instance = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(paths.profile.join(".pistonport-launcher.lock"))?;
    if fs2::FileExt::try_lock_exclusive(&instance).is_err() {
        return Ok(());
    }
    #[cfg(windows)]
    rune::set_app_icon_resource(1);
    let spec = AppSpec {
        setup: Some(App::setup),
        min_size: Some((860.0, 525.0)),
        ..AppSpec::new(
            "PistonPort",
            (1024.0, 600.0),
            || unreachable!("The launcher acquires its own profile before opening"),
            view::view,
        )
    };
    let native_window = Rc::new(RefCell::new(None));
    let remember_window = native_window.clone();
    let app = rune::App::new(spec).on_window(move |window| {
        #[cfg(windows)]
        center_window(window);
        *remember_window.borrow_mut() = Some(window.clone());
    });
    let mut rune_args: Vec<String> = std::env::args().skip(1).collect();
    if !rune_args.iter().any(|arg| arg == "--scroll-bars") {
        rune_args.push("--scroll-bars".into());
        rune_args.push("always".into());
    }
    let handle = app.handle();
    app.args(rune_args)
        .init(move || App::open(paths, handle, native_window))
        .window_options(WindowOptions {
            borderless: true,
            ..WindowOptions::default()
        })
        .run();
    Ok(())
}
