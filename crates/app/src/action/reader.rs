use super::*;

pub(super) fn read_book(shell: &mut Shell, address: &str, dir: &str) -> Result<Spawned, crate::fault::Fault> {
    let who = crate::readerx::who(address)?;
    let dir = one_place(dir)?;
    let g = ground(shell)?;
    let eps = shell.endpoints.clone();
    // Ledger bytes are looked up at the check page's four levels: this machine, the vault, a record bundle, a
    // publish address. The "record content" field gives the place for the last two; the first two are tried
    // even when it is empty. The result names the level that supplied the bytes; finding none means "not
    // obtained", never 0 entries.
    let shelf = shelf_of(shell, dir);
    let nets = read_nets_now()?;
    shell.book = None;
    Ok(shell.tasks.spawn(Kind::Book, move || {
        let found = crate::supplyx::find_book(&shelf, &who.hex());
        let bytes = found.supply.as_ref().map(|s| s.items.clone()).unwrap_or_default();
        // Scan up to the chain head (the lowest across endpoints), never 0..0.
        crate::task::stage_at(Kind::Book, 0);
        let (b, missed) = if nets.is_empty() {
            let g = to_head(&eps, g)?;
            crate::task::stage_at(Kind::Book, 1);
            (crate::readerx::read(&eps, &g, &who, &bytes)?, Vec::new())
        } else {
            // Across networks: each chain's head is fetched along with its window.
            crate::task::stage_at(Kind::Book, 1);
            crate::readerx::read_wide(&eps, &g, &who, &bytes, &nets)?
        };
        Ok(Done::Book {
            who: b.who,
            anchors: b.anchors,
            asked: b.asked,
            entries: b.entries,
            label: b.label,
            timeline: b.timeline,
            grants: b.grants,
            latest: b.latest,
            from: found.supply.as_ref().map(|s| (s.level, s.place.clone())),
            files: found.supply.as_ref().and_then(|s| s.files),
            misses: found.misses,
            missed,
        })
    }))
}

pub(super) fn book_address(shell: &mut Shell, address: &str, on: bool) -> Result<String, crate::fault::Fault> {
    let a = crate::readerx::who(address)?.hex();
    shell.commit_settings(|s| {
        if on {
            if !s.book.iter().any(|x| *x == a) {
                s.book.push(a.clone());
            }
        } else {
            s.book.retain(|x| *x != a);
        }
    })?;
    Ok(a)
}
