# 실행 검증 기록

## 2026-10-28 waiting floating pane 알림 제거

사용자 요청으로 알림 전용 코어·호스트 모듈, collector의 알림 동기화, notification 플러그인 모드와 전용 smoke 스크립트를 제거했다. 빌드·설치에서는 대시보드 WASM과 host만 배포하고 이전 알림 WASM을 정리한다. waiting 상태 감지와 대시보드 표시는 유지하며 저장 구조와 스키마는 변경하지 않는다. 아래 2026-10-23 알림 기록은 제거 전 이력이다.

Rust 포맷, workspace 테스트 122개(기존 수동 테스트 1개 제외), native/WASI Clippy와 host·WASM release 빌드를 통과했다. 실제 Zellij 0.45.0 임시 세션 `nr-cnys0t5n`에서 permission_required를 주입해 waiting 상태 저장과 collector polling 지속, floating plugin 미생성, collector reload 후 waiting 유지와 알림 미생성을 확인했다. 임시 세션은 종료했으며 결과는 `.local/nr-cnys0t5n/result.json`에 남겼다.

`bash scripts/build.sh`, `bash scripts/install.sh`를 실행하고 `zellij-with-code-agent` 세션에 적용했다. reload 후 이전 클라이언트 코드가 남아 collector를 교체했으며 최종 collector ID 11 하나, 권한 승인, Connected, polling 증가와 알림 필드 제거를 확인했다. 열린 dashboard 모드 pane은 없었고 미리보기 알림 pane은 닫았다. 설치본 host·WASM과 dist의 해시 일치, 설치된 알림 WASM 제거를 확인했다. 최종 응답은 `.local/notification-removal-applied.json`에 보존한다. 최초 적용은 현재 세션으로 제한했다. 이후 사용자의 전체 세션 적용 요청에 따라 실행 중인 `운영`, `study`, `hermes`, `zellij-with-code-agent` 네 세션을 모두 갱신·검증했다. 최종 collector ID는 각각 141, 6, 7, 11이며 세션당 하나만 유지한다. 각 세션에서 권한 승인, Connected, polling 증가, 알림 필드 제거와 알림 pane 부재를 확인했다. 열린 dashboard 모드 pane은 없었다. `rea`는 종료 상태라 복원하지 않았다. 전체 적용 결과는 `.local/notification-removal-all-sessions.json`에 보존한다.

## 2026-10-23 새 에이전트 전체 권한 실행

`n`과 자식 worktree 생성의 공통 host 실행 경로에 도구별 승인 생략 옵션을 추가했다. Codex는 `--dangerously-bypass-approvals-and-sandbox`, Cursor는 `--force --sandbox disabled --approve-mcps --trust`, Claude·agy는 `--dangerously-skip-permissions`, Hermes는 `--yolo`, Pi는 프로젝트 파일 신뢰를 위한 `--approve`를 전달한다. 설치된 CLI의 `--help`로 옵션을 확인했다. Pi 확장 기능의 독자적인 승인은 우회하지 않는다. 기존 실행에는 소급 적용되지 않으며 저장 구조·스키마·복구 절차는 변경하지 않는다.

Rust 포맷 검사, 코어 49개·호스트 68개 테스트와 native workspace Clippy를 통과했다(기존 수동 테스트 1개 제외). host와 `wasm32-wasip1` 플러그인을 release 빌드했다. 전체 `scripts/smoke.py`는 이번 변경의 생성 검사에 도달하기 전 `initial dashboard selection prefers current session`에서 시간 초과했다. 전체 smoke 통과로 간주하지 않는다. 별도 `.local/full-permission-runtime.py`는 실제 Zellij 0.45.0 임시 세션에서 모델 요청 없이 실행 파일 fixture를 사용해 6개 도구의 pane 내부 argv, terminal_command와 중복 요청의 pane 재사용을 확인했다. 결과는 `.local/full-permission-runtime.json`, 로그는 `.local/full-permission-runtime.log`이며 임시 세션을 종료했다. 같은 argv 검사를 정규 smoke에도 추가했다.

검증 후 `bash scripts/build.sh`, `bash scripts/install.sh`로 설치했다. 사용자의 전체 세션 적용 요청 시 실행 중이었던 `hermes`는 다른 작업의 적용 진행 중 종료되어 복원하지 않았다. 최종 실행 목록의 유일한 운영 세션 `zellij-with-code-agent`에서 기존 collector ID 0·66을 reload하고 권한 승인·오류 없음·Connected·poll 및 revision 증가를 확인했다. 다른 작업이 교체한 collector를 포함한 기존 두 인스턴스는 유지했고 추가 생성하지 않았다. 열린 dashboard 모드 인스턴스는 없었으며, 새로 열 때 설치본을 사용한다. dist와 설치된 host·두 WASM의 SHA-256 일치를 확인했다. 결과는 `.local/apply-full-permission-all.json`, 최종 로그는 `.local/apply-full-permission-final.log`다.

## 2026-10-23 waiting 우상단 알림창 (2026-10-28 제거)

기존 collector가 전체 세션의 도달 가능한 live waiting 실행을 집계해 각 탭 우상단에 40열 × 4행의 테두리 없는 pinned floating plugin pane을 유지한다. 알림은 포커스를 가져가지 않으며, 최대 세 항목과 대기 개수를 표시한다. 대기가 많으면 마지막 행에 나머지 개수를 표시한다. 알림 plugin은 권한 요청·호스트 실행·상태 수집 없이 호스트가 전달한 요약만 표시한다. 상태 스키마는 4로 유지한다.

`bash scripts/check.sh`에서 코어 49개·호스트 68개·플러그인 4개 테스트(총 121개), 포맷, native/WASI Clippy를 통과했다. 수동 성능 테스트 1개는 기존과 같이 제외했다. `bash scripts/build.sh`로 host와 두 WASM을 빌드했다.

`python3 scripts/notification-smoke.py`로 실제 Zellij 0.45.0의 격리된 임시 세션 `notification-pefgdhlh`에서 다음을 통과했다. 테스트의 프로세스 목록은 이 임시 세션으로 제한해 다른 사용자 세션의 waiting 개수가 섞이지 않게 했다.

- waiting 생성 시 대기 개수의 실제 터미널 출력, 120열 화면에서 x=80·y=0·40×4 크기, 기존 터미널 포커스 유지와 알림 비선택 상태
- 두 클라이언트·8개 동시 helper 요청에서 중복 생성 없음
- collector reload 뒤 기존 알림 pane 재사용, 새 탭에 하나씩 생성
- 100열 화면으로 resize 후 오른쪽 끝 유지
- waiting 해소 시 모든 알림 닫기, 다시 waiting이면 재생성, 실행 종료 시 닫기

기술 검증 중 `x=100%`는 창 너비를 0으로 만들 수 있음을 확인했다. 타일 pane들의 실제 전체 너비에서 알림 너비를 뺀 셀 좌표를 사용하며, 작은 화면에서는 알림 너비를 화면 너비로 제한한다. 이 동작은 별도 호스트 회귀 테스트로 고정했다. 임시 세션은 종료했으며 결과는 `.local/notification-pefgdhlh/result.json`, `.local/notification-pefgdhlh/notice-pane.json`, `.local/notification-smoke.log`에 보존한다. 전체 기존 smoke 통과를 주장하지 않는다.

기존 `zellij-with-code-agent` 서버에서 SDK `shim.rs:2037`의 동기 `get_session_list()` 응답 decode가 panic을 일으키고 이후 RefCell 접근이 연쇄 실패하는 것을 로그로 확인했다. 동기 조회 세 곳을 제거하고 `ModeUpdate`/`SessionUpdate`로 현재 세션을 받도록 수정했다. 세션 정보 도착 전 창 열기·이동은 보류한다. 이벤트의 현재 세션 표시·이름 변경 보존을 테스트했다. 수정 뒤 알림 검증 전체를 다시 통과했고, 임시 `window-xwomgq4f` 세션에서 실제 Alt q 열기·반복 열기·두 클라이언트 중복 방지·collector reload 뒤 재열기를 통과했다. 결과는 `.local/notification-window-check.log`다. 기술 검증용 세션은 모두 종료했다.

검증 후 `bash scripts/build.sh`와 `bash scripts/install.sh`로 최종 host·대시보드 WASM·알림 WASM을 다시 빌드·설치했다. 최초 알림 버전은 `loop`, `study`, `asbplayer`, `운영`, `hermes`, `rea`에서 활성화와 polling 증가를 확인했다. 현재 `zellij-with-code-agent`에서는 panic 상태로 남은 기존 collector와 검증용 collector를 닫고 최종 collector 하나(ID 69)로 교체했으며, 권한 부여 후 `waiting_notifications=true`, `session_events=true`, 올바른 현재 세션 이름과 polling 증가를 확인했다. 기존 에이전트 pane은 종료하지 않았다. 현재 세션 확인 결과는 `.local/notification-current-verified.json`이다.

최종 재적용 시 나머지 여섯 세션의 CLI 연결 소켓이 조회되지 않아 최종 수정본의 reload 재확인은 수행하지 못했다. 저장된 세션 기록과 일부 서버 프로세스는 남아 있으나 CLI는 해당 세션을 찾지 못한다. 이 제한을 전체 일곱 세션의 최종 재검증 성공으로 표현하지 않는다. 세션 재시작·복원과 에이전트 재실행은 수행하지 않았다. 앞선 적용 로그는 `.local/notification-apply.log`, `.local/notification-retry.log`, `.local/notification-debug-hermes.log`, 최종 시도 로그는 `.local/notification-apply-final.log`에 남겼다.

## 2026-10-23 idle 순환의 모든 idle 전환 최신순 반영

idle 후보 정렬을 `working_to_idle_at_ms`에서 `status_since_ms` 내림차순으로 변경했다. 기존 기준은 working→idle만 기록해 waiting→idle이나 found→idle이 최신이어도 앞에 오지 않는 문제가 있었다. 이제 수락한 모든 idle 전환을 반영하며 반복 idle·중복·역순 관측은 시각을 갱신하지 않는다. waiting 우선과 현재 후보 다음부터 순환하는 동작은 유지한다. 기존 저장 필드를 사용하므로 스키마 변경·별도 이관은 없다.

`bash scripts/check.sh`에서 코어 48개·호스트 66개·플러그인 3개 테스트, 포맷과 native/WASI Clippy를 통과했다(수동 성능 테스트 1개 제외). `bash scripts/build.sh`로 release host/WASM을 빌드했다. `.local/idle-transition-runtime.py`로 실제 Zellij 0.45.0 임시 세션에서 waiting 우선, idle 복귀, 최근 waiting→idle이 오래된 working→idle보다 먼저 선택되는 실제 Alt i 이동과 다음 키에서 오래된 idle로의 순환을 확인했다. 최신 idle의 이전 working→idle 기록이 상대 후보보다 오래됐음을 함께 검사해 두 정렬 기준을 구분했다. 전체 smoke 통과를 주장하지 않으며, 앞 항목에 기록된 최초 선택 검사 문제를 피한 기능 검증이다. 임시 세션은 종료했다. 결과는 `.local/smoke-34cc7f7729/idle-transition-result.json`, 로그는 `.local/idle-transition-runtime.log`다.

`bash scripts/install.sh` 후 현재 작업 세션 `loop`의 collector ID 0을 reload했다. 설치 host/WASM과 dist의 SHA-256 일치, permissions=true, error=null, polls 2→6와 revision 증가를 확인했다. 열린 대시보드는 없으며 다음 열기부터 적용된다. 임시 연결은 정리했다. 적용 결과는 `.local/apply-idle-transition-loop.json`이다.

사용자의 전체 세션 적용 요청에 따라 실행 중인 `study`, `loop`, `asbplayer`, `운영`, `hermes`, `zellij-with-code-agent`, `rea` 7개 세션의 collector ID 0을 모두 reload했다. 각 세션에서 중복 collector 없이 기존 pane ID 유지, permissions=true, error=null, polls와 revision 증가를 확인했다. 열린 대시보드는 없으며 다음 열기부터 최신 동작을 사용한다. 종료된 임시 검증 세션은 복원하지 않았다. 검증용 임시 클라이언트는 정리했다. 설치본과 dist의 SHA-256 일치를 확인했으며 결과는 `.local/apply-idle-transition-all.json`, 로그는 `.local/apply-idle-transition-all.log`다.

