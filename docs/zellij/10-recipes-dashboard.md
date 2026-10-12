# Zellij CLI 자동화 Recipes와 Agent Dashboard 설계

> 조사 범위: 공식 CLI Recipes, Programmatic Control, Subscribe, Plugin/Pipe 및 관련 CLI Actions.
>
> **실행 검증은 하지 않았다.** 아래 shell/KDL/plugin protocol 예시는 설계용이다. 대상 OS, shell, Zellij version, 설치 binary의 `--help`와 실제 session에서 검증해야 한다.
>
> 표기 규칙: **[공식 사실]**은 공식 문서가 보장하거나 예시로 든 내용, **[추천 패턴]**은 agent dashboard 운영 설계 제안이다.

## 1. Dashboard 구축에 필요한 공식 primitive

| 계층 | primitive | 역할 |
|---|---|---|
| session | `attach --create-background` | attach하지 않은 headless workspace 생성 |
| tab | `new-tab`, `list-tabs`, `--tab-id` | agent group/phase 분리 |
| pane | `new-pane`, `list-panes`, pane ID 반환 | agent process 실행 및 lifecycle 확인 |
| input | `paste`, `send-keys`, `write-chars`, `write` | interactive pane 제어 |
| output | `subscribe`, `dump-screen --full` | 실시간 화면과 최종 scrollback 관측 |
| synchronization | blocking 계열 flag, `exited`, `exit_status` | 단계 진행 조건 |
| presentation | floating, pinned, borderless, pane color | 상태 UI/overlay |
| plugin | `launch-plugin`, `pipe`, Plugin API events | session 내부 dashboard UI와 structured events |
| layout | `new-tab --layout[-string]`, `override-layout` | 반복 가능한 workspace 구성 |

## 2. Dashboard용 핵심 CLI 플래그

### 2.1 `zellij action new-pane`

```text
zellij action new-pane [OPTIONS] [-- <COMMAND>...]
```

전체 옵션:

```text
  -c, --close-on-exit
      --cwd <CWD>
  -d, --direction <DIRECTION>
  -f, --floating
  -i, --in-place
      --close-replaced-pane
  -n, --name <NAME>
  -p, --plugin <PLUGIN>
      --configuration <CONFIG>
      --skip-plugin-cache
  -s, --start-suspended
  -x, --x <X>
  -y, --y <Y>
      --width <WIDTH>
      --height <HEIGHT>
      --pinned <PINNED>
      --stacked
      --tab-id <TAB_ID>
      --pane-id <PANE_ID>
  -b, --blocking
      --block-until-exit-success
      --block-until-exit-failure
      --block-until-exit
      --near-current-pane
      --no-focus
      --borderless <BORDERLESS>
```

주요 제약:

- `--direction`과 `--floating` 충돌
- `--in-place`는 `--floating`/`--direction`과 충돌
- `--close-replaced-pane`는 `--in-place` 필요
- `--plugin`은 command 및 `--direction`과 충돌
- x/y/width/height/pinned는 floating에 사용
- `--stacked`는 floating/direction과 충돌
- `--tab-id`는 `--in-place`, `--near-current-pane`과 충돌
- `--pane-id`는 `--in-place`로 대체할 pane을 지정하며 기본은 focused pane
- blocking 네 종류는 상호 배타적

Dashboard에서 중요한 옵션:

- `--no-focus`: connected client의 focus를 빼앗지 않는다.
- `--tab-id`: 현재 focus에 의존하지 않고 정확한 tab에 worker를 만든다.
- direct command: pane shell prompt 상태를 거치지 않고 process를 실행한다.
- `--name`: 사람이 dashboard에서 역할을 구분한다.

### 2.2 top-level `zellij run`

`zellij run -- <COMMAND>`은 `new-pane`의 단축 경로다. 공식 Run 문서의 전체 옵션:

```text
  -b, --borderless <true|false>
      --block-until-exit
      --block-until-exit-failure
      --block-until-exit-success
      --blocking
  -c, --close-on-exit
      --close-replaced-pane
      --cwd <CWD>
  -d, --direction <DIRECTION>
  -f, --floating
  -i, --in-place
  -n, --name <NAME>
      --near-current-pane
      --no-focus
      --pinned <PINNED>
  -s, --start-suspended
      --stacked
      --tab-id <TAB_ID>
      --width <WIDTH>
  -x, --x <X>
  -y, --y <Y>
  -h, --help
```

