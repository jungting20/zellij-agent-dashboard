# Zellij 셸 연동: OSC 133, 자동 시작, 환경 변수

> **조사 기준**: Zellij 공식 온라인 문서, 2026-10-08 열람. 아래 설정과 스크립트는 **실행·검증하지 않은 문서 기반 예시**다. 온라인 문서와 설치 버전이 다를 수 있으므로 실제 적용 전 `zellij --version`, `zellij setup --help` 및 셸 설정 문법을 확인한다.

## 1. OSC 133 semantic prompt 지원

Zellij는 **0.45.0부터** OSC 133 semantic prompt escape sequence를 이해한다. pane 안 셸이 이 mark를 출력하면 Zellij는 prompt, 입력한 명령, 명령 출력, 종료 상태의 경계를 안다. Zellij 쪽에서 별도 활성화 설정은 없으며, 셸이 sequence를 출력하면 자동 해석한다.

| Sequence | 의미 |
|---|---|
| `OSC 133 ; A` | prompt 시작 |
| `OSC 133 ; P` | 속성을 가진 prompt 시작 |
| `OSC 133 ; B` | prompt 종료 / 입력 명령 시작 |
| `OSC 133 ; I` | 입력 명령 시작 |
| `OSC 133 ; C` | 명령 출력 시작 |
| `OSC 133 ; D ; <exit-status>` | 명령 종료 및 종료 상태 |

`fish`는 이를 기본 출력한다. `bash`와 `zsh`는 integration snippet이 필요하다. kitty, WezTerm, VS Code 등 다른 터미널용으로 배포되는 snippet도 같은 표준 sequence를 내보내면 Zellij에서 작동한다.

### OSC 133으로 가능해지는 것

- **명령+출력 선택**: mark된 출력 내부를 triple-click하면 논리 줄 대신 해당 명령과 출력 전체를 선택한다. config의 `osc133_command_selection false`로 끌 수 있으며 기본은 true다.
- **키보드 선택**: scroll/search 모드에서 `SelectCommandAtScrollPosition` action을 쓴다. 기본 바인딩은 `m`.
- **prompt 단위 scrollback 이동**: scroll/search 모드에서 이전 prompt `[` (`ScrollToPreviousPrompt`), 다음 prompt `]` (`ScrollToNextPrompt`).
- **직전 명령 출력 복사**: scroll 모드 `c`의 `CopyLastCommandOutput` action은 focused pane의 마지막 명령 출력을 OSC 133 경계로 찾아 복사하고 해당 영역을 잠시 flash한다.
- 각 명령의 exit status도 알 수 있어, 예를 들어 stack의 접힌 pane 제목에서 실패 명령을 표시할 수 있다.

텍스트 더블클릭 선택의 문자 구분자는 OSC 133 기능 자체는 아니지만 관련 옵션 `word_separators`로 설정한다. 공백은 항상 구분자이며 기본값은 다음과 같다.

```kdl
word_separators "[]{}<>()"
```

## 2. Zellij가 제공하는 환경 변수

| 변수 | Zellij 세션 안에서의 값/동작 |
|---|---|
| `ZELLIJ` | `0` |
| `ZELLIJ_SESSION_NAME` | 세션 이름 |

`ZELLIJ_SESSION_NAME`은 세션을 rename해도 **기존 terminal pane에서는 갱신되지 않으며**, rename 뒤 새로 만든 pane에는 갱신된다.

config 또는 layout의 `env { KEY VALUE }` 블록으로 임의 환경 변수를 Zellij가 여는 terminal pane에 주입할 수도 있다.

```kdl
env {
    PROJECT_NAME "api"
    RUST_BACKTRACE 1
}
```

## 3. interactive shell에서 자동 시작

셸을 열 때 이미 Zellij 내부인지 `ZELLIJ`로 확인한다. fish의 문서 예시는 다음과 같다.

```fish
if set -q ZELLIJ
else
  zellij
end
```

Zellij는 셸별 auto-start 코드를 생성한다. 공식 문서는 Bash/Zsh rc 파일에 `eval ...`을 추가하는 방식을 예시로 든다. 기존 rc 파일을 자동 수정하거나 같은 줄을 중복 추가하지 않도록, 먼저 임시 파일로 생성 결과를 검토한다.

```bash
# 생성된 코드를 확인한다. 이 명령은 ~/.bashrc, ~/.zshrc를 바꾸지 않는다.
zellij setup --generate-auto-start bash > /tmp/zellij-auto-start.bash
zellij setup --generate-auto-start zsh  > /tmp/zellij-auto-start.zsh
```