## 2026-10-23 idle 이동의 waiting 최우선 처리

`idle-and-pinned`와 `idle-and-unpinned`는 해당 패널의 유효한 `waiting` 실행이 있으면 waiting만 순환한다. waiting이 없으면 기존 최근 working→idle 전환 순서로 idle을 순환한다. 부모 고정 상속과 종료·pane 부재·생존 미확인 실행 제외를 유지한다. 저장 구조 변경은 없다.

`bash scripts/check.sh`에서 코어 48개·호스트 66개·플러그인 3개 테스트, 포맷과 native/WASI Clippy를 통과했다(기존 수동 성능 테스트 1개 제외). `bash scripts/build.sh`로 release host/WASM을 빌드했다. 전체 smoke는 기존 최초 선택 검사에서 시간 초과했다(`.local/idle-waiting-smoke.log`). 해당 검사 대신 dashboard 준비만 기다리는 별도 검증 스크립트 `.local/idle-waiting-runtime.py`로 실제 Zellij 0.45.0 임시 세션의 Alt i waiting 우선·단일 waiting 재선택·waiting 해제 후 idle 복귀를 통과했다. 전체 smoke 통과로 간주하지 않는다. 결과는 `.local/smoke-6ca3ecc4ef/idle-waiting-result.json`, 로그는 `.local/idle-waiting-runtime.log`이며 임시 세션은 종료했다.

`bash scripts/install.sh`로 설치하고 현재 작업 세션 `loop`의 collector ID 0을 reload했다. 설치 host/WASM과 dist의 SHA-256 일치, permissions=true, error=null, polls 2→4와 revision 증가를 확인했다. 열린 대시보드는 없어 다음 열기부터 적용된다. 임시 연결 클라이언트는 정리했다. 결과는 `.local/apply-idle-waiting-loop.json`이다.

## 2026-10-16 대시보드 최초 선택

첫 비어 있지 않은 목록에서 현재 Zellij 세션의 첫 에이전트를 선택한다. 현재 세션의 에이전트가 없으면 전체 표시 순서의 첫 항목을 선택하며, 고정 항목이면 고정 영역으로 초점도 이동한다. 이후 상태 갱신에서는 사용자의 선택을 유지한다. 권한 승인 후 세션 목록으로 현재 세션을 확인하며, 세션 이름이 없는 ModeUpdate로 이미 확인한 이름을 지우지 않는다. 저장 구조는 변경하지 않았다.

`bash scripts/check.sh`의 코어 37개·호스트 55개 테스트, 포맷·native/WASI Clippy와 스크립트 검사를 통과했다(기존 호스트 성능 테스트 1개 제외). `bash scripts/build.sh`로 release host/WASM을 빌드했다. 실제 Zellij 0.45.0 임시 세션에서 고정·일반 영역의 현재 세션 우선 선택과 현재 세션 부재 시 첫 항목 선택을 고정 snapshot 응답으로 검증했다. 결과는 `.local/initial-focus-8a7fcfb356/result.json`이며 임시 세션은 종료했다. 전체 smoke는 두 번째 임시 세션의 collector 권한 승인 대기에서 중단됐으므로 전체 회귀 통과로 기록하지 않는다(`.local/initial-focus-smoke.log`).

`bash scripts/install.sh`로 설치하고 dist와 설치 host/WASM의 SHA-256 일치를 확인했다. 현재 작업 세션 `loop`의 collector만 reload해 기존 plugin ID 유지, permissions=true, error=null, 상태 revision·poll 증가를 확인했다. 열린 대시보드는 없어 다음 열기부터 적용된다. 임시 연결 클라이언트는 종료했다. 적용 결과는 `.local/apply-initial-focus-loop.json`이다.

이어 사용자의 전체 세션 적용 요청에 따라 실행 중인 `study`, `loop`, `운영`, `zellij-with-code-agent` 네 세션의 collector를 모두 reload했다. 각 세션에서 기존 plugin ID 0 유지, 중복 없음, permissions=true, error=null, revision·poll 증가를 확인했다. 열린 대시보드는 없었으며 다음 열기부터 최신 동작을 사용한다. 종료 상태인 `hermes`는 복원하지 않았다. 임시 연결 클라이언트는 종료하고 기존 연결은 유지했다. 결과는 `.local/apply-initial-focus-all.json`이다.

## 2026-10-14 Alt w의 idle 제외 필터

전역 이동에 `non-idle` 필터를 추가했다. 표시 가능하고 생존이 확인된 실행 중 `idle`만 제외하며 `working`, `waiting`, `compact`, `done`, `failed`, `found`를 포함한다. `working-only`와 대시보드 `Tab`은 기존 working 전용 동작을 유지한다. 저장 구조와 버전은 바뀌지 않는다.

`bash scripts/check.sh`에서 코어 34개·호스트 55개 테스트, 포맷, native/WASI Clippy와 스크립트 검사를 통과했다(수동 성능 테스트 1개 제외). 새 코어 검사는 모든 비-idle 상태, 고정 여부, 순환과 idle만 남았을 때 빈 결과, 종료·부재·생존 미확인 실행 제외를 확인한다. 샌드박스 안에서는 기존 프로세스 조회 테스트가 실패하여 권한을 허용한 환경에서 재실행했다. 로그는 `.local/non-idle-check.log`다.

`python3 scripts/smoke.py`를 통과했다. 실제 Alt w로 working·waiting pane 이동과 기존 Tab·idle 필터·멀티 클라이언트·detach/reload 후 세션 간 이동을 확인했다. 결과는 `.local/smoke-9071d9f039/result.json`, 로그는 `.local/non-idle-smoke.log`다. 첫 시도는 기존 화면 idle 확인 단계에서 시간 초과했고, 두 번째는 전역 탐색의 테스트 시작 위치가 다른 세션으로 순환하여 실패했다. 비에이전트 pane에서 탐색을 시작하도록 테스트를 수정한 뒤 통과했다. 소유한 임시 세션은 종료했다.

검증 후 `bash scripts/build.sh`와 `bash scripts/install.sh`를 실행했다. 승인받은 운영 세션 `study`, `zellij-with-code-agent`, `loop`, `hermes`, `운영`의 기존 collector ID 0을 reload하고 설치본 host/WASM과 dist의 SHA-256 일치, permissions=true·error=null과 polls/revision 증가를 확인했다. 열린 대시보드는 없었다. `zellij-with-code-agent`에서는 임시 클라이언트의 초기 조회가 완료되지 않아 첫 검증이 시간 초과했고, 재실행에서 기존 연결 클라이언트의 reload 후 정상 수집을 확인했다. 적용 결과는 `.local/apply-non-idle-all.json`, 재확인 결과는 `.local/retry-non-idle.json`이다. `/Users/in05908_mac/.config/zellij/config.kdl`에서 Alt w의 payload를 `working-only`→`non-idle`로 변경하고 `zellij setup --check`를 통과했다. 설정 원본은 `.local/non-idle-config-before.kdl`에 보존했다.

## 2026-10-14 Alt w로 working 전체 순환

기존 `working-only` 필터를 전역 `Alt w`에 연결했다. 고정 여부와 관계없이 정확히 `working` 상태인 실행만 순환하며 `waiting`, `compact`, `done`, `idle`은 포함하지 않는다. 새 도메인 상태나 저장 필드는 추가하지 않았다. README 키 표와 기능 이관표를 갱신하고 사용자 `/Users/in05908_mac/.config/zellij/config.kdl`의 `shared_except "locked"`에 `mode=collector`, `name=agent-next`, `payload=working-only` 바인딩을 추가했다. `zellij setup --check`에서 설정 문법을 확인했다. 기존 설정 백업은 `.local/working-next-config-before.kdl`이다.

`bash scripts/check.sh`에서 코어 33개·호스트 55개 테스트, 포맷, native/WASI Clippy와 스크립트 검사를 통과했다. 수동 성능 테스트 1개는 제외했다. `bash scripts/build.sh` 후 `python3 scripts/smoke.py`를 통과했다. 소유한 임시 세션에서 실제 Alt w 키로 다음 working pane 이동, working-only의 idle 제외와 고정 해제된 working 실행 선택을 확인했다. 결과는 `.local/smoke-53e4dc3f63/result.json`, 로그는 `.local/working-next-smoke.log`다. 임시 세션은 종료했다.

`bash scripts/install.sh` 후 앞서 사용자가 지정한 전체 운영 세션 `study`, `zellij-with-code-agent`, `loop`, `hermes`, `운영`의 기존 collector ID 0을 reload했다. 재시작된 인스턴스의 permissions=true·error=null과 polls/revision 증가를 확인했고, 설치본 host/WASM과 dist의 SHA-256 일치를 확인했다. 열린 대시보드는 없었다. 검증용 임시 연결은 정리했으며 적용 결과는 `.local/apply-working-next-all.json`, 로그는 `.local/apply-working-next-all.log`다.

## 2026-10-14 메모리 누적과 외부 명령 자원 회수

외부 명령을 별도 프로세스 그룹으로 실행하고 오류 시 그룹 종료·직접 자식 회수·nonblocking 출력 읽기 취소·스레드 join을 수행하도록 수정했다. collector 탐색 큐는 대기 32건으로 제한하고 초과 키는 버린다. 종료 실행 이력은 최대 24시간·256건으로 정리하며 UI 스냅샷에서 ended/gone 실행을 제외한다. 저장 필드와 Store/스냅샷 3·DB 구조 2·이벤트 1은 유지한다.

`bash scripts/check.sh`에서 코어 33개·호스트 55개 테스트와 포맷, native/WASI Clippy, 스크립트 검사를 통과했다. 수동 성능 테스트 1개는 제외했다. 타임아웃·출력 한도 발생 후 하위 프로세스 종료, writer가 열린 상태의 읽기 스레드 취소, 큐 초과 10,000회·FIFO·복구, 종료 이력 상한·TTL·살아 있는 실행 보존을 검증했다. 임시 재현 프로그램에서 타임아웃 6회 후 FD가 수정 전 4→16개였고 수정 후 4→4개로 유지됐다. 로그는 `.local/memory-check.log`, `.local/memory-fd-audit.log`다.

`python3 scripts/smoke.py`를 통과했다. 임시 세션 `zad-smoke-01a6a031ec`, `zad renamed 01a6a031ec`에서 상태 감지·세션 이름 변경·중복 조작·멀티 클라이언트·detach·reload·탐색을 검증하고 종료한 실행이 UI 스냅샷에 포함되지 않는 검사를 추가했다. 결과는 `.local/smoke-01a6a031ec/result.json`, 로그는 `.local/memory-smoke.log`다. 임시 세션은 종료했다.

검증 후 `bash scripts/build.sh`와 `bash scripts/install.sh`를 실행했다. 설치본 host/WASM과 dist의 SHA-256 일치를 확인했다. 설치본을 사용하는 임시 세션 `zad-memory-d806ddadf9`에서 호스트 탐색 응답을 4초 지연하고 실제 탐색 키 200개를 보내 대기 큐 상한 32건, 지연 해제 후 큐 소진, collector reload, 대시보드 열기와 상태 조회를 확인했다. 최초 설치본 reload 검사는 권한 승인 대기로 시간 초과했고, 새 임시 세션에서 권한을 승인한 뒤 재검증해 통과했다. 결과는 `.local/memory-runtime-d806ddadf9/result.json`, 로그는 `.local/memory-runtime.log`다. 임시 세션은 종료했다. 사용자가 지정한 운영 `loop` 세션에 설치본을 반영했다. 열린 대시보드는 없었으며 임시 클라이언트를 연결해 기존 alias `agent-dashboard`, 설정 `mode=collector`, plugin ID 0을 유지하여 reload했다. 재시작된 인스턴스의 polls 2→8, revision 1126744→1126763, permissions=true, error=null과 collector 숨김을 확인했다. 새 pane 생성 없이 반영했으며 검증용 클라이언트 연결을 정리했다. 적용 결과는 `.local/apply-memory-loop.json`, 로그는 `.local/apply-memory-loop.log`다.

