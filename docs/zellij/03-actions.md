# Zellij 공식 CLI Actions 전체 정리

## 범위와 검증 상태

- 기준 문서: <https://zellij.dev/documentation/cli-actions.html>
- 보조 대조 문서: <https://zellij.dev/documentation/zellij-run-and-edit.html>
- 버전 교차 확인: Zellij Utils 0.45.0 공식 docs.rs CLI source, <https://docs.rs/zellij-utils/0.45.0/src/zellij_utils/cli.rs.html#1-1650>
- 조사 시 공식 `CLI Actions` 페이지에 열거된 action은 **78개**다.
- 별도 `Nested Sessions` 페이지가 CLI에서도 사용 가능하다고 명시하지만 `CLI Actions` 목록에는 없는 nested action **3개**도 보완 조사했다. 따라서 조사 범위는 **핵심 목록 78개 + 보완 3개 = action 이름 81개**다.
- 이 문서는 공식 웹 문구와 예시를 바탕으로 정리했으며, **로컬 Zellij 바이너리로 실행 검증하지 않았다.** 설치 버전의 `zellij action --help` 및 `zellij action <ACTION> --help` 결과가 최종 기준이다.
- 공식 페이지에는 적용 Zellij 버전이나 문서 생성 커밋이 표시되지 않는다. 따라서 최신 웹 문서와 사용자의 설치 버전 사이에 옵션 차이가 있을 수 있다.

## 공통 규칙

기본 호출 형식은 다음과 같다.

```bash
zellij action <ACTION> [ARGS] [OPTIONS]
```

### pane ID

터미널 pane과 plugin pane은 숫자 ID가 겹칠 수 있어 `terminal_1`, `plugin_1`처럼 타입 접두사를 붙인다. 숫자만 `3`이라고 쓰면 `terminal_3`과 같다. 터미널 pane ID는 pane 내부의 `$ZELLIJ_PANE_ID`로도 확인할 수 있다.

### tab 위치와 stable ID

문서는 위치 기반 `go-to-tab <INDEX>`와 stable ID 기반 `go-to-tab-by-id <TAB_ID>`를 별개로 제공한다. `list-tabs`, `current-tab-info`가 stable ID를 노출한다. 다만 `go-to-tab`의 인덱스가 0 기반인지 1 기반인지 해당 페이지는 명시하지 않는다.

### 표기

- `<...>`: 값이 필요한 인자 또는 옵션 값
- `[ARGS]`: 선택 인자
- `--`: 뒤의 토큰을 새 pane/tab에서 실행할 명령으로 분리
- 별도 언급이 없으면 대상은 현재 클라이언트의 활성 tab 또는 focused pane이다.

---

## 1. 상태 조회와 출력

### `are-floating-panes-visible`

- **용도:** 지정 tab 또는 활성 tab에서 floating pane이 보이는지 확인한다. 보이면 stdout에 `true`, 종료 코드 0. 안 보이면 `false`, 종료 코드 1.
- **형식:** `zellij action are-floating-panes-visible [-t|--tab-id <TAB_ID>]`
- **옵션:** `-t, --tab-id <TAB_ID>` 특정 tab ID.
- **예시:** `zellij action are-floating-panes-visible --tab-id 3`
- **주의:** 텍스트뿐 아니라 종료 코드도 의미가 있다.

### `current-tab-info`

- **용도:** 현재 활성 tab의 이름과 ID를 기본 출력하고, JSON으로 전체 `TabInfo`를 출력할 수 있다.
- **형식:** `zellij action current-tab-info [-j|--json]`
- **옵션:** `-j, --json` 전체 정보를 JSON으로 출력.
- **예시:** `zellij action current-tab-info --json`
- **주의:** JSON 예시 필드에는 `position`, `name`, `active`, `panes_to_hide`, fullscreen/sync/floating 상태, viewport/display 크기, pane 수, `tab_id`, bell 상태 등이 포함된다. 문서는 JSON 스키마 안정성을 보장한다고 쓰지 않는다.

### `dump-layout`

- **용도:** 현재 세션의 Layout을 stdout에 출력한다.
- **형식:** `zellij action dump-layout`
- **인자/옵션:** 없음.
- **예시:** `zellij action dump-layout > current-layout.kdl`
- **주의:** 공식 예시는 단순 stdout 출력이며, 리다이렉션은 셸 기능이다.

### `dump-screen`

- **용도:** pane viewport를 파일 또는 stdout으로 덤프하며 full scrollback과 ANSI 스타일을 선택할 수 있다.
- **형식:** `zellij action dump-screen [OPTIONS]`
- **옵션:**
  - `--path <PATH>` 파일 경로. 생략하면 stdout.
  - `-f, --full` 전체 scrollback 포함.
  - `-p, --pane-id <PANE_ID>` 특정 pane. 생략하면 focused pane.
  - `-a, --ansi` ANSI 스타일 보존.
- **예시:** `zellij action dump-screen --pane-id terminal_3 --full --ansi --path /tmp/screen.txt`
- **주의:** `--ansi` 출력은 escape sequence를 포함할 수 있다.

### `list-clients`

- **용도:** 현재 세션에 연결된 client, focused pane ID, 감지 가능한 실행 프로그램을 표로 출력한다.
- **형식:** `zellij action list-clients`
- **인자/옵션:** 없음.
- **예시:** `zellij action list-clients`
- **주의:** 실행 프로그램은 기본 shell이 아니고 Zellij가 감지할 수 있을 때만 표시된다.

### `list-panes`

- **용도:** 현재 세션의 pane 목록을 출력한다.
- **형식:** `zellij action list-panes [OPTIONS]`
- **옵션:**
  - `-t, --tab` tab 정보 포함.
  - `-c, --command` 실행 command 정보 포함.
  - `-s, --state` focused/floating/exited 등 상태 포함.
  - `-g, --geometry` 위치와 크기 포함.
  - `-a, --all` 가능한 모든 필드 포함.
  - `-j, --json` JSON 출력.
- **예시:** `zellij action list-panes --all`, `zellij action list-panes --json`
- **주의:** 기본 표는 `PANE_ID TYPE TITLE`; `--all` 표와 JSON은 더 많은 필드를 가진다. 웹 문서는 출력 스키마의 버전 안정성을 명시하지 않는다.

### `list-tabs`

- **용도:** 모든 tab과 관련 정보를 출력한다.
- **형식:** `zellij action list-tabs [OPTIONS]`
- **옵션:**
  - `-s, --state` active/fullscreen/sync/floating visibility 상태.
  - `-d, --dimensions` 크기 정보.
  - `-p, --panes` pane 개수.
  - `-l, --layout` swap layout 이름과 dirty 상태.
  - `-a, --all` 모든 필드.
  - `-j, --json` JSON 출력.
