# Mac App Store 제출 자료

App Store Connect 에 앱을 만들 때 그대로 붙여 넣는 문구와 스크린샷입니다.

## 앱 정보

| 항목 | 값 |
|---|---|
| 플랫폼 | macOS |
| 이름 | `DiskZ` (아래 "이름" 참고) |
| 기본 언어 | 한국어 |
| 번들 ID | `com.zowoo.app.disklens` |
| SKU | `diskz-mac` |
| 기본 카테고리 | 유틸리티 |
| 보조 카테고리 | 생산성 |
| 가격 | 무료 (0원) |
| 연령 등급 | 설문 전부 "없음" → 4+ |
| 저작권 | `2026 <개발자 계정의 이름>` |
| 지원 URL | `https://github.com/izowooi/crispy-app/tree/main/disk-lens#readme` |
| 개인정보 처리방침 URL | `https://github.com/izowooi/crispy-app/blob/main/disk-lens/PRIVACY.md` |
| 앱이 수집하는 개인정보 | "데이터를 수집하지 않음" |
| 수출 규정(암호화) | 암호화를 쓰지 않음 → Info.plist 에 `ITSAppUsesNonExemptEncryption = NO` |

### 이름

`DiskLens` 는 App Store 에 이미 있는 이름이라 **`DiskZ`** 로 등록했습니다. 앱 창 제목, 앱 파일 이름, pkg 이름도 DiskZ 입니다. 번들 ID 는 그대로 `com.zowoo.app.disklens` 입니다(번들 ID 는 사용자에게 보이지 않습니다).

### 언어

앱은 11개 언어로 표시됩니다: English, 한국어, 简体中文, 日本語, Deutsch, Français, Italiano, Español, العربية, Polski, Русский.
App Store 는 앱 안의 언어 폴더(`*.lproj`)를 보고 "언어" 항목을 자동으로 채웁니다. 스토어 페이지 문구는 한국어·영어를 아래에, 나머지 9개 언어를 [`localizations.md`](localizations.md) 에 두었습니다. 스크린샷과 영상은 한국어·영어만 있고, 다른 언어에는 올리지 않으면 기본 언어의 것이 보입니다.

## 한국어

**이름** (30자 이내)

```
DiskZ
```

**부제** (30자 이내)

```
무엇이 용량을 차지하는지 한눈에
```

**홍보 문구** (170자 이내, 심사 없이 언제든 바꿀 수 있음)

```
디스크가 꽉 찼는데 범인을 모르겠다면. 폴더를 고르면 무엇이 용량을 차지하는지 그림으로 보여 줍니다. 큰 것은 크게, 작은 것은 작게.
```

**설명**

```
디스크가 꽉 찼는데 무엇 때문인지 모르겠다면 DiskZ 로 확인하세요.

폴더나 디스크를 고르면 그 안의 내용을 크기에 비례한 그림으로 보여 줍니다. 큰 것은 크게, 작은 것은 작게 그려지므로 용량을 가장 많이 차지하는 폴더와 파일이 바로 눈에 들어옵니다.

이렇게 씁니다
• 볼 폴더나 디스크를 고릅니다.
• 궁금한 칸을 누르면 그 폴더 안으로 들어갑니다.
• 이름으로 항목을 찾고, 정확한 크기와 위치를 확인합니다.
• Finder 에서 바로 열어 직접 정리합니다.

두 가지 그림
• 트리맵: 네모 칸의 넓이가 곧 용량입니다.
• 방사형 차트: 안쪽 고리가 상위 폴더, 바깥 고리가 그 안의 내용입니다.

안심하고 쓰세요
• 읽기만 합니다. 파일을 지우거나 옮기거나 고치지 않습니다.
• 인터넷에 연결하지 않습니다. 파일 이름도 크기도 밖으로 나가지 않습니다.
• 계정, 광고, 추적이 없습니다.

한국어, 영어를 비롯해 11개 언어를 지원합니다.
```

**키워드** (100자 이내, 쉼표로 구분, 이름에 든 단어는 넣지 않음)

```
디스크,용량,저장공간,분석,정리,폴더,크기,트리맵,하드,공간,파일,시각화,disk,space,storage
```

**이 버전의 새로운 기능** (첫 버전은 입력란이 없음)

```
첫 출시입니다.
```

## English (U.S.)

**Name**

```
DiskZ
```

**Subtitle** (30 characters max)

