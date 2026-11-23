use std::sync::Arc;

use rune::{
    Align, Cx, El, Icon, Image, ImageCache, ScrollFade, ScrollSpec, Spring, SpringPair,
    SurfaceFit, TextStyle, Transition, col, el, row, text,
};
use astryx_rune::native_ui::{
    Loader, Placement, Select, SelectAppearance, TextInput, Toast, ToastRegion,
};
use astryx_rune::{BadgeColor, Button, Size, Theme, Variant, badge, icon_button};

type Kit = Theme;

use crate::icons;
use crate::profile::{
    ANISOTROPIES, COMPAT_MODES, FOV_SCALES, MODES, Preset, RATES, SHADOW_SIZES, SIZES, Slot,
};
use crate::trophy_data::{Tier, tier_color};
use crate::{App, DEFAULT_PAD, Message, Page, Picker, SettingsTab, ToastNotice};

const ICON: &[u8] = include_bytes!("../../../../pistonport/launcher/pistonport.png");
const DUALSHOCK_STICK: &[u8] =
    include_bytes!("../../../assets/pads/icons/playstation/playstation_stick_l.png");
const XBOX_STICK: &[u8] = include_bytes!("../../../assets/pads/icons/xbox/xbox_stick_l.png");

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

fn enter_only(transition: Transition) -> Transition {
    transition.springs(SpringPair::new(Spring::new(0.2, 0.0), Spring::NONE))
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum ButtonVariant {
    Contrast,
    Neutral,
    GhostMuted,
    Danger,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ControlSize {
    Regular,
    Large,
}

fn piston_button(
    kit: &Kit,
    key: impl Into<String>,
    label: impl Into<String>,
    glyph: Option<&'static Icon>,
    variant: ButtonVariant,
    size: ControlSize,
    disabled: bool,
    on_press: Option<Box<dyn Fn(&mut App, &mut rune::Tx) + 'static>>,
) -> El {
    let astryx_variant = match variant {
        ButtonVariant::Contrast => Variant::Primary,
        ButtonVariant::Neutral => Variant::Secondary,
        ButtonVariant::GhostMuted => Variant::Ghost,
        ButtonVariant::Danger => Variant::Destructive,
    };
    let astryx_size = match size {
        ControlSize::Regular => Size::Small,
        ControlSize::Large => Size::Medium,
    };
    let mut btn = Button::new(label)
        .variant(astryx_variant)
        .size(astryx_size)
        .disabled(disabled);
    if let Some(ic) = glyph {
        btn = btn.icon(ic);
    }
    if let Some(cb) = on_press {
        btn = btn.on_press_tx(move |app: &mut App, tx| cb(app, tx));
    }
    btn.build(kit).key(key.into())
}

fn piston_icon_button(
    kit: &Kit,
    key: impl Into<String>,
    label: impl Into<String>,
    glyph: &'static Icon,
    neutral: bool,
    danger: bool,
    size: ControlSize,
    on_press: impl Fn(&mut App, &mut rune::Tx) + 'static,
) -> El {
    let astryx_size = match size {
        ControlSize::Regular => Size::Small,
        ControlSize::Large => Size::Medium,
    };
    let variant = if danger {
        Variant::Destructive
    } else if neutral {
        Variant::Secondary
    } else {
        Variant::Ghost
    };
    icon_button(label, glyph)
        .variant(variant)
        .size(astryx_size)
        .on_press_tx(on_press)
        .build(kit)
        .key(key.into())
}

fn action(
    kit: &Kit,
    key: &str,
    label: &str,
    glyph: &'static Icon,
    message: Option<Message>,
    variant: ButtonVariant,
) -> El {
    let disabled = message.is_none();
    let cb: Option<Box<dyn Fn(&mut App, &mut rune::Tx) + 'static>> = message.map(|m| {
        let f: Box<dyn Fn(&mut App, &mut rune::Tx) + 'static> =
            Box::new(move |app: &mut App, tx| app.update(m.clone(), tx));
        f
    });
    piston_button(kit, key, label, Some(glyph), variant, ControlSize::Large, disabled, cb)
}

fn caption(kit: &Kit, key: &str, label: &str, glyph: &'static Icon, message: Message) -> El {
    let danger = matches!(message, Message::Close);
    let muted_icon = matches!(message, Message::OpenCoffee);
    piston_icon_button(
        kit,
        key,
        label,
        glyph,
        !muted_icon,
        danger,
        ControlSize::Large,
        move |app: &mut App, tx| app.update(message.clone(), tx),
    )
}

