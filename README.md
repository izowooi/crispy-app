# Crispy App

로컬에서 사용하는 데스크톱 앱 모노레포입니다. 각 앱은 독립적으로 빌드합니다.

| 앱 | 설명 | 버전 |
| --- | --- | --- |
| [Disk Lens](disk-lens/) | 디스크 사용량을 시각화하는 Rust 앱 | 1.0.0 |
| [Sua View](sua-view/) | 로컬 이미지·만화 압축 파일 뷰어 | 0.1.0 |

[Releases](https://github.com/izowooi/crispy-app/releases)에서 macOS Apple Silicon 빌드를 받을 수 있습니다. Windows 소스와 빌드 스크립트는 포함되며 Windows 바이너리는 별도 검증 후 배포합니다.

macOS 앱은 ad-hoc 서명되며 Developer ID 서명·Apple 공증을 포함하지 않습니다. macOS 보안 정책에 따라 실행이 제한될 수 있습니다. 소스에서 직접 빌드할 수도 있습니다.

## 라이선스

[MIT](LICENSE). 의존성은 각 패키지의 라이선스를 따릅니다.