외부 controller가 다른 session을 대상으로 할 때는 `zellij --session "$session" action new-pane ...` 형태가 더 명시적이다.

### 2.3 `zellij action new-tab`

```text
zellij action new-tab [OPTIONS] [-- <COMMAND>...]
```

전체 옵션:

```text
  -c, --cwd <CWD>
  -l, --layout <LAYOUT>
      --layout-dir <LAYOUT_DIR>
      --layout-string <LAYOUT_STRING>
  -n, --name <NAME>
      --initial-plugin <PLUGIN>
      --close-on-exit
      --start-suspended
      --block-until-exit-success
      --block-until-exit-failure
      --block-until-exit
      --no-focus
```

- command와 `--initial-plugin`은 충돌한다.
- `--layout-string`은 `--layout`과 충돌한다.
- `--close-on-exit`/`--start-suspended`는 initial command가 필요하다.
- `--no-focus`는 새 tab과 layout 내부 focus property가 모든 client focus를 바꾸지 않도록 한다.
- 성공 시 숫자 tab ID를 stdout으로 반환한다.

### 2.4 floating/overlay 관련 action

```bash
zellij action show-floating-panes --tab-id 3
zellij action hide-floating-panes --tab-id 3
zellij action are-floating-panes-visible --tab-id 3
zellij action toggle-pane-pinned --pane-id terminal_5
zellij action toggle-pane-embed-or-floating --pane-id terminal_5
zellij action set-pane-borderless --pane-id terminal_5 --borderless true
zellij action set-pane-color --pane-id terminal_5 --bg '#3a0000'
zellij action set-pane-color --pane-id terminal_5 --reset
```

`show`/`hide`는 공식 CLI Actions 기준:

- `0`: 실제 상태 변경
- `1`: tab not found
- `2`: 이미 원하는 상태

Programmatic Control 페이지의 code 1 설명과 불일치하므로 실제 binary 검증이 필요하다.

## 3. 기본 background workflow

**[공식 사실]** background session:

```bash
zellij attach --create-background agent-dashboard
```

layout 지정:

```bash
zellij attach --create-background agent-dashboard \
  options --default-layout compact
```

이후 다른 process에서 global `--session`으로 제어한다.

```bash
worker_pane=$(
  zellij --session agent-dashboard action new-pane \
    --name worker-1 \
    --no-focus \
    -- some-agent-command --task task-1
)

zellij --session agent-dashboard subscribe \
  --pane-id "$worker_pane" \
  --format json
```

## 4. 추천 Dashboard 상태 모델

Zellij의 화면/프로세스 상태를 하나로 합치지 말고 다음처럼 분리한다.

```text
Session
  id/name
  lifecycle: creating | running | unavailable | closing | closed

Agent
  agent_id
  tab_id
  pane_id
  desired_state: queued | running | cancel_requested | stopped
  process_state: not_started | running | exited
  exit_status: null | integer
  pane_state: open | held | closed
  presentation_state: hidden | visible | focused
  last_viewport
  final_scrollback_path
  sequence
```

중요 구분:

- process exit != pane close
- pane held != process running
- subscribe exit != child success
- tab position != tab ID
- JSON pane numeric ID != canonical `terminal_N`/`plugin_N`

## 5. Pattern A: 외부 controller + terminal panes

### 5.1 언제 적합한가

- agent process 자체 stdout/stderr를 controller가 별도로 저장할 수 있음
- UI는 Zellij, orchestration은 shell/Python/다른 daemon에서 수행
- WASM plugin 개발 없이 빠르게 자동화

### 5.2 권장 순서

1. session 존재 여부 확인 또는 background 생성
2. 전용 tab 생성, 반환 tab ID 저장
3. 각 agent를 direct command pane으로 생성, 반환 pane ID 저장
4. `list-panes --json`을 주기적 authoritative snapshot으로 사용
5. `subscribe --format json`은 화면 변화/사람 표시용으로 사용
6. process exit 뒤 `dump-screen --full`
7. artifact 확보 뒤 pane close

