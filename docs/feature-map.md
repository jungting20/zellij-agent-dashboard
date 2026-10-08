# Agent Dashboard 기능 이관표

기존 대시보드의 기능을 조회, 기본 조작, 보조 메뉴, 후속 엔진으로 구분해 이관한다. 2026년 10월 8일 코드와 [실행 검증 기록](runtime-validation.md)을 기준으로 구현과 검증 범위를 구분한다. 구현한 메뉴도 전체 동작을 실제 검증한 것으로 간주하지 않는다.

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

## 현재 구현과 검증

자동 테스트는 해당 기능의 직접적인 테스트만 적는다. 공통 입력 편집·렌더링 테스트가 보조 메뉴의 외부 조작까지 검증하지는 않는다. 실제 Zellij 검증은 위 실행 검증 기록에 남은 결과를 기준으로 하며 이번 문서 갱신에서 다시 실행하지 않았다.

| 기능 | 구현 상태 | 자동 테스트 근거 | 실제 Zellij 검증 | 남은 작업 |
|---|---|---|---|---|
| 목록·상태·검색·선택·활동 | 구현 | `model` 상태·출처·세대, `view` 검색·선택·폭 테스트 | 검색·선택·fixture 상태 전환·훅 출처 전환 | 실제 도구 전체 턴과 권한 거절 UI 검증 |
| 세션·탭 그룹·고정 영역 | 구현 | `both_panels_group_sessions_and_tabs_and_remember_independent_selection` | 여러 세션 조회, 고정 저장·reload 보존 | 그룹·영역 전환 키의 전체 실검증 |
| 프로젝트 그룹 | 프로젝트 이름 표시, 독립 그룹 미구현 | 프로젝트별 그룹 전용 테스트 없음 | 독립 그룹 검증 없음 | 기존 프로젝트 그룹 동작과 비교·구현 |
| 부모·자식 계층 | launch ID로 연결, 계층 렌더링 구현 | `cycles_render_once_and_multiline_instruction_and_output_fit`, 숨긴 부모 상속 방지 | 부모 연결·worktree 전체 흐름 검증 없음 | 실제 생성·연결·재실행 검증; 수동 실행은 부모 정보 없이 표시 |
| pane·세션 이동 | 구현 | pane 부재·실행 세대 교체 거부 | Enter 세션 간 이동·오래된 대상 거부 | 자식 선택 메뉴 이동 검증 |
| 고정·별칭 | 저장·편집 구현 | 고정 멱등성·종료 보호; 별칭 전용 테스트 없음 | 호스트 고정·별칭·중복 결과·reload 보존 | Space·별칭 메뉴 전체 검증 |
| 여러 줄 입력·종료·새 실행 | 호스트·화면 구현 | 중복 입력, paste 뒤 세대 교체, 고정 보호, CLI pane 생성 | 호스트 fixture 생성·입력·종료·중복 억제 | 각 메뉴를 통한 전체 흐름·실제 도구 입력 검증 |
| 요청 기록·결과 조회 | 구현, 결과 불명확 시 자동 재전송 금지 | pending 복원·요청 ID 충돌·저장소 주입 | 요청 결과 재사용·reload 보존 | 실제 결과 기록 전 helper 중단·복구 검증 |
| 마지막 지시·전체 보기 | 훅 prompt와 대시보드 입력으로 저장·표시 | 로컬 지시 신호·여러 줄 렌더링 | 지시 popup 전체 흐름 검증 없음 | 실제 prompt 훅·`p` 표시 검증 |
| 마지막 출력 preview | 현재 pane 화면 조회 구현 | CLI 화면 조회·대상 확인 관련 테스트 | fixture preview | 실제 도구 출력·화면 변경 중 조회 검증 |
| 외부 에디터 | 파일·pane 실행·종료 후 복원 구현 | 에디터 전용 테스트 없음 | 검증 기록 없음 | `I`/Ctrl+e → 편집 → 복원 → 전송·실패 검증 |
| 최근 경로·도구 선택 | 최근 100개 경로 저장·catalog·메뉴 구현 | 최근 경로·메뉴 실행 전용 테스트 없음 | 메뉴 전체 검증 없음 | 경로 선택·직접 입력·reload·실행 검증 |
| worktree·lazygit·자식 셸·병합 지시 | 호스트 조작·메뉴 구현 | 각 조작 전용 테스트 없음 | 검증 기록 없음 | 격리 Git 저장소에서 성공·실패·중복·세대 교체 검증 |

