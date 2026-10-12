# Zellij 공식 `run` / `edit` 명령 정리

## 범위와 검증 상태

- 기준 문서: <https://zellij.dev/documentation/zellij-run-and-edit.html>
- 대응 action 대조: <https://zellij.dev/documentation/cli-actions.html>
- 버전 교차 확인: Zellij Utils 0.45.0 공식 docs.rs CLI source, <https://docs.rs/zellij-utils/0.45.0/src/zellij_utils/cli.rs.html#1-1650>
- 대상은 top-level 명령 **2개**, `zellij run`과 `zellij edit`다.
- 이 문서는 공식 웹 문서를 조사해 작성했으며 **로컬 Zellij 바이너리로 실행 검증하지 않았다.** 실제 설치 버전의 `zellij run --help`, `zellij edit --help`가 최종 기준이다.
- 공식 페이지에는 적용 버전 또는 commit이 표시되지 않으므로 설치 버전과 웹 문서 간 불일치 가능성이 있다.

---

## 1. `zellij run`

### 용도와 기본 형식

특정 command를 실행하는 새 Zellij pane을 만든다.

```bash
zellij run [OPTIONS] -- <COMMAND> [ARGS...]
```

공식 기본 예:

```bash
zellij run -- git diff
```

`--`는 Zellij 옵션과 새 pane에서 실행할 command를 구분한다.

### 정확한 옵션

| 옵션 | 값 | 공식 문서상 의미 | 제약/주의 |
|---|---:|---|---|
| `-b, --borderless <BORDERLESS>` | `true` 또는 `false` | border 없이 pane 시작 | borderless면 mouse로 pane을 이동할 수 없다고 경고 |
| `--block-until-exit` | 없음 | command가 종료 상태와 무관하게 끝나거나 pane이 닫힐 때까지 block | 아래 `--blocking`과 문구가 다름; 0.45.0 source에서 다른 block 옵션과 상호 배타적 |
| `--block-until-exit-failure` | 없음 | command가 non-zero로 끝나거나 pane이 닫힐 때까지 block | 성공 종료 시 처리의 세부 동작은 페이지가 설명하지 않음; 0.45.0 source에서 다른 block 옵션과 상호 배타적 |
| `--block-until-exit-success` | 없음 | command가 0으로 끝나거나 pane이 닫힐 때까지 block | 실패 종료 시 처리의 세부 동작은 페이지가 설명하지 않음; 0.45.0 source에서 다른 block 옵션과 상호 배타적 |
| `--blocking` | 없음 | command가 끝나고 그 pane이 닫힐 때까지 block | command 종료만이 아니라 pane close도 문구에 포함; 0.45.0 source에서 다른 block 옵션과 상호 배타적 |
| `-c, --close-on-exit` | 없음 | command 종료 즉시 pane 닫기 | 기본 동작과 반대 |
| `--close-replaced-pane` | 없음 | in-place로 대체한 pane을 suspend하지 않고 닫기 | `--in-place`에서만 효과 |
| `--cwd <CWD>` | 경로 | 새 pane 작업 디렉터리 변경 | 상대 경로 해석 기준은 페이지가 설명하지 않음 |
| `-d, --direction <DIRECTION>` | 방향 | 지정 방향에 새 pane 열기 | 허용 값 목록을 Run 페이지는 명시하지 않음; 0.45.0 source에서 `--floating`과 충돌 |
| `-f, --floating` | 없음 | floating mode로 열기 | 0.45.0 source에서 direction/in-place/stacked와 충돌하며 geometry/pinned의 전제 |
| `-h, --help` | 없음 | 도움말 출력 | 설치 바이너리의 현재 옵션 확인에 사용 |
| `--height <HEIGHT>` | 정수 또는 `%` | floating pane 높이 | 예: `1`, `10%`; 0.45.0 source에서 `--floating` 필요 |
| `-i, --in-place` | 없음 | 현재 pane 자리에 열고 기존 pane을 일시 suspend | `--close-replaced-pane`으로 suspend 대신 close 가능; 0.45.0 source에서 floating/direction과 충돌 |
| `-n, --name <NAME>` | 문자열 | 새 pane 이름 | 공백은 shell quoting 필요 |
| `--near-current-pane` | 없음 | 사용자 focus를 따르지 않고 명령을 실행한 현재 pane 가까이에 배치 | `--tab-id`와 충돌 |
| `--no-focus` | 없음 | 연결된 어떤 client의 focus도 변경하지 않음 | 배치는 focused pane이 아니라 명령을 실행한 pane 기준 |
| `--pinned <PINNED>` | boolean placeholder | floating pane을 항상 위에 고정할지 | 문서 표에는 possible values가 따로 적혀 있지 않음; 0.45.0 source 타입은 `Option<bool>`이고 `--floating` 필요 |
| `-s, --start-suspended` | 없음 | 처음에는 command를 suspended 상태로 시작하고 ENTER 후 실행 | 공식 문구의 “you first presses”는 문법 오탈자로 보임 |
| `--stacked` | 없음 | stacked pane으로 열기 | Run 페이지는 의미/충돌 조건을 더 설명하지 않음; Actions 페이지와 0.45.0 source에서는 floating/direction과 충돌 |
| `--tab-id <TAB_ID>` | 숫자 ID | 해당 ID의 tab에 pane 열기 | `--in-place`, `--near-current-pane`과 충돌 |
| `--width <WIDTH>` | 정수 또는 `%` | floating pane 너비 | 예: `1`, `10%`; 0.45.0 source에서 `--floating` 필요 |
| `-x, --x <X>` | 정수 또는 `%` | floating pane x 좌표 | 예: `1`, `10%`; 0.45.0 source에서 `--floating` 필요 |
| `-y, --y <Y>` | 정수 또는 `%` | floating pane y 좌표 | 예: `1`, `10%`; 0.45.0 source에서 `--floating` 필요 |

