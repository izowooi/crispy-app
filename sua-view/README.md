**English** · [한국어](docs/readme/README_ko.md)

# Sua View

**A small, lightweight image viewer built around speed.**

[Download](https://github.com/izowooi/crispy-app/releases/tag/sua-view-v0.1.0)

![Windows reading view with two original sample pages and minimal controls](docs/images/windows-reading.png)

## Open fast. Get straight to the image.

Sua View is a **minimal local image viewer focused on speed and a small footprint**. It puts loading speed at the center of its design: open a folder or archive, see the image, turn the page.

The core engine is written in **Rust**. It indexes ZIP/CBZ archives and reads the pages it needs without extracting the entire archive first. The interface uses native AppKit on macOS and WPF on Windows, with no Electron or WebView. Files stay local; no account or server connection is needed.

**Speed is the priority.** The goal is to open quickly and get out of the way. Actual loading time depends on image size, compression and storage.

## Download and launch

[**Download the public release →**](https://github.com/izowooi/crispy-app/releases/tag/sua-view-v0.1.0)

| Platform | Download | Start |
| --- | --- | --- |
| Windows 10/11 x64 | `Sua-View-0.1.0-windows-x64.zip` | **Extract everything**, then launch `SuaView.exe`. Keep the DLL and configuration file beside it. Requires .NET Framework 4.8. |
| macOS 12+ · Apple Silicon | `Sua-View-0.1.0-macOS-arm64.zip` | Extract and copy `Sua View.app` to Applications. |

Windows binaries are unsigned. The macOS app is ad-hoc signed and is not notarized by Apple. SHA-256 files are available with the release.

## Windows home and reading screens

![Windows home screen with the Open book button](docs/images/windows-home.png)

The home screen shows the **책 열기…** (Open book) button. Use it or `Ctrl+O` to open an archive or image. Drag and drop a file or folder into the window, or use `Ctrl+Shift+O` to choose an image folder.

The opening screenshot shows two-page, right-to-left reading. Controls fade away while reading and return when you move the mouse. All pictured pages are original synthetic samples created by this project. These screenshots show the current Windows UI in Korean.

## Windows: select in Explorer, press F3

Enable the shortcut once to open selected folders and ZIP/CBZ files, even when the viewer is closed.

1. Extract the release into a folder you plan to keep, then launch `SuaView.exe`.
2. **Right-click inside SuaView** and choose **탐색기 F3 연결 켜기** (Enable Explorer F3).
3. Select an image folder, `.zip` or `.cbz` file in Windows File Explorer.
4. Press **F3** to open it in SuaView.

<img src="docs/images/windows-f3-menu.png" alt="Windows context menu: the first item enables Explorer F3; the second disables it" width="250">

The first two menu items enable and disable the connection. The helper starts automatically when **the current user signs in**, without administrator privileges. It replaces Explorer's F3 search shortcut while leaving F3 in other apps unchanged. On Windows 11 it uses the active Explorer tab. If multiple items are selected, it opens the first one.

Choose **탐색기 F3 연결 끄기** (Disable Explorer F3), or **Disable F3 and startup** from the helper's notification-area menu, to remove the connection and automatic startup. Disable it before moving the app folder, then enable it again from the new location. For this session only, run `SuaViewF3.exe` directly without enabling automatic startup.

## macOS: select in Finder, press ⇧⌘J

Select a file or folder in Finder and press **⇧⌘J**. The service is called **Sua View로 열기** (Open with Sua View). If needed, enable it under **System Settings → Keyboard → Keyboard Shortcuts → Services**. You can change the shortcut there too. See the [macOS guide](docs/USAGE.md#finder에서-단축키로-열기) for service registration and source installation.

## Reading essentials

Natural file ordering, single-page and two-page views, left-to-right and right-to-left reading, zoom, fit, fullscreen and saved reading positions.

| Action | Windows | macOS |
| --- | --- | --- |
| Open file | `Ctrl+O` | `⌘O` |
| Open selection from Explorer / Finder | `F3` after setup | `⇧⌘J` after enabling the service |
| Previous / next | `←` / `→`, follows reading direction | Same |
| Next / previous | `Space` / `Shift+Space` | Same |
| Single / two pages | `D` | `D` |
| Switch reading direction | `R` | `R` |
| Fit width / fit screen | `W` / `0` | Same |
| Go to page | `G` | `G` |
| Fullscreen / exit | `F` / `Esc` | Same |

## Supported formats

| Input | Windows | macOS |
| --- | --- | --- |
| ZIP / CBZ · Stored, Deflate | Supported | Supported |
| Image folders, including subfolders | Supported | Supported |
| JPG, PNG, GIF, BMP, TIFF | Supported; first GIF frame | Subject to OS decoder support; first animation frame |
| RAR / CBR, 7z / CB7, TAR / CBT | Unsupported | Subject to system libarchive support |
| WebP, HEIC, AVIF | Unsupported | Subject to OS image decoder support |

PDF, encrypted archives and multipart archives are unsupported. On macOS, RAR/7z/TAR performance can differ from ZIP depending on archive structure. More details: [Windows guide](docs/WINDOWS.md) · [macOS guide](docs/USAGE.md).

## Build from source

Requires Rust 1.88+. On macOS, install Xcode Command Line Tools. On Windows, install MSVC C++ Build Tools, the Windows SDK and .NET Framework 4.8. Run the same command on either OS to build its native app:

```sh
cd sua-view
cargo run --locked --bin sua-view-build
```

Output: `dist/Sua View.app` on macOS; `dist/Sua-View-windows-x64/SuaView.exe` and a distribution ZIP on Windows. See [platform architecture](docs/PLATFORMS.md) for the Rust core and native UI boundaries.

## Development checks

```sh
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo fmt --check
```

Licensed under [MIT](../LICENSE). Dependency notices are included in release archives.