pub fn view(app: &App, cx: &mut Cx<App>) -> El {
    let kit = Theme::neutral(true);
    kit.configure(cx);
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
        )
        .child(muted(&kit, concat!("v", env!("CARGO_PKG_VERSION"))));
    let mut header = row()
        .key("header")
        .h(52.0)
        .px(18.0)
        .gap(8.0)
        .fill(kit.c.layer_01)
        .center_y()
        .child(brand);
    if let Some(Ok(update)) = &app.update_info
        && update.available
    {
        let url = update.url.clone();
        let asset_url = update.asset_url.clone();
        let pill_label = if app.installing_update {
            "Updating…".to_owned()
        } else {
            format!("Update {}", update.tag)
        };
        let pill = piston_button(
            &kit,
            "header-update-pill",
            pill_label,
            Some(&icons::UPLOAD),
            ButtonVariant::Contrast,
            ControlSize::Regular,
            app.installing_update,
            Some(Box::new(move |app: &mut App, tx| {
                if app.page == Page::Settings && app.settings_tab == SettingsTab::System {
                    app.update(Message::InstallUpdate(asset_url.clone(), url.clone()), tx);
                } else {
                    app.page = Page::Settings;
                    app.settings_tab = SettingsTab::System;
                    app.menu = None;
                }
            })),
        )
        .transition(Transition::scale(0.92).combined(Transition::opacity()));
        header = header.child(pill);
    }
    header = header.child(el().grow(1.0).h_full().window_drag());
    if app.page != Page::Welcome {
        let tabs = [
            ("Garage", "garage", &icons::FLAG, Page::Garage),
            ("Settings", "settings", &icons::SETTINGS, Page::Settings),
            ("Controls", "controls", &icons::GAMEPAD, Page::Controls),
            ("Saves", "saves", &icons::SAVE, Page::Saves),
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
        .child(
            caption(
                &kit,
                "buy-coffee",
                "Buy Me a Coffee",
                &icons::COFFEE,
                Message::OpenCoffee,
            )
            .help("Buy Me a Coffee"),
        )
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
        .key("shell")
        .w_full()
        .h_full()
        .fill(kit.c.bg_base)
        .items_stretch()
        .child(header)
        .child(page);
    if app.page != Page::Welcome {
        shell = shell.child(footer(app, &kit, &mut overlays));
    }
    if let Some(notice) = &app.toast {
        overlays.push(ToastRegion::new().toast(toast_card(app, &kit, notice)).build(&kit));
    }
    astryx_rune::root(shell, overlays)
}

fn toast_card(app: &App, kit: &Kit, notice: &ToastNotice) -> El {
    let id = notice.id;
    let mut toast = Toast::new(format!("toast-{id}"), notice.title.clone()).on_close(
        move |app: &mut App| {
            if app.toast.as_ref().is_some_and(|t| t.id == id) {
                app.toast = None;
            }
        },
    );
    if let Some(detail) = &notice.detail {
        toast = toast.description(detail.clone());
    }
    if notice.crash_action && !app.busy {
        toast = toast.action(kit, "Copy Crash Report", false, move |app: &mut App, tx| {
            app.toast = None;
            app.update(Message::CopyCrashReport, tx);
        });
    }
    toast.build(kit)
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
    let page = col()
        .key("welcome-page")
        .grow(1.0)
        .min_h(0.0)
        .center()
        .gap(14.0)
        .padding(28.0);
    if app.busy {
        let label = if app.status.is_empty() {
            "Loading Cars 3: Driven to Win…"
        } else {
            &app.status
        };
        return page
            .child(
                Loader::new()
                    .size(28.0)
                    .color(kit.c.text_base)
                    .build(kit)
                    .key("welcome-loader"),
            )
            .child(muted(kit, label));
    }
    page.child(heading(kit, "Choose Your Game Source"))
        .child(muted(
            kit,
            "Select a playable PS4 PKG or the folder containing eboot.bin.",
        ))
        .child(source_actions(app, kit))
}

fn garage(app: &App, kit: &Kit) -> El {
    let page = el()
        .key("garage-page")
        .grow(1.0)
        .min_h(0.0)
        .fill(kit.c.bg_base)
        .transition(Transition::none());
    let Some(cover) = app.assets.as_ref().and_then(|a| a.cover.as_ref()) else {
        return page;
    };
    let stage = el()
        .key("garage-backdrop-stage")
        .absolute(0.0, 0.0)
        .w_full()
        .h_full()
        .dissolve()
        .child(
            el().key(format!("garage-bg-{}", app.backdrop_seq))
                .absolute(0.0, 0.0)
                .w_full()
                .h_full()
                .pixels_fit(cover.pixels(), SurfaceFit::Fill)
                .surface_anchor(Align::Center, Align::End)
                .transition(Transition::opacity())
                .motion(Spring::new(0.65, 0.0)),
        );
    let play_btn = if app.playing {
        let label = if app.stopping { "Stopping…" } else { "Stop" };
        piston_button(
            kit,
            "stop",
            label,
            Some(&icons::STOP),
            ButtonVariant::Danger,
            ControlSize::Large,
            false,
            (!app.stopping).then(|| {
                let f: Box<dyn Fn(&mut App, &mut rune::Tx) + 'static> =
                    Box::new(|app: &mut App, tx| app.update(Message::StopGame, tx));
                f
            }),
        )
        .w(176.0)
        .h(48.0)
    } else {
        action(
            kit,
            "play",
            "Play",
            &icons::PLAY,
            (!app.busy).then_some(Message::Play),
            ButtonVariant::Contrast,
        )
        .w(176.0)
        .h(48.0)
    };
    let overlay = col()
        .key("garage-play-overlay")
        .absolute(0.0, 0.0)
        .w_full()
        .h_full()
        .justify_center()
        .items_end()
        .pr(44.0)
        .transition(Transition::opacity())
        .motion(Spring::new(0.35, 0.0))
        .child(play_btn);
    page.child(stage).child(overlay)
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
            format!(
                "{}  ·  {}  ·  {}  ·  Vulkan",
                SIZES
                    .iter()
                    .find(|c| c.0 == app.settings.size)
                    .map_or(app.settings.size.as_str(), |c| c.1),
                app.settings.rate,
                app.settings.mode
            ),
        ));
    let frame = frame.child(status);
    if app.page != Page::Trophies {
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
            .center_y()
            .gap(10.0)
            .child(muted(kit, "Save Slot"))
            .child(slot),
    )
}

