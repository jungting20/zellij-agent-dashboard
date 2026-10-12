# 세션 생성·연결·조회·종료·복원

조사일: 2026-10-08. 온라인 가이드 및 0.45.0 CLI 정의 기준. 아래 예제는 문서 기반이며 로컬 실행 검증하지 않았다.

## 1. 생성과 연결

```sh
zellij --session agent-dashboard
zellij attach agent-dashboard
zellij attach --create agent-dashboard
zellij attach --create-background agent-dashboard
```

- `attach` 별칭: `a`.
- 세션 이름을 생략하고 실행 중 세션이 하나뿐이면 그 세션에 연결한다.
- `-c`, `--create`: 없으면 세션을 생성하고 연결한다.
- `-b`, `--create-background`: 없으면 연결되지 않은 세션을 백그라운드에 생성한다. TTY를 점유하지 않는 자동화 시작점이다.
- `--index <N>`: 생성일 순으로 정렬된 활성 세션의 인덱스로 연결한다. 인덱스 기준값/정렬 변화는 설치 버전 도움말로 확인하고 영속 식별자로 쓰지 않는다.
- `-f`, `--force-run-commands`: 종료된 세션 복원 시 저장된 명령을 즉시 실행한다. 파괴적 명령까지 재실행할 수 있다.
- `attach` 뒤에 `options` 하위 명령을 추가할 수 있다. 예: `zellij attach --create agent-dashboard options --mouse-mode false`.
- 원격 연결 옵션 `--token`, `--remember`, `--forget`, `--ca-cert`, `--insecure`는 [Web/원격 접속](11-web-remote.md)을 참고한다.

### 초기 명령: 최신 온라인 가이드의 확장

```sh
zellij attach --create agent-dashboard -- htop
zellij attach --create-background agent-dashboard -- sh -c 'printf "ready\n"; sleep 60'
zellij attach --create --close-on-exit command-demo -- sh -c 'printf "done\n"'
zellij attach --create --start-suspended command-demo -- htop
```

- `--` 이후 명령은 **이번 호출이 새 세션을 생성할 때만** 실행한다. 이미 실행 중인 세션에 attach하면 무시한다.
- `--close-on-exit`: 초기 명령 종료 시 해당 pane을 닫는다. 기본은 종료 후 pane 유지.
- `--start-suspended`: Enter를 누르기 전까지 초기 명령 실행을 보류한다.
- 위 두 옵션은 초기 명령이 있어야 한다.
- 레이아웃도 사용하면 focused tab에 command pane을 배치한다.
- 원격 Web 세션에는 초기 명령을 지원하지 않는다.
- **버전 주의:** 이 기능은 조사 당시 온라인 Commands 가이드에는 있으나 대조한 docs.rs 0.45.0 `Attach` 정의에는 없다. `zellij attach --help`에 인자/옵션이 실제로 있는지 확인한다. 없으면 별도의 `run`/`action new-pane` 또는 layout command pane을 사용한다.

## 2. 목록과 관찰

```sh
zellij list-sessions
zellij ls --no-formatting
zellij ls --short
zellij ls --reverse
zellij watch agent-dashboard
```

`list-sessions` 옵션:

- `-n`, `--no-formatting`: 색/서식 제외.
- `-s`, `--short`: 세션 이름만 출력.
- `-r`, `--reverse`: 기본 오름차순의 역순.

Commands 페이지는 실행 중 세션 목록이라고 설명하지만 Session Resurrection 페이지는 종료되어 복원 가능한 세션도 여기서 확인한다고 설명한다. 따라서 목록에 이름이 있다고 곧바로 활성 세션이라고 판단하지 않는다. 설치 버전의 실제 출력과 추가 조회 성공 여부를 확인한다.

`watch` 또는 `w`는 터미널에서 세션을 읽기 전용으로 관찰한다. 출력은 보이지만 입력은 보내지 않는다. pane별 데이터 처리용 `subscribe`와 목적이 다르다. `watch`의 인증 보안과 Web read-only 토큰도 서로 구분한다.

## 3. 연결 해제, 종료, 영구 삭제의 차이

```sh
# 세션은 유지하고 이 클라이언트 연결만 해제
zellij --session agent-dashboard action detach

# 아래는 실행 중 프로그램에 영향을 주는 파괴적 동작 예시다.
# 실제 대상과 복원 정책을 확인한 뒤에만 실행한다.
# zellij kill-session agent-dashboard
# zellij delete-session agent-dashboard
# zellij delete-session --force agent-dashboard
```

