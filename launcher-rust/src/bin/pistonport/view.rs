use std::sync::Arc;

use rune::{
    Align, Cx, El, Icon, Image, ImageCache, ScrollSpec, SurfaceFit, TextStyle, col, el, row, text,
};
use rune_kit::{
    Button, ButtonVariant, ControlSize, IconButton, Kit, Placement, Select, SelectAppearance,
    Switch, TextInput,
};

use crate::icons;
use crate::profile::{MODES, RATES, SIZES, Slot};
use crate::trophy_data::{Tier, tier_color};
use crate::{App, DEFAULT_PAD, Message, Page, Picker};

const ICON: &[u8] = include_bytes!("../../../../pistonport/launcher/pistonport.png");
const DUALSHOCK_STICK: &[u8] = include_bytes!("../../../assets/pads/dualshock4/leftstick.png");
const XBOX_STICK: &[u8] = include_bytes!("../../../assets/pads/xbox/leftstick.png");

macro_rules! pad_art {
    ($system:literal, $name:literal) => {
        include_bytes!(concat!(
            "../../../assets/pads/icons/",
            $system,
            "/",
            $name,
            ".png"
        ))
    };
}

const DUALSHOCK_ART: [&[u8]; 15] = [
    pad_art!("playstation", "playstation_trigger_r2"),
    pad_art!("playstation", "playstation_trigger_l2"),
    pad_art!("playstation", "playstation_button_color_square"),
    pad_art!("playstation", "playstation_button_color_cross"),
    pad_art!("playstation", "playstation_button_color_circle"),
    pad_art!("playstation", "playstation4_button_options"),
    pad_art!("playstation", "playstation_button_color_triangle"),
    pad_art!("playstation", "playstation_trigger_l1"),
    pad_art!("playstation", "playstation_trigger_r1"),
    pad_art!("playstation", "playstation_button_l3"),
    pad_art!("playstation", "playstation_button_r3"),
    pad_art!("playstation", "playstation_dpad_up"),
    pad_art!("playstation", "playstation_dpad_down"),
    pad_art!("playstation", "playstation_dpad_left"),
    pad_art!("playstation", "playstation_dpad_right"),
];

const XBOX_ART: [&[u8]; 15] = [
    pad_art!("xbox", "xbox_rt"),
    pad_art!("xbox", "xbox_lt"),
    pad_art!("xbox", "xbox_button_color_x"),
    pad_art!("xbox", "xbox_button_color_a"),
    pad_art!("xbox", "xbox_button_color_b"),
    pad_art!("xbox", "xbox_button_menu"),
    pad_art!("xbox", "xbox_button_color_y"),
    pad_art!("xbox", "xbox_lb"),
    pad_art!("xbox", "xbox_rb"),
    pad_art!("xbox", "xbox_ls"),
    pad_art!("xbox", "xbox_rs"),
    pad_art!("xbox", "xbox_dpad_up"),
    pad_art!("xbox", "xbox_dpad_down"),
    pad_art!("xbox", "xbox_dpad_left"),
    pad_art!("xbox", "xbox_dpad_right"),
];
const XBOX_LABELS: [&str; 15] = [
    "RT",
    "LT",
    "X",
    "A",
    "B",
    "Menu",
    "Y",
    "LB",
    "RB",
    "LS Click",
    "RS Click",
    "D-Pad Up",
    "D-Pad Down",
    "D-Pad Left",
    "D-Pad Right",
];

pub struct Artwork {
    pub cache: Arc<ImageCache>,
    pub cover: Option<Image>,
    icon: Image,
    dualshock_stick: Image,
    xbox_stick: Image,
    dualshock: [Image; 15],
    xbox: [Image; 15],
}

impl Artwork {
    pub fn load(cache: Arc<ImageCache>) -> Self {
        Self {
            icon: cache.load(ICON),
            dualshock_stick: cache.load(DUALSHOCK_STICK),
            xbox_stick: cache.load(XBOX_STICK),
            dualshock: DUALSHOCK_ART.map(|bytes| cache.load(bytes)),
            xbox: XBOX_ART.map(|bytes| cache.load(bytes)),
            cover: None,
            cache,
        }
    }
}