### 동작상 핵심 주의점

1. **기본은 command 종료 후 pane 유지**
   
   새 command가 끝나도 pane이 즉시 닫히지 않는다. pane frame에 exit status가 표시되며, 사용자는 `<ENTER>`로 같은 command를 다시 실행하거나 `<Ctrl-c>`로 pane을 닫을 수 있다. 즉시 닫으려면 `--close-on-exit`을 사용한다.

2. **`--blocking`과 `--block-until-exit*`는 문서 표현이 다름**
   
   `--blocking`은 “command가 끝나고 pane이 닫힐 때까지”, 다른 세 옵션은 “특정 exit 조건이 충족되거나 pane이 닫힐 때까지”로 쓰여 있다. 정확한 edge case는 웹 페이지에 더 설명되어 있지 않다. 0.45.0 source에서는 네 block 옵션이 상호 배타적이다.

3. **`--no-focus`의 배치 기준**
   
   focus를 바꾸지 않을 뿐 아니라 pane 배치도 현재 focused pane이 아니라 이 command를 실행한 pane을 기준으로 한다. background 자동화에서 중요한 차이다.

4. **`--tab-id`는 숫자 stable ID**
   
   `zellij action list-tabs` 또는 `zellij action query-tab-names`로 확인할 수 있다고 공식 문서가 안내한다. 단, `query-tab-names` 페이지는 이름 목록이라고만 설명해 ID 출력 형식을 자세히 보여 주지 않아 문서 간 설명이 충분히 맞물리지는 않는다.

5. **borderless의 mouse 제약**
   
   `--borderless true`로 시작하면 mouse로 pane을 이동할 수 없다고 공식 옵션 설명이 직접 경고한다.

### 예시

```bash
# 기본 command pane
zellij run -- git diff

# floating pane, 이름과 크기 지정
zellij run --floating --name "tests" --width 80% --height 60% -- cargo test

# 특정 작업 디렉터리에서 실행 후 바로 닫기
zellij run --cwd /path/to/project --close-on-exit -- make test

# 현재 pane을 임시 대체
zellij run --in-place -- htop

# 현재 pane을 영구적으로 닫고 대체
zellij run --in-place --close-replaced-pane -- htop

# background 배치
zellij run --no-focus -- tail -f /tmp/app.log

# 특정 tab ID에 배치
zellij run --tab-id 3 -- tail -f /tmp/app.log

# 성공 종료까지 호출 측을 block
zellij run --block-until-exit-success -- sh -c 'exit 0'
```

> 첫 번째는 공식 문서 예시다. 나머지는 공식 옵션 조합을 보여 주기 위한 구성 예시이며 로컬 실행 검증하지 않았다.

### `zellij action new-pane`과의 대응 및 차이

Actions 페이지는 `new-pane`을 `zellij run`으로 줄일 수 있다고 설명하지만, 문서에 열거된 CLI surface는 완전히 같지 않다.

| 항목 | `zellij run` | `zellij action new-pane` |
|---|---|---|
| command 구분 | `-- <COMMAND>` | `-- <COMMAND>` |
| 기본 새 terminal pane | 지원 | 지원 |
| floating/in-place/direction/stacked | 지원 | 지원 |
| blocking 계열 | 지원 | 지원 |
| `--no-focus`, `--tab-id` | 지원 | 지원 |
| plugin 직접 생성 | 문서에 없음 | `--plugin`, `--configuration`, `--skip-plugin-cache` |
| in-place 대상 pane 지정 | 문서에 없음 | `--pane-id <PANE_ID>` |
| `-b` 단축 옵션 | **borderless** | **blocking** |
| borderless 긴 옵션 | `--borderless <BORDERLESS>` | `--borderless <BORDERLESS>` |

