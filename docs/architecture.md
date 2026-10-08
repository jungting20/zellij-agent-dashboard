# Agent Dashboard 아키텍처

Zellij 서버가 백그라운드 수집기를 실행한다. 화면 pane과 수집기의 수명을 분리하고, 호스트 보조 명령은 요청을 처리한 뒤 종료한다. 기존 에이전트 데몬에 연결하지 않는다.

## 실행 경로

```text
Claude 훅 ── dashboard-host hook ── 공유 JSON 저장소
                 │                     ↑
                 └── pipe 변경 통지      │ 파일 잠금 + 원자적 교체
                                       │
Zellij collector ── 2초 타이머 ── dashboard-host scan
                                       │
Zellij dashboard ─────────────── dashboard-host snapshot
       │
       └── Enter ── dashboard-host resolve ── 실행 세대 확인 ── pane 이동
```

`dashboard-core`는 이벤트·상태 전이와 화면 모델을 담당한다. `dashboard-host`는 프로세스 탐지, 저장소, Claude 훅을 담당한다. `dashboard-plugin`은 같은 WASI 실행 파일을 `mode=collector`와 `mode=dashboard` 설정으로 사용한다. 파일 경로는 WASI 마운트 경로로 변환하지 않고 네이티브 보조 명령에 절대 경로로 전달한다.

collector는 시작 권한을 받은 뒤 화면에서 숨겨진다. 대시보드 화면을 닫거나 클라이언트가 detach해도 타이머 수집이 남는다. 모든 수집기 세션을 종료하면 주기적 탐지도 멈춘다. 연결된 Claude 훅 자체는 저장소를 직접 갱신할 수 있다.

## 호스트 인터페이스와 의존성 주입

호스트 시작점은 `SystemCommandRunner`와 `ZellijCli`를 생성하고 `HostDependencies`로 명령 처리 함수에 전달한다. 기능 로직은 구체적인 CLI 구현을 생성하거나 전역 실행기에 접근하지 않는다.

`TerminalHost`는 terminal pane 목록과 화면 조회, 문자·바이트 입력, 종료, 생성, 변경 통지를 제공한다. `SessionId`, 불투명한 `PaneId`, `TerminalPane`, `NewPane`을 사용하며 Zellij JSON, `terminal_N`, CLI 옵션은 계약에 포함하지 않는다. 목록에는 terminal pane만 포함한다. 생성 옵션은 cwd, 제목, floating, close-on-exit, no-focus, 프로그램과 argv다. `notify_changed`는 이벤트 ID를 전달하는 최선의 통지이며 저장된 상태가 기준이다.

`ZellijCli`는 실행 파일 경로와 `CommandRunner`를 주입받고 Zellij 명령 조립과 응답 파싱을 담당한다. `CommandRunner`는 Zellij 외에도 git, ps, zoxide, 자식 worktree 셸 명령을 실행한다. `CommandSpec`은 프로그램·argv·cwd·환경 변수 설정/제거·선택적 timeout·스트림별 출력 제한·수집/폐기 모드를 정의한다. 실행기는 종료 상태와 stdout/stderr를 반환하며 실행 오류, timeout, 출력 초과를 구분한다. timeout과 출력 초과 시 자식을 종료하고 회수한다. argv는 셸에 재해석하지 않고 직접 전달한다. 사용자가 요청한 셸 명령과 EDITOR 실행만 기존 셸 경로를 유지한다.

입력의 bracketed paste와 Enter는 별도 호출이며 그 사이에 대상 실행 세대를 재검증한다. 중복 요청, 고정 대상 보호, 불명확한 결과의 자동 재전송 금지와 통지 실패 무시 정책은 유지한다. 기존 timeout과 출력 제한을 보존하며 ps는 timeout 없이 64 MiB 제한과 locale 설정을 사용한다.

상태 파일과 외부 JSON 계약은 기존 숫자 pane ID를 유지한다. 호스트 경계의 변환 함수가 이를 공통 pane 식별자로 연결하므로 스키마 이관은 필요하지 않다. 다른 실행 환경을 지원할 때 `TerminalHost` 구현은 교체할 수 있지만, 전체 앱 이관에는 이 숫자 ID 호환 경계, Zellij 환경 변수와 서버 프로세스에 기반한 발견 방식, 플러그인 SDK 기반 화면 이동의 추가 변경이 필요하다. 파일·환경 접근과 실행 스크립트는 이번 주입 범위에 포함하지 않는다.

## 공유 상태와 동시성

기본 저장 위치는 `${XDG_STATE_HOME:-$HOME/.local/state}/zellij-agent-dashboard`다. `store.json`은 schema version, revision, 마지막 스캔 시점, 에이전트와 활동 기록을 포함한다. 별도 `store.lock` 파일의 OS 잠금을 잡은 명령만 파일을 읽거나 갱신한다. 잠금 대기는 최대 2초다.

여러 세션·클라이언트의 collector가 같은 저장소를 사용한다. 별도의 작성자 선출이나 상주 조정 프로세스를 두지 않는다. 공유 마지막 스캔 시점으로 프로세스 탐지를 1.8초 이상 간격으로 제한한다. 훅은 같은 잠금 아래 현재 실행 ID를 확인하고 순서 번호를 부여해 반영한다.