fn selection<T: Clone + 'static>(
    key: &str,
    title: &str,
    current: &str,
    options: Vec<(T, String)>,
    width: f32,
    _placement: Placement,
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
        menu.set_min_w(if matches!(key, "gpu-adapter" | "display-monitor") {
            280.0
        } else if matches!(key, "compat-mode" | "render-resolution") {
            215.0
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
        .fill(kit.c.bg_base)
        .items_stretch()
        .gap(14.0)
        .px(26.0)
        .py(15.0)
        .transition(enter_only(
            Transition::offset(0.0, 8.0).combined(Transition::opacity()),
        ))
        .child(
            row()
                .justify_between()
                .center_y()
                .child(heading(kit, title))
                .child(actions),
        )
}

fn section_heading(kit: &Kit, label: &str) -> El {
    col()
        .w_full()
        .gap(6.0)
        .pb(4.0)
        .child(
            text(label.to_uppercase())
                .font(TextStyle::new(11.5, 650.0))
                .color(kit.c.text_muted),
        )
        .child(el().w_full().h(1.0).fill(kit.c.border_muted))
}

fn setting_row(kit: &Kit, label: &str, control: El) -> El {
    row()
        .key(format!("row-{label}"))
        .w_full()
        .h(36.0)
        .justify_between()
        .center_y()
        .gap(10.0)
        .child(body(kit, label))
        .child(control)
}

fn toggle_switch(
    kit: &Kit,
    key: &str,
    label: &str,
    checked: bool,
    active: bool,
    on_toggle: impl Fn(&mut App, &mut rune::Tx) + 'static,
) -> El {
    let knob = if checked { 20.0 } else { 16.0 };
    let x = if checked { 18.0 } else { 4.0 };
    let track = el()
        .key(key.to_owned())
        .role("switch")
        .label(label.to_owned())
        .size(40.0, 24.0)
        .radius(12.0)
        .checked(checked)
        .focusable(active)
        .focus_ring(2.0, 1.0, kit.c.border_focus)
        .disabled(!active)
        .motion(rune::tok::motion::SNAPPY)
        .fill(if checked {
            kit.color("accent")
        } else {
            kit.color("track")
        })
        .child(
            el().key("thumb")
                .size(knob, knob)
                .radius(knob / 2.0)
                .absolute(x, (24.0 - knob) / 2.0)
                .motion(rune::tok::motion::SNAPPY)
                .fill(if checked {
                    kit.color("onAccent")
                } else {
                    kit.color("onDark")
                })
                .shadow(1.0, 2.0, rune::Color::BLACK.alpha(0.18)),
        );
    if !active {
        track.layer_opacity(0.5)
    } else {
        track.on_press_tx(on_toggle)
    }
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
                172.0,
                Placement::BottomEnd,
                app,
                kit,
                layers,
                changed,
            )
        };

    let has_update = matches!(&app.update_info, Some(Ok(info)) if info.available);
    let system_label = if has_update {
        "Source & Updates (1)"
    } else {
        "Source & Updates"
    };
    let group_tabs = [
        ("Display & Graphics", "tab-graphics", SettingsTab::Graphics),
        ("Upscaling & Hardware", "tab-upscaling", SettingsTab::Upscaling),
        ("Gameplay & Audio", "tab-gameplay", SettingsTab::Gameplay),
        (system_label, "tab-system", SettingsTab::System),
    ];
    let tab_bar = row()
        .key("settings-group-tabs")
        .w_full()
        .padding(4.0)
        .gap(6.0)
        .fill(kit.c.layer_01)
        .radius(kit.radius.md)
        .children(group_tabs.into_iter().map(|(label, key, tab)| {
            let selected = app.settings_tab == tab;
            let variant = if selected {
                ButtonVariant::Contrast
            } else {
                ButtonVariant::GhostMuted
            };
            piston_button(
                kit,
                key,
                label,
                None,
                variant,
                ControlSize::Regular,
                false,
                Some(Box::new(move |app: &mut App, tx| {
                    app.update(Message::SettingsTab(tab), tx)
                })),
            )
        }));

    let content = match app.settings_tab {
        SettingsTab::Graphics => {
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
            let size = selection(
                "render-resolution",
                "Render Resolution",
                resolution,
                SIZES.iter().map(|c| (c.0.to_owned(), c.1.into())).collect(),
                172.0,
                Placement::BottomEnd,
                app,
                kit,
                overlays,
                Message::Size,
            );
            let shadow = field(
                "shadow-resolution",
                "Shadow Resolution",
                cfg.shadow.1,
                SHADOW_SIZES.iter().map(|c| (*c, c.1.into())).collect(),
                overlays,
                Message::Shadow,
            );
            let anisotropy = field(
                "anisotropy",
                "Anisotropic Filtering",
                cfg.anisotropy.1,
                ANISOTROPIES.iter().map(|c| (*c, c.1.into())).collect(),
                overlays,
                Message::Anisotropy,
            );
            let preset_choices = [
                (Preset::Handheld, "Handheld (720p)"),
                (Preset::Balanced, "Balanced (1080p)"),
                (Preset::HighRefresh, "High Refresh (1440p)"),
                (Preset::Ultra, "Ultra (4K)"),
            ];
            let preset_current = preset_choices
                .iter()
                .find(|(preset, _)| {
                    let mut expected = cfg.clone();
                    expected.apply_preset(*preset);
                    expected == *cfg
                })
                .map(|(_, label)| *label)
                .unwrap_or("Custom");
            let preset_select = selection(
                "quick-preset",
                "Quick Presets",
                preset_current,
                preset_choices
                    .iter()
                    .map(|(preset, label)| (*preset, (*label).to_owned()))
                    .collect(),
                215.0,
                Placement::BottomEnd,
                app,
                kit,
                overlays,
                Message::Preset,
            );
            let presets = row()
                .w_full()
                .py(2.0)
                .justify_between()
                .center_y()
                .gap(10.0)
                .child(medium(kit, "Quick Presets"))
                .child(preset_select);
            let left = col()
                .grow(1.0)
                .min_w(0.0)
                .gap(6.0)
                .child(section_heading(kit, "Display"))
                .child(setting_row(kit, "Display Mode", mode))
                .child(setting_row(kit, "Render Resolution", size))
                .child(setting_row(kit, "Frame Rate Limit", rate))
                .child(setting_row(
                    kit,
                    "Vertical Sync (VSync)",
                    toggle_switch(
                        kit,
                        "vsync",
                        "Vertical Sync (VSync)",
                        cfg.vsync,
                        active,
                        |app, tx| app.update(Message::Vsync(!app.settings.vsync), tx),
                    ),
                ));
            let right = col()
                .grow(1.0)
                .min_w(0.0)
                .gap(6.0)
                .child(section_heading(kit, "Rendering Quality"))
                .child(setting_row(kit, "Shadow Resolution", shadow))
                .child(setting_row(kit, "Anisotropic Filtering", anisotropy))
                .child(setting_row(
                    kit,
                    "Force High LOD",
                    toggle_switch(
                        kit,
                        "high-lod",
                        "Force High LOD",
                        cfg.high_lod,
                        active,
                        |app, tx| app.update(Message::HighLod(!app.settings.high_lod), tx),
                    ),
                ))
                .child(setting_row(
                    kit,
                    "Motion Blur",
                    toggle_switch(
                        kit,
                        "motion-blur",
                        "Motion Blur",
                        cfg.motion_blur,
                        active,
                        |app, tx| app.update(Message::MotionBlur(!app.settings.motion_blur), tx),
                    ),
                ))
                .child(setting_row(
                    kit,
                    "Bloom",
                    toggle_switch(
                        kit,
                        "bloom",
                        "Bloom",
                        cfg.bloom,
                        active,
                        |app, tx| app.update(Message::Bloom(!app.settings.bloom), tx),
                    ),
                ));
            col()
                .key("settings-graphics-content")
                .w_full()
                .gap(16.0)
                .child(presets.key("presets-card"))
                .child(
                    row()
                        .key("graphics-columns")
                        .w_full()
                        .gap(32.0)
                        .child(left.key("graphics-left"))
                        .child(right.key("graphics-right")),
                )
        }
        SettingsTab::Upscaling => {
            let compat = field(
                "compat-mode",
                "GPU Compatibility",
                cfg.compat.1,
                COMPAT_MODES.iter().map(|c| (*c, c.1.into())).collect(),
                overlays,
                Message::Compat,
            );
            let mut gpu_options = vec![("auto".to_owned(), "Auto".to_owned())];
            for (id, name) in &app.hardware.gpus {
                gpu_options.push((id.clone(), format!("#{id}: {name}")));
            }
            let gpu_current = gpu_options
                .iter()
                .find(|(id, _)| id == &cfg.gpu_id)
                .map(|(_, label)| label.clone())
                .unwrap_or_else(|| format!("#{}", cfg.gpu_id));
            let gpu = selection(
                "gpu-adapter",
                "GPU Adapter",
                &gpu_current,
                gpu_options,
                172.0,
                Placement::BottomEnd,
                app,
                kit,
                overlays,
                Message::Gpu,
            );
            let mut monitor_options = vec![(String::new(), "Primary Display".to_owned())];
            for (id, label) in &app.hardware.monitors {
                monitor_options.push((id.clone(), label.clone()));
            }
            let monitor_current = monitor_options
                .iter()
                .find(|(id, _)| id == &cfg.monitor)
                .map(|(_, label)| label.clone())
                .unwrap_or_else(|| format!("Display {}", cfg.monitor));
            let monitor = selection(
                "display-monitor",
                "Monitor",
                &monitor_current,
                monitor_options,
                172.0,
                Placement::BottomEnd,
                app,
                kit,
                overlays,
                Message::Monitor,
            );
            let rcas_strength = selection(
                "rcas-strength",
                "RCAS Strength",
                &format!("{}%", cfg.rcas_strength),
                (0..=100)
                    .step_by(5)
                    .map(|value| (value, format!("{value}%")))
                    .collect(),
                100.0,
                Placement::BottomEnd,
                app,
                kit,
                overlays,
                Message::RcasStrength,
            );
            let left = col()
                .grow(1.0)
                .min_w(0.0)
                .gap(6.0)
                .child(section_heading(kit, "Upscaling & Shaders"))
                .child(setting_row(
                    kit,
                    "FSR 1 Upscaling",
                    toggle_switch(
                        kit,
                        "fsr1",
                        "FSR 1 Upscaling",
                        cfg.fsr1,
                        active,
                        |app, tx| app.update(Message::Fsr1(!app.settings.fsr1), tx),
                    ),
                ))
                .child(setting_row(
                    kit,
                    "RCAS Sharpening",
                    toggle_switch(
                        kit,
                        "rcas",
                        "RCAS Sharpening",
                        cfg.rcas,
                        active,
                        |app, tx| app.update(Message::Rcas(!app.settings.rcas), tx),
                    ),
                ))
                .child(setting_row(kit, "RCAS Strength", rcas_strength))
                .child(setting_row(
                    kit,
                    "Async Shaders",
                    toggle_switch(
                        kit,
                        "async-shaders",
                        "Async Shaders",
                        cfg.async_shaders,
                        active,
                        |app, tx| app.update(Message::AsyncShaders(!app.settings.async_shaders), tx),
                    ),
                ));
            let right = col()
                .grow(1.0)
                .min_w(0.0)
                .gap(6.0)
                .child(section_heading(kit, "Hardware & Compatibility"))
                .child(setting_row(kit, "GPU Adapter", gpu))
                .child(setting_row(kit, "Monitor", monitor))
                .child(setting_row(kit, "GPU Compatibility", compat));
            row()
                .key("settings-upscaling-content")
                .w_full()
                .gap(32.0)
                .child(left.key("upscaling-left"))
                .child(right.key("upscaling-right"))
        }
        SettingsTab::Gameplay => {
            let fov = field(
                "fov-scale",
                "Camera FOV",
                cfg.fov.1,
                FOV_SCALES.iter().map(|c| (*c, c.1.into())).collect(),
                overlays,
                Message::Fov,
            );
            let vibration = selection(
                "vibration",
                "Controller Vibration",
                &format!("{}%", cfg.vibration),
                (0..=100)
                    .step_by(10)
                    .map(|value| (value, format!("{value}%")))
                    .collect(),
                100.0,
                Placement::BottomEnd,
                app,
                kit,
                overlays,
                Message::Vibration,
            );
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
            let left = col()
                .grow(1.0)
                .min_w(0.0)
                .gap(6.0)
                .child(section_heading(kit, "Camera & HUD"))
                .child(setting_row(kit, "Camera FOV", fov))
                .child(setting_row(
                    kit,
                    "Skip Intro Logos",
                    toggle_switch(
                        kit,
                        "skip-intro",
                        "Skip Intro Logos",
                        cfg.skip_intro,
                        active,
                        |app, tx| app.update(Message::SkipIntro(!app.settings.skip_intro), tx),
                    ),
                ))
                .child(setting_row(
                    kit,
                    "Show Frame Rate",
                    toggle_switch(
                        kit,
                        "show-fps",
                        "Show Frame Rate",
                        cfg.fps,
                        active,
                        |app, tx| app.update(Message::Fps(!app.settings.fps), tx),
                    ),
                ))
                .child(setting_row(
                    kit,
                    "Show Keyboard Hints",
                    toggle_switch(
                        kit,
                        "show-keyboard-hints",
                        "Show Keyboard Hints",
                        cfg.keyboard_hints,
                        active,
                        |app, tx| {
                            app.update(Message::KeyboardHints(!app.settings.keyboard_hints), tx)
                        },
                    ),
                ));
            let right = col()
                .grow(1.0)
                .min_w(0.0)
                .gap(6.0)
                .child(section_heading(kit, "Controller & Audio"))
                .child(setting_row(kit, "Controller Vibration", vibration))
                .child(setting_row(kit, "Master Volume", volume));
            row()
                .key("settings-gameplay-content")
                .w_full()
                .gap(32.0)
                .child(left.key("gameplay-left"))
                .child(right.key("gameplay-right"))
        }
        SettingsTab::System => {
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
            let source_card = col()
                .w_full()
                .px(14.0)
                .py(10.0)
                .gap(7.0)
                .fill(kit.c.layer_01)
                .radius(kit.radius.md)
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
                );

            let (update_title, update_detail, download_target) = if app.installing_update {
                (
                    "Downloading & Installing Update…".to_owned(),
                    "PistonPort will close, extract the new version, and restart automatically."
                        .to_owned(),
                    None,
                )
            } else if app.checking_update {
                (
                    "Checking GitHub Releases…".to_owned(),
                    format!("Installed version: v{}", env!("CARGO_PKG_VERSION")),
                    None,
                )
            } else {
                match &app.update_info {
                    Some(Ok(info)) if info.available => (
                        format!("New Release Available: {}", info.tag),
                        format!(
                            "{}  ·  Installed: v{}",
                            info.title,
                            env!("CARGO_PKG_VERSION")
                        ),
                        Some((info.asset_url.clone(), info.url.clone())),
                    ),
                    Some(Ok(info)) => (
                        format!("PistonPort is Up to Date (v{})", env!("CARGO_PKG_VERSION")),
                        format!("Latest GitHub Release: {} ({})", info.tag, info.title),
                        None,
                    ),
                    Some(Err(err)) => (
                        "Update Check Unavailable".to_owned(),
                        err.clone(),
                        None,
                    ),
                    None => (
                        format!("Installed Version: v{}", env!("CARGO_PKG_VERSION")),
                        "Check GitHub Releases for new PistonPort updates.".to_owned(),
                        None,
                    ),
                }
            };
            let mut update_actions = row().gap(8.0).center_y();
            if let Some((asset_url, fallback_url)) = download_target {
                update_actions = update_actions.child(action(
                    kit,
                    "download-update",
                    "Update Now",
                    &icons::UPLOAD,
                    (!app.busy && !app.installing_update)
                        .then_some(Message::InstallUpdate(asset_url, fallback_url)),
                    ButtonVariant::Contrast,
                ));
            }
            update_actions = update_actions.child(action(
                kit,
                "check-update",
                if app.checking_update {
                    "Checking…"
                } else {
                    "Check for Updates"
                },
                &icons::RESET,
                (!app.checking_update && !app.installing_update).then_some(Message::CheckUpdate),
                ButtonVariant::Neutral,
            ));
            let update_card = row()
                .w_full()
                .px(14.0)
                .py(10.0)
                .gap(14.0)
                .justify_between()
                .center_y()
                .fill(kit.c.layer_01)
                .radius(kit.radius.md)
                .child(
                    col()
                        .grow(1.0)
                        .min_w(0.0)
                        .gap(4.0)
                        .child(medium(kit, &update_title))
                        .child(muted(kit, update_detail)),
                )
                .child(update_actions);

            let diag_card = row()
                .w_full()
                .px(14.0)
                .py(10.0)
                .gap(14.0)
                .justify_between()
                .center_y()
                .fill(kit.c.layer_01)
                .radius(kit.radius.md)
                .child(
                    col()
                        .grow(1.0)
                        .min_w(0.0)
                        .gap(4.0)
                        .child(medium(kit, "Diagnostics & Crash Reports"))
                        .child(muted(
                            kit,
                            "Open native-last.log or copy system specs, settings, and crash details to clipboard.",
                        )),
                )
                .child(
                    row()
                        .gap(8.0)
                        .child(action(
                            kit,
                            "open-log",
                            "Open Log",
                            &icons::FOLDER,
                            Some(Message::OpenLog),
                            ButtonVariant::Neutral,
                        ))
                        .child(action(
                            kit,
                            "copy-report",
                            "Copy Crash Report",
                            &icons::PACKAGE,
                            active.then_some(Message::CopyCrashReport),
                            ButtonVariant::Neutral,
                        )),
                );

            let support_card = row()
                .w_full()
                .px(14.0)
                .py(10.0)
                .gap(14.0)
                .justify_between()
                .center_y()
                .fill(kit.c.layer_01)
                .radius(kit.radius.md)
                .child(
                    col()
                        .grow(1.0)
                        .min_w(0.0)
                        .gap(4.0)
                        .child(medium(kit, "Support PistonPort"))
                        .child(muted(
                            kit,
                            "Enjoying Cars 3: Driven to Win on PC? Support ongoing development and updates.",
                        )),
                )
                .child(action(
                    kit,
                    "support-coffee",
                    "Buy Me a Coffee",
                    &icons::COFFEE,
                    Some(Message::OpenCoffee),
                    ButtonVariant::Neutral,
                ));

            col()
                .key("settings-system-content")
                .w_full()
                .gap(9.0)
                .child(source_card.key("source-card"))
                .child(update_card.key("update-card"))
                .child(diag_card.key("diag-card"))
                .child(support_card.key("support-card"))
        }
    };

    let scroll_key = match app.settings_tab {
        SettingsTab::Graphics => "settings-graphics-scroll",
        SettingsTab::Upscaling => "settings-upscaling-scroll",
        SettingsTab::Gameplay => "settings-gameplay-scroll",
        SettingsTab::System => "settings-system-scroll",
    };
    let header_actions = row()
        .gap(8.0)
        .child(action(
            kit,
            "reset-settings",
            "Restore Defaults",
            &icons::RESET,
            active.then_some(Message::ResetSettings),
            ButtonVariant::Neutral,
        ))
        .child(action(
            kit,
            "save-settings",
            "Save Settings",
            &icons::SAVE,
            active.then_some(Message::Save),
            ButtonVariant::Contrast,
        ));
    page_frame(kit, "settings-page", "Settings", header_actions)
        .child(tab_bar)
        .child(
            scroll(kit, scroll_key, content.pb(12.0)).transition(enter_only(
                Transition::offset(0.0, 8.0).combined(Transition::opacity()),
            )),
        )
}

