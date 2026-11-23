use rune::Icon;

macro_rules! lucide {
    ($id:ident, $name:literal) => {
        pub static $id: Icon = Icon {
            name: $name,
            view: 24.0,
            body: include_str!(concat!(
                "../../../../pistonport/launcher/lucide/",
                $name,
                ".svg"
            )),
        };
    };
}

lucide!(FLAG, "flag");
lucide!(SETTINGS, "sliders-horizontal");
lucide!(GAMEPAD, "gamepad-2");
lucide!(TROPHY, "trophy");
lucide!(MINUS, "minus");
lucide!(CLOSE, "x");
lucide!(PLAY, "play");
lucide!(SAVE, "save");
lucide!(PACKAGE, "package");
lucide!(FOLDER, "folder-open");
lucide!(UPLOAD, "upload");
lucide!(RESET, "rotate-ccw");
lucide!(CHECK, "check");
lucide!(LOCK, "lock");
lucide!(COFFEE, "coffee");

pub static STOP: Icon = Icon {
    name: "square",
    view: 24.0,
    body: r#"<rect width="14" height="14" x="5" y="5" rx="2"/>"#,
};
