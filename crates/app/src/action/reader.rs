use super::*;

pub(super) fn read_book(shell: &mut Shell, address: &str, dir: &str) -> Result<Spawned, crate::fault::Fault> {
    let who = crate::readerx::who(address)?;
    let g = ground(shell)?;
    let eps = shell.endpoints.clone();
    // Bytes go through the check page's four levels: this machine, vault, record bundle, publish address; the
    // "record content" cell is the person's place for the record bundle or publish address level. Even when
    // empty the first two levels are tried, and the face names the level that supplied the material; none at
    // all means "not obtained", never passing for 0 entries.
    let shelf = shelf_of(shell, dir.trim());
    shell.book = None;
    Ok(shell.tasks.spawn(Kind::Book, move || {
        let found = crate::supplyx::find_book(&shelf, &who.hex());
        let bytes = found.supply.as_ref().map(|s| s.items.clone()).unwrap_or_default();
        // Scan to the chain head: the basis's upper bound is asked of the chain now (the smallest across
        // endpoints), never scanning 0..0.
        crate::task::stage_at(Kind::Book, 0);
        let g = to_head(&eps, g)?;
        crate::task::stage_at(Kind::Book, 1);
        let b = crate::readerx::read(&eps, &g, &who, &bytes)?;
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