검토 후 사용자가 기존 `~/.bashrc` 또는 `~/.zshrc`의 적절한 interactive-shell 영역에 다음 형태의 한 줄을 **한 번만** 수동 추가한다.

```bash
# bash: ~/.bashrc에 추가할 형태
eval "$(zellij setup --generate-auto-start bash)"

# zsh: ~/.zshrc에 추가할 형태
eval "$(zellij setup --generate-auto-start zsh)"
```

fish도 먼저 생성 결과를 검토한다.

```fish
zellij setup --generate-auto-start fish | string collect > /tmp/zellij-auto-start.fish
```

검토 후 `$HOME/.config/fish/config.fish`의 `if status is-interactive` 블록 안에 다음 형태를 수동으로 한 번만 추가한다.

```fish
eval (zellij setup --generate-auto-start fish | string collect)
```

생성 스크립트의 동작은 환경 변수로 제어할 수 있다.

| 변수 | 기본값 | 의미 |
|---|---|---|
| `ZELLIJ_AUTO_ATTACH` | `false` | 기본 세션이 이미 있으면 새 세션 대신 attach |
| `ZELLIJ_AUTO_EXIT` | `false` | Zellij 종료 시 부모 셸도 종료 |

공식 문서는 특정 fish 버전에서 설정이 작동하지 않을 수 있으며 issue #1534를 확인하라고 경고한다. 자동 시작을 적용하면 login shell, nested shell, SSH 접속에서 의도치 않은 세션 생성/attach가 없는지 별도 검증한다.

## 4. 세션 선택 및 레이아웃 선택 스크립트

공식 integration 문서의 의존성 있는 예시를 보존해 정리한다. 아래 스크립트는 Bash 및 외부 도구가 필요하다.

### 세션을 목록에서 선택

의존성: `bash`, [`sk` (skim)](https://github.com/lotabout/skim).

```bash
#!/usr/bin/env bash
ZJ_SESSIONS=$(zellij list-sessions)
NO_SESSIONS=$(echo "${ZJ_SESSIONS}" | wc -l)

if [ "${NO_SESSIONS}" -ge 2 ]; then
    zellij attach "$(echo "${ZJ_SESSIONS}" | sk)"
else
    zellij attach -c
fi
```

문서 예시는 출력 행 수가 2 이상이면 skim 선택을 열고, 아니면 `zellij attach -c`를 실행한다. `list-sessions` 출력 형식, 빈 출력, 종료 상태는 설치 버전에 맞게 먼저 확인해야 한다.

### 기본 레이아웃 디렉터리에서 선택해 시작

의존성: `bash`, `tr`, `fd`, `sed`, `sk`, `grep`.

```bash
#!/usr/bin/env bash
set -euo pipefail
ZJ_LAYOUT_DIR=$(zellij setup --check \
    | grep "LAYOUT DIR" - \
    | grep -o '".*"' - | tr -d '"')

if [[ -d "${ZJ_LAYOUT_DIR}" ]]; then
    ZJ_LAYOUT="$(fd --type file . "${ZJ_LAYOUT_DIR}" \
        | sed 's|.*/||' \
        | sk \
        || exit)"
    zellij --layout "${ZJ_LAYOUT}"
fi
```

이 예시는 `zellij setup --check` 출력에서 `LAYOUT DIR`를 grep으로 추출하므로, 사람이 읽는 help/status 출력이 바뀌면 깨질 수 있다. 또한 `sed`가 경로를 버리고 basename만 전달하므로, 같은 이름의 파일이 있으면 선택 결과가 모호할 수 있다. 이는 문서 예시의 구조에서 알 수 있는 제한이며, 동작을 보장하는 해석은 아니다.

## 5. 관련 설정 체크리스트

- OSC 133 명령 선택만 끄려면 `osc133_command_selection false`.
- 더블클릭 단어 경계는 `word_separators`로 조정한다.
- nested Zellij을 자주 열면 `nested_session_handling`의 `ask`(기본), `fullscreen`, `descend`, `never` 중 원하는 정책을 명시한다.
- 자동 시작은 `ZELLIJ` 존재 검사와 interactive-shell 가드가 중요하다.
- `ZELLIJ_SESSION_NAME`은 rename 뒤 기존 pane에서 stale할 수 있으므로, 장기 실행 프로세스가 세션 이름을 기준으로 판단하면 이 제약을 고려한다.

## 출처

- [Shell Integration (OSC 133)](https://zellij.dev/documentation/shell-integration.html)
- [Integration](https://zellij.dev/documentation/integration.html)
- [Options](https://zellij.dev/documentation/options.html)
