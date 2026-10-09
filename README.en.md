# ZIKARON Desk

[中文](README.md) | English

Version 0.1.2

Decentralized Infrastructure of Right: <https://kaptonia.github.io/>

A desktop workbench for the zikaron/1 ledger law. It keeps a signed, append-only ledger of content
hashes, anchors the ledger on an Ethereum chain, issues and checks grants on recorded evidence and works,
and verifies what others hand over. One app serves two roles: the recorder, who records evidence and
works and grants rights, and the user, who holds grants and checks what they received.

The command line `zikaron` offers the same ledger actions as the window, one verb per action. It works on plain
ledger folders and on the ledger mirrors and record kits the app exports; it does not read the local data the
app keeps sealed. With `--home`, writing and putting on chain are done by the running desktop app in its own data folder,
and the entries show in its window at once.

## Download

The 0.1.2 packages ([release page](https://github.com/kaptonia/zikaron/releases/tag/v0.1.2)):

| Platform | Download |
|---|---|
| macOS (Apple silicon, macOS 11 or later) | [`ZIKARON-0.1.2-macos-arm64.dmg`](https://github.com/kaptonia/zikaron/releases/download/v0.1.2/ZIKARON-0.1.2-macos-arm64.dmg) · [`.pkg` installer](https://github.com/kaptonia/zikaron/releases/download/v0.1.2/ZIKARON-0.1.2-macos-arm64.pkg) |
| Windows 10 / 11 (x86_64) | [`ZIKARON-0.1.2-windows-x86_64.zip`](https://github.com/kaptonia/zikaron/releases/download/v0.1.2/ZIKARON-0.1.2-windows-x86_64.zip) |
| Linux (x86_64, glibc 2.31 or later) | [`zikaron-desk_0.1.2_amd64.deb`](https://github.com/kaptonia/zikaron/releases/download/v0.1.2/zikaron-desk_0.1.2_amd64.deb) · [`AppImage`](https://github.com/kaptonia/zikaron/releases/download/v0.1.2/ZIKARON-0.1.2-x86_64.AppImage) |
| Source | [`zikaron-0.1.2-src.tar.gz`](https://github.com/kaptonia/zikaron/releases/download/v0.1.2/zikaron-0.1.2-src.tar.gz) |

Check the SHA-256 against [`SHA256SUMS.txt`](https://github.com/kaptonia/zikaron/releases/download/v0.1.2/SHA256SUMS.txt) first, then install as described under "Install" below.

## Platforms

- macOS on Apple silicon (release packages)
- Linux x86_64 (`.deb` and AppImage release packages)
- Windows 10 and 11 on x86_64 (a zip package: unpack and run)

Every push to `main` and every pull request builds and runs `cargo test --workspace` on macOS, Linux and Windows (`.github/workflows/test.yml`); until it has been run on a Windows machine, the Windows cell is not yet required to pass.

## Networks

The app ships with four known deployments of the registry contract (`base/zikaron-core/contracts`, one
pinned build, runtime codeHash `0xfa97a1d9b22fab2b52f4e27c9a965b32734c40001b565ab365d05c887118f57d`):

| Network | Chain id | Registry contract | From block |
|---|---|---|---|
| Ethereum mainnet (default) | 1 | `0x36Ea8A857a5FE813429d4D9947000C644A88809A` | 26087229 |
| Sepolia testnet | 11155111 | `0xC29410B882c4C3b77e33659d2f06ac563e7B08a3` | 11715660 |
| Arbitrum One | 42161 | `0x36Ea8A857a5FE813429d4D9947000C644A88809A` | 511445184 |
| OP Mainnet | 10 | `0x36Ea8A857a5FE813429d4D9947000C644A88809A` | 157735914 |

Anchoring on mainnet and the two L2 networks spends real ETH for gas. Each identity keeps the network it
uses: when you create or import an identity, pick any row above under Network; the first-run wizard offers mainnet
(selected) and a custom network. To try the app out on the testnet, choose Sepolia testnet when you create an
identity. A custom chain, contract and nodes can also be set in Settings. Settings can also add read-only
networks, used only to check others' material and never to send transactions.

## Install

The 0.1.2 release provides macOS and Linux packages and a Windows zip; each carries the third-party licences.

**macOS.** Install `ZIKARON.app` from the `.dmg` (drag it into Applications) or with the `.pkg`
installer. The app is signed with a self-signed certificate (Kaptonia) and is not notarized. Files downloaded in a browser carry macOS's quarantine flag, and macOS refuses to open them; on recent macOS, "Open Anyway" in System Settings > Privacy & Security does not always work either. Check the SHA-256 with `shasum -a 256 <file>` first, then remove the quarantine flag in Terminal:

- With the dmg: before opening the dmg, run the command below, then open the dmg and drag `ZIKARON.app` to Applications:

  ```
  xattr -d com.apple.quarantine ~/Downloads/ZIKARON-0.1.2-macos-arm64.dmg
  ```

  If you have already dragged the app in, run this instead:

  ```
  xattr -dr com.apple.quarantine /Applications/ZIKARON.app
  ```

- With the pkg: run the command below, then double-click the pkg to install:

  ```
  xattr -d com.apple.quarantine ~/Downloads/ZIKARON-0.1.2-macos-arm64.pkg
  ```

Change the paths to wherever you downloaded the files.

macOS 11 or later is required. The command line is `ZIKARON.app/Contents/MacOS/zikaron`; the pkg also installs a
copy at `/usr/local/bin/zikaron`. After installing from the dmg, drag the app into Applications first, then turn on "Enable command line" in the app's Settings > Local data (it places a link in `/usr/local/bin`, with the system's administrator password dialog when needed) to type
`zikaron` in a terminal.

**Linux.** On x86_64, use the `.deb` (`sudo apt install ./zikaron-desk_0.1.2_amd64.deb`, which installs the window
program `zikaron-desk` and the command line `zikaron` in `/usr/bin`) or the AppImage (make it executable with `chmod +x` and run it;
it launches the window program only, the command line being inside but not exposed, and "Enable command line" is not available in it; without FUSE, run it with
`--appimage-extract-and-run`). Both need glibc 2.31 or later; the packages are not signed, so check them with
`sha256sum <file>` first. On aarch64 Linux, `packaging/linux/build.sh` makes only the `.deb` (an AppImage runtime is pinned for x86_64 only); other architectures
build from source only, as described below.
The system file dialog goes through the desktop portal, so `xdg-desktop-portal` and a backend such as `xdg-desktop-portal-gtk`
should be installed.

**Windows.** On Windows 10 or 11 (x86_64) use `ZIKARON-0.1.2-windows-x86_64.zip`: check the SHA-256 first in PowerShell with `Get-FileHash <file> -Algorithm SHA256`, unpack it into any folder and double-click `zikaron-desk.exe`; the command line is `zikaron.exe` in the same folder (run it in a terminal; double-clicked, it flashes and closes). Nothing is installed and no runtime is needed. The programs are not signed: on first start Windows says "Windows protected your PC"; click "More info", then "Run anyway". On Windows 11 with Smart App Control on, unsigned programs are blocked outright and cannot be allowed one by one; turn it off first in Windows Security › App & browser control › Smart App Control settings. Machine data lives in `%LOCALAPPDATA%\ZIKARON\` by default; deleting the unpacked folder does not delete it. To type `zikaron` in any terminal, turn on "Enable command line" in the app's Settings > Local data, which adds the command line's folder to this user's `PATH`.

The user manual is in [`docs/manual/`](docs/manual/) ([English](docs/manual/MANUAL-en.md),
[中文](docs/manual/MANUAL-zh.md)).

What changed in each version is in [`CHANGELOG.en.md`](CHANGELOG.en.md).

The command line's output shapes, exit codes and flags are in [`CLI-SCHEMA.md`](CLI-SCHEMA.md).

## Build from source

Rust 1.97.1 (pinned by `rust-toolchain.toml`; rustup picks it up by itself), 2024 edition.

On Linux, install the build dependencies first (Debian / Ubuntu):

    sudo apt install build-essential pkg-config libxkbcommon-dev libxkbcommon-x11-0 \
      libwayland-dev libx11-dev libxcursor-dev libxrandr-dev libxi-dev libgl1-mesa-dev \
      xdg-desktop-portal xdg-desktop-portal-gtk

Then build:

    cargo build --release

This produces two binaries:

| Binary | What it is |
|---|---|
| `target/release/app` | The desktop window (packaged as `ZIKARON.app` on macOS, as `zikaron-desk` on Linux and as `zikaron-desk.exe` on Windows) |
| `target/release/zikaron` | The command line |

The build also produces a few small programs: `zk1`, `zkk`, `zka`, `zkg` and `zks` for conformance tests and test
drivers, and the packaging tool `zikaron-pack`.

## Test

    cargo test --workspace

## Packaging

| Script | Output |
|---|---|
| `packaging/macos/build.sh [--identity NAME]` | `ZIKARON.app` in a `.dmg` and a `.pkg` (macOS) |
| `packaging/linux/build.sh` | Built natively on Linux: `.deb` and AppImage on x86_64; only the `.deb` on aarch64 (the AppImage step stops with an error) |
| `packaging/linux/cross-build.sh` | The same Linux x86_64 packages from another host, with zig and `cargo-zigbuild` |
| `cargo run --release --locked -p zikaron-pack -- windows --out dist` | The Windows x86_64 package folder: window binary, command line, licences (built on Windows with the MSVC toolchain; `.github/workflows/package-windows.yml` runs exactly this) |

The macOS and Linux packages land in `dist/`, each with a `SHA256SUMS-<os>-<arch>.txt`; the Windows command lays out
the folder `dist/ZIKARON-<version>-windows-x86_64/` only, with no zip and no checksum file (the zip on the release page is made from that folder). Every package carries the third-party notices (`THIRD-PARTY-LICENSES.txt`, made from `Cargo.lock` for the target) and nothing of the building machine's user name, owners, host or system version.

## Layout

| Path | Contents |
|---|---|
| `crates/zikaron` | The zikaron/1 core: canonical bytes, entries, signatures, audit |
| `crates/zikaron-kit` | The zikaron.kit/1 layer: documents, payloads, disclosure kits, depth, grant checks |
| `crates/zikaron-store` | Ledger storage on disk (standard library only) |
| `crates/zikaron-net` | Transport: one HTTP or HTTPS exchange with a node or a remote file (TLS set up in one place) |
| `crates/zikaron-anchor` | Chain access: scanning anchors, JSON-RPC, sending anchoring transactions |
| `crates/zikaron-cli` | The `zikaron` command line |
| `crates/zikaron-glue` | Reading and writing disclosure kits and grant files, ledger mirrors, and conventions the app and the command line share |
| `crates/zikaron-os` | What differs by operating system: system randomness, owner-only files, syncing to disk, a rename that replaces, system proxy settings, where the machine directory is, "Enable command line", the local channel between the app and the command line |
| `crates/zikaron-pack` | Packaging pieces: the third-party notices generator, the steps that keep the building machine out of a package, building and laying out the Windows package, checking the AppImage runtime |
| `crates/zikaron-ui` | The widget library and skin |
| `crates/app` | The desktop app |
| `base/` | The law texts, their reference implementations, conformance corpora and the registry contract |
| `contracts/` | Registry variants used by tests of the anchoring path |
| `packaging/` | Packaging scripts for macOS and Linux, and the icons |
| `docs/manual/` | The user manual, in English and Chinese |
| `CLI-SCHEMA.md` | Output shapes, exit codes, refusals and flags of the command line |

No crate depends on `base/` by path.

## The law

The rules the product follows are in `base/zikaron-v1.md` (the ledger law) and
`base/zikaron-kit-v1.md` (the kit law). Comments in the code cite them as `law §N` and `kit law §N`.

## License

MIT. See [`LICENSE`](LICENSE). The bundled fonts keep their own license
(`crates/zikaron-ui/fonts/OFL-*.txt`).
