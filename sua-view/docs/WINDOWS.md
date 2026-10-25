# Sua View for Windows 0.1.0

Windows 10/11 x64용 로컬 만화책 뷰어입니다. ZIP 전체를 풀지 않고 기존 Rust 엔진으로 페이지를 읽으며 Windows WPF로 표시합니다. 네트워크나 서버를 사용하지 않습니다.

## 실행

`Sua-View-0.1.0-windows-x64.zip`을 새 폴더에 **모두 압축 해제**한 뒤 `SuaView.exe`를 실행하세요. 함께 들어 있는 `sua_view_core.dll`과 `SuaView.exe.config`를 같은 폴더에 유지하세요. 설치와 관리자 권한은 필요 없습니다. .NET Framework 4.8이 필요합니다(Windows 11에는 기본 포함). 코드 서명 인증서는 포함하지 않았으므로 다운로드한 앱 실행 시 Windows의 확인 화면이 나타날 수 있습니다.

`SuaView.exe "C:\만화\책.cbz"`로 파일·폴더 경로를 전달하거나 창에 끌어 놓을 수도 있습니다. 시작 화면의 `책 열기…`로 파일을, `…` 메뉴의 `이미지 폴더 열기…`로 폴더를 선택합니다.

시작 화면은 Mac과 같은 중앙 책 아이콘·제목·열기 버튼으로 구성합니다. 책을 읽을 때 마우스를 움직이면 하단 조작부가 나타나며, 약 2초 후 숨겨집니다. 폴더 열기·양면·읽기 방향·맞춤·전체 화면 등은 하단 `…` 또는 화면 우클릭 메뉴에서 선택합니다. 색상·아이콘 모양·글자 크기·조작부 크기는 Mac과 공유하는 `assets/ReaderTheme.json`에서 관리합니다. 창 최소화·최대화·닫기는 Windows 방식으로 유지합니다.

## 조작

| 키 | 기능 |
| --- | --- |
| Ctrl+O / Ctrl+Shift+O | 파일 / 폴더 열기 |
| ← / → | 읽기 방향에 따라 이전 / 다음 |
| Space / Shift+Space, PageDown / PageUp | 다음 / 이전 |
| Home / End | 처음 / 마지막 |
| D / R | 한 쪽·두 쪽 / 읽기 방향 |
| W / 0 | 너비 / 화면 맞춤 |
| + / −, Ctrl+휠 | 확대 / 축소 |
| 휠 / 드래그, Shift+휠 | 확대된 이미지 이동 / 가로 이동 |
| G | 페이지 번호로 이동 |
| F, F11, 더블 클릭 / Esc | 전체 화면 / 전체 화면 종료 |
| F1 / ? | 도움말 |

두 쪽 모드에서도 표지와 가로로 긴 페이지는 한 장으로 표시합니다. 읽던 위치는 `%LOCALAPPDATA%\Sua View\positions.txt`에 최대 50권까지 저장합니다.
화면 좌우 영역을 클릭하거나, 화면 맞춤 상태에서 휠을 움직여 페이지를 넘길 수도 있습니다.

## Windows 첫 버전의 지원 범위

- ZIP/CBZ(Stored/Deflate), 이미지 폴더·하위 폴더, 개별 이미지와 같은 폴더의 페이지를 자연 정렬합니다. 한글 경로를 지원합니다.
- JPG, PNG, GIF(첫 프레임), BMP, TIFF는 Windows 디코더로 표시합니다. 이미지는 한 장당 최대 2천만 픽셀이며 디코딩 캐시는 최대 두 장입니다. 전체 프로세스 메모리는 캐시보다 큽니다.
- **RAR/CBR, 7z/CB7, TAR/CBT 및 WebP/HEIC/AVIF는 이 Windows 버전에서 지원하지 않습니다.** 확장자가 CBR이어도 실제 내용이 ZIP이면 읽습니다. macOS 버전과 지원 범위가 다릅니다.
- PDF, 암호화·분할 압축, 온라인 서재는 지원하지 않습니다. 오래된 CP949 ZIP 파일명은 깨질 수 있습니다.
- Windows에는 macOS의 선읽기와 읽기 설정 복원이 아직 없습니다. 자동 숨김 조작부와 드래그 이동은 제공합니다.

## 소스 빌드

Rust stable(MSVC), Visual Studio C++ Build Tools, Windows .NET Framework 4.8이 필요합니다. PowerShell에서:

```powershell
cargo run --locked --bin sua-view-build
```

macOS에서도 같은 명령을 사용하며 OS별 빌드는 Rust 조건부 컴파일로 선택한다. 플랫폼별 수정 없이 유지하는 구조는 [PLATFORMS.md](PLATFORMS.md)를 참조한다. Windows 스크립트를 직접 실행하는 기존 방법도 유지한다.

Rust 테스트와 Clippy를 통과하면 `dist/Sua-View-windows-x64/SuaView.exe`, 배포 ZIP과 SHA-256을 생성합니다. 빌드 산출물은 Git에 넣지 않습니다. `sua-view-engine.exe`는 압축 인덱싱·원시 읽기 검사용 CLI입니다.

합성 테스트 이미지 생성에는 Python 3와 Pillow가 필요합니다. 다음 검사는 배포 ZIP을 한글 경로에 풀어 실제 DLL·이미지 디코더·WPF 창을 실행하고, 한 쪽·두 쪽·연속 이동·읽기 방향·가로 페이지·확대·전체 화면을 확인합니다. `local/windows-smoke-*`에 결과와 화면 PNG를 저장하며 읽던 위치는 바꾸지 않습니다.

```powershell
python scripts/make-fixtures.py
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/smoke-windows.ps1
```