- **예시:** `zellij action list-tabs --all`, `zellij action list-tabs --json`
- **주의:** 기본 표의 `POSITION`과 stable `TAB_ID`를 혼동하지 말아야 한다.

### `query-tab-names`

- **용도:** 모든 tab 이름을 텍스트 목록으로 조회한다.
- **형식:** `zellij action query-tab-names`
- **인자/옵션:** 없음.
- **예시:** `zellij action query-tab-names`
- **주의:** 공식 문서는 정확한 구분자나 출력 형식 안정성을 설명하지 않는다.

---

## 2. Pane 생성, 편집, 종료

### `clear`

- **용도:** focused pane의 모든 buffer를 지운다.
- **형식:** `zellij action clear [-p|--pane-id <PANE_ID>]`
- **옵션:** `-p, --pane-id <PANE_ID>` 특정 pane.
- **예시:** `zellij action clear --pane-id terminal_3`
- **주의:** 단순 화면 clear가 아니라 문서 표현상 “all buffers”를 지운다.

### `close-pane`

- **용도:** focused pane 또는 지정 pane을 닫는다.
- **형식:** `zellij action close-pane [-p|--pane-id <PANE_ID>]`
- **옵션:** `-p, --pane-id <PANE_ID>` 특정 pane.
- **예시:** `zellij action close-pane --pane-id terminal_3`
- **주의:** 실행 중 프로세스가 있는 pane을 닫는 부작용에 유의한다.

### `edit`

- **용도:** 파일을 기본 editor로 새 Zellij pane에 열고, 생성된 pane ID를 반환한다.
- **형식:** `zellij action edit <PATH> [OPTIONS]`
- **인자:** `<PATH>` 열 파일 경로.
- **옵션:**
  - `-d, --direction <DIRECTION>` 열 방향 `[right|down]`; `--floating`과 충돌.
  - `-f, --floating` floating으로 열기.
  - `-i, --in-place` focused pane 자리에 열기.
  - `--close-replaced-pane` 대체한 pane을 suspend하지 않고 닫기; `--in-place` 필요.
  - `-l, --line-number <LINE_NUMBER>` 지정 줄로 열기.
  - `--cwd <CWD>` editor pane 작업 디렉터리.
  - `-x, --x <X>`, `-y, --y <Y>`, `--width <WIDTH>`, `--height <HEIGHT>` floating 위치/크기; `--floating` 필요.
  - `--pinned <PINNED>` floating pane 고정; `--floating` 필요.
  - `--near-current-pane` 현재 pane 가까이에 배치.
  - `--no-focus` 어떤 client의 focus도 바꾸지 않음.
  - `--tab-id <TAB_ID>` 특정 tab; `--in-place`, `--near-current-pane`과 충돌.
  - `-b, --borderless <BORDERLESS>` border 없이 시작.
- **예시:** `zellij action edit ./my-file.rs -f`, `zellij action edit ./my-file.rs --in-place`
- **주의:** `zellij edit`로도 호출할 수 있다. 두 공식 페이지의 옵션 표현 차이는 이 문서 하단의 불일치 절을 참고한다.

### `edit-scrollback`

- **용도:** pane scrollback을 기본 editor로 연다.
- **형식:** `zellij action edit-scrollback [OPTIONS]`
- **옵션:** `-p, --pane-id <PANE_ID>` 특정 pane, `-a, --ansi` ANSI 스타일 보존.
- **예시:** `zellij action edit-scrollback --pane-id terminal_3 --ansi`
- **주의:** ANSI 보존 시 editor에서 escape sequence가 보일 수 있다.

### `new-pane`

- **용도:** 새 terminal pane 또는 plugin pane을 만들며 생성 pane ID를 반환한다. 방향을 생략하면 가장 큰 가용 공간을 사용하려 시도한다.
- **형식:** `zellij action new-pane [OPTIONS] [-- <COMMAND> [ARGS...]]`
- **선택 인자:** 기본 shell 대신 실행할 command. 반드시 `--` 뒤에 둔다.
- **옵션:**
  - `-c, --close-on-exit` command 종료 즉시 pane 닫기.
  - `--cwd <CWD>` 새 pane 작업 디렉터리.
  - `-d, --direction <DIRECTION>` 열 방향; `--floating`과 충돌.
  - `-f, --floating` floating pane.
  - `-i, --in-place` focused pane을 임시 대체; `--floating`, `--direction`과 충돌.
  - `--close-replaced-pane` 대체 pane을 suspend하지 않고 닫기; `--in-place` 필요.
  - `-n, --name <NAME>` pane 이름.
  - `-p, --plugin <PLUGIN>` plugin URL; command 및 `--direction`과 충돌.
  - `--configuration <CONFIG>` plugin 설정; `--plugin` 필요.
  - `--skip-plugin-cache` plugin cache 생략; `--plugin` 필요.
  - `-s, --start-suspended` command를 suspended 상태로 시작.
  - `-x, --x <X>`, `-y, --y <Y>`, `--width <WIDTH>`, `--height <HEIGHT>` floating 위치/크기; `--floating` 필요.
  - `--pinned <PINNED>` floating pane 고정; `--floating` 필요.
  - `--stacked` stacked mode; `--floating`, `--direction`과 충돌.
  - `--tab-id <TAB_ID>` 특정 tab; `--in-place`, `--near-current-pane`과 충돌.
  - `--pane-id <PANE_ID>` `--in-place`에서 대체할 pane; 기본은 focused pane.
  - `-b, --blocking` pane 종료까지 block.
  - `--block-until-exit-success` command가 0으로 끝날 때까지 block.
  - `--block-until-exit-failure` command가 non-zero로 끝날 때까지 block.
  - `--block-until-exit` 종료 상태와 무관하게 command 종료까지 block.
  - `--near-current-pane` focus를 따르지 않고 현재 pane 가까이에 배치.
  - `--no-focus` 어떤 client의 focus도 바꾸지 않음.
  - `--borderless <BORDERLESS>` border 없이 시작.
- **예시:**
  ```bash
  zellij action new-pane -f
  zellij action new-pane --name "follow this log!" -- tail -f /tmp/my-log-file
  zellij action new-pane --in-place --pane-id terminal_4 -- htop
  zellij action new-pane --plugin zellij:strider --floating
  zellij action new-pane --no-focus -- tail -f /tmp/my-log-file
  ```
