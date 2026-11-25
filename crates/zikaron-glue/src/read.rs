pub fn is_kit(dir: &std::path::Path) -> bool {
    dir.join(crate::names::MANIFEST).is_file()
}

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

pub struct Unpacked {
    pub dir: std::path::PathBuf,
}

impl Drop for Unpacked {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

pub fn unpack(file: &std::path::Path) -> Result<Unpacked, crate::container::Bad> {
    crate::seam_v2();
    let mut n = [0u8; 8];
    zikaron_os::fill_random(&mut n).map_err(|e| crate::container::Bad::Io(format!("read {}: {e}", zikaron_os::ENTROPY_SOURCE)))?;
    let dir = std::env::temp_dir().join(format!("zikaron-kit-{}", zikaron::hexfmt::encode(&n).trim_start_matches("0x")));
    crate::container::unpack_to(file, &dir)?;
    Ok(Unpacked { dir })
}
