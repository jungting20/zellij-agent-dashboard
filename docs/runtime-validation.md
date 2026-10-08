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


## 2026년 10월 8일 공통 상태 신호와 화면 어댑터

`StateSignal`을 통한 코어 입력으로 리팩토링하고, 참고 저장소의 Claude/Codex/Gemini/Cursor 화면 규칙·매처·화면 영역을 이관했다. 원본 YAML과 내장 JSON의 내용이 같은 것을 확인했다. 저장소와 스냅샷은 스키마 2이며, 기존 이벤트 JSON은 버전 1을 유지한다. 출처 및 라이선스 확인은 [화면 어댑터 출처](screen-adapter-provenance.md)에 기록했다.

`./scripts/check.sh`에서 코어 21개와 호스트 28개, 총 49개 테스트, Rust 포맷, 네이티브/WASI Clippy와 셸/Python 구문 검사를 통과했다. `./scripts/build.sh`로 네이티브 host와 wasm32-wasip1 release 플러그인을 빌드했다. 테스트는 출처별 순서와 중복, 훅 전환 후 늦은 화면 결과, 시작 유예, 연속 idle 확인과 overlay/실패 시 취소, 실행 세대 교체, 오래된 프로세스 목록 무시, 스키마 1 이관, 저장 후 출처 복원, 입력을 훅 연결로 취급하지 않는 동작을 포함한다. host 대역으로 저장소 잠금 밖의 화면 조회, 조회 도중 훅 도착·프로세스 교체, 중복 collector의 읽기 억제, 수집 예약 만료 후 복구도 확인했다.

Zellij 0.45.0의 소유한 임시 세션 두 개에서 `python3 scripts/smoke.py`와 `python3 scripts/smoke.py --real-claude`를 통과했다. 결과는 각각 `.local/smoke-b912f2731b/result.json`과 `.local/smoke-5d10a182a5/result.json`에 남겼다. 확인 범위는 다음과 같다.

| 항목 | 확인한 동작 |
|---|---|
| 화면 경로 | 훅 없는 Codex fixture가 화면 경로를 선택하고 working/waiting/idle로 전환 |
| 화면 보존 | transcript overlay에서 working 유지, 미일치 화면은 서로 다른 관측으로 idle 확인 |
| 훅 경로 전환 | 공통 이벤트 ingest 후 hook으로 전환하고 이후 idle 화면이 working을 덮어쓰지 않음 |
| 실제 도구 훅 | 격리 설정의 실제 Claude SessionStart로 hook 출처 확인, 모델 요청 없음 |
| 생존·세대 | CLI로 종료한 pane의 오래된 identity를 resolve에서 거부 |
| 화면·collector 수명 | 대시보드 종료, 두 클라이언트, 모든 클라이언트 detach, 재연결과 reload 후 수집·저장 상태 복원 |
| 기존 조작 | pane 생성, preview, 한국어 여러 줄 입력, 요청 중복 억제, 종료와 세션 간 이동 |

reload 뒤 상태 파일에 hook과 screen 출처가 모두 보존되고 수집 예약이 해제된 것도 확인했다. 실제 Gemini/Cursor 및 Claude 전체 대화 흐름의 화면 감지를 확인한 것은 아니다. 이 프로필들은 내장 규칙의 정규식 컴파일과 대표 화면 단위 테스트로 검증했다. OSC progress는 Zellij 조회 계약이 제공하지 않아 사용하지 않는다. 이번 실제 Claude 화면은 2.1.292였으며 이후 로컬 `claude --version`은 2.1.293을 보고했다. 전체 훅 이벤트 지원 범위를 이 검증만으로 확장하지 않는다.

smoke는 명시적 세션 CLI 호출에서 호출자의 Zellij 환경 변수를 제거하고 fixture cwd도 임시 디렉터리로 지정한다. 초기 permission 자동 입력의 잔여 문자는 fixture 명령 전에 지운다. 세션 이동은 한 클라이언트에서 지시하고, 출발 세션의 클라이언트 감소와 대상 pane 초점을 확인한다. 서로 다른 세션에 같은 클라이언트 ID가 있으면 이동 뒤 대상 클라이언트 수가 늘지 않을 수 있다. 선택적 Claude 검사는 collector 수명 검증 뒤에 실행하여 도구 onboarding 화면이 lifecycle 자동화에 개입하지 않도록 했다.

## 2026-10-08 Repository 분리와 SQLite 전환

JSON 파일 저장 호출을 `Repository::read()`와 `Repository::begin()` / `UnitOfWork::commit()`으로 분리하고 `HostDependencies`로 주입했다. 저장 구현은 네이티브 SQLite 어댑터로 교체했다. 코어의 상태 전이와 플러그인의 JSON 계약은 유지하며 DB 구조는 `user_version=1`, 도메인·스냅샷 스키마는 2, 이벤트 JSON은 1이다. 전체 코어 스냅샷을 읽지만 저장은 변경된 레코드만 갱신한다.

`./scripts/check.sh`에서 코어 21개와 호스트 36개, 총 57개 테스트, Rust 포맷, 네이티브/WASI Clippy 및 셸/Python 구문 검사를 통과했다. `./scripts/build.sh`로 네이티브 host와 wasm32-wasip1 release 플러그인을 빌드했다. SQLite 라이브러리는 host에만 포함한다.

추가 검증은 메모리 Repository 주입, 동시 초기화·JSON 이관·작성자의 갱신 보존, 작성 중 조회의 커밋 상태 확인, 미커밋 롤백, 커밋 후 작성자 예약 해제, JSON 1→2 이관, 원본 JSON 보존과 재이관 방지, 손상·미래 버전 거부, 이관 실패 후 재시도, 변경 없는 행의 갱신 방지, 삭제 반영과 테이블 갱신 실패 시 전체 롤백, DB·WAL·SHM 권한을 포함한다. 초기 WAL 설정은 BUSY 핸들러 없이 잠금 오류가 날 수 있어 초기화 단계에만 제한된 재시도를 적용했다.

Zellij 0.45.0의 소유한 임시 세션 두 개에서 `python3 scripts/smoke.py --real-claude`를 통과했다. 결과는 `.local/smoke-c29d923c5f/result.json`에 남겼고 검사 종료 후 임시 세션을 종료했다. JSON 이관 원본의 바이트 보존, 두 세션·멀티 클라이언트 수집, 화면 판별과 훅 전환, 중복 입력·종료·별칭 요청, 세션 이동, 전체 detach 후 수집, 재연결·collector reload, 고정·별칭·요청 결과 복원, 실제 Claude SessionStart 훅, SQLite `integrity_check=ok`, WAL과 DB 구조 버전을 확인했다. Claude에는 프롬프트를 제출하거나 모델 요청을 하지 않았다. 운영 세션에 설치나 reload를 적용하지 않았다.

통합 검증에서 고정을 검색 검사보다 먼저 수행하면 대상 행이 다른 패널로 이동하므로 고정 검사를 세션 이동 뒤에 수행하도록 조정했다. 재연결 직후에는 백그라운드 collector 응답만으로 클라이언트 등록을 판단하지 않고 `list-clients`로 실제 등록을 확인한 뒤 reload한다. 이 대기 추가 전에는 reload 직후 pipe 응답이 시간 초과됐으며, 추가 후 전체 검증이 통과했다.