### 5.3 설명용 controller 골격

```bash
#!/usr/bin/env bash
set -euo pipefail

session=${SESSION:-agent-board}

zellij attach --create-background "$session"

tab_id=$(
  zellij --session "$session" action new-tab \
    --name agents \
    --no-focus
)

worker_1=$(
  zellij --session "$session" action new-pane \
    --tab-id "$tab_id" \
    --name worker-1 \
    --no-focus \
    -- sh -lc 'exec some-agent-command --agent-id worker-1'
)

worker_2=$(
  zellij --session "$session" action new-pane \
    --tab-id "$tab_id" \
    --name worker-2 \
    --no-focus \
    -- sh -lc 'exec some-agent-command --agent-id worker-2'
)

printf '%s\n' "$worker_1" "$worker_2" > /tmp/agent-board.panes

zellij --session "$session" subscribe \
  --pane-id "$worker_1" \
  --pane-id "$worker_2" \
  --format json \
  --scrollback 200 \
| jq --unbuffered -c '
    if .event == "pane_update" then
      {
        pane_id,
        initial: .is_initial,
        last_line: (.viewport[-1] // "")
      }
    else
      .
    end
  '
```

이 예시는 실행하지 않았다. `sh -lc`는 shell이 반드시 필요한 경우에만 쓰고, 신뢰되지 않은 task를 문자열에 삽입하지 않는다.

### 5.4 기계 log는 pane 화면과 별도로

**[추천 패턴]** agent command가 지원하면 structured event file/socket/NDJSON을 별도로 기록한다. `subscribe`는 terminal viewport라서 완전한 log transport가 아니다.

예:

```text
agent command stdout/stderr -> append-only log/artifact
agent structured NDJSON     -> controller state reducer
Zellij subscribe            -> visual preview and anomaly hints
list-panes --json           -> Zellij lifecycle reconciliation
```

## 6. Pattern B: Zellij plugin dashboard

### 6.1 언제 적합한가

- Zellij session 안에 실시간 UI가 필요
- `PaneUpdate`, `TabUpdate`, `CommandPaneExited`, `PaneClosed` 같은 event를 직접 받고 싶음
- Pipe로 외부/다른 plugin과 structured message를 주고받고 싶음

### 6.2 권한

읽기 전용 dashboard의 후보:

- `ReadApplicationState`
- `ReadPaneContents`가 필요한 경우에만 추가
- CLI Pipe 응답/backpressure에 `ReadCliPipes`

실제 제어까지 하면 최소 필요 범위에서:

- `ChangeApplicationState`
- `OpenTerminalsOrPlugins`
- `WriteToStdin`
- `MessageAndLaunchOtherPlugins`

### 6.3 event 구독

```rust
fn load(&mut self, _configuration: BTreeMap<String, String>) {
    subscribe(&[
        EventType::TabUpdate,
        EventType::PaneUpdate,
        EventType::CommandPaneExited,
        EventType::PaneClosed,
    ]);
}
```

plugin event는 특정 순서를 따르지 않는다고 공식 lifecycle 문서가 경고한다. 다음 규칙을 둔다.

- event를 현재 state에 적용하는 reducer 사용
- context/request ID로 command 결과 상관관계 처리
- duplicate/late event를 허용
- 주기적 full `PaneUpdate`/snapshot으로 reconcile
- permission 승인 전에 온 event를 안전하게 무시/queue

### 6.4 외부 controller와 Pipe 연결

```bash
printf '%s' '{"schema_version":1,"kind":"snapshot.request","request_id":"r-1"}' \
| zellij --session agent-board pipe \
    --name agent.snapshot.request \
    --plugin agent-dashboard
```

plugin은 `PipeSource::Cli(pipe_id)`를 저장하고 `cli_pipe_output()`으로 NDJSON 응답을 보낼 수 있다. 여러 plugin instance가 응답할 수 있으므로 `dashboard_instance_id`를 포함한다.

## 7. Pattern C: hybrid dashboard

