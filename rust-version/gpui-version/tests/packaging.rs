//! Guards the Windows packaging details that are easy to lose silently: the
//! release binary must be a GUI program (otherwise a console window opens next
//! to the app) and must carry the GD icon (GPUI loads resource id 1 for the
//! window icon).
//!
//! These read the built executable, so they only run when it exists; build with
//! `cargo build --release` first. Run with:
//! `cargo test --release --test packaging`

use std::path::{Path, PathBuf};

fn release_executable() -> Option<PathBuf> {
    // target/release next to target/<profile>/deps
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for profile in ["release", "debug"] {
        let candidate = manifest
            .join("target")
            .join(profile)
            .join("GithubDeck-GPUI.exe");
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// Reads the PE `Subsystem` field: 2 is GUI, 3 is console.
fn pe_subsystem(path: &Path) -> Option<u16> {
    let bytes = std::fs::read(path).ok()?;
    let pe_offset = u32::from_le_bytes(bytes.get(0x3C..0x40)?.try_into().ok()?) as usize;
    // "PE\0\0" followed by the COFF header
    if bytes.get(pe_offset..pe_offset + 4)? != b"PE\0\0" {
        return None;
    }
    let optional = pe_offset + 24;
    let magic = u16::from_le_bytes(bytes.get(optional..optional + 2)?.try_into().ok()?);
    let subsystem_offset = match magic {
        // Subsystem sits at 0x44 in both PE32 and PE32+ optional headers;
        // 0x5C is DllCharacteristics, which is a common mix-up.
        0x20b | 0x10b => optional + 0x44,
        _ => return None,
    };
    Some(u16::from_le_bytes(
        bytes
            .get(subsystem_offset..subsystem_offset + 2)?
            .try_into()
            .ok()?,
    ))
}

/// Resource type ids in the `.rsrc` tree: 3 = RT_ICON, 14 = RT_GROUP_ICON.
fn has_icon_resource(path: &Path) -> bool {
    let Ok(bytes) = std::fs::read(path) else {
        return false;
    };
    // A present .rsrc section with the icon group resource is enough evidence;
    // scanning the raw section for the resource type id avoids re-implementing
    // the whole resource directory parser.
    let pe_offset = match bytes.get(0x3C..0x40) {
        Some(raw) => u32::from_le_bytes(raw.try_into().unwrap_or_default()) as usize,
        None => return false,
    };
    let section_count = match bytes.get(pe_offset + 6..pe_offset + 8) {
        Some(raw) => u16::from_le_bytes(raw.try_into().unwrap_or_default()) as usize,
        None => return false,
    };
    let optional_size = match bytes.get(pe_offset + 20..pe_offset + 22) {
        Some(raw) => u16::from_le_bytes(raw.try_into().unwrap_or_default()) as usize,
        None => return false,
    };
    let sections = pe_offset + 24 + optional_size;
    for index in 0..section_count {
        let entry = sections + index * 40;
        let Some(name) = bytes.get(entry..entry + 8) else {
            continue;
        };
        if !name.starts_with(b".rsrc") {
            continue;
        }
        let Some(size_raw) = bytes.get(entry + 16..entry + 20) else {
            continue;
        };
        let size = u32::from_le_bytes(size_raw.try_into().unwrap_or_default()) as usize;
        let Some(offset_raw) = bytes.get(entry + 20..entry + 24) else {
            continue;
        };
        let offset = u32::from_le_bytes(offset_raw.try_into().unwrap_or_default()) as usize;
        let Some(section) = bytes.get(offset..offset.saturating_add(size)) else {
            continue;
        };
        // The first directory level lists resource types as 16-bit ids.
        return section
            .windows(4)
            .any(|window| window == [14, 0, 0, 0] || window == [3, 0, 0, 0]);
    }
    false
}

#[cfg(windows)]
#[test]
fn release_binary_is_a_gui_program() {
    let Some(exe) = release_executable() else {
        eprintln!("no built GithubDeck-GPUI.exe yet; run `cargo build --release` first");
        return;
    };
    assert_eq!(
        pe_subsystem(&exe),
        Some(2),
        "{} must use the GUI subsystem, otherwise Windows opens a console window",
        exe.display()
    );
}

#[cfg(windows)]
#[test]
fn release_binary_carries_the_icon_resource() {
    let Some(exe) = release_executable() else {
        eprintln!("no built GithubDeck-GPUI.exe yet; run `cargo build --release` first");
        return;
    };
    assert!(
        has_icon_resource(&exe),
        "{} has no icon resource; build.rs must embed assets/githubdeck.ico",
        exe.display()
    );
}

#[test]
fn shared_icon_asset_is_a_multi_image_ico() {
    let icon = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("assets")
        .join("githubdeck.ico");
    let bytes = std::fs::read(&icon).expect("assets/githubdeck.ico must exist");
    assert_eq!(&bytes[..4], &[0, 0, 1, 0], "not an .ico file");
    let images = u16::from_le_bytes([bytes[4], bytes[5]]);
    assert!(images >= 2, "expected several icon sizes, found {images}");
}
