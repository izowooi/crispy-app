# Obsidian Disk

CrystalDiskMark의 기본 측정 흐름을 Rust로 다시 구현한 작은 디스크 벤치마크입니다. 현재 Windows x64용이며 macOS 측정 엔진은 후속 작업입니다. 설치나 관리자 권한 없이 단일 실행 파일로 동작합니다.

## 사용

`obsidian-disk.exe`를 실행하고 **Folder…**로 측정할 드라이브의 폴더를 고릅니다. 반복 횟수, 테스트 파일 크기, 회당 시간을 정한 뒤 **All**을 누릅니다. 각 행 버튼은 해당 항목만 측정하며 **Stop**은 진행 중인 작업을 중지합니다. MB/s, IOPS, µs 표시 전환과 **Copy results**를 지원합니다. 측정 설정이나 폴더를 바꾸면 이전 결과를 지웁니다.

| 항목 | 블록 크기 | 큐 깊이 | 스레드 |
| --- | --- | --- | --- |
| SEQ1M Q8T1 | 1 MiB | 8 | 1 |
| SEQ1M Q1T1 | 1 MiB | 1 | 1 |
| RND4K Q32T1 | 4 KiB | 32 | 1 |
| RND4K Q1T1 | 4 KiB | 1 | 1 |

기본 설정은 1 GiB, 3회, 회당 5초입니다. 각 읽기·쓰기 항목마다 같은 길이의 워밍업 1회를 제외하고 가장 빠른 측정 회차의 결과를 표시합니다. 기본 전체 측정은 파일 준비를 제외하고 약 160초입니다. 빠른 확인에는 64 MiB / 1회 / 1초를 사용하세요.

선택한 폴더에 새 임시 파일만 생성하고 전체를 난수 패턴으로 채운 뒤 측정합니다. 기존 파일이나 원시 디스크를 덮어쓰지 않습니다. 파일은 정상 완료·중지·앱 종료 시 닫히면서 Windows에서 삭제됩니다. 테스트 크기 외에 최소 64 MiB의 여유 공간이 필요합니다. 쓰기 테스트는 실제 디스크 쓰기를 발생시킵니다.

## 측정 방식

- Rust 엔진에서 Win32 overlapped I/O를 직접 사용합니다. DiskSpd나 C++ 실행 파일을 호출하지 않습니다.
- OS 파일 캐시를 우회하고 write-through를 요청합니다. 버퍼는 64 KiB 정렬, 요청 크기·오프셋은 4 KiB 배수입니다. 이를 지원하지 않는 파일 시스템·드라이브는 오류를 표시할 수 있습니다.
- 완료된 요청부터 처리하고 각 슬롯을 재사용해 큐 깊이를 유지합니다. UI와 측정은 별도 스레드에서 동작합니다.
- MB/s는 1,000,000 bytes/s, IOPS는 초당 완료 요청 수입니다. µs는 제출부터 앱이 완료를 관측할 때까지의 평균 시간입니다. 장치 펌웨어 내부 지연 시간이 아니며, 운영체제·완료 관측 비용을 포함합니다.
- 종료 시 남은 I/O를 모두 회수하고, 그 시간과 완료 바이트도 처리량에 포함합니다. 읽기·쓰기 모두 실제 데이터 파일을 대상으로 측정합니다.

원본의 `DiskBench.cpp`와 `DiskMarkDlg.cpp`에서 기본 4개 워크로드, 워밍업 제외, 반복 최고값 흐름을 참고했습니다. 자체 Rust 엔진이므로 CrystalDiskMark와 수치가 정확히 같음을 보장하지 않습니다. 테마, 스킨, 다국어 팩, 사용자 지정 큐·스레드, 혼합 R/W 및 추가 프로파일은 현재 구현에 포함되지 않습니다.

## Windows 빌드

Rust stable MSVC와 Visual Studio C++ Build Tools / Windows SDK가 필요합니다.

```powershell
cd obsidian-disk
cargo build --release --locked
cargo test --locked
cargo clippy --all-targets --locked -- -D warnings
```

실행 파일은 `target/release/obsidian-disk.exe`입니다. MSVC 런타임은 정적으로 링크합니다. 로컬 ZIP 패키지는 `./scripts/package-windows.ps1`로 만듭니다(스크립트 실행이 제한된 환경에서는 `powershell -NoProfile -ExecutionPolicy Bypass -File ./scripts/package-windows.ps1`). 패키징은 x64 타깃을 명시하며 GitHub Release나 태그를 생성하지 않습니다.

## 구조와 후속 macOS 작업

`src/main.rs`는 egui UI, 백그라운드 실행, 결과 표시를 담당하고 `src/engine.rs`는 워크로드와 Windows I/O를 담당합니다. macOS 지원 시 같은 워크로드·Score 인터페이스에 플랫폼 엔진을 추가할 수 있습니다. 현재 macOS 빌드는 지원하지 않습니다.

라이선스: [MIT](../LICENSE). 원본: [CrystalDiskMark](https://github.com/hiyohiyo/CrystalDiskMark), hiyohiyo, MIT License. 이 앱은 독립적인 Rust 재구현이며 원본의 이름·스킨·이미지나 DiskSpd 바이너리를 배포하지 않습니다.
