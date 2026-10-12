# Web 서버와 원격 터미널 접속

조사일: 2026-10-08. 공식 온라인 가이드 및 0.45.0 WebCli/Attach 정의 기준. 실행/네트워크 공개/토큰 생성은 수행하지 않았으며 예제는 문서 기반이다.

## 1. 기본 동작

Web 서버는 기본적으로 꺼져 있다. 브라우저에서 기존 세션을 보거나 새 세션을 만들고 복원할 수 있다. 기본 주소는 `http://127.0.0.1:8082`다.

```sh
zellij web                         # 서버 시작
zellij web --daemonize             # 백그라운드 시작
zellij web --status                # 상태 조회
zellij web --status --timeout 5
# 서버 이용자에게 영향을 주므로 필요할 때만:
# zellij web --stop
```

`zellij web`은 다른 관리 동작이 지정되지 않으면 시작한다. 명시적인 시작 옵션은 `--start`다. `--daemonize`의 단축형은 `-d`이며 최상위 `--debug`의 `-d`와 위치가 다르다.

## 2. 서버 옵션 전체

| 옵션 | 용도 및 제약 |
| --- | --- |
| `--start` | 서버 시작 |
| `--stop` | 서버 중지. 다른 옵션과 함께 쓰지 않는 독립 관리 동작 |
| `--status` | online/offline 및 online이면 base URL 조회 |
| `--timeout <SECONDS>` | 상태 조회 제한 시간, 기본 30초. `--status` 필요 |
| `-d`, `--daemonize` | 백그라운드 시작. stop/status/일부 토큰 관리와 충돌 |
| `--server-startup-timeout <SECONDS>` | Windows daemon 시작 대기 기본 10초. Unix에서는 무시 |
| `--ip <IP>` | 수신 IP, 기본 127.0.0.1. status 대상 지정에도 사용 |
| `--port <PORT>` | 수신 포트, 기본 8082. status 대상 지정에도 사용 |
| `--cert <PATH>` | TLS 인증서 경로 |
| `--key <PATH>` | TLS 개인키 경로 |
| `--create-token` | 로그인 토큰 생성 |
| `--create-read-only-token` | 기존 세션 관찰용 읽기 전용 토큰 생성 |
| `--token-name <NAME>` | 토큰 이름 |
| `--list-tokens` | 토큰 이름/생성일 목록. 토큰 원문을 복원하지 않음 |
| `--revoke-token <NAME>` | 이름으로 토큰 폐기 |
| `--revoke-all-tokens` | 모든 로그인 토큰 폐기 |

`--cert`/`--key`는 시작용으로, status 또는 token 관리와 섞지 않는다. 모든 조합의 허용 여부는 `zellij web --help`로 확인한다.

## 3. 인증 토큰

```sh
# 실제 사용 시 안전한 터미널에서 실행하고 출력된 비밀은 공유하지 않는다.
zellij web --create-token
zellij web --create-read-only-token
zellij web --list-tokens
# 예: 특정 토큰 폐기
# zellij web --revoke-token observer-token
```

- 로그인 토큰은 로컬 DB에 해시로 저장되므로 **생성 시 한 번만** 원문을 볼 수 있다. 이후 복구 대신 폐기/재발급한다.
- 일반 로그인 사용자는 해당 OS 사용자의 터미널에 접근하는 신뢰된 사용자로 간주된다. 애플리케이션별 세밀한 역할 권한 체계라고 가정하지 않는다.
- 읽기 전용 토큰 사용자는 세션을 관찰할 수 있지만 입력/상호작용은 할 수 없다. 0.45.0 CLI 설명은 기존 세션에 watcher로 attach한다고 명시한다.
- 토큰 폐기 시 연관된 브라우저 세션 토큰도 폐기된다.
- 실제 토큰, 개인키, 쿠키를 문서·Git·대시보드 로그에 넣지 않는다.

**문서/코드 차이:** 온라인 가이드는 `zellij web --create-read-only-token --token-name "observer-token"` 조합을 안내한다. 대조한 0.45.0 소스에서는 생성 옵션에 `exclusive(true)`가 지정되어 다른 인자와의 조합을 제한할 가능성이 있다. 이름을 붙이는 조합은 해당 설치 버전에서 확인하고, 실패하면 이름 없는 생성 도움말 경로를 사용한다. 문서만 보고 호환된다고 단정하지 않는다.

## 4. TLS와 공개 범위

127.0.0.1 이외 주소에 바인딩하면 TLS가 필수다. localhost에서도 TLS 사용이 권장된다.

```kdl
web_server false
web_server_ip "127.0.0.1"
web_server_port 8082
web_server_cert "/path/to/cert.pem"
web_server_key "/path/to/key.pem"
enforce_https_on_localhost true
```

- `web_server true`를 사용하면 Zellij 시작 시 Web 서버를 자동 시작한다.
- `web_server_ip "0.0.0.0"`는 모든 인터페이스에 공개하므로 특별한 이유 없이 설정하지 않는다.
- 인터넷 등 신뢰하지 않는 네트워크에 공개할 때 공식 문서는 nginx 같은 reverse proxy를 권장한다. 내장 서버에는 자체 rate limiting이 없다.
- `web_client { base_url "/zellij"; }`는 프록시 하위 경로용 설정이다.
- 브라우저 terminal의 font/cursor/theme 설정은 CLI 기능과 별개다. 세부 UI 옵션은 공식 Web Client 페이지를 참고한다.

URL 뒤에 세션 이름을 붙이면 해당 이름의 세션을 생성/연결/복원한다. 예: `https://my-server/zellij/my-session`. 공유 링크가 새 세션/복원 동작을 유발할 수 있다는 점도 고려한다.

## 5. 터미널에서 원격 접속

```sh
zellij attach https://my-server:8082/my-session --token <LOGIN_TOKEN>
zellij attach https://my-server:8082/my-session --token <LOGIN_TOKEN> --remember
zellij attach https://my-server:8082 --forget
zellij attach https://my-server:8082/my-session --ca-cert /path/to/ca.pem
```

`<LOGIN_TOKEN>`은 설명용 자리표시자다. 실제 값을 문서에 기록하지 않는다. 명령행 인자는 셸 history나 프로세스 조회에 노출될 수 있으므로 보안 정책에 따라 사용한다.

- `-t`, `--token <TOKEN>`: 원격 서버 로그인 토큰.
- `-r`, `--remember`: 4주 동안 재인증을 위한 자격 증명을 로컬 저장.
- `--forget`: 서버에 대해 저장된 자격 증명 삭제.
- `--ca-cert <FILE>`: 서버 검증에 사용할 PEM CA 인증서.
- `--insecure`: TLS 인증서 검증 전체를 생략. 개발/신뢰된 환경에 한정하고 기본 해결책으로 쓰지 않는다.
- 자체 서명 인증서는 기본적으로 거부된다. 가능하면 CA를 명시하는 검증 경로를 사용한다.
- 원격 attach에는 [세션 초기 명령](02-sessions.md)을 사용할 수 없다.

## 6. 빌드 지원 여부

Web 기능은 `web-server-capability` 컴파일 기능으로 제외할 수 있다. 공식 배포에는 `zellij-no-web` 바이너리도 있다. `web`이 동작하지 않으면 바이너리 종류/컴파일 기능부터 확인한다. Windows는 ConPTY와 named pipes를 사용하며, daemon 시작 대기 방식도 Unix와 다르다.

## 출처

- https://zellij.dev/documentation/web-client.html
- https://zellij.dev/documentation/faq.html
- https://docs.rs/zellij-utils/0.45.0/src/zellij_utils/cli.rs.html (WebCli, Attach)
