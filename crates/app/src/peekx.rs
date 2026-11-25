use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Ask {
    Kit { home: PathBuf, pick: String, rows: u64 },
    Names { paths: Vec<String> },
    GrantFile { path: String },
    Size { path: String },
    Landing { dir: String },
    Files { id: String, table: u64 },
}

impl Ask {
    pub fn which(&self) -> u8 {
        match self {
            Ask::Kit { .. } => 0,
            Ask::Names { .. } => 1,
            Ask::GrantFile { .. } => 2,
            Ask::Size { .. } => 3,
            Ask::Landing { .. } => 4,
            Ask::Files { .. } => 5,
        }
    }
}

#[derive(Clone, Debug)]
pub enum Got {
    Kit { preview: Result<(Vec<String>, Vec<String>), crate::fault::Fault>, originals: Result<Vec<crate::kitx::Listed>, crate::fault::Fault>, admit: Option<crate::kitx::Originals>, withdrawn: Result<Vec<crate::kitx::Listed>, crate::fault::Fault> },
    Names(Vec<(String, Result<Vec<(String, Option<String>)>, crate::fault::Fault>)>),
    GrantFile(Option<Result<(usize, usize), crate::fault::Fault>>),
    Size(Option<u64>),
    Landing(crate::home::Chosen),
    Files(Vec<String>),
}

pub fn read(ask: &Ask) -> Got {
    match ask {
        Ask::Kit { home, pick, .. } => {
            let h = crate::home::Home::bare(home.clone());
            let (from, rest) = pick.split_once('\n').unwrap_or((pick, ""));
            let (to, ids) = rest.split_once('\n').unwrap_or((rest, ""));
            match crate::kitx::Pick::parse_ids(from, to, ids) {
                Ok(p) => Got::Kit { preview: crate::kitx::preview(&h, &p), originals: crate::kitx::originals(&h, &p), admit: crate::kitx::choose(&h, &p).ok().map(|x| crate::kitx::Originals::of(&x)), withdrawn: crate::kitx::withdrawn_originals(&h, &p) },
                Err(f) => Got::Kit { preview: Err(f.clone()), originals: Err(f.clone()), admit: None, withdrawn: Err(f) },
            }
        }
        Ask::Names { paths } => Got::Names(paths.iter().map(|x| (x.clone(), crate::kitx::preview_names(Path::new(x)))).collect()),
        Ask::GrantFile { path } => {
            let p = Path::new(path);
            Got::GrantFile((!path.is_empty() && crate::grantfilex::is_grant_file(p)).then(|| crate::grantfilex::open(p).map(|o| (o.hops.len(), o.terms.len()))))
        }
        Ask::Size { path } => Got::Size(crate::anchorx::file_size(Path::new(path))),
        Ask::Landing { dir } => Got::Landing(crate::home::choose(&crate::home::Kind::Home, Path::new(dir))),
        Ask::Files { id, .. } => Got::Files(
            crate::home::machine_dir().and_then(|m| crate::recordsx::read(&m)).map(|rows| rows.into_iter().filter(|r| r.id.eq_ignore_ascii_case(id)).map(|r| r.name).collect()).unwrap_or_default(),
        ),
    }
}

pub fn pick_input(from: &str, to: &str, ids: &str) -> String {
    format!("{}\n{}\n{}", from.trim(), to.trim(), ids.trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_ask_reads_its_own_input() {
        let dir = std::env::temp_dir().join(format!("zk-peek-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("empty")).expect("a folder");
        std::fs::write(dir.join("a.txt"), b"twelve bytes").expect("a file");
        let a = dir.join("a.txt").display().to_string();
        assert!(matches!(read(&Ask::Size { path: a.clone() }), Got::Size(Some(12))));
        assert!(matches!(read(&Ask::GrantFile { path: a.clone() }), Got::GrantFile(None)), "a text file is no grant file");
        let empty = dir.join("empty").display().to_string();
        assert!(matches!(read(&Ask::Landing { dir: empty.clone() }), Got::Landing(c) if c.at == dir.join("empty")));
        assert!(matches!(read(&Ask::Names { paths: vec![a.clone()] }), Got::Names(v) if v.len() == 1 && v[0].1.is_ok()));
        assert_eq!(Ask::Size { path: a.clone() }, Ask::Size { path: a });
        assert_ne!(Ask::Landing { dir: empty }.which(), Ask::Size { path: String::new() }.which());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