가장 견고한 조합:

- 외부 controller: process 실행, artifact, retry policy, timeout, durable state
- Zellij: pane/tab workspace와 human intervention
- plugin dashboard: 읽기 쉬운 status UI, focus/navigation, lightweight control
- Pipe: controller와 plugin 사이 command/event bridge

**[추천 패턴]** plugin을 orchestration의 유일한 durable source of truth로 두지 않는다. session/plugin 종료, permission 거부, version mismatch 시에도 controller가 작업 결과를 복구할 수 있어야 한다.

## 8. Human-in-the-loop pipeline

공식 blocking 기능은 사람이 output을 보고 고친 뒤 retry하는 dashboard에 맞는다.

```bash
zellij --session agent-board action new-pane \
  --block-until-exit-success \
  --name tests \
  -- cargo test

zellij --session agent-board action new-pane \
  --block-until-exit-success \
  --name release \
  -- cargo build --release

zellij --session agent-board action new-pane \
  --block-until-exit \
  --name deploy \
  -- ./deploy.sh
```

실패 시 held pane에서 `Enter`로 retry 가능하다. 단, pane을 닫아도 block이 풀릴 수 있으므로 “unblock == 성공”으로 모델링하지 않는다. 별도로 exit status를 확인한다.

무인 CI에는 indefinite retry 대기보다 timeout과 명시적 status polling이 낫다.

## 9. 상태 overlay와 시각 신호

### 9.1 borderless pinned overlay

공식 recipe 패턴:

```bash
status_pane=$(
  zellij --session agent-board action new-pane \
    --floating \
    --borderless true \
    --pinned true \
    --width '30%' \
    --height 5 \
    --x '68%' \
    --y 1 \
    --name agent-status \
    --no-focus \
    -- watch -n2 -t cat /tmp/agent-board.status
)
```

고려사항:

- borderless pane은 mouse로 이동하기 어려울 수 있다.
- fixed/percent 좌표는 작은 viewport에서 겹칠 수 있다.
- `watch`/`cat`은 OS 의존적이다.
- file writer와 reader 사이 atomic rename을 사용해 partial status를 피한다.

### 9.2 pane color

```bash
# running
zellij action set-pane-color --pane-id "$pane" --bg '#001a3a'

# failed
zellij action set-pane-color --pane-id "$pane" --bg '#5a0000'

# reset
zellij action set-pane-color --pane-id "$pane" --reset
```

**[추천 패턴]** 색만으로 상태를 전달하지 말고 pane title/text에도 `RUNNING`, `FAILED`, `DONE`을 표시한다. action 후 list/state로 존재 여부를 재확인하고 닫힌 pane 오류를 정상 race로 처리한다.

## 10. layout 활용

### 10.1 inline layout

```bash
zellij --session agent-board action new-tab \
  --name agents \
  --no-focus \
  --layout-string '
layout {
  pane split_direction="vertical" {
    pane name="coordinator"
    pane split_direction="horizontal" {
      pane name="worker-1"
      pane name="worker-2"
    }
  }
}'
```

### 10.2 runtime override

```bash
zellij --session agent-board action override-layout \
  --layout-string 'layout { pane; pane; pane; }' \
  --retain-existing-terminal-panes \
  --retain-existing-plugin-panes \
  --apply-only-to-active-tab
```

`--layout-string`은 parsing/validation 후 session으로 전송되어 syntax error가 즉시 보고된다고 공식 문서가 설명한다.

**[추천 패턴]** 이미 작업 중인 dashboard에는 `override-layout`이 pane을 닫을 수 있으므로 retain 옵션과 사전 `list-panes` snapshot을 사용한다.

## 11. Retry, timeout, cancellation

### 11.1 자동 retry

- held pane `Enter` retry는 사람 개입 workflow에 적합하다.
- 무인 retry는 command pane을 닫고 새 pane/새 attempt ID로 실행하는 편이 audit하기 쉽다.
- 동일 pane 재실행은 `CommandPaneExited`가 여러 번 발생하므로 attempt counter가 필요하다.

### 11.2 timeout

공식 CLI recipe는 timeout policy를 제공하지 않는다. controller에서 monotonic deadline을 관리한다.

