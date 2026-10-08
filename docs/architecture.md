# Agent Dashboard 아키텍처

Zellij 서버가 백그라운드 수집기를 실행한다. 화면 pane과 수집기의 수명을 분리하고, 호스트 보조 명령은 요청을 처리한 뒤 종료한다. 기존 에이전트 데몬에 연결하지 않는다.

## 실행 경로

```text
Claude 훅 ── dashboard-host hook ── 공유 SQLite 저장소
                 │                     ↑
                 └── pipe 변경 통지      │ Repository 트랜잭션
                                       │
Zellij collector ── 2초 타이머 ── dashboard-host scan
                                       ├── 프로세스 생존 확인
                                       └── 훅 미연결 실행의 화면 판별
                                       │
Zellij dashboard ─────────────── dashboard-host snapshot
       │
       └── Enter ── dashboard-host resolve ── 실행 세대 확인 ── pane 이동
```

`dashboard-core`는 공통 `StateSignal`, 상태 전이와 화면 모델을 담당한다. `dashboard-host`의 도구별 어댑터는 Claude 훅 JSON과 pane 화면을 공통 신호로 정규화한다. 외부 수집과 저장은 host에 남는다. `dashboard-plugin`은 같은 WASI 실행 파일을 `mode=collector`와 `mode=dashboard` 설정으로 사용한다. 파일 경로는 WASI 마운트 경로로 변환하지 않고 네이티브 보조 명령에 절대 경로로 전달한다.

collector는 시작 권한을 받은 뒤 화면에서 숨겨진다. 대시보드 화면을 닫거나 클라이언트가 detach해도 타이머 수집이 남는다. 모든 수집기 세션을 종료하면 주기적 탐지도 멈춘다. 연결된 Claude 훅 자체는 저장소를 직접 갱신할 수 있다.

## 호스트 인터페이스와 의존성 주입

호스트 시작점은 `SystemCommandRunner`, `ZellijCli`와 SQLite Repository를 생성하고 `HostDependencies`로 명령 처리 함수에 전달한다. 기능 로직은 구체적인 CLI 구현을 생성하거나 전역 실행기에 접근하지 않는다.

`TerminalHost`는 terminal pane 목록과 화면 조회, 문자·바이트 입력, 종료, 생성, 변경 통지를 제공한다. `SessionId`, 불투명한 `PaneId`, `TerminalPane`, `NewPane`을 사용하며 Zellij JSON, `terminal_N`, CLI 옵션은 계약에 포함하지 않는다. 목록에는 terminal pane만 포함한다. 생성 옵션은 cwd, 제목, floating, close-on-exit, no-focus, 프로그램과 argv다. `notify_changed`는 이벤트 ID를 전달하는 최선의 통지이며 저장된 상태가 기준이다.

`ZellijCli`는 실행 파일 경로와 `CommandRunner`를 주입받고 Zellij 명령 조립과 응답 파싱을 담당한다. `CommandRunner`는 Zellij 외에도 git, ps, zoxide, 자식 worktree 셸 명령을 실행한다. `CommandSpec`은 프로그램·argv·cwd·환경 변수 설정/제거·선택적 timeout·스트림별 출력 제한·수집/폐기 모드를 정의한다. 실행기는 종료 상태와 stdout/stderr를 반환하며 실행 오류, timeout, 출력 초과를 구분한다. timeout과 출력 초과 시 자식을 종료하고 회수한다. argv는 셸에 재해석하지 않고 직접 전달한다. 사용자가 요청한 셸 명령과 EDITOR 실행만 기존 셸 경로를 유지한다.

입력의 bracketed paste와 Enter는 별도 호출이며 그 사이에 대상 실행 세대를 재검증한다. 중복 요청, 고정 대상 보호, 불명확한 결과의 자동 재전송 금지와 통지 실패 무시 정책은 유지한다. 기존 timeout과 출력 제한을 보존하며 ps는 timeout 없이 64 MiB 제한과 locale 설정을 사용한다.

상태 파일과 외부 JSON 계약은 기존 숫자 pane ID를 유지한다. 호스트 경계의 변환 함수가 이를 공통 pane 식별자로 연결하므로 스키마 이관은 필요하지 않다. 다른 실행 환경을 지원할 때 `TerminalHost` 구현은 교체할 수 있지만, 전체 앱 이관에는 이 숫자 ID 호환 경계, Zellij 환경 변수와 서버 프로세스에 기반한 발견 방식, 플러그인 SDK 기반 화면 이동의 추가 변경이 필요하다. 파일·환경 접근과 실행 스크립트는 이번 주입 범위에 포함하지 않는다.