추가 사용자 요청에 따라 당시 실행 중인 전체 5개 세션 `study`, `loop`, `zellij-with-code-agent`, `hermes`, `운영`에도 같은 설치본을 적용했다. 각 세션의 기존 collector ID 0과 alias/configuration을 유지해 reload하고, 재시작 후 polls와 revision 증가·permissions=true·error=null·숨김 상태를 확인했다. 열린 대시보드는 없었으며 새 pane은 생성하지 않았다. `hermes`와 `운영`은 최초 연결 직후 ping이 시간 초과됐으나 응답 대기를 포함해 재시도하여 반영과 상태 갱신을 확인했다. 검증용 임시 클라이언트는 모두 정리했으며 기존 클라이언트는 유지했다. 종료된 세션은 되살리지 않았다. 세션별 적용 결과는 `.local/apply-memory-all.json`, 로그는 `.local/apply-memory-all.log`다.

## 2026-10-14 세션 이름 변경 후 auth 이동 복구

`auth` 실행은 서버 PID 43075의 세션 이름이 `likable-glockenspiel`에서 `study`로 변경된 뒤에도 옛 이름으로 수집됐다. 서버 argv와 기존 에이전트 환경변수는 그대로인데 socket 파일은 `study`로 변경된 것을 확인했다. macOS의 `LOCAL_PEERPID`로 실제 socket의 서버 PID를 확인하고 서버 조상으로 에이전트를 연결하도록 수정했다. Linux는 `SO_PEERCRED` 경로를 구현했으며 이번 실검증은 macOS/Zellij 0.45.0이다.

`bash scripts/check.sh`에서 코어 31개·호스트 53개 테스트, 포맷, 네이티브/WASI Clippy와 스크립트 검사를 통과했다. 수동 성능 테스트 1개는 제외했다. 이름 변경과 옛 이름 재사용 시 서버 구분, 감지 출처·작업 상태·고정·별칭 유지, 원래 이름으로 복귀, 이전 조작 대상과 늦은 pane 관측 거절을 검증했다. `bash scripts/build.sh`로 release host/WASM을 빌드했다. 로컬 rustup proxy 경로가 없어 기존 `/opt/homebrew/bin/rustup`과 1.88.0 toolchain bin 경로를 환경변수로 지정했다.

`python3 scripts/smoke.py`를 소유한 임시 세션 두 개에서 통과했다. 두 번째 세션을 에이전트 실행 후 이름 변경하고 새 이름의 발견·resolve, 실제 Enter 세션 이동·원래 대시보드 닫힘, collector 유지와 reload를 확인했다. 결과는 `.local/smoke-33937820e5/result.json`, 전체 로그는 `.local/smoke-auth.log`이며 임시 세션은 종료했다.

`bash scripts/install.sh`로 설치하고 dist와 설치 host/WASM의 SHA-256 일치를 확인했다. 적용 대상은 `study`와 `zellij`이며 기존 collector plugin ID 0으로 reload하고 permissions=true, error=null, revision 갱신을 확인했다. 설치 host로 실제 `auth`가 `study`의 terminal pane 1로 resolve되는 것을 확인했다. `zellij`에 설치본 대시보드를 열고 `/study` 검색으로 실제 auth 행을 선택한 뒤 Enter로 `study`의 auth pane 이동과 원래 대시보드 닫힘을 확인했다. 반영 검증용 클라이언트는 종료했다. 적용 결과와 로그는 `.local/apply-auth.json`, `.local/apply-auth.log`다. 저장 형식 변경은 없다.

2026년 10월 7일 macOS에서 Zellij 0.45.0, Rust 1.88.0, Claude Code 2.1.70으로 첫 조회 버전을 검증했다. 임시 세션을 사용했으며 기존 사용자 세션의 pane이나 플러그인을 교체하지 않았다.

## 자동 검사

`./scripts/check.sh`로 Rust 포맷, 단위 테스트 17개, 네이티브와 wasm32-wasip1 Clippy, 셸 문법과 Python 구문을 확인했다. `./scripts/build.sh`는 호스트 실행 파일과 실제 `_start`를 갖는 WASI 플러그인을 빌드한다.

단위 테스트는 이벤트 중복·역순, pane 실행 세대 교체, SessionEnd 종료 경쟁, 캐시 복원과 생존 검증, 늦은 도구 결과, 동시 저장소 작성, 손상 파일 보존, 검색·정렬 중 선택 유지, 한국어 폭과 스크롤을 포함한다.

## 실제 Zellij 검증

`python3 scripts/smoke.py --real-claude`는 다음을 확인한다.

| 항목 | 확인한 동작 |
|---|---|
| 백그라운드 로딩 | 동일 WASM의 collector와 dashboard 설정 구분, 최초 권한 승인 |
| 화면 종료 | dashboard pane 종료 이후 collector 타이머 계속 증가 |
| 다중 클라이언트 | 두 클라이언트 연결 중 공유 상태 조회와 수집 |
| 여러 세션 | 공백 있는 이름을 포함한 두 세션의 실행 파일 발견 |
| 실제 화면 입력 | `/` 검색과 선택한 실행 ID 확인 |
| 세션 이동 | Enter 이후 다른 세션의 대상 terminal pane으로 클라이언트 이동 |
| 중복 이벤트 | 같은 공통 JSON 이벤트를 두 번 넣어 두 번째 반영 거부 |
| 오래된 대상 | pane 종료 후 이전 실행 ID의 resolve 거부 |
| 실제 Claude | 격리한 설정에서 SessionStart 훅으로 idle 및 보고 시각 저장 |
| detach | 모든 터미널 클라이언트 종료 후 collector 타이머 계속 증가 |
| 재연결·reload | 클라이언트 재연결과 collector reload 후 저장 revision 복원 |
| 실행 스크립트 | 지정한 임시 세션에 floating dashboard 로드 |

프로세스 발견용 `codex`는 입력을 로컬 파일에 기록하며 대기하는 Rust fixture이며 실제 Codex 이벤트 검증을 대신하지 않는다. 중복 검증에 쓰는 TurnStarted는 명시적으로 넣은 공통 이벤트다. Claude에는 프롬프트를 제출하지 않으며 모델 요청을 하지 않는다. Claude 설정·신뢰 기록과 상태 파일은 `.local/smoke-*` 아래에 격리한다.

## 확인한 제약과 남은 검증

Zellij 0.45.0은 연결된 클라이언트가 없으면 plugin reload를 거부한다. detach 이후 기존 collector의 동작은 유지되지만 reload는 클라이언트를 다시 연결한 상태에서 수행해야 한다.

Claude의 실제 이벤트 검증 범위는 SessionStart다. 작업·도구·승인 대기·완료·압축·실패 이벤트를 포함한 전체 에이전트 턴 검증, 명시적 권한 거절 UI, 다른 도구의 상세 어댑터, 한글 이름의 임시 세션과 운영체제별 프로세스 탐지는 추가 검증 대상으로 남긴다. 현재 실행 중인 한글 세션의 도구는 읽기 전용 스캔에서 발견했고 해당 세션에는 조작을 하지 않았다.

초기 조회 버전 검증 당시 입력 전송·종료·새 실행과 해당 조작의 요청 중복 배제는 구현 전이었다. 이후 호스트 인터페이스 추출 검증 결과는 아래에 기록한다. 여러 호스트 명령의 파일 쓰기 직렬화 검증을 외부 조작의 단일 실행 검증으로 해석하지 않는다. 세션·프로젝트 그룹 영역, 부모·자식 관계 수집과 고정·별칭 편집도 후속 단계다.

실행 결과는 각 임시 디렉터리의 `result.json`에 남는다. 실패 시에는 terminal.log를 보존하며 검사 종료 후 해당 임시 세션을 종료한다.

## 2026-10-08 호스트 인터페이스 추출 검증

`TerminalHost`와 `CommandRunner` 주입 이후 `./scripts/check.sh`와 `./scripts/build.sh`를 통과했다. 코어 15개와 호스트 19개, 총 34개 단위 테스트와 네이티브/WASI Clippy, Rust 포맷, 셸/Python 구문, WASI release 빌드를 확인했다. 기존 메뉴 편집 코드의 Clippy 경고는 `.last()`를 `.next_back()`으로 바꿔 동작을 유지하며 해소했다.

추가 단위 테스트는 CLI 실행 파일 경로 주입, argv의 한국어·공백·여러 줄·셸 특수 문자 보존, terminal/plugin ID 분리, pane 메타데이터·생성 ID 파싱, malformed JSON과 실행 오류 전달을 검증한다. 실제 명령 실행기는 양쪽 출력과 종료 코드, 실행 실패, timeout, 출력 초과, cwd·환경 변수 설정/제거, 출력 폐기를 검증한다. 호스트 대역은 중복 입력의 paste/Enter 재전송 방지, 입력 도중 프로세스 세대 교체 시 Enter 차단과 `Uncertain` 유지, 고정 에이전트 종료 차단을 검증한다.

`python3 scripts/smoke.py --real-claude`를 소유한 임시 세션 두 개에서 실행했다. 기존 조회·멀티 클라이언트·다른 세션으로 이동·오래된 대상 거부·detach·재연결·reload·launcher 검증을 모두 통과했다. 추가로 호스트 action을 통한 pane 생성과 동일 요청의 결과 재사용, 생성된 프로세스 발견, 화면 조회, 한국어 여러 줄 입력과 중복 입력 억제, pane 종료와 종료 요청 재사용을 확인했다. Claude SessionStart 훅은 주입된 변경 통지 경로를 사용했으며 모델 요청은 하지 않았다. 통지는 최선의 전달이고 실패를 무시하므로 이 결과를 통지 유실 없는 전달 보장으로 해석하지 않는다.

검증 결과는 `.local/smoke-0c888d8498/result.json`에 남겼으며 검사 종료 후 임시 세션을 종료했다. 상태 스키마는 기존 1을 유지했다. 현재 운영 세션에 설치나 reload를 적용하지 않았다.

첫 추가 검증에서는 테스트 호출자가 상속한 `ZELLIJ_PANE_ID`가 다른 세션의 pane을 가리켜 `--no-focus`로 생성한 pane의 화면이 비었다. Zellij CLI 직접 호출에서도 재현했고, 테스트 action 실행 환경에서 `ZELLIJ`, `ZELLIJ_SESSION_NAME`, `ZELLIJ_PANE_ID`를 제거한 뒤 통과했다. 이 리팩토링은 CLI의 원래 환경 상속 동작을 변경하지 않는다. 다른 세션으로 no-focus 실행할 때 호출 pane 문맥을 어떻게 전달할지는 별도의 호스트 동작 개선 대상이다.


## 2026년 10월 8일 공통 상태 신호와 화면 어댑터

`StateSignal`을 통한 코어 입력으로 리팩토링하고, 참고 저장소의 Claude/Codex/Gemini/Cursor 화면 규칙·매처·화면 영역을 이관했다. 원본 YAML과 내장 JSON의 내용이 같은 것을 확인했다. 저장소와 스냅샷은 스키마 2이며, 기존 이벤트 JSON은 버전 1을 유지한다. 출처 및 라이선스 확인은 [화면 어댑터 출처](screen-adapter-provenance.md)에 기록했다.

`./scripts/check.sh`에서 코어 21개와 호스트 28개, 총 49개 테스트, Rust 포맷, 네이티브/WASI Clippy와 셸/Python 구문 검사를 통과했다. `./scripts/build.sh`로 네이티브 host와 wasm32-wasip1 release 플러그인을 빌드했다. 테스트는 출처별 순서와 중복, 훅 전환 후 늦은 화면 결과, 시작 유예, 연속 idle 확인과 overlay/실패 시 취소, 실행 세대 교체, 오래된 프로세스 목록 무시, 스키마 1 이관, 저장 후 출처 복원, 입력을 훅 연결로 취급하지 않는 동작을 포함한다. host 대역으로 저장소 잠금 밖의 화면 조회, 조회 도중 훅 도착·프로세스 교체, 중복 collector의 읽기 억제, 수집 예약 만료 후 복구도 확인했다.

Zellij 0.45.0의 소유한 임시 세션 두 개에서 `python3 scripts/smoke.py`와 `python3 scripts/smoke.py --real-claude`를 통과했다. 결과는 각각 `.local/smoke-b912f2731b/result.json`과 `.local/smoke-5d10a182a5/result.json`에 남겼다. 확인 범위는 다음과 같다.

