//! `zikaron-pack`: the packaging pieces from a script.
//!
//! - `notices --target <triple> --root <package>... [--with <file>]... --out <file>`: the third-party notices.
//! - `hfs-owners <image> --epoch <seconds>`: owners and dates in the HFS+ catalog of a read/write image made
//!   with no partition map (the volume starts at the image's first byte).
//! - `dmg-names <image>`: the partition names of a disk image in one neutral form.
//! - `elf-comment <file>...`: zero the toolchain words of each ELF file's `.comment`.
//! - `windows --out <dir> [--target <triple>]`: build the Windows package and lay it out under `<dir>` (the
//!   target defaults to the package's own, `x86_64-pc-windows-msvc`).
//!
//! Each verb says what it did on one line and exits 0, or names what failed and exits 1.

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(said) => {
            println!("{said}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("zikaron-pack: {e}");
            ExitCode::from(1)
        }
    }
}

fn read(p: &str) -> Result<Vec<u8>, String> {
    std::fs::read(p).map_err(|e| format!("cannot read {p}: {e}"))
}
fn write(p: &str, b: &[u8]) -> Result<(), String> {
    std::fs::write(p, b).map_err(|e| format!("cannot write {p}: {e}"))
}

fn run(args: &[String]) -> Result<String, String> {
    match args.first().map(String::as_str) {
        Some("notices") => {
            let (mut target, mut out, mut roots, mut with) = (None, None, Vec::new(), Vec::new());
            let mut it = args[1..].iter();
            while let Some(a) = it.next() {
                let v = it.next().ok_or_else(|| format!("{a} wants a value"))?;
                match a.as_str() {
                    "--target" => target = Some(v.clone()),
                    "--out" => out = Some(v.clone()),
                    "--root" => roots.push(v.clone()),
                    "--with" => with.push(PathBuf::from(v)),
                    other => return Err(format!("unknown argument {other}")),
                }
            }
            let target = target.ok_or("--target is required")?;
            let out = out.ok_or("--out is required")?;
            if roots.is_empty() {
                return Err("at least one --root is required".into());
            }
            let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.toml");
            let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
            let r: Vec<&str> = roots.iter().map(String::as_str).collect();
            let text = zikaron_pack::notices::make(&cargo, &manifest, &target, &r, &with)?;
            write(&out, text.as_bytes())?;
            Ok(format!("notices: {} crates for {target} written to {out}", text.lines().take_while(|l| !l.trim().is_empty()).count()))
        }
        Some("hfs-owners") => {
            let [_, img, flag, epoch] = args else { return Err("usage: hfs-owners <image> --epoch <seconds>".into()) };
            if flag != "--epoch" {
                return Err("usage: hfs-owners <image> --epoch <seconds>".into());
            }
            let epoch: u64 = epoch.parse().map_err(|_| format!("--epoch {epoch} is not a number of seconds"))?;
            let mut b = read(img)?;
            let r = zikaron_pack::hfs::neutral_owners(&mut b, epoch)?;
            write(img, &b)?;
            Ok(format!("hfs-owners: {} records, {} owners changed, {} attribute records", r.records, r.owners_changed, r.attribute_records))
        }
        Some("dmg-names") => {
            let [_, img] = args else { return Err("usage: dmg-names <image>".into()) };
            let (b, n) = zikaron_pack::udif::neutral_names(&read(img)?)?;
            write(img, &b)?;
            Ok(format!("dmg-names: {n} names rewritten"))
        }
        Some("elf-comment") if args.len() > 1 => {
            let mut said = Vec::new();
            for f in &args[1..] {
                let mut b = read(f)?;
                let n = zikaron_pack::elf::neutral_comment(&mut b).map_err(|e| format!("{f}: {e}"))?;
                write(f, &b)?;
                said.push(format!("{n} bytes"));
            }
            Ok(format!("elf-comment: zeroed {}", said.join(", ")))
        }
        Some("windows") => {
            let (mut target, mut out) = (zikaron_pack::windows::TARGET.to_string(), None);
            let mut it = args[1..].iter();
            while let Some(a) = it.next() {
                let v = it.next().ok_or_else(|| format!("{a} wants a value"))?;
                match a.as_str() {
                    "--target" => target = v.clone(),
                    "--out" => out = Some(PathBuf::from(v)),
                    other => return Err(format!("unknown argument {other}")),
                }
            }
            let out = out.ok_or("--out is required")?;
            let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
            let at = zikaron_pack::windows::package(&root, &target, &out)?;
            Ok(format!("windows: laid out {}", at.display()))
        }
        _ => Err("usage: zikaron-pack notices|hfs-owners|dmg-names|elf-comment|windows ...".into()),
    }
}