fn body(kit: &Kit, value: impl Into<String>) -> El {
    text(value).font(kit.ty.ui).color(kit.c.text_base)
}

fn muted(kit: &Kit, value: impl Into<String>) -> El {
    text(value).font(kit.ty.ui).color(kit.c.text_muted)
}

fn heading(kit: &Kit, value: &str) -> El {
    text(value)
        .font(rune::tok::type_::TITLE)
        .color(kit.c.text_base)
}

fn medium(kit: &Kit, value: &str) -> El {
    text(value).font(kit.ty.ui_medium).color(kit.c.text_base)
}

fn art(image: Option<&Image>, width: f32, height: f32) -> El {
    let area = el().size(width, height);
    if let Some(image) = image {
        area.pixels_fit(image.pixels(), SurfaceFit::Fit)
    } else {
        area
    }
}

fn action(
    kit: &Kit,
    key: &str,
    label: &str,
    glyph: &'static Icon,
    message: Option<Message>,
    variant: ButtonVariant,
) -> El {
    let mut button = Button::new(label)
        .icon(glyph)
        .variant(variant)
        .control_size(ControlSize::Large)
        .disabled(message.is_none());
    if let Some(message) = message {
        button = button.on_press_tx(move |app: &mut App, tx| app.update(message.clone(), tx));
    }
    button.build(kit).key(key.to_owned())
}

fn caption(kit: &Kit, key: &str, label: &str, glyph: &'static Icon, message: Message) -> El {
    let danger = matches!(message, Message::Close);
    let button = IconButton::new(glyph)
        .control_size(ControlSize::Large)
        .label(label)
        .on_press_tx(move |app: &mut App, tx| app.update(message.clone(), tx))
        .build(kit)
        .key(key.to_owned());
    if danger {
        button.hover(kit.c.danger.fg.alpha(0.45))
    } else {
        button
    }
}

pub fn view(app: &App, _: &mut Cx<App>) -> El {
    let mut kit = Kit::neutral(true);
    kit.glyph.chevron_down = &icons::CHEVRON_DOWN;
    kit.glyph.chevron_up = &icons::CHEVRON_UP;
    kit.glyph.check_small = &icons::CHECK;
    let mut overlays = Vec::new();
    let assets = app.assets.as_ref();
    let brand = row()
        .gap(8.0)
        .center_y()
        .window_drag()
        .child(art(assets.map(|a| &a.icon), 30.0, 30.0))
        .child(
            text("PistonPort")
                .font(TextStyle::new(18.0, 650.0))
                .color(kit.c.text_base),
        );
    let mut header = row()
        .key("header")
        .h(52.0)
        .px(18.0)
        .gap(7.0)
        .fill(kit.c.layer_01)
        .center_y()
        .child(brand)
        .child(el().grow(1.0).h_full().window_drag());
    if app.page != Page::Welcome {
        let tabs = [
            ("Garage", "garage", &icons::FLAG, Page::Garage),
            ("Saves", "saves", &icons::SAVE, Page::Saves),
            ("Settings", "settings", &icons::SETTINGS, Page::Settings),
            ("Controls", "controls", &icons::GAMEPAD, Page::Controls),
            ("Trophies", "trophies", &icons::TROPHY, Page::Trophies),
        ];
        let nav = row()
            .key("navigation")
            .gap(4.0)
            .children(tabs.into_iter().map(|(label, key, glyph, page)| {
                let variant = if app.page == page {
                    ButtonVariant::Neutral
                } else {
                    ButtonVariant::GhostMuted
                };
                action(&kit, key, label, glyph, Some(Message::Page(page)), variant)
            }));
        header = header
            .child(nav)
            .child(el().grow(1.0).h_full().window_drag());
    }
    header = header
        .child(caption(
            &kit,
            "minimize",
            "Minimize",
            &icons::MINUS,
            Message::Minimize,
        ))
        .child(caption(
            &kit,
            "close",
            "Close",
            &icons::CLOSE,
            Message::Close,
        ));
    let page = match app.page {
        Page::Welcome => welcome(app, &kit),
        Page::Garage => garage(app, &kit),
        Page::Saves => saves(app, &kit),
        Page::Settings => settings(app, &kit, &mut overlays),
        Page::Controls => controls(app, &kit, &mut overlays),
        Page::Trophies => trophies(app, &kit),
    };
    let mut shell = col()
        .w_full()
        .h_full()
        .fill(kit.c.bg_base)
        .items_stretch()
        .child(header)
        .child(page);
    if app.page != Page::Welcome {
        shell = shell.child(footer(app, &kit, &mut overlays));
    }
    rune_kit::root(shell, overlays)
}

