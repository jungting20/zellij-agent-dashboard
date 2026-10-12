# Zellij Subscribe와 Programmatic Control

> 조사 범위: 공식 Programmatic Control, Zellij Subscribe, CLI Recipes, CLI Actions 및 0.45.1 공식 CLI/API 문서.
>
> **실행 검증은 하지 않았다.** 예시의 실제 동작, shell quoting, exit code, 설치 binary의 플래그는 대상 환경에서 확인해야 한다.
>
> 표기 규칙: **[공식 사실]**은 문서에 명시된 계약이고, **[추천 패턴]**은 안정적인 자동화를 위한 제안이다.

## 1. 제어 모델

**[공식 사실]** 외부 프로세스는 별도 socket API, HTTP API, language binding 없이 다음을 subprocess로 호출해 Zellij를 제어할 수 있다.

- query: `zellij action list-panes`, `list-tabs`, `current-tab-info`, `list-clients`, `dump-screen`, `dump-layout`
- mutate: `new-pane`, `new-tab`, `paste`, `send-keys`, `close-pane`, layout/appearance/session actions
- observe: `zellij subscribe`
- synchronize: `new-pane`/`zellij run`의 blocking 계열 플래그
- 다른 session 대상: global `zellij --session <SESSION>`

기본 제어 loop:

1. background session 생성
2. pane/tab 생성과 반환 ID 저장
3. ID 대상으로 명령 또는 입력 전송
4. `subscribe` 또는 `dump-screen`/`list-panes`로 관측
5. 결과에 따라 다음 action 수행

## 2. Pane ID와 Tab ID

### 2.1 Pane ID

**[공식 사실]** terminal pane과 plugin pane의 숫자 ID는 겹칠 수 있다.

- `terminal_1`
- `plugin_1`
- bare integer `1`: `terminal_1`과 동치

terminal pane에서는 `$ZELLIJ_PANE_ID`로 자신의 숫자 ID를 알 수 있다. CLI에서 pane을 명확히 지정할 때는 prefix를 붙인 문자열을 권장한다.

```bash
zellij action list-panes
```

```text
PANE_ID      TYPE      TITLE
terminal_1   terminal  /bin/bash
plugin_0     plugin    tab-bar
terminal_2   terminal  vim main.rs
```

### 2.2 JSON의 숫자 `id` 주의

`list-panes --json`은 외부 CLI 문자열 ID가 아니라 숫자 `id`와 `is_plugin`을 별도 field로 제공한다.

```json
{
  "id": 1,
  "is_plugin": false
}
```

**[추천 패턴]** JSON을 읽을 때 다음처럼 canonical ID를 재구성한다.

```jq
.[] | ((if .is_plugin then "plugin_" else "terminal_" end) + (.id|tostring))
```

숫자만 저장하면 `terminal_1`과 `plugin_1`을 혼동할 수 있다.

### 2.3 Tab ID

CLI Actions 문서는 tab ID를 stable ID로 설명한다. `list-tabs --json`의 `tab_id`를 사용하고, 표시 순서인 `position`과 혼동하지 않는다.

```bash
zellij action list-tabs --json
zellij action current-tab-info --json
```

## 3. `zellij subscribe` 전체 인터페이스

### 3.1 문법

```text
zellij [--session <SESSION_NAME>] subscribe [OPTIONS] --pane-id <PANE_ID>...
```

### 3.2 전체 옵션

| 짧은/긴 옵션 | 값 | 필수 | 동작 |
|---|---:|---:|---|
| `-p, --pane-id` | `<PANE_ID>` | 예 | 한 개 이상의 pane. `terminal_N`, `plugin_N`, bare integer 지원 |
| `-s, --scrollback` | 선택적 `[LINES]` | 아니오 | 최초 전달에 scrollback 포함. 값 생략 시 전체, 숫자면 마지막 N lines |
| `-f, --format` | `<raw|json>` | 아니오 | 기본 `raw`; `json`은 NDJSON |
| `--ansi` | 없음 | 아니오 | 색상/스타일 ANSI escape 보존 |

0.45.1 CLI 소스에서 `--pane-id`는 required `Vec<String>`이며 한 번에 1개 이상 값을 받도록 정의되어 있다. 온라인 예시는 안전하게 옵션을 반복한다.

