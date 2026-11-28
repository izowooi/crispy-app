# DeepZ 무료 번역 서버

API 키를 넣지 않은 DeepZ 사용자의 번역을 하루 정해진 횟수만큼 대신 처리하는 Cloudflare Worker 입니다.
키를 하나라도 넣은 사용자는 이 서버를 거치지 않습니다.

- 주소: `https://deepz-api.izowooi.workers.dev/v1/translate` (POST)
- 앱이 보낸 글을 AI 서비스(Claude, 실패하면 OpenAI)에 넘기고, 돌아오는 스트림을 그대로 흘려보냅니다.
- 번역할 글은 저장하거나 기록하지 않습니다. 저장하는 것은 설치 번호·IP 별 "오늘 쓴 횟수"뿐입니다.

## 제한

`wrangler.jsonc` 의 `vars` 에서 바꾸고 다시 배포합니다.

| 값 | 기본 | 뜻 |
|---|---|---|
| `DAILY_LIMIT` | 10 | 설치 하나가 하루에 쓸 수 있는 횟수 |
| `IP_DAILY_LIMIT` | 40 | 같은 IP 에서 하루에 쓸 수 있는 횟수 (설치 번호를 바꿔 가며 쓰는 것을 막음) |
| `GLOBAL_DAILY_LIMIT` | 3000 | 서버 전체의 하루 횟수. 넘으면 그날은 모두 거절 (요금 상한) |
| `MAX_CHARS` | 5000 | 한 번에 번역하는 글자 수 |
| `MODEL` | `claude-haiku-5-5` | 번역 모델 |
| `OPENAI_MODEL` | `gpt-6.2-sol` | Claude 가 실패했을 때 쓰는 모델 |

하루는 한국 시간 자정에 바뀝니다.

## 남이 쓰지 못하게 하는 장치

- 서버가 프롬프트를 직접 만듭니다. 받는 것은 번역할 글과 언어(한국어/영어)뿐이라 번역 말고 다른 용도로 쓸 수 없습니다.
- 요청마다 서명(HMAC-SHA256)과 시각이 붙어야 합니다. 서명용 값은 저장소에 없고 배포한 앱 안에만 들어 있습니다.
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
npx wrangler secret put APP_SECRET          # 앱 빌드에 쓴 DEEPZ_APP_SECRET 과 같은 값
npx wrangler tail           # 실시간 로그 (글 내용은 나오지 않음)
```