- `detach`: 클라이언트 연결 해제. 세션 프로세스 종료와 다르다.
- `kill-session [NAME]` (`k`): 실행 중인 대상 세션을 종료한다.
- `kill-all-sessions` (`ka`): 전체 실행 세션 종료. 확인 프롬프트가 있으며 `-y`, `--yes`가 생략한다.
- `delete-session [NAME]` (`d`): 복원 가능한 세션 정보를 영구 삭제한다. `-f`, `--force`는 실행 중이면 먼저 종료한다.
- `delete-all-sessions` (`da`): 모든 복원 정보 삭제. `-y`, `--yes`, `-f`, `--force`를 지원한다.

**공식 가이드 오탈자:** Commands 페이지 제목에는 `kill-sessions`라고 적혀 있지만, 0.45.0 CLI의 실제 enum은 `KillSession`이고 CLI Recipes/자동완성 설명도 단수형 `kill-session`을 사용한다. 스크립트에서는 `zellij kill-session --help`로 확인한 단수형을 사용한다.

`kill-all-sessions`, `delete-all-sessions`, `--force`, `--yes`는 테스트 정리 용도로 무차별 실행하지 않는다. 자동화가 만든 고유 세션 하나만 대상으로 한다.

## 4. 세션 복원

기본적으로 세션 레이아웃과 각 pane의 명령이 cache 디렉터리에 직렬화된다. 정상 종료나 충돌 뒤에도 `list-sessions` 및 session-manager의 EXITED 항목에서 찾고 `attach`로 복원할 수 있다.

```sh
zellij list-sessions
zellij attach previous-session
# 저장된 명령을 검토한 경우에만:
# zellij attach --force-run-commands previous-session
```

- 복원은 프로세스의 메모리/정확한 실행 상태를 되살리는 체크포인트가 아니다. 명령을 다시 실행하는 방식이다.
- 기본적으로 명령은 `Press ENTER to run...` 상태에서 대기하여 위험한 재실행을 방지한다.
- 탭 순서는 직렬화되므로 재정렬된 순서를 유지한다.
- 가이드는 세션 데이터를 1초마다 KDL 레이아웃으로 저장한다고 설명한다. 캐시 레이아웃은 읽고 수정하거나 `zellij --layout session-layout.kdl`로 사용할 수 있다.

설정 예시:

```kdl
session_serialization true
pane_viewport_serialization true
scrollback_lines_to_serialize 1000
```

- `session_serialization false`: 직렬화/복원 비활성화.
- `pane_viewport_serialization true`: 화면 내용도 저장. session serialization이 활성화되어야 한다.
- `scrollback_lines_to_serialize 0`: viewport serialization이 켜졌을 때 전체 scrollback 저장. 저장량/민감정보에 주의.
- `post_command_discovery_hook`: 발견한 명령을 `$RESURRECT_COMMAND`로 전달받고 stdout을 저장할 명령으로 사용. 래퍼 명령 감지 오류를 보완하지만 셸 코드를 실행하는 설정이다.

화면 및 scrollback 저장은 API 키, 고객 정보, 터미널 출력까지 디스크에 남길 수 있다. 최소한의 줄 수와 접근 권한을 사용한다.

## 5. 중첩 세션

공식 가이드는 0.45.0부터 중첩 세션을 일급 워크플로로 지원한다고 설명한다. guest는 pane에 설정된 `ZELLIJ` 환경변수를 통해 중첩 여부를 감지하고 host에 알린다. 자기 자신의 세션으로 다시 attach하는 것은 여전히 금지된다.

```kdl
nested_session_handling "ask"
```

값: `ask`(기본, 선택 프롬프트), `fullscreen`(guest pane 확대), `descend`(guest로 입력 전달), `never`(자동 처리 없음).

Nested Sessions 가이드는 관련 동작을 CLI에서도 사용할 수 있다고 설명하지만, 0.45.0/0.45.1 `CliAction` 정의에는 대응 명령이 없다. `focus-guest-session`, `focus-host-session`, `toggle-host-fullscreen`은 이름 규칙에 따른 후보 표기이며 지원이 확인된 CLI 명령이 아니다. `zellij action --help`에서 확인하기 전 호출하지 않는다. 상세 차이는 [Action의 중첩 세션 보완 항목](03-actions.md)을 참고한다. 기본 키바인드의 session mode에서 `[`는 guest로 진입, `]`는 host로 복귀, `f`는 host 안에서 fullscreen 토글이다. 기본 키바인드는 사용자가 바꿀 수 있다.

대시보드가 외부에서 제어할 때는 환경변수만으로 대상을 추론하지 말고 `--session`과 pane ID를 함께 관리한다.

## 출처

- https://zellij.dev/documentation/commands.html
- https://zellij.dev/documentation/controlling-zellij-through-cli.html
- https://zellij.dev/documentation/session-resurrection.html
- https://zellij.dev/documentation/nested-sessions.html
- https://docs.rs/zellij-utils/0.45.0/src/zellij_utils/cli.rs.html (Sessions, Attach)
