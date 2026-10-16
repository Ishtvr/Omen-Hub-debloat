//! Minimal Win32 `.res` writer for the build script: icon group and version
//! information only. MSVC `link.exe` and `lld-link` accept the file directly,
//! so no resource compiler or extra build dependency is required.

const RT_ICON: u16 = 3;
const RT_GROUP_ICON: u16 = 14;
const RT_VERSION: u16 = 16;
const LANG_NEUTRAL: u16 = 0;

/// Resource ID loaded at runtime with `LoadImageW(MAKEINTRESOURCE(1))`.
pub const ICON_GROUP_ID: u16 = 1;

pub struct VersionInfo<'a> {
    pub version: [u16; 4],
    pub strings: &'a [(&'a str, &'a str)],
}

/// Splits an `.ico` file into RT_ICON images plus the RT_GROUP_ICON directory.
pub fn icon_resources(ico: &[u8]) -> Result<Vec<(u16, u16, Vec<u8>)>, String> {
    let word = |at: usize| -> Result<u16, String> {
        ico.get(at..at + 2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
            .ok_or_else(|| "Truncated icon directory".to_string())
    };
    let dword = |at: usize| -> Result<u32, String> {
        ico.get(at..at + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .ok_or_else(|| "Truncated icon directory".to_string())
    };
    if word(0)? != 0 || word(2)? != 1 {
        return Err("Not an icon file".into());
    }
    let count = word(4)? as usize;
    if count == 0 || count > 32 {
        return Err("Unexpected icon image count".into());
    }
    let mut group = Vec::with_capacity(6 + 14 * count);
    group.extend_from_slice(&[0, 0, 1, 0]);
    group.extend_from_slice(&(count as u16).to_le_bytes());
    let mut resources = Vec::with_capacity(count + 1);
    for index in 0..count {
        let entry = 6 + 16 * index;
        let size = dword(entry + 8)? as usize;
        let offset = dword(entry + 12)? as usize;
        let image = ico
            .get(offset..offset.checked_add(size).ok_or("Icon image overflow")?)
            .ok_or("Icon image outside file")?;
        let id = index as u16 + 1;
        // Width, height, colors, reserved, planes, bit count and byte count
        // stay identical; the file offset becomes the RT_ICON identifier.
        group.extend_from_slice(&ico[entry..entry + 12]);
        group.extend_from_slice(&id.to_le_bytes());
        resources.push((RT_ICON, id, image.to_vec()));
    }
    resources.push((RT_GROUP_ICON, ICON_GROUP_ID, group));
    Ok(resources)
}

fn utf16z(text: &str) -> Vec<u8> {
    text.encode_utf16()
        .chain(Some(0))
        .flat_map(u16::to_le_bytes)
        .collect()
}

fn align(buffer: &mut Vec<u8>) {
    while buffer.len() % 4 != 0 {
        buffer.push(0);
    }
}

/// One VS_VERSIONINFO tree node: header, key, value and aligned children.
fn node(key: &str, value: &[u8], value_length: u16, text: bool, children: &[Vec<u8>]) -> Vec<u8> {
    let mut buffer = vec![0, 0];
    buffer.extend_from_slice(&value_length.to_le_bytes());
    buffer.extend_from_slice(&(text as u16).to_le_bytes());
    buffer.extend_from_slice(&utf16z(key));
    align(&mut buffer);
    buffer.extend_from_slice(value);
    for child in children {
        align(&mut buffer);
        buffer.extend_from_slice(child);
    }
    let length = buffer.len() as u16;
    buffer[..2].copy_from_slice(&length.to_le_bytes());
    buffer
}

pub fn version_resource(info: &VersionInfo<'_>) -> (u16, u16, Vec<u8>) {
    let [major, minor, patch, build] = info.version.map(u32::from);
    let mut fixed = Vec::with_capacity(52);
    for value in [
        0xFEEF_04BD_u32,
        0x0001_0000,
        (major << 16) | minor,
        (patch << 16) | build,
        (major << 16) | minor,
        (patch << 16) | build,
        0x3F,
        0,
        0x0004_0004, // VOS_NT_WINDOWS32
        1,           // VFT_APP
        0,
        0,
        0,
    ] {
        fixed.extend_from_slice(&value.to_le_bytes());
    }
    let strings: Vec<Vec<u8>> = info
        .strings
        .iter()
        .map(|(key, value)| {
            let characters = value.encode_utf16().count() as u16 + 1;
            node(key, &utf16z(value), characters, true, &[])
        })
        .collect();
    // German (0x0407), Unicode code page 1200.
    let table = node("040704B0", &[], 0, true, &strings);
    let string_info = node("StringFileInfo", &[], 0, true, &[table]);
    let translation = node("Translation", &[0x07, 0x04, 0xB0, 0x04], 4, false, &[]);
    let var_info = node("VarFileInfo", &[], 0, true, &[translation]);
    let root = node(
        "VS_VERSION_INFO",
        &fixed,
        fixed.len() as u16,
        false,
        &[string_info, var_info],
    );
    (RT_VERSION, 1, root)
}

fn entry(output: &mut Vec<u8>, kind: u16, id: u16, flags: u16, data: &[u8]) {
    output.extend_from_slice(&(data.len() as u32).to_le_bytes());
    output.extend_from_slice(&32u32.to_le_bytes());
    for ordinal in [kind, id] {
        output.extend_from_slice(&0xFFFFu16.to_le_bytes());
        output.extend_from_slice(&ordinal.to_le_bytes());
    }
    output.extend_from_slice(&0u32.to_le_bytes());
    output.extend_from_slice(&flags.to_le_bytes());
    output.extend_from_slice(&LANG_NEUTRAL.to_le_bytes());
    output.extend_from_slice(&0u32.to_le_bytes());
    output.extend_from_slice(&0u32.to_le_bytes());
    output.extend_from_slice(data);
    align(output);
}

/// Complete `.res` image beginning with the mandatory empty entry.
pub fn res_file(resources: &[(u16, u16, Vec<u8>)]) -> Vec<u8> {
    let mut output = Vec::new();
    entry(&mut output, 0, 0, 0, &[]);
    for (kind, id, data) in resources {
        // Same legacy memory flags as rc.exe; Win32 ignores them.
        let flags = match *kind {
            RT_ICON => 0x1010,
            RT_GROUP_ICON => 0x1030,
            _ => 0x0030,
        };
        entry(&mut output, *kind, *id, flags, data);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ico(sizes: &[(u8, &[u8])]) -> Vec<u8> {
        let mut file = vec![0, 0, 1, 0];
        file.extend_from_slice(&(sizes.len() as u16).to_le_bytes());
        let mut offset = 6 + 16 * sizes.len() as u32;
        for (side, data) in sizes {
            file.extend_from_slice(&[*side, *side, 0, 0, 1, 0, 32, 0]);
            file.extend_from_slice(&(data.len() as u32).to_le_bytes());
            file.extend_from_slice(&offset.to_le_bytes());
            offset += data.len() as u32;
        }
        for (_, data) in sizes {
            file.extend_from_slice(data);
        }
        file
    }

    #[test]
    fn icon_group_references_each_image_by_ordinal() {
        let resources = icon_resources(&ico(&[(16, b"abc"), (0, b"defgh")])).unwrap();
        assert_eq!(resources[0], (RT_ICON, 1, b"abc".to_vec()));
        assert_eq!(resources[1], (RT_ICON, 2, b"defgh".to_vec()));
        let (kind, id, group) = &resources[2];
        assert_eq!((*kind, *id), (RT_GROUP_ICON, ICON_GROUP_ID));
        assert_eq!(group.len(), 6 + 2 * 14);
        assert_eq!(&group[6..8], &[16, 16]);
        assert_eq!(&group[14..18], &3u32.to_le_bytes());
        assert_eq!(&group[18..20], &1u16.to_le_bytes());
        assert_eq!(&group[28..32], &5u32.to_le_bytes());
        assert_eq!(&group[32..34], &2u16.to_le_bytes());
        assert!(icon_resources(b"\0\0\x02\0\x01\0").is_err());
        assert!(icon_resources(&ico(&[(16, b"abc")])[..20]).is_err());
    }

    #[test]
    fn version_tree_is_aligned_and_sized() {
        let (kind, _, data) = version_resource(&VersionInfo {
            version: [0, 1, 0, 0],
            strings: &[("ProductName", "Omen Hub++")],
        });
        assert_eq!(kind, RT_VERSION);
        assert_eq!(u16::from_le_bytes([data[0], data[1]]) as usize, data.len());
        assert_eq!(u16::from_le_bytes([data[2], data[3]]), 52);
        // Header (6) + "VS_VERSION_INFO\0" (32) + padding (2) = fixed info at 40.
        assert_eq!(&data[40..44], &0xFEEF_04BDu32.to_le_bytes());
        assert_eq!(&data[48..52], &1u32.to_le_bytes());
        let text: Vec<u16> = data
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        let needle: Vec<u16> = "Omen Hub++".encode_utf16().collect();
        assert!(text.windows(needle.len()).any(|w| w == needle));
    }

    #[test]
    fn committed_icon_covers_shell_and_tray_sizes() {
        let resources = icon_resources(include_bytes!("../assets/omen-hub-pp.ico")).unwrap();
        let (_, _, group) = resources.last().unwrap();
        let sides: Vec<u8> = group[6..].chunks_exact(14).map(|e| e[0]).collect();
        // 0 encodes 256 pixels in icon directories.
        for side in [16, 20, 24, 32, 40, 48, 64, 0] {
            assert!(sides.contains(&side), "missing {side}");
        }
        for (kind, _, image) in &resources[..resources.len() - 1] {
            assert_eq!(*kind, RT_ICON);
            assert_eq!(&image[..8], b"\x89PNG\r\n\x1a\n");
        }
    }

    #[test]
    fn res_entries_are_dword_aligned() {
        let file = res_file(&[(RT_ICON, 1, vec![1, 2, 3])]);
        assert_eq!(&file[..8], &[0, 0, 0, 0, 32, 0, 0, 0]);
        assert_eq!(file.len(), 32 + 32 + 4);
        assert_eq!(&file[32..36], &3u32.to_le_bytes());
        assert_eq!(&file[40..44], &[0xFF, 0xFF, 3, 0]);
        assert_eq!(&file[64..68], &[1, 2, 3, 0]);
    }
}