```bash
zellij subscribe \
  --pane-id terminal_1 \
  --pane-id plugin_2 \
  --format json \
  --scrollback 100 \
  --ansi
```

`--scrollback`은 0.45.1 소스에서 값 없이 주어질 때 내부적으로 `0`을 사용하며, 사용자 문서는 이를 “전체 scrollback” 의미로 설명한다.

### 3.3 최초/후속/종료 동작

**[공식 사실]**

- subscribe 직후 각 대상 pane의 전체 현재 viewport가 즉시 전달된다.
- `--scrollback`이 있으면 scrollback은 최초 전달에서 viewport보다 앞선다.
- JSON 최초 event는 `is_initial: true`다.
- 이후에는 viewport content가 바뀔 때만 전달된다.
- 이후 전달에는 scrollback이 포함되지 않는다.
- 모든 구독 pane이 닫히거나 session이 끝나면 client가 자동 종료한다.
- command process가 exit했더라도 held pane이 닫히지 않았다면 `subscribe` 종료 조건과 다르다.

### 3.4 raw 출력

raw mode는 viewport lines를 stdout에 쓰고 flush한다. 여러 pane을 함께 구독할 때 raw record 자체에는 pane ID envelope가 없다.

**[추천 패턴]** 여러 pane의 기계 처리는 항상 JSON mode를 사용한다. raw mode는 단일 pane을 사람이 보거나 단순 grep할 때만 쓴다.

### 3.5 NDJSON schema

공식 문서는 두 event를 제시한다. 아래 타입은 공식 sample을 구조화한 표현이며 별도의 versioned JSON Schema 파일은 아니다.

```ts
type SubscribeEvent = PaneUpdate | PaneClosed;

type PaneUpdate = {
  event: "pane_update";
  pane_id: string;          // 예: "terminal_1", "plugin_2"
  viewport: string[];       // 렌더링된 현재 viewport 전체 lines
  scrollback: string[] | null;
  is_initial: boolean;
};

type PaneClosed = {
  event: "pane_closed";
  pane_id: string;
};
```

공식 응답 예:

```json
{"event":"pane_update","pane_id":"terminal_1","viewport":["line1","line2"],"scrollback":null,"is_initial":true}
{"event":"pane_closed","pane_id":"terminal_1"}
```

여러 pane event가 한 stdout stream에 섞이므로 항상 `pane_id`로 분기한다.

### 3.6 viewport snapshot이지 process stdout log가 아님

**[공식 사실]** subscribe는 “rendered pane viewport”를 변화 시점마다 보낸다. append-only command stdout stream이라고 명시하지 않는다.

따라서 다음 현상이 가능하다.

- 같은 줄이 여러 snapshot에 반복된다.
- progress bar처럼 같은 화면 줄이 덮어써진다.
- viewport 밖으로 밀린 과거 내용은 후속 event에서 사라진다.
- TUI의 최종 화면은 원본 byte stream과 다르다.

**[추천 패턴]**

- 정확한 command stdout/stderr가 필요하면 command 자체를 파일/pipe로 수집한다.
- terminal 화면 상태 감지가 목적일 때만 subscribe를 source of truth로 삼는다.
- output pattern 감지는 snapshot별 중복 제거나 이전 viewport와 diff를 적용한다.
- final artifact는 pane 종료를 확인한 뒤 `dump-screen --full`로 따로 캡처한다.

### 3.7 ANSI

기본은 ANSI styling 제거다. `--ansi`는 색/스타일 sequence를 보존한다. Programmatic Control 문서는 cursor movement가 아니라 이미 렌더링된 lines에 대한 styling이라고 설명한다.

**[추천 패턴]** 기계 parsing은 기본 plain text를 사용한다. ANSI가 의미를 가질 때만 보존하며, escape sequence를 HTML/shell/log viewer에 그대로 주입하지 않는다.

## 4. point-in-time 관측

### 4.1 `dump-screen` 전체 옵션

```text
zellij action dump-screen [OPTIONS]
      --path <PATH>          파일로 저장. 생략 시 STDOUT
  -f, --full                 전체 scrollback 포함
  -p, --pane-id <PANE_ID>    특정 pane. 생략 시 focused pane
  -a, --ansi                 ANSI styling 보존
```

예:

```bash
zellij --session agents action dump-screen \
  --pane-id terminal_7 \
  --full
```