fn scroll(kit: &Kit, key: &str, content: El) -> El {
    let viewport = col()
        .key("scroll")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .scroll(ScrollSpec {
            stick_bottom: false,
        })
        .scroll_fade(ScrollFade::feather(18.0))
        .child(content.pr(14.0));
    astryx_rune::with_scrollbar(kit, key, viewport)
        .w_full()
        .grow(1.0)
        .min_h(0.0)
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
    .pr(24.0)
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
        .child(scroll(kit, "controls-scroll", list))
}

fn save_row(app: &App, kit: &Kit, slot: &Slot, detail: &str) -> El {
    let label = if slot.id == "default" {
        "Use My Progress"
    } else {
        "Use Save"
    };
    let control = if app.selected.id == slot.id {
        row()
            .key(format!("active-save-{}", slot.id))
            .center_y()
            .child(badge(kit, "Active", BadgeColor::Green))
            .transition(enter_only(
                Transition::scale(0.94).combined(Transition::opacity()),
            ))
    } else {
        action(
            kit,
            &format!("use-save-{}", slot.id),
            label,
            &icons::CHECK,
            (!app.busy).then_some(Message::SelectSlot(slot.clone())),
            ButtonVariant::Neutral,
        )
        .transition(enter_only(
            Transition::scale(0.94).combined(Transition::opacity()),
        ))
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
    let export = action(
        kit,
        &format!("export-save-{}", slot.id),
        "Export…",
        &icons::SAVE,
        (!app.busy).then_some(Message::ExportSave(slot.clone())),
        ButtonVariant::GhostMuted,
    );
    row()
        .key(format!("save-row-{}", slot.id))
        .w_full()
        .px(16.0)
        .py(12.0)
        .gap(12.0)
        .center_y()
        .fill(kit.c.layer_01)
        .radius(kit.radius.md)
        .transition(Transition::offset(0.0, 6.0).combined(Transition::opacity()))
        .child(info)
        .child(export)
        .child(control)
}

fn saves(app: &App, kit: &Kit) -> El {
    let mut list = col().w_full().gap(7.0);
    if let Some(slot) = app.slots.iter().find(|slot| slot.id == "default") {
        list = list.child(save_row(app, kit, slot, "Start or continue your own game."));
    }
    list = list.child(col().pt(9.0).child(section_heading(kit, "Built-In Save")));
    for slot in app.slots.iter().filter(|slot| slot.is_builtin()) {
        list = list.child(save_row(
            app,
            kit,
            slot,
            "All Characters Unlocked · 136 / 136 Skill Checks · 22 / 22 Trophies",
        ));
    }
    let mut imported = app
        .slots
        .iter()
        .filter(|slot| slot.id != "default" && !slot.is_builtin())
        .peekable();
    if imported.peek().is_some() {
        list = list.child(col().pt(9.0).child(section_heading(kit, "Imported Saves")));
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
    .child(scroll(kit, "saves-scroll", list))
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
    let mut summary_el = row().w(150.0).justify_end().center_y();
    if !summary.is_empty() {
        summary_el = summary_el.child(
            body(kit, summary)
                .key("trophies-summary")
                .transition(Transition::none()),
        );
    }
    let page = page_frame(kit, "trophies-page", "Trophies", summary_el)
        .child(muted(kit, &app.selected.label));
    if !valid_slot {
        return page.child(body(kit, "This save slot could not be found.").color(kit.c.danger.fg));
    }
    match &app.trophies.catalog {
        Some(Ok(rows)) => {
            let headings = row()
                .gap(12.0)
                .pl(10.0)
                .pr(24.0)
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
            page.child(headings).child(
                scroll(kit, "trophies-scroll", list).transition(enter_only(
                    Transition::offset(0.0, 8.0).combined(Transition::opacity()),
                )),
            )
        }
        Some(Err(error)) => page
            .child(body(kit, "Could Not Load Trophies").color(kit.c.danger.fg))
            .child(muted(kit, error)),
        None => page.child(muted(kit, "Loading Trophies…")),
    }
}
