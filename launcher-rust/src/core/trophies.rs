//! Trophies: the icons in the game's trophy package, the unlocked flags and the tier of an icon.

use std::fs;
use std::io;
use std::path::Path;

use super::i18n::tr;
pub use super::trophy_data::{read_trp_icons, read_unlocked_trophies, tier_color, Tier};
#[cfg(test)]
use std::collections::{BTreeMap, BTreeSet};

/// How many trophies the game has.
pub const TROPHY_COUNT: usize = 35;

/// The tier by the colour of the icon; `None` when the image cannot be decoded or is all dark.
///
/// The icon is cut down to 24 x 24 with a Catmull-Rom filter, the same cubic family as the
/// bicubic filter of the Python launcher, so the average colour can differ by a rounding step.
pub fn trophy_tier(png: &[u8]) -> Option<Tier> {
    let rgb = image::load_from_memory(png).ok()?.to_rgb8();
    let small = image::imageops::resize(&rgb, 24, 24, image::imageops::FilterType::CatmullRom);
    let (mut r, mut g, mut b, mut n) = (0.0f64, 0.0f64, 0.0f64, 0u32);
    for pixel in small.pixels().filter(|p| p.0.iter().any(|&c| c > 60)) {
        r += f64::from(pixel.0[0]);
        g += f64::from(pixel.0[1]);
        b += f64::from(pixel.0[2]);
        n += 1;
    }
    if n == 0 {
        return None;
    }
    let n = f64::from(n);
    let (r, g, b) = (r / n, g / n, b / n);
    let top = r.max(g).max(b);
    let spread = (top - r.min(g).min(b)) / top;
    Some(if b >= r {
        Tier::Platinum
    } else if spread < 0.4 {
        Tier::Silver
    } else if spread < 0.72 {
        Tier::Bronze
    } else {
        Tier::Gold
    })
}

/// Locks every trophy again: the file is moved to `<name>.bak`; `fs::rename` replaces an older copy
/// in one step on every platform.
/// Returns how many trophies were unlocked (0: nothing is changed). The game must be closed:
/// it keeps the trophies in memory while it runs.
pub fn reset_trophies(path: &Path) -> io::Result<usize> {
    let unlocked = read_unlocked_trophies(path).len();
    if unlocked > 0 {
        let mut name = path.file_name().unwrap_or_default().to_os_string();
        name.push(".bak");
        let backup = path.with_file_name(name);
        fs::rename(path, &backup)?;
    }
    Ok(unlocked)
}

/// The name of a tier in `lang`.
pub fn tier_label(tier: Tier, lang: &str) -> String {
    let en = match tier {
        Tier::Platinum => "Platinum",
        Tier::Gold => "Gold",
        Tier::Silver => "Silver",
        Tier::Bronze => "Bronze",
    };
    tr(lang, en)
}

/// How many players earned each trophy, in percent, by id (0 platinum, 1-18 bronze, 19-29 silver,
/// 30-34 gold). The port has no PSN connection, so these are fixed numbers: the share of the players
/// tracked by Exophase (exophase.com) who earned each trophy, as listed in October 2026. The same
/// table as the game's window (`gpu/shim/spartanport_trophy_rarity.h`).
const EARNED: [f32; TROPHY_COUNT] = [
    17.58, // 0
    96.17, 88.31, 77.38, 73.00, 68.89, 63.78, 74.60, 65.73, 61.36, 59.96, // 1-10
    20.99, 74.21, 22.06, 22.69, 25.90, 24.58, 75.05, 61.74, // 11-18
    81.22, 67.16, 65.39, 58.79, 76.16, 62.61, 63.58, 38.23, 44.92, 36.38, // 19-28
    32.85, // 29
    54.48, 20.21, 22.39, 20.98, 20.05, // 30-34
];

/// The share of players who earned trophy `id` (percent); `None` when the package does not hold
/// this game's list of 35 (a different game or region has others).
pub fn earned(id: u32, trophy_count: usize) -> Option<f32> {
    if trophy_count != TROPHY_COUNT {
        return None;
    }
    EARNED.get(id as usize).copied()
}

/// PlayStation's rarity tiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rarity {
    Common,
    Rare,
    VeryRare,
    UltraRare,
}

impl Rarity {
    /// Common over 50%, rare over 15%, very rare over 5%, ultra rare below.
    pub fn of(percent: f32) -> Rarity {
        if percent > 50.0 {
            Rarity::Common
        } else if percent > 15.0 {
            Rarity::Rare
        } else if percent > 5.0 {
            Rarity::VeryRare
        } else {
            Rarity::UltraRare
        }
    }

    pub fn english(self) -> &'static str {
        match self {
            Rarity::Common => "Common",
            Rarity::Rare => "Rare",
            Rarity::VeryRare => "Very rare",
            Rarity::UltraRare => "Ultra rare",
        }
    }

    /// The colour the tier is drawn in, as in the game's window.
    pub fn color(self) -> &'static str {
        match self {
            Rarity::Common => "#b8bfcc",
            Rarity::Rare => "#f2c761",
            Rarity::VeryRare => "#d194f5",
            Rarity::UltraRare => "#6bccff",
        }
    }
}

