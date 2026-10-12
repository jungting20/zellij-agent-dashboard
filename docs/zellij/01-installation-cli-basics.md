# 설치와 CLI 기본 구조

조사일: 2026-10-08. 공식 온라인 가이드와 `zellij-utils 0.45.0` 공개 CLI 정의를 기준으로 정리했다. 현재 조사 환경의 PATH에서는 `zellij`를 찾지 못했으므로 아래 명령은 실행 검증하지 않았다. 설치된 바이너리의 `--help`를 최종 기준으로 삼는다.

## 1. 설치

공식 가이드는 OS 패키지, 릴리스 바이너리, Cargo 설치를 안내한다. 패키지 저장소는 Zellij 유지관리자와 직접 관련 없는 제3자일 수 있다.

```sh
# Rust/Cargo가 이미 설치된 경우: 소스 컴파일
cargo install --locked zellij

# cargo-binstall이 이미 설치된 경우: 바이너리 설치
cargo binstall zellij

# 공식 릴리스에서 OS/CPU에 맞는 파일을 내려받은 뒤
# 와일드카드로 여러 파일을 동시에 풀지 않도록 실제 파일명을 지정한다.
tar -xvf <다운로드한-zellij.tar.gz>
chmod +x zellij
./zellij --version
```

Cargo 컴파일 실패 시 공식 가이드는 rustup 설치 사용자의 `rustup update`를 제안한다. 업데이트는 도구체인에 영향을 주므로 필요할 때 수행한다. Linux/macOS/Windows 릴리스가 안내되어 있으며, Windows는 압축 해제 후 `zellij.exe`를 실행한다. 이 문서의 셸 예시는 기본적으로 POSIX 계열 셸용이다.

## 2. 도움말부터 확인

```sh
zellij --version
zellij --help
zellij action --help
zellij action new-pane --help
zellij attach --help
zellij run --help
zellij subscribe --help
zellij web --help
zellij options --help
zellij setup --check
```

Zellij 바이너리 버전과 실행 중인 서버의 버전/기능은 같다고 가정하지 않는다. 온라인 가이드가 특정 설치 버전보다 앞설 수 있다. 특히 attach 초기 명령, 출력 조회/구독, 명시적 pane/tab ID, nested session 기능은 개별 도움말로 확인한다.

## 3. 기본 문법과 대상 세션

```sh
zellij                                      # 세션 시작
zellij --session agent-dashboard             # 이름을 정해 시작
zellij attach agent-dashboard                # 기존 세션에 연결
zellij attach --create agent-dashboard       # 없으면 생성하고 연결
zellij --session agent-dashboard action new-pane
```

- 기본 구조: `zellij [전역 옵션] [하위 명령] [그 명령의 옵션/인자]`.
- 세션 내부에서도 외부에서도 CLI로 세션을 제어할 수 있다.
- 자동화에서는 `--session <이름>`을 명시한다. 현재 포커스나 세션 추론에 의존하지 않는다.
- `--` 뒤의 문자열은 `run` 등에서 실행할 프로그램과 인자다. 셸 파이프/리다이렉션이 필요하면 `sh -c '…'`와 같이 셸을 명시한다.
- `run`은 새 command pane, `edit`은 편집기 pane, `action`은 세션의 동작, `pipe`는 플러그인 메시지, `subscribe`는 렌더된 화면 구독이다. 이들을 프로세스 stdout 스트리밍과 혼동하지 않는다.

## 4. 최상위 옵션

0.45.0 `CliArgs` 및 공식 Commands 가이드 기준. 전역 옵션은 하위 명령 앞에 두는 것이 명확하다.

| 옵션 | 기능 및 주의 |
| --- | --- |
| `-h`, `--help` | 도움말 |
| `-V`, `--version` | 버전 출력 |
| `-s`, `--session <NAME>` | 새 세션 이름 또는 제어 대상 세션 지정 |
| `-l`, `--layout <NAME_OR_PATH>` | 레이아웃 적용. 세션 내부 또는 `--session`을 쓰면 실행 중 세션에 탭 추가; 그 외에는 새 세션 |
| `--layout-string <KDL>` | 파일 대신 KDL 문자열. `--layout`, `--new-session-with-layout`과 충돌 |
| `-n`, `--new-session-with-layout <PATH>` | 레이아웃으로 새 세션 시작. 기존 세션에 탭을 추가하는 `--layout`과 구별 |
| `-c`, `--config <PATH>` | 설정 파일 경로. `ZELLIJ_CONFIG_FILE`과 연관 |
| `--config-dir <DIR>` | 설정 디렉터리. `ZELLIJ_CONFIG_DIR`과 연관 |
| `--data-dir <DIR>` | 플러그인 데이터 탐색 위치 변경 |
| `--max-panes <N>` | 화면 pane 수 상한. 상한을 넘어 새 pane을 만들면 오래된 pane이 닫힐 수 있음 |
| `-d`, `--debug` | 추가 디버그 정보 수집. 로그에 민감한 정보가 들어갈 수 있음 |