- **주의:** `--no-focus` 배치는 명령을 낸 pane 기준이다. 문서는 이를 `zellij run`으로 줄일 수 있다고 하지만 옵션 집합과 단축 옵션은 완전히 같지 않다. 특히 `new-pane -b`는 `--blocking`, `run -b`는 `--borderless`다.

---

## 3. Pane focus, 이동, resize, stack

### `focus-last-pane`

- **용도:** 현재 pane 직전에 focused였던 pane으로 돌아간다. tiled/floating을 넘나들며, 현재 pane이 fullscreen이면 제자리에서 pane을 교체한다.
- **형식/예시:** `zellij action focus-last-pane`
- **인자/옵션:** 없음.
- **주의:** 단순 순서상 이전 pane이 아니라 focus 이력 기준이다.

### `focus-next-pane`

- **용도:** 다음 pane으로 focus 이동.
- **형식/예시:** `zellij action focus-next-pane`
- **인자/옵션:** 없음.

### `focus-pane-id`

- **용도:** ID로 특정 pane focus.
- **형식:** `zellij action focus-pane-id <PANE_ID>`
- **인자:** `terminal_1`, `plugin_2`, 또는 `3` 같은 pane ID.
- **예시:** `zellij action focus-pane-id terminal_1`
- **주의:** 숫자 `3`은 `terminal_3`으로 해석된다.

### `focus-previous-pane`

- **용도:** 이전 pane으로 focus 이동.
- **형식/예시:** `zellij action focus-previous-pane`
- **인자/옵션:** 없음.

### `move-focus`

- **용도:** 지정 방향의 pane으로 focus 이동.
- **형식:** `zellij action move-focus <DIRECTION>`
- **인자:** `[right|left|up|down]`.
- **예시:** `zellij action move-focus left`

### `move-focus-or-tab`

- **용도:** 지정 방향의 pane으로 focus를 옮기고, 화면 가장자리라면 tab으로 이동.
- **형식:** `zellij action move-focus-or-tab <DIRECTION>`
- **인자:** `[right|left|up|down]`.
- **예시:** `zellij action move-focus-or-tab left`
- **주의:** 화면 가장자리에서는 pane 이동 대신 tab 전환이 일어날 수 있다.

### `move-pane`

- **용도:** focused pane 또는 지정 pane의 위치를 옮긴다.
- **형식:** `zellij action move-pane [DIRECTION] [-p|--pane-id <PANE_ID>]`
- **선택 인자:** `[right|left|up|down]`.
- **옵션:** `-p, --pane-id <PANE_ID>` 특정 pane.
- **예시:** `zellij action move-pane --pane-id terminal_3 right`
- **주의:** 방향 인자는 선택 사항이지만, 생략 시 동작을 문서가 자세히 설명하지 않는다.

### `move-pane-backwards`

- **용도:** layout 순서에서 pane 위치를 역방향으로 회전.
- **형식:** `zellij action move-pane-backwards [-p|--pane-id <PANE_ID>]`
- **옵션:** `-p, --pane-id <PANE_ID>` 특정 pane.
- **예시:** `zellij action move-pane-backwards`

### `resize`

- **용도:** focused pane 또는 지정 pane resize.
- **형식:** `zellij action resize <RESIZE> [-p|--pane-id <PANE_ID>]`
- **인자:** `[right|left|up|down|+|-]`.
- **옵션:** `-p, --pane-id <PANE_ID>` 특정 pane.
- **예시:** `zellij action resize --pane-id terminal_3 +`
- **주의:** 문서는 resize step 크기나 `+`/`-`의 정확한 축별 정책을 설명하지 않는다. 또한 0.45.0 source는 `resize: Resize` 필수 값과 `direction: Option<Direction>`을 별도 인자로 정의하고 주석에 `increase|decrease`를 쓴다. 최신 웹 문서의 단일 인자 `[right|left|up|down|+|-]` 표기와 명백히 다르다.

### `stack-panes`

- **용도:** 주어진 pane ID 목록을 하나의 stack으로 만든다.
- **형식:** `zellij action stack-panes -- <PANE_ID>...`
- **인자:** 하나 이상의 pane ID 목록.
- **예시:** `zellij action stack-panes -- terminal_1 plugin_1 terminal_2`
- **주의:** 예시는 terminal/plugin ID가 같은 숫자여도 타입 접두사로 구별됨을 보여 준다.

---

## 4. Scroll과 terminal 입력

### Scroll 계열

| Action | 용도 | 정확한 형식 | 예시 | 주의 |
|---|---|---|---|---|
| `half-page-scroll-down` | 반 페이지 아래로 | `zellij action half-page-scroll-down [-p|--pane-id <PANE_ID>]` | `zellij action half-page-scroll-down` | `-p, --pane-id`로 대상 지정 |
| `half-page-scroll-up` | 반 페이지 위로 | `zellij action half-page-scroll-up [-p|--pane-id <PANE_ID>]` | `zellij action half-page-scroll-up` | 동일 |
| `page-scroll-down` | 한 페이지 아래로 | `zellij action page-scroll-down [-p|--pane-id <PANE_ID>]` | `zellij action page-scroll-down` | 동일 |
| `page-scroll-up` | 한 페이지 위로 | `zellij action page-scroll-up [-p|--pane-id <PANE_ID>]` | `zellij action page-scroll-up` | 동일 |
| `scroll-down` | 1줄 아래로 | `zellij action scroll-down [-p|--pane-id <PANE_ID>]` | `zellij action scroll-down` | 동일 |
| `scroll-up` | 1줄 위로 | `zellij action scroll-up [-p|--pane-id <PANE_ID>]` | `zellij action scroll-up` | 동일 |
| `scroll-to-bottom` | 맨 아래로 | `zellij action scroll-to-bottom [-p|--pane-id <PANE_ID>]` | `zellij action scroll-to-bottom` | 동일 |
| `scroll-to-top` | 맨 위로 | `zellij action scroll-to-top [-p|--pane-id <PANE_ID>]` | `zellij action scroll-to-top` | 동일 |

### `paste`

- **용도:** bracketed paste mode로 terminal에 텍스트를 붙여 넣는다.
- **형식:** `zellij action paste <TEXT> [-p|--pane-id <PANE_ID>]`
- **인자:** 붙여 넣을 텍스트.
- **옵션:** `-p, --pane-id <PANE_ID>` 특정 pane.
- **예시:** `zellij action paste "Hello, World!"`
- **주의:** `write-chars`가 아니라 bracketed paste semantics를 사용한다.

### `send-keys`

