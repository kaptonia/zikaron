# ZIKARON Desk

中文 | [English](README.en.md)

版本 0.1.1

去中心化法权基础设施：<https://kaptonia.github.io/>

一个面向 zikaron/1 账本法的桌面工作台。它保管一本签名的、只增不改的内容哈希账本，把账本锚定到以太坊链上，为已记录的证据和作品签发、核验授权，并核验别人交来的东西。一个应用服务两种角色：记录者，记录证据和作品并授予权利；使用方，持有授权并核验收到的东西。

命令行 `zikaron` 提供与窗口相同的账本操作，一个动作一个动词。它读写普通的账本文件夹，以及应用导出的镜像包和记录包；应用加封保存的本机数据，命令行不读。

## 下载

0.1.1 的安装包（[发布页](https://github.com/kaptonia/zikaron/releases/tag/v0.1.1)）：

| 平台 | 下载 |
|---|---|
| macOS（Apple 芯片，macOS 11 起） | [`ZIKARON-0.1.1-macos-arm64.dmg`](https://github.com/kaptonia/zikaron/releases/download/v0.1.1/ZIKARON-0.1.1-macos-arm64.dmg) · [`.pkg` 安装包](https://github.com/kaptonia/zikaron/releases/download/v0.1.1/ZIKARON-0.1.1-macos-arm64.pkg) |
| Windows 10 / 11（x86_64） | [`ZIKARON-0.1.1-windows-x86_64.zip`](https://github.com/kaptonia/zikaron/releases/download/v0.1.1/ZIKARON-0.1.1-windows-x86_64.zip) |
| Linux（x86_64，glibc 2.31 起） | [`zikaron-desk_0.1.1_amd64.deb`](https://github.com/kaptonia/zikaron/releases/download/v0.1.1/zikaron-desk_0.1.1_amd64.deb) · [`AppImage`](https://github.com/kaptonia/zikaron/releases/download/v0.1.1/ZIKARON-0.1.1-x86_64.AppImage) |
| 源码 | [`zikaron-0.1.1-src.tar.gz`](https://github.com/kaptonia/zikaron/releases/download/v0.1.1/zikaron-0.1.1-src.tar.gz) |

下载后先用 [`SHA256SUMS.txt`](https://github.com/kaptonia/zikaron/releases/download/v0.1.1/SHA256SUMS.txt) 核对 SHA-256，再按下文「安装」一节安装。

## 平台

- Apple 芯片的 macOS（提供安装包）
- x86_64 的 Linux（提供 `.deb` 与 AppImage 安装包）
- x86_64 的 Windows 10 与 11（提供 zip 包，解压即用）

每次推到 `main` 和每个合并请求，都在 macOS、Linux 与 Windows 上构建并跑 `cargo test --workspace`（`.github/workflows/test.yml`）；在 Windows 实机上走过之前，Windows 一格还不要求通过。

## 网络

应用内置登记合约的四处已知部署（`base/zikaron-core/contracts`，一份钉定的构建，运行时 codeHash `0xfa97a1d9b22fab2b52f4e27c9a965b32734c40001b565ab365d05c887118f57d`）：

| 网络 | 链号 | 登记合约 | 起始区块 |
|---|---|---|---|
| 以太坊主网（默认） | 1 | `0x36Ea8A857a5FE813429d4D9947000C644A88809A` | 26087229 |
| Sepolia 测试网 | 11155111 | `0xC29410B882c4C3b77e33659d2f06ac563e7B08a3` | 11715660 |
| Arbitrum One | 42161 | `0x36Ea8A857a5FE813429d4D9947000C644A88809A` | 511445184 |
| OP Mainnet | 10 | `0x36Ea8A857a5FE813429d4D9947000C644A88809A` | 157735914 |

在主网与两条 L2 网络上锚定要花真实的 ETH 付 gas。每个身份记着自己用的网络：新建或导入身份时在「网络」里选上表任一行；首次启动向导提供主网（默认选中）和自定义网络。想在测试网上试用，新建身份时选「Sepolia 测试网」。自定义的链、合约和节点也可以在设置里配置。设置里还可以添加只读网络，只用来核验别人的材料，不在上面发交易。

## 安装

0.1.1 提供 macOS 与 Linux 安装包和 Windows 的 zip 包，每个包都附带第三方组件许可。

**macOS。** 从 `.dmg` 安装 `ZIKARON.app`（拖进「应用程序」），或用 `.pkg` 安装包安装。应用用自签证书（Kaptonia）签名，未经公证。从浏览器下载的文件带有 macOS 的隔离标记，系统会拒绝打开；在较新的 macOS 上，「系统设置 > 隐私与安全性」里的「仍要打开」也不一定有效。先用 `shasum -a 256 <文件>` 核对 SHA-256，再在「终端」里去掉隔离标记：

- 用 dmg：打开 dmg 之前先运行下面这条，再打开 dmg，把 `ZIKARON.app` 拖进「应用程序」：

  ```
  xattr -d com.apple.quarantine ~/Downloads/ZIKARON-0.1.1-macos-arm64.dmg
  ```

  如果已经拖进去了，改为运行：

  ```
  xattr -dr com.apple.quarantine /Applications/ZIKARON.app
  ```

- 用 pkg：先运行下面这条，再双击安装：

  ```
  xattr -d com.apple.quarantine ~/Downloads/ZIKARON-0.1.1-macos-arm64.pkg
  ```

命令里的路径按你实际下载的位置改。

要求 macOS 11 或更新。命令行在 `ZIKARON.app/Contents/MacOS/zikaron`；用 pkg 安装时，另装一份到 `/usr/local/bin/zikaron`。

**Linux。** x86_64 用 `.deb`（`sudo apt install ./zikaron-desk_0.1.1_amd64.deb`，装上窗口程序 `zikaron-desk` 与命令行 `zikaron`）或 AppImage（`chmod +x` 后直接运行，只启动窗口程序，命令行在包内但不直接暴露；系统没有 FUSE 时加 `--appimage-extract-and-run`）。两者都要求 glibc 2.31 或更新，包没有签名，先用 `sha256sum <文件>` 核对。在 aarch64 的 Linux 上，`packaging/linux/build.sh` 也能打出这两种包；其他架构只能按下文从源码构建。系统文件对话框经由桌面门户，所以要装 `xdg-desktop-portal` 和一个后端，如 `xdg-desktop-portal-gtk`。

**Windows。** x86_64 的 Windows 10 或 11 用 `ZIKARON-0.1.1-windows-x86_64.zip`：先在 PowerShell 里用 `Get-FileHash <文件> -Algorithm SHA256` 核对 SHA-256，解压到任意文件夹，双击 `zikaron-desk.exe` 即开，命令行是同一文件夹里的 `zikaron.exe`（要在终端里运行，双击它会一闪而过）。不用安装，也不需另装运行库。程序没有签名，首次打开时 Windows 会提示「Windows 已保护你的电脑」，按「更多信息」再按「仍要运行」。Windows 11 开着「智能应用控制」时，未签名的程序会被直接拦下、不能单独放行，要先在「Windows 安全中心 › 应用和浏览器控制 › 智能应用控制设置」里关掉它。机器数据默认在 `%LOCALAPPDATA%\ZIKARON\`，删掉解压的文件夹不会删掉它。

用户手册在 [`docs/manual/`](docs/manual/)（[中文](docs/manual/MANUAL-zh.md)、[English](docs/manual/MANUAL-en.md)）。

各版本的更新见 [`CHANGELOG.md`](CHANGELOG.md)。

命令行的输出形状、退出码和参数见 [`CLI-SCHEMA.md`](CLI-SCHEMA.md)。

## 从源码构建

Rust stable，2021 edition。

在 Linux 上先装构建依赖（Debian / Ubuntu）：

    sudo apt install build-essential pkg-config libxkbcommon-dev libxkbcommon-x11-0 \
      libwayland-dev libx11-dev libxcursor-dev libxrandr-dev libxi-dev libgl1-mesa-dev \
      xdg-desktop-portal xdg-desktop-portal-gtk

然后构建：

    cargo build --release

会产出两个程序：

| 程序 | 是什么 |
|---|---|
| `target/release/app` | 桌面窗口（在 macOS 上打包为 `ZIKARON.app`，在 Linux 上为 `zikaron-desk`，在 Windows 上为 `zikaron-desk.exe`） |
| `target/release/zikaron` | 命令行 |

构建还会产出几个小程序：一致性测试与测试驱动用的 `zk1`、`zkk`、`zka`、`zkg`、`zks`，以及打包工具 `zikaron-pack`。

## 测试

    cargo test --workspace

## 打包

| 脚本 | 产出 |
|---|---|
| `packaging/macos/build.sh [--identity NAME]` | 装在 `.dmg` 里的 `ZIKARON.app` 和一个 `.pkg`（macOS） |
| `packaging/linux/build.sh` | `.deb` 和 AppImage，在 x86_64 或 aarch64 的 Linux 上本机构建 |
| `packaging/linux/cross-build.sh` | 在别的主机上用 zig 和 `cargo-zigbuild` 交叉打出同样的 Linux x86_64 包 |
| `cargo run --release --locked -p zikaron-pack -- windows --out dist` | Windows x86_64 包的文件夹：窗口程序、命令行、许可（在 Windows 上用 MSVC 工具链构建；`.github/workflows/package-windows.yml` 即调这一条） |

macOS 与 Linux 的包落在 `dist/`，各附一份 `SHA256SUMS-<系统>-<架构>.txt`；Windows 那一条命令只排出文件夹 `dist/ZIKARON-<版本>-windows-x86_64/`，不打 zip，也不出校验和文件（发布页上的 zip 由这个文件夹压成）。每个包都带第三方许可声明（`THIRD-PARTY-LICENSES.txt`，按目标平台从 `Cargo.lock` 现生），不带构建机的用户名、属主、主机或系统版本。

## 目录

| 路径 | 内容 |
|---|---|
| `crates/zikaron` | zikaron/1 核心：规范字节、条目、签名、审计 |
| `crates/zikaron-kit` | zikaron.kit/1 层：文书、载荷、披露包、深度、授权核验 |
| `crates/zikaron-store` | 账本的盘上存储（只用标准库） |
| `crates/zikaron-net` | 传输：到节点与远端档的一趟 HTTP/HTTPS 问答（TLS 配置只此一处） |
| `crates/zikaron-anchor` | 链访问：扫描锚、JSON-RPC、发送锚定交易 |
| `crates/zikaron-cli` | 命令行 `zikaron` |
| `crates/zikaron-glue` | 披露包导出与共用约定 |
| `crates/zikaron-os` | 随操作系统而异的几样：系统随机数、只本人可读写的文件、落盘同步、替换式改名 |
| `crates/zikaron-pack` | 打包件：第三方许可声明生成器，以及让包里不带构建机信息的几步 |
| `crates/zikaron-ui` | 组件库与外观 |
| `crates/app` | 桌面应用 |
| `base/` | 法文本、其参考实现、一致性语料和登记合约 |
| `contracts/` | 锚定路径测试用的登记合约变体 |
| `packaging/` | macOS 与 Linux 的打包脚本与图标 |
| `docs/manual/` | 用户手册，中英两份 |
| `CLI-SCHEMA.md` | 命令行的输出形状、退出码、拒因和参数 |

没有任何 crate 按路径依赖 `base/`。

## 法

产品遵循的规则在 `base/zikaron-v1.md`（账本法）和 `base/zikaron-kit-v1.md`（kit 法）。代码注释以 `law §N` 和 `kit law §N` 引用它们。

## 许可

MIT，见 [`LICENSE`](LICENSE)。随附的字体保留各自的许可（`crates/zikaron-ui/fonts/OFL-*.txt`）。
