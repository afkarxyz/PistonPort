//! Trophy archive icons, saved unlock flags and official tiers.
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

pub(super) const MAGIC: [u8; 4] = [0xdc, 0xa2, 0x4d, 0x00];
pub(super) const HEADER_SIZE: usize = 0x60;
pub(super) const ENTRY_SIZE: usize = 0x30;
const MAX_ENTRIES: u32 = 4096;

fn be_u32(data: &[u8], at: usize) -> Option<u32> {
    let bytes = data.get(at..at.checked_add(4)?)?;
    Some(u32::from_be_bytes(bytes.try_into().ok()?))
}

fn be_u64(data: &[u8], at: usize) -> Option<u64> {
    let bytes = data.get(at..at.checked_add(8)?)?;
    Some(u64::from_be_bytes(bytes.try_into().ok()?))
}

/// The id of an entry named `TROP<3 digits>.PNG`.
fn icon_id(name: &[u8]) -> Option<u32> {
    let digits = name.strip_prefix(b"TROP")?.strip_suffix(b".PNG")?;
    if digits.len() != 3 || !digits.iter().all(u8::is_ascii_digit) {
        return None;
    }
    Some(digits.iter().fold(0, |n, d| n * 10 + u32::from(d - b'0')))
}

/// {trophy id: PNG bytes} from the game's `sce_sys/trophy/trophy00.trp` (empty when missing or odd).
///
/// The header is big endian: magic DC A2 4D 00, file count at 0x10, entry size at 0x14; the
/// entries start at 0x60 (32-byte name, offset, size). The icons are TROP000.PNG, TROP001.PNG...
/// The texts (TROP*.ESFM) are encrypted and not read here.
pub fn read_trp_icons(path: &Path) -> BTreeMap<u32, Vec<u8>> {
    let Ok(data) = fs::read(path) else {
        return BTreeMap::new();
    };
    parse_trp_icons(&data)
}

pub fn parse_trp_icons(data: &[u8]) -> BTreeMap<u32, Vec<u8>> {
    let mut icons = BTreeMap::new();
    if data.len() < HEADER_SIZE || data[..4] != MAGIC {
        return icons;
    }
    let (Some(count), Some(element)) = (be_u32(data, 0x10), be_u32(data, 0x14)) else {
        return icons;
    };
    if !(0x30..=0x100).contains(&element) {
        return icons;
    }
    let element = element as usize;
    for index in 0..count.min(MAX_ENTRIES) as usize {
        let start = HEADER_SIZE + index * element;
        let Some(entry) = data.get(start..start + ENTRY_SIZE) else {
            break;
        };
        let name = entry[..0x20].split(|&b| b == 0).next().unwrap_or_default();
        let Some(id) = icon_id(name) else { continue };
        let (Some(offset), Some(size)) = (be_u64(entry, 0x20), be_u64(entry, 0x28)) else {
            continue;
        };
        let Some(end) = offset.checked_add(size) else {
            continue;
        };
        if size > 0 && end <= data.len() as u64 {
            if let Some(png) = data.get(offset as usize..end as usize) {
                icons.insert(id, png.to_vec());
            }
        }
    }
    icons
}

/// The trophy ids the port recorded: `user/trophies.dat` holds four little-endian 32-bit flag words.
pub fn read_unlocked_trophies(path: &Path) -> BTreeSet<u32> {
    let Ok(data) = fs::read(path) else {
        return BTreeSet::new();
    };
    let mut raw = [0u8; 16];
    let head = &data[..data.len().min(16)];
    raw[..head.len()].copy_from_slice(head);
    (0..128u32)
        .filter(|&i| {
            let at = (i / 32) as usize * 4;
            let word = u32::from_le_bytes([raw[at], raw[at + 1], raw[at + 2], raw[at + 3]]);
            word >> (i % 32) & 1 == 1
        })
        .collect()
}

/// The grade of a trophy, told by the colour of its icon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tier {
    Platinum,
    Gold,
    Silver,
    Bronze,
}

/// The colour a tier is drawn in.
pub fn tier_color(tier: Tier) -> &'static str {
    match tier {
        Tier::Platinum => "#8fd3e8",
        Tier::Gold => "#e0b43a",
        Tier::Silver => "#c4c4c8",
        Tier::Bronze => "#cd7f32",
    }
}
