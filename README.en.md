# ZIKARON Desk

[中文](README.md) | English

Version 0.1.0

A desktop workbench for the zikaron/1 ledger law. It keeps a signed, append-only ledger of content
hashes, anchors the ledger on an Ethereum chain, issues and checks grants on recorded evidences and works,
and verifies what others hand over. One app serves two seats: the recorder, who records evidences and
works and grants rights, and the user, who holds grants and checks what they received.

The command line `zikaron` offers the same ledger actions as the window, one verb per action.

## Platforms

- macOS on Apple silicon (release packages)
- Linux x86_64 (build from source; no release packages for 0.1.0)

## Networks

The app ships with two known deployments of the registry contract (`base/zikaron-core/contracts`, one
pinned build, runtime codeHash `0xfa97a1d9b22fab2b52f4e27c9a965b32734c40001b565ab365d05c887118f57d`):

| Network | Chain id | Registry contract | From block |
|---|---|---|---|
| Ethereum mainnet (default) | 1 | `0x36Ea8A857a5FE813429d4D9947000C644A88809A` | 26087229 |
| Sepolia testnet | 11155111 | `0xC29410B882c4C3b77e33659d2f06ac563e7B08a3` | 11715660 |

Anchoring on mainnet spends real ETH for gas. The first-run wizard offers mainnet (selected) and a custom
network; to try the app out on the testnet, choose Custom and fill in the Sepolia row above. A custom chain,
contract and nodes can also be set in Settings.

## Install

The 0.1.0 release provides macOS packages. On Linux, build from source (see below).

**macOS.** Install `ZIKARON.app` from the `.dmg` (drag it into Applications) or with the `.pkg`
installer. The app is signed with a self-signed certificate (Kaptonia) and is not notarized. Files downloaded in a browser carry macOS's quarantine flag, and macOS refuses to open them; on recent macOS, "Open Anyway" in System Settings > Privacy & Security does not always work either. Check the SHA-256 with `shasum -a 256 <file>` first, then remove the quarantine flag in Terminal:

- With the dmg: before opening the dmg, run the command below, then open the dmg and drag `ZIKARON.app` to Applications:

  ```
  xattr -d com.apple.quarantine ~/Downloads/ZIKARON-0.1.0-macos-arm64.dmg
  ```

  If you have already dragged the app in, run this instead:

  ```
  xattr -dr com.apple.quarantine /Applications/ZIKARON.app
  ```

- With the pkg: run the command below, then double-click the pkg to install:

  ```
  xattr -d com.apple.quarantine ~/Downloads/ZIKARON-0.1.0-macos-arm64.pkg
  ```

Change the paths to wherever you downloaded the files.

**Linux.** There are no prebuilt packages for 0.1.0. Build from source as described below; on Linux,
`packaging/linux/build.sh` also makes a `.deb` and an AppImage. The window program is `zikaron-desk`.
The system file dialog goes through the desktop portal, so `xdg-desktop-portal` and a backend such as `xdg-desktop-portal-gtk`
should be installed.

The user manual is in [`docs/manual/`](docs/manual/) ([English](docs/manual/MANUAL-en.md),
[中文](docs/manual/MANUAL-zh.md)). The command line's output shapes, exit codes and flags are in
[`CLI-SCHEMA.md`](CLI-SCHEMA.md).

## Build from source

Rust stable, 2021 edition.

On Linux, install the build dependencies first (Debian / Ubuntu):

    sudo apt install build-essential pkg-config libxkbcommon-dev libxkbcommon-x11-0 \
      libwayland-dev libx11-dev libxcursor-dev libxrandr-dev libxi-dev libgl1-mesa-dev \
      xdg-desktop-portal xdg-desktop-portal-gtk

Then build:

    cargo build --release

This produces two binaries:

| Binary | What it is |
|---|---|
| `target/release/app` | The desktop window (packaged as `ZIKARON.app` on macOS and as `zikaron-desk` on Linux) |
| `target/release/zikaron` | The command line |

The build also produces small helper binaries used by the tests.

## Test

    cargo test --workspace

## Packaging

| Script | Output |
|---|---|
| `packaging/macos/build.sh [--identity NAME]` | `ZIKARON.app` in a `.dmg` and a `.pkg` (macOS) |
| `packaging/linux/build.sh` | `.deb` and AppImage, built natively on Linux |
| `packaging/linux/cross-build.sh` | The same Linux x86_64 packages from another host, with zig and `cargo-zigbuild` |

Packages land in `dist/`, each with a `SHA256SUMS` file.

## Layout

| Path | Contents |
|---|---|
| `crates/zikaron` | The zikaron/1 core: canonical bytes, entries, signatures, audit |
| `crates/zikaron-kit` | The zikaron.kit/1 layer: documents, payloads, disclosure kits, depth, grant checks |
| `crates/zikaron-store` | Ledger storage on disk (standard library only) |
| `crates/zikaron-anchor` | Chain access: scanning anchors, JSON-RPC, sending anchoring transactions |
| `crates/zikaron-cli` | The `zikaron` command line |
| `crates/zikaron-glue` | Disclosure kit export and shared conventions |
| `crates/zikaron-ui` | The widget library and skin |
| `crates/app` | The desktop app |
| `base/` | The law texts, their reference implementations, conformance corpora and the registry contract |
| `contracts/` | Registry variants used by tests of the anchoring path |
| `packaging/` | Packaging scripts for macOS and Linux |
| `docs/manual/` | The user manual, in English and Chinese |
| `CLI-SCHEMA.md` | Output shapes, exit codes, refusals and flags of the command line |

No crate depends on `base/` by path.

## The law

The rules the product follows are in `base/zikaron-v1.md` (the ledger law) and
`base/zikaron-kit-v1.md` (the kit law). Comments in the code cite them as `law §N` and `kit law §N`.

## License

MIT. See [`LICENSE`](LICENSE). The bundled fonts keep their own license
(`crates/zikaron-ui/fonts/OFL-*.txt`).
