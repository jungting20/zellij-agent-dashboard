# Agent Dashboard 아키텍처

화면 pane과 상태 수집기의 수명을 분리한다. Zellij 서버가 백그라운드 수집기를 실행하고 화면 플러그인은 수집 결과를 읽고 사용자 요청을 전달한다. 아래 구조는 구현 방향이며 파일 접근과 인스턴스 소유권은 M0에서 확정한다.

## 구성

```text
도구별 훅 ── 공통 JSON 이벤트 ── 세션별 collector
                                      │
프로세스와 pane 관찰 ──────────────────┤
                                      ├── 세션 상태 스냅샷
                                      └── 변경 통지
                                             │
                                  dashboard 플러그인
                                             │
                                  명시적인 조작 요청
                                             │
                                 대상 세션 collector
                                             │
                              Zellij API 또는 호스트 명령
```

collector와 dashboard는 동일 WASI 바이너리의 별도 설정 역할을 우선 사용한다. Zellij는 같은 URL이라도 설정이 다르면 별도 pipe 목적지로 취급한다. 대상 설정은 설치 프로그램, 훅, 화면에서 동일하게 사용한다. [Zellij pipe 목적지 문서](https://zellij.dev/documentation/plugin-pipes)

초기 Rust workspace는 `crates/dashboard-core`와 `crates/dashboard-plugin`으로 나눈다. 코어는 이벤트와 상태 전이를 담당하고 플러그인은 Zellij 이벤트, 타이머, 렌더링, 호스트 어댑터를 연결한다. 호스트 파일 갱신이나 프로세스 탐지에 필요하면 한 요청 후 종료하는 보조 명령을 추가한다.

## 상태 소유권

각 세션의 collector가 해당 세션 에이전트의 상태와 조작을 담당한다. 다른 세션은 스냅샷을 읽고 요청을 대상 세션으로 전달한다. 초기 단계에서는 전역 작업 큐나 전역 스케줄러를 만들지 않는다.

collector는 논리적 역할이며 실제 인스턴스 수가 하나라는 보장은 두지 않는다. 여러 클라이언트의 인스턴스가 같은 메시지를 받을 수 있으므로, 저장 작업과 조작 실행은 검증된 단일 작성자 경로와 요청 ID 중복 배제를 통해 처리한다. 단순 타임스탬프 비교만으로 작성자를 선출하지 않는다. 호스트 잠금 또는 Zellij 인스턴스 라우팅의 실제 동작을 M0에서 비교해 선택한다.

선택한 행, 검색어, 입력 초안은 화면별 상태다. 고정과 별칭은 공유 설정으로 저장하되 에이전트 실행 세대에 연결한다. 종료한 에이전트의 설정이 재사용된 pane ID의 다른 프로세스에 자동 적용되지 않게 한다.

## 에이전트 식별

원본 세션 이름은 호스트 조작에 사용하고 파일 경로에는 충돌을 피하는 별도 키를 사용한다. 세션 이름만으로 실행 생명주기를 식별하지 않는다.

| 필드 | 목적 |
|---|---|
| `agent_id` | 화면과 명령이 참조하는 안정적인 에이전트 실행 ID |
| `session_name` | Zellij가 사용하는 원본 세션 이름 |
| `session_epoch` | 같은 이름으로 다시 시작한 세션 구분 |
| `pane_id` | 해당 세션의 실제 Zellij pane |
| `incarnation_id` | pane 안에서 재시작한 에이전트 실행 구분 |
| `tool_session_id` | 도구가 제공하면 연결하는 도구 자체 세션 ID |

런처로 만든 에이전트는 실행 ID를 환경에 전달할 수 있다. 수동 실행은 도구 세션 ID와 실제 프로세스 시작 정보를 대조해 실행 세대를 구성한다. 입력·종료 전에 실행 세대를 확인할 수 없는 경우 현재 대상 확인을 다시 요청하고 실행을 보류한다.

## 이벤트와 상태 계약

메시지는 문자열 JSON payload로 전송한다. 쉼표로 필드를 합치는 방식은 사용하지 않아 경로, 한국어, 여러 줄 지시를 보존한다. 예시는 실제 pane을 대상으로 한 실행 요청이 아닌 계약 초안이다.

```json
{
  "schema_version": 1,
  "event_id": "example-event",
  "agent_id": "example-agent",
  "session_name": "example-session",
  "session_epoch": "example-session-run",
  "pane_id": 7,
  "incarnation_id": "example-agent-run",
  "tool": "claude",
  "kind": "turn_started",
  "sequence": 12,
  "observed_at": "2026-10-07T00:00:00Z",
  "summary": "요청 요약"
}
```

순서 번호는 도구가 제공하거나 어댑터에서 신뢰할 수 있게 생성할 때 사용한다. 서로 다른 훅 프로세스의 임의 카운터나 벽시계만으로 순서를 확정하지 않는다. 순서를 판단할 수 없는 이벤트는 명시적인 상태 재확인 경로를 사용한다.

| 공통 이벤트 | 상태 또는 처리 |
|---|---|
| `session_started` | `idle` |
| `turn_started`, `tool_started`, `tool_finished` | `working` |
| `input_required`, `permission_required` | `waiting` |
| `turn_finished` | `done` |
| `turn_failed` | `failed` |
| `compact_started`, `compact_finished` | `compact`, `working` |
| `session_ended` | 실행 종료 기록 |
| `process_discovered` | 훅 보고 전이면 `found` |

보고 없음은 별도 `stale` 정보이고 세션 생존 여부는 별도 `liveness` 정보다. 시간 경과만으로 완료나 종료로 바꾸지 않는다. 도구가 제공하지 않는 이벤트를 지원한다고 표시하지 않는다.

같은 실행의 유효한 훅을 상태 판별의 우선 근거로 사용한다. 프로세스와 pane 관찰은 생존 여부를 확인한다. 화면 탐지는 훅 미지원 또는 연결 문제의 보완 경로로 사용하며 오래된 화면이 새 훅 상태를 덮어쓰지 않게 한다.

## 저장과 복구

계획한 호스트 저장 위치는 `${XDG_STATE_HOME:-$HOME/.local/state}/zellij-agent-dashboard`다. 일시적인 이벤트 전달 파일과 durable 설정을 구분하고 실제 공유 파일 접근 방법은 M0에서 확인한다. WASI 파일 경로를 호스트 경로와 동일하다고 가정하지 않는다.

- 세션 스냅샷은 현재 실행 ID, 정보 출처, 보고 시각, 버전, revision을 포함한다.
- 고정과 별칭, 조작 요청 결과는 재로딩 후 복원할 설정으로 저장한다.
- 원자적 파일 교체와 잠금으로 부분 쓰기와 작성자 충돌을 처리한다.
- collector 재로딩 시 실제 세션과 프로세스를 확인하기 전에는 저장 상태를 미확인 캐시로 표시한다.
- 종료한 세션의 상태와 생존 여부를 구분하고 이전 pane 세대로 입력을 전달하지 않는다.

초기 저장은 버전이 있는 JSON으로 시작한다. 기존 SQLite 데이터는 자동으로 읽거나 수정하지 않는다. schema 변경 시 기본값과 migration을 함께 추가한다.

## 조작과 결과

모든 조작 요청은 `request_id`, 대상 에이전트와 실행 세대, 동작과 매개변수를 갖는다. 수집기는 대상이 현재 실행과 일치하는지 확인하고 호스트 어댑터로 실행한다. 요청 완료는 명령을 발행한 시점이 아닌 호스트 결과를 확인한 시점에 표시한다.

실행 전 intent와 실행 후 결과를 저장한다. 호스트 조작 이후 결과 저장 전에 중단되면 성공 여부를 `uncertain`으로 표시하고 자동 재전송하지 않는다. 파일 저장과 외부 pane 조작이 하나의 트랜잭션이라고 가정하지 않는다.

같은 세션의 pane 이동·입력·종료에는 확인한 플러그인 API를 사용하고, 다른 세션의 조작은 대상 collector 또는 유한한 Zellij CLI 명령으로 연결한다. Zellij의 권한과 명령 결과 이벤트를 사용한다. [플러그인 명령](https://zellij.dev/documentation/plugin-api-commands), [플러그인 이벤트](https://zellij.dev/documentation/plugin-api-events)

## 확인할 실행 조건

| 항목 | 검증 내용 |
|---|---|
| 수집기 수명 | 화면 종료와 detach 후 이벤트 및 타이머 처리 |
| 다중 클라이언트 | 한 이벤트의 중복 수신, 한 조작의 단일 실행 |
| 권한 | 최초 로딩과 권한 거절 후 복구 |
| 파일 접근 | WASI 경로, 공유 저장 위치, 원자적 교체와 잠금 |
| 다른 세션 | 원본 이름을 이용한 정확한 이동과 조작 |
| 도구 어댑터 | 설치 버전의 이벤트, JSON 형식, 종료와 실패 구분 |

백그라운드 로딩과 pipe의 일반 지원은 공식 문서로 확인했다. 위 실행 조건의 실제 조합은 M0에서 임시 세션으로 검증한다.