입력·종료·실행의 실제 검증은 모델 요청을 하지 않는 Codex fixture에 대한 호스트 호출이다. 이를 실제 Codex 대화나 모든 화면 메뉴의 검증으로 확대하지 않는다. 요청 기록은 현재 4,096개 제한이며 자동 정리 경로는 없다.

## 도구별 연결 순서

| 도구 | 기존 실행 파일 | 현재 기준과 새 연결 방향 |
|---|---|---|
| Claude | `claude` | 프로세스 발견·훅·화면 어댑터 구현, 2.1.70 SessionStart 실동작 확인 |
| Codex | `codex` | 프로세스 발견·화면 어댑터 구현, 상세 훅 연결은 후속 작업 |
| Cursor CLI | `agent` | 프로세스 발견·화면 어댑터 구현, 실제 실행 발견 확인, 전체 대화 흐름 검증은 후속 작업 |
| Gemini 프로필 | `agy`, `gemini` | native/Node 프로세스 발견·화면 어댑터 구현, 전체 대화 흐름 검증은 후속 작업 |
| Hermes | `hermes` | 프로세스 발견 구현, 상세 상태와 실동작 검증은 후속 작업 |
| Pi | `pi`, Node의 `pi-coding-agent` | 프로세스 발견·실행 메뉴 구현, 상세 상태 어댑터·실동작 검증은 후속 작업 |

훅, 확장 API, 구조화된 이벤트의 지원 여부는 설치된 도구 버전별로 검증한다. 어댑터가 없는 도구도 프로세스 발견을 지원하면 `found`로 표시할 수 있지만 작업·대기·완료 상태를 추정해서 확정하지 않는다.

## 현재 화면과 상태 저장

목록, 상태 긴급도 정렬, 검색, 실행 ID를 유지하는 선택, 마지막 보고 시점, stale/cached 표시와 pane 이동을 구현했다. 세션·탭 그룹, 부모·자식 계층과 고정/일반 영역의 선택을 제공한다. 프로젝트 이름은 행에 표시하며 프로젝트별 독립 그룹은 없다. 최근 상태 변화는 한 줄로 표시한다.

여러 collector의 상태 갱신은 SQLite 트랜잭션과 공유 수집 예약으로 제어한다. 고정·별칭·요청 결과와 launch ID의 부모 연결 정보를 저장한다. 입력·종료·새 실행과 보조 메뉴는 구현했으며 검증 범위는 위 표를 따른다. 상세 구조와 버전·복구 정책은 [아키텍처](architecture.md)를 참고한다.

## 참조 기준

2026년 10월 7일 로컬 소스를 기준으로 작성했다. 링크는 같은 머신의 참고 저장소를 가리킨다.

- 기존 대시보드: `/Users/in05908_mac/zellij-with-codeagent`, commit `1f49244a9af40d0ae936fb4088eca297810e3a64`
- 훅과 상태 파일 참고: `/Users/in05908_mac/study/zj-agent-mob`, commit `010bd2cc3ab232c799774c0a3320ea41418a6798`

| 참조 소스 | 활용할 동작 |
|---|---|
| [기존 키와 화면 상태](/Users/in05908_mac/zellij-with-codeagent/internal/agentdashboard/model.go) | 기능, 선택과 화면 수명 |
| [기존 상태 판별](/Users/in05908_mac/zellij-with-codeagent/internal/codingagent/detector.go) | 훅 미연결 실행의 화면 감지 경로 |
| [기존 Monitor](/Users/in05908_mac/zellij-with-codeagent/internal/codingagent/monitor.go) | 세대 검증과 상태 변화 |
| [도구 프로필](/Users/in05908_mac/zellij-with-codeagent/internal/codingagent/profile.go) | 실행 파일과 도구 구분 |
| [기존 백그라운드 bridge](/Users/in05908_mac/zellij-with-codeagent/plugins/agent-next-bridge/src/main.rs) | 권한과 다중 클라이언트 처리 참고 |
| [참고 훅](/Users/in05908_mac/study/zj-agent-mob/scripts/zj-agent-mob-hook.sh) | 공통 상태 매핑과 세션 간 전달 |
| [참고 상태 관리](/Users/in05908_mac/study/zj-agent-mob/src/state.rs) | 상태 통합과 오래된 보고 표시 |
| [참고 프로세스 탐지](/Users/in05908_mac/study/zj-agent-mob/src/discover.rs) | 수동 실행 에이전트 발견 |

참고 동작을 새 구조에 맞게 구현한다. 소스 코드를 직접 가져오는 단계에서는 원본 라이선스와 고지 보존 여부를 확인한다.
