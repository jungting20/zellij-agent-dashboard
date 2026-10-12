# Zellij 레이아웃: KDL 문법, pane, tab, template, swap

> **조사 기준**: Zellij 공식 온라인 문서, 2026-10-08 열람. 다음 KDL과 명령 예시는 **실행·검증하지 않은 문서 기반 예시**다. 설치된 Zellij 버전과 온라인 문서가 다를 수 있으므로 `zellij setup --dump-layout default`, `zellij setup --dump-swap-layout default`, `zellij --help`로 확인한다.

## 1. 개념·파일 위치·적용

레이아웃은 pane과 tab의 배치를 정의하는 KDL 텍스트 파일이다. 기본은 설정 디렉터리의 `layouts/default.kdl`을 로드하며, 없으면 pane 하나와 tab 하나로 시작한다. 이 기본 디렉터리의 레이아웃은 확장자/경로 없이 이름으로 지정할 수 있다.

```bash
# 파일 경로, layouts 디렉터리의 이름, 원격 URL 모두 가능
zellij --layout /path/to/layout.kdl
zellij --layout my-layout
zellij --layout https://example.com/layout.kdl

# 실행 중인 세션에서 같은 명령: 새 tab 하나 이상으로 레이아웃을 시작
zellij --layout /path/to/layout.kdl

# 현재 tab의 배치를 교체
zellij action override-layout /path/to/new-layout.kdl
zellij action override-layout /path/to/new-layout.kdl \
  --retain-existing-terminal-panes --apply-only-to-active-tab
```

원격 URL 레이아웃은 보안을 위해 모든 명령 pane을 `Waiting to run <command>` 배너 뒤에서 대기시킨다. 사용자가 각각 실행을 승인해야 한다.

`default_layout "이름"`과 `layout_dir "경로"`는 config에서 지정할 수 있다. 레이아웃 파일은 config에서 가능한 설정을 함께 포함할 수 있고, 그 값은 로드된 config보다 우선한다. 단 `new-tab` action으로 읽을 때 레이아웃 내 config 필드는 무시된다.

## 2. 최소 문법과 tiled split

모든 구조는 `layout` 아래에 둔다. `pane`은 실제 pane일 수도, 자식 pane을 정렬하는 논리 컨테이너일 수도 있다. 컨테이너의 속성은 **제목 줄**에 둔다. `split_direction` 기본은 `horizontal`이며, `layout` 자체도 horizontal이다.

```kdl
layout {
    pane
    pane split_direction="vertical" {
        pane
        pane command="htop"
    }
}
```

`split_direction` 값은 `"horizontal"` 또는 `"vertical"`이다. 레이아웃 전체를 vertical로 시작하려면 아래처럼 논리 pane을 하나 둔다.

```kdl
layout {
    pane split_direction="vertical" {
        pane
        pane
    }
}
```

### pane 속성

| 속성 | 값 | 설명 |
|---|---|---|
| `split_direction` | `"horizontal"` / `"vertical"` | 자식 배치 방향. 기본 horizontal |
| `size` | `"50%"` 또는 정수 | 컨테이너 안 공간. 일반 pane의 고정 정수 크기는 resize/close에서 불안정할 수 있다고 문서가 경고 |
| `borderless` | bool, 기본 false | pane 프레임 제거 |
| `focus` | bool, 기본 false | 시작 focus. 여러 개면 처음 선언된 pane이 focus |
| `name` | 문자열 | pane 제목 |
| `cwd` | 절대/상대 경로 | 작업 디렉터리 |
| `command` | 실행 파일 경로 또는 PATH 명령 | 기본 셸 대신 실행할 명령 |
| `edit` | 파일 경로 | `$EDITOR`/`$VISUAL` (또는 `scrollback_editor`)로 파일 열기 |
| `default_fg`, `default_bg` | `"#rrggbb"` 또는 `"rgb:rr/gg/bb"` | pane 기본 색 |
| `stacked` | bool | 자식들을 stack으로 배치 |
| `expanded` | bool | stacked 컨테이너에서 처음 확장할 자식 |

명령 pane의 `args`는 자식 블록 안에 두며, 제목 줄에는 쓸 수 없다. `close_on_exit true`는 명령 종료 즉시 pane을 닫고, `start_suspended true`는 Enter를 누를 때까지 명령 실행을 보류한다.

