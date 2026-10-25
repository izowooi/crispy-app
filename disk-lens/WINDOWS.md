# Disk Lens for Windows

Windows 10/11 x64용 읽기 전용 디스크 용량 시각화 앱입니다. 트리맵·방사형 차트, 한국어·영어, 스캔 취소와 마지막 경로·차트 복원을 지원합니다.

## 실행

ZIP을 원하는 폴더에 풀고 `Disk Lens.exe`를 실행하세요. 시작할 때 자동으로 스캔하지 않습니다. 디스크 또는 폴더를 선택하고 스캔 버튼을 누르세요. 다른 드라이브는 폴더 선택 창에서 선택할 수 있습니다.

콘솔 요약은 PowerShell에서 다음과 같이 확인합니다.

```powershell
.\disk-lens-cli.exe --scan 'D:\Example'
```

설정은 `%LOCALAPPDATA%\Disk Lens\preferences`에 저장합니다. 실행 파일은 코드 서명되지 않았습니다. C 런타임은 정적으로 링크하여 별도 Visual C++ 런타임 설치가 필요하지 않습니다.

## 개발환경

[Rust 공식 설치 프로그램](https://rust-lang.org/tools/install/)과 Visual Studio 2022 Build Tools의 C++ 빌드 도구 및 Windows SDK가 필요합니다. PowerShell에서 설치할 수도 있습니다.

```powershell
winget install --id Microsoft.VisualStudio.2022.BuildTools --exact --override '--wait --passive --norestart --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended'
winget install --id Rustlang.Rustup --exact
```

설치 완료 후 터미널을 다시 열고 프로젝트 폴더에서 실행합니다.

```powershell
cargo test --locked
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo fmt --check
cargo run --locked
cargo run --locked --release --features console -- --scan 'D:\Example'
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/bundle-windows.ps1
```

마지막 명령은 `dist/Disk-Lens-1.0.0-Windows-x64.zip`과 SHA-256 파일을 만듭니다. GUI 실행 파일은 콘솔 창을 열지 않고, CLI 실행 파일은 표준 출력과 종료 코드를 제공합니다. 아이콘은 macOS와 같은 `assets/disk-lens.svg`에서 생성합니다. PowerShell 실행 정책은 해당 프로세스에만 적용하며 시스템 설정은 변경하지 않습니다.

## 집계 범위

Windows 파일 핸들에서 볼륨·파일 ID·하드링크 수와 실제 할당 크기를 조회합니다. 하드링크의 용량은 한 번만 집계합니다. 정션·심볼릭 링크 등 재분석 지점을 따라가지 않으며 클라우드 파일 내용은 읽지 않습니다. 다른 볼륨으로 넘어가지 않습니다. 접근할 수 없는 항목은 오류 수로 표시합니다.

보호된 파일, 파일 시스템 메타데이터, 대체 데이터 스트림, 스냅샷 및 공유 저장 공간 때문에 폴더 합계와 볼륨 사용량은 다를 수 있습니다. 전체 디스크 스캔은 사용자가 접근 가능한 범위만 보여 줍니다. 삭제·권한 변경·자동 관리자 승격 기능은 없습니다. macOS 전용 전체 디스크 접근 권한 안내는 Windows에서 표시하지 않습니다.
