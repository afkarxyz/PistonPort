//! English text and grades from the supported Cars 3 PS4 trophy set.
use aes::Aes128;
use anyhow::{Context, Result, ensure};
use cbc::cipher::{BlockDecryptMut, KeyIvInit, block_padding::NoPadding};
use roxmltree::Document;
use std::{collections::BTreeMap, slice};

const COMMUNICATION_ID: &str = "NPWR12285_00";
// ESFM uses a different derived key per trophy set. This one is scoped to Cars 3.
const TROPHY_SET_KEY: [u8; 16] = [
    0xcc, 0xd2, 0xf6, 0x80, 0x3c, 0x24, 0x5f, 0xab, 0x25, 0x08, 0xea, 0x77, 0xc4, 0x58, 0x11, 0x65,
];

fn entries(trp: &[u8]) -> Result<BTreeMap<&[u8], &[u8]>> {
    ensure!(
        trp.len() >= 0x60 && trp.starts_with(b"\xdc\xa2\x4d\0"),
        "invalid trophy pack"
    );
    let count = u32::from_be_bytes(trp[16..20].try_into()?) as usize;
    let stride = u32::from_be_bytes(trp[20..24].try_into()?) as usize;
    ensure!(
        count <= 4096 && (0x30..=0x100).contains(&stride),
        "invalid trophy directory"
    );
    let end = 0x60 + count * stride;
    let dir = trp.get(0x60..end).context("incomplete trophy directory")?;
    let mut files = BTreeMap::new();
    for record in dir.chunks_exact(stride) {
        let name = record[..32].split(|b| *b == 0).next().unwrap_or_default();
        if !matches!(name, b"TROPCONF.ESFM" | b"TROP_01.ESFM" | b"TROP.ESFM") {
            continue;
        }
        let offset = usize::try_from(u64::from_be_bytes(record[32..40].try_into()?))?;
        let size = usize::try_from(u64::from_be_bytes(record[40..48].try_into()?))?;
        ensure!(
            offset >= end && size <= 1024 * 1024,
            "invalid trophy metadata entry"
        );
        let limit = offset
            .checked_add(size)
            .context("invalid trophy metadata extent")?;
        let data = trp
            .get(offset..limit)
            .context("incomplete trophy metadata")?;
        ensure!(
            files.insert(name, data).is_none(),
            "duplicate trophy metadata entry"
        );
    }
    Ok(files)
}

fn decrypt(entry: &[u8]) -> Result<String> {
    ensure!(
        entry.len() > 16 && entry.len() % 16 == 0,
        "invalid ESFM size"
    );
    let mut xml = entry[16..].to_vec();
    cbc::Decryptor::<Aes128>::new_from_slices(&TROPHY_SET_KEY, &entry[..16])?
        .decrypt_padded_mut::<NoPadding>(&mut xml)
        .map_err(|_| anyhow::anyhow!("invalid ESFM block"))?;
    let end = xml
        .iter()
        .rposition(|b| *b == b'>')
        .context("invalid trophy XML")?;
    xml.truncate(end + 1);
    Ok(String::from_utf8(xml)?)
}

fn rows<'a>(doc: &'a Document<'a>) -> Result<BTreeMap<u8, roxmltree::Node<'a, 'a>>> {
    let root = doc.root_element();
    ensure!(
        root.has_tag_name("trophyconf")
            && root
                .children()
                .find(|n| n.has_tag_name("npcommid"))
                .and_then(|n| n.text())
                == Some(COMMUNICATION_ID),
        "unexpected trophy set"
    );
    let mut result = BTreeMap::new();
    for row in root.children().filter(|n| n.has_tag_name("trophy")) {
        let id = row
            .attribute("id")
            .context("missing trophy ID")?
            .parse::<u8>()?;
        ensure!(
            id < 128 && result.insert(id, row).is_none(),
            "invalid trophy ID"
        );
    }
    ensure!(!result.is_empty(), "missing trophy records");
    Ok(result)
}

fn text(row: roxmltree::Node<'_, '_>, name: &str) -> String {
    row.children()
        .find(|n| n.has_tag_name(name))
        .and_then(|n| n.text())
        .unwrap_or_default()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn metadata(trp: &[u8]) -> Result<String> {
    use std::fmt::Write;
    let files = entries(trp)?;
    let conf = decrypt(
        files
            .get(b"TROPCONF.ESFM".as_slice())
            .context("missing grade table")?,
    )?;
    let english = decrypt(
        files
            .get(b"TROP_01.ESFM".as_slice())
            .or_else(|| files.get(b"TROP.ESFM".as_slice()))
            .context("missing trophy text")?,
    )?;
    let conf = Document::parse(&conf)?;
    let english = Document::parse(&english)?;
    let tiers = rows(&conf)?;
    let titles = rows(&english)?;
    ensure!(
        tiers.keys().eq(titles.keys()),
        "trophy metadata tables disagree"
    );
    let mut output = String::new();
    for (id, row) in tiers {
        let grade = row.attribute("ttype").context("missing trophy grade")?;
        ensure!(
            matches!(grade, "P" | "G" | "S" | "B"),
            "invalid trophy grade"
        );
        let name = text(titles[&id], "name");
        let detail = text(titles[&id], "detail");
        ensure!(
            !name.is_empty() && !name.contains('|'),
            "invalid trophy name"
        );
        writeln!(&mut output, "@grade.{id}={grade}\n{id}={name} | {detail}")?;
    }
    Ok(output)
}

/// The buffers are owned by the launcher's Python bridge and must not overlap.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pp_trophy_texts(
    trp: *const u8,
    len: usize,
    output: *mut u8,
    capacity: usize,
) -> i32 {
    if trp.is_null() || output.is_null() || len > 32 * 1024 * 1024 {
        return -1;
    }
    let Ok(text) = metadata(unsafe { slice::from_raw_parts(trp, len) }) else {
        return -1;
    };
    if text.len() > capacity {
        return -1;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(text.as_ptr(), output, text.len());
    }
    text.len() as i32
}