| 항목 | 확인한 동작 |
|---|---|
| 화면 경로 | 훅 없는 Codex fixture가 화면 경로를 선택하고 working/waiting/idle로 전환 |
| 화면 보존 | transcript overlay에서 working 유지, 미일치 화면은 서로 다른 관측으로 idle 확인 |
| 훅 경로 전환 | 공통 이벤트 ingest 후 hook으로 전환하고 이후 idle 화면이 working을 덮어쓰지 않음 |
| 실제 도구 훅 | 격리 설정의 실제 Claude SessionStart로 hook 출처 확인, 모델 요청 없음 |
| 생존·세대 | CLI로 종료한 pane의 오래된 identity를 resolve에서 거부 |
| 화면·collector 수명 | 대시보드 종료, 두 클라이언트, 모든 클라이언트 detach, 재연결과 reload 후 수집·저장 상태 복원 |
| 기존 조작 | pane 생성, preview, 한국어 여러 줄 입력, 요청 중복 억제, 종료와 세션 간 이동 |

reload 뒤 상태 파일에 hook과 screen 출처가 모두 보존되고 수집 예약이 해제된 것도 확인했다. 실제 Gemini/Cursor 및 Claude 전체 대화 흐름의 화면 감지를 확인한 것은 아니다. 이 프로필들은 내장 규칙의 정규식 컴파일과 대표 화면 단위 테스트로 검증했다. OSC progress는 Zellij 조회 계약이 제공하지 않아 사용하지 않는다. 이번 실제 Claude 화면은 2.1.292였으며 이후 로컬 `claude --version`은 2.1.293을 보고했다. 전체 훅 이벤트 지원 범위를 이 검증만으로 확장하지 않는다.

smoke는 명시적 세션 CLI 호출에서 호출자의 Zellij 환경 변수를 제거하고 fixture cwd도 임시 디렉터리로 지정한다. 초기 permission 자동 입력의 잔여 문자는 fixture 명령 전에 지운다. 세션 이동은 한 클라이언트에서 지시하고, 출발 세션의 클라이언트 감소와 대상 pane 초점을 확인한다. 서로 다른 세션에 같은 클라이언트 ID가 있으면 이동 뒤 대상 클라이언트 수가 늘지 않을 수 있다. 선택적 Claude 검사는 collector 수명 검증 뒤에 실행하여 도구 onboarding 화면이 lifecycle 자동화에 개입하지 않도록 했다.

## 2026-10-08 Repository 분리와 SQLite 전환

JSON 파일 저장 호출을 `Repository::read()`와 `Repository::begin()` / `UnitOfWork::commit()`으로 분리하고 `HostDependencies`로 주입했다. 저장 구현은 네이티브 SQLite 어댑터로 교체했다. 코어의 상태 전이와 플러그인의 JSON 계약은 유지하며 DB 구조는 `user_version=1`, 도메인·스냅샷 스키마는 2, 이벤트 JSON은 1이다. 전체 코어 스냅샷을 읽지만 저장은 변경된 레코드만 갱신한다.

`./scripts/check.sh`에서 코어 21개와 호스트 36개, 총 57개 테스트, Rust 포맷, 네이티브/WASI Clippy 및 셸/Python 구문 검사를 통과했다. `./scripts/build.sh`로 네이티브 host와 wasm32-wasip1 release 플러그인을 빌드했다. SQLite 라이브러리는 host에만 포함한다.

추가 검증은 메모리 Repository 주입, 동시 초기화·JSON 이관·작성자의 갱신 보존, 작성 중 조회의 커밋 상태 확인, 미커밋 롤백, 커밋 후 작성자 예약 해제, JSON 1→2 이관, 원본 JSON 보존과 재이관 방지, 손상·미래 버전 거부, 이관 실패 후 재시도, 변경 없는 행의 갱신 방지, 삭제 반영과 테이블 갱신 실패 시 전체 롤백, DB·WAL·SHM 권한을 포함한다. 초기 WAL 설정은 BUSY 핸들러 없이 잠금 오류가 날 수 있어 초기화 단계에만 제한된 재시도를 적용했다.

Zellij 0.45.0의 소유한 임시 세션 두 개에서 `python3 scripts/smoke.py --real-claude`를 통과했다. 결과는 `.local/smoke-c29d923c5f/result.json`에 남겼고 검사 종료 후 임시 세션을 종료했다. JSON 이관 원본의 바이트 보존, 두 세션·멀티 클라이언트 수집, 화면 판별과 훅 전환, 중복 입력·종료·별칭 요청, 세션 이동, 전체 detach 후 수집, 재연결·collector reload, 고정·별칭·요청 결과 복원, 실제 Claude SessionStart 훅, SQLite `integrity_check=ok`, WAL과 DB 구조 버전을 확인했다. Claude에는 프롬프트를 제출하거나 모델 요청을 하지 않았다. 운영 세션에 설치나 reload를 적용하지 않았다.

통합 검증에서 고정을 검색 검사보다 먼저 수행하면 대상 행이 다른 패널로 이동하므로 고정 검사를 세션 이동 뒤에 수행하도록 조정했다. 재연결 직후에는 백그라운드 collector 응답만으로 클라이언트 등록을 판단하지 않고 `list-clients`로 실제 등록을 확인한 뒤 reload한다. 이 대기 추가 전에는 reload 직후 pipe 응답이 시간 초과됐으며, 추가 후 전체 검증이 통과했다.

## 2026-10-08 이동 가능한 에이전트 목록과 pane 확인

기본 목록은 종료되지 않았고 `gone`이 아니며 실제 terminal pane이 확인된 실행만 표시한다. 종료 기록은 24시간 보관하되 목록에서 숨긴다. Codex `app-server`의 native/Node 런처를 발견 대상에서 제외한다. pane 존재 여부는 `PanePresence`와 관측 시각으로 저장하며 프로세스 생존이나 작업 상태를 바꾸지 않는다. 조회 실패는 이전 pane 확인 결과를 유지하고, 실행 세대가 교체되거나 더 최신 pane 결과가 저장되면 늦은 결과를 거부한다. resolve·입력·종료 대상 검증과 preview는 pane 목록을 다시 확인한다.

Store/스냅샷 스키마는 3으로 올렸다. 스키마 1/2는 자동 이관하며 기존 pane 제목으로 존재를 추정하지 않고 `unknown`, 관측 시각 0으로 초기화한다. SQLite DB 구조 버전 1과 이벤트 JSON 버전 1은 유지한다. 상태 백업과 이전 host/WASM은 `.local/backup-before-pane-presence-66b3d2a4de/`에 보존했다.

`./scripts/check.sh`에서 코어 24개와 호스트 40개, 총 64개 테스트, 포맷, 네이티브/WASI Clippy와 셸/Python 구문 검사를 통과했다. `./scripts/build.sh`로 네이티브 host와 wasm32-wasip1 release 플러그인을 빌드했다. 추가 테스트는 app-server 오탐, 종료·부재·미확인 pane의 목록 제외, 숨긴 부모의 고정 상속 방지, pane 부재에도 작업 상태 보존, 조회 실패 시 기존 pane 확인 유지, pane 조회의 역순·실행 세대 교체, 스키마 2 이관, 프로세스가 살아 있어도 부재 pane에 대한 이동·입력 차단을 포함한다.

`python3 scripts/smoke.py --real-claude`를 임시 Zellij 세션 두 개에서 통과했다. 결과는 `.local/smoke-04fb77e98d/result.json`이며 세션과 검증용 백그라운드 프로세스는 종료했다. pane ID 999999를 상속한 살아 있는 Codex fixture를 발견하되 pane을 `missing`으로 확인하고 resolve를 거절했다. `app-server` fixture는 발견되지 않았고, 테스트 세션으로 검색한 대시보드에서는 종료·부재 pane을 선택할 수 없었다. 입력·종료·중복 요청·세션 이동·detach·reload·SQLite 보존과 실제 Claude SessionStart 훅도 통과했다. 전체 세션을 수집하는 특성을 고려해 목록 제외 검증은 테스트 세션으로 검색 범위를 제한한다.

사용자가 테스트하던 `study` 세션의 collector를 reload하고 대시보드 `plugin_11`을 열었다. 다른 운영 세션에는 설치·reload를 적용하지 않았다.

## 2026-10-08 코어 상태 변경 경계 정리

`Store`는 읽기 전용 `StoreData` 접근과 명시적인 복원 경계를 제공하며, 호스트의 직접적인 도메인 필드 변경을 코어 메서드로 옮겼다. 별칭·종료 보호와 결과·부모 연결·요청 결과·pane/cwd·화면 시도 시각·수집 예약을 코어에서 처리한다. 종료 보호는 요청 접수와 종료 직전에 같은 함수를 사용한다. 늦은 화면 시도 기록은 훅 전환·실행 세대 교체 뒤 반영하지 않는다. 예약 해제는 토큰을 확인하며 만료된 자기 예약도 정리할 수 있다. 외부 JSON과 DB 구조 버전은 유지한다.

`./scripts/check.sh`에서 코어 27개와 호스트 40개, 총 67개 테스트와 포맷·네이티브/WASI Clippy·스크립트 구문 검사를 통과했다. `./scripts/build.sh`로 host와 wasm32-wasip1 release 플러그인을 빌드했다. 추가 테스트는 별칭의 세대 검증·멱등성, 공통 종료 보호, launch 연결의 세대 거부·멱등성, 요청 결과의 재완료 거부와 경로 보존, 교체된 예약 보호, 역순·훅 전환 뒤 화면 시도 거부를 포함한다.

`python3 scripts/smoke.py`를 임시 세션 두 개에서 통과했다. 결과는 `.local/smoke-71c2c4ee73/result.json`이다. 수집기 수명·멀티 클라이언트·화면 상태·훅 출처 전환·호스트 입력/종료/실행·중복 요청·고정/별칭 보존·세션 이동·detach·reload·SQLite 무결성을 확인했다. 실제 Claude 옵션은 이 단계에서 실행하지 않았다. 테스트가 소유한 세션과 프로세스는 정리했으며 운영 세션에 설치·reload를 적용하지 않았다.

## 2026-10-08 용도별 Repository 조회와 요청 인덱스

스냅샷·단일 에이전트·단일 요청·최근 요청·실행 메뉴·runtime 조회를 분리했다. 스냅샷과 runtime은 요청 이력을 읽지 않으며 최근 요청은 `at_ms DESC, id ASC` 인덱스와 LIMIT으로 조회한다. 갱신은 이 단계에서 전체 Store 비교를 유지해 부분 조회 결과의 누락을 삭제로 해석하지 않도록 했다.

DB 구조 버전은 2이며 버전 1의 요청 JSON에서 시각을 채우고 인덱스를 추가한다. 도메인·스냅샷은 3, 이벤트 JSON은 1을 유지한다. 이관 실패는 컬럼·인덱스·버전까지 롤백하고 원본 JSON을 보존한다. `./scripts/check.sh`에서 코어 27개·호스트 44개, 총 71개 테스트와 포맷·네이티브/WASI Clippy·구문 검사를 통과했다. 추가 검증은 버전 1 이관과 실패 후 재시도, 인덱스 사용, 관련 없는 손상 payload의 조회 제외, 커밋된 스냅샷, 최신 요청 개수 제한과 동일 시각의 순서를 포함한다. 미래 도메인 스키마가 용도별 조회에서도 거부되는 테스트를 추가 확인했다. `./scripts/build.sh`로 release host/WASM을 빌드했다.

`python3 scripts/smoke.py`를 임시 세션 두 개에서 통과했다. 결과는 `.local/smoke-8891ad352e/result.json`이며 기존 수집·상태·조작·중복·이동·detach·reload 시나리오와 DB 버전 2·WAL·무결성을 확인했다. 실제 Claude 옵션은 다음 통합 검증에서 실행한다. 운영 세션에 설치·reload를 적용하지 않았다.

프로세스 시작을 포함한 CLI 스냅샷 조회를 12회씩 측정했다. 에이전트 없는 상태에서 요청 0개/4,000개(각 메시지 4 KiB)의 중앙값은 이전 host 3.603/16.305ms, 조회 분리 host 3.752/4.284ms였다. 로컬 측정으로 절대 성능 보장을 의미하지 않는다. 결과는 `.local/repository-benchmark-30e95cc48d/result.json`이다.

## 2026-10-08 작업별 트랜잭션과 변경분 저장

코어가 변경된 agent/request/launch ID, 삭제된 agent ID와 메타데이터·활동·최근 경로 변경을 추적한다. SQLite는 이 변경분만 직렬화·저장하며 트랜잭션 시작/커밋의 전체 Store 복제와 전체 JSON 비교를 제거했다. 활동·최근 경로는 각각 최대 50개·100개인 목록 단위로 교체한다. 메타데이터는 컬렉션을 복제하지 않고 별도로 구성한다. 변경 추적과 부분 조회의 요청 총개수는 저장 JSON에 포함하지 않는다.

