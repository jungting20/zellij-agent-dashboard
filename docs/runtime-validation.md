# 실행 검증 기록

2026년 10월 7일 macOS에서 Zellij 0.45.0, Rust 1.88.0, Claude Code 2.1.70으로 첫 조회 버전을 검증했다. 임시 세션을 사용했으며 기존 사용자 세션의 pane이나 플러그인을 교체하지 않았다.

## 자동 검사

`./scripts/check.sh`로 Rust 포맷, 단위 테스트 17개, 네이티브와 wasm32-wasip1 Clippy, 셸 문법과 Python 구문을 확인했다. `./scripts/build.sh`는 호스트 실행 파일과 실제 `_start`를 갖는 WASI 플러그인을 빌드한다.

단위 테스트는 이벤트 중복·역순, pane 실행 세대 교체, SessionEnd 종료 경쟁, 캐시 복원과 생존 검증, 늦은 도구 결과, 동시 저장소 작성, 손상 파일 보존, 검색·정렬 중 선택 유지, 한국어 폭과 스크롤을 포함한다.

## 실제 Zellij 검증

`python3 scripts/smoke.py --real-claude`는 다음을 확인한다.

| 항목 | 확인한 동작 |
|---|---|
| 백그라운드 로딩 | 동일 WASM의 collector와 dashboard 설정 구분, 최초 권한 승인 |
| 화면 종료 | dashboard pane 종료 이후 collector 타이머 계속 증가 |
| 다중 클라이언트 | 두 클라이언트 연결 중 공유 상태 조회와 수집 |
| 여러 세션 | 공백 있는 이름을 포함한 두 세션의 실행 파일 발견 |
| 실제 화면 입력 | `/` 검색과 선택한 실행 ID 확인 |
| 세션 이동 | Enter 이후 다른 세션의 대상 terminal pane으로 클라이언트 이동 |
| 중복 이벤트 | 같은 공통 JSON 이벤트를 두 번 넣어 두 번째 반영 거부 |
| 오래된 대상 | pane 종료 후 이전 실행 ID의 resolve 거부 |
| 실제 Claude | 격리한 설정에서 SessionStart 훅으로 idle 및 보고 시각 저장 |
| detach | 모든 터미널 클라이언트 종료 후 collector 타이머 계속 증가 |
| 재연결·reload | 클라이언트 재연결과 collector reload 후 저장 revision 복원 |
| 실행 스크립트 | 지정한 임시 세션에 floating dashboard 로드 |

프로세스 발견용 `codex`는 입력을 로컬 파일에 기록하며 대기하는 Rust fixture이며 실제 Codex 이벤트 검증을 대신하지 않는다. 중복 검증에 쓰는 TurnStarted는 명시적으로 넣은 공통 이벤트다. Claude에는 프롬프트를 제출하지 않으며 모델 요청을 하지 않는다. Claude 설정·신뢰 기록과 상태 파일은 `.local/smoke-*` 아래에 격리한다.

## 확인한 제약과 남은 검증

Zellij 0.45.0은 연결된 클라이언트가 없으면 plugin reload를 거부한다. detach 이후 기존 collector의 동작은 유지되지만 reload는 클라이언트를 다시 연결한 상태에서 수행해야 한다.

Claude의 실제 이벤트 검증 범위는 SessionStart다. 작업·도구·승인 대기·완료·압축·실패 이벤트를 포함한 전체 에이전트 턴 검증, 명시적 권한 거절 UI, 다른 도구의 상세 어댑터, 한글 이름의 임시 세션과 운영체제별 프로세스 탐지는 추가 검증 대상으로 남긴다. 현재 실행 중인 한글 세션의 도구는 읽기 전용 스캔에서 발견했고 해당 세션에는 조작을 하지 않았다.

초기 조회 버전 검증 당시 입력 전송·종료·새 실행과 해당 조작의 요청 중복 배제는 구현 전이었다. 이후 호스트 인터페이스 추출 검증 결과는 아래에 기록한다. 여러 호스트 명령의 파일 쓰기 직렬화 검증을 외부 조작의 단일 실행 검증으로 해석하지 않는다. 세션·프로젝트 그룹 영역, 부모·자식 관계 수집과 고정·별칭 편집도 후속 단계다.

실행 결과는 각 임시 디렉터리의 `result.json`에 남는다. 실패 시에는 terminal.log를 보존하며 검사 종료 후 해당 임시 세션을 종료한다.

## 2026-10-08 호스트 인터페이스 추출 검증

`TerminalHost`와 `CommandRunner` 주입 이후 `./scripts/check.sh`와 `./scripts/build.sh`를 통과했다. 코어 15개와 호스트 19개, 총 34개 단위 테스트와 네이티브/WASI Clippy, Rust 포맷, 셸/Python 구문, WASI release 빌드를 확인했다. 기존 메뉴 편집 코드의 Clippy 경고는 `.last()`를 `.next_back()`으로 바꿔 동작을 유지하며 해소했다.

추가 단위 테스트는 CLI 실행 파일 경로 주입, argv의 한국어·공백·여러 줄·셸 특수 문자 보존, terminal/plugin ID 분리, pane 메타데이터·생성 ID 파싱, malformed JSON과 실행 오류 전달을 검증한다. 실제 명령 실행기는 양쪽 출력과 종료 코드, 실행 실패, timeout, 출력 초과, cwd·환경 변수 설정/제거, 출력 폐기를 검증한다. 호스트 대역은 중복 입력의 paste/Enter 재전송 방지, 입력 도중 프로세스 세대 교체 시 Enter 차단과 `Uncertain` 유지, 고정 에이전트 종료 차단을 검증한다.

`python3 scripts/smoke.py --real-claude`를 소유한 임시 세션 두 개에서 실행했다. 기존 조회·멀티 클라이언트·다른 세션으로 이동·오래된 대상 거부·detach·재연결·reload·launcher 검증을 모두 통과했다. 추가로 호스트 action을 통한 pane 생성과 동일 요청의 결과 재사용, 생성된 프로세스 발견, 화면 조회, 한국어 여러 줄 입력과 중복 입력 억제, pane 종료와 종료 요청 재사용을 확인했다. Claude SessionStart 훅은 주입된 변경 통지 경로를 사용했으며 모델 요청은 하지 않았다. 통지는 최선의 전달이고 실패를 무시하므로 이 결과를 통지 유실 없는 전달 보장으로 해석하지 않는다.

검증 결과는 `.local/smoke-0c888d8498/result.json`에 남겼으며 검사 종료 후 임시 세션을 종료했다. 상태 스키마는 기존 1을 유지했다. 현재 운영 세션에 설치나 reload를 적용하지 않았다.

첫 추가 검증에서는 테스트 호출자가 상속한 `ZELLIJ_PANE_ID`가 다른 세션의 pane을 가리켜 `--no-focus`로 생성한 pane의 화면이 비었다. Zellij CLI 직접 호출에서도 재현했고, 테스트 action 실행 환경에서 `ZELLIJ`, `ZELLIJ_SESSION_NAME`, `ZELLIJ_PANE_ID`를 제거한 뒤 통과했다. 이 리팩토링은 CLI의 원래 환경 상속 동작을 변경하지 않는다. 다른 세션으로 no-focus 실행할 때 호출 pane 문맥을 어떻게 전달할지는 별도의 호스트 동작 개선 대상이다.