/// The orders of the list; most earned first, as in the game's window.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Order {
    #[default]
    MostEarned,
    Rarest,
    List,
}

impl Order {
    pub const ALL: [Order; 3] = [Order::MostEarned, Order::Rarest, Order::List];

    pub fn english(self) -> &'static str {
        match self {
            Order::MostEarned => "Most earned first",
            Order::Rarest => "Rarest first",
            Order::List => "List order",
        }
    }
}

/// `ids` in `order`: by the share of players, the ones not known last; equal ones keep their order.
pub fn sorted(ids: &[u32], order: Order) -> Vec<u32> {
    let mut ids = ids.to_vec();
    let count = ids.len();
    let key = |id: &u32| earned(*id, count);
    match order {
        Order::List => {}
        Order::MostEarned => ids.sort_by(|a, b| match (key(a), key(b)) {
            (Some(a), Some(b)) => b.total_cmp(&a),
            (a, b) => b.is_some().cmp(&a.is_some()),
        }),
        Order::Rarest => ids.sort_by(|a, b| match (key(a), key(b)) {
            (Some(a), Some(b)) => a.total_cmp(&b),
            (a, b) => b.is_some().cmp(&a.is_some()),
        }),
    }
    ids
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::path::PathBuf;

    fn trp_with(entries: &[(&str, &[u8])], magic: [u8; 4], element: usize) -> Vec<u8> {
        let header_size = 0x60;
        let offset = header_size + element * entries.len();
        let mut table = Vec::new();
        let mut body = Vec::new();
        for (name, data) in entries {
            let mut entry = vec![0u8; element.max(0x30)];
            entry[..name.len()].copy_from_slice(name.as_bytes());
            entry[0x20..0x28].copy_from_slice(&((offset + body.len()) as u64).to_be_bytes());
            entry[0x28..0x30].copy_from_slice(&(data.len() as u64).to_be_bytes());
            entry.truncate(element);
            table.extend(entry);
            body.extend_from_slice(data);
        }
        let mut out = magic.to_vec();
        out.extend(3u32.to_be_bytes());
        out.extend(((header_size + table.len() + body.len()) as u64).to_be_bytes());
        out.extend((entries.len() as u32).to_be_bytes());
        out.extend((element as u32).to_be_bytes());
        out.resize(header_size, 0);
        out.extend(table);
        out.extend(body);
        out
    }

    fn trp(entries: &[(&str, &[u8])]) -> Vec<u8> {
        trp_with(entries, [0xdc, 0xa2, 0x4d, 0x00], 0x40)
    }

    fn write(dir: &tempfile::TempDir, name: &str, data: &[u8]) -> PathBuf {
        let path = dir.path().join(name);
        fs::write(&path, data).unwrap();
        path
    }

    fn solid_png(rgb: [u8; 3]) -> Vec<u8> {
        let img = image::RgbImage::from_pixel(32, 32, image::Rgb(rgb));
        let mut out = Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    }

    #[test]
    fn icons_are_read_by_id() {
        let dir = tempfile::tempdir().unwrap();
        let path = write(
            &dir,
            "trophy00.trp",
            &trp(&[
                ("TROPCONF.ESFM", b"encrypted"),
                ("TROP000.PNG", b"\x89PNG-fake0"),
                ("TROP034.PNG", b"\x89PNG-fake34"),
                ("TROP_00.ESFM", b"x"),
                ("ICON0.PNG", b"icon"),
                ("TROP0001.PNG", b"y"),
                ("TROP00A.PNG", b"z"),
            ]),
        );
        let icons = read_trp_icons(&path);
        assert_eq!(icons.len(), 2);
        assert_eq!(icons[&0], b"\x89PNG-fake0");
        assert_eq!(icons[&34], b"\x89PNG-fake34");
    }

    #[test]
    fn bad_packages_give_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let good = trp(&[("TROP001.PNG", b"abc")]);
        let none = BTreeMap::new();
        assert_eq!(read_trp_icons(&dir.path().join("missing.trp")), none);
        assert_eq!(read_trp_icons(&write(&dir, "a.trp", b"")), none);
        let mut wrong = good.clone();
        wrong[..4].copy_from_slice(&[0, 0, 0, 0]);
        assert_eq!(read_trp_icons(&write(&dir, "b.trp", &wrong)), none);
        let small = trp_with(&[("TROP001.PNG", b"abc")], [0xdc, 0xa2, 0x4d, 0x00], 8);
        assert_eq!(read_trp_icons(&write(&dir, "c.trp", &small)), none);
        let big = trp_with(&[("TROP001.PNG", b"abc")], [0xdc, 0xa2, 0x4d, 0x00], 0x101);
        assert_eq!(read_trp_icons(&write(&dir, "e.trp", &big)), none);
        assert_eq!(
            read_trp_icons(&write(&dir, "d.trp", &good[..good.len() - 2])),
            none
        );
    }

    #[test]
    fn a_huge_count_never_reads_past_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let mut data = trp(&[("TROP001.PNG", b"abc"), ("TROP002.PNG", b"def")]);
        data[0x10..0x14].copy_from_slice(&100_000u32.to_be_bytes());
        data.resize(1024, 0);
        let icons = read_trp_icons(&write(&dir, "huge.trp", &data));
        assert_eq!(icons.keys().copied().collect::<Vec<_>>(), vec![1, 2]);

        // An entry whose offset + size overflows u64 is dropped, the valid one stays.
        let mut data = trp(&[("TROP001.PNG", b"abc"), ("TROP002.PNG", b"def")]);
        let second = 0x60 + 0x40;
        data[second + 0x20..second + 0x28].copy_from_slice(&u64::MAX.to_be_bytes());
        data[second + 0x28..second + 0x30].copy_from_slice(&16u64.to_be_bytes());
        let icons = read_trp_icons(&write(&dir, "overflow.trp", &data));
        assert_eq!(icons.keys().copied().collect::<Vec<_>>(), vec![1]);
    }

    #[test]
    fn unlocked_bits() {
        let dir = tempfile::tempdir().unwrap();
        let mut flags = [0u32; 4];
        for trophy in [0usize, 5, 31, 32, 34, 127] {
            flags[trophy / 32] |= 1 << (trophy % 32);
        }
        let raw: Vec<u8> = flags.iter().flat_map(|w| w.to_le_bytes()).collect();
        let path = write(&dir, "trophies.dat", &raw);
        assert_eq!(
            read_unlocked_trophies(&path),
            BTreeSet::from([0, 5, 31, 32, 34, 127])
        );
        assert!(read_unlocked_trophies(&dir.path().join("none.dat")).is_empty());
        assert_eq!(
            read_unlocked_trophies(&write(&dir, "short.dat", b"\x02")),
            BTreeSet::from([1])
        );
    }

    #[test]
    fn tier_follows_the_colour_of_the_icon() {
        let cases = [
            (Tier::Platinum, [139, 167, 167]),
            (Tier::Bronze, [137, 89, 52]),
            (Tier::Silver, [136, 119, 104]),
            (Tier::Gold, [141, 104, 27]),
        ];
        for (tier, rgb) in cases {
            assert_eq!(trophy_tier(&solid_png(rgb)), Some(tier));
        }
        assert_eq!(trophy_tier(b"not a png"), None);
        assert_eq!(trophy_tier(&solid_png([0, 0, 0])), None);
    }

    #[test]
    fn reset_moves_the_file_and_counts() {
        let dir = tempfile::tempdir().unwrap();
        let path = write(&dir, "trophies.dat", &[0b101, 0, 0, 0]);
        write(&dir, "trophies.dat.bak", b"old");
        assert_eq!(reset_trophies(&path).unwrap(), 2);
        assert!(!path.exists());
        let bak = dir.path().join("trophies.dat.bak");
        assert_eq!(fs::read(&bak).unwrap(), vec![0b101, 0, 0, 0]);

        let empty = write(&dir, "empty.dat", &[0, 0, 0, 0]);
        assert_eq!(reset_trophies(&empty).unwrap(), 0);
        assert!(empty.exists());
    }

    #[test]
    fn rarity_is_only_known_for_this_games_list() {
        assert_eq!(earned(0, TROPHY_COUNT), Some(17.58));
        assert_eq!(earned(1, TROPHY_COUNT), Some(96.17));
        assert_eq!(earned(35, TROPHY_COUNT), None);
        assert_eq!(earned(0, 50), None);
        assert_eq!(Rarity::of(96.17), Rarity::Common);
        assert_eq!(Rarity::of(17.58), Rarity::Rare);
        assert_eq!(Rarity::of(5.5), Rarity::VeryRare);
        assert_eq!(Rarity::of(5.0), Rarity::UltraRare);
    }

    #[test]
    fn orders_sort_by_the_share_of_players() {
        let ids: Vec<u32> = (0..TROPHY_COUNT as u32).collect();
        assert_eq!(sorted(&ids, Order::List), ids);
        let most = sorted(&ids, Order::MostEarned);
        assert_eq!((most[0], most[1], most[34]), (1, 2, 0));
        let rarest = sorted(&ids, Order::Rarest);
        assert_eq!((rarest[0], rarest[1], rarest[34]), (0, 34, 1));
        // Another list: no share known, so every order keeps the list's.
        assert_eq!(sorted(&[3, 1, 2], Order::Rarest), [3, 1, 2]);
    }

    #[test]
    fn tier_labels_and_colors_exist() {
        assert_eq!(tier_label(Tier::Gold, "en"), "Gold");
        assert_eq!(tier_color(Tier::Platinum), "#8fd3e8");
    }
}
