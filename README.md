# ZIKARON Desk

中文 | [English](README.en.md)

版本 0.1.0

一个面向 zikaron/1 账本法的桌面工作台。它保管一本签名的、只增不改的内容哈希账本，把账本锚定到以太坊链上，为已记录的作品签发、核验授权，并核验别人交来的东西。一个应用服务两种角色：记录者，记录作品并授予权利；使用方，持有授权并核验收到的东西。

命令行 `zikaron` 提供与窗口相同的账本操作，一个动作一个动词。

## 平台

- Apple 芯片的 macOS（提供安装包）
- x86_64 的 Linux（从源码构建；0.1.0 不提供安装包）

## 网络

应用内置登记合约的两处已知部署（`base/zikaron-core/contracts`，一份钉定的构建，运行时 codeHash `0xfa97a1d9b22fab2b52f4e27c9a965b32734c40001b565ab365d05c887118f57d`）：

| 网络 | 链号 | 登记合约 | 起始区块 |
|---|---|---|---|
| 以太坊主网（默认） | 1 | `0x36Ea8A857a5FE813429d4D9947000C644A88809A` | 26087229 |
| Sepolia 测试网 | 11155111 | `0xC29410B882c4C3b77e33659d2f06ac563e7B08a3` | 11715660 |

在主网上锚定要花真实的 ETH 付 gas。首次启动向导提供主网（默认选中）和自定义网络；想在测试网上试用，选「自定义」，照上表 Sepolia 一行填写。自定义的链、合约和节点也可以在设置里配置。

## 安装

0.1.0 提供 macOS 安装包。Linux 请从源码构建（见下文）。

**macOS。** 从 `.dmg` 安装 `ZIKARON.app`（拖进「应用程序」），或用 `.pkg` 安装包安装。应用用自签证书（Kaptonia）签名，未经公证。从浏览器下载的文件带有 macOS 的隔离标记，系统会拒绝打开；在较新的 macOS 上，「系统设置 > 隐私与安全性」里的「仍要打开」也不一定有效。先用 `shasum -a 256 <文件>` 核对 SHA-256，再在「终端」里去掉隔离标记：

- 用 dmg：打开 dmg 之前先运行下面这条，再打开 dmg，把 `ZIKARON.app` 拖进「应用程序」：

  ```
  xattr -d com.apple.quarantine ~/Downloads/ZIKARON-0.1.0-macos-arm64.dmg
  ```

  如果已经拖进去了，改为运行：

  ```
  xattr -dr com.apple.quarantine /Applications/ZIKARON.app
  ```

- 用 pkg：先运行下面这条，再双击安装：

  ```
  xattr -d com.apple.quarantine ~/Downloads/ZIKARON-0.1.0-macos-arm64.pkg
  ```

命令里的路径按你实际下载的位置改。

**Linux。** 0.1.0 没有现成的安装包。按下文从源码构建；在 Linux 上，`packaging/linux/build.sh` 还能打出 `.deb` 和 AppImage。窗口程序是 `zikaron-desk`。系统文件对话框经由桌面门户，所以要装 `xdg-desktop-portal` 和一个后端，如 `xdg-desktop-portal-gtk`。

用户手册在 [`docs/manual/`](docs/manual/)（[中文](docs/manual/MANUAL-zh.md)、[English](docs/manual/MANUAL-en.md)）。命令行的输出形状、退出码和参数见 [`CLI-SCHEMA.md`](CLI-SCHEMA.md)。

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
| `target/release/app` | 桌面窗口（在 macOS 上打包为 `ZIKARON.app`，在 Linux 上为 `zikaron-desk`） |
| `target/release/zikaron` | 命令行 |

构建还会产出几个供测试用的小辅助程序。

## 测试

    cargo test --workspace

## 打包

| 脚本 | 产出 |
|---|---|
| `packaging/macos/build.sh [--identity NAME]` | 装在 `.dmg` 里的 `ZIKARON.app` 和一个 `.pkg`（macOS） |
| `packaging/linux/build.sh` | `.deb` 和 AppImage，在 Linux 上本机构建 |
| `packaging/linux/cross-build.sh` | 在别的主机上用 zig 和 `cargo-zigbuild` 交叉打出同样的 Linux x86_64 包 |

包落在 `dist/`，各附一份 `SHA256SUMS` 文件。

## 目录

| 路径 | 内容 |
|---|---|
| `crates/zikaron` | zikaron/1 核心：规范字节、条目、签名、审计 |
| `crates/zikaron-kit` | zikaron.kit/1 层：文书、载荷、披露包、深度、授权核验 |
| `crates/zikaron-store` | 账本的盘上存储（只用标准库） |
| `crates/zikaron-anchor` | 链访问：扫描锚、JSON-RPC、发送锚定交易 |
| `crates/zikaron-cli` | 命令行 `zikaron` |
| `crates/zikaron-glue` | 披露包导出与共用约定 |
| `crates/zikaron-ui` | 组件库与外观 |
| `crates/app` | 桌面应用 |
| `base/` | 法文本、其参考实现、一致性语料和登记合约 |
| `contracts/` | 锚定路径测试用的登记合约变体 |
| `packaging/` | macOS 与 Linux 的打包脚本 |
| `docs/manual/` | 用户手册，中英两份 |
| `CLI-SCHEMA.md` | 命令行的输出形状、退出码、拒因和参数 |

没有任何 crate 按路径依赖 `base/`。

## 法

产品遵循的规则在 `base/zikaron-v1.md`（账本法）和 `base/zikaron-kit-v1.md`（kit 法）。代码注释以 `law §N` 和 `kit law §N` 引用它们。

## 许可

MIT，见 [`LICENSE`](LICENSE)。随附的字体保留各自的许可（`crates/zikaron-ui/fonts/OFL-*.txt`）。