수집·훅·대상 확인은 runtime 트랜잭션으로 요청 이력을 읽지 않는다. 요청 접수는 runtime 상태·해당 요청과 SQL 전체 개수를 읽어 4,096개 제한 및 중복을 함께 검사한다. 결과·launch 기록은 해당 요청과 launch·최근 경로를 읽는다. 도메인 이관이 필요한 요청 트랜잭션은 에이전트 기본값도 함께 이관한다. 전체 복원은 명시적인 full 트랜잭션에서만 허용하며 부분 조회의 누락을 삭제로 해석하지 않는다. 명시적으로 정리한 agent ID만 삭제한다.

`./scripts/check.sh`에서 코어 27개·호스트 48개, 총 75개 테스트를 통과했으며 별도의 수동 성능 측정 테스트 1개는 기본 검사에서 제외한다. 포맷·네이티브/WASI Clippy·스크립트 구문 검사와 `./scripts/build.sh` release host/WASM 빌드도 통과했다. 검증은 부분 조회의 관련 없는 손상 payload 제외와 행 보존, 범위 밖 요청 변경·부분 트랜잭션의 전체 복원 거부, 전체 요청 한도와 기존 요청 재사용, 동시 8개 claim 중 1개만 접수, action/request 범위의 도메인 이관, 명시적 agent 삭제, 결과 저장 실패 시 메타데이터와 pending 결과 보존을 포함한다. 코어 종료 보호는 부모 고정에 대해서도 접수와 종료 직전의 동일한 거부를 확인했다.

`python3 scripts/smoke.py --real-claude`를 임시 세션 두 개에서 통과했다. 결과는 `.local/smoke-04ab713396/result.json`이다. 화면 감지·훅 출처 전환·입력/종료/실행·중복 요청·고정/별칭 보존·멀티 클라이언트·이동·detach·reload·DB 버전 2·무결성과 실제 Claude SessionStart 훅을 확인했다. 모델 요청은 하지 않았고 테스트 세션과 프로세스는 정리했다. 운영 세션에 설치·reload를 적용하지 않았다.

수동 실험 `cargo test -p dashboard-host repository_transaction_cost_with_request_history -- --ignored --nocapture`에서 요청 0개/4,000개(메시지 4 KiB), 수집 예약 획득+해제 두 트랜잭션의 중앙값을 12회씩 측정했다. debug 빌드의 전체 조회 scope는 0.729/192.637ms, runtime scope는 0.657/1.268ms였다. 현재 코드의 두 scope를 비교한 값이며 이전 바이너리와의 성능 배수로 해석하지 않는다. 측정 로그는 `.local/benchmark-transactions.log`다.

release CLI 스냅샷 조회도 다시 12회씩 측정했다. 이전 host의 요청 0개/4,000개 중앙값은 4.455/16.527ms, 최종 host는 3.678/3.538ms였다. 에이전트가 없는 격리 상태이며 프로세스 시작을 포함한 로컬 측정이다. 결과는 `.local/repository-benchmark-04495774b9/result.json`이다.

## 2026-10-08 h/l 고정·일반 영역 이동

`h`/왼쪽 방향키는 PINNED, `l`/오른쪽 방향키는 UNPINNED 영역을 선택한다. 같은 방향을 반복 입력해도 해당 영역에 머물며 영역별 선택을 유지한다. `Tab` 전환은 유지하고 좁은 화면에서도 선택 영역을 표시한다. 검색·메뉴 입력 중에는 기존 문자 입력 처리를 유지한다.

`./scripts/check.sh`에서 코어 27개·호스트 48개, 총 75개 테스트와 포맷·네이티브/WASI Clippy·스크립트 구문 검사를 통과했다. 영역별 선택 복원 테스트에 같은 영역 재선택을 추가했고 `./scripts/build.sh`로 release host/WASM을 빌드했다.

`python3 scripts/smoke.py`를 임시 Zellij 세션 두 개에서 통과했다. 실제 키 입력으로 `hh`의 고정 영역 유지, `ll`의 빈 일반 영역 유지, `h`의 선택 복원과 고정 해제 후 `l`의 일반 영역 선택을 확인했다. 기존 입력·종료·세션 이동·detach·reload·SQLite 검증도 통과했다. 결과는 `.local/smoke-70e0af5fcf/result.json`이다. 테스트 세션과 프로세스는 정리했으며 운영 세션에는 설치·reload를 적용하지 않았다.

## 2026-10-08 전역 agent next와 Tab working 순환

별도 bridge를 추가하지 않고 같은 WASM의 collector가 `agent-next`를 처리하도록 구현했다. 네 전역 키 필터와 `all`·`working-only`를 지원한다. 순환 선택은 표시 순서·부모 고정 상속을 공유하고 호스트에서 정확한 실행 세대와 pane을 재검증한다. 대시보드 `Tab`은 검색 결과의 양쪽 패널에서 살아 있는 working 에이전트만 선택하며 패널 이동은 `h/l`·`←/→`로 제공한다. 상태 파일 구조 변경은 없다.

`./scripts/check.sh`에서 코어 29개·호스트 49개, 총 78개 테스트와 포맷·네이티브/WASI Clippy·스크립트 구문 검사를 통과했다. 수동 성능 테스트 1개는 기존처럼 제외했다. `./scripts/build.sh`의 release host·wasm32-wasip1 빌드를 통과했다. 새 테스트는 정확한 idle, 부모 고정 상속, working 패널 간 순환과 빈 결과 보존, 종료·미확인·부재 pane 제외, 후보의 실행 세대 교체와 부재 시 이동 거부를 확인한다.

Zellij 0.45.0에서 `python3 scripts/smoke.py`를 통과했다. 결과는 `.local/smoke-6e5b9f09b0/result.json`, 로그는 `.local/agent-next-smoke-final.log`다. 실제 Tab 입력으로 working 선택·패널 이동·순환을 확인했다. 대시보드 없이 Alt+u의 순환, Alt+i의 idle 고정 필터, 같은 세션과 세션 경계를 포함한 연속 키, 빈 필터의 초점 보존, 다른 클라이언트 초점 유지, 전체 detach·재연결·collector reload 뒤 전역 키의 세션 간 이동을 확인했다. unpinned·idle-unpinned는 코어 및 실제 호스트 `next` 조회로 검증했으며 Alt+o/p의 실제 키 입력은 이 실행에서 별도로 검증하지 않았다. 기존 수집·상태·입력·종료·중복·세션 이동·SQLite 검증도 통과했고 임시 세션과 프로세스를 정리했다. 실제 Claude 옵션은 이번 변경에서 실행하지 않았다.

초기 검증에서 collector의 ModeUpdate에는 세션 이름이 없었고 전역 키의 private 메시지가 여러 클라이언트 인스턴스에 전달되는 것을 확인했다. 실제 세션은 세션 목록에서, 초점 pane과 연결된 클라이언트는 ListClients로 조회한다. 연결된 최소 ID의 클라이언트 하나만 요청을 처리하며 detach된 인스턴스와 다른 클라이언트의 큐는 버린다. 키 입력자 ID는 Zellij의 PipeMessage 계약에 없어 다중 클라이언트에서는 처리 클라이언트의 초점만 이동한다. 세션 이동 중 연속 키가 유실되지 않도록 대기 중인 순환을 끝낸 뒤 최종 대상으로 초점을 이동한다.

명시적으로 요청한 `/Users/in05908_mac/.config/zellij/config.kdl`의 Alt+u/i/o/p와 load_plugins를 이 프로젝트의 dist WASM·host·상태 경로로 교체하고 설정 검사를 통과했다. 백업은 `/Users/in05908_mac/.config/zellij/config.kdl.backup-20261008-agent-next-4ef2b83e`다. 실행 중 운영 세션에 강제 reload는 적용하지 않았다. 기존 Alt+q의 CLI 대시보드 실행 설정은 이번 bridge 교체 범위에 포함하지 않았다.

## 2026-10-08 Alt+q 최초 창 크기와 임시 창 제거

`agent-dashboard-open` private 메시지를 collector가 처리하고 연결된 최소 ID 클라이언트가 유한한 `dashboard-host open-dashboard SESSION` 명령을 실행한다. 호스트 어댑터는 기존 floating dashboard가 있으면 재사용하고, 없으면 `zellij plugin --width 90% --height 90% --x 5% --y 5%`로 생성한다. terminal 보조 pane과 생성 후 resize 단계를 제거했다. 세션별 파일 잠금과 collector의 진행 중 요청 억제로 중복 생성을 제한한다. 상태 파일 구조는 변경하지 않았다.

`./scripts/check.sh`에서 코어 29개·호스트 50개 테스트, 포맷·네이티브/WASI Clippy와 스크립트 구문 검사를 통과했고 release host/WASM을 빌드했다. Zellij 0.45.0 임시 세션 `zad-window-manual-test`의 80×24 화면에서 Alt+q로 72×21 창을 생성하고 반복 입력 시 같은 pane 재사용, terminal pane 추가 없음, 두 클라이언트에서 중복 없음, collector reload 뒤 재열기를 확인했다. 테스트 세션은 종료했다.

전체 `scripts/smoke.py` 재실행은 임시 클라이언트 시작·권한 승인 자동화에서 시간 초과되어 완료되지 않았다. 기록은 `.local/smoke-6f748963cf`와 `.local/smoke-80659fec63`이다. 이 실행을 전체 회귀 검증 통과로 기록하지 않는다. 사용자 요청에 따라 전역 Alt+q 설정을 교체하고 현재 세션 `운영`의 collector에만 reload를 적용했다. 연결된 클라이언트가 없어 검증용 클라이언트를 잠시 붙여 reload와 permissions=true 응답을 확인한 뒤 detach했다.

## 최근 상태 변화 제목 표시

하단 활동 영역을 제거하고 고정/일반 목록 제목 옆 괄호 안에 현재 검색 결과에 속한 최신 상태 변화 한 건을 표시한다. 내용은 제목의 남은 너비에 맞춰 생략하며 별도 행을 차지하지 않는다. 상태 파일 구조는 변경하지 않았다.

Rust 포맷 검사, 코어 테스트 29개와 WASM release 빌드를 통과했다. 임시 세션에서 대시보드 로딩·검색까지 확인했지만 plugin pane의 화면 조회를 이용한 제목 표시 검사는 시간 초과됐다. 전체 smoke 통과나 실제 표시 확인 성공으로 기록하지 않는다. 로그는 `.local/smoke-inline-activity.log`, 진단은 `.local/smoke-73e8805a72`다. 임시 세션과 프로세스는 정리했으며 운영 세션에 reload는 적용하지 않았다.

2026-10-12 사용자 요청으로 `scripts/check.sh`의 코어 30개·호스트 50개 테스트, 포맷·네이티브/WASI Clippy·스크립트 구문 검사를 통과하고 `scripts/build.sh`와 `scripts/install.sh`로 release host/WASM을 빌드·설치했다. 수동 성능 테스트 1개는 제외했다. 설치 파일과 dist의 SHA-256 일치를 확인했다. `운영` 세션에 검증용 클라이언트를 잠시 연결해 기존 별칭 collector를 reload하고 권한 승인·상태 revision 응답을 확인했다. 설치된 host로 대시보드를 열어 실제 클라이언트 출력의 PINNED/UNPINNED 제목과 상태 변화 화살표를 확인했으며 화면 인스턴스도 permissions=true, error=null, source_session=운영을 응답했다. 검증용 클라이언트는 종료했고 대시보드는 열어 두었다. 기록은 `.local/apply-20261012.json`, `.local/apply-dashboard-20261012.json`, `.local/apply-dashboard-20261012.capture`다. 전체 smoke 재실행은 두 번째 임시 세션의 collector 권한 승인 대기에서 시간 초과됐으므로 전체 회귀 검증 통과로 기록하지 않는다. 해당 임시 세션은 스크립트에서 정리했으며 로그는 `.local/smoke-build-apply-20261012.log`다.

## 대시보드 세션에서 lazygit 실행

