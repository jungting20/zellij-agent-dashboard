# Zellij 설정과 CLI 옵션

> **조사 기준**: Zellij 공식 온라인 문서(아래 출처), 2026-10-08 열람. 이 문서의 명령과 KDL 예시는 **실행·검증하지 않은 문서 기반 예시**다. 설치된 Zellij 버전의 `zellij --help`, `zellij options --help`와 대조해 사용한다. 특히 공식 문서는 0.45.0 이후 변경 사항을 포함한다.
>
> 범위: 설정 및 CLI 설정 옵션. 테마 목록과 모든 키바인드의 상세 목록은 제외한다.

## 1. 형식과 설정 파일 탐색 순서

Zellij의 설정 언어는 **KDL**이다. 기본 설정 파일 이름은 `config.kdl`이며, 설정 디렉터리를 다음 우선순위로 찾는다.

1. `--config-dir` 플래그
2. `ZELLIJ_CONFIG_DIR` 환경 변수
3. `$HOME/.config/zellij`
4. 플랫폼 기본 위치: Linux 예시 `/home/alice/.config/zellij`, macOS 예시 `/Users/Alice/Library/Application Support/org.Zellij-Contributors.Zellij`
5. 시스템 위치 `/etc/zellij`

`--config FILE` 또는 `ZELLIJ_CONFIG_FILE`로 특정 파일을 직접 지정한다. 기본 디렉터리의 설정을 읽지 않고 시작하는 방법으로 공식 문서는 `zellij options --clean`을 제시한다.

```bash
# 기본 설정을 우선 임시 파일로 출력해 검토한다. 기존 config.kdl을 덮어쓰지 않는다.
zellij setup --dump-config > /tmp/zellij-default-config.kdl

# 특정 설정 파일을 직접 지정
zellij --config /path/to/config.kdl

# 기본 위치의 config를 읽지 않고 Zellij 내장 기본값으로 시작
zellij setup --clean
```

- 첫 실행 때 파일을 자동 생성하는 경우가 많으므로, 덮어쓰기 전 존재 여부를 확인한다.
- 활성 설정 파일은 감시된다. 대부분의 변경은 재시작 없이 적용되지만, 옵션별로 재시작 필요가 명시된 경우는 예외다.
- 설정 병합 우선순위는 **CLI 설정 옵션(`zellij options`) > 레이아웃 옵션(`layout.kdl`/`zellij --layout`) > config 옵션(`config.kdl`)**이다. 단, `new-tab` action으로 레이아웃을 불러오면 레이아웃 내 설정 필드는 무시된다.

## 2. KDL 기본 형태

루트에 단일 옵션을 놓고, 맵/계층형 값은 중괄호 블록으로 작성한다.

```kdl
// config.kdl
mouse_mode false
scroll_buffer_size 20000
default_shell "fish"
layout_dir "/home/me/.config/zellij/layouts"

env {
    RUST_BACKTRACE 1
    PROJECT_ENV "dev"
}

ui {
    pane_frames {
        rounded_corners true
        hide_session_name true
    }
}
```

## 3. 설정 옵션 요약

아래 이름은 config KDL의 `snake_case` 이름이다. CLI에서는 원칙적으로 같은 이름을 `kebab-case`로 쓴다. 예: `nested_session_handling` → `--nested-session-handling`.

### 동작·표시

| KDL 옵션 | 값/기본값 | 의미 |
|---|---|---|
| `on_force_close` | `detach`(기본), `quit` | SIGTERM/INT/QUIT/HUP 수신 시 동작 |
| `simplified_ui` | bool, 기본 false | 플러그인에 단순 UI 요청 |
| `default_shell` | 경로, 기본 `$SHELL` | 새 pane의 셸 |
| `default_mode` | 모드, 기본 `normal` | 시작 모드 |
| `mouse_mode` | bool, 기본 true | 마우스 이벤트 처리 |
| `pane_frames` | bool, 기본 true | pane 프레임 표시 |
| `pane_frame_style` | `titles`(기본), `full`, `none` | 프레임 모양. `pane_frames false`는 강제로 `none` |
| `rounded_corners` | `ui { pane_frames { ... } }` 안 bool | 프레임 모서리 둥글게 |
| `hide_session_name` | 같은 `ui` 블록 안 bool | UI에서 세션 이름 숨김 |
| `scroll_buffer_size` | 양의 정수, 기본 10000 | pane별 스크롤백 줄 수 |
| `scrollback_editor` | 경로, 기본 `$EDITOR` 또는 `$VISUAL` | 스크롤백과 `edit`용 편집기 |
| `auto_layout` | bool, 기본 true | pane 수에 맞춰 사전 정의 레이아웃 적용 |
| `stacked_resize` | bool, 기본 true | 비방향 resize 시 이웃 pane을 stack 시도 |
| `stacked_pane_list` | bool, 기본 true | stack을 제목 목록+하단 확장 pane으로 렌더링. false는 0.45.0 이전 방식 |
| `styled_underlines` | bool, 기본 true | 확장 ANSI styled underline 처리 |
| `visual_bell` | bool, 기본 true | bell 시 pane/tab 프레임 플래시 및 `[!]` 표시 |
| `show_startup_tips` / `show_release_notes` | bool, 기본 true | 시작 팁 / 새 버전 릴리스 노트 표시 |