- **용도:** 하나 이상의 key를 terminal에 보낸다.
- **형식:** `zellij action send-keys [KEY]... [-p|--pane-id <PANE_ID>]`
- **인자:** 공백으로 구분된 key 이름. 예: `"Ctrl a"`, `"F1"`, `"Alt Shift b"`.
- **옵션:** `-p, --pane-id <PANE_ID>` 특정 pane.
- **예시:** `zellij action send-keys --pane-id terminal_3 "Alt b" "Enter"`
- **주의:** 조합 key는 shell에서 하나의 인자로 유지되도록 따옴표를 쓰는 편이 안전하다.

### `write`

- **용도:** pane에 byte 배열을 쓴다.
- **형식:** `zellij action write <BYTE>... [-p|--pane-id <PANE_ID>]`
- **인자:** 쓸 byte 값 배열.
- **옵션:** `-p, --pane-id <PANE_ID>` 특정 pane.
- **예시:** `zellij action write 102 111 111`
- **주의:** 예시는 ASCII `foo`에 해당한다. 허용 byte 범위는 페이지에 명시되지 않는다.

### `write-chars`

- **용도:** pane에 문자열을 쓴다.
- **형식:** `zellij action write-chars <STRING> [-p|--pane-id <PANE_ID>]`
- **인자:** 쓸 문자열.
- **옵션:** `-p, --pane-id <PANE_ID>` 특정 pane.
- **예시:** `zellij action write-chars "Hi there!"`

---

## 5. Floating, fullscreen, border, frame, color, theme

### `change-floating-pane-coordinates`

- **용도:** 지정 floating pane의 좌표, 크기, borderless, pinned 상태를 변경.
- **형식:** `zellij action change-floating-pane-coordinates [OPTIONS]`
- **옵션:**
  - `-p, --pane-id <PANE_ID>` floating pane ID.
  - `-b, --borderless <BORDERLESS>` border 상태.
  - `--height <HEIGHT>`, `--width <WIDTH>` 정수 또는 `%`.
  - `-x, --x <X>`, `-y, --y <Y>` 정수 또는 `%`.
  - `--pinned <PINNED>` 항상 위에 둘지 여부.
- **예시:** `zellij action change-floating-pane-coordinates --pane-id terminal_15 --height 10 --width 10 -x 10 -y 10`
- **주의:** 제목은 coordinates지만 크기/border/pinned도 바꾼다. 문서의 **ARGS** 문장은 pane ID를 인자로 표현하지만 실제 옵션 목록과 예시는 `--pane-id`를 사용한다. 이는 문서 내부 표현 불일치다.

### `hide-floating-panes`

- **용도:** 지정 tab 또는 활성 tab의 모든 floating pane을 숨김.
- **형식:** `zellij action hide-floating-panes [-t|--tab-id <TAB_ID>]`
- **옵션:** `-t, --tab-id <TAB_ID>` 특정 tab.
- **예시:** `zellij action hide-floating-panes --tab-id 3`
- **주의:** 변경됨 0, 이미 숨김 2, tab 없음 1의 종료 코드를 반환한다.

### `show-floating-panes`

- **용도:** 지정 tab 또는 활성 tab의 모든 floating pane을 표시.
- **형식:** `zellij action show-floating-panes [-t|--tab-id <TAB_ID>]`
- **옵션:** `-t, --tab-id <TAB_ID>` 특정 tab.
- **예시:** `zellij action show-floating-panes --tab-id 3`
- **주의:** 변경됨 0, 이미 표시됨 2, tab 없음 1.

### `toggle-floating-panes`

- **용도:** 현재/지정 tab의 모든 floating pane visibility를 toggle하고, floating pane이 하나도 없으면 하나를 연다.
- **형식:** `zellij action toggle-floating-panes [-t|--tab-id <TAB_ID>]`
- **옵션:** `-t, --tab-id <TAB_ID>` 특정 tab.
- **예시:** `zellij action toggle-floating-panes`
- **주의:** 단순 show/hide뿐 아니라 floating pane 생성이 일어날 수 있다.

### `toggle-fullscreen`

- **용도:** pane fullscreen과 일반 layout을 toggle하되 UI bar는 유지.
- **형식:** `zellij action toggle-fullscreen [-p|--pane-id <PANE_ID>]`
- **옵션:** `-p, --pane-id <PANE_ID>` 특정 pane.
- **예시:** `zellij action toggle-fullscreen`

### `toggle-no-ui-fullscreen`

- **용도:** tab/status bar 영역까지 포함한 전체 display fullscreen과 일반 layout을 toggle.
- **형식:** `zellij action toggle-no-ui-fullscreen [-p|--pane-id <PANE_ID>] [-t|--tab-id <TAB_ID>]`
- **옵션:** `-p, --pane-id <PANE_ID>`, `-t, --tab-id <TAB_ID>`.
- **예시:** `zellij action toggle-no-ui-fullscreen --pane-id terminal_3`
- **주의:** `toggle-fullscreen`과 달리 UI bar도 가린다. pane ID와 tab ID를 함께 쓸 때의 제약은 문서에 명시되지 않는다. 또한 0.45.0 CLI source의 이 action에는 `pane_id`만 있고 `tab_id`가 없으므로, 최신 웹 문서의 `--tab-id`는 0.45.0과 맞지 않는다.

### `toggle-pane-embed-or-floating`

- **용도:** floating pane은 tiled로 embed하고, embedded pane은 floating으로 전환.
- **형식:** `zellij action toggle-pane-embed-or-floating [-p|--pane-id <PANE_ID>]`
- **옵션:** `-p, --pane-id <PANE_ID>` 특정 pane.
- **예시:** `zellij action toggle-pane-embed-or-floating`

### `toggle-pane-pinned`

- **용도:** floating pane의 항상 위(pinned) 상태 toggle.
- **형식:** `zellij action toggle-pane-pinned [-p|--pane-id <PANE_ID>]`
- **옵션:** `-p, --pane-id <PANE_ID>` 특정 pane.
- **예시:** `zellij action toggle-pane-pinned`
- **주의:** 문서상 현재 pane이 floating일 때 동작한다.

### `set-pane-borderless`

- **용도:** 특정 pane의 borderless 상태를 명시적으로 설정.
- **형식:** `zellij action set-pane-borderless --pane-id <PANE_ID> --borderless <BORDERLESS>`
- **필수 옵션:** `-p, --pane-id <PANE_ID>`, `-b, --borderless <BORDERLESS>`.
- **예시:** `zellij action set-pane-borderless --pane-id terminal_3 --borderless true`
- **버전 주의:** 최신 웹 문서는 `<BORDERLESS>` 값을 필수로 쓰지만, 0.45.0 source는 `borderless: bool` flag와 “flag present/absent”라는 주석을 사용한다. 0.45.0과 최신 웹 문서 사이에서 syntax가 바뀌었거나 문서가 어긋난 정황이므로 해당 버전의 `--help`를 확인해야 한다.