`gg` 요청에 대시보드 세션을 담아 선택한 에이전트의 현재 cwd로 그 세션에 새 floating lazygit pane을 연다. 기존 요청의 선택적 세션 기본값과 중복 비교 호환성을 검증했다. `./scripts/check.sh`에서 코어 30개·호스트 50개 테스트, 포맷·네이티브/WASI Clippy·스크립트 구문 검사를 통과했다. 수동 성능 테스트 1개는 제외했다. `./scripts/build.sh`로 release host와 wasm32-wasip1 플러그인을 빌드했다.

`python3 scripts/smoke.py` 전체 검증을 임시 세션 두 개에서 통과했다. 결과는 `.local/smoke-2055c25588/result.json`이다. floating 대시보드에서 다른 세션의 에이전트를 선택하고 실제 `gg` 입력으로 대시보드 세션의 floating pane 생성·대상 pane의 현재 cwd 사용·요청 재전송 시 중복 생성 방지·`q` 종료와 대시보드 조작 재개를 확인했다. 기존 멀티 클라이언트·세션 이동·detach·reload·SQLite 검사도 통과했다. 초기 실행의 오래된 cwd 비교와 불필요한 Esc 입력을 고쳤으며, 중단한 실행과 권한 승인 시간 초과 실행은 통과 기록에 포함하지 않았다. 임시 세션과 프로세스는 정리했고 운영 세션에 설치·reload는 적용하지 않았다.

## 전체 실행 세션 적용 (2026-10-12)

사용자 요청으로 최신 설치 host/WASM과 dist의 SHA-256 일치를 확인하고, 실행 중인 `study`, `운영`, `wiki`, `zellij-with-code-agent`, `loop` 다섯 세션의 기존 `agent-dashboard` 별칭 collector를 모두 reload했다. 각 세션에서 기존 plugin ID 0 유지, permissions=true, error=null, 상태 revision 응답과 중복 인스턴스 없음까지 확인했다. 적용 당시 열려 있는 dashboard 화면 인스턴스는 없었으며 이후 새 화면은 최신 설치 host/WASM을 사용한다. 연결된 클라이언트가 없는 네 세션에는 검증용 클라이언트를 잠시 연결했다가 종료했고 기존 클라이언트는 유지했다. 결과는 `.local/apply-all-sessions-20261012.json`이다.

## 에이전트 이동 후 대시보드 닫기 (2026-10-12)

대시보드의 대상 resolve가 성공하면 이동 요청 직후 `close_self()`로 화면 pane을 닫는다. resolve 실패·다른 실행 응답은 기존 오류 처리로 화면을 유지한다. collector의 전역 next 경로는 닫지 않는다. Zellij 이동 API에는 성공 응답이 없어 대상 확인과 이동 요청 사이의 실행 종료까지 성공 여부를 확인하는 계약은 아니다.

`bash scripts/check.sh`에서 코어 30개·호스트 50개 테스트, 포맷·네이티브/WASI Clippy·스크립트 구문 검사를 통과했고 `bash scripts/build.sh`로 release host/WASM을 빌드했다. 수동 성능 테스트 1개는 제외했다. `python3 scripts/smoke.py` 전체 검증을 임시 세션 두 개에서 통과했다. 실제 Enter 입력으로 같은 세션의 대상 pane 초점 및 대시보드 제거, 다른 세션 이동 및 원래 세션의 대시보드 제거, collector 유지와 전역 이동을 확인했다. 결과는 `.local/smoke-c8778ae2d4/result.json`, 로그는 `.local/smoke-close-after-focus-20261012.log`이며 임시 세션은 정리했다.

`bash scripts/install.sh`로 설치하고 dist와 설치 host/WASM의 SHA-256 일치를 확인했다. 사용자 세션 `운영`, `study`, `loop`, `wiki`, `zellij-with-code-agent`의 collector와 `운영`에 열려 있던 대시보드를 reload했다. 모든 대상의 permissions=true, error=null, 상태 revision과 기존 plugin ID 유지·중복 없음까지 확인했다. 임시 연결 클라이언트는 종료했고 기존 클라이언트는 유지했다. 적용 결과는 `.local/apply-close-after-focus-20261012.json`이다.

## 선택 에이전트 폴더에서 Neovim 실행 (2026-10-12)

대시보드의 `e` 키로 선택 실행의 ID·세대를 담은 Neovim 조작을 요청한다. 호스트 어댑터가 대상 pane의 현재 cwd를 조회한 뒤 실행 세대를 다시 검증하고 대시보드 세션에 `nvim .` floating pane을 생성한다. pane cwd가 제공되지 않으면 검증된 에이전트의 cwd를 사용한다. 요청 접수·결과 계약으로 중복 요청은 기존 pane ID를 반환하며, 실행 오류는 대시보드에 표시한다. Neovim 종료 시 편집기 pane은 닫히고 대시보드는 유지된다.

포맷·네이티브/WASI Clippy·스크립트 구문 검사, 코어 30개·호스트 50개 테스트와 release host/WASM 빌드를 통과했다. 수동 성능 테스트 1개는 제외했다. 전체 `scripts/smoke.py`를 임시 세션 두 개에서 통과했다. 다른 세션의 에이전트를 선택해 실제 `e` 입력, 대시보드 세션의 floating pane 생성, 대상 pane cwd 일치, Neovim 내부 `getcwd()` 기록, 같은 요청 재전송 시 pane 재사용, `:qa!` 종료 후 대시보드 복귀를 확인했다. 나머지 이동·lazygit·출처 전환·멀티 클라이언트·reload·SQLite 회귀 검사도 통과했다. 결과는 `.local/smoke-9e6b6c77a8/result.json`, 로그는 `.local/smoke-neovim-20261012-pass.log`다. 초기 실행의 화면 경로 출력 검사와 다음 실행의 lazygit cwd 검사는 실패했으며 통과 기록에 포함하지 않는다. Neovim은 파일로 작업 경로를 확인하도록, lazygit은 시작 후 저장소 루트로 cwd를 바꾸는 동작을 허용하도록 검사 조건을 보완했다. Neovim 미설치 환경의 오류 UI는 이번 실제 검증 범위에 포함하지 않는다. 임시 세션은 정리했다.

최신 host/WASM을 설치하고 dist와 설치 파일의 SHA-256 일치를 확인했다. 실행 중인 `운영`, `study`, `loop`, `wiki`, `zellij-with-code-agent` 다섯 세션의 collector를 모두 reload해 권한·오류 없음·revision 응답과 기존 plugin ID 유지·중복 없음까지 확인했다. 적용 시 열려 있던 대시보드 화면은 없었다. 임시 연결 클라이언트는 종료했고 기존 연결은 유지했다. 적용 결과는 `.local/apply-neovim-20261012.json`이다.


## 2026-10-13 새 에이전트 경로에 따른 세션·탭 선택

`n`의 `Launch` 요청을 호스트에서 처리할 때 실행 중 세션 전체의 terminal pane 현재 cwd를 조회한다. 선택 경로와 cwd는 canonicalize 후 정확히 비교한다. 같은 세션·탭의 여러 pane은 하나의 후보로 합치고, 요청한 대시보드 세션의 후보를 우선한다. 후보가 없거나 우선 적용 후에도 여러 탭이 남으면 오류를 반환한다. 선택한 세션 실행 세대를 다시 검사하고 생성 직전에 해당 탭에 일치하는 cwd가 남아 있는지 재조회한다. 새 pane에는 stable tab ID를 `--tab-id`로 전달한다. 상태 저장 구조와 스키마는 변경하지 않았다.

Zellij 0.45.0의 설치된 `new-pane --help`에서 `--tab-id` 지원을 확인했다. `bash scripts/check.sh`에서 코어 30개, 호스트 51개 테스트를 통과했고 기존 호스트 테스트 1개는 ignored다. 포맷, 네이티브/WASI Clippy와 구문 검사를 통과했다. `bash scripts/build.sh`로 최종 host와 WASM을 빌드하고 `bash scripts/install.sh`로 설치했다.

전체 임시 세션 smoke는 `.local/smoke-b5fbd8f39b/result.json`에 기록했다. 요청 세션과 다른 세션의 비활성 탭에 선택 경로가 열려 있는 경우 해당 탭에 생성되는지, 대상 없는 경로 오류, 요청 재사용, 생성 프로세스 발견과 화면·입력·종료, collector detach/reload와 대시보드 재열기를 확인했다. 생성 직전 경로 재조회 보강 후 최종 빌드는 별도 임시 세션에서 다른 세션·비활성 탭, 심볼릭 링크 경로, 호출자 세션·pane 환경 상속, 중복 요청, 여러 일치 탭과 대상 없음 오류를 검증했다. 결과는 `.local/route-0019e3c051/result.json`에 기록했고 생성한 임시 세션은 종료했다.

사용자의 전체 세션 reload 요청에 따라 실행 중인 `운영`, `study`, `wiki`, `zellij-with-code-agent`, `loop`, `hermes` 6개 세션의 `agent-dashboard` collector를 `start-or-reload-plugin`으로 reload했다. 당시 열린 대시보드는 없었다. 연결된 클라이언트가 없는 세션에는 임시 클라이언트를 attach한 뒤 reload하고 분리했다. 각 세션에서 permissions, 상태 revision, 오류 없음, reload 후 poll 증가를 확인했다. 종료된 smoke 세션은 복원하지 않았다. 적용 기록은 `.local/reload-all-dashboard-result.json`에 남겼다.

## 2026-10-22 새 에이전트 생성 전 pane 조회 오류

저장된 요청 `14:1:1792630439277954000:1`은 `rea`에서 Claude 실행을 요청했으며 `expected value at line 1 column 1`로 끝났다. 해당 메시지의 호스트 경로는 생성 전 `list-panes --json` 파싱이다. 당시 원본 응답은 보관되지 않아 어느 세션의 어떤 출력이 원인이었는지는 확정하지 못했다. 조사 시 실행 중인 7개 세션은 모두 정상 JSON을 반환했다.

현재 세션에서 경로가 일치하는 탭을 먼저 조회하고, 후보가 있으면 무관한 세션을 조회하지 않는다. 현재 세션에 후보가 없을 때만 다른 세션을 조회하며, 그 과정의 조회 실패는 숨은 후보로 인한 잘못된 탭 선택을 막기 위해 계속 오류로 처리한다. JSON 파싱 실패에는 세션명과 제어 문자를 제거하고 240자로 제한한 응답을 포함한다. Launch의 실행 기록을 저장하기 전 발생한 오류는 `Failed`로 표시하고, 실행 기록 저장 이후 오류는 `Uncertain`을 유지한다. 같은 요청의 자동 재실행은 하지 않는다. 상태 스키마는 변경하지 않았다.

`bash scripts/check.sh`에서 코어 37개·호스트 58개 테스트, 포맷·네이티브/WASI Clippy·스크립트 구문 검사를 통과했다. 기존 수동 성능 테스트 1개는 ignored다. `bash scripts/build.sh`로 host와 WASM release를 빌드했다. 실제 Zellij 0.45.0 임시 세션에서 무관한 세션의 잘못된 JSON 응답을 CLI 래퍼로 주입해 현재 세션 생성 성공·중복 생성 방지·현재 세션에 후보가 없을 때 세션/응답을 포함한 Failed 결과·실패 요청 중복 억제를 확인했다. 결과는 `.local/launch-json-e4ea0135/result.json`이다. 별도의 임시 세션에서 다른 세션의 비활성 탭, 심볼릭 링크, 호출자 pane 환경, 중복 요청, 다중 후보와 후보 없음도 검증했다. 결과는 `.local/route-0bb04d9f06/result.json`이며 모든 임시 세션은 종료했다. 전체 smoke는 이번에 실행하지 않았다.

`bash scripts/install.sh`로 설치하고 dist와 설치 host/WASM의 SHA-256 일치를 확인했다. 오류 요청의 대상인 `rea` 세션 collector만 reload했다. 기존 plugin ID 유지·pane 추가 없음·permissions=true·error=null·revision과 poll 증가를 확인했다. 당시 열린 dashboard는 없어 다음 열기부터 최신 설치본을 사용한다. 검증용 임시 클라이언트는 종료했다. 적용 기록은 `.local/apply-launch-json-rea.json`이다. 과거 실패 요청은 보존했으며 자동 재전송하지 않았다.

## 2026-10-22 idle 이동을 최근 working → idle 순서로 변경