fn source_actions(app: &App, kit: &Kit) -> El {
    row()
        .gap(8.0)
        .center_y()
        .child(action(
            kit,
            "pick-pkg",
            "Choose PKG…",
            &icons::PACKAGE,
            (!app.busy).then_some(Message::Pick(Picker::Package)),
            ButtonVariant::Contrast,
        ))
        .child(action(
            kit,
            "pick-folder",
            "Choose Folder…",
            &icons::FOLDER,
            (!app.busy).then_some(Message::Pick(Picker::Folder)),
            ButtonVariant::Neutral,
        ))
}

fn welcome(app: &App, kit: &Kit) -> El {
    let mut page = col()
        .key("welcome-page")
        .grow(1.0)
        .min_h(0.0)
        .center()
        .gap(14.0)
        .padding(28.0)
        .child(heading(kit, "Choose Your Game Source"));
    if !app.busy {
        page = page
            .child(muted(
                kit,
                "Select a playable PS4 PKG or the folder containing eboot.bin.",
            ))
            .child(source_actions(app, kit));
    }
    if !app.status.is_empty() {
        page = page.child(body(kit, &app.status).color(if app.error {
            kit.c.danger.fg
        } else {
            kit.c.text_muted
        }));
    }
    page
}

fn garage(app: &App, kit: &Kit) -> El {
    let page = el()
        .key("garage-page")
        .grow(1.0)
        .min_h(0.0)
        .fill(kit.c.bg_base);
    if let Some(cover) = app.assets.as_ref().and_then(|a| a.cover.as_ref()) {
        page.pixels_fit(cover.pixels(), SurfaceFit::Fill)
            .surface_anchor(Align::Start, Align::Start)
    } else {
        page.center().child(heading(kit, "Cars 3: Driven to Win"))
    }
}

fn footer(app: &App, kit: &Kit, overlays: &mut Vec<El>) -> El {
    let frame = row()
        .key("footer")
        .h(74.0)
        .pb(8.0)
        .px(22.0)
        .center_y()
        .gap(14.0)
        .fill(kit.c.layer_01);
    if app.page == Page::Garage {
        let mut play = col().center().gap(4.0).child(
            action(
                kit,
                "play",
                if app.playing { "Playing" } else { "Play" },
                &icons::PLAY,
                (!app.busy).then_some(Message::Play),
                ButtonVariant::Contrast,
            )
            .w(140.0),
        );
        if app.error || app.busy {
            play = play.child(body(kit, &app.status).color(if app.error {
                kit.c.danger.fg
            } else {
                kit.c.text_muted
            }));
        }
        return frame.center().child(play);
    }
    let status = col()
        .min_w(0.0)
        .grow(1.0)
        .gap(3.0)
        .child(body(kit, &app.status).color(if app.error {
            kit.c.danger.fg
        } else {
            kit.c.text_base
        }))
        .child(muted(
            kit,
            format!("{}  ·  {}  ·  Vulkan", app.settings.rate, app.settings.mode),
        ));
    let frame = frame.child(status);
    if !matches!(app.page, Page::Settings | Page::Trophies) {
        return frame;
    }
    let slot = selection(
        "save-slot",
        "Save Slot",
        &app.selected.label,
        app.slots
            .iter()
            .map(|slot| (slot.clone(), slot.label.clone()))
            .collect(),
        240.0,
        Placement::TopEnd,
        app,
        kit,
        overlays,
        Message::SelectSlot,
    );
    frame.child(
        row()
            .items_end()
            .gap(8.0)
            .child(col().gap(3.0).child(muted(kit, "Save Slot")).child(slot))
            .child(action(
                kit,
                "load-save",
                "Load Save…",
                &icons::UPLOAD,
                (!app.busy).then_some(Message::Pick(Picker::Save)),
                ButtonVariant::Neutral,
            )),
    )
}

