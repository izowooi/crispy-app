# Crispy App

로컬에서 사용하는 데스크톱 앱 모노레포입니다. 각 앱은 독립적으로 빌드합니다.

| 앱 | 설명 | 버전 |
| --- | --- | --- |
| [DiskLens](disk-lens/) | 무엇이 용량을 차지하는지 그림으로 보여 주는 macOS·Windows 앱 | 1.0.3 |
| [DiskZ — DiskLens 의 Mac App Store 판](disk-lens-appstore/) | DiskLens 의 App Store 판(샌드박스 빌드)과 제출 자료 | 1.0.0 (제출 전) |
| [Sua View](sua-view/) | 로컬 이미지·만화 압축 파일 뷰어 | 0.1.0 |
| [Obsidian Disk](obsidian-disk/) | CrystalDiskMark 기본 테스트를 Rust로 재구현한 Windows 디스크 벤치마크 | 0.1.0 (개발) |
| [DeepZ](deepz/) | 글을 선택하고 ⌘C(Ctrl+C)를 두 번 누르면 커서 옆에 번역이 뜨는 macOS·Windows 앱 | 1.0.1 |

[Releases](https://github.com/izowooi/crispy-app/releases)에서 macOS Apple Silicon 및 Windows x64 빌드를 받을 수 있습니다. Windows ZIP을 모두 압축 해제한 뒤 실행하세요. SuaView는 우클릭 메뉴의 **탐색기 F3 연결 켜기**로 폴더·ZIP 바로 열기를 활성화합니다. macOS Finder에서는 **⇧⌘J**를 사용합니다.

macOS 앱은 ad-hoc 서명되며 Developer ID 서명·Apple 공증을 포함하지 않습니다. macOS 보안 정책에 따라 실행이 제한될 수 있습니다. 소스에서 직접 빌드할 수도 있습니다.

## 라이선스

[MIT](LICENSE). 의존성은 각 패키지의 라이선스를 따릅니다.
