# 문제 해결·검증·안전 수칙

조사일: 2026-10-08. 공식 가이드의 진단 사항과 이 프로젝트에 권장하는 운영 원칙을 구분해 기록한다. 실제 프로젝트 구현/설치된 바이너리의 동작은 검증하지 않았다.

## 1. 먼저 수집할 정보

```sh
command -v zellij
zellij --version
zellij --help
zellij setup --check
zellij action --help
zellij --session <TARGET_SESSION> action list-panes --help
```

버전, 대상 세션, 정확한 argv, cwd, 종료 코드, stderr를 함께 기록한다. 토큰·셸 환경변수 전체·민감한 화면 출력은 로그에서 제외한다. 도움말 조회와 실제 세션 조회/변경을 구분한다.

조사 환경에서는 `command -v zellij`가 실패했다. 따라서 현재 PATH에 바이너리가 없다는 사실만 확인했으며 시스템 전체에 설치되지 않았다고 단정하지 않는다. 이 문서의 예제가 로컬 통합 테스트를 통과했다는 뜻도 아니다.

## 2. 명령/옵션이 없다고 나올 때

1. 온라인 가이드와 설치 버전이 일치하는지 확인한다.
2. 전역 옵션은 하위 명령 앞에, 하위 명령 옵션은 그 뒤에 둔다.
3. `run -- ...`, `plugin -- ...` 등 인자 구분자를 확인한다.
4. 사라진 옵션 대신 새 옵션을 사용한다. 예: `options --disable-mouse-mode` 대신 `options --mouse-mode false`, `options --no-pane-frames` 대신 `options --pane-frames false`.
5. keybind action 이름(PascalCase), CLI action 이름(kebab-case), plugin API 이름(snake_case)을 혼동하지 않는다. 이들은 반드시 일대일 동일 옵션이 있는 인터페이스가 아니다.
6. 온라인 가이드의 잘못된 `kill-sessions` 표기는 `kill-session --help`로 검증한다.
7. attach 초기 명령, `send-keys`, 조회 JSON, subscribe, explicit ID 옵션 등은 도움말 기반 capability check를 둔다.

## 3. 입력이 엉뚱한 pane으로 갈 때

대시보드 권장 패턴:

- 실행할 때마다 대상 `--session`을 명시한다.
- pane ID와 tab ID는 목록/생성 응답에서 취득하고 재시작·복원 뒤 다시 발견한다.
- tab position과 stable tab ID를 혼동하지 않는다. 1-based/0-based는 각 명령의 문서를 확인한다.
- `terminal_N`과 `plugin_N`을 구분하고 숫자 변환/원본 ID를 별도로 관리한다.
- focused pane에 의존하는 명령은 다중 클라이언트 환경에서 경쟁 상태가 생긴다.
- 문자열 입력(`write-chars`)과 키 입력(`send-keys`, `write`)을 구분한다. Enter를 보내면 현재 셸 명령이 실제 실행될 수 있다.
- 외부 입력으로 `sh -c` 문자열을 조합하지 말고 프로세스 실행 API에 인자 배열을 전달한다.

상세 프로토콜과 ID 형식은 [구독/프로그래밍 제어](09-subscribe-programmatic-control.md), [레시피](10-recipes-dashboard.md)를 참고한다.

## 4. 화면 조회/구독이 기대와 다를 때

- Zellij의 rendered viewport/scrollback은 원래 자식 프로세스 stdout 바이트 스트림과 같지 않다.
- ANSI 포함 여부, raw/JSON 형식, scrollback 범위를 명시한다.
- JSON 줄 단위 스트리밍은 분할된 chunk를 재조립해서 한 줄씩 파싱한다.
- 화면 내용에는 비밀이 포함될 수 있으므로 저장/전송/렌더링 전에 보안 경계를 둔다.
- pane 닫힘, 세션 종료, 취소, timeout, 구독 재연결을 처리한다.
- blocking CLI의 종료 코드를 곧바로 작업 프로세스의 종료 코드로 단정하지 않는다. 공식 인터페이스가 보장하는 범위와 조회 상태를 확인한다.

## 5. 터미널 호환성: 공식 안내

### 글꼴/UI

문자가 깨지면 Nerd Font 등 필요한 글리프가 있는 글꼴을 쓰거나 다음 옵션을 시험한다.

```sh
zellij options --simplified-ui true
zellij --layout compact
```

`pane_frames false`로 프레임을 감출 수 있고, 공식 최신 FAQ에 따르면 0.45.0 기본 `pane_frame_style "titles"`는 pane 위 제목 한 줄만 사용한다.

