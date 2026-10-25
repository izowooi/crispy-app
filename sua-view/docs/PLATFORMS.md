# 같은 소스에서 macOS·Windows 빌드하기

두 운영체제에서 소스를 수정하거나 `define`을 수동으로 바꿀 필요가 없다. 개발 도구가 설치된 각 운영체제에서 같은 명령을 실행한다.

```sh
cargo run --locked --bin sua-view-build
```

Rust 빌드 도우미의 `#[cfg(target_os = "macos")]`와 `#[cfg(target_os = "windows")]`가 해당 운영체제의 빌드 명령만 컴파일한다. macOS에서는 AppKit `.app`, Windows에서는 WPF `.exe`와 포터블 ZIP을 만든다. `-- --dry-run`을 붙이면 선택한 명령만 확인한다. 이 명령은 현재 운영체제용 앱을 만들며 다른 운영체제로 크로스 컴파일하는 기능은 아니다.

## 공통 코드와 플랫폼 코드의 경계

| 기능 | 관리 위치 | 수정 시 영향 |
| --- | --- | --- |
| 압축 인덱스·임의 읽기·폴더·자연 정렬·입력 제한 | `src/lib.rs`, `src/archive.rs`, `src/natural.rs` 등 | 양쪽 공통 엔진 |
| 가로 페이지 판정·표지/양면·다음/이전 펼침·이미지 확장자 판정 | `src/reading.rs`, `src/lib.rs` | 양쪽 UI가 같은 Rust C ABI 호출 |
| 플랫폼에 맞는 빌드 선택 | `src/bin/build-app.rs`의 `#[cfg(target_os = ...)]` | 공통 명령 자동 분기 |
| 시작 화면 색상·아이콘 모양·글자 크기·독서 조작부 크기와 숨김 시간 | `assets/ReaderTheme.json` | Mac 리소스·Windows 내장 리소스로 같은 설정 배포 |
| libarchive 연결 | `src/archive.rs`의 `#[cfg(unix)]`, `#[cfg(not(unix))]` | Unix 구현과 그 외 플랫폼 오류 처리 자동 선택 |
| 창·메뉴·키보드·OS 이미지 디코더·로컬 설정 저장 | `native/SuaView.swift`, `native/windows/SuaView.cs` | 해당 운영체제 전용 어댑터 |

공통 동작에는 불필요한 `cfg`를 붙이지 않는다. 같은 코드를 그대로 양쪽에서 컴파일하는 것이 중복을 줄인다. `cfg`는 다른 OS API나 실행 도구를 선택하는 경계에 사용한다. Swift와 C#은 다른 언어이므로 Rust의 `cfg`로 한 파일에 합칠 수 없고, AppKit과 WPF도 각 OS 전용이다. 화면 자체를 하나로 관리하려면 별도로 공통 UI 프레임워크로 이전해야 한다.

공통 Rust 함수를 추가하면 `include/sua_view.h`와 C#의 `Core` 선언을 맞춘다. C ABI의 불리언은 언어별 마샬링 차이를 피하도록 `uint8_t`/`byte`의 0·1로 전달한다. ABI 변경 후에는 코어와 해당 UI를 함께 다시 빌드한다.

Windows도 Mac과 같은 중앙 아이콘 시작 화면, 상시 도구 모음이 없는 캔버스, 마우스 움직임에 나타나는 하단 독서 패널과 옵션 메뉴를 사용한다. WPF 버튼·슬라이더·메뉴 템플릿은 `native/windows/ReaderStyles.xaml`, 창과 HUD 구성은 `ReaderChrome.cs`에 둔다. Windows 창 버튼과 OS 글꼴·재질의 차이는 각 플랫폼 관례를 따른다.

## 지원 범위와 검사

조건부 컴파일은 OS API의 선택을 해결하며 기능 차이를 자동으로 채우지는 않는다. Windows의 RAR/7z/TAR 미지원과 이미지 디코더·UI 기능 차이는 [WINDOWS.md](WINDOWS.md), macOS 기능은 [USAGE.md](USAGE.md)에 명시한다.

공개 저장소의 첫 릴리스는 macOS Apple Silicon에서 로컬 빌드·검증한다. Windows 빌드와 실제 창 조작 검증은 별도로 수행해야 한다.