fn selection<T: Clone + 'static>(
    key: &str,
    title: &str,
    current: &str,
    options: Vec<(T, String)>,
    width: f32,
    placement: Placement,
    app: &App,
    kit: &Kit,
    overlays: &mut Vec<El>,
    choose: impl Fn(T) -> Message + Clone + 'static,
) -> El {
    let labels: Vec<_> = options.iter().map(|(_, label)| label.clone()).collect();
    let toggle = key.to_owned();
    let first_overlay = overlays.len();
    let trigger = Select::new(key, &labels)
        .label(title)
        .current(Some(current))
        .width(width)
        .appearance(SelectAppearance::Field)
        .placement(placement)
        .disabled(app.busy)
        .open(app.menu.as_deref() == Some(key))
        .on_toggle(move |app: &mut App| {
            app.menu = (app.menu.as_ref() != Some(&toggle)).then(|| toggle.clone());
        })
        .on_close(|app: &mut App| app.menu = None)
        .on_choose(move |app: &mut App, label, tx| {
            app.menu = None;
            if let Some((choice, _)) = options.iter().find(|(_, name)| name == label) {
                app.update(choose(choice.clone()), tx);
            }
        })
        .build(kit, overlays);
    // Rune mounts the menu surface last. Widen it for the more descriptive choices.
    if let Some(menu) = overlays[first_overlay..].last_mut() {
        menu.set_min_w(if key == "save-slot" {
            365.0
        } else {
            width.max(160.0)
        });
    }
    trigger
}

fn page_frame(kit: &Kit, key: &str, title: &str, actions: El) -> El {
    col()
        .key(key.to_owned())
        .grow(1.0)
        .min_h(0.0)
        .items_stretch()
        .gap(14.0)
        .px(26.0)
        .py(15.0)
        .child(
            row()
                .justify_between()
                .center_y()
                .child(heading(kit, title))
                .child(actions),
        )
}

fn setting_row(kit: &Kit, label: &str, control: El) -> El {
    row()
        .w_full()
        .h(36.0)
        .justify_between()
        .center_y()
        .gap(10.0)
        .child(body(kit, label))
        .child(control)
}