```text
running -> cancel_requested -> SIGINT/keys -> grace period
        -> force close pane -> timed_out
```

`send-keys "Ctrl c"`는 pane foreground application에 입력하는 것이지 process tree kill 보장은 아니다. shell/process group 종료는 agent runner가 관리하는 것이 안전하다.

### 11.3 cancellation race

- cancel 직전에 process가 성공할 수 있다.
- `Ctrl-c` 직후 pane이 held/closed로 바뀔 수 있다.
- controller가 닫은 pane을 subscribe가 잠시 후 `pane_closed`로 알릴 수 있다.

최종 상태는 timestamp 한 개가 아니라 attempt ID, exit status, cancellation request sequence로 결정한다.

## 12. Concurrency 규칙

### 12.1 반드시 직렬화할 것

- pane 생성 후 그 ID로 입력
- 같은 pane에 대한 여러 `paste`/`send-keys`
- layout 변경과 해당 tab의 pane 배치
- plugin load 직후 첫 필수 handshake

```bash
pane=$(
  zellij --session "$session" action new-pane --no-focus --name worker
) &&
zellij --session "$session" action paste --pane-id "$pane" 'echo ready' &&
zellij --session "$session" action send-keys --pane-id "$pane" Enter
```

### 12.2 병렬화 가능한 것

서로 다른 pane 대상의 독립 action:

```bash
zellij --session "$session" action paste --pane-id terminal_1 'make build' &
zellij --session "$session" action paste --pane-id terminal_2 'make test' &
wait
```

단, 각 pane에 single writer를 유지한다.

### 12.3 reconciliation

실시간 event만 신뢰하지 않고 주기적으로:

```bash
zellij --session "$session" action list-panes --json
zellij --session "$session" action list-tabs --json
```

을 읽어 desired/observed state를 비교한다.

## 13. Scrollback 및 artifact recipe

### 13.1 종료 전후 캡처

```bash
row=$(
  zellij --session "$session" action list-panes --json \
  | jq -c --argjson id "${pane#terminal_}" \
      '.[] | select(.is_plugin == false and .id == $id)'
)

if [[ $(jq -r '.exited' <<<"$row") == true ]]; then
  zellij --session "$session" action dump-screen \
    --pane-id "$pane" \
    --full \
    --path "/tmp/${pane}.screen.txt"
fi
```

### 13.2 capture policy

- plain text와 ANSI capture를 분리한다.
- `dump-screen`은 rendered terminal content이며 raw process output이 아니다.
- secrets가 viewport/scrollback에 있을 수 있으므로 artifact 접근권한과 retention을 제한한다.
- pane title, cwd, command도 민감정보가 될 수 있다.

## 14. 보안과 운영 위험

| 위험 | 예 | 완화 |
|---|---|---|
| shell injection | task 문자열을 `sh -lc`에 합침 | argv 직접 전달, allowlist, escaping library |
| focus hijack | background agent 생성이 사용자 focus 변경 | `--no-focus`, `--tab-id` |
| 잘못된 pane | 숫자 ID만 저장 | canonical prefix ID와 `is_plugin` 확인 |
| stale session | 오래된 protocol session에 새 action | 최소 version, postcondition query, 재생성 전략 |
| log 손실 | viewport를 stdout log로 간주 | process log 별도 저장, final full dump |
| secret 노출 | scrollback, subscribe, pane metadata | redaction, 권한, retention, ANSI 제거 |
| plugin 공급망 | 원격 WASM URL/변경 가능한 alias | pinned artifact/checksum, 최소 권한, alias 검증 |
| Pipe 중복 | 여러 plugin instance | idempotency key, leader, producer ID |
| Pipe deadlock | plugin이 unblock 누락 | watchdog, cancel route, finally unblock |
| resource exhaustion | agent/pane 무제한 생성 | concurrency limit, pane limit, cleanup loop |
| layout destructive change | override가 pane을 닫음 | retain flags, snapshot, maintenance window |

## 15. Version/capability matrix