### `toggle-pane-borderless`

- **용도:** 특정 pane의 borderless 상태 toggle.
- **형식:** `zellij action toggle-pane-borderless --pane-id <PANE_ID>`
- **필수 옵션:** `-p, --pane-id <PANE_ID>`.
- **예시:** `zellij action toggle-pane-borderless --pane-id terminal_3`

### `set-pane-frame-style`

- **용도:** 세션 전체 pane frame 스타일을 runtime에 설정. 설정 파일의 `pane_frame_style`에 대응.
- **형식:** `zellij action set-pane-frame-style <STYLE>`
- **인자:** `full`(각 pane 전체 border), `titles`(각 pane 위 제목 한 줄), `none`(frame 없음).
- **예시:** `zellij action set-pane-frame-style titles`
- **주의:** 세션 전체에 적용된다.

### `toggle-pane-frames`

- **용도:** UI의 pane frame 표시를 toggle.
- **형식/예시:** `zellij action toggle-pane-frames`
- **인자/옵션:** 없음.
- **주의:** Zellij UI 일부가 frame에 의존하므로 제거하면 UI가 혼란스러울 수 있다고 공식 문서가 경고한다.

### `set-pane-color`

- **용도:** pane의 기본 foreground/background 색을 설정하거나 reset.
- **형식:** `zellij action set-pane-color [OPTIONS]`
- **옵션:**
  - `-p, --pane-id <PANE_ID>` 대상 pane; 기본 `$ZELLIJ_PANE_ID`.
  - `--fg <FG>` foreground. 예: `#00e000`, `rgb:00/e0/00`.
  - `--bg <BG>` background. 예: `#001a3a`, `rgb:00/1a/3a`.
  - `--reset` terminal 기본색으로 reset; `--fg`, `--bg`와 충돌.
- **예시:** `zellij action set-pane-color --fg "#00e000" --bg "#001a3a"`
- **주의:** `#`은 shell comment로 해석될 수 있어 따옴표가 필요하다.

### Theme 계열

| Action | 용도 | 형식/예시 | 주의 |
|---|---|---|---|
| `set-dark-theme` | 구성된 `theme_dark`로 전환 | `zellij action set-dark-theme` | `theme_dark` 설정 사용 |
| `set-light-theme` | 구성된 `theme_light`로 전환 | `zellij action set-light-theme` | `theme_light` 설정 사용 |
| `toggle-theme` | dark/light theme toggle | `zellij action toggle-theme` | 두 설정 모두에 의존 |

---

## 6. Tab 탐색, 생성, 이름, 이동

### Tab 이동 계열

| Action | 용도 | 인자/옵션 | 예시 | 주의 |
|---|---|---|---|---|
| `go-to-next-tab` | 다음 tab | 없음 | `zellij action go-to-next-tab` | 순환 여부는 페이지에 명시 안 됨 |
| `go-to-previous-tab` | 이전 tab | 없음 | `zellij action go-to-previous-tab` | 동일 |
| `go-to-tab` | 위치/index로 tab 이동 | 필수 `<INDEX>` | `zellij action go-to-tab 1` | 0/1 기반을 페이지가 명시하지 않음 |
| `go-to-tab-by-id` | stable ID로 tab 이동 | 필수 `<TAB_ID>` 정수 | `zellij action go-to-tab-by-id 5` | position과 ID를 구별 |
| `go-to-tab-name` | 이름으로 tab 이동 | 필수 `<NAME>`; `-c, --create` 사용 시 없으면 생성 | `zellij action go-to-tab-name "Tab #1"` | `--create`로 실제 생성 시 새 tab ID 출력 |

> `go-to-tab-name`의 `--create` 설명은 “존재하지 않으면 생성”이다. 위 표의 “없으면 생성”은 `--create` 옵션을 사용했을 때만 해당한다.

### `close-tab`

- **용도:** 현재 tab 또는 stable ID로 지정한 tab을 닫는다.
- **형식:** `zellij action close-tab [-t|--tab-id <TAB_ID>]`
- **옵션:** `-t, --tab-id <TAB_ID>` 특정 stable ID.
- **예시:** `zellij action close-tab --tab-id 5`

### `close-tab-by-id`

- **용도:** stable ID로 tab을 닫는다.
- **형식:** `zellij action close-tab-by-id <TAB_ID>`
- **인자:** 정수 tab ID.
- **예시:** `zellij action close-tab-by-id 5`
- **주의:** `close-tab --tab-id 5`와 기능이 겹치지만 공식 문서는 둘 다 별도 action으로 열거한다.

### `new-tab`

- **용도:** 선택 layout/name/초기 command 또는 plugin으로 새 tab을 만들고 새 tab ID를 반환.
- **형식:** `zellij action new-tab [OPTIONS] [-- <COMMAND> [ARGS...]]`
- **선택 인자:** 초기 pane의 command. `--` 뒤에 두며 `--initial-plugin`과 충돌.
- **옵션:**
  - `-c, --cwd <CWD>` 새 tab 작업 디렉터리.
  - `-l, --layout <LAYOUT>` layout.
  - `--layout-dir <LAYOUT_DIR>` layout 기본 폴더; `--layout` 필요.
  - `--layout-string <LAYOUT_STRING>` raw KDL; `--layout`과 충돌.
  - `-n, --name <NAME>` tab 이름.
  - `--initial-plugin <PLUGIN>` 초기 pane plugin; command와 충돌.
  - `--close-on-exit` 초기 command 종료 시 pane 닫기; command 필요.
  - `--start-suspended` 초기 command suspended 시작; command 필요.
  - `--block-until-exit-success`, `--block-until-exit-failure`, `--block-until-exit` 각 조건까지 block. 0.45.0 source에서는 초기 command가 필요하고 세 옵션은 서로 충돌.
  - `--no-focus` client focus를 바꾸지 않고 background에 생성.
- **예시:** `zellij action new-tab --layout /path/to/layout.kdl --name "my tab"`, `zellij action new-tab --no-focus --name "background tab"`
- **주의:** `--layout`과 `--cwd`를 함께 쓰면 layout의 상대 경로 기준이 이 cwd로 바뀌고 global cwd를 대체한다. `--no-focus` 사용 시 layout의 `focus` 속성도 억제된다.

### `move-tab`

- **용도:** focused tab 또는 지정 tab을 좌우로 이동.
- **형식:** `zellij action move-tab <DIRECTION> [-t|--tab-id <TAB_ID>]`
- **인자:** `[right|left]`.
- **옵션:** `-t, --tab-id <TAB_ID>` 특정 tab.
- **예시:** `zellij action move-tab left --tab-id 3`

