# Zellij Agent Dashboard

별도 에이전트 관리 데몬 없이 Zellij 플러그인으로 에이전트 상태를 확인하고 pane을 조작하는 프로젝트다. `/Users/in05908_mac/zellij-with-codeagent`의 `agent-dashboard`를 먼저 이관하며, `/Users/in05908_mac/study/zj-agent-mob`의 훅과 상태 파일 구조를 참고한다.

조회·기본 조작과 보조 메뉴를 구현했다. Rust 상태 코어, 유한한 호스트 보조 명령, 백그라운드 수집기, 대시보드 화면과 Claude 훅 설정 생성기를 포함한다. 기능별 구현·검증 범위와 남은 작업은 [기능 이관표](docs/feature-map.md#현재-구현과-검증)에 정리한다.

## 실행

Zellij `0.45.0`, rustup의 Rust `1.88.0`과 `wasm32-wasip1` 타깃이 필요하다. 현재 호스트 어댑터의 실행 검증 환경은 macOS다.

```sh
rustup toolchain install 1.88.0 --profile minimal --component rustfmt,clippy
rustup target add wasm32-wasip1 --toolchain 1.88.0
./scripts/build.sh
./scripts/dashboard.sh SESSION_NAME
```

클라이언트가 연결된 실행 중인 세션을 지정한다. Zellij 안에서는 세션 이름을 생략할 수 있다. 처음 로드할 때 표시되는 플러그인 권한을 승인하면 수집을 시작한다. 수집기는 화면 pane을 닫아도 남는다. 스크립트는 지정한 세션에만 수집기와 화면을 로드하며 사용자 Zellij 설정을 수정하지 않는다.

목록은 현재 pane이 확인된 실행만 표시하며 종료 기록과 Codex app-server는 제외한다. 상태는 훅이 연결되지 않은 실행에서 화면으로 판별한다. Claude 훅을 연결하면 해당 실행은 훅 경로로 전환한다. 별도 설정 파일을 생성해 Zellij pane 안의 Claude 실행에 적용한다.

```sh
mkdir -p .local
./dist/dashboard-host hook-config > .local/claude-dashboard.json
claude --settings "$PWD/.local/claude-dashboard.json"
```

훅 생성과 로드는 자동으로 전역 Claude 설정을 수정하지 않는다. `ZAD_STATE_DIR`로 다른 저장 위치를 사용하면 생성기에도 같은 절대 경로를 전달한다.

```sh
export ZAD_STATE_DIR="$PWD/.local/state"
./dist/dashboard-host --state-dir "$ZAD_STATE_DIR" hook-config > .local/claude-dashboard.json
./scripts/dashboard.sh SESSION_NAME
```

상태는 기본적으로 `${XDG_STATE_HOME:-$HOME/.local/state}/zellij-agent-dashboard/store.sqlite3`에 저장한다. Repository 계층을 통해 SQLite를 사용하며 네이티브 host에 SQLite 라이브러리를 포함하므로 별도 DB 서버나 SQLite CLI 설치는 필요하지 않다. 기존 `store.json`은 최초 실행에 자동 이관하고 원본을 보존한다. 업그레이드 전에 이전 host를 사용하는 collector와 훅을 중단한다. 백업·복구와 버전 정책은 [공유 상태 문서](docs/architecture.md#공유-상태와-동시성)를 참고한다.

화면 키는 `j/k`, 방향키, 숫자 `1–9` 선택, `/` 검색, `R` 새로고침, `Enter` pane 이동, `q` 닫기다. `h`/`←`는 고정 영역, `l`/`→`는 일반 영역으로 이동한다. `Tab`은 검색 결과의 양쪽 영역에서 살아 있는 `working` 에이전트만 순환 선택한다. 마지막 대상 다음에는 처음으로 돌아가며 대상이 없으면 선택을 유지한다. `Space` 고정 변경, `a` 별칭, `i` 입력, `d` 종료, `n` 새 실행, `p` 마지막 지시, `I` 외부 에디터, `g` worktree 메뉴, `m` 병합 지시, `u` 최근 조작 결과를 제공한다. 보조 메뉴 전체 흐름의 실제 검증은 남아 있다.

## 전역 agent next 키

`Alt q`로 대시보드를 열려면 `shared_except "locked"`에 다음 키를 추가한다. 아래 collector 설정과 같은 경로를 사용한다. 수집기가 유한한 host 명령으로 창을 처음부터 너비·높이 90%로 생성하므로 실행용 terminal pane과 크기 변경 단계가 없다. 이미 열린 화면은 재사용한다. 여러 클라이언트에서는 agent next와 동일하게 연결된 최소 ID 클라이언트가 처리한다.

```kdl
bind "Alt q" {
    MessagePlugin "agent-dashboard" {
        mode "collector"
        name "agent-dashboard-open"
    }
    SwitchToMode "normal";
}
```

CLI에서는 `bash scripts/open-dashboard.sh SESSION_NAME`으로 같은 동작을 요청한다. host와 WASM은 같은 `dist` 디렉터리에 둔다. 실행 중인 collector에는 새 메시지 처리를 위해 대상을 지정한 reload가 필요하다.

별도 bridge 없이 같은 `agent-dashboard.wasm`의 collector가 `agent-next` 메시지를 처리한다. 대시보드 화면이 닫혀 있어도 사용할 수 있다. 연결된 클라이언트 중 가장 작은 ID의 클라이언트가 처리하며 그 클라이언트의 현재 세션·pane을 기준으로 대시보드 표시 순서의 다음 실행으로 이동하며 마지막 다음에는 처음으로 돌아간다. 종료·미확인 실행과 부재 pane은 제외하고 이동 직전 실행 세대를 재검증한다. 고정 필터는 화면과 같은 부모 고정 상속을 적용하며 idle 필터는 정확히 `idle` 상태만 포함한다. 해당 대상이 없으면 이동하지 않는다.

각 컴퓨터에서 `bash scripts/build.sh && bash scripts/install.sh`를 실행하면 WASM과 해당 컴퓨터용 host를 `~/.config/zellij/plugins/`에 함께 설치한다. Zellij `config.kdl`의 기존 `plugins` 블록에 아래 별칭을 추가한다.

```kdl
plugins {
    agent-dashboard location="file:~/.config/zellij/plugins/agent-dashboard.wasm"
}
```

`host_path`를 생략하면 `~/.config/zellij/plugins/dashboard-host`를 사용하고, `state_dir`를 생략하면 host의 HOME/XDG 기본 상태 경로를 사용한다. 명시할 때는 절대 경로나 `~/` 경로를 지원한다. 설정과 payload는 셸의 위치 인자로 전달한다. 별칭 변경은 Zellij 세션 재시작 후 반영된다. `load_plugins`와 키의 설정은 동일하게 유지하며 `scripts/dashboard.sh`에서도 같은 WASM과 상태 경로를 사용한다. 기존 `agent-next-bridge.wasm`, `executable_path`, `bridge_revision` 설정을 대체한다.

```kdl
keybinds {
    shared_except "locked" {
        bind "Alt u" {
            MessagePlugin "agent-dashboard" {
                mode "collector"
                name "agent-next"
                payload "pinned-only"
            }
        }
    }
}
load_plugins {
    "agent-dashboard" {
        mode "collector"
    }
}
```

같은 `MessagePlugin` 설정에 아래 키와 payload를 적용한다.

| 키 | payload | 대상 |
|---|---|---|
| `Alt u` | `pinned-only` | 고정 |
| `Alt i` | `idle-and-pinned` | idle이면서 고정 |
| `Alt o` | `unpinned-only` | 일반 |
| `Alt p` | `idle-and-unpinned` | idle이면서 일반 |

`all`과 `working-only` payload도 지원한다. 대상 지정 없이 브로드캐스트한 `agent-next`는 무시하고 collector를 지정한 private 메시지만 처리한다. 화면 인스턴스는 이동 요청을 처리하지 않는다. Zellij 0.45.0의 키 메시지는 여러 클라이언트 인스턴스로 전달되므로 연결된 최소 ID 클라이언트만 처리하고 다른 인스턴스는 요청을 버린다. 다중 클라이언트에서는 처리 클라이언트의 초점만 이동하며, 키를 누른 클라이언트 식별자는 메시지 계약에 없다. 연속 키는 순서대로 처리한다. 설정 변경은 새 세션부터 적용하며 실행 중 세션에 설치·reload할 때는 대상을 명시한다.

Claude, Codex, Cursor CLI(`agent`), Gemini(`agy`/`gemini`), Hermes와 Pi(`pi` 또는 Node의 `pi-coding-agent`) 발견 코드를 제공한다. 도구별 실제 검증 범위는 기능 이관표를 따른다. 상세 훅 어댑터는 현재 Claude만 제공한다. Claude, Codex, Gemini, Cursor는 훅이 없으면 화면 규칙으로 상태를 판별한다. Hermes와 Pi는 화면 규칙이 없어 `found`로 표시한다. 훅 없는 경로 정보는 프로세스가 상속한 `PWD`나 확인한 pane 메타데이터를 사용한다.

## 검증

```sh
./scripts/check.sh
python3 scripts/smoke.py
python3 scripts/smoke.py --portable-paths
python3 scripts/smoke.py --real-claude
```

smoke 검증은 이름이 무작위인 전용 임시 세션만 만들고 종료한다. `--portable-paths`는 플러그인 별칭과 `~/` host·상태 경로로 같은 검증을 실행한다. `--real-claude`는 격리한 설정으로 Claude를 실행하고 모델 요청 없이 SessionStart 훅을 확인한다. 테스트용 `codex` 실행 파일은 프로세스 발견용 fixture다. 상세 범위와 결과는 [실행 검증 기록](docs/runtime-validation.md)에 기록한다.

## 현재 구현 범위

- 백그라운드 수집기와 화면용 대시보드를 분리한다.
- 실행별로 훅 또는 화면 중 하나로 상태를 관측하고 프로세스 스캔으로 생존 여부를 확인한다.
- 여러 Zellij 세션의 상태, 작업 요약, 경과 시간, 마지막 보고 시점을 표시한다.
- 세션·탭 그룹, 고정/일반 영역, launch ID로 연결한 부모·자식 계층과 활동 기록을 표시한다.
- 목록 이동, 검색, 새로고침, 선택한 pane으로 이동을 제공한다.
- 고정·별칭 저장, 입력 전송, 종료, 새 에이전트 실행과 요청 결과 저장을 제공한다.
- 마지막 지시·출력 preview·에디터·최근 경로·worktree 등의 보조 메뉴를 제공한다. 실제 검증 범위는 기능별로 구분한다.

티켓 큐, 후속 지시 스케줄러, 전체 RuntimeService, 기존 HTTP API의 이관은 후속 작업으로 남긴다. 기존 대시보드의 관련 메뉴도 기능 이관표에서 추적한다.

## 문서

- [구현 계획](docs/implementation-plan.md): 단계별 작업과 완료 조건
- [아키텍처](docs/architecture.md): 수집기, 화면, 상태 저장, 메시지 계약
- [기능 이관표](docs/feature-map.md): 기존 기능과 이번 구현 범위

## 기술 방향

Rust와 Zellij `0.45.0`에 맞춘 `zellij-tile`로 WASI 실행 파일을 만든다. 하나의 플러그인 바이너리가 설정에 따라 `collector`와 `dashboard` 역할로 실행된다.

Zellij 서버가 수집기의 실행 기반이다. 수집기는 2초마다 유한한 호스트 명령을 실행하고 공유 SQLite 저장소를 갱신한다. 여러 수집기의 쓰기는 SQLite 트랜잭션으로 직렬화하고 프로세스 스캔은 공유 주기로 제한한다. 모든 수집기 세션이 종료되면 수집도 중단되고, 다음 실행에서 저장 상태와 실제 프로세스를 대조해 복원한다.

프로세스 탐지와 파일 갱신에 호스트 프로그램이 필요하면 요청 하나를 처리하고 종료하는 보조 명령을 사용한다. 기존 `agentd` 또는 `zellij-agent daemon serve`에 연결하지 않는다.

상태 변경은 코어 메서드로 모았으며, 용도별 Repository 조회와 작업별 트랜잭션의 변경분 저장을 적용했다. DB 구조 버전 1은 2로 자동 이관한다. 이관 전 백업과 롤백 방법은 공유 상태 문서를 따른다. 프로젝트별 독립 그룹과 보조 메뉴 전체 실동작 검증 등의 남은 기능 작업은 [계획](docs/implementation-plan.md)과 기능 이관표에서 추적한다.