### 4.2 선택 기준

| 요구 | 도구 |
|---|---|
| 지금 화면 | `dump-screen` |
| 전체 현재 scrollback | `dump-screen --full` |
| 변화 실시간 수신 | `subscribe` |
| 특정 text까지 기다림 | `subscribe --format json` + stateful filter |
| 종료 뒤 최종 화면 | pane exit 확인 후 `dump-screen --full` |
| 간헐 polling | `dump-screen` loop |

### 4.3 scrollback 보존 위험

`--close-on-exit`로 pane이 즉시 닫히면 뒤늦은 `dump-screen` 대상이 없어질 수 있다. final scrollback이 중요하면 pane을 held 상태로 남기고 `exited`/`exit_status` 확인 후 dump한 뒤 닫는다.

## 5. 구조화 query JSON

### 5.1 `list-panes --json`

옵션:

```text
zellij action list-panes
  -t, --tab       tab 정보 포함
  -c, --command   실행 command 정보 포함
  -s, --state     focus/floating/exited 등 상태 포함
  -g, --geometry  위치/크기 포함
  -a, --all       모든 field 포함
  -j, --json      JSON 출력
```

공식 sample의 전체 pane object:

```json
{
  "id": 1,
  "is_plugin": false,
  "is_focused": true,
  "is_fullscreen": false,
  "is_floating": false,
  "is_suppressed": false,
  "title": "/bin/bash",
  "exited": false,
  "exit_status": null,
  "is_held": false,
  "pane_x": 0,
  "pane_content_x": 1,
  "pane_y": 1,
  "pane_content_y": 2,
  "pane_rows": 24,
  "pane_content_rows": 22,
  "pane_columns": 80,
  "pane_content_columns": 78,
  "cursor_coordinates_in_pane": [0, 5],
  "terminal_command": null,
  "plugin_url": null,
  "is_selectable": true,
  "tab_id": 0,
  "tab_position": 0,
  "tab_name": "Tab #1",
  "pane_command": "bash",
  "pane_cwd": "/home/user/project"
}
```

공식 sample을 기준으로 한 실용 타입:

```ts
type PaneInfoJson = {
  id: number;
  is_plugin: boolean;
  is_focused: boolean;
  is_fullscreen: boolean;
  is_floating: boolean;
  is_suppressed: boolean;
  title: string;
  exited: boolean;
  exit_status: number | null;
  is_held: boolean;
  pane_x: number;
  pane_content_x: number;
  pane_y: number;
  pane_content_y: number;
  pane_rows: number;
  pane_content_rows: number;
  pane_columns: number;
  pane_content_columns: number;
  cursor_coordinates_in_pane: [number, number] | null;
  terminal_command: string | null;
  plugin_url: string | null;
  is_selectable: boolean;
  tab_id: number;
  tab_position: number;
  tab_name: string;
  pane_command?: string;
  pane_cwd?: string;
};
```

`pane_command`와 `pane_cwd`는 plugin sample에 나타나지 않으므로 consumer는 optional로 다루는 것이 안전하다. 공식 문서는 versioned JSON Schema나 field 안정성 정책을 제공하지 않는다.

### 5.2 `list-tabs --json` / `current-tab-info --json`

`list-tabs` 옵션:

```text
zellij action list-tabs
  -s, --state
  -d, --dimensions
  -p, --panes
  -l, --layout
  -a, --all
  -j, --json
```

`current-tab-info` 옵션:

```text
zellij action current-tab-info
  -j, --json
```

공식 JSON sample:

```json
{
  "position": 0,
  "name": "Tab #1",
  "active": true,
  "panes_to_hide": 0,
  "is_fullscreen_active": false,
  "is_sync_panes_active": false,
  "are_floating_panes_visible": false,
  "other_focused_clients": [],
  "active_swap_layout_name": "default",
  "is_swap_layout_dirty": false,
  "viewport_rows": 24,
  "viewport_columns": 80,
  "display_area_rows": 26,
  "display_area_columns": 80,
  "selectable_tiled_panes_count": 2,
  "selectable_floating_panes_count": 0,
  "tab_id": 0,
  "has_bell_notification": false,
  "is_flashing_bell": false
}
```

`list-tabs --json`은 이 object의 array이고 `current-tab-info --json`은 단일 object다.

### 5.3 다른 query

