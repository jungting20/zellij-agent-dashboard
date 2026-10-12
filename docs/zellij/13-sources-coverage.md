# 출처·조사 범위·갱신 방법

조사일: 2026-10-08.

## 조사 기준

Zellij User Guide의 CLI 관련 장을 일차 출처로 사용했다. 공식 릴리스 Rust crate의 CLI 선언(docs.rs)은 가이드의 누락/오탈자를 확인하는 보완 출처다. 온라인 문서에는 페이지별 고정 버전이 표시되지 않으므로 특정 릴리스와 동일하다고 가정하지 않는다.

이 문서 모음의 기본 보완 기준은 `zellij-utils 0.45.0`이다. 플러그인/자동화 문서에서 별도로 대조한 0.45.1 자료는 해당 파일에 명시되어 있다. 서로 다른 버전의 기능을 한 설치 환경에 모두 존재한다고 가정하지 않는다.

## 공식 출처와 분류

### 시작·최상위 명령·세션

- https://zellij.dev/documentation/introduction.html
- https://zellij.dev/documentation/installation.html
- https://zellij.dev/documentation/commands.html
- https://zellij.dev/documentation/controlling-zellij-through-cli.html
- https://zellij.dev/documentation/session-resurrection.html
- https://zellij.dev/documentation/nested-sessions.html

대응: `01-installation-cli-basics.md`, `02-sessions.md`.

### CLI 상세

- https://zellij.dev/documentation/cli-actions.html
- https://zellij.dev/documentation/zellij-run-and-edit.html
- https://zellij.dev/documentation/zellij-plugin-and-pipe.html
- https://zellij.dev/documentation/zellij-subscribe.html
- https://zellij.dev/documentation/cli-recipes.html
- https://zellij.dev/documentation/programmatic-control.html

대응: `03-actions.md`, `04-run-edit.md`, `08-plugins-pipes.md`, `09-subscribe-programmatic-control.md`, `10-recipes-dashboard.md`.

### 설정·레이아웃·셸

- https://zellij.dev/documentation/configuration.html
- https://zellij.dev/documentation/command-line-options.html
- https://zellij.dev/documentation/options.html
- https://zellij.dev/documentation/layouts.html
- https://zellij.dev/documentation/creating-a-layout.html
- https://zellij.dev/documentation/swap-layouts.html
- https://zellij.dev/documentation/layouts-with-config.html
- https://zellij.dev/documentation/layout-examples.html
- https://zellij.dev/documentation/shell-integration.html
- https://zellij.dev/documentation/integration.html

대응: `05-configuration.md`, `06-layouts.md`, `07-shell-integration.md`.

### 플러그인 통신·원격·호환성

- https://zellij.dev/documentation/plugin-loading.html
- https://zellij.dev/documentation/plugin-aliases.html
- https://zellij.dev/documentation/plugin-pipes.html
- https://zellij.dev/documentation/plugin-api-permissions.html
- https://zellij.dev/documentation/web-client.html
- https://zellij.dev/documentation/faq.html
- https://zellij.dev/documentation/compatibility.html

대응: `08-plugins-pipes.md`, `11-web-remote.md`, `12-troubleshooting-safety.md`.

### 버전 고정 보완 자료

- https://docs.rs/zellij-utils/0.45.0/src/zellij_utils/cli.rs.html
- https://docs.rs/zellij-utils/0.45.0/zellij_utils/setup/struct.Setup.html
- https://docs.rs/zellij-utils/0.45.0/zellij_utils/cli/index.html

0.45.1 및 추가 보완 자료는 이를 실제로 사용한 `08`~`10` 문서의 출처를 참고한다. docs.rs의 `latest` URL은 시간이 지나면 바뀌므로 유지보수 시 버전 고정 URL로 기록하는 것이 좋다.

## 범위 체크리스트

- [x] 설치·버전·도움말·최상위 옵션·명령·별칭
- [x] setup의 설정 검사/기본 설정/레이아웃/플러그인 덤프/셸 생성
- [x] 세션 생성/attach/백그라운드/목록/watch/detach/종료/삭제
- [x] 종료 세션 복원·초기 명령·중첩 세션
- [x] CLI Actions 공식 페이지에 열거된 **78개 action** 전체 분류 및 세부 설명
- [x] run/edit 인자·옵션·command pane 수명·blocking
- [x] KDL 설정·CLI options·우선순위·경로
- [x] 레이아웃·tab/pane·floating·stack·templates·swap·cwd
- [x] completion·자동 시작·셸 환경·OSC 통합
- [x] plugin·pipe·aliases·메시지·권한 경계
- [x] subscribe·JSON·pane/tab ID·비대화식 제어
- [x] 대시보드 자동화 레시피·동기화·오류·취소 패턴
- [x] web 서버·토큰·HTTPS·원격 attach·읽기 전용 접속
- [x] 호환성·문제 해결·보안 경고
- [ ] 설치된 로컬 Zellij로 예제 실행 검증: 조사 환경 PATH에서 바이너리를 찾지 못해 미수행
- [ ] 모든 OS·모든 과거 릴리스의 동작 검증: 이 조사의 범위 밖

플러그인 개발 API 전체, 모든 테마, 모든 키바인드, 내부 IPC 구현 전체는 범위 밖이다. CLI와 키바인드 action의 차이는 각 문서에서 설명한다.

## 발견한 불일치 기록

1. Commands의 `kill-sessions` 제목과 실제 `kill-session` 명칭.
2. Configuration의 `options --clean` 예제와 `setup --clean` 실제 정의.
3. 최신 Commands의 attach 초기 명령이 0.45.0 Attach 소스에는 없음.
4. `toggle-no-ui-fullscreen --tab-id`, `set-pane-borderless`, `resize`의 가이드/버전 고정 소스 차이.
5. `new-pane -b`와 `run -b`의 의미 차이. 긴 옵션을 권장.
6. plugin 옵션표 일부 누락, blocking과 show/hide 종료 코드 설명 차이.
7. `scrollback_lines_to_serialize` 예제의 잘못된 옵션명.
8. Web read-only token 이름 조합 안내와 0.45.0 Clap exclusive 선언의 차이.

각 항목의 정확한 차이와 대응은 관련 문서에 기록했다. 추측으로 하나의 문법으로 통일하지 않는다.

## 갱신 절차

1. 설치된 `zellij --version`, `zellij --help`, 하위 명령 `--help`를 안전하게 수집한다. 기존 세션을 변경할 필요는 없다.
2. 공식 가이드 목차와 action 목록을 다시 비교하고 새 명령/삭제 옵션/기본값 변경을 확인한다.
3. 온라인 문서와 릴리스 소스가 다르면 버전 경계를 유지해 기록한다.
4. 새 예제는 가능한 한 고유 이름의 임시 세션에서만 검증한다. 실제 수행 여부와 결과를 문서에 명시한다.
5. 문서 간 상대 링크, 코드 fence, 명령의 argv 경계/quoting, 위험 동작 경고를 점검한다.
6. `README.md`, 이 출처 목록, 프로젝트 루트 `AGENTS.md`의 문서 탐색 지침을 함께 갱신한다.

원문 전체의 무차별 복제 대신 명령/옵션을 목적별로 재구성했다. 내용의 최종 확인은 각 공식 출처와 설치된 CLI 도움말을 통해 수행한다.