## 공유 상태와 동시성

기본 저장 위치는 `${XDG_STATE_HOME:-$HOME/.local/state}/zellij-agent-dashboard`이며 파일명은 `store.sqlite3`다. 실행 스크립트의 `ZAD_STATE_DIR`와 호스트의 `--state-dir`로 경로를 지정할 수 있다. SQLite는 네이티브 host에만 포함하며 플러그인은 기존 JSON 명령 응답을 사용한다. 별도 서버나 영구 연결을 추가하지 않는다.

`Repository`는 snapshot, agent, request, recent_requests, catalog, runtime의 용도별 조회를 제공한다. snapshot은 메타데이터·에이전트·활동만 읽으며 요청 이력을 읽지 않는다. agent와 request는 지정한 ID를 조회하고 recent_requests는 시각 내림차순·ID 오름차순 인덱스로 제한된 개수를 읽는다. 각 조회는 하나의 읽기 트랜잭션에서 일관된 결과를 반환한다. 전체 `read()`는 이관·검증용으로 유지한다. `Repository::begin()`은 갱신용 `UnitOfWork`를 반환하며 `commit()` 없이 종료하면 롤백한다. 호출부에는 `HostDependencies`로 Repository를 주입한다. 상태 전이·출처 선택·실행 세대 검증은 Rust 코어에 남고 SQL, 연결, 파일 권한과 이관은 호스트 어댑터에 둔다. 편집기에서 사용하는 `edit-*.txt`는 외부 에디터용 파일이므로 DB 상태와 별도로 유지한다.

DB는 `metadata`, `agents`, `activities`, `requests`, `launches`, `recent_directories` 테이블을 사용한다. 식별자/순서를 키로 삼고 각 레코드의 내용은 JSON 컬럼에 보존한다. 코어의 전체 스냅샷을 읽어 전이한 뒤 변경된 레코드만 추가·갱신·삭제한다. 요청 테이블에는 정렬용 `at_ms` 컬럼과 `requests_recent` 인덱스를 두며 JSON의 시각과 함께 갱신한다. 갱신 트랜잭션은 현재 전체 Store 비교 방식을 유지한다. 부분 조회 결과는 이 저장 경로에 전달하지 않는다. `metadata`는 revision, 마지막 스캔 시점, 수집 예약과 도메인 스키마를 포함한다.