- `list-clients`: connected client ID, focused pane, 감지 가능한 running program. JSON이라고 문서화되지 않았다.
- `query-tab-names`: plain text tab name 목록.
- `dump-layout`: KDL.

기계 처리 계약이 필요한 곳에는 JSON 지원 명령을 우선한다.

## 6. 리소스 생성과 반환값

공식 문서상 stdout으로 ID를 반환하는 주요 명령:

| 명령 | 반환 예 |
|---|---|
| `new-pane` | `terminal_5` |
| `edit` | pane ID |
| `launch-plugin` | plugin pane ID |
| `launch-or-focus-plugin` | plugin pane ID |
| `new-tab` | 숫자 tab ID |
| `go-to-tab-name --create` | 실제 생성된 경우 tab ID |

```bash
PANE_ID=$(zellij --session agents action new-pane --name worker -- cargo test)
zellij --session agents action dump-screen --pane-id "$PANE_ID" --full
```

**[추천 패턴]** 반환값을 임의 parsing하지 말고 trim한 뒤 `^(terminal|plugin)_[0-9]+$` 또는 tab integer를 validation한다. command substitution 실패와 빈 문자열을 확인한다.

## 7. 입력 인터페이스

### 7.1 `paste`

```text
zellij action paste [--pane-id <PANE_ID>] <TEXT>
```

bracketed paste mode를 사용한다. multi-line text에 `write-chars`보다 빠르고 견고하다고 공식 recipe가 설명한다.

### 7.2 `write-chars`

```text
zellij action write-chars [--pane-id <PANE_ID>] <CHARS>
```

문자열을 terminal에 쓴다.

### 7.3 `write`

```text
zellij action write [--pane-id <PANE_ID>] <BYTE>...
```

byte array를 쓴다.

```bash
zellij action write --pane-id terminal_3 102 111 111
```

### 7.4 `send-keys`

```text
zellij action send-keys [--pane-id <PANE_ID>] <KEY>...
```

사람이 읽는 key 이름을 보낸다.

```bash
zellij action send-keys --pane-id terminal_3 "Alt b" "Enter"
```

### 7.5 shell injection과 command launch

`paste "cargo build"` 후 `Enter`는 pane 안의 현재 shell/application에 입력을 주입한다. prompt 상태, shell quoting, foreground TUI에 따라 의미가 달라진다.

**[추천 패턴]** 비대화형 worker는 가능하면 shell에 text를 보내지 말고 직접 command pane을 생성한다.

```bash
PANE_ID=$(zellij --session agents action new-pane -- \
  cargo build --release)
```

shell feature가 필요할 때만 명시적으로 shell을 실행하고 신뢰되지 않은 값을 string interpolation하지 않는다.

## 8. Blocking과 exit

### 8.1 `zellij run`/`new-pane` blocking 플래그

| 플래그 | 공식 의미 |
|---|---|
| `--blocking` | command가 끝나고 pane도 닫힐 때까지 block |
| `--block-until-exit` | command가 어떤 status로든 exit하거나 pane이 닫힐 때까지 block |
| `--block-until-exit-success` | status 0으로 exit하거나 pane이 닫힐 때까지 block |
| `--block-until-exit-failure` | non-zero로 exit하거나 pane이 닫힐 때까지 block |

`new-pane`에서 네 플래그는 상호 충돌한다. `new-tab` initial command에는 `--block-until-exit-success`, `--block-until-exit-failure`, `--block-until-exit`가 있다.

### 8.2 held pane과 retry

일반 command pane은 command exit 뒤 즉시 닫히지 않고 exit status를 frame에 표시한다. 사용자는:

- `Enter`: 같은 command 재실행
- `Ctrl-c`: pane 닫기

`--block-until-exit-success`에서 실패하면 pane이 남고 caller도 block된다. 사용자가 수정 후 `Enter`로 재실행해 성공하면 진행한다. 다른 process가 `send-keys ... "Enter"`로 retry를 유발할 수도 있다.

`--block-until-exit-failure`는 반대로 non-zero가 될 때까지 기다린다.

### 8.3 중요한 종료 해석

