use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum Trouble {
    Occupied(String),
    Io(String, String),
}

impl Trouble {
    pub fn code(&self) -> &'static str {
        match self {
            Trouble::Occupied(_) => "E_OCCUPIED",
            Trouble::Io(..) => "E_IO",
        }
    }

    pub fn subject(&self) -> &str {
        match self {
            Trouble::Occupied(p) | Trouble::Io(p, _) => p.as_str(),
        }
    }

    pub fn said(&self) -> Option<&str> {
        match self {
            Trouble::Io(_, w) => Some(w.as_str()),
            Trouble::Occupied(_) => None,
        }
    }
}

fn say(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

fn failed(subject: &Path, what: &str, at: &Path, e: &std::io::Error) -> Trouble {
    Trouble::Io(say(subject), format!("{what} {}: {e}", at.display()))
}

fn beside(out: &Path) -> Result<PathBuf, Trouble> {
    let parent = zikaron_os::parent_dir(out);
    let nonce = nonce().map_err(|e| failed(Path::new(zikaron_os::ENTROPY_SOURCE), "read", Path::new(zikaron_os::ENTROPY_SOURCE), &e))?;
    Ok(parent.join(zikaron_store::layout::tmp_file_name(&nonce)))
}

fn nonce() -> std::io::Result<String> {
    let mut b = [0u8; 8];
    zikaron_os::fill_random(&mut b)?;
    Ok(b.iter().map(|x| format!("{x:02x}")).collect())
}

pub fn staging_beside(out: &Path) -> Result<PathBuf, Trouble> {
    beside(out)
}

pub fn land_bytes(out: &Path, bytes: &[u8]) -> Result<(), Trouble> {
    land_bytes_for(Readers::Anyone, out, bytes)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Readers {
    Anyone,
    Owner,
}

pub fn land_bytes_for(readers: Readers, out: &Path, bytes: &[u8]) -> Result<(), Trouble> {
    if out.exists() {
        return Err(Trouble::Occupied(say(out)));
    }
    let tmp = beside(out)?;
    let _ = std::fs::remove_file(&tmp);
    let wrote = (|| -> std::io::Result<()> {
        let mut f = create_for(readers, &tmp)?;
        f.write_all(bytes)?;
        f.sync_all()
    })();
    if let Err(e) = wrote {
        let _ = std::fs::remove_file(&tmp);
        return Err(failed(out, "write", &tmp, &e));
    }
    let linked = zikaron_os::land_new(&tmp, out);
    let _ = std::fs::remove_file(&tmp);
    match linked {
        Ok(_) => sync_parent(out),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Err(Trouble::Occupied(say(out))),
        Err(e) => Err(failed(out, "land", out, &e)),
    }
}

pub fn land_over(readers: Readers, out: &Path, bytes: &[u8]) -> Result<(), Trouble> {
    let tmp = beside(out)?;
    let _ = std::fs::remove_file(&tmp);
    let wrote = (|| -> std::io::Result<()> {
        let mut f = create_for(readers, &tmp)?;
        f.write_all(bytes)?;
        f.sync_all()
    })();
    if let Err(e) = wrote {
        let _ = std::fs::remove_file(&tmp);
        return Err(failed(out, "write", &tmp, &e));
    }
    zikaron_os::rename_over(&tmp, out).map_err(|u| {
        let _ = std::fs::remove_file(&tmp);
        failed(out, "land", &u.at, &u.error)
    })
}

#[derive(Debug)]
pub enum Streamed<E> {
    Fill(E),
    Land(Trouble),
}

pub fn land_stream<E>(readers: Readers, out: &Path, fill: impl FnOnce(&mut std::fs::File) -> Result<(), E>) -> Result<(), Streamed<E>> {
    if out.exists() {
        return Err(Streamed::Land(Trouble::Occupied(say(out))));
    }
    let tmp = beside(out).map_err(Streamed::Land)?;
    let _ = std::fs::remove_file(&tmp);
    let mut f = match create_for(readers, &tmp) {
        Ok(f) => f,
        Err(e) => return Err(Streamed::Land(failed(out, "write", &tmp, &e))),
    };
    if let Err(x) = fill(&mut f) {
        drop(f);
        let _ = std::fs::remove_file(&tmp);
        return Err(Streamed::Fill(x));
    }
    if let Err(e) = f.sync_all() {
        drop(f);
        let _ = std::fs::remove_file(&tmp);
        return Err(Streamed::Land(failed(out, "write", &tmp, &e)));
    }
    drop(f);
    let linked = zikaron_os::land_new(&tmp, out);
    let _ = std::fs::remove_file(&tmp);
    match linked {
        Ok(_) => sync_parent(out).map_err(Streamed::Land),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Err(Streamed::Land(Trouble::Occupied(say(out)))),
        Err(e) => Err(Streamed::Land(failed(out, "land", out, &e))),
    }
}

fn create_for(readers: Readers, tmp: &Path) -> std::io::Result<std::fs::File> {
    let mut o = zikaron_os::Options::new();
    o.write(true).create(true).truncate(true);
    if readers == Readers::Owner {
        zikaron_os::owner_only(&mut o);
    }
    o.open(tmp)
}

pub fn land_tree(out: &Path, staged: &Path) -> Result<(), Trouble> {
    if out.exists() {
        return Err(Trouble::Occupied(say(out)));
    }
    match std::fs::rename(staged, out) {
        Ok(()) => sync_parent(out),
        Err(e) => Err(failed(out, "rename", staged, &e)),
    }
}

pub fn put_new_from(path: &Path, body: &mut dyn std::io::Read) -> Result<(), Trouble> {
    let mut o = zikaron_os::Options::new();
    o.write(true).create_new(true);
    let mut f = o.open(path).map_err(|e| if e.kind() == std::io::ErrorKind::AlreadyExists { Trouble::Occupied(say(path)) } else { failed(path, "write", path, &e) })?;
    std::io::copy(body, &mut f).map_err(|e| failed(path, "write", path, &e))?;
    f.sync_all().map_err(|e| failed(path, "write", path, &e))
}

pub fn take_back(at: &Path, tree: bool) {
    let _ = if tree { std::fs::remove_dir_all(at) } else { std::fs::remove_file(at) };
}

pub fn place_file(staged: &Path, out: &Path) -> Result<(), Trouble> {
    let linked = zikaron_os::land_new(staged, out);
    let _ = std::fs::remove_file(staged);
    match linked {
        Ok(_) => sync_parent(out),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Err(Trouble::Occupied(say(out))),
        Err(e) => Err(failed(out, "land", out, &e)),
    }
}

pub fn put(path: &Path, bytes: &[u8]) -> Result<(), Trouble> {
    std::fs::write(path, bytes).map_err(|e| failed(path, "write", path, &e))
}

pub fn put_from(path: &Path, body: &mut dyn std::io::Read) -> Result<(), Trouble> {
    let mut f = std::fs::File::create(path).map_err(|e| failed(path, "write", path, &e))?;
    std::io::copy(body, &mut f).map(|_| ()).map_err(|e| failed(path, "write", path, &e))
}

pub fn mkdir(path: &Path) -> Result<(), Trouble> {
    std::fs::create_dir_all(path).map_err(|e| failed(path, "create directory", path, &e))
}

fn sync_parent(out: &Path) -> Result<(), Trouble> {
    let parent = zikaron_os::parent_dir(out);
    zikaron_os::sync_dir(parent, out).map_err(|e| failed(parent, "directory sync", parent, &e))
}
