# DeepZ (Mac App Store 판)

[DeepZ](../deepz/) 의 Mac App Store 판입니다. 글을 선택하고 `⌘C` 를 두 번 누르면 커서 옆에 번역이 뜹니다.
직접 다운로드판의 macOS 코드를 그대로 가져와, App Store 의 샌드박스에서 쓸 수 없는 기능을 빼고 몇 가지를 바꿨습니다. 두 판은 코드를 공유하지 않습니다.

## 직접 다운로드판과 다른 점

| | 직접 다운로드판 | Mac App Store 판 |
|---|---|---|
| 키 없이 번역 | 안 됨 (API 키 필요) | 하루 10회 무료, 한 번에 5,000자 |
| API 키 저장 | 본인만 읽는 파일 | macOS 키체인 |
| `⌘C` 두 번 감지 | 클립보드 감지 + 정밀 감지(손쉬운 사용 권한) | 클립보드 감지만. 권한이 필요 없습니다 |
| 처음 쓸 때 | 바로 번역 | 글을 AI 서비스로 보낸다는 안내에 한 번 동의 |
| 모델 바꾸기 | 키 파일에서 가능 | 없음 |
| Windows | 있음 | 없음 |

## API 키 넣기

키가 없어도 하루 10회까지는 무료로 번역됩니다. 더 쓰려면 본인의 API 키를 넣습니다. 키를 넣으면 횟수 제한이 없고, 글이 DeepZ 서버를 거치지 않고 AI 서비스로 바로 갑니다. 요금은 쓴 만큼 해당 서비스에 냅니다.

1. 아래 셋 중 한 곳에서 키를 발급받습니다. 하나만 있으면 됩니다.

   | 서비스 | 키 발급 페이지 | 키의 모양 |
   |---|---|---|
   | Claude (Anthropic) | https://console.anthropic.com/settings/keys | `sk-ant-` 로 시작 |
   | OpenAI | https://platform.openai.com/api-keys | `sk-` 로 시작 |
   | Gemini (Google) | https://aistudio.google.com/apikey | `AIza` 로 시작 |

2. 메뉴 막대의 **Z 아이콘** → **API 키 설정…** 을 누릅니다.
3. 해당하는 칸에 키를 붙여 넣고(`⌘V`) **저장** 을 누릅니다.

키를 바꾸려면 새 키를 붙여 넣고, 지우려면 칸을 비우고 저장합니다. 키는 이 Mac 의 키체인에만 저장됩니다.
키를 여러 개 넣으면 Claude → OpenAI → Gemini 순서로 쓰고, 하나가 실패하면 다음 것으로 넘어갑니다.

## 빌드

Xcode Command Line Tools 가 필요합니다. Apple Silicon 전용, macOS 13 이상입니다.

```sh
./scripts/bundle.sh        # 임시 서명 + 샌드박스 → dist/DeepZ.app (내 Mac 에서 동작 확인용)
```

무료 번역 서버에 보내는 요청의 서명 값은 저장소에 없습니다. 빌드할 때 `DEEPZ_STORE_SECRET` (또는 `~/git/.auth/deepz.env`)에서 읽습니다. 이 값 없이 빌드한 앱은 무료 번역이 빠지고 API 키로만 동작합니다. 서버는 [`../deepz/server`](../deepz/server/) 에 있습니다.

## App Store 에 올리기

1. 배포용 인증서 두 개(`Apple Distribution`, `3rd Party Mac Developer Installer`)와 `com.izowooi.app.deepz` 의 Mac App Store 프로파일(`deepz_release`)이 필요합니다.
2. 서명한 pkg 를 만듭니다. 다시 올릴 때는 `BUILD` 를 1씩 키웁니다.

   ```sh
   SIGN_APP="Apple Distribution: 이름 (팀ID)" \
   SIGN_PKG="3rd Party Mac Developer Installer: 이름 (팀ID)" \
   PROFILE="$HOME/Library/Developer/Xcode/UserData/Provisioning Profiles/<deepz_release 파일>.provisionprofile" \
   BUILD=1 ./scripts/bundle.sh
   ```

3. `~/git/tools/mac-appstore-upload/upload.sh dist/DeepZ-<버전>.pkg` 로 올립니다.
4. App Store Connect 에 넣을 문구와 스크린샷은 [`store/`](store/) 에 있습니다.

배포용 서명으로 만든 앱은 내 Mac 에서 바로 실행되지 않습니다. 동작 확인은 임시 서명 빌드나 TestFlight 로 합니다.