- 모든 조건부 flag는 공식 0.45.x source 설명상 **pane을 수동으로 닫아도 unblock**될 수 있다.
- 따라서 `--block-until-exit-success`가 반환했다고 해서 반드시 마지막 child exit가 0이었다고 단정하면 안 된다. pane이 닫혔을 가능성을 고려한다.
- 공식 문서는 blocking CLI process의 exit code가 child process exit code와 같다고 보장하지 않는다.
- 확실한 exit result는 `list-panes --json`의 `exited`, `exit_status`, `is_held`를 읽거나 plugin `CommandPaneExited` event를 사용한다.
- `CommandPaneExited`는 pane close가 아니다. held pane에서 재실행하면 여러 번 발생할 수 있다.

### 8.4 `--close-on-exit` 조합

`--close-on-exit`는 command exit 즉시 pane을 닫는다. blocking과 함께 쓰면 리뷰/retry window 및 뒤늦은 scrollback capture가 사라질 수 있다. 공식 source에서 모든 조합이 명시적으로 충돌 처리되지는 않으므로 실제 binary에서 검증한다.

## 9. 관측 loop 예

다음은 공식 pattern을 보강한 설명용 예이며 실행하지 않았다.

```bash
#!/usr/bin/env bash
set -euo pipefail

session=agent-lab
zellij attach --create-background "$session"

pane_id=$(
  zellij --session "$session" action new-pane \
    --name worker \
    -- cargo test
)

case "$pane_id" in
  terminal_[0-9]*) ;;
  *) printf 'unexpected pane id: %s\n' "$pane_id" >&2; exit 1 ;;
esac

numeric_id=${pane_id#terminal_}

while :; do
  pane_json=$(zellij --session "$session" action list-panes --json)
  row=$(jq -c --argjson id "$numeric_id" \
    '.[] | select(.is_plugin == false and .id == $id)' <<<"$pane_json")

  if [[ -z "$row" ]]; then
    printf 'pane closed before status capture\n' >&2
    exit 2
  fi

  if [[ $(jq -r '.exited' <<<"$row") == true ]]; then
    status=$(jq -r '.exit_status // "unknown"' <<<"$row")
    break
  fi

  sleep 1
done

zellij --session "$session" action dump-screen \
  --pane-id "$pane_id" --full

printf 'exit_status=%s\n' "$status"
zellij --session "$session" action close-pane --pane-id "$pane_id"
```

## 10. 경쟁 상태와 ordering

### 10.1 공식 ordering 설명

- 각 `zellij action`은 별도 process로 server에 연결해 message를 보내고 끊는다.
- server가 받은 순서대로 action을 처리한다.
- CLI는 action이 논리적으로 완료될 때까지 block한다.
- `&&` 또는 앞 명령의 반환값을 캡처한 순차 실행은 ordering을 보존한다.
- 여러 process가 동시에 action을 보내면 서로 간 ordering 보장은 없다.
- 서로 다른 pane/tab의 독립 action은 병렬화할 수 있다.
- 같은 pane에 동시 input을 보내면 예측 불가능하게 interleave될 수 있다.
- query는 mutation 중에도 가능하고, query 시점의 consistent snapshot을 반환한다고 Programmatic Control 문서가 설명한다.

### 10.2 추가 위험

| 위험 | 결과 | 권고 |
|---|---|---|
| create 직후 별도 process가 stale list 사용 | 새 pane 누락/잘못된 대상 | 생성 stdout ID를 직접 전달 |
| 같은 pane에 여러 writer | text/key interleave | pane별 single writer queue |
| viewport pattern polling | transient text 놓침 | `subscribe` 또는 command stdout 별도 capture |
| subscribe 늦은 연결 | 과거 viewport 밖 output 손실 | 시작부터 subscribe 또는 `--scrollback`; final `dump-screen --full` |
| process exit와 pane close 혼동 | watcher가 끝나지 않음/너무 일찍 끝남 | `exited`, `exit_status`, `pane_closed`를 다른 상태로 모델링 |
| `--close-on-exit` | final state/dump 기회 상실 | artifact가 필요하면 held pane 유지 |
| multi-client focus 변화 | 다른 사용자 화면 방해 | `--no-focus`, `--tab-id` 사용 |
| old session에 new command | 기능이 조용히 실패 가능 | action 후 상태 query로 postcondition 확인 |
| ANSI/terminal content를 shell 재사용 | escape/injection | JSON decode 후 data로 취급, `eval` 금지 |

## 11. Plugin API를 써야 하는 경우