```kdl
layout {
    pane name="logs" command="tail" close_on_exit=true {
        args "-f" "/tmp/app.log"
        cwd "/tmp"
    }
    pane command="bash" start_suspended=true {
        args "-c" "echo ready"
    }
    pane edit="./README.md"
    pane default_fg="#00e000" default_bg="rgb:00/1a/3a"
}
```

plugin pane은 현재 자식 블록 안에서 `plugin location="..."`로만 지정한다.

```kdl
pane size=2 borderless=true {
    plugin location="zellij:status-bar"
}
```

## 3. tab과 floating pane

`tab`은 시작 시 여러 Zellij tab을 만들며, 속성(`name`, `split_direction`, `hide_floating_panes`, `cwd`, `focus`)은 제목 줄에 둔다. 명시적 `tab`이 하나라도 있으면 해당 속성을 `layout`에 두면 오류이며 각 `tab`으로 옮겨야 한다. focus tab은 하나만 가능하다.

```kdl
layout {
    tab name="editor" cwd="/work/project" focus=true {
        pane edit="src/main.rs"
        pane edit="Cargo.toml"
    }
    tab name="run" split_direction="vertical" {
        pane command="cargo" { args "run" }
        pane command="cargo" { args "test" }
    }
}
```

`floating_panes`는 root 또는 `tab` 안에 두며, 그 안의 pane은 floating이다. `x`, `y`, `width`, `height`는 화면 가장자리 기준 정수(cell) 또는 퍼센트 문자열이다. floating pane에는 일반 pane 속성을 쓸 수 있지만 자식 pane/`split_direction`처럼 무관한 속성은 쓰지 않는다. `hide_floating_panes=true`인 tab의 floating pane은 시작 때 숨겨진다.

```kdl
layout {
    tab name="main" hide_floating_panes=true {
        pane
        floating_panes {
            pane command="lazygit" {
                x "10%"
                y "10%"
                width "80%"
                height "70%"
            }
        }
    }
}
```

## 4. CWD 조합 규칙

상대 `cwd`는 바깥 컨테이너 `cwd`에 순서대로 이어 붙는다. 우선순위/조합 순서는 **pane → tab → layout 전역 cwd → 명령을 실행한 위치**다. 절대 경로는 바깥 cwd를 덮어쓴다.

```kdl
layout {
    cwd "/home/me/project"
    tab cwd="crates" {
        pane cwd="app" // /home/me/project/crates/app
        pane cwd="/tmp" // /tmp
    }
}
```

## 5. template

반복되는 pane/tab 정의는 template으로 줄인다. template 이름을 `pane`/`tab` 대신 노드명으로 쓴다.

### pane_template

```kdl
layout {
    pane_template name="follow-log" command="tail"
    follow-log { args "-f" "/tmp/service.log" }
}
```

- `command`를 가진 pane template의 직접 소비자는 `args`, `cwd`를 줄 수 있다. 다른 template을 거친 간접 소비에는 이 규칙이 적용되지 않는다.
- 컨테이너 template에서 `children`은 소비자의 자식 pane이 삽입될 자리다. `children`은 pane 안에는 중첩할 수 있지만 다른 `pane_template` 안에는 둘 수 없다.

```kdl
layout {
    pane_template name="vertical-sandwich" split_direction="vertical" {
        pane command="htop"
        children
        pane command="bash"
    }
    vertical-sandwich {
        pane command="git" { args "status" }
    }
}
```

### tab_template, default_tab_template, new_tab_template

`tab_template`도 `children`으로 공통 tab UI를 정의한다. template 소비자가 넘기는 속성은 제목 줄에만 쓴다.

```kdl
layout {
    default_tab_template {
        pane size=1 borderless=true { plugin location="zellij:tab-bar" }
        children
        pane size=2 borderless=true { plugin location="zellij:status-bar" }
    }
    tab name="code" { pane }
    tab name="tests" { pane command="cargo" { args "test" } }
}
```

- `default_tab_template`는 모든 일반 `tab`과 세션에서 새로 여는 tab에 적용한다. 다른 `tab_template`를 쓴 tab에는 적용하지 않는다.
- 명시적 `tab`이 전혀 없으면 전체 레이아웃이 `default_tab_template`처럼 취급된다.
- `new_tab_template`은 초기 tab들과 별개로, 이후 새 tab을 열 때만 쓰는 blueprint다.

