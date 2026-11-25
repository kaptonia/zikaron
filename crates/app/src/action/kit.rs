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
    companion: bool,
) -> Result<Spawned, crate::fault::Fault> {
    let form = match form.trim() {
        "" => zikaron_glue::pack::Form::File,
        w => zikaron_glue::pack::Form::of(w).ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::KitFormWord, w.to_string()))?,
    };
    let sealed_to = crate::addressedx::to_of(recipient, recipient_key)?;
    let word = crate::kitx::originals_word(originals).ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::KitOriginalsWord, originals.trim().to_string()))?;
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
    let out = crate::home::landing(out)?.display().to_string();
    let pick = crate::kitx::Pick::parse_ids(from, to, ids)?;
    let attachments: Vec<String> = attach
        .lines()
        .map(|x| x.trim().to_string())
        .filter(|x| !x.is_empty())
        .collect();
    let note = note.to_string();
    let publish = shell.settings.publish.clone();
    let ground = shell.settings.ground();
    let ask = crate::exitgate::ask_of(shell)?;
    Ok(shell.tasks.spawn(Kind::Kit, move || {
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
        let originals = crate::kitx::Originals::of(&crate::kitx::choose(&home, &pick)?);
        let (mut kept, mut left_out, mut unreadable) = (Vec::new(), Vec::new(), Vec::new());
        let mut withdrawn: Vec<String> = Vec::new();
        let mut attachments = attachments;
        if word.gather {
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
        if word.say_withdrawn {
            withdrawn = crate::kitx::withdrawn_originals(&home, &pick)?.into_iter().map(|o| o.path).collect();
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
            companion,
        )?;
        if key.is_none() {
            if let Ok(pile) = crate::kitx::whole(&home) {
                crate::task::stage_at(Kind::Kit, 3);
                let _ = crate::sendx::proofs_beside(&pile, &ground, std::path::Path::new(&made.path));
            }
        }
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
            withdrawn,
            facts: made.facts,
        })
    }))
}