fn settings(app: &App, kit: &Kit, overlays: &mut Vec<El>) -> El {
    use crate::profile::Choice;
    let cfg = &app.settings;
    let active = !app.busy;
    let field =
        |key, title, current, options, layers: &mut Vec<El>, changed: fn(Choice) -> Message| {
            selection(
                key,
                title,
                current,
                options,
                162.0,
                Placement::BottomEnd,
                app,
                kit,
                layers,
                changed,
            )
        };
    let mode = field(
        "display-mode",
        "Display Mode",
        cfg.mode.1,
        MODES.iter().map(|c| (*c, c.1.into())).collect(),
        overlays,
        Message::Mode,
    );
    let rate = field(
        "frame-limit",
        "Frame Rate Limit",
        cfg.rate.1,
        RATES.iter().map(|c| (*c, c.1.into())).collect(),
        overlays,
        Message::Rate,
    );
    let resolution = SIZES
        .iter()
        .find(|c| c.0 == cfg.size)
        .map_or(cfg.size.as_str(), |c| c.1);
    let size = if cfg.mode.0 == "windowed" {
        selection(
            "window-size",
            "Window Size",
            resolution,
            SIZES.iter().map(|c| (c.0.to_owned(), c.1.into())).collect(),
            162.0,
            Placement::BottomEnd,
            app,
            kit,
            overlays,
            Message::Size,
        )
    } else {
        row().w(162.0).justify_end().child(muted(kit, resolution))
    };
    let volume = selection(
        "volume",
        "Master Volume",
        &format!("{}%", cfg.volume),
        (0..=200)
            .step_by(5)
            .map(|value| (value, format!("{value}%")))
            .collect(),
        100.0,
        Placement::BottomEnd,
        app,
        kit,
        overlays,
        Message::Volume,
    );
    let identity = app
        .game_identity
        .as_ref()
        .filter(|(path, _)| path == app.source.text.trim())
        .map(|(_, identity)| muted(kit, identity).key("game-identity"));
    let source = TextInput::new(&app.source)
        .width(None)
        .label("Game Source")
        .placeholder("Choose a playable PKG or extracted game folder")
        .disabled(!active)
        .on_edit(|app: &mut App| &mut app.source)
        .on_change(|app: &mut App, _| app.dirty())
        .build(kit)
        .key("game-source")
        .grow(1.0)
        .min_w(0.0);
    let display = col()
        .grow(1.0)
        .min_w(0.0)
        .gap(4.0)
        .child(medium(kit, "Display"))
        .child(setting_row(kit, "Display Mode", mode))
        .child(setting_row(kit, "Window Size", size))
        .child(setting_row(kit, "Frame Rate Limit", rate));
    let game = col()
        .grow(1.0)
        .min_w(0.0)
        .gap(4.0)
        .child(medium(kit, "In Game"))
        .child(setting_row(
            kit,
            "Show Frame Rate",
            Switch::new(cfg.fps)
                .label("Show Frame Rate")
                .hide_label()
                .disabled(!active)
                .on_toggle_tx(|app: &mut App, tx| app.update(Message::Fps(!app.settings.fps), tx))
                .build(kit)
                .key("show-fps"),
        ))
        .child(setting_row(
            kit,
            "Show Keyboard Hints",
            Switch::new(cfg.keyboard_hints)
                .label("Show Keyboard Hints")
                .hide_label()
                .disabled(!active)
                .on_toggle_tx(|app: &mut App, tx| {
                    app.update(Message::KeyboardHints(!app.settings.keyboard_hints), tx)
                })
                .build(kit)
                .key("show-keyboard-hints"),
        ))
        .child(setting_row(
            kit,
            "Vertical Sync (VSync)",
            Switch::new(cfg.vsync)
                .label("Vertical Sync (VSync)")
                .hide_label()
                .disabled(!active)
                .on_toggle_tx(|app: &mut App, tx| {
                    app.update(Message::Vsync(!app.settings.vsync), tx)
                })
                .build(kit)
                .key("vsync"),
        ))
        .child(setting_row(kit, "Master Volume", volume));
    page_frame(
        kit,
        "settings-page",
        "Settings",
        action(
            kit,
            "save-settings",
            "Save Settings",
            &icons::SAVE,
            active.then_some(Message::Save),
            ButtonVariant::Neutral,
        ),
    )
    .child(
        col()
            .gap(6.0)
            .child(
                row()
                    .justify_between()
                    .center_y()
                    .child(medium(kit, "Game Source"))
                    .children(identity),
            )
            .child(
                row()
                    .center_y()
                    .gap(8.0)
                    .child(source)
                    .child(source_actions(app, kit)),
            ),
    )
    .child(scroll(
        "settings-scroll",
        row().w_full().gap(32.0).child(display).child(game),
    ))
}

fn scroll(key: &str, content: El) -> El {
    col()
        .key(key.to_owned())
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .scroll(ScrollSpec {
            stick_bottom: false,
        })
        .child(content)
}

fn control_columns(action: El, keyboard: El, playstation: El, xbox: El) -> El {
    row()
        .w_full()
        .center_y()
        .h(35.0)
        .gap(12.0)
        .px(10.0)
        .child(el().grow(1.0).min_w(0.0).child(action))
        .child(el().w(115.0).child(keyboard))
        .child(el().w(203.0).child(playstation))
        .child(el().w(147.0).child(xbox))
}

