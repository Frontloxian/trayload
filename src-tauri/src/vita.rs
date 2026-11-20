//! Reads the Title ID of a PS Vita game from its .vpk (a zip with sce_sys/param.sfo inside).
//! Vita3K starts installed games by Title ID, not by file.

use std::{fs, io::Read, path::Path};

/// The TITLE_ID entry of a PARAM.SFO file, for example "PCSE00000".
pub fn title_id_from_sfo(b: &[u8]) -> Option<String> {
    let u32at = |o: usize| b.get(o..o + 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]));
    let u16at = |o: usize| b.get(o..o + 2).map(|s| u16::from_le_bytes([s[0], s[1]]));
    if b.get(0..4)? != b"\0PSF" {
        return None;
    }
    let (key_table, data_table, count) = (u32at(8)? as usize, u32at(12)? as usize, u32at(16)? as usize);
    for i in 0..count.min(64) {
        let e = 20 + i * 16;
        let key_off = u16at(e)? as usize;
        let (len, data_off) = (u32at(e + 4)? as usize, u32at(e + 12)? as usize);
        let key_start = key_table + key_off;
        let key_end = key_start + b.get(key_start..)?.iter().position(|&c| c == 0)?;
        if &b[key_start..key_end] == b"TITLE_ID" {
            let raw = b.get(data_table + data_off..data_table + data_off + len)?;
            let end = raw.iter().position(|&c| c == 0).unwrap_or(raw.len());
            let id = String::from_utf8_lossy(&raw[..end]).to_string();
            return (!id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric())).then_some(id);
        }
    }
    None
}

pub fn title_id(vpk: &Path) -> Result<String, String> {
    let f = fs::File::open(vpk).map_err(|e| format!("cannot open the game file: {e}"))?;
    let mut z = zip::ZipArchive::new(f).map_err(|_| "this .vpk can't be read as a zip".to_string())?;
    let name = (0..z.len())
        .filter_map(|i| z.by_index(i).ok().map(|e| e.name().to_string()))
        .find(|n| n.eq_ignore_ascii_case("sce_sys/param.sfo"))
        .ok_or("no PARAM.SFO in this .vpk")?;
    let mut data = vec![];
    z.by_name(&name)
        .map_err(|e| e.to_string())?
        .take(1_000_000)
        .read_to_end(&mut data)
        .map_err(|e| e.to_string())?;
    title_id_from_sfo(&data).ok_or_else(|| "could not read the Title ID from this game".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sfo(key: &str, val: &str) -> Vec<u8> {
        let mut keys = key.as_bytes().to_vec();
        keys.push(0);
        let mut data = val.as_bytes().to_vec();
        data.push(0);
        let (kt, dt) = (20 + 16, 20 + 16 + keys.len());
        let mut v = b"\0PSF".to_vec();
        v.extend([1, 1, 0, 0]);
        v.extend((kt as u32).to_le_bytes());
        v.extend((dt as u32).to_le_bytes());
        v.extend(1u32.to_le_bytes());
        v.extend(0u16.to_le_bytes());
        v.extend(0x0204u16.to_le_bytes());
        v.extend((data.len() as u32).to_le_bytes());
        v.extend((data.len() as u32).to_le_bytes());
        v.extend(0u32.to_le_bytes());
        v.extend(keys);
        v.extend(data);
        v
    }

    #[test]
    fn reads_title_id() {
        assert_eq!(title_id_from_sfo(&sfo("TITLE_ID", "PCSE00123")).as_deref(), Some("PCSE00123"));
        assert_eq!(title_id_from_sfo(&sfo("TITLE", "Game")), None);
        assert_eq!(title_id_from_sfo(b"nope"), None);
    }
}