### 테마·클립보드·링크

| KDL 옵션 | 값/기본값 | 의미 |
|---|---|---|
| `theme` | 이름, 기본 `default` | 정적 기본 테마 |
| `theme_dark`, `theme_light` | 테마 이름 | 터미널의 dark/light 보고에 따른 테마. **둘 다** 있어야 `theme` 대신 전환 |
| `explicit_theme_hue` | `dark` 또는 `light`, 기본 미설정 | 터미널 보고를 무시하고 시작 전 hue 고정 |
| `copy_command` | 명령 문자열 | OSC 52 대신 표준입력으로 선택 텍스트를 넘길 복사 명령 (`pbcopy`, `wl-copy` 등) |
| `copy_clipboard` | `system`(기본), `primary` | OSC 52 대상. `copy_command` 사용 시 적용 안 됨 |
| `copy_on_select` | bool, 기본 true | 마우스 선택 해제 시 자동 복사 |
| `osc8_hyperlinks` | bool, 기본 false | OSC 8 링크를 클릭 가능하게 출력 |
| `word_separators` | 기본 `"[]{}<>()"` | 더블클릭 단어 선택의 추가 구분 문자. 공백은 항상 구분자 |
| `osc133_command_selection` | bool, 기본 true | OSC 133 표시된 출력 triple-click 시 명령+출력 선택 |

### 세션·경로·환경

| KDL 옵션 | 값/기본값 | 의미 |
|---|---|---|
| `default_layout` | 이름, 기본 `default` | 시작할 layouts 폴더 내 레이아웃 |
| `layout_dir`, `theme_dir` | 디렉터리 | 레이아웃/테마 탐색 디렉터리 |
| `default_cwd` | 경로 | 별도 지정 없는 새 pane의 작업 디렉터리 |
| `env { KEY VALUE }` | 맵 | Zellij가 시작하는 모든 terminal pane에 주입할 환경 변수 |
| `session_name` | 문자열 | 생성할 세션 이름. 미지정 시 랜덤 |
| `attach_to_session` | bool, 기본 false | 같은 `session_name`이 있으면 새로 만들지 않고 attach |
| `mirror_session` | bool, 기본 false | 여러 사용자가 붙을 때 커서/화면 미러링 |
| `nested_session_handling` | `ask`(기본), `fullscreen`, `descend`, `never` | pane 안의 중첩 Zellij 세션 처리 |

### 복원·프로토콜·마우스

| KDL 옵션 | 값/기본값 | 의미 |
|---|---|---|
| `session_serialization` | bool, 기본 true | 세션을 캐시에 저장해 복원 가능하게 함 |
| `pane_viewport_serialization` | bool, 기본 false | 보이는 viewport까지 직렬화. `session_serialization`과 함께 사용 |
| `scrollback_lines_to_serialize` | 0 또는 정수 | viewport 직렬화 시 저장할 scrollback 줄. 0은 전부, 상한은 scrollback 크기 |
| `serialization_interval` | 초 단위 정수 | 디스크 직렬화 주기 |
| `disable_session_metadata` | bool, 기본 false | 세션 메타데이터 디스크 기록 비활성화. session-manager/listing이 제대로 동작하지 않을 수 있음 |
| `post_command_discovery_hook` | 셸 명령 | 복원용 실행 명령 탐지 뒤 실행. `$RESURRECT_COMMAND`를 받아 stdout을 저장 값으로 사용 |
| `support_kitty_keyboard_protocol` | bool | Kitty 키보드 프로토콜. 지원 터미널이면 기본 true |
| `support_kitty_graphics_protocol` | bool | Kitty 이미지 프로토콜. 지원 터미널이면 기본 true, **변경 후 재시작 필요** |
| `host_notification_protocol` | `auto`(기본), `osc9`, `osc99`, `bell`, `off` | pane의 OSC 9/99/777 알림을 호스트 터미널로 전달하는 방식 |
| `scroll_mode_sync` | bool, 기본 true | 스크롤이 scroll mode 진입/종료와 동기화되는지 |
| `advanced_mouse_actions` | bool, 기본 true | hover, 다중 pane 선택, 마우스 resize 활성화 |
| `mouse_scroll_resize` | bool, 기본 true | Ctrl+휠로 focused pane 크기 변경 |
| `mouse_hover_effects`, `mouse_hover_tips` | bool, 기본 true | hover 시각 효과 / 맥락 도움말 |
| `focus_follows_mouse`, `mouse_click_through` | bool, 기본 false | hover focus / focus 클릭을 프로그램에도 전달 |

### Web·공유·보안