### Tab 이름 계열

| Action | 용도 | 정확한 인자/옵션 | 예시 |
|---|---|---|---|
| `rename-tab` | focused/지정 tab 이름 변경 | 필수 `<NAME>`; `-t, --tab-id <TAB_ID>` | `zellij action rename-tab --tab-id 3 "my tab"` |
| `rename-tab-by-id` | stable ID로 이름 변경 | 필수 `<TAB_ID> <NAME>` | `zellij action rename-tab-by-id 5 "my new tab name"` |
| `undo-rename-tab` | 이전에 지정한 tab 이름 제거 | `-t, --tab-id <TAB_ID>` 선택 | `zellij action undo-rename-tab` |

### Pane 이름 계열

| Action | 용도 | 정확한 인자/옵션 | 예시 |
|---|---|---|---|
| `rename-pane` | focused/지정 pane 이름 변경 | 필수 `<NAME>`; `-p, --pane-id <PANE_ID>` | `zellij action rename-pane --pane-id terminal_3 "my pane"` |
| `undo-rename-pane` | 이전에 지정한 pane 이름 제거 | `-p, --pane-id <PANE_ID>` 선택 | `zellij action undo-rename-pane` |

---

## 7. Layout, swap layout, mode, session

### `next-swap-layout`

- **용도:** 현재/지정 tab의 다음 swap layout으로 전환.
- **형식:** `zellij action next-swap-layout [-t|--tab-id <TAB_ID>]`
- **옵션:** `-t, --tab-id <TAB_ID>` 특정 tab.
- **예시:** `zellij action next-swap-layout`

### `previous-swap-layout`

- **용도:** 현재/지정 tab의 이전 swap layout으로 전환.
- **형식:** `zellij action previous-swap-layout [-t|--tab-id <TAB_ID>]`
- **옵션:** `-t, --tab-id <TAB_ID>` 특정 tab.
- **예시:** `zellij action previous-swap-layout`

### `override-layout`

- **용도:** 활성 tab의 layout을 지정 layout으로 덮어쓴다.
- **형식:** `zellij action override-layout [LAYOUT_PATH] [OPTIONS]`
- **선택 인자:** layout 파일 경로. 이것 또는 `--layout-string` 중 하나는 필요.
- **옵션:**
  - `--layout-dir <LAYOUT_DIR>` layout 기본 폴더.
  - `--layout-string <LAYOUT_STRING>` raw KDL; path 인자와 충돌.
  - `--retain-existing-terminal-panes` 새 layout에 맞지 않는 기존 terminal pane 유지.
  - `--retain-existing-plugin-panes` 새 layout에 맞지 않는 기존 plugin pane 유지.
  - `--apply-only-to-active-tab` 활성 tab에만 적용.
- **예시:** `zellij action override-layout --layout-string 'layout { pane; pane; }'`
- **주의:** `--layout-string`은 세션으로 보내기 전에 parse/validate되어 문법 오류가 즉시 보고된다. 문서의 첫 문장은 활성 tab을 말하지만 `--apply-only-to-active-tab` 옵션도 별도로 있어 기본 적용 범위 표현이 직관적이지 않다.

### `switch-mode`

- **용도:** 연결된 모든 client의 입력 mode를 전환.
- **형식:** `zellij action switch-mode <MODE>`
- **인자:** `[locked|pane|tab|resize|move|search|session|tmux]`.
- **예시:** `zellij action switch-mode locked`
- **주의:** 전체 연결 client에 적용된다. 페이지는 위 8개 mode만 열거한다.

### `detach`

- **용도:** 현재 세션을 background에 남기고 현재 client를 detach.
- **형식/예시:** `zellij action detach`
- **인자/옵션:** 없음.

### `rename-session`

- **용도:** 현재 세션 이름 변경.
- **형식:** `zellij action rename-session <NAME>`
- **예시:** `zellij action rename-session "my-new-session-name"`

### `save-session`

- **용도:** 현재 세션 상태를 즉시 disk에 저장. 자동 serialization 주기 밖에서 수동 저장할 때 사용.
- **형식/예시:** `zellij action save-session`
- **인자/옵션:** 없음.

### `switch-session`

- **용도:** 다른 세션으로 전환하며 tab/pane focus, layout, cwd를 선택할 수 있다.
- **형식:** `zellij action switch-session <SESSION_NAME> [OPTIONS]`
- **인자:** 전환할 세션 이름.
- **옵션:**
  - `--tab-position <TAB_POSITION>` 전환 후 focus할 tab 위치.
  - `--pane-id <PANE_ID>` 전환 후 focus할 pane ID.
  - `-l, --layout <LAYOUT>` 적용할 layout.
  - `--layout-dir <LAYOUT_DIR>` layout 기본 폴더; `--layout` 필요.
  - `--layout-string <LAYOUT_STRING>` raw KDL; `--layout`과 충돌.
  - `-c, --cwd <CWD>` 전환 시 작업 디렉터리.
- **예시:** `zellij action switch-session my-session --tab-position 2 --layout /path/to/layout.kdl`
- **주의:** 대상 세션이 없을 때 생성 여부 등은 이 페이지에서 설명하지 않는다.

### `toggle-active-sync-tab`

- **용도:** 현재/지정 tab에서 입력을 모든 pane에 보낼지 focused pane에만 보낼지 toggle.
- **형식:** `zellij action toggle-active-sync-tab [-t|--tab-id <TAB_ID>]`
- **옵션:** `-t, --tab-id <TAB_ID>` 특정 tab.
- **예시:** `zellij action toggle-active-sync-tab`
- **주의:** 활성화하면 입력이 여러 pane에 동시에 전달될 수 있다.

---

## 8. Plugin과 pipe

### `launch-or-focus-plugin`

- **용도:** 세션 어디에도 plugin이 없으면 launch하고, 있으면 focus. plugin pane ID 반환.
- **형식:** `zellij action launch-or-focus-plugin <PLUGIN_URL> [OPTIONS]`
- **인자:** plugin URL. 예: `file:/path/to/my/plugin.wasm`, `zellij:strider`.
- **옵션:**
  - `-f, --floating` launch 시 floating.
  - `-i, --in-place` launch 시 focused pane 자리.
  - `--close-replaced-pane` 대체 pane 닫기; `--in-place` 필요.
  - `-m, --move-to-focused-tab` 이미 실행 중이면 focused tab으로 이동.
  - `-c, --configuration <CONFIG>` `key=value` plugin 설정.
  - `-s, --skip-plugin-cache` cache를 건너뛰고 강제 reload.
  - `--tab-id <TAB_ID>` 특정 tab; `--in-place`와 충돌.