`Alt i`(`idle-and-pinned`)와 `Alt p`(`idle-and-unpinned`)는 일치하는 후보를 마지막 `working → idle` 전환 시각 내림차순으로 순환한다. 현재 실행이 후보이면 다음 후보로, 후보가 아니면 최신 후보로 이동하며 끝에서는 처음으로 돌아간다. 전환 기록이 없는 초기 idle·기존 버전 실행은 뒤에 두고 동률은 기존 표시 순서를 유지한다. 다른 필터와 화면 목록 정렬은 유지한다.

코어가 실제 수락한 전환 시각을 선택적 `Agent.working_to_idle_at_ms`에 저장한다. 반복 idle·중복·역순·훅 전환 뒤 무시한 화면 결과는 시각을 갱신하지 않는다. 화면 연속 확인은 확정 관측 시각을 사용한다. 새 실행은 초기화하고 활동 50건 제한과 별도로 저장해 reload 후에도 보존한다. 하위 호환 필드이므로 DB 구조 2·Store/스냅샷 3·이벤트 1을 유지하며 기존 필드 없음의 기본값은 `None`이다. 버전·기본값·롤백 시 시각 보존 한계는 `docs/architecture.md`에 기록했다.

`bash scripts/check.sh`에서 코어 39개·호스트 58개 테스트, 포맷·네이티브/WASI Clippy·스크립트 구문 검사를 통과했다. 기존 수동 성능 테스트 1개는 ignored다. 최신순 순환·동률·기록 없음·현재 후보 부재·고정 구분·새 전환·미확인/부재 pane 제외와 기존 비idle 정렬, 화면/훅 전환 시각·중복/역순·저장 복원·새 세대 초기화·이전 Agent JSON 기본값을 확인했다.

Zellij 0.45.0 임시 세션에서 실제 Codex 화면 형식의 fixture 3개를 실행해 pane 순서와 다른 완료 순서, 초기 idle 후순위, `Alt i` 최신순/끝 순환, collector reload 후 시각과 순서 보존, 새 working → idle 후 최신 후보 변경, `Alt p`의 최신순 이동을 확인했다. 결과는 `.local/idle-recency-e3e0170e/result.json`의 19개 확인 항목이다. 임시 세션은 종료했다. 최초 두 검증 시도는 임시 설정 KDL 문법 오류로 세션 생성에 실패했으며 최종 검증에서는 수정했다. 전체 smoke는 이번에 재실행하지 않았다.

검증 후 `bash scripts/build.sh`와 `bash scripts/install.sh`로 최종 host/WASM을 빌드·설치하고 dist와 설치 파일의 SHA-256 일치를 확인했다. 현재 작업 세션 `loop`와 앞서 확인한 `rea`의 collector를 reload했다. 두 세션 모두 기존 plugin ID 0과 중복 없음, permissions=true·error=null, reload에 따른 poll 초기화와 이후 poll/revision 증가를 확인했다. 당시 열린 dashboard는 없었다. 설치 host의 스냅샷에서 새 필드 제공도 확인했다. 임시 연결 클라이언트는 종료했고 기존 클라이언트는 유지했다. 결과는 `.local/apply-idle-recency-loop.json`과 `.local/apply-idle-recency-rea.json`이다. `rea`의 최초 적용 검사는 진행 중인 pane 변경으로 전체 pane 집합 동일성 조건을 만족하지 못했고, 이후 collector ID 보존을 검증하도록 보완했다. 최종 적용에서는 호출자의 Zellij 환경을 제거해 대상 세션을 명시하고, 실제 처리 클라이언트의 poll 183→3 초기화와 3→4 증가를 확인했다.

사용자의 전체 세션 적용 요청에 따라 실행 중인 `study`, `loop`, `asbplayer`, `운영`, `hermes`, `rea`, `zellij-with-code-agent` 7개 세션의 collector에 최신 설치본을 reload했다. dist와 설치 host/WASM 해시 일치를 확인했고, 각 세션에서 기존 plugin ID 0 유지·중복 없음·permissions=true·error=null·reload 후 poll/revision 증가를 확인했다. 열린 dashboard는 없었으며 다음 열기부터 설치된 최신본을 사용한다. 클라이언트가 없는 세션에는 검증용 클라이언트를 잠시 연결한 뒤 종료했고 기존 연결은 유지했다. 최초 `loop` 검사는 reload 전 현재 클라이언트 응답 대기에서 시간 초과되어 먼저 reload한 뒤 다시 검사해 poll 2→4와 revision 증가를 확인했다. 최종 실행 세션 목록과 적용 목록도 대조했다. 통합 결과는 `.local/apply-idle-recency-all.json`, `loop` 재검증은 `.local/apply-idle-recency-loop-retry.json`이다.

## 2026-10-22 working 중 입력 자동 예약과 에이전트별 FIFO

사용자 선택에 따라 `i` 입력은 working/compact 중 자동 예약하고, idle에서 큐가 비어 있으면 기존처럼 즉시 전송한다. 이미 예약이나 전송 항목이 있으면 뒤에 추가해 FIFO를 보존한다. `Q`에서 진행 상태·대기 목록·전체 내용(Enter)을 확인하고 `d`로 대기 항목을 취소한다. 불명확한 전송은 `r`의 별도 확인을 거쳐 그 항목 재전송 없이 다음 예약부터 재개한다. 최대 32개·총 64 KiB이며 큐는 실행 identity에 묶는다.

정책은 코어 `instruction_queue`에, 한 수집당 최대 한 항목의 유한한 외부 전송은 호스트 `instruction_queue::dispatch`에 구현했다. claim을 저장한 뒤 잠금 밖에서 pane/프로세스 세대를 재검증해 전송하고, 시작과 완료를 모두 관측해야 다음 항목을 보낸다. 전송 중 시작/완료가 먼저 도착해도 전송 결과 저장 전에는 다음 항목을 claim하지 않는다. 화면 idle을 TurnFinished 사건으로 바꾸지 않으며 훅 전환 뒤 화면 결과는 기존 정책대로 무시한다. 조회 실패/overlay·중복 idle로 큐를 비우지 않는다. 실패·claim 만료·시작 미확인은 자동 재전송 없이 확인 대기로 남는다. 시작이 관측된 긴 작업에는 시간 제한을 두지 않는다.

Store/스냅샷을 4로 올렸고 DB 구조 2·훅 사건 1은 유지했다. 1/2/3에서 자동 이관하고 큐 필드 없음은 빈 큐다. 상태·기본값·구버전 작성자 보호·롤백 방법은 아키텍처에 기록했다. 신규 큐·취소·재개에 기존 요청 ID 중복 억제를 적용한다. 별도 데몬·상주 서버·시간 기반 스케줄러는 추가하지 않았다.

`bash scripts/check.sh`에서 코어 45개·호스트 61개 테스트, 포맷·native/WASI Clippy·스크립트 구문 검사를 통과했다. 기존 수동 성능 테스트 1개는 ignored다. FIFO·한 번에 하나·중복 등록·개수/바이트 한도·취소·시작/완료 확인·전송 중 완료·훅 출처·overlay/오래된 화면·재로딩·전송 claim 만료·불명확 재개·실패·새 실행 초기화·스키마 3 기본값 이관을 확인했다. 호스트 동시 dispatcher 두 개의 단일 전송, 여러 줄 보존, paste 후 세대 교체 시 Enter 차단과 재전송 금지도 확인했다.

실제 Zellij 0.45.0 임시 세션에서 Codex 화면 fixture와 실제 `i/Q/d/r` 입력으로 3개 예약·선택 취소·FIFO 순차 전송·반복 working 관측 중 다음 항목 유지·여러 줄/한국어·요청 재사용·collector reload·idle 즉시 입력을 확인했다. CLI 래퍼로 write 실패를 주입해 확인 대기·idle 재관측에도 재전송 없음·확인 메뉴 후 실패 항목 제외와 다음 항목 전송을 확인했다. pane 종료 뒤 새 실행에 이전 예약이 전달되지 않는 것도 확인했다. 최종 결과는 `.local/instruction-queue-fe8dc709/result.json`이며 임시 세션과 해당 종료 기록은 정리했다. 최초 시도는 임시 플러그인 권한 승인 대기에서, 다음 시도는 성공 시 입력 메뉴가 닫히는 기존 계약과 다른 검증 조건으로 중단됐다. 검증 도구를 보완한 뒤 최종 빌드에서 통과했다. 전체 `scripts/smoke.py`는 이번에 재실행하지 않았고 실제 AI 제공자별 전체 대화 검증으로 확대하지 않는다.

검증 후 `bash scripts/build.sh`와 `bash scripts/install.sh`로 최종 host/WASM을 빌드·설치했다. 업그레이드 전 SQLite 온라인 백업과 기존 설치 파일은 `.local/instruction-queue-upgrade-backup-20261022-104049`에 보존했다. 실행 중인 `study`, `loop`, `asbplayer`, `운영`, `hermes`, `zellij-with-code-agent` 6개 세션의 collector를 reload해 기존 plugin ID 유지·중복 없음·permissions=true·error=null·Connected 응답·reload 후 poll/revision 증가를 확인했다. 당시 열린 dashboard는 없었다. 종료 상태인 `rea`는 복원하지 않았으며 다음 실행에서 설치본을 사용한다. 임시 클라이언트는 종료했고 기존 연결은 유지했다. 설치 파일 해시 일치, 실제 스냅샷 스키마 4와 마지막 세션별 CLI 재조회로 접근 가능한 6개 세션 대비 적용 범위도 확인했다. 프로세스 목록에 남은 `rea`는 CLI에서 종료 상태로 확인되어 적용 대상으로 복원하지 않았다. 결과는 `.local/apply-instruction-queue-all.json`이다.

## 2026-10-22 미전송 채팅 입력 강조

Codex/Claude composer 어댑터와 독립 `Draft` 신호를 추가했다. 대시보드는 프로젝트 칸 앞에 노란색 굵은 `[입력중]`을 표시한다. 작업 상태와 훅 출처는 변경하지 않고, 미전송 텍스트 자체는 저장하지 않는다. 기본값과 하위 호환 정책은 아키텍처의 미전송 입력 표시 항목을 따른다.

포맷·workspace 테스트·host/plugin Clippy·WASM Clippy와 release WASM 빌드를 확인했다. 코어는 훅 독립성·중복/역순·실행 세대 교체·reload/기존 데이터 기본값·선택 행의 색상 유지와 해제를 검증한다. 호스트는 한국어/여러 줄·dim placeholder와 RGB SGR 파싱·과거 대화/메뉴 제외·훅 연결 전후 composer 수집·조회 실패/ANSI만 있는 빈 화면 보존을 검증한다. 최종 추가 회귀 검사는 collector 8개가 통과했다.

실제 Zellij 0.45.0의 임시 세션 `zad-draft-2c3b0208`에서 raw 입력을 받는 Codex 화면 fixture로 한국어 미전송 입력·다른 pane(대시보드)으로 포커스를 옮긴 뒤 표시·노란색 ANSI·collector reload·삭제/전송 후 해제·훅 연결 뒤 working 상태를 보존하면서 표시/해제를 확인했다. `.local/draft-highlight-ac6720c8/result.json`에 13개 통과 결과를, 같은 폴더에 터미널 출력과 표시 당시 ANSI를 보존했다. 재현 스크립트는 `.local/verify-draft-highlight.py`이며 `uv run --with pyte python .local/verify-draft-highlight.py`로 실행했다. `dump-screen`은 플러그인 pane에서 빈 값을 반환하므로 PTY 출력을 연속 해석해 검증했다. 초기 검증 도구의 빈 플러그인 dump, pyte private device-status 처리 및 출력 앞부분 잘림 문제를 보완했다. 임시 세션과 해당 종료 기록은 정리했다. 실제 Claude 제공자 대화와 Cursor/Gemini/Hermes/Pi composer 감지는 이번 실검증 범위에 포함하지 않는다. 전체 기존 smoke는 재실행하지 않았다.

검증 후 `bash scripts/build.sh`, `bash scripts/install.sh`를 실행하고 사용자가 지정한 실행 중인 모든 세션 `study`, `loop`, `asbplayer`, `운영`, `hermes`, `zellij-with-code-agent`, `rea`의 collector를 reload했다. 7개 모두 기존 plugin ID 유지·중복 없음·permissions=true·error=null·Connected·reload 후 poll/revision 증가를 확인했다. 적용 당시 열린 대시보드는 없었으며 다음 열기부터 설치된 새 WASM을 사용한다. 설치 파일과 dist 해시는 일치한다. 기존 연결은 유지하고 검증용 attach 클라이언트만 종료했다. 결과는 `.local/apply-draft-highlight-all.json`, 실제 Codex 실행 5개의 composer 관측과 미지원 Cursor 실행의 기본값 확인은 `.local/draft-live-observations.json`에 기록했다. 설치 전 SQLite 온라인 백업과 이전 host/WASM은 `.local/draft-highlight-backup-20261022-165820`에 보존했다.