저장소 디렉터리는 700, 상태와 잠금 파일은 600 권한이다. 임시 파일에 직렬화하고 fsync 후 원자적으로 교체한다. 손상되거나 더 새로운 스키마의 파일은 덮어쓰지 않고 오류를 반환한다. 복구하려면 모든 관련 수집기를 중단하고 기존 상태 파일을 백업·이동한 뒤 다시 시작한다. 현재 스키마는 1이며 기존 SQLite 데이터는 읽지 않는다.

## 실행 식별과 상태

에이전트 ID는 세션 이름, 세션 실행 세대, pane ID, 프로세스 실행 세대의 JSON 튜플이다. 세션 세대는 Zellij 서버 PID와 시작 시각, 프로세스 세대는 에이전트 PID와 시작 시각을 사용한다. 원본 이름을 그대로 보존하므로 공백과 한국어 이름으로 세션을 찾을 수 있다. OS 프로세스 시작 시각의 해상도는 초 단위다.

`ps`에서 실행 파일과 Zellij 환경을 확인하고 중첩된 도구 런처 중 가장 깊은 실행 프로세스를 등록한다. 성공한 전체 목록과 현재 보조 명령의 존재를 확인한 경우만 생존 정보를 갱신한다. 현재 macOS에서 실제 실행을 검증했다. 훅 없는 `cwd`는 상속된 `PWD`이며 실제 훅의 `cwd`가 보고되면 갱신한다.

상태는 `found`, `idle`, `working`, `waiting`, `done`, `failed`, `compact`다. 생존 여부 `live/gone/unverified`와 60초 이후의 `stale` 표시는 작업 상태와 분리한다. 시간 경과나 프로세스 존재만으로 완료·진행을 추정하지 않는다. 프로세스 검증이 10초 이상 없으면 저장된 live 상태를 cached로 표시한다. 종료 기록은 최대 24시간 유지하고 활동 기록은 최근 50개를 보존한다.

이벤트는 버전, event ID, 전체 identity, tool, kind, sequence, observed_at_ms, cwd, summary, detail을 갖는다. 등록되지 않은 실행, 다른 세대, 중복 ID, 이전 순서·보고 시각의 이벤트를 무시한다. SessionEnd 뒤 아직 종료 중인 프로세스를 발견해도 이전 실행을 되살리지 않는다.

## 훅과 변경 통지

Claude 훅은 현재 pane의 실제 Claude 프로세스가 자신의 조상인지 확인한다. 원본 JSON을 공통 이벤트로 변환하고 상태 파일에 먼저 기록한다. `zellij pipe --name agent-dashboard-changed`는 빠른 갱신을 위한 통지이며 전달 실패 시 다음 주기 조회로 수렴한다. 통지 프로세스는 최대 500ms 후 종료한다. 훅 오류는 stderr에 남기고 종료 코드는 0을 유지해 Claude 실행을 막지 않는다.

설정 생성기는 별도 JSON을 출력한다. 사용자 전역 훅을 자동 등록하지 않는다. 제공하는 이벤트는 SessionStart, UserPromptSubmit, PreToolUse, PostToolUse, PostToolUseFailure, PermissionRequest, Notification, Stop, PreCompact, SessionEnd다. Notification은 permission_prompt와 idle_prompt만 받는다. PostToolUseFailure는 도구 결과로 처리하며 전체 턴 실패로 간주하지 않는다. StopFailure와 PostCompact는 파서에만 있고 설치 설정에는 아직 포함하지 않는다.

실제 Claude 검증은 SessionStart만 수행했다. 나머지 이벤트 매핑의 전체 실동작 검증, Codex 및 다른 도구의 훅 어댑터, 화면 내용 보완 탐지는 후속 작업이다.

## 화면과 권한

각 화면은 선택한 실행 ID, 검색어, 고정 필터를 따로 갖는다. 정렬이 바뀌어도 같은 실행을 선택한다. 상태 긴급도 정렬, 검색, 크기 제한, 한국어 폭 계산과 제어 문자 제거를 코어에서 처리한다. 고정·별칭·부모 필드는 저장 계약에 있지만 편집과 관계 수집은 아직 제공하지 않는다. 현재 활동 화면은 최신 변화 하나를 표시한다.

플러그인은 RunCommands, ReadApplicationState, ChangeApplicationState, ReadCliPipes 권한을 요청한다. 명시적 거절 뒤에는 반복 요청하지 않고 재로딩으로 다시 시작한다. CLI pipe 입출력에도 별도의 ReadCliPipes 권한이 필요하다.

Enter는 보조 명령으로 현재 프로세스를 다시 검증한 뒤 `switch_session_with_focus`를 호출한다. 입력 전송·종료·실행은 아직 제공하지 않는다. M3에서는 실행 전 intent와 실행 후 결과를 저장하고, 결과가 불명확한 조작을 자동 재전송하지 않는 경로를 추가한다.

Zellij 0.45.0은 연결된 클라이언트가 없을 때 plugin reload를 거부한다. 수집기 자체의 detach 이후 실행과는 다른 조건이다. 재로딩은 클라이언트를 연결한 상태에서 수행한다. 검증 범위는 [실행 검증 기록](runtime-validation.md)을 따른다.