| 기능 | 공식 도입/확인 기준 |
|---|---|
| Plugin Pipes, aliases, background session | 0.40.0 release |
| pinned floating panes | 0.42.0 계열 release로 알려짐; 실제 binary help 확인 |
| CLI automation 대폭 확장, subscribe, 조건부 blocking, ID 반환 | 0.44.0 release |
| 조사한 latest release/docs.rs | 0.45.1 |

0.44.0은 client/server protobuf contract를 도입했다. 이후 version이 오래된 session에 연결될 수 있어도 새 기능은 오래된 session에서 조용히 실패할 수 있다.

**[추천 패턴]** dashboard 시작 시 capability probe:

```bash
zellij --version
zellij subscribe --help
zellij action list-panes --help
zellij action new-pane --help
zellij action pipe --help
```

그리고 작은 non-destructive query/action 뒤 expected field/state를 확인한다. help parsing 자체를 장기 API로 삼기보다 supported version 목록과 integration test를 유지한다.

## 16. 공식 사실과 추천 패턴 요약

### 공식 사실

- CLI action과 subscribe로 subprocess 기반 제어 가능
- pane/tab 생성 명령은 ID 반환
- pane ID는 terminal/plugin prefix로 구분
- subscribe는 rendered viewport snapshot NDJSON 제공
- blocking pane은 human retry workflow 지원
- `--no-focus`, `--tab-id`로 background 배치 가능
- Pipe는 broadcast/private 목적지, plugin first-message launch, backpressure 지원
- 여러 process가 같은 session을 동시에 제어할 때 수신 순서 외 전역 ordering 보장 없음

### 추천 패턴

- 외부 durable controller + Zellij UI + 선택적 plugin dashboard의 hybrid 구조
- pane별 single writer
- event stream + 주기적 JSON snapshot reconciliation
- process output과 rendered viewport 분리
- attempt/request/message ID 및 idempotency
- direct argv launch 우선, shell string injection 회피
- exit, held, closed를 분리한 상태 machine
- 실제 binary version/help와 postcondition 검증

## 17. 발견한 문서 불일치

1. `--blocking` 설명이 페이지별로 “pane close까지”와 “command 완료 + pane close까지”로 다르다. Run source/문서는 후자를 지지한다.
2. show/hide floating pane의 exit code 1 의미가 Programmatic Control과 CLI Actions에서 다르다.
3. top-level `zellij plugin` 온라인 옵션 표는 0.45.1 source의 `--close-replaced-pane`, `--pinned`, `--borderless`를 누락한다.
4. Plugin permission 서술 페이지는 Type Reference에 있는 `ReadCliPipes`, `MessageAndLaunchOtherPlugins`, `WebAccess`를 누락한다.
5. 현재 JSON 출력은 sample과 field 목록은 있지만 formal versioned schema가 없다.

## 18. 공식 자료

사용자가 지정한 공식 문서:

- https://zellij.dev/documentation/cli-recipes.html
- https://zellij.dev/documentation/programmatic-control.html
- https://zellij.dev/documentation/zellij-subscribe.html
- https://zellij.dev/documentation/zellij-plugin-and-pipe.html
- https://zellij.dev/documentation/plugin-loading.html
- https://zellij.dev/documentation/plugin-aliases.html
- https://zellij.dev/documentation/plugin-pipes.html

교차 확인에 사용한 공식 문서/소스:

- https://zellij.dev/documentation/cli-actions.html
- https://zellij.dev/documentation/zellij-run-and-edit.html
- https://zellij.dev/documentation/controlling-zellij-through-cli.html
- https://zellij.dev/documentation/plugin-api.html
- https://zellij.dev/documentation/plugin-api-commands.html
- https://zellij.dev/documentation/plugin-api-events.html
- https://zellij.dev/documentation/plugin-api-types.html
- https://zellij.dev/documentation/plugin-api-permissions.html
- https://zellij.dev/documentation/plugin-lifecycle.html
- https://docs.rs/zellij-utils/0.45.1/src/zellij_utils/cli.rs.html
- https://zellij.dev/news/welcome-screen-pipes-filepicker/
- https://zellij.dev/news/remote-sessions-windows-cli/
- https://github.com/zellij-org/zellij/releases/latest