따라서 자동화 script에서 두 형태를 기계적으로 치환하면 안 된다. 특히 `-b`는 의미가 반대 수준으로 달라 가장 위험하다.

---

## 2. `zellij edit`

### 용도와 기본 형식

기본 editor로 파일을 새 Zellij pane에 연다.

```bash
zellij edit [OPTIONS] <FILE>
```

공식 예:

```bash
zellij edit ./main.rs
zellij edit --floating ./main.rs
zellij edit ./main.rs --line-number 10
```

### 정확한 옵션

| 옵션 | 값 | 공식 문서상 의미 | 제약/주의 |
|---|---:|---|---|
| `-b, --borderless <BORDERLESS>` | `true` 또는 `false` | border 없이 pane 시작 | mouse로 pane 이동 불가 경고 |
| `--close-replaced-pane` | 없음 | in-place로 대체한 pane을 suspend하지 않고 닫기 | `--in-place`에서만 효과 |
| `--cwd <CWD>` | 경로 | editor 작업 디렉터리 변경 | FILE 상대 경로와의 조합 해석은 페이지가 설명하지 않음 |
| `-d, --direction <DIRECTION>` | 방향 | 지정 방향에 editor pane 열기 | Edit 페이지는 허용 값 목록을 쓰지 않음; 0.45.0 source에서 `--floating`과 충돌 |
| `-f, --floating` | 없음 | floating mode로 열기 | 0.45.0 source에서 direction/in-place와 충돌하며 geometry/pinned의 전제 |
| `-h, --help` | 없음 | 도움말 출력 | 설치 바이너리 확인용 |
| `--height <HEIGHT>` | 정수 또는 `%` | floating pane 높이 | 예: `1`, `10%`; 0.45.0 source에서 `--floating` 필요 |
| `-i, --in-place` | 없음 | 현재 pane 자리에 열고 기존 pane을 일시 suspend | 0.45.0 source에서 floating/direction과 충돌 |
| `-l, --line-number <LINE_NUMBER>` | 줄 번호 | 지정 line으로 파일 열기 | 0.45.0 source 타입은 `usize`; editor별 line 이동 방식/호환성은 페이지가 설명하지 않음 |
| `--near-current-pane` | 없음 | focus를 따르지 않고 현재 pane 가까이에 배치 | `--tab-id`와 충돌 |
| `--no-focus` | 없음 | 연결된 어떤 client의 focus도 변경하지 않음 | placement는 명령을 실행한 pane 기준이라고 Run 설명과 action 설명이 안내 |
| `--pinned <PINNED>` | boolean placeholder | floating pane을 항상 위에 고정할지 | possible values는 별도 명시 안 됨; 0.45.0 source 타입은 `Option<bool>`이고 `--floating` 필요 |
| `--tab-id <TAB_ID>` | 숫자 ID | 해당 ID의 tab에 editor pane 열기 | `--in-place`, `--near-current-pane`과 충돌 |
| `--width <WIDTH>` | 정수 또는 `%` | floating pane 너비 | 예: `1`, `10%`; 0.45.0 source에서 `--floating` 필요 |
| `-x, --x <X>` | 정수 또는 `%` | floating pane x 좌표 | 예: `1`, `10%`; 0.45.0 source에서 `--floating` 필요 |
| `-y, --y <Y>` | 정수 또는 `%` | floating pane y 좌표 | 예: `1`, `10%`; 0.45.0 source에서 `--floating` 필요 |

### editor 선택

공식 문서는 기본 editor를 다음에서 정한다고 설명한다.

- `$EDITOR`
- `$VISUAL`
- 또는 Zellij 설정의 `scrollback_editor`

다만 `$EDITOR`와 `$VISUAL`이 둘 다 설정됐을 때의 우선순위, 환경 변수와 `scrollback_editor` 사이의 정확한 우선순위는 이 페이지가 명시하지 않는다.

### 예시

```bash
# 일반 pane에서 파일 열기
zellij edit ./main.rs

# floating editor
zellij edit --floating ./main.rs

# 10번째 줄에서 열기
zellij edit ./main.rs --line-number 10

# floating 위치와 크기 지정
zellij edit --floating --x 10% --y 10% --width 80% --height 80% ./main.rs

# 현재 pane을 임시 대체
zellij edit --in-place ./main.rs

# 현재 pane을 닫고 editor로 대체
zellij edit --in-place --close-replaced-pane ./main.rs

# 특정 작업 디렉터리와 tab ID 사용
zellij edit --cwd /path/to/project --tab-id 3 src/main.rs

# focus를 바꾸지 않고 열기
zellij edit --no-focus ./main.rs
```

