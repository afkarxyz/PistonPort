#![cfg_attr(windows, windows_subsystem = "windows")]

mod icons;
mod profile;
mod trophies;
#[allow(dead_code)]
#[path = "../../core/trophy_data.rs"]
mod trophy_data;
mod view;

use std::{cell::RefCell, fs, path::PathBuf, rc::Rc, sync::Arc};

use profile::{Choice, Paths, Settings, Slot, SourceInspection};
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

#[derive(Clone, Copy)]
enum Picker {
    Package,
    Folder,
    Save,
}

#[derive(Clone)]
enum Message {
    Page(Page),
    Pick(Picker),
    Picked(Picker, Option<PathBuf>),
    Inspected(String, Result<GameSource, String>, bool),
    Mode(Choice),
    Size(String),
    Rate(Choice),
    Vsync(bool),
    Fps(bool),
    KeyboardHints(bool),
    Volume(u16),
    Map(usize, Pad),
    Reset,
    Save,
    Commit(String, Result<Option<GameSource>, String>, bool),
    SelectSlot(Slot),
    SaveDone(Result<Vec<u8>, String>, bool),
    TrophiesLoaded(String, Result<Vec<Trophy>, String>),
    Play,
    GameDone(Result<Vec<u8>, String>),
    Minimize,
    Close,
}

#[derive(Clone)]
struct GameSource {
    identity: String,
    cover: Option<Image>,
}

fn load_game_source(
    paths: &Paths,
    source: &str,
    cache: Option<&ImageCache>,
) -> Result<GameSource, String> {
    let SourceInspection { identity, cover } = paths.inspect(source)?;
    let cover = cache
        .filter(|_| !cover.is_empty())
        .map(|cache| cache.load(cover))
        .filter(|image| image.wait().is_ok());
    Ok(GameSource { identity, cover })
}

struct App {
    paths: Paths,
    handle: AppHandle<Self>,
    native_window: Rc<RefCell<Option<WindowHandle>>>,
    page: Page,
    settings: Settings,
    source: TextEdit,
    saved_source: String,
    game_identity: Option<(String, String)>,
    assets: Option<view::Artwork>,
    pad: [Pad; 15],
    slots: Vec<Slot>,
    selected: Slot,
    trophies: TrophyPage,
    menu: Option<String>,
    busy: bool,
    playing: bool,
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
        let error = migration_error.is_some();
        let (slots, selected) = profile::slots(&paths.profile);
        let source = profile::read(&paths.profile.join("game-folder.txt"));
        Self {
            settings: Settings::load(&paths.profile),
            pad: load_bindings(&paths.profile.join("controller.map")),
            page: Page::Welcome,
            source: TextEdit::new(&source),
            saved_source: source.clone(),
            game_identity: None,
            assets: None,
            handle,
            native_window,
            paths,
            slots,
            selected,
            trophies: TrophyPage::default(),
            menu: None,
            busy: false,
            playing: false,
            status: migration_error.unwrap_or_default(),
            error,
        }
    }

    fn setup(&mut self, wake: Waker) {
        let cache = Arc::new(ImageCache::new(wake));
        self.assets = Some(view::Artwork::load(cache));
        if !self.source.text.is_empty() {
            self.busy = true;
            self.report("Checking Game Source…", false);
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
        self.status = message.into();
        self.error = error;
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
        let cover = if let Some(inspection) = inspection {
            self.game_identity = Some((source.to_owned(), inspection.identity));
            inspection.cover
        } else {
            self.game_identity = None;
            None
        };
        if let Some(assets) = &mut self.assets {
            assets.cover = cover;
        }
    }

    fn inspect(&mut self, path: String, picked: bool) {
        let paths = self.paths.clone();
        let cache = self.assets.as_ref().map(|assets| assets.cache.clone());
        let first_selection = picked && self.page == Page::Welcome;
        self.background(move || {
            let result = load_game_source(&paths, &path, cache.as_deref()).and_then(|inspection| {
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
        let cache = self.assets.as_ref().map(|assets| assets.cache.clone());
        let settings = self.settings.clone();
        let pad = self.pad;
        self.background(move || {
            let result = (|| {
                let inspection = if !source.is_empty() && (changed_source || play) {
                    Some(load_game_source(&paths, &source, cache.as_deref())?)
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
                            self.report("Ready to Play", false);
                        }
                        Err(error) => self.report(error, true),
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
                        Err(error) => self.report(error, true),
                    }
                } else if self.source.text == path
                    && let Ok(inspection) = result
                {
                    self.set_source_info(&path, Some(inspection));
                }
            }
            Message::Mode(value) if !self.busy => {
                self.settings.mode = value;
                self.dirty();
            }
            Message::Size(value) if !self.busy && self.settings.mode.0 == "windowed" => {
                self.settings.size = value;
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
            Message::Save if !self.busy => self.commit(false),
            Message::Play if !self.busy => self.commit(true),
            Message::Commit(source, result, play) => {
                self.busy = false;
                match result {
                    Err(error) => self.report(error, true),
                    Ok(inspection) => {
                        if source != self.saved_source || play {
                            self.set_source_info(&source, inspection);
                        }
                        self.source = TextEdit::new(&source);
                        self.saved_source = source.clone();
                        self.report("Settings Saved", false);
                        if play {
                            self.busy = true;
                            self.playing = true;
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
            Message::SaveDone(result, imported) => {
                self.busy = false;
                (self.slots, self.selected) = profile::slots(&self.paths.profile);
                self.refresh_trophy_progress();
                match result {
                    Ok(_) => self.report(
                        if imported {
                            "Imported Save Selected"
                        } else {
                            "Save Slot Selected"
                        },
                        false,
                    ),
                    Err(error) => self.report(error, true),
                }
            }
            Message::TrophiesLoaded(source, result) if source == self.trophies.source => {
                self.trophies.catalog = Some(result);
            }
            Message::GameDone(result) => {
                self.busy = false;
                self.playing = false;
                self.settings = Settings::load(&self.paths.profile);
                self.pad = load_bindings(&self.paths.profile.join("controller.map"));
                (self.slots, self.selected) = profile::slots(&self.paths.profile);
                self.refresh_trophy_progress();
                match result {
                    Ok(_) => self.report("Ready to Play", false),
                    Err(error) => self.report(error, true),
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let paths = Paths::discover()?;
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
    let handle = app.handle();
    app.init(move || App::open(paths, handle, native_window))
        .window_options(WindowOptions {
            borderless: true,
            ..WindowOptions::default()
        })
        .run();
    Ok(())
}
