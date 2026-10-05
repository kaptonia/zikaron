use super::*;

pub(super) fn pick_kit(
    shell: &mut Shell,
    from: &str,
    to: &str,
    ids: &str,
) -> Result<(usize, Vec<String>), crate::fault::Fault> {
    let home = shell.home.as_ref().ok_or_else(|| {
        crate::fault::Fault::known(crate::fault::Known::NoHome, String::new())
    })?;
    let pick = crate::kitx::Pick::parse_ids(from, to, ids)?;
    let got = crate::kitx::choose(home, &pick)?;
    Ok((got.items.len(), got.pulled))
}

pub(super) fn export_kit(
    shell: &mut Shell,
    from: &str,
    to: &str,
    ids: &str,
    attach: &str,
    note: &str,
    out: &str,
) -> Result<Spawned, crate::fault::Fault> {
    let root = shell
        .home
        .as_ref()
        .map(|h| h.root().to_path_buf())
        .ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::NoHome, String::new()))?;
    // An empty or relative landing place is refused before going to the background and exporting
    // (`home::landing`).
    let out = crate::home::landing(out)?.display().to_string();
    let pick = crate::kitx::Pick::parse_ids(from, to, ids)?;
    // One attachment per line (blank lines ignored): the face's cell is multi-line.
    let attachments: Vec<String> = attach
        .lines()
        .map(|x| x.trim().to_string())
        .filter(|x| !x.is_empty())
        .collect();
    let note = note.to_string();
    let publish = shell.settings.publish.clone();
    let ask = crate::exitgate::ask_of(shell)?;
    Ok(shell.tasks.spawn(Kind::Kit, move || {
        crate::task::stage_at(Kind::Kit, 0);
        let home = crate::home::Home::open(&root)?;
        // On the interface path, which items go into the kit is decided by digests read at export time: items
        // whose digest is not an original of the selected records (including unreadable ones) are left out
        // and reported one by one, and the rest go to `kitx::export`, whose entry gate judges each again. The
        // interface line's sentence is an advance hint; filtering by it at export would track paths and miss
        // changed files, silently dropping real originals. The command line (`kit-out`) does not take this
        // path and still refuses strictly by name.
        let originals = crate::kitx::Originals::of(&crate::kitx::choose(&home, &pick)?);
        let (mut kept, mut left_out, mut unreadable) = (Vec::new(), Vec::new(), Vec::new());
        crate::task::stage_at(Kind::Kit, 1);
        let total = attachments.len() as u64;
        for (i, p) in attachments.into_iter().enumerate() {
            crate::task::count(Kind::Kit, i as u64, total);
            match crate::kitx::digest_of(std::path::Path::new(p.trim())) {
                Ok(d) if originals.admits(&d) => kept.push(p),
                Ok(_) => left_out.push(p),
                Err(f) => unreadable.push((p, f)),
            }
        }
        crate::task::stage_at(Kind::Kit, 2);
        // The exit gate, last before the bundle is written.
        let pass = crate::exitgate::pass(&ask)?;
        let made = crate::kitx::export(
            &pass,
            &home,
            &pick,
            &kept,
            &note,
            std::path::Path::new(&out),
        )?;
        Ok(Done::Kit {
            root,
            publish,
            path: made.path,
            kit_id: made.kit_id,
            entries: made.entries,
            files: made.files,
            pulled: made.pulled,
            dropped: made.dropped,
            left_out,
            unreadable,
        })
    }))
}
