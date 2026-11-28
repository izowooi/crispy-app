# DiskZ — DiskLens 의 Mac App Store 판

[DiskLens](../disk-lens/) 를 Mac App Store 에 올리기 위한 샌드박스 빌드입니다. `DiskLens` 라는 이름이 App Store 에 이미 있어 스토어에서는 **DiskZ** 라는 이름을 씁니다. 직접 다운로드판의 코드를 그대로 가져와 샌드박스에서 쓸 수 없는 기능만 뺐습니다. 두 판은 코드를 공유하지 않고 각각 고칩니다.

## 직접 다운로드판과 다른 점

| | 직접 다운로드판 (`disk-lens`) | App Store 판 (여기) |
|---|---|---|
| 배포 | GitHub 릴리스 (macOS, Windows) | Mac App Store (macOS 만) |
| 앱 이름 | DiskLens | DiskZ |
| 번들 ID | `local.jongwoo.disklens` | `com.zowoo.app.disklens` |
| 읽을 수 있는 곳 | 어디든 (보호된 폴더는 전체 디스크 접근 권한으로) | 사용자가 폴더 선택 창에서 직접 고른 곳만 |
| 디스크 전체 | "디스크 스캔" 을 누르면 바로 | "디스크 선택…" → 열린 창에서 **열기** |
| 전체 디스크 접근 권한 안내 | 있음 | 없음 (샌드박스에서는 쓸 수 없음) |
| 지난번 폴더 기억 | 있음 | 없음. 앱을 켤 때마다 다시 고릅니다 |
| 터미널 `--scan`, 시작 폴더 지정 | 있음 | 없음 (`--lang <언어 코드>` 만 남음) |
| 차트, 찾기, Finder 에서 보기, 언어 | 같음 | 같음 |
| 아이콘 | `assets/disk-lens.svg` | 같은 파일 |

## 내 Mac 에서 확인

```sh
./scripts/bundle.sh
open dist/DiskZ.app
```

임시 서명이지만 샌드박스는 켜져 있어 스토어에서 받은 것과 같은 제한으로 동작합니다. 설정은 `~/Library/Containers/com.zowoo.app.disklens/` 안에 저장됩니다.

## App Store 에 올리기

1. developer.apple.com 에서 App ID `com.zowoo.app.disklens` (macOS), 인증서 `Mac Installer Distribution`, 프로비저닝 프로파일(Mac App Store Connect)을 만듭니다. `Apple Distribution` 인증서는 iOS 와 함께 씁니다.
2. 서명해서 pkg 를 만듭니다. 인증서 이름은 `security find-identity -v` 로 확인합니다.

   ```sh
   SIGN_APP="Apple Distribution: 이름 (팀ID)" \
   SIGN_PKG="3rd Party Mac Developer Installer: 이름 (팀ID)" \
   PROFILE=~/Downloads/DiskZ.provisionprofile \
   BUILD=1 ./scripts/bundle.sh
   ```

3. `~/git/tools/mac-appstore-upload/upload.sh dist/DiskZ-<버전>.pkg` 로 올립니다(검증 후 업로드). 인증은 `~/git/.auth/testflight.env` 의 App Store Connect API 키를 씁니다. 이 스크립트의 원본이 `scripts/upload.sh` 이고, 고치면 공용 도구 폴더에 복사합니다. Transporter 앱에 끌어다 놓아도 됩니다. 다시 올릴 때는 `BUILD` 를 1씩 키웁니다.
4. App Store Connect 에 넣을 문구, 스크린샷, 미리보기 영상은 [`store/`](store/) 에 있습니다.

배포용 서명으로 만든 앱은 내 Mac 에서 바로 실행되지 않습니다(스토어를 거쳐야 실행됩니다). 동작 확인은 위의 임시 서명 빌드로 합니다.

프로파일은 Xcode → Settings → Accounts → Download Manual Profiles 로 받으면 `~/Library/Developer/Xcode/UserData/Provisioning Profiles/` 에 들어옵니다. 그중 Mac App Store 용(`disklens_release`)을 `PROFILE` 에 줍니다.

## 개발

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
```

Apple Silicon 전용, macOS 12 이상입니다.