```
See what fills your disk
```

**Promotional Text**

```
Disk full and no idea why? Pick a folder and DiskZ draws what is taking the space. Big things look big, small things look small.
```

**Description**

```
Disk full and no idea why? DiskZ shows you.

Pick a folder or a disk and DiskZ draws its contents in proportion to their size. Big things look big and small things look small, so the folders and files using the most space stand out at once.

How it works
• Choose a folder or a disk.
• Click any block to go inside that folder.
• Find items by name and check their exact size and location.
• Reveal them in Finder and tidy up yourself.

Two charts
• Treemap: the area of each block is its size.
• Sunburst: inner rings are parent folders, outer rings are what they contain.

Safe by design
• Read-only. It never deletes, moves or changes your files.
• No network access. File names and sizes never leave your Mac.
• No accounts, no ads, no tracking.

Available in 11 languages, including English, Korean, Chinese, Japanese, German, French, Italian, Spanish, Arabic, Polish and Russian.
```

**Keywords**

```
disk,space,storage,usage,analyzer,treemap,sunburst,folder,size,cleanup,files,visualize,drive,full
```

## 심사 메모 (App Review Information → Notes)

로그인이 없으므로 데모 계정은 "로그인 필요" 를 끕니다. 메모는 영어로 씁니다.

```
DiskZ is a read-only disk usage visualizer. It has no account, no network access and no in-app purchases.

How to test:
1. Launch the app and click "Choose folder…", then pick any folder in the open panel. ("Choose disk…" opens the same panel at the top of the disk; click Open to scan the whole disk.)
2. The app scans the folder and shows a treemap. Click a block to go into that folder.
3. Use the "…" menu → View to switch to the sunburst chart.

The app is sandboxed and uses only com.apple.security.files.user-selected.read-only. It reads file sizes inside the folder the user selects in the open panel and never modifies or deletes files. It does not ask for Full Disk Access.
```

## 미리보기 영상 (App Preview)

한국어 현지화에 `preview/ko-preview.mp4`, 영어 현지화에 `preview/en-preview.mp4` 를 올립니다. 스토어에서 스크린샷보다 앞, 첫 화면에 자동 재생됩니다.

- 1920 × 1080, 30fps, H.264, 27초(허용 15~30초), 무음 스테레오 AAC.
- 내용: 트리맵 → 폴더 안으로 두 번 → 뒤로 → 파일 선택 → 방사형 차트로 전환 → 고리를 눌러 안으로 → 가운데를 눌러 위로.
- App Store 판 앱을 샌드박스에서 실행해 창을 녹화한 것입니다. 원본은 `preview/raw-window-ko.mov`, `raw-window-en.mov`, 다시 만들 때는 `./store/make-preview.sh`.
- 올린 뒤 App Store Connect 에서 **포스터 프레임**(재생 전에 보이는 정지 화면)을 고를 수 있습니다. 트리맵이 꽉 찬 첫 장면을 권합니다.
- 처리에 최대 24시간이 걸릴 수 있고, 영상도 심사 대상입니다.

## 스크린샷

`screenshots/` 의 `ko-1` ~ `ko-5` 를 한국어에, `en-1` ~ `en-5` 를 영어에 번호 순서대로 올립니다. 2560 × 1600, 알파 없는 PNG 입니다 (Mac 허용 규격: 1280×800, 1440×900, 2560×1600, 2880×1800 중 하나, 1~10장).

| 파일 | 한국어 문구 | 영어 문구 |
|---|---|---|
| `*-1-treemap.png` | 무엇이 용량을 차지하는지 한눈에 | See what fills your disk |
| `*-2-folder.png` | 눌러서 폴더 안으로 | Click to go inside |
| `*-3-sunburst.png` | 방사형 차트로도 | Or view it as a sunburst |
| `*-4-details.png` | 큰 파일이 어디 있는지 바로 | Find the big files fast |
| `*-5-welcome.png` | 직접 고른 폴더만, 읽기만 | Only what you choose, read-only |

- App Store 판 앱의 실제 화면을 Retina 로 찍은 것입니다(`captures/ko`, `captures/en`). 개인 폴더가 나오지 않도록 가짜 파일로 만든 `내 자료` / `My Files` 폴더를 스캔했습니다.
- 화면이 바뀌면 `captures/` 의 그림을 바꾸고 `python3 store/make-screenshots.py` 를 실행합니다.