WAL과 `synchronous=FULL`을 사용한다. 조회는 작성자 예약 없이 수행하며 갱신은 `BEGIN IMMEDIATE`로 직렬화한다. SQLite 잠금 대기와 초기 WAL 설정의 BUSY 재시도는 각각 최대 2초다. 여러 세션·클라이언트의 collector가 같은 DB를 사용하며 별도의 작성자 선출을 두지 않는다. 상태 디렉터리는 로컬 파일시스템에 둔다. [SQLite WAL 문서](https://www.sqlite.org/wal.html)

공유 마지막 스캔 시점으로 프로세스 탐지를 1.8초 이상 간격으로 제한한다. `scan_lease`의 토큰과 10초 만료 시각으로 중복 collector의 동시 수집을 제한한다. 실패 시 자신의 예약을 해제하고, helper 중단 시 만료 후 다음 collector가 이어받는다. 메타데이터·화면·프로세스 읽기는 갱신 트랜잭션 밖에서 수행하고, 화면 조회 후 프로세스 전체 목록을 다시 확인한다. 만료되거나 교체된 예약의 결과는 저장하지 않는다. 훅은 갱신 트랜잭션 안에서 현재 실행 ID를 확인하고 순서 번호를 부여해 반영한다. 종료 조작의 마지막 고정 보호 검사와 pane 종료는 기존과 같이 직렬화한다.

저장소 디렉터리는 700, DB와 WAL 보조 파일은 600 권한이다. DB 구조 버전은 SQLite `user_version=2`이고 도메인 Store와 스냅샷 스키마는 3, 이벤트 JSON은 1을 유지한다. 도메인 스키마와 DB 구조 버전은 별도로 관리한다. 손상된 DB, 알 수 없는 DB 구조 버전, 더 새로운 도메인 스키마는 재초기화하거나 덮어쓰지 않고 오류를 반환한다.

DB 구조 버전 1은 작성자 트랜잭션에서 요청의 `at_ms` 컬럼을 추가하고 기존 JSON 시각을 채운 뒤 인덱스를 생성해 2로 이관한다. 도메인 버전과 요청 JSON이 유효한지 확인하며 실패하면 컬럼·인덱스·버전 변경을 롤백하고 기존 내용을 보존한다. 동시 이관은 잠금 안에서 버전을 재확인한다. DB 버전 1 바이너리로 롤백하려면 이관 전 DB 백업을 복원한다. 이 변경에서 도메인·스냅샷·이벤트 JSON 버전은 올리지 않는다.

DB 최초 초기화 시 같은 디렉터리의 `store.json`을 읽어 하나의 트랜잭션으로 이관한다. JSON이 없으면 `Store::default()`로 시작한다. 동시 초기화는 작성자 잠금 안에서 버전을 재확인해 이관을 한 번만 수행한다. JSON과 기존 `store.lock`은 보존하며 성공 이후에는 SQLite만 기준으로 사용한다. 이관 실패는 테이블과 버전 갱신까지 롤백하고 원본 JSON을 유지한다. 기존 JSON 스키마 1은 출처 이관을 거쳐 3으로, 스키마 2는 pane 확인 정보의 기본값을 추가해 3으로 변환한다. 기존 pane 제목만으로 현재 존재를 확정하지 않으며 `pane.presence=unknown`, `pane.observed_at_ms=0`으로 시작한다. DB에 저장된 스키마 2도 읽을 때 자동 이관하고 다음 쓰기에 3으로 저장한다. 이전 바이너리는 스키마 3을 거부하므로 host와 플러그인을 함께 갱신하고, 롤백은 업그레이드 전 DB 백업을 사용한다. 순서가 있는 실행의 상태·보고 시각·훅 출처를 보존하고 순서가 없는 발견 실행은 화면 감지를 시작한다. 수집 예약의 기본값은 없음이다. 참고 프로젝트에서 사용하던 SQLite DB는 읽지 않는다.

업그레이드 전에 이전 host를 사용하는 collector와 훅 실행을 중단해야 한다. 구버전 JSON 작성자와 신버전 SQLite 작성자를 동시에 운영하지 않는다. 백업은 관련 collector와 훅을 중단한 뒤 상태 디렉터리 전체를 복사하거나 SQLite의 온라인 백업 도구로 수행한다. 실행 중 `store.sqlite3`만 복사하면 WAL의 최신 변경을 빠뜨릴 수 있다. 복구는 실행을 중단하고 DB와 `-wal`, `-shm`을 함께 백업·이동한 뒤 정상 DB를 복원한다. 정상 JSON으로 재이관하려면 DB 세 파일을 이동하고 JSON을 복원한 뒤 시작한다. 이전 바이너리로 롤백할 때는 보존한 JSON을 사용하며, 이관 이후 DB에서 발생한 변경은 그 JSON에 포함되지 않는다.

## 실행 식별과 상태

에이전트 ID는 세션 이름, 세션 실행 세대, pane ID, 프로세스 실행 세대의 JSON 튜플이다. 세션 세대는 Zellij 서버 PID와 시작 시각, 프로세스 세대는 에이전트 PID와 시작 시각을 사용한다. 원본 이름을 그대로 보존하므로 공백과 한국어 이름으로 세션을 찾을 수 있다. OS 프로세스 시작 시각의 해상도는 초 단위다.

`ps`에서 실행 파일과 Zellij 환경을 확인하고 중첩된 도구 런처 중 가장 깊은 실행 프로세스를 등록한다. Codex의 `app-server` 하위 명령은 native/Node 런처 모두 발견 대상에서 제외한다. 성공한 전체 목록과 현재 보조 명령의 존재를 확인한 경우만 생존 정보를 갱신한다. 현재 macOS에서 실제 실행을 검증했다. 훅 없는 `cwd`는 상속된 `PWD`이며 실제 훅의 `cwd`가 보고되면 갱신한다.

상태는 `found`, `idle`, `working`, `waiting`, `done`, `failed`, `compact`다. 생존 여부 `live/gone/unverified`와 60초 이후의 `stale` 표시는 작업 상태와 분리한다. 시간 경과나 프로세스 존재만으로 완료·진행을 추정하지 않는다. 프로세스 검증이 10초 이상 없으면 저장된 live 상태를 cached로 표시한다. 종료 기록은 최대 24시간 유지하고 활동 기록은 최근 50개를 보존한다. 기본 목록은 종료되지 않았고 `gone`이 아니며 pane이 `present`로 확인된 실행만 표시한다. 이전에 pane이 확인된 `unverified` 실행은 cached 표시로 유지한다. 종료되거나 숨겨진 부모의 고정·그룹 정보는 살아 있는 자식 목록에 적용하지 않는다.

`pane.presence=unknown|present|missing`은 프로세스 생존과 작업 상태에서 분리한다. 호스트의 성공한 terminal pane 목록 조회로 존재 여부를 기록하고, 목록 실패·수집 시간 제한은 이전 확인 결과를 유지한다. 화면 판별은 확인된 pane에만 수행한다. pane 결과 반영은 전체 실행 identity와 관측 시각을 확인해 교체된 실행과 오래된 조회를 거부한다. 화면·프로세스·pane 조회는 갱신 트랜잭션 밖에서 수행한다. Enter의 resolve와 입력·종료 등 대상 검증은 실제 pane 목록을 다시 확인하고 현재 프로세스 세대를 재검증한다. pane 조회 실패나 부재는 조작을 허용하지 않는다.

이벤트는 버전, event ID, 전체 identity, tool, kind, sequence, observed_at_ms, cwd, summary, detail을 갖는다. 등록되지 않은 실행, 다른 세대, 중복 ID, 이전 순서·보고 시각의 이벤트를 무시한다. SessionEnd 뒤 아직 종료 중인 프로세스를 발견해도 이전 실행을 되살리지 않는다.

## 훅과 변경 통지

Claude 훅은 현재 pane의 실제 Claude 프로세스가 자신의 조상인지 확인한다. 원본 JSON을 공통 이벤트로 변환하고 상태 파일에 먼저 기록한다. `zellij pipe --name agent-dashboard-changed`는 빠른 갱신을 위한 통지이며 전달 실패 시 다음 주기 조회로 수렴한다. 통지 프로세스는 최대 500ms 후 종료한다. 훅 오류는 stderr에 남기고 종료 코드는 0을 유지해 Claude 실행을 막지 않는다.

설정 생성기는 별도 JSON을 출력한다. 사용자 전역 훅을 자동 등록하지 않는다. 제공하는 이벤트는 SessionStart, UserPromptSubmit, PreToolUse, PostToolUse, PostToolUseFailure, PermissionRequest, Notification, Stop, PreCompact, SessionEnd다. Notification은 permission_prompt와 idle_prompt만 받는다. PostToolUseFailure는 도구 결과로 처리하며 전체 턴 실패로 간주하지 않는다. StopFailure와 PostCompact는 파서에만 있고 설치 설정에는 아직 포함하지 않는다.

실제 Claude 검증은 SessionStart만 수행했다. 나머지 이벤트 매핑의 전체 실동작 검증, Codex 및 다른 도구의 훅 어댑터, 화면 내용 탐지는 아래 어댑터가 담당한다.

## 공통 상태 신호와 감지 경로

`StateSignal::Hook(AgentEvent)`와 `StateSignal::Screen(StatusObservation)`은 같은 `Store::apply_signal()` 경계를 사용한다. 사건과 현재 상태 관측의 의미는 유지한다. `Instruction`은 대시보드가 실제 전송한 지시만 저장하며 훅 연결이나 턴 시작으로 간주하지 않는다. 기존 `Store::apply()`와 ingest JSON은 호환 진입점으로 남는다.

에이전트 실행마다 `status_source=unknown|screen|hook`을 저장한다. 발견 후 3초 동안 화면 판별을 유예한다. 유효한 훅을 아직 받지 않은 실행은 화면을 사용하고, 첫 유효한 훅이 도착하면 해당 실행 세대 동안 훅만 사용한다. 훅의 무응답 시간으로 화면에 복귀하지 않는다. 출처 선택은 reload 후에도 유지하고 새로운 프로세스 세대에서는 초기화한다. 훅 지원 도구라는 사실만으로 설치 여부를 확정할 수 없으므로 실제 이벤트 수신을 연결 근거로 삼는다. 연결 해제나 수동 출처 선택 설정은 이번 구현에 포함하지 않는다.

화면 규칙은 참고 저장소의 Claude, Codex, Gemini, Cursor 프로필을 JSON으로 내장한다. 우선순위와 같은 우선순위의 선언 순서, contains/regex/line_regex/all/any/not, 프롬프트·수평 구분선·하단 영역, transcript 등의 상태 보존 규칙을 유지한다. 원본 `blocked`는 `waiting`, `idle`은 `idle`로 변환하며 화면만으로 `done`을 만들지 않는다. Hermes는 규칙이 없으므로 훅이 없으면 `found`를 유지한다. 출처와 라이선스 확인은 [화면 어댑터 출처](screen-adapter-provenance.md)에 기록한다.

일반 pane 제목은 idle 근거로 쓰지 않고 명시적인 working/waiting 제목 규칙만 허용한다. 이번 스캔에서 조회하지 못한 제목은 상태 판별에 쓰지 않는다. Zellij dump-screen/list-panes는 OSC progress를 제공하지 않아 해당 규칙은 활성화되지 않는다.

규칙에 일치하지 않으면 원본과 같이 idle 후보로 처리한다. working에서 명시적인 idle 표시 없이 idle로 바뀌려면 서로 다른 새 관측 3개가 연속으로 필요하다. 원본의 100ms 타이머/700ms 마감은 이관하지 않고 collector 주기를 사용한다. 10초 이상 관측 공백, 보존 overlay, 빈 화면 또는 화면 조회 실패는 후보를 취소한다. 빈 화면·조회 실패는 작업 상태와 마지막 유효 보고 시각을 유지한다. 명시적인 프롬프트 idle은 즉시 반영한다.

화면 조회에는 라운드당 1.2초 예산을 두고 가장 오래 조회하지 않은 실행부터 처리한다. 개별 CLI 조회는 기존 500ms 제한을 사용하므로 마지막 조회는 예산을 최대 500ms 초과할 수 있다. 많은 에이전트는 여러 collector 주기에 나눠 조회한다. 관측 직후 실행 세대를 재검증하고, 코어에서도 전체 identity·생존·출처·관측 ID·시각을 확인한다. 훅과 화면의 순서/시각은 별도로 검증하므로 화면 뒤에 처음 받은 훅이 다른 출처의 순서 때문에 거부되지 않는다. 훅 전환 뒤 늦게 끝난 화면 결과는 무시한다.

## 화면과 권한

각 화면은 선택한 실행 ID, 검색어와 고정/일반 영역별 선택을 따로 갖는다. 정렬이 바뀌어도 같은 실행을 선택한다. 상태 긴급도 정렬, 검색, 크기 제한, 한국어 폭 계산과 제어 문자 제거를 코어에서 처리한다. 세션·탭 그룹과 부모·자식 계층을 표시한다. 고정·별칭을 편집·저장하고, 이 대시보드가 실행한 에이전트는 launch ID로 발견된 프로세스와 연결해 부모 정보를 저장한다. 수동 실행의 부모 관계는 추정하지 않는다. 프로젝트 이름은 행에 표시하며 프로젝트별 독립 그룹은 없다. 현재 활동 화면은 최신 변화 하나를 표시한다.

플러그인은 RunCommands, ReadApplicationState, ChangeApplicationState, ReadCliPipes 권한을 요청한다. 명시적 거절 뒤에는 반복 요청하지 않고 재로딩으로 다시 시작한다. CLI pipe 입출력에도 별도의 ReadCliPipes 권한이 필요하다.

Enter는 보조 명령으로 현재 프로세스를 다시 검증한 뒤 `switch_session_with_focus`를 호출한다. 입력·종료·실행과 보조 메뉴의 외부 조작도 호스트를 경유한다. 요청 ID와 내용을 실행 전에 저장하고 결과를 실행 후 기록한다. 중복 요청은 기존 결과를 반환하며, pending이나 결과가 불명확한 조작을 자동 재전송하지 않는다. 실제 검증은 호스트 fixture 조작을 포함하며 메뉴별 전체 흐름의 검증 여부는 [기능 이관표](feature-map.md#현재-구현과-검증)에 구분한다.

Zellij 0.45.0은 연결된 클라이언트가 없을 때 plugin reload를 거부한다. 수집기 자체의 detach 이후 실행과는 다른 조건이다. 재로딩은 클라이언트를 연결한 상태에서 수행한다. 검증 범위는 [실행 검증 기록](runtime-validation.md)을 따른다.
