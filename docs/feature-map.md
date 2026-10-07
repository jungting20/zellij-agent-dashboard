# Agent Dashboard 기능 이관표

기존 대시보드의 기능을 조회, 기본 조작, 보조 메뉴, 후속 엔진으로 구분해 이관한다. 이번 프로젝트의 첫 결과는 M2 조회 대시보드이며 전체 기존 기능과의 동등성을 의미하지 않는다.

## 기능별 범위

| 기능 | 기존 위치 또는 키 | 새 구현 단계 |
|---|---|---|
| 에이전트 목록과 상태, 세션과 프로젝트 표시 | `model.go`, `view.go` | M1과 M2 |
| 상태 변화 활동 기록 | `activity.go` | M2 |
| 고정 영역과 일반 영역, 영역별 선택 유지 | `view.go`, `Tab` | M2 표시, M3 저장 |
| 부모와 자식 계층 | `hierarchy.go` | M2 |
| 숫자 선택, 방향키, j/k, 새로고침 | `model.go`, `R` | M2 |
| 선택 에이전트 pane과 세션 이동 | `Enter` | M2 |
| 검색과 마지막 보고 시점 표시 | 새 개선 기능 | M2 |
| 고정과 해제 | `Space` | M3 |
| 별칭 선택과 사용자 정의 | `alias.go`, `a` | M3 |
| 여러 줄 입력 전송 | `input.go`, `i` | M3 |
| 에이전트 pane 종료, 고정 에이전트 종료 제한 | `d` | M3 |
| 새 에이전트 실행 | 기존 CLI와 최근 경로 메뉴 | M3 기반, M4 메뉴 |
| 마지막 지시 요약과 전체 보기 | `instruction.go`, `p` | M4 |
| 마지막 출력 미리보기 | `view.go` | M4 |
| 외부 에디터 편집 | `editor.go`, `I` | M4 |
| 최근 경로와 실행 도구 선택 | `recent.go`, `n` | M4 |
| worktree 추가와 부모 연결 | `worktree.go`, `g` 메뉴 | M4 |
| 자식 worktree 탭 이동 | `worktree_menu.go` | M4 |
| lazygit 실행 | `lazygit.go` | M4 |
| 자식 worktree 셸 명령과 결과 표시 | `worktree_menu.go` | M4 |
| 자식 선택 후 부모에게 merge 지시 전송 | `merge.go`, `m` | M4 |
| 티켓 목록, 생성과 실행 메뉴 | `ticket.go`, `t` | 후속 티켓 엔진 이관 시 |
| 후속 지시 편집과 pause, resolve | `followup.go`, `f` | 후속 실행 엔진 이관 시 |
| 승인과 거절 | 참고 프로젝트의 훅 기능 | 별도 후속 기능 |
| 음성 알림 큐, HTTP API, 전체 SQLite 복구 | 데몬 기능 | 이번 범위 밖 |

## 도구별 연결 순서

| 도구 | 기존 실행 파일 | 현재 기준과 새 연결 방향 |
|---|---|---|
| Claude | `claude` | 프로세스 발견과 훅 어댑터 구현, 2.1.70 SessionStart 실동작 확인 |
| Codex | `codex` | 프로세스 발견 구현, 상세 훅 연결은 후속 작업 |
| Cursor CLI | `agent` | 프로세스 발견 구현, 실제 실행 발견 확인, 상세 상태는 후속 작업 |
| Gemini 프로필 | `agy` | Gemini 이름으로 프로세스 발견 구현, 실동작 검증은 후속 작업 |
| Hermes | `hermes` | 프로세스 발견 구현, 상세 상태와 실동작 검증은 후속 작업 |
| Pi | 미등록 | 도구 실체와 실행 방식 확인 후 별도 추가 |

훅, 확장 API, 구조화된 이벤트의 지원 여부는 설치된 도구 버전별로 검증한다. 어댑터가 없는 도구도 프로세스 발견을 지원하면 `found`로 표시할 수 있지만 작업·대기·완료 상태를 추정해서 확정하지 않는다.

## 첫 조회 버전의 구현 상태

목록, 상태 긴급도 정렬, 검색, 실행 ID를 유지하는 선택, 마지막 보고 시점, stale/cached 표시와 pane 이동을 구현했다. 최근 상태 변화는 한 줄로 표시한다. 여러 collector의 상태 갱신은 공유 저장소 잠금으로 직렬화한다.

세션·프로젝트 그룹별 영역, 부모·자식 관계 수집, 고정 영역별 선택, 고정과 별칭 편집은 아직 구현하지 않았다. 고정·별칭·부모 필드는 상태 계약에만 포함했다. 입력, 종료, 새 실행, 보조 메뉴도 후속 단계로 남아 있다.

## 참조 기준

2026년 10월 7일 로컬 소스를 기준으로 작성했다. 링크는 같은 머신의 참고 저장소를 가리킨다.

- 기존 대시보드: `/Users/in05908_mac/zellij-with-codeagent`, commit `1f49244a9af40d0ae936fb4088eca297810e3a64`
- 훅과 상태 파일 참고: `/Users/in05908_mac/study/zj-agent-mob`, commit `010bd2cc3ab232c799774c0a3320ea41418a6798`

| 참조 소스 | 활용할 동작 |
|---|---|
| [기존 키와 화면 상태](/Users/in05908_mac/zellij-with-codeagent/internal/agentdashboard/model.go) | 기능, 선택과 화면 수명 |
| [기존 상태 판별](/Users/in05908_mac/zellij-with-codeagent/internal/codingagent/detector.go) | 화면 탐지의 보완 경로 |
| [기존 Monitor](/Users/in05908_mac/zellij-with-codeagent/internal/codingagent/monitor.go) | 세대 검증과 상태 변화 |
| [도구 프로필](/Users/in05908_mac/zellij-with-codeagent/internal/codingagent/profile.go) | 실행 파일과 도구 구분 |
| [기존 백그라운드 bridge](/Users/in05908_mac/zellij-with-codeagent/plugins/agent-next-bridge/src/main.rs) | 권한과 다중 클라이언트 처리 참고 |
| [참고 훅](/Users/in05908_mac/study/zj-agent-mob/scripts/zj-agent-mob-hook.sh) | 공통 상태 매핑과 세션 간 전달 |
| [참고 상태 관리](/Users/in05908_mac/study/zj-agent-mob/src/state.rs) | 상태 통합과 오래된 보고 표시 |
| [참고 프로세스 탐지](/Users/in05908_mac/study/zj-agent-mob/src/discover.rs) | 수동 실행 에이전트 발견 |

참고 동작을 새 구조에 맞게 구현한다. 소스 코드를 직접 가져오는 단계에서는 원본 라이선스와 고지 보존 여부를 확인한다.
