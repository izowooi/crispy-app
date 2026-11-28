# DeepZ 개인정보 처리방침

최종 수정: 2026-11-28

DeepZ 는 선택한 글을 번역해 주는 앱입니다. 이 문서는 직접 다운로드판과 Mac App Store 판 모두에 적용됩니다.

## 앱이 보내는 것

- **번역할 글**: 복사 단축키를 두 번 눌렀을 때, 그때 선택한 글만 번역을 위해 전송합니다. 평소에 복사한 내용이나 키 입력은 저장하지도 보내지도 않습니다.
  - 본인의 API 키를 넣은 경우: 그 키의 AI 서비스(Anthropic, OpenAI, Google)로 바로 보냅니다. 다른 서버를 거치지 않습니다.
  - 키 없이 무료 번역을 쓰는 경우(Mac App Store 판): DeepZ 번역 서버(Cloudflare 에서 운영)를 거쳐 AI 서비스(Anthropic, 장애 때는 OpenAI 또는 Google)로 보냅니다. 서버는 글을 전달만 하고 저장하거나 기록하지 않습니다.
- **무료 번역 횟수를 세기 위한 값**: 앱이 처음 실행될 때 만든 임의의 설치 번호와 접속 IP 주소별로 "오늘 쓴 횟수"만 서버에 남깁니다. 다음 날 초기화됩니다. 이 값으로 사용자가 누구인지 알 수 없고, 다른 용도로 쓰지 않습니다.

## 앱이 보내지 않는 것

- 이름, 이메일, 계정 정보 (가입이나 로그인이 없습니다)
- 번역 기록 (어디에도 저장하지 않습니다)
- 사용 통계, 광고 식별자, 위치
- 비밀번호 관리자가 숨김으로 표시한 클립보드 항목 (읽지 않습니다)

## 내 컴퓨터에 저장되는 것

- API 키: Mac App Store 판은 macOS 키체인에, 직접 다운로드판은 본인만 읽을 수 있는 파일에 저장합니다. 키 설정 창에서 언제든 바꾸거나 지울 수 있습니다.
- 설치 번호와 오늘 쓴 무료 번역 횟수.

앱을 지우면 함께 지울 수 있습니다. 방법은 [README](README.md#7-업데이트와-삭제)에 있습니다.

## AI 서비스의 처리

전송된 글은 각 AI 서비스의 약관과 개인정보 처리방침에 따라 처리됩니다.

- Anthropic: https://www.anthropic.com/legal/privacy
- OpenAI: https://openai.com/policies/privacy-policy
- Google: https://policies.google.com/privacy

민감한 내용은 번역하지 않는 것을 권합니다.

## 문의

https://github.com/izowooi/crispy-app/issues