| KDL 옵션 | 값/기본값 | 의미 |
|---|---|---|
| `web_server` | bool, 기본 false | 시작 때 로컬 web server 보장 |
| `web_server_ip`, `web_server_port` | 기본 `127.0.0.1`, `8082` | web server listen 주소/포트 |
| `web_server_cert`, `web_server_key` | 경로 | HTTPS 인증서/개인키. 둘 다 있어야 HTTPS 제공 |
| `enforce_https_on_localhost` | bool | localhost HTTPS 강제. non-localhost에서는 항상 강제 |
| `web_client { base_url ... }` | 예: `base_url "/zellij"` | reverse proxy 하위 경로용 base URL 및 web client 설정 |
| `web_sharing` | `on`, `off`(기본), `disabled` | 새 세션의 web 공유 기본값. `disabled`는 opt-in도 차단 |
| `dangerously_enable_paste_buffer_read` | bool, 기본 false | 프로그램의 OSC 52 **클립보드 읽기** 허용. SSH 원격 프로그램도 무단 읽을 수 있으므로 위험. OSC 52 쓰기는 이 옵션과 무관 |

## 4. CLI 설정 옵션 전체 대조표

공식 문서에서 `zellij options --help` 출력으로 제공한 옵션이다. 모든 config 옵션에 거의 대응하며, 반복 플래그는 0.45.0(Clap 4)부터 오류 대신 **마지막 값이 우선**한다.

- 예외 1: KDL `pane_viewport_serialization`의 CLI는 `--serialize-pane-viewport`.
- 예외 2: `web_server_ip`, `web_server_port`, `web_server_cert`, `web_server_key`, `enforce_https_on_localhost`는 플래그가 아니라 `zellij options`의 위치 인수다.
- 이전 `--disable-mouse-mode`, `--no-pane-frames`는 제거됐다. 각각 `--mouse-mode false`, `--pane-frames false`를 쓴다.

```text
zellij options [OPTIONS] [WEB_SERVER_IP] [WEB_SERVER_PORT]
  [WEB_SERVER_CERT] [WEB_SERVER_KEY] [ENFORCE_HTTPS_FOR_LOCALHOST]
```

**문서에 열거된 플래그**

```text
--simplified-ui                 --theme                         --theme-dark
--theme-light                   --explicit-theme-hue            --default-mode
--default-shell                 --default-cwd                   --default-layout
--layout-dir                    --theme-dir                     --mouse-mode
--pane-frames                   --pane-frame-style              --mirror-session
--on-force-close                --scroll-buffer-size            --copy-command
--copy-clipboard                --copy-on-select                --osc8-hyperlinks
--scrollback-editor             --session-name                  --attach-to-session
--auto-layout                   --session-serialization         --serialize-pane-viewport
--scrollback-lines-to-serialize --styled-underlines              --serialization-interval
--disable-session-metadata      --support-kitty-keyboard-protocol
--support-kitty-graphics-protocol --web-server                   --web-sharing
--stacked-resize                --stacked-pane-list             --show-startup-tips
--show-release-notes            --advanced-mouse-actions        --mouse-scroll-resize
--scroll-mode-sync              --mouse-hover-effects           --mouse-hover-tips
--visual-bell                   --focus-follows-mouse           --mouse-click-through
--osc133-command-selection      --word-separators               --host-notification-protocol
--post-command-discovery-hook   --client-async-worker-tasks     --nested-session-handling
--dangerously-enable-paste-buffer-read
```

`--client-async-worker-tasks`는 활성 client당 async worker 수이며, 0은 물리 CPU 코어 수를 쓴다. 공식 도움말상 현재 web client에만 적용된다. bool 플래그는 값을 명시한다. 예: `zellij options --mouse-mode false --pane-frames false`.

### 문서 간 주의점

- Configuration 문서는 `zellij options --clean`을 안내하지만, 같은 시점의 `zellij options --help` 목록에는 `--clean`이 보이지 않는다. 0.45.0 API 문서는 `clean`을 `zellij setup --clean`의 옵션으로 정의한다. 따라서 이 문서는 `zellij setup --clean`을 사용하며, 설치 버전의 help를 우선한다.
- `scrollback_lines_to_serialize` 설명 예제는 `pane_viewport_serialization 100`으로 표시되어 있어 옵션명과 맞지 않는다. 의도된 설정 키는 제목상 `scrollback_lines_to_serialize`이므로, 실제 사용 전 dump/default config와 `--help`로 확인한다.

## 출처

- [Configuration](https://zellij.dev/documentation/configuration.html)
- [Command Line Configuration Options](https://zellij.dev/documentation/command-line-options.html)
- [Options](https://zellij.dev/documentation/options.html)
- [Layouts with Config](https://zellij.dev/documentation/layouts-with-config.html)
- [zellij-utils 0.45.0 `Setup` API](https://docs.rs/zellij-utils/0.45.0/zellij_utils/setup/struct.Setup.html) (clean 동작 및 설정 병합 우선순위)
