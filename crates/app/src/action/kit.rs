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
    form: &str,
    recipient: &str,
    recipient_key: &str,
    originals: &str,
) -> Result<Spawned, crate::fault::Fault> {
    // The form's word: `folder`, `file`, or none for one file; any other word is refused by name.
    let form = match form.trim() {
        "" => zikaron_glue::pack::Form::File,
        w => zikaron_glue::pack::Form::of(w).ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::KitFormWord, w.to_string()))?,
    };
    // Whom the package is sealed to, judged before any background work: at most one of the two, each in its form
    // (`addressedx::to_of`); only the one-file form is sealed, judged where the package is written (`kitx::export_to`).
    let sealed_to = crate::addressedx::to_of(recipient, recipient_key)?;
    // The originals' word: none, the chosen records' originals go in; `none`, they do not; any other is refused.
    let with_originals = match originals.trim() {
        "" => true,
        "none" => false,
        w => return Err(crate::fault::Fault::known(crate::fault::Known::KitOriginalsWord, w.to_string())),
    };
    // An address's key is looked for on the main network and every read-only network.
    let main = crate::addressedx::Main {
        chain: shell.settings.chain_id,
        registry: shell.settings.registry.map(|a| a.0),
        from_block: shell.settings.from_block,
        nodes: shell.endpoints.iter().filter(|e| Some(e.chain) == shell.settings.chain_id).map(|e| e.url.clone()).collect(),
    };
    let readonly = match sealed_to {
        Some(crate::addressedx::To::Address(_)) => read_nets_now()?,
        _ => Vec::new(),
    };
    let root = shell
        .home
        .as_ref()
        .map(|h| h.root().to_path_buf())
        .ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::NoHome, String::new()))?;
    // An empty or relative output path is refused before any background work (`home::landing`).
    let out = crate::home::landing(out)?.display().to_string();
    let pick = crate::kitx::Pick::parse_ids(from, to, ids)?;
    // One attachment per line of the multi-line field; blank lines are ignored.
    let attachments: Vec<String> = attach
        .lines()
        .map(|x| x.trim().to_string())
        .filter(|x| !x.is_empty())
        .collect();
    let note = note.to_string();
    let publish = shell.settings.publish.clone();
    let ask = crate::exitgate::ask_of(shell)?;
    Ok(shell.tasks.spawn(Kind::Kit, move || {
        // The recipient's key first: an address's is found on chain (its own proof), and every refusal leaves zero
        // bytes written.
        let key = match sealed_to {
            None => None,
            Some(crate::addressedx::To::Key(k)) => Some(k),
            Some(crate::addressedx::To::Address(a)) => {
                crate::task::stage_at(Kind::Kit, 3);
                Some(crate::addressedx::key_for(&a, &main, &readonly)?)
            }
        };
        crate::task::stage_at(Kind::Kit, 0);
        let home = crate::home::Home::open(&root)?;
        // In the UI, attachments are chosen by their digests at export time: files whose digest is not an
        // original of the selected records (or that cannot be read) are left out and reported one by one, and
        // the rest go to `kitx::export`, which checks each again. The UI's earlier hint is advisory only:
        // filtering by it would track paths, miss changed files and silently drop real originals. The command
        // line (`kit-out`) does not use this path and refuses strictly by name.
        let originals = crate::kitx::Originals::of(&crate::kitx::choose(&home, &pick)?);
        let (mut kept, mut left_out, mut unreadable) = (Vec::new(), Vec::new(), Vec::new());
        // Asked for the chosen records' originals: those the export page lists for this choice come first (one no
        // longer in place is left out as unreadable); every one is judged by its digest below as the page's are.
        let mut attachments = attachments;
        if with_originals {
            let mut listed = Vec::new();
            for o in crate::kitx::originals(&home, &pick)? {
                if !o.present {
                    unreadable.push((o.path.clone(), crate::fault::Fault::known(crate::fault::Known::FileMissing, o.path)));
                } else if !listed.contains(&o.path) && !attachments.contains(&o.path) {
                    listed.push(o.path);
                }
            }
            listed.append(&mut attachments);
            attachments = listed;
        }
        crate::task::stage_at(Kind::Kit, 1);
        let total = attachments.len() as u64;
        for (i, p) in attachments.into_iter().enumerate() {
            crate::task::count(Kind::Kit, i as u64, total);
            match crate::kitx::digest_of(std::path::Path::new(p.trim())) {
                Ok(d) if originals.admits_any(&d) => kept.push(p),
                Ok(_) => left_out.push(p),
                Err(f) => unreadable.push((p, f)),
            }
        }
        crate::task::stage_at(Kind::Kit, 2);
        // The exit gate, last before the bundle is written.
        let pass = crate::exitgate::pass(&ask)?;
        let made = crate::kitx::export_to(
            &pass,
            &home,
            &pick,
            &kept,
            &note,
            std::path::Path::new(&out),
            form,
            key.as_ref(),
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
            originals: made.originals,
            left_out,
            unreadable,
            facts: made.facts,
        })
    }))
}
