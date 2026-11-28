# Mac App Store 제출 자료

App Store Connect 에 그대로 붙여 넣는 문구와 스크린샷입니다. 첫 버전은 한국어만 올립니다.

## 앱 정보

| 항목 | 값 |
|---|---|
| 플랫폼 | macOS |
| 이름 | `DeepZ` |
| 기본 언어 | 한국어 |
| 번들 ID | `com.izowooi.app.deepz` |
| 기본 카테고리 | 생산성 |
| 보조 카테고리 | 유틸리티 |
| 가격 | 무료 (0원) |
| 저작권 | `2026 <개발자 계정의 이름>` |
| 지원 URL | `https://github.com/izowooi/crispy-app/tree/main/deepz-appstore#readme` |
| 개인정보 처리방침 URL | `https://github.com/izowooi/crispy-app/blob/main/deepz/PRIVACY.md` |
| 수출 규정(암호화) | 표준 HTTPS 와 인증용 서명만 사용 → 면제. Info.plist 에 `ITSAppUsesNonExemptEncryption = NO` 가 들어 있습니다 |

## 문구

**이름** (30자 이내)

```
DeepZ
```

**부제** (30자 이내)

```
⌘C 두 번으로 바로 번역
```

**홍보 문구** (170자 이내, 심사 없이 언제든 바꿀 수 있음)

```
글을 선택하고 ⌘C 를 두 번. 번역기 창을 열 필요 없이 커서 바로 옆에 한국어 번역이 뜹니다. 가입 없이 하루 10회 무료.
```

**설명**

```
읽다가 막히는 문장을 만나면, 선택하고 ⌘C 를 두 번 누르세요.
번역기 창을 열고 붙여 넣을 필요 없이, 커서 바로 옆에 번역이 뜹니다.

이렇게 씁니다
• 어떤 앱에서든 글을 마우스로 선택합니다.
• ⌘C 를 빠르게 두 번 누릅니다.
• 커서 옆 작은 창에 번역이 글자 단위로 바로 나타납니다.
• 복사 버튼으로 번역을 가져가고, Esc 나 바깥을 눌러 닫습니다.

알아서 방향을 정합니다
• 영어·일본어·중국어 등 어떤 언어든 한국어로 번역합니다.
• 한국어 글을 선택하면 영어로 번역합니다.
• 원문 언어를 고를 필요가 없습니다.

빠릅니다
• 첫 글자가 1초 안팎에 나오고, 번역이 만들어지는 대로 화면에 찍힙니다.
• 메뉴 막대에만 머무는 작은 앱이라 작업을 방해하지 않습니다.

가입 없이 무료로
• 계정도 로그인도 없습니다. 설치하면 바로 하루 10회까지 무료로 번역합니다(한 번에 5,000자).
• 더 많이 쓰려면 본인의 Claude, OpenAI, Gemini API 키를 넣으세요. 횟수 제한이 없어지고 한 번에 20,000자까지 번역합니다. 요금은 쓴 만큼 해당 서비스에 냅니다.

개인정보
• ⌘C 를 두 번 눌렀을 때 선택한 글만 번역을 위해 전송합니다. 평소에 복사한 내용은 보내지 않습니다.
• 번역 기록을 저장하지 않습니다.
• 본인의 API 키를 넣으면 글이 해당 AI 서비스로 바로 전송되고, 키는 이 Mac 의 키체인에만 저장됩니다.
• 비밀번호 관리자가 숨김으로 표시한 항목은 읽지 않습니다.
• 특별한 권한을 요구하지 않습니다.

번역에는 AI 서비스(Anthropic Claude 등)를 사용합니다. 인터넷 연결이 필요합니다.
```

**키워드** (쉼표로 구분, 100자 이내)

```
번역,번역기,영어,일본어,중국어,한영,영한,AI,단축키,메뉴바,클립보드,translate,translator
```

**이 버전의 새로운 기능** (첫 버전은 비워 둬도 됩니다)

```
첫 버전입니다.
```

## 스크린샷

[`screenshots/`](screenshots/) 의 5장을 번호 순서대로 올립니다. 모두 2560×1600, 알파 없는 PNG 입니다.

| 파일 | 문구 |
|---|---|
| `ko-1-en-ko.png` | 선택하고 ⌘C 두 번, 번역이 바로 옆에 |
| `ko-2-ja-ko.png` | 어떤 언어든 한국어로 |
| `ko-3-ko-en.png` | 한국어는 영어로 |
| `ko-4-menu.png` | 가입 없이 하루 10회 무료 |
| `ko-5-keys.png` | 내 API 키를 넣으면 제한 없이 |

- 앱 화면 원본은 [`captures/`](captures/) 에 있고, `python3 store/make-screenshots.py` 로 다시 합성합니다.
- 번역 장면의 바탕 문서 창("Notes")은 촬영용으로 만든 예시 창입니다. 팝업과 메뉴, 키 창은 실제 앱 화면입니다.
- 미리보기 영상은 만들지 않았습니다. 없어도 제출할 수 있습니다.

## 심사 메모 (App Review Information → Notes)

로그인 정보는 필요 없습니다("로그인 필요" 체크 해제). 아래를 그대로 붙여 넣습니다.

```
DeepZ is a menu bar app (no Dock icon, no main window). After launch, a "Z" icon appears in the menu bar.

How to test:
1. Open any app with text (Safari, Notes, TextEdit) and select a sentence with the mouse.
2. Press Command-C twice quickly (within 0.6 seconds).
3. The first time, a dialog explains that the selected text is sent to an AI service for translation. Click "동의하고 번역" (Agree and translate).
4. A small panel appears next to the cursor with the Korean translation. Korean text is translated into English.

No account or API key is needed: the app includes 10 free translations per day through our own server (https://deepz-api.zowoo.uk), which forwards the text to the AI provider and does not store it. Users may optionally enter their own Anthropic/OpenAI/Google API key (menu → "API 키 설정…"); keys are stored in the macOS Keychain and requests then go directly to that provider.

The app detects the double copy by watching the pasteboard change count; it reads the pasteboard only when the same text is copied twice in a row. It does not use Accessibility or Input Monitoring permissions. Entitlements: App Sandbox and outgoing network connections only.

The UI is in Korean only in this version.
```

## 설문 답변

**앱이 수집하는 개인정보**: 보수적으로 아래처럼 신고하는 것을 권합니다. 서버가 글을 저장하지 않으므로 "수집하지 않음" 으로 볼 여지도 있지만, 글이 기기 밖(제3자 AI 서비스)으로 나가기 때문입니다.

| 데이터 | 용도 | 사용자와 연결 | 추적 |
|---|---|---|---|
| 사용자 콘텐츠 → 기타 사용자 콘텐츠 (번역할 글) | 앱 기능 | 아니요 | 아니요 |
| 식별자 → 기기 ID (임의의 설치 번호) | 앱 기능 | 아니요 | 아니요 |

**연령 등급**: 설문 전부 "없음". 웹 탐색, 사용자 간 대화, 광고가 없습니다.

**콘텐츠 권한**: 제3자 콘텐츠를 포함하지 않습니다.

## 제출 순서

1. `scripts/bundle.sh` 로 서명한 pkg 를 만들어 올립니다([README](../README.md#app-store-에-올리기)).
2. 빌드 처리가 끝나면 TestFlight 로 내 Mac 에 설치해 번역이 되는지 확인합니다. 배포용 서명 빌드는 TestFlight 나 스토어를 거쳐야 실행됩니다.
3. 버전 페이지에 위 문구와 스크린샷을 넣고, 빌드를 고른 뒤 심사에 제출합니다.
