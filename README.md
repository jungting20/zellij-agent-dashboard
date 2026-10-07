# Zellij Agent Dashboard

별도 에이전트 관리 데몬 없이 Zellij 플러그인으로 에이전트 상태를 확인하고 pane을 조작하는 프로젝트다. `/Users/in05908_mac/zellij-with-codeagent`의 `agent-dashboard`를 먼저 이관하며, `/Users/in05908_mac/study/zj-agent-mob`의 훅과 상태 파일 구조를 참고한다.

현재는 프로젝트와 구현 계획을 초기화한 상태다. 실행 가능한 플러그인과 훅 설치 프로그램은 다음 단계에서 만든다.

## 첫 구현 범위

- 백그라운드 수집기와 화면용 대시보드를 분리한다.
- 에이전트 훅을 우선 사용하고 프로세스 스캔으로 실행 여부를 보완한다.
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

Rust와 `zellij-tile`로 WASI 실행 파일을 만든다. 하나의 플러그인 바이너리가 설정에 따라 `collector`와 `dashboard` 역할로 실행되는 구조를 우선 검증한다. Zellij는 `load_plugins`를 통한 백그라운드 로딩을 지원한다. [Zellij 플러그인 로딩 문서](https://zellij.dev/documentation/plugin-loading)

Zellij 서버가 수집기의 실행 기반이다. 화면 pane을 닫아도 수집기가 남는 동작은 첫 기술 검증에서 확인한다. 모든 Zellij 세션이 종료되면 수집도 중단되고, 다음 실행에서 저장 상태와 실제 프로세스를 대조해 복원한다.

프로세스 탐지와 파일 갱신에 호스트 프로그램이 필요하면 요청 하나를 처리하고 종료하는 보조 명령을 사용한다. 기존 `agentd` 또는 `zellij-agent daemon serve`에 연결하지 않는다.

## 개발 시작점

[구현 계획의 M0](docs/implementation-plan.md#m0-zellij-실행-조건-검증)부터 진행한다. 현지 확인 환경은 Zellij `0.45.0`과 Rust `1.88.0`이다. 현재 `wasm32-wasip1` 타깃은 설치되어 있지 않아 WASM 빌드 단계에서 추가해야 한다. `zellij-tile` 버전과 최소 지원 Zellij 버전은 M0 결과로 확정한다.