### macOS Option/Alt

터미널별 Option-as-Alt 설정이 다르다. Terminal.app, iTerm2, Alacritty 등 실제 터미널 설정을 확인한다. 키 충돌은 Unlock-First preset 또는 modifier 변경으로 해결할 수 있다. CLI 자동화는 이런 UI 키바인드를 그대로 타이핑하기보다 명시적인 action을 사용한다.

### 복사/붙여넣기

일부 터미널은 OSC 52를 지원하지 않는다. 로컬 OS에 맞는 외부 명령을 지정할 수 있다.

```kdl
copy_command "pbcopy"                     // macOS
// copy_command "wl-copy"                // Wayland
// copy_command "xclip -selection clipboard" // X11
```

SSH 원격 세션에서 호스트 클립보드로 복사하려면 공식 FAQ는 OSC 52를 지원하는 터미널을 안내한다. 0.45.0부터 OSC 52 **읽기**는 기본 차단된다. `dangerously_enable_paste_buffer_read`를 켜면 SSH를 포함한 모든 pane의 프로그램이 확인 없이 클립보드를 읽을 수 있으므로 편의상 켜지 않는다. 쓰기/복사와 읽기 권한은 다르다.

### 마우스와 Backspace

`mouse_mode`가 켜져 있을 때 Shift를 누르면 Zellij 대신 터미널이 선택/링크/복사/스크롤을 처리하도록 우회할 수 있다. Backspace가 Ctrl-H로 전달되어 Move mode가 열리면 터미널에서 `^?`를 보내도록 설정하거나 Ctrl-H 바인딩을 변경한다.

### 편집기

scrollback 편집에는 `$EDITOR`, 그 다음 `$VISUAL`을 확인한다. 세션을 시작하기 전에 설정하거나 `scrollback_editor`를 사용한다.

```sh
export EDITOR=/usr/bin/vim
```

### 색상·이미지·알림

- styled underline 문제는 `styled_underlines false`로 진단할 수 있다.
- kitty graphics는 호스트 터미널 지원이 필요하며 `support_kitty_graphics_protocol` 변경은 재시작이 필요하다.
- Sixel은 실제 호스트 터미널이 지원할 때 pane 프로그램에 광고된다.
- desktop notification은 OSC 9/99/777을 받아 호스트에 전달하며 `host_notification_protocol`로 `auto`, `osc9`, `osc99`, `bell`, `off` 선택. 실제 표시 여부는 OS/터미널에 달렸다.

## 6. 버전 업그레이드와 소켓

최신 FAQ는 IPC contract가 같은 버전 사이에는 실행 중 세션에 업그레이드 후에도 attach할 수 있다고 설명한다. 소켓 경로는 과거 바이너리 버전별 경로 대신 `contract_version_1` 네임스페이스를 사용한다. Windows는 named pipes, Unix 계열은 domain sockets를 사용한다.

대시보드는 가능하면 공개 CLI를 사용하고 소켓 경로/프로토콜 내부 구현을 하드코딩하지 않는다. 이 설명은 과거 모든 버전 조합이 호환된다는 보장이 아니다.

## 7. 변경·테스트 경계

프로젝트 운영 권장 사항:

1. 문서 조사와 실제 세션 변경은 분리한다. 조사만 요청되면 서버 시작, 전역 설정 수정, 셸 초기화 파일 수정, 패키지 설치를 하지 않는다.
2. 테스트는 고유 이름의 임시 세션에서 수행한다. 사용자 세션에 입력하거나 pane을 닫지 않는다.
3. `kill-all-sessions`, `delete-all-sessions`, `--force-run-commands`, `--force`, `--yes`, `--max-panes`, `close-replaced-pane`, `--insecure`는 안전한 기본값이 아니다.
4. Web 토큰, SSH/클라우드 자격 증명, scrollback, 로그는 비공개로 취급한다.
5. 새 CLI 기능은 명령 도움말 검사, 응답 파싱 테스트, 취소/오류 테스트를 함께 추가한다.
6. 문서에는 조사일·출처·지원 버전·실제 실행 검증 여부를 기록한다.

## 출처

- https://zellij.dev/documentation/faq.html
- https://zellij.dev/documentation/compatibility.html
- https://zellij.dev/documentation/commands.html
- https://zellij.dev/documentation/programmatic-control.html
- https://zellij.dev/documentation/zellij-subscribe.html
- https://zellij.dev/documentation/session-resurrection.html
- https://zellij.dev/documentation/web-client.html
