# Zellij Agent Dashboard

별도 에이전트 관리 데몬 없이 Zellij 플러그인으로 에이전트 상태를 확인하고 pane을 조작하는 프로젝트다. `/Users/in05908_mac/zellij-with-codeagent`의 `agent-dashboard`를 먼저 이관하며, `/Users/in05908_mac/study/zj-agent-mob`의 훅과 상태 파일 구조를 참고한다.

첫 조회 버전을 구현했다. Rust 상태 코어, 유한한 호스트 보조 명령, 백그라운드 수집기, 대시보드 화면과 Claude 훅 설정 생성기를 포함한다. 기존 대시보드의 모든 기능을 이관한 상태는 아니다.

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

화면 키는 `j/k`, 방향키, 숫자 `1–9` 선택, `/` 검색, `R` 새로고침, `Enter` pane 이동, `q` 닫기다. `Tab`은 저장된 고정 항목만 필터링한다. 고정 변경과 별칭 편집은 다음 구현 단계다.

Claude, Codex, Cursor CLI(`agent`), Gemini 프로필(`agy`), Hermes 실행 파일을 발견한다. 상세 훅 어댑터는 현재 Claude만 제공한다. Claude, Codex, Gemini, Cursor는 훅이 없으면 화면 규칙으로 상태를 판별한다. Hermes는 화면 규칙이 없어 `found`로 표시하며 Pi 탐지는 아직 추가하지 않았다. 훅 없는 경로 정보는 프로세스가 상속한 `PWD`를 사용한다.

## 검증

```sh
./scripts/check.sh
python3 scripts/smoke.py
python3 scripts/smoke.py --real-claude
```

smoke 검증은 이름이 무작위인 전용 임시 세션만 만들고 종료한다. `--real-claude`는 격리한 설정으로 Claude를 실행하고 모델 요청 없이 SessionStart 훅을 확인한다. 테스트용 `codex` 실행 파일은 프로세스 발견용 fixture다. 상세 범위와 결과는 [실행 검증 기록](docs/runtime-validation.md)에 기록한다.

## 첫 구현 범위

- 백그라운드 수집기와 화면용 대시보드를 분리한다.
- 실행별로 훅 또는 화면 중 하나로 상태를 관측하고 프로세스 스캔으로 생존 여부를 확인한다.
- 여러 Zellij 세션의 상태, 작업 요약, 경과 시간, 마지막 보고 시점을 표시한다.
- 기존 대시보드의 고정 영역, 부모와 자식 관계, 활동 기록을 단계적으로 이관한다.
- 목록 이동, 검색, 새로고침, 선택한 pane으로 이동을 먼저 구현한다.
- 이어서 고정과 별칭 저장, 입력 전송, 종료, 새 에이전트 실행을 구현한다.

티켓 큐, 후속 지시 스케줄러, 전체 RuntimeService, 기존 HTTP API의 이관은 후속 작업으로 남긴다. 기존 대시보드의 관련 메뉴도 기능 이관표에서 추적한다.

## 문서

- [구현 계획](docs/implementation-plan.md): 단계별 작업과 완료 조건
- [아키텍처](docs/architecture.md): 수집기, 화면, 상태 저장, 메시지 계약
- [기능 이관표](docs/feature-map.md): 기존 기능과 이번 구현 범위

## 기술 방향

Rust와 Zellij `0.45.0`에 맞춘 `zellij-tile`로 WASI 실행 파일을 만든다. 하나의 플러그인 바이너리가 설정에 따라 `collector`와 `dashboard` 역할로 실행된다.

Zellij 서버가 수집기의 실행 기반이다. 수집기는 2초마다 유한한 호스트 명령을 실행하고 공유 SQLite 저장소를 갱신한다. 여러 수집기의 쓰기는 SQLite 트랜잭션으로 직렬화하고 프로세스 스캔은 공유 주기로 제한한다. 모든 수집기 세션이 종료되면 수집도 중단되고, 다음 실행에서 저장 상태와 실제 프로세스를 대조해 복원한다.

프로세스 탐지와 파일 갱신에 호스트 프로그램이 필요하면 요청 하나를 처리하고 종료하는 보조 명령을 사용한다. 기존 `agentd` 또는 `zellij-agent daemon serve`에 연결하지 않는다.

다음 구현은 [계획](docs/implementation-plan.md)에 남겨 둔 그룹·계층 표시, 고정과 별칭 저장, 입력·종료·실행 조작이다.