fn controls(app: &App, kit: &Kit, overlays: &mut Vec<El>) -> El {
    let active = !app.busy;
    let actions = row()
        .gap(8.0)
        .child(action(
            kit,
            "reset-controls",
            "Restore Defaults",
            &icons::RESET,
            active.then_some(Message::Reset),
            ButtonVariant::Neutral,
        ))
        .child(action(
            kit,
            "save-controls",
            "Save Controls",
            &icons::SAVE,
            active.then_some(Message::Save),
            ButtonVariant::Neutral,
        ));
    let headings = control_columns(
        medium(kit, "Action"),
        medium(kit, "Keyboard"),
        medium(kit, "DualShock"),
        medium(kit, "Xbox Controller"),
    )
    .fill(kit.c.layer_01);
    let steer = control_columns(
        body(kit, "Steer"),
        body(kit, "A / D"),
        row()
            .gap(7.0)
            .center_y()
            .child(art(
                app.assets.as_ref().map(|a| &a.dualshock_stick),
                21.0,
                21.0,
            ))
            .child(body(kit, "Left Stick")),
        row()
            .gap(7.0)
            .center_y()
            .child(art(app.assets.as_ref().map(|a| &a.xbox_stick), 21.0, 21.0))
            .child(body(kit, "Left Stick")),
    );
    let mut list = col().w_full().child(steer);
    for (index, input) in DEFAULT_PAD.iter().enumerate() {
        let assigned = app.pad[index];
        let mapped = DEFAULT_PAD
            .iter()
            .position(|p| *p == assigned)
            .unwrap_or(index);
        let dualshock = app.assets.as_ref().map(|a| &a.dualshock[mapped]);
        let xbox = app.assets.as_ref().map(|a| &a.xbox[mapped]);
        let select = selection(
            &format!("pad-{}", input.virtual_button),
            input.action,
            assigned.label,
            DEFAULT_PAD.iter().map(|p| (*p, p.label.into())).collect(),
            147.0,
            Placement::BottomStart,
            app,
            kit,
            overlays,
            move |pad| Message::Map(index, pad),
        );
        list = list.child(
            control_columns(
                body(kit, input.action),
                body(kit, input.key),
                row()
                    .gap(7.0)
                    .center_y()
                    .child(art(dualshock, 21.0, 21.0))
                    .child(select),
                row()
                    .gap(7.0)
                    .center_y()
                    .child(art(xbox, 21.0, 21.0))
                    .child(body(kit, XBOX_LABELS[mapped])),
            )
            .key(input.virtual_button),
        );
    }
    page_frame(kit, "controls-page", "Controls", actions)
        .child(headings)
        .child(scroll("controls-scroll", list))
}

fn save_row(app: &App, kit: &Kit, slot: &Slot, detail: &str) -> El {
    let label = if slot.id == "default" {
        "Use My Progress"
    } else {
        "Use Save"
    };
    let control = if app.selected.id == slot.id {
        muted(kit, "Active")
    } else {
        action(
            kit,
            &format!("use-save-{}", slot.id),
            label,
            &icons::CHECK,
            (!app.busy).then_some(Message::SelectSlot(slot.clone())),
            ButtonVariant::Neutral,
        )
    };
    let mut info = col().gap(3.0).grow(1.0).min_w(0.0).child(medium(
        kit,
        slot.label
            .strip_suffix(" (Built-In)")
            .unwrap_or(&slot.label),
    ));
    if !detail.is_empty() {
        info = info.child(muted(kit, detail));
    }
    row()
        .key(format!("save-row-{}", slot.id))
        .w_full()
        .h(48.0)
        .px(10.0)
        .gap(12.0)
        .center_y()
        .fill(kit.c.layer_01)
        .radius(kit.radius.md)
        .child(info)
        .child(control)
}

