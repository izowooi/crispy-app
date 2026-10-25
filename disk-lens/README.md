# Disk Lens

Rust와 egui로 만든 macOS·Windows 디스크 용량 시각화 앱입니다. Treemap 등 차트로 폴더별 사용량을 살펴보고 한국어·영어 UI를 전환할 수 있습니다. 스캔은 읽기 전용입니다.

## macOS 빌드

Rust stable(edition 2024 지원)과 Xcode Command Line Tools, macOS 12 이상이 필요합니다.

```sh
cd disk-lens
cargo run --locked --release
# 배포용 앱
./scripts/bundle.sh
open 'dist/Disk Lens.app'
# GUI 없이 스캔
cargo run --locked --release -- --scan /path/to/folder
```

실행 후 위치를 선택하고 스캔을 시작합니다. 보호된 폴더는 시스템 설정에서 전체 디스크 접근 권한을 허용해야 합니다. APFS 공유 블록·스냅샷·접근 불가 파일 때문에 합계가 시스템 저장 공간 표시와 다를 수 있습니다.

## 다운로드

[Disk Lens 1.0.0](https://github.com/izowooi/crispy-app/releases/tag/disk-lens-v1.0.0): macOS Apple Silicon(arm64), macOS 12 이상. ZIP을 풀고 앱을 Applications에 복사합니다. ad-hoc 서명만 포함하며 Apple 공증은 없습니다.

Windows 소스와 패키징 방법은 [WINDOWS.md](WINDOWS.md)를 참고하세요. 첫 공개 릴리스에는 Windows 바이너리가 없습니다.

## 검증

```sh
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo fmt --check
```

[MIT 라이선스](../LICENSE).