- **예시:** `zellij action launch-or-focus-plugin zellij:strider --in-place --move-to-focused-tab`
- **주의:** 기존 instance가 있으면 새로 만들지 않고 focus하므로, 항상 새 instance가 필요하면 `launch-plugin`을 쓴다.

### `launch-plugin`

- **용도:** 항상 새 plugin instance를 launch하고 pane ID 반환.
- **형식:** `zellij action launch-plugin <PLUGIN_URL> [OPTIONS]`
- **인자:** plugin URL.
- **옵션:**
  - `-f, --floating` floating.
  - `-i, --in-place` focused pane 자리.
  - `--close-replaced-pane` 대체 pane 닫기; `--in-place` 필요.
  - `-c, --configuration <CONFIG>` `key=value` 설정.
  - `-s, --skip-plugin-cache` cache 생략.
  - `--tab-id <TAB_ID>` 특정 tab; `--in-place`와 충돌.
  - `--no-focus` client focus를 바꾸지 않음.
- **예시:** `zellij action launch-plugin file:/path/to/plugin.wasm --floating`
- **주의:** 호출마다 새 instance를 만든다.

### `start-or-reload-plugin`

- **용도:** plugin이 없으면 launch하고, 있으면 cache를 건너뛰어 reload. 주로 plugin 개발용.
- **형식:** `zellij action start-or-reload-plugin <PLUGIN_URL> [-c|--configuration <CONFIG>]`
- **인자:** plugin URL.
- **옵션:** `-c, --configuration <CONFIG>` `key=value` 설정.
- **예시:** `zellij action start-or-reload-plugin file:/path/to/plugin.wasm -c "key=value"`

### `pipe`

- **용도:** pipe를 통해 하나 이상의 plugin에 데이터를 보낸다. plugin이 실행 중이 아니면 launch할 수 있다.
- **형식:** `zellij action pipe [PAYLOAD] [OPTIONS]`
- **선택 인자:** payload. 비우면 stdin을 listen.
- **옵션:**
  - `-n, --name <NAME>` pipe 이름.
  - `-a, --args <ARGS>` `key=value` pipe 인자.
  - `-p, --plugin <PLUGIN>` 대상 plugin URL.
  - `-c, --plugin-configuration <CONFIG>` `key=value` plugin 설정.
  - `-l, --force-launch-plugin` 이미 실행 중이어도 새 plugin launch.
  - `-s, --skip-plugin-cache` cache를 건너뛰고 강제 reload.
  - `-f, --floating-plugin <BOOL>` launch된 plugin을 floating으로 둘지.
  - `-i, --in-place-plugin <BOOL>` plugin을 in-place로 launch; `--floating-plugin`과 충돌.
  - `-w, --plugin-cwd <CWD>` launch plugin 작업 디렉터리.
  - `-t, --plugin-title <TITLE>` plugin pane 제목.
- **예시:**
  ```bash
  zellij action pipe --name "my-pipe" --plugin "file:/path/to/plugin.wasm" "some payload"
  echo "data from stdin" | zellij action pipe --name "my-pipe" --plugin "my-plugin-alias"
  ```
- **주의:** `<ARGS>`/`<CONFIG>`의 반복 가능 여부와 여러 `key=value`의 정확한 구분법은 페이지가 설명하지 않는다.

---

## 9. Nested session 보완 action 3개

공식 `Nested Sessions` 문서는 아래 세 action을 keybinding, CLI, plugin API에서 사용할 수 있다고 명시한다. 그러나 세 action은 조사 시점의 `CLI Actions` 페이지 78개 목록에 없고, `zellij-utils` 0.45.0 및 0.45.1의 `CliAction` enum에서도 찾을 수 없었다. 반면 0.45.0 `input/actions.rs`의 일반 `Action` enum에는 `FocusGuestSession`, `FocusHostSession`, `ToggleHostFullscreen` variant가 존재한다.

따라서 아래 kebab-case CLI 이름은 Zellij의 통상적인 action 이름 변환과 별도 문서의 “available through the CLI” 문장을 따른 것이지만, **현재 웹 CLI reference와 0.45.x CLI source로 실제 parser 노출을 확인하지 못했다.** 반드시 설치 버전에서 `zellij action --help`로 존재 여부를 확인해야 한다.

### `focus-guest-session` (보완, CLI 노출 불확실)

- **keybinding action:** `FocusGuestSession`
- **용도:** focused pane 안의 nested guest session으로 descend하여 keyboard/mouse 입력을 guest로 보낸다.
- **문서에서 유추되는 CLI 형식:** `zellij action focus-guest-session`
- **인자/옵션:** Nested Sessions 페이지는 별도 인자나 옵션을 설명하지 않는다.
- **예시:** `zellij action focus-guest-session`
- **기본 keybinding:** `session` mode의 `[`.
- **주의:** 위 CLI spelling과 parser 지원은 0.45.x `CliAction` source에서 확인되지 않았다. 실제 `--help` 확인이 필요하다.

### `focus-host-session` (보완, CLI 노출 불확실)

- **keybinding action:** `FocusHostSession`
- **용도:** guest 안에서 host session으로 ascend하여 host에 focus를 돌린다.
- **문서에서 유추되는 CLI 형식:** `zellij action focus-host-session`
- **인자/옵션:** Nested Sessions 페이지는 별도 인자나 옵션을 설명하지 않는다.
- **예시:** `zellij action focus-host-session`
- **기본 keybinding:** `session` mode의 `]`.
- **주의:** guest session 안에서 발행되어 이를 포함하는 host에 작용한다. nested가 아닌 session에서는 효과가 없다. CLI parser 지원은 실제 `--help` 확인이 필요하다.

### `toggle-host-fullscreen` (보완, CLI 노출 불확실)

- **keybinding action:** `ToggleHostFullscreen`
- **용도:** guest session을 담고 있는 host 쪽 pane의 fullscreen 상태를 toggle한다.
- **문서에서 유추되는 CLI 형식:** `zellij action toggle-host-fullscreen`
- **인자/옵션:** Nested Sessions 페이지는 별도 인자나 옵션을 설명하지 않는다.
- **예시:** `zellij action toggle-host-fullscreen`
- **기본 keybinding:** `session` mode의 `f`.
- **주의:** guest 안에서 발행하며 nested가 아니면 효과가 없다. CLI parser 지원은 실제 `--help` 확인이 필요하다.

### Nested action 문서 모순 요약