fn saves(app: &App, kit: &Kit) -> El {
    let mut list = col().w_full().gap(7.0);
    if let Some(slot) = app.slots.iter().find(|slot| slot.id == "default") {
        list = list.child(save_row(app, kit, slot, "Start or continue your own game."));
    }
    list = list.child(col().pt(9.0).child(medium(kit, "Built-In Save")));
    for slot in app.slots.iter().filter(|slot| slot.is_builtin()) {
        list = list.child(save_row(
            app,
            kit,
            slot,
            "Smokey Unlocked · 136 / 136 Skill Checks · 22 / 22 Trophies",
        ));
    }
    let mut imported = app
        .slots
        .iter()
        .filter(|slot| slot.id != "default" && !slot.is_builtin())
        .peekable();
    if imported.peek().is_some() {
        list = list.child(col().pt(9.0).child(medium(kit, "Imported Saves")));
        for slot in imported {
            list = list.child(save_row(app, kit, slot, ""));
        }
    }
    page_frame(
        kit,
        "saves-page",
        "Saves",
        action(
            kit,
            "import-save",
            "Import Save…",
            &icons::UPLOAD,
            (!app.busy).then_some(Message::Pick(Picker::Save)),
            ButtonVariant::Contrast,
        ),
    )
    .child(scroll("saves-scroll", list))
}

fn trophies(app: &App, kit: &Kit) -> El {
    const STATUS_WIDTH: f32 = 102.0;
    let valid_slot = app.slots.contains(&app.selected);
    let summary = match &app.trophies.catalog {
        Some(Ok(rows)) if valid_slot => {
            let unlocked = rows
                .iter()
                .filter(|t| app.trophies.unlocked.contains(&t.id))
                .count();
            format!("{unlocked} / {} Unlocked", rows.len())
        }
        _ => String::new(),
    };
    let page = page_frame(kit, "trophies-page", "Trophies", body(kit, summary))
        .child(muted(kit, &app.selected.label));
    if !valid_slot {
        return page.child(body(kit, "This save slot could not be found.").color(kit.c.danger.fg));
    }
    match &app.trophies.catalog {
        Some(Ok(rows)) => {
            let headings = row()
                .gap(12.0)
                .px(10.0)
                .h(32.0)
                .center_y()
                .fill(kit.c.layer_01)
                .child(el().w(36.0))
                .child(medium(kit, "Trophy").grow(1.0))
                .child(medium(kit, "Tier").w(100.0))
                .child(medium(kit, "Status").w(STATUS_WIDTH));
            let list = col().w_full().children(rows.iter().map(|t| {
                let unlocked = app.trophies.unlocked.contains(&t.id);
                let tier = match t.tier {
                    Tier::Platinum => "Platinum",
                    Tier::Gold => "Gold",
                    Tier::Silver => "Silver",
                    Tier::Bronze => "Bronze",
                };
                let ink = rune::color::hex(tier_color(t.tier)).unwrap_or(kit.c.text_base);
                let (status, status_icon, status_color) = if unlocked {
                    ("Unlocked", &icons::CHECK, kit.c.success.fg)
                } else {
                    ("Locked", &icons::LOCK, kit.c.text_muted)
                };
                row()
                    .key(format!("trophy-{}", t.id))
                    .w_full()
                    .gap(12.0)
                    .center_y()
                    .px(10.0)
                    .py(6.0)
                    .child(art(t.icon.as_ref(), 36.0, 36.0).opacity(if unlocked {
                        1.0
                    } else {
                        0.5
                    }))
                    .child(
                        col()
                            .grow(1.0)
                            .min_w(0.0)
                            .gap(3.0)
                            .child(medium(kit, &t.name))
                            .child(muted(kit, &t.detail)),
                    )
                    .child(body(kit, tier).color(ink).w(100.0))
                    .child(
                        row()
                            .w(STATUS_WIDTH)
                            .gap(6.0)
                            .center_y()
                            .child(
                                rune::icon(status_icon, 15.0)
                                    .color(status_color)
                                    .a11y_hidden(),
                            )
                            .child(body(kit, status).color(status_color)),
                    )
            }));
            page.child(headings).child(scroll("trophies-scroll", list))
        }
        Some(Err(error)) => page
            .child(body(kit, "Could Not Load Trophies").color(kit.c.danger.fg))
            .child(muted(kit, error)),
        None => page.child(muted(kit, "Loading Trophies…")),
    }
}
