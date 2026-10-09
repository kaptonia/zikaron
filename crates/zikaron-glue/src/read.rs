//! Reading a disclosure kit's entries by the layout names in `names` (entries folder, entry suffix). The kit
//! core verifies the kit (`kitdir::verify_kit`); this only reads the bytes.

/// Whether a directory is a disclosure kit (its manifest is there).
pub fn is_kit(dir: &std::path::Path) -> bool {
    dir.join(crate::names::MANIFEST).is_file()
}

/// The entry files in a kit's entries folder, in name order (on error, the unreadable file's path).
pub fn kit_entries(dir: &std::path::Path) -> Result<Vec<Vec<u8>>, String> {
    crate::seam_v2();
    let room = dir.join(crate::names::ENTRIES_DIR);
    let mut names: Vec<std::path::PathBuf> = std::fs::read_dir(&room)
        .map_err(|_| room.display().to_string())?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.to_string_lossy().ends_with(crate::names::ENTRY_SUFFIX))
        .collect();
    names.sort();
    let mut items = Vec::new();
    for p in names {
        items.push(std::fs::read(&p).map_err(|_| p.display().to_string())?);
    }
    Ok(items)
}