## 6. stacked pane

`stacked=true` 컨테이너의 자식은 한 pane만 보통 크기로 확장되고 나머지는 제목 한 줄로 접힌다. `expanded=true` 자식이 있으면 그것을 먼저 확장하며, 없으면 가장 아래 pane이 기본이다. 0.45.0부터는 제목 목록과 하단에 고정된 확장 pane으로 렌더링된다. 이전 방식은 config의 `stacked_pane_list false`로 복원한다.

```kdl
layout {
    pane stacked=true {
        pane command="watch" { args "-n" "1" "date" }
        pane command="htop" expanded=true
        pane
    }
}
```

## 7. Swap layouts

swap layout은 pane을 열고 닫을 때 미리 정의한 위치로 재배치하는 확장이다.

- `swap_tiled_layout`: tiled pane용. 하나 이상의 `tab` 또는 tab template을 포함한다.
- `swap_floating_layout`: floating pane용. 하나 이상의 `floating_panes` 또는 tab template을 포함한다.
- 각 node에 `name`을 붙이면 UI에 현재 layout 이름이 표시된다.
- 하나의 layout에 여러 swap node를 둘 수 있다. 기본 키는 `Alt + []`로 수동 전환하며, pane 수 제약과 맞지 않을 때 자동 전환할 수도 있다.
- 기본으로 로드한 **Base** layout도 tiled/floating 각각 현재 pane 총수의 암묵적 `exact_panes` 제약을 가진다.

```kdl
layout {
    swap_tiled_layout name="two-to-three" {
        tab max_panes=2 {
            pane
            pane
        }
        tab {
            pane split_direction="vertical" {
                pane
                pane
                pane
            }
        }
    }

    swap_floating_layout name="float-grid" {
        floating_panes max_panes=1 { pane }
        floating_panes max_panes=2 {
            pane x=0 width="50%"
            pane x="50%" width="50%"
        }
    }
}
```

각 swap `tab`/`floating_panes`에는 `max_panes`, `min_panes`, `exact_panes` 중 하나의 제약을 붙일 수 있다. swap node가 화면보다 많은 pane을 정의하면 breadth-first로 배치한다. 더 적게 정의하면 정의된 pane부터 적용하고 나머지는 **명세되지 않은 방식**으로 배치된다.

swap layout 안의 `command`/`plugin` pane은 base layout에 이미 화면에 있어야 한다. swap 정의에 있다고 새로 열거나, 없다고 닫지 않는다. 그렇지 않은 동작은 공식 문서에서 unspecified이며 향후 변경될 수 있다고 한다.

별도 파일도 가능하다. base가 `my-layout.kdl`이면 같은 폴더에 `my-layout.swap.kdl`을 둔다. 이 파일에는 `layout` wrapper 없이 `swap_tiled_layout`/`swap_floating_layout`를 직접 둔다.

```bash
zellij setup --dump-layout default > /tmp/default.kdl
zellij setup --dump-swap-layout default > /tmp/default.swap.kdl
```

## 8. 실무용 예제: Rust 프로젝트

```kdl
layout {
    default_tab_template {
        pane size=1 borderless=true { plugin location="zellij:tab-bar" }
        children
        pane size=2 borderless=true { plugin location="zellij:status-bar" }
    }
    cwd "/home/me/code/my-rust-app"
    tab name="dev" focus=true {
        pane split_direction="vertical" size="60%" {
            pane edit="src/main.rs"
            pane edit="Cargo.toml"
        }
        pane split_direction="vertical" size="40%" {
            pane command="cargo" focus=true { args "run" }
            pane command="cargo" { args "test" }
        }
    }
}
```

위의 경로, plugin, 실행 명령은 환경에 맞게 바꿔야 하며, 실제 실행 전 경로와 명령 존재 여부를 확인한다.

## 출처

- [Layouts](https://zellij.dev/documentation/layouts.html)
- [Creating a Layout](https://zellij.dev/documentation/creating-a-layout.html)
- [Swap Layouts](https://zellij.dev/documentation/swap-layouts.html)
- [Layouts with Config](https://zellij.dev/documentation/layouts-with-config.html)
- [Layout Examples](https://zellij.dev/documentation/layout-examples.html)