## 2026-10-22 입력창 한글 키 이벤트 손상 수정

Zellij 0.45.0의 `zellij-utils::plugin_api::key::char_index_to_char`는 protobuf의 유니코드 정수를 `as u8 as char`로 변환한다. 실제 임시 세션의 기존 WASM에서 `i`를 누르고 PTY로 `한글`을 입력하면 `한`은 `\`로, `글`은 NUL로 변환되어 최종 입력은 `\`만 남는 것을 재현했다. 붙여넣기는 별도 `PastedText` 경로여서 이 문제와 구분한다.

플러그인의 SDK 진입점을 유지하면서 이벤트 수신 경계에서 원래 유니코드 스칼라를 복원한다. 수정 범위는 Key 이벤트의 문자 값이며 modifier·명명 키·다른 이벤트는 SDK 변환을 유지한다. 잘못된 스칼라는 이벤트를 거부한다. SDK 등록 매크로에서 가져온 진입점의 출처와 MIT 고지는 `crates/dashboard-plugin/src/entrypoints.rs`와 `crates/dashboard-plugin/notices/zellij-tile-LICENSE.md`에 보존했다. 상태 파일 구조와 버전은 변경하지 않았다.

`bash scripts/check.sh`의 포맷, workspace 테스트(코어 47개·호스트 66개·플러그인 3개 통과, 기존 수동 성능 검사 1개 ignored), native/WASI Clippy와 스크립트 검사를 통과했다. protobuf 직렬화/역직렬화의 한글 완성형·자모·일본어·이모지·ASCII·modifier·명명 키·붙여넣기·다른 이벤트와 잘못된 스칼라를 검증한다.

실제 Zellij 0.45.0 임시 세션 `zad-draft-14764529`에서 새 release WASM으로 `i` 입력과 예약, 한글·일본어·이모지 원문 보존, 한글 Backspace 후 재입력(`가나` → `가다`), 한국어 여러 줄 bracketed paste, 한글 검색을 확인했다. 예약 원문 세 개와 14개 확인 항목은 `.local/draft-highlight-beb018ef/result.json`에 기록했다. 재현 스크립트는 `.local/verify-korean-input.py`이며 임시 세션은 종료했다. macOS 입력기의 실제 조합 중 화면은 자동 검증 범위에 포함하지 않는다. 전체 smoke는 재실행하지 않았다.

검증 후 `bash scripts/build.sh`와 `bash scripts/install.sh`로 host와 WASM을 빌드·설치했다. 이전 설치본은 `.local/unicode-input-backup-20261022-172516`에 보존했다. 현재 작업 대상 `loop`의 collector를 reload하고 기존 plugin ID 유지·중복 없음·permissions=true·error=null·Connected와 poll/revision 증가를 확인했다. 기존 열린 dashboard는 없었다. 설치된 동일 별칭으로 검증용 dashboard를 열어 실제 PTY 한글 검색 값 `한글`을 확인한 뒤 해당 검증 pane만 닫았다. 임시 클라이언트도 종료했다. dist와 설치 파일 SHA-256 일치 및 적용 기록은 `.local/apply-unicode-input-loop.json`에 있다. 다른 세션에는 reload하지 않았다.


사용자의 전체 세션 적용 요청에 따라 실행 중인 `study`, `loop`, `asbplayer`, `운영`, `hermes`, `zellij-with-code-agent`, `rea` 7개 세션의 collector에 동일한 한글 입력 수정본을 reload했다. dist와 설치 host/WASM의 SHA-256 일치를 확인했고, 각 세션의 기존 plugin ID 0 유지·중복 없음·permissions=true·error=null·Connected와 reload 후 poll/revision 증가를 검증했다. 열린 dashboard는 없어 다음 열기부터 반영된다. 기존 클라이언트는 유지했고 검증용 임시 attach 클라이언트는 종료했다. 최종 실행 세션과 적용 범위, pane 및 클라이언트 재조회 결과는 `.local/apply-unicode-input-all.json`에 기록했다.


## 2026-10-23 대시보드 스크롤 누적과 목록 이동 지연 수정

임시 Zellij 0.45.0 세션에서 키 입력 없이 약 15초 동안 대시보드 프레임의 `SCROLL: 0/457` 증가를 재현했다. 기존 렌더는 마지막 화면 행에도 줄바꿈을 출력했다. 화면 원점을 지정하고 이전 화면을 지운 뒤 행 사이에만 CRLF를 출력하도록 대시보드·알림·초기/오류 화면의 출력 경로를 통일했다. 세션·모드·클라이언트 메타데이터와 진단 ping은 재그리기를 요청하지 않는다.

목록 선택은 즉시 반영하고, 선택이 바뀐 경우에만 80ms 후 출력 미리보기를 조회한다. 연속 이동은 마지막 선택으로 합치며 이전 실행의 늦은 미리보기는 기존 대상 검증으로 버린다. 미리보기 타이머와 2초 상태 조회 기한을 구분해 키 입력이 주기 조회를 늘리지 않도록 했다. 상태 파일 구조와 버전은 유지한다.

`bash scripts/check.sh`의 포맷, workspace 테스트(코어 49개·호스트 68개·플러그인 6개 통과, 기존 수동 성능 테스트 1개 ignored), native/WASI Clippy, 스크립트 검사를 통과했다. 마지막 행 줄바꿈 금지·높이 제한·이전 내용 지우기·ANSI 보존을 플러그인 테스트로 확인했다. `bash scripts/build.sh`로 최종 host/WASM을 빌드했다.

실제 임시 세션에서 에이전트 fixture 3개를 발견하고 21번 연속 j/k 이동이 미리보기 명령 1개로 합쳐지는 것, 마지막 선택과 미리보기 일치, 2초 조회 유지, 폭 40·80·120에서 선택 보존, 10초 대기 후 대시보드 프레임에 SCROLL 없음까지 확인했다. PTY와 pyte로 대시보드 프레임을 확인했으며 fixture terminal의 기존 스크롤과 구분했다. 결과는 `.local/scroll-navigation-3ffe06e7/result.json`, 화면은 같은 폴더의 `screen.txt`, 검증 도구는 `.local/verify-scroll-navigation.py`다. 별도 빈 대시보드의 15초 대기에서도 스크롤이 없었다(`.local/render-scroll-c147e653/result.json`). 최초 검증 도구의 레이아웃 문법, PTY 시작 크기 경쟁, 반환값과 fixture 스크롤 오인 조건을 보완한 뒤 통과했다. 임시 세션은 종료했으며 전체 기존 smoke는 이번에 재실행하지 않았다.

`bash scripts/install.sh`로 설치하고 이 저장소의 실행 대상 `zellij-with-code-agent` 세션 collector를 reload했다. 기존 plugin ID 69 유지, 권한 승인·오류 없음·Connected·poll 1→2를 확인했다. 적용 당시 열린 대시보드는 없었으며, 설치 별칭으로 검증용 대시보드를 열어 j/k 이동·마지막 미리보기와 7초 대기 후 SCROLL 없음을 확인하고 해당 pane만 닫았다. 검증용 attach 클라이언트는 종료했고 다른 세션은 reload하지 않았다. 설치 파일과 dist의 host/대시보드/알림 WASM 해시는 일치한다. 적용 결과는 `.local/apply-scroll-navigation.json`, 이전 설치본은 `.local/scroll-navigation-backup-20261023-171607`에 보존했다.

사용자의 전체 세션 적용 요청에 따라 실행 중인 `zellij-with-code-agent`, `study`, `운영` 3개 세션에 동일 수정본을 reload했다. 적용 도중 새로 시작된 `study`와 재시작된 `운영`도 새 실행 상태에서 다시 적용했다. 각 세션에서 기존 collector ID 유지·권한 승인·오류 없음·Connected·poll 증가, 설치 별칭 대시보드의 마지막 선택 미리보기와 대기 후 SCROLL 없음을 확인했다. 최초 목록의 열린 대시보드는 적용 시점에 이미 닫혀 있었으며 검증용 pane만 생성·종료했다. 기존 클라이언트는 유지하고 검증용 attach만 종료했다. 마지막 실행 세션 목록과 적용 목록 일치·collector 중복 없음·설치 해시 일치를 확인했다. 종료된 세션은 복원하지 않았으며 이후 실행에서는 설치본을 사용한다. 통합 결과는 `.local/apply-scroll-all.json`이다.

## 2026-10-28 `n` 프로젝트 세션·탭 자동 생성

선택한 경로를 정규화해 기존 terminal pane의 cwd와 비교한다. 기존 탭이 없으면 폴더 이름의 세션과 탭을 생성하고, 같은 이름의 실행 중 세션이 있으면 그 세션에 프로젝트 탭을 추가한다. 기존 탭 우선순위·모호한 목적지 거부·요청 ID 중복 억제·실행 직전 세대/cwd 재검증은 유지한다. 프로세스 스캔에 종료된 서버가 남을 수 있어 CLI의 실행 중 세션 목록과 교차 확인하고 EXITED 세션은 탐색에서 제외한다. 상태/DB/요청 스키마 변경은 없다.

Zellij 0.45.0에서 background attach 직후에는 초기 pane의 cwd 메타데이터가 아직 없을 수 있어 최대 5초 동안 준비를 확인한다. 연결된 클라이언트가 없는 기존 세션은 `new-tab`이 ID만 반환하고 pane을 만들지 않는 것을 실제 확인했다. 이 경우 호스트 어댑터에서 macOS `script`로 소유한 임시 PTY 클라이언트를 연결하고, 연결 확인 후 탭을 만든 뒤 해당 클라이언트만 종료한다. 입력 pipe는 클라이언트 처리 중 EOF가 발생하지 않도록 열어 두며, 준비 확인과 helper 실행 모두 시간 제한이 있다. 상주 helper나 수집기는 추가하지 않는다.

`bash scripts/check.sh`에서 포맷·workspace 테스트(코어 48, 호스트 68, 플러그인 6 통과, 수동 성능 테스트 1개 제외)·host/WASM Clippy를 확인했다. 실제 Zellij 임시 세션에서 Codex 실행 파일을 sleep fixture로 대체해 새 세션/탭/agent 생성·심볼릭 링크 정규화·동일 요청 재사용·기존 탭 재사용·클라이언트가 없는 동명 세션에 다른 경로 탭 추가·모호한 탭 거부를 검증했다. 실제 대시보드에 `n` → 절대 경로 → 도구 선택을 입력해 새 프로젝트 세션에서 agent pane 생성도 확인했다. 종료된 `rea`가 목록에 있는 상태에서 탐색이 성공했다. 결과는 `.local/project-launch-e0e449a6/result.json`, 재현 스크립트는 `.local/verify-project-launch.py`, 최종 자동 검사 기록은 `.local/project-launch-final-check.log`에 있다. 실제 AI 제공자 대화는 실행하지 않았다. 검증용 세션은 종료했다.

초기 검증에서 EXITED 세션의 잔존 프로세스, 비동기 초기 pane 준비와 무클라이언트 탭 생성 문제를 발견해 보완했다. 모호성 검증용 탭도 연결 클라이언트 없이 생성하면 만들어지지 않아, 검증 설정에 임시 attach와 pane 생성 확인을 추가한 뒤 최종 검증을 통과했다.

최종 검증 후 `bash scripts/build.sh`와 `bash scripts/install.sh`로 host/WASM을 빌드·설치했다. 사용자가 선택한 전체 실행 세션 `운영`, `study`, `hermes`, `zellij-with-code-agent` 4개의 collector를 reload하고 기존 ID 0 유지·중복 없음·권한 승인·오류 없음·Connected·poll 증가를 확인했다. 열린 대시보드는 없었으며 다음 열기부터 설치본을 사용한다. 종료된 `rea`는 복원하지 않았다. 검증용 attach 클라이언트만 종료했고 설치본과 dist의 host/WASM SHA-256 일치를 확인했다. 적용 기록은 `.local/apply-project-launch.json`, 이전 설치본은 `.local/project-launch-backup-20261028-093145`에 보존했다.
