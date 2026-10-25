# Sua View

Rust 코어와 macOS AppKit / Windows WPF 화면으로 구성한 로컬 만화책·이미지 뷰어입니다. 자연 정렬, 한 쪽·두 쪽 보기, 읽기 방향, 확대, 읽던 위치 저장을 지원합니다.

## macOS 빌드

macOS 12 이상, Rust 1.88 이상과 Xcode Command Line Tools가 필요합니다.

```sh
cd sua-view
cargo run --locked --bin sua-view-build
open 'dist/Sua View.app'
```

파일을 드래그하거나 `⌘O`로 엽니다. macOS에서는 ZIP/CBZ와 이미지 폴더, 시스템 libarchive가 지원하는 RAR/7z/TAR를 읽습니다. 암호화·분할 압축과 PDF는 지원하지 않습니다. 이미지 형식은 OS 디코더 지원에 따릅니다.

[사용법과 단축키](docs/USAGE.md) · [플랫폼 구조](docs/PLATFORMS.md) · [Windows 빌드와 제한](docs/WINDOWS.md)

## 다운로드

[Sua View 0.1.0](https://github.com/izowooi/crispy-app/releases/tag/sua-view-v0.1.0): macOS Apple Silicon(arm64), macOS 12 이상. ZIP을 풀고 앱을 Applications에 복사합니다. ad-hoc 서명만 포함하며 Apple 공증은 없습니다. Windows 바이너리는 별도 검증 후 배포합니다.

## 검증

```sh
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo fmt --check
```

[MIT 라이선스](../LICENSE).
