# Sua View

macOS와 Windows용 로컬 만화책 뷰어. Windows 빌드·실행·지원 형식은 [WINDOWS.md](WINDOWS.md)를 참조한다. 아래는 macOS 버전 설명이다. Rust가 압축 파일과 페이지 인덱스를 관리하고, 얇은 Swift/AppKit 화면에서 macOS ImageIO로 이미지를 디코딩한다. WebView, Electron, 서버, 상주 런처는 사용하지 않는다.

## 빌드와 실행

두 운영체제에서 공통으로 `cargo run --locked --bin sua-view-build`를 실행하면 현재 OS용 앱을 만든다. 소스를 바꾸거나 플랫폼 define을 수동 지정할 필요가 없다. 공통 로직과 OS별 어댑터의 경계는 [PLATFORMS.md](PLATFORMS.md)를 참조한다.

Rust stable과 Xcode Command Line Tools가 필요하다. Apple Silicon에서 빌드하면 arm64 앱이, Intel에서 빌드하면 x86_64 앱이 만들어진다.

```sh
./scripts/build-app.sh
open 'dist/Sua View.app'
./scripts/open-selected.sh '/path/to/book.cbz'
```

`dist/Sua View.app`은 로컬 ad-hoc 서명된 실행 가능한 앱이다. 다른 Mac에 배포하기 위한 Developer ID 서명과 공증은 포함하지 않는다. 빌드 산출물과 레퍼런스는 Git에 넣지 않는다.

## Finder에서 단축키로 열기

```sh
./scripts/install-app.sh
```

`~/Applications/Sua View.app`에 설치하고 macOS에 파일 열기와 Finder 서비스를 등록한다. 기존 설치본은 날짜가 붙은 `.app.previous-…` 폴더로 보존한다.

Finder에서 만화책 파일이나 폴더를 선택하고 **⇧⌘J**를 누른다. 서비스 이름은 **Sua View로 열기**다. 앱이 종료되어 있어도 macOS가 실행하고 선택한 경로를 전달한다. 서비스가 아직 활성화되지 않았다면 **시스템 설정 → 키보드 → 키보드 단축키 → 서비스 → Sua View로 열기**에서 활성화한다. 동일한 화면에서 F3 등 원하는 키로 바꿀 수 있다. F3의 Mission Control 할당과 충돌하면 macOS에서 해당 할당을 먼저 변경해야 한다.

이 서비스는 Apple의 [NSServices와 NSKeyEquivalent](https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/SysServices/Articles/properties.html) 규약을 사용한다. 사용자 전역 키 설정이나 기존 파일 기본 연결을 강제로 바꾸지 않는다.

Raycast/Alfred/Shortcuts 등에서 선택한 파일 경로를 인수로 전달할 때는 `scripts/open-selected.sh`를 실행해도 된다. 경로에 공백이 있어도 개별 인수로 넘기면 된다.

## 읽기

파일을 창에 끌어놓거나 `⌘O`로 연다. ZIP/CBZ는 압축 해제 폴더를 만들지 않고 읽는다. 이미지 파일을 열면 같은 폴더의 이미지도 숫자 순서로 탐색한다. 폴더를 열면 하위 폴더도 포함하며 숨김 파일·macOS 메타데이터·심볼릭 링크는 제외한다. 읽던 위치는 최근 50권까지 로컬에 기억한다.

| 조작 | 단축키 |
| --- | --- |
| 이전 / 다음 | `←` / `→` (읽기 방향 적용) |
| 다음 / 이전 | `Space` / `⇧Space`, `PageDown` / `PageUp` |
| 처음 / 마지막 | `Home` / `End` |
| 한 쪽 / 두 쪽 | `D` |
| 좌→우 / 우→좌 | `R` |
| 너비 / 화면 맞춤 | `W` / `0` |
| 확대 / 축소 | `+` / `−`, 트랙패드 핀치 |
| 페이지 번호로 이동 | `G` 또는 하단 쪽수 클릭 |
| 전체 화면 | `F`, `⌃⌘F`, 더블 클릭 |
| 전체 화면 종료 | `Esc` |
| 단축키 도움말 | `?` |

화면 좌우 영역 클릭으로도 넘길 수 있다. 확대했을 때는 드래그·휠로 이동한다. 하단 조작부는 잠시 후 숨겨지고 마우스를 움직이면 나타난다. **보기** 메뉴 또는 하단 `…`에서 표지 한 장, 읽기 방향 등을 바꾼다. 가로로 긴 이미지는 두 쪽 모드에서도 한 장으로 표시한다. 한글 입력 상태에서도 독서용 문자 단축키는 동일한 물리 키로 작동한다.

## 지원 범위와 메모리

- ZIP/CBZ: Stored와 Deflate 압축을 직접 인덱싱해 임의 페이지를 읽는다. 확장자가 CBR/7z여도 내용이 ZIP이면 같은 빠른 경로를 쓴다.
- RAR/CBR, 7z/CB7, TAR/CBT: macOS 기본 libarchive로 읽는다. solid 압축이나 뒤로 탐색은 앞부분을 다시 풀어야 할 수 있어 ZIP과 속도가 다르다.
- JPG, PNG, WebP, GIF, BMP, TIFF, HEIC, AVIF 등은 OS ImageIO 지원에 따른다. 애니메이션은 첫 프레임만 표시한다. PDF, 암호화 압축, 분할 압축, 서재·온라인 기능은 제공하지 않는다.
- UTF-8 또는 유효한 Unicode Path 추가 필드가 있는 ZIP 이름은 정상 표시한다. 이 정보가 없는 오래된 CP949 ZIP 이름은 기본 CP437 문자 인코딩으로 표시되어 한글이 깨질 수 있지만 페이지 자체는 읽을 수 있다.
- 디코딩 캐시는 192MiB, 한 장의 디코딩 결과는 최대 80MiB다. 현재 화면과 근처 페이지만 선읽기하고, 확대 시 필요한 해상도로 다시 디코딩한다. 실제 프로세스 메모리는 화면·압축 버퍼·ImageIO 등을 포함하므로 캐시 크기와 다르다.

## 검증

```sh
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
zsh -n scripts/*.sh
```

합성 페이지를 만들어 실제 앱을 검증할 수 있다. Python Pillow가 필요하며 생성물은 `local/fixtures/`에만 저장된다.

```sh
python3 scripts/make-fixtures.py --large
target/release/sua-view-engine inspect local/fixtures/sample.cbz
target/release/sua-view-engine benchmark local/fixtures/large.cbz 200
python3 scripts/smoke-app.py local/fixtures/large.cbz
python3 scripts/smoke-app.py local/fixtures/large.cbz --stress --runs 1
```

`smoke-app.py`는 실제 앱을 실행해 첫 쪽→다음→이전→두 쪽→방향 전환→맞춤을 검증하고 로컬에 JSON 증거를 저장한다. `--stress`는 전체 페이지를 단면·양면으로 탐색하면서 연속 입력, 가로 페이지 경계, 이동 후 뒤로 가기의 누락·중복을 검사한다. 앱의 `SUA_VIEW_METRICS=/path/file.jsonl`은 선택적으로 타이밍을 기록하며, 기본 실행에는 로그·네트워크 통신이 없다. `SUA_VIEW_TESTING=1`은 합성 시험이 읽던 위치와 설정을 바꾸지 않도록 한다.

원본 만화책이나 계정, 네트워크 연결 없이 검증한다.