- `nested-sessions.html`: 세 action이 “also available through the CLI”라고 명시.
- `cli-actions.html`: 78개 목차/본문에 세 action이 없음.
- `zellij-utils` 0.45.0/0.45.1 `cli.rs`: `CliAction` variant가 없음.
- `zellij-utils` 0.45.0 `input/actions.rs`: 일반 keybinding/runtime `Action` enum에는 세 variant가 있음.

즉 기능 action 자체는 0.45.0에 존재하지만, 0.45.x에서 CLI subcommand로 실제 노출되는지는 조사 자료가 서로 모순된다.

---

## 공식 CLI Actions 페이지 목록 체크리스트 (78개)

1. `are-floating-panes-visible`
2. `change-floating-pane-coordinates`
3. `clear`
4. `close-pane`
5. `close-tab`
6. `close-tab-by-id`
7. `current-tab-info`
8. `detach`
9. `dump-layout`
10. `dump-screen`
11. `edit`
12. `edit-scrollback`
13. `focus-last-pane`
14. `focus-next-pane`
15. `focus-pane-id`
16. `focus-previous-pane`
17. `go-to-next-tab`
18. `go-to-previous-tab`
19. `go-to-tab`
20. `go-to-tab-by-id`
21. `go-to-tab-name`
22. `half-page-scroll-down`
23. `half-page-scroll-up`
24. `hide-floating-panes`
25. `launch-or-focus-plugin`
26. `launch-plugin`
27. `list-clients`
28. `list-panes`
29. `list-tabs`
30. `move-focus`
31. `move-focus-or-tab`
32. `move-pane`
33. `move-pane-backwards`
34. `move-tab`
35. `new-pane`
36. `new-tab`
37. `next-swap-layout`
38. `override-layout`
39. `page-scroll-down`
40. `page-scroll-up`
41. `paste`
42. `pipe`
43. `previous-swap-layout`
44. `query-tab-names`
45. `rename-pane`
46. `rename-session`
47. `rename-tab`
48. `rename-tab-by-id`
49. `resize`
50. `save-session`
51. `scroll-down`
52. `scroll-to-bottom`
53. `scroll-to-top`
54. `scroll-up`
55. `send-keys`
56. `set-dark-theme`
57. `set-light-theme`
58. `set-pane-borderless`
59. `set-pane-frame-style`
60. `set-pane-color`
61. `show-floating-panes`
62. `start-or-reload-plugin`
63. `stack-panes`
64. `switch-mode`
65. `switch-session`
66. `toggle-active-sync-tab`
67. `toggle-floating-panes`
68. `toggle-fullscreen`
69. `toggle-no-ui-fullscreen`
70. `toggle-pane-borderless`
71. `toggle-pane-embed-or-floating`
72. `toggle-pane-frames`
73. `toggle-pane-pinned`
74. `toggle-theme`
75. `undo-rename-pane`
76. `undo-rename-tab`
77. `write`
78. `write-chars`

---

## 문서상 불일치와 한계

1. **버전 식별 불가:** 두 공식 페이지는 적용 Zellij 버전/commit을 표시하지 않는다. 설치 바이너리와 최신 웹 문서의 옵션이 다를 수 있다.
2. **확인된 0.45.0 대비 차이:** 최신 웹 문서의 `toggle-no-ui-fullscreen`은 `--tab-id`를 열거하지만 0.45.0 source에는 없다. `set-pane-borderless`도 최신 문서는 `--borderless <BORDERLESS>` 값 형식인데 0.45.0 source는 bool flag 형태와 present/absent 설명을 쓴다. `resize` 역시 최신 웹 문서의 단일 인자 `[right|left|up|down|+|-]`와 달리 0.45.0 source는 `Resize` 값과 선택 `Direction`을 별도 positional로 정의한다.
3. **`new-pane`과 `run`은 완전한 동일 CLI가 아님:** Actions 페이지는 `new-pane`을 `zellij run`으로 줄일 수 있다고 하지만, 옵션 집합이 다르다. `new-pane`에는 plugin 관련 옵션과 `--pane-id`가 있고, Run 페이지에는 없다. 반대로 단축 옵션 의미도 다르다. `new-pane -b`는 blocking, `run -b`는 borderless다.
4. **`edit`의 방향 표현 차이:** action 문서는 `--direction [right|down]`만 명시하지만 top-level Edit 문서는 값 범위를 쓰지 않는다. 어느 쪽이 현재 바이너리의 완전한 허용 집합인지 웹 문서만으로 확정할 수 없다.
5. **`change-floating-pane-coordinates`의 pane ID 표기:** 설명은 pane ID를 “ARGS”라고 부르지만 옵션 표와 예시는 `--pane-id`를 사용한다.
6. **`override-layout` 적용 범위 표현:** 첫 문장은 활성 tab layout을 덮어쓴다고 하지만 `--apply-only-to-active-tab` 옵션도 별도로 존재한다. 기본 범위를 문장만으로 단정하기 어렵다.
7. **index 기준 불명:** `go-to-tab <INDEX>`의 0/1 기반 여부를 해당 페이지가 명시하지 않는다.
8. **일부 값/반복 규칙 미기재:** 여러 boolean placeholder, `pipe`의 `key=value` 복수 전달법, JSON/표 출력 스키마 안정성 등은 설명이 부족하다.
9. **Nested CLI 노출 모순:** Nested Sessions 페이지는 `FocusGuestSession`, `FocusHostSession`, `ToggleHostFullscreen`이 CLI에서도 가능하다고 하지만 CLI Actions 페이지와 0.45.x `CliAction` source에는 없다. 일반 `Action` enum에만 존재하므로 CLI 호출은 설치 버전의 `zellij action --help`로 확인해야 한다.
10. **실행 미검증:** 이 조사는 문서 조사이며 실제 세션 생성, pane 종료, plugin launch 등 부작용이 있는 로컬 실행은 하지 않았다.

## Sources

- Zellij, “CLI Actions”: <https://zellij.dev/documentation/cli-actions.html>
- Zellij, “Zellij Run & Edit”: <https://zellij.dev/documentation/zellij-run-and-edit.html>
- Zellij, “Nested Sessions”: <https://zellij.dev/documentation/nested-sessions.html>
- docs.rs, `zellij-utils` 0.45.0 `cli.rs`: <https://docs.rs/zellij-utils/0.45.0/src/zellij_utils/cli.rs.html#1-1650>
- docs.rs, `zellij-utils` 0.45.1 `cli.rs`: <https://docs.rs/zellij-utils/0.45.1/src/zellij_utils/cli.rs.html>
- docs.rs, `zellij-utils` 0.45.0 `input/actions.rs`: <https://docs.rs/zellij-utils/0.45.0/src/zellij_utils/input/actions.rs.html>
