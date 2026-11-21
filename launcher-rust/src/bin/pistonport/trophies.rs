use std::collections::{BTreeMap, BTreeSet};

use crate::trophy_data::{Tier, parse_trp_icons};
use rune::{Image, ImageCache};

#[derive(Debug, Clone)]
pub struct Trophy {
    pub id: u32,
    pub name: String,
    pub detail: String,
    pub tier: Tier,
    pub icon: Option<Image>,
}

#[derive(Default)]
pub struct TrophyPage {
    pub source: String,
    pub catalog: Option<Result<Vec<Trophy>, String>>,
    pub unlocked: BTreeSet<u32>,
}

pub fn parse_catalog(frame: &[u8], images: &ImageCache) -> Result<Vec<Trophy>, String> {
    let invalid = || "Could not read trophy metadata.".to_owned();
    let header = frame.get(..4).ok_or_else(invalid)?;
    let length = u32::from_le_bytes(header.try_into().map_err(|_| invalid())?) as usize;
    if length == 0 || length > 32 * 1024 {
        return Err(invalid());
    }
    let end = 4 + length;
    let metadata =
        std::str::from_utf8(frame.get(4..end).ok_or_else(invalid)?).map_err(|_| invalid())?;
    let mut icons = parse_trp_icons(frame.get(end..).ok_or_else(invalid)?);
    let fields: BTreeMap<_, _> = metadata
        .lines()
        .map(|line| line.split_once('=').ok_or_else(invalid))
        .collect::<Result<_, _>>()?;
    let mut rows = Vec::new();
    for (key, value) in &fields {
        let Some(id) = key.strip_prefix("@grade.") else {
            continue;
        };
        let tier = match *value {
            "P" => Tier::Platinum,
            "G" => Tier::Gold,
            "S" => Tier::Silver,
            "B" => Tier::Bronze,
            _ => return Err(invalid()),
        };
        let (name, detail) = fields
            .get(id)
            .and_then(|row| row.split_once(" | "))
            .ok_or_else(invalid)?;
        let id = id.parse::<u32>().map_err(|_| invalid())?;
        if name.is_empty() || id >= 128 {
            return Err(invalid());
        }
        rows.push(Trophy {
            id,
            name: name.into(),
            detail: detail.into(),
            tier,
            icon: icons.remove(&id).map(|png| images.load(png)),
        });
    }
    if rows.is_empty() {
        return Err(invalid());
    }
    rows.sort_unstable_by_key(|row| row.id);
    Ok(rows)
}
