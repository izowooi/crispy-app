# DeepZ 무료 번역 서버

API 키를 넣지 않은 DeepZ 사용자의 번역을 하루 정해진 횟수만큼 대신 처리하는 Cloudflare Worker 입니다.
키를 하나라도 넣은 사용자는 이 서버를 거치지 않습니다.

- 주소: `https://deepz-api.zowoo.uk/v1/translate` (POST)
- Cloudflare Workers(엣지에서 도는 서버리스 함수)라 기동 지연이 거의 없습니다.
- 앱이 보낸 글을 AI 서비스(Claude → 실패하면 OpenAI → Gemini)에 넘기고, 돌아오는 스트림을 그대로 흘려보냅니다.
- 번역할 글은 저장하거나 기록하지 않습니다. 저장하는 것은 설치 번호·IP 별 "오늘 쓴 횟수"뿐입니다.

## 제한

`wrangler.jsonc` 의 `vars` 에서 바꾸고 다시 배포합니다.

| 값 | 기본 | 뜻 |
|---|---|---|
| `DAILY_LIMIT` | 10 | 설치 하나가 하루에 쓸 수 있는 횟수. 앱도 같은 횟수를 세고, 다 쓰면 요청을 보내지 않습니다 |
| `IP_DAILY_LIMIT` | 400 | 같은 IP 에서 하루에 쓸 수 있는 횟수 (설치 번호를 바꿔 가며 쓰는 것을 막음) |
| `GLOBAL_DAILY_LIMIT` | 3000 | 서버 전체의 하루 횟수. 넘으면 그날은 모두 거절 |
| `GLOBAL_MONTHLY_LIMIT` | 30000 | 서버 전체의 한 달 횟수. 넘으면 그 달은 모두 거절 (요금 상한) |
| `MAX_CHARS` | 5000 | 한 번에 번역하는 글자 수 |
| `PROVIDERS` | `claude,openai,gemini` | 쓰는 순서. 앞의 것이 실패하면 다음으로 넘어감 |
| `CLAUDE_MODEL` / `CLAUDE_EFFORT` | `claude-sonnet-5-5` / `medium` | |
| `OPENAI_MODEL` / `OPENAI_EFFORT` | `gpt-6.1-terra` / `low` | |
| `GEMINI_MODEL` | `gemini-4-hydrogen` | 추론을 끌 수 없어 첫 글자가 3초쯤 걸림. 홍콩 경유 요청은 Google 이 거절함 |

하루와 한 달은 한국 시간 기준으로 바뀝니다.

### 모델을 이렇게 정한 근거 (2026-11-28 측정, 글 16개)

| 모델 · effort | 첫 글자 (중앙값) | 끝까지 | 1회 요금 | 품질 (10점, 심사 모델 2개) |
|---|---|---|---|---|
| claude-sonnet-5-5 · medium | 0.81초 | 1.62초 | $0.0010 | 8.6 / 8.9 |
| claude-sonnet-5-5 · low | 0.78초 | 1.51초 | $0.0010 | 8.8 / 9.1 |
| gpt-6.1-terra · medium | 1.32초 | 1.88초 | $0.0004 | 8.4 / 9.3 |
| gpt-6.1-terra · low | 1.11초 | 1.54초 | $0.0003 | 8.4 / 9.1 |
| gemini-4-hydrogen · low | 2.68초 | 2.90초 | $0.0024 | 8.8 / 9.3 |

품질 점수 차이는 글마다의 편차(0.6~1.1점)보다 작아서, 이 표본으로는 세 모델과 두 effort 의 품질이 다르다고 할 수 없습니다.

## 남이 쓰지 못하게 하는 장치

- 서버가 프롬프트를 직접 만듭니다. 받는 것은 번역할 글과 언어(한국어/영어)뿐이라 번역 말고 다른 용도로 쓸 수 없습니다.
- 요청마다 서명(HMAC-SHA256)과 시각이 붙어야 합니다. 서명용 값은 저장소에 없습니다. 직접 빌드용과 Mac App Store 판용 두 개가 있고, 하나가 새면 그것만 바꿉니다. GitHub 배포 파일에는 넣지 않습니다.
- 웹 페이지에서 보낸 요청(`Origin` 헤더)은 받지 않습니다.
- 설치·IP·전체 단위의 하루 한도가 있습니다.

앱 실행 파일을 분석하면 서명용 값을 꺼낼 수 있으므로 완전한 차단은 아닙니다. 그 경우에도 IP·전체 한도 안에서만 쓸 수 있습니다.
값이 새면 새 값으로 바꾸고(`APP_SECRET` 과 `~/git/.auth/deepz.env`) 앱을 다시 배포합니다. 이전 버전 앱의 무료 번역은 그때부터 동작하지 않습니다.

## 배포

```sh
cd deepz/server
node --test                 # 규칙 테스트
npx wrangler deploy
npx wrangler secret put ANTHROPIC_API_KEY   # 필수
npx wrangler secret put OPENAI_API_KEY      # 선택 (Claude 실패 때 대체)
npx wrangler secret put GEMINI_API_KEY      # 선택
npx wrangler secret put APP_SECRET          # 앱 빌드에 쓴 DEEPZ_APP_SECRET 과 같은 값
npx wrangler secret put APP_SECRET_STORE    # Mac App Store 판 빌드에 쓴 DEEPZ_STORE_SECRET 과 같은 값
npx wrangler tail           # 실시간 로그 (글 내용은 나오지 않음)
```

키를 바꿀 때도 같은 명령입니다. Cloudflare 대시보드의 Workers & Pages → `deepz-api` → Settings → Variables and Secrets 에서 바꿔도 됩니다. 저장하면 바로 적용됩니다.