> 앞의 세 개는 공식 문서 예시다. 나머지는 공식 옵션 조합을 보여 주기 위한 구성 예시이며 로컬 실행 검증하지 않았다.

### `zellij action edit`과의 대응 및 차이

두 문서는 같은 기능을 설명하지만 표기 수준에 차이가 있다.

| 항목 | `zellij edit` 페이지 | `zellij action edit` 페이지 |
|---|---|---|
| positional file | 예시로 명확 | `<PATH>` 인자로 명시 |
| 반환값 | 설명 없음 | 생성 pane ID를 반환한다고 명시 |
| `--direction` 허용 값 | 값 목록 미기재 | `[right|down]` 명시 |
| `-h, --help` | 열거 | action 옵션 목록에는 미기재 |
| floating/in-place/cwd/line/tab/no-focus 등 | 열거 | 동일 계열 열거 |
| borderless 값 | `true`, `false` 명시 | `<BORDERLESS>`만 표기 |

즉 `zellij edit`가 action `edit`의 top-level 형태라는 설명은 일치하지만, 두 페이지의 세부 정보가 동일한 완전 목록으로 정규화되어 있지는 않다.

---

## 3. Run과 Edit 비교

| 구분 | `zellij run` | `zellij edit` |
|---|---|---|
| 주 목적 | 임의 command 실행 | 기본 editor로 file 열기 |
| positional | `-- <COMMAND> [ARGS...]` | `<FILE>` |
| command 종료 처리 | 기본 유지, rerun/close UI 제공 | editor 종료 후 처리 세부 설명 없음 |
| close-on-exit | `-c, --close-on-exit` 있음 | 문서에 없음 |
| blocking | 4종 있음 | 없음 |
| start suspended | `-s, --start-suspended` 있음 | 없음 |
| name | `-n, --name` 있음 | 없음 |
| line number | 없음 | `-l, --line-number` 있음 |
| common placement | direction, floating, in-place, near-current, no-focus, tab-id | 동일 |
| common floating geometry | x, y, width, height, pinned, borderless | 동일 |
| close replaced pane | 있음 | 있음 |

---

## 4. 문서상 불일치와 한계

1. **버전 정보 없음:** 두 대상 웹 페이지는 Zellij 버전 또는 source commit을 표시하지 않는다. 최신 사이트 문서와 설치된 바이너리의 `--help`가 다를 수 있다.
2. **0.45.0 source로 보완 가능한 제약:** Run/Edit의 direction 대 floating, in-place 대 direction/floating, geometry/pinned의 floating 요구 조건과 block 옵션 상호 충돌은 0.45.0 source에 명시되어 있으나 웹 옵션 표에는 대부분 드러나지 않는다. 버전 차이일 수도 있고 웹 설명 누락일 수도 있어 버전별 `--help` 확인이 필요하다.
3. **`run`과 `action new-pane`의 불완전한 alias 관계:** 공식 문구는 축약 가능하다고 하지만 옵션 집합과 단축 옵션이 다르다. 특히 `-b` 의미가 충돌한다.
4. **`edit` 방향 값 범위 불일치:** action 페이지는 `[right|down]`, top-level Edit 페이지는 `<DIRECTION>`만 제시한다.
5. **도움말 표기 차이:** top-level 페이지는 `-h, --help`를 열거하지만 action `edit`/`new-pane` 섹션은 이를 열거하지 않는다. CLI framework가 자동 제공하는지 여부는 웹 문서만으로 단정하지 않았다.
6. **block 조건 edge case 미설명:** 성공 대기 중 실패 종료, 실패 대기 중 성공 종료 후 호출이 어떻게 지속되는지 등은 페이지가 구체적으로 설명하지 않는다.
7. **boolean 값 일부 미열거:** borderless는 `true/false`를 명시하지만 `--pinned <PINNED>` 등은 값 범위를 명시하지 않는다.
8. **editor 우선순위 미기재:** `$EDITOR`, `$VISUAL`, `scrollback_editor`의 정확한 precedence가 없다.
9. **output/return 세부 부족:** action `edit`은 pane ID 반환을 명시하지만 top-level Edit 페이지는 반환 형식을 설명하지 않는다.
10. **로컬 미검증:** 실제 Zellij 실행, pane 생성/종료, editor/plugin 호출은 하지 않았다.

## Sources

- Zellij, “Zellij Run & Edit”: <https://zellij.dev/documentation/zellij-run-and-edit.html>
- Zellij, “CLI Actions”: <https://zellij.dev/documentation/cli-actions.html>
- docs.rs, `zellij-utils` 0.45.0 `cli.rs`: <https://docs.rs/zellij-utils/0.45.0/src/zellij_utils/cli.rs.html#1-1650>