`--server <SOCKET_PATH>`는 0.45.0 코드에 숨김 내부 옵션으로 존재한다. 일반 사용/대시보드 통합의 지원 인터페이스로 사용하지 않는다. `--help`와 `--version`은 Clap이 생성하는 옵션이다.

```sh
zellij --layout compact
zellij --layout ./dashboard.kdl
zellij --layout-string 'layout { pane split_direction="vertical" { pane; pane; } }'
zellij --config ./config.kdl options --mouse-mode false
```

## 5. 최상위 명령과 별칭

| 명령 | 별칭 | 문서 |
| --- | --- | --- |
| `attach` | `a` | [세션](02-sessions.md) |
| `list-sessions` | `ls` | [세션](02-sessions.md) |
| `watch` | `w` | [세션](02-sessions.md) |
| `kill-session` | `k` | [세션](02-sessions.md) |
| `kill-all-sessions` | `ka` | [세션](02-sessions.md) |
| `delete-session` | `d` | [세션](02-sessions.md) |
| `delete-all-sessions` | `da` | [세션](02-sessions.md) |
| `action` | `ac` | [Action 전체](03-actions.md) |
| `run` | `r` | [Run/Edit](04-run-edit.md) |
| `edit` | `e` | [Run/Edit](04-run-edit.md) |
| `plugin` | `p` | [플러그인](08-plugins-pipes.md) |
| `pipe` | 코드상 별칭 없음 | [플러그인](08-plugins-pipes.md) |
| `list-aliases` | `la` | [플러그인](08-plugins-pipes.md) |
| `subscribe` | 코드상 별칭 없음 | [구독/제어](09-subscribe-programmatic-control.md) |
| `options` | 코드상 별칭 없음 | [설정](05-configuration.md) |
| `setup` | 코드상 별칭 없음 | 아래 |
| `web` | 코드상 별칭 없음 | [Web/원격](11-web-remote.md) |

`help`는 CLI 프레임워크의 도움말 기능이다. 예: `zellij help`, `zellij help action`.

## 6. setup 전체

| 옵션 | 용도 |
| --- | --- |
| `--check` | 설정 검사와 현재 사용 디렉터리 표시 |
| `--clean` | 기본 경로의 설정 파일을 읽지 않고 내장 기본 설정으로 시작 |
| `--dump-config` | 기본 설정을 stdout에 출력. 현재 사용자가 병합한 설정을 덤프한다는 뜻이 아님 |
| `--dump-layout <LAYOUT>` | 지정한 내장 레이아웃을 stdout에 출력 |
| `--dump-swap-layout <LAYOUT>` | 지정한 내장 swap layout을 stdout에 출력 |
| `--dump-plugins [DIR]` | 내장 플러그인을 디렉터리 또는 기본 data dir에 기록. 실제 파일 쓰기 동작 |
| `--generate-completion <SHELL>` | 셸 자동완성 출력 |
| `--generate-auto-start <SHELL>` | 셸용 자동 시작 스크립트 출력 |

```sh
zellij setup --check
zellij setup --dump-config                # 먼저 화면에서 확인
zellij setup --dump-layout default
zellij setup --generate-completion zsh
zellij setup --generate-auto-start zsh
```

설정 파일에 바로 `>`로 저장하면 기존 내용을 덮어쓴다. 기존 파일 확인/백업 후 저장하고 셸 설정에도 중복 삽입하지 않는다. completion과 auto-start의 설치 위치/연동은 [셸 통합](07-shell-integration.md)을 참고한다.

## 출처

- https://zellij.dev/documentation/installation.html
- https://zellij.dev/documentation/commands.html
- https://zellij.dev/documentation/controlling-zellij-through-cli.html
- https://docs.rs/zellij-utils/0.45.0/src/zellij_utils/cli.rs.html (CliArgs, Command, Sessions)
- https://docs.rs/zellij-utils/0.45.0/zellij_utils/setup/struct.Setup.html