CLI는 subprocess 중심이고 lifecycle event 전체를 노출하지 않는다. 더 깊은 integration은 WASM Plugin API를 사용한다.

관련 event:

- `PaneUpdate(PaneManifest)`
- `TabUpdate(Vec<TabInfo>)`
- `CommandPaneOpened(u32, context)`
- `CommandPaneExited(u32, Option<i32>, context)`
- `PaneClosed(PaneId)`
- `PaneRenderReport(HashMap<PaneId, PaneContents>)`
- `ActionComplete(Action, Option<PaneId>, context)`

plugin lifecycle event는 특정 순서를 따르지 않는다고 공식 문서가 경고한다. permission result보다 다른 event가 먼저 올 수도 있으므로 state machine과 correlation context를 사용한다.

## 12. 버전 호환성

- `zellij subscribe`, 확장 CLI automation, pane/tab 생성 ID 반환, 조건부 blocking은 0.44.0에서 발표됐다.
- 최신 조사 기준 release/docs.rs latest는 0.45.1이다.
- 0.44.0에서 새로운 client/server contract가 도입됐지만, 새 기능을 오래된 session에 보낼 때 조용히 실패할 수 있다.
- JSON field는 공식 sample과 현재 API를 기준으로 하며 versioned JSON Schema 안정성 보장은 문서에 없다.

**[추천 패턴]**

1. 최소 Zellij version을 명시한다. 이 문서 기능 전체는 `>=0.44.0`을 baseline으로 둔다.
2. 시작 시 client version과 running session 세대를 기록한다.
3. JSON parser는 모르는 field를 허용하고, 필요한 field 누락은 명확히 실패시킨다.
4. 새 action 뒤 `list-panes`/`list-tabs`로 postcondition을 확인한다.
5. 배포 대상 binary의 `--help` 출력을 golden file로 저장해 upgrade 때 diff한다.

## 13. 발견한 공식 문서 불일치 또는 모호성

1. **`--blocking` 표현 차이**
   - Programmatic Control 표: “pane이 닫힐 때까지”.
   - CLI Recipes/Run 문서: “command가 끝나고 pane이 닫힐 때까지”.
   - `zellij run` 0.45.1 source 설명은 후자와 일치한다.
2. **`hide-floating-panes`/`show-floating-panes` exit code**
   - Programmatic Control 표는 `1 = floating pane 없음`, `2 = 이미 hidden/visible`이라고 적는다.
   - CLI Actions 문서는 `1 = tab을 찾지 못함`, `2 = 이미 hidden/visible`, `0 = 변경`이라고 적는다.
   - 자동화에서 code 1 의미는 실제 binary로 확인해야 한다.
3. **JSON은 sample이지 formal schema가 아님**
   - Programmatic Control은 field 요약과 sample을 제공하지만 schema version, required/optional/compatibility 정책을 제공하지 않는다.
4. **`--pane-id` 반복 방식**
   - Subscribe 문서는 “repeatable”이라고 설명하고 예시는 옵션을 반복한다.
   - 0.45.1 Clap source는 한 `--pane-id` 뒤 1개 이상 value를 받는 `Vec`로 정의한다. portable usage는 문서 예시처럼 반복이다.
5. **blocking CLI exit code 전파 미명시**
   - child exit condition은 설명하지만 caller process의 exit code가 child와 같은지는 공식 문서가 명시하지 않는다.

## 14. 공식 자료

사용자가 지정한 공식 문서:

- https://zellij.dev/documentation/programmatic-control.html
- https://zellij.dev/documentation/zellij-subscribe.html
- https://zellij.dev/documentation/cli-recipes.html

교차 확인에 사용한 공식 문서/소스:

- https://zellij.dev/documentation/cli-actions.html
- https://zellij.dev/documentation/controlling-zellij-through-cli.html
- https://zellij.dev/documentation/zellij-run-and-edit.html
- https://zellij.dev/documentation/plugin-api.html
- https://zellij.dev/documentation/plugin-api-events.html
- https://zellij.dev/documentation/plugin-api-types.html
- https://zellij.dev/documentation/plugin-lifecycle.html
- https://docs.rs/zellij-utils/0.45.1/src/zellij_utils/cli.rs.html
- https://zellij.dev/news/remote-sessions-windows-cli/
- https://github.com/zellij-org/zellij/releases/latest
