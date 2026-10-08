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

## 2026-10-08 이동 가능한 에이전트 목록과 pane 확인

기본 목록은 종료되지 않았고 `gone`이 아니며 실제 terminal pane이 확인된 실행만 표시한다. 종료 기록은 24시간 보관하되 목록에서 숨긴다. Codex `app-server`의 native/Node 런처를 발견 대상에서 제외한다. pane 존재 여부는 `PanePresence`와 관측 시각으로 저장하며 프로세스 생존이나 작업 상태를 바꾸지 않는다. 조회 실패는 이전 pane 확인 결과를 유지하고, 실행 세대가 교체되거나 더 최신 pane 결과가 저장되면 늦은 결과를 거부한다. resolve·입력·종료 대상 검증과 preview는 pane 목록을 다시 확인한다.

Store/스냅샷 스키마는 3으로 올렸다. 스키마 1/2는 자동 이관하며 기존 pane 제목으로 존재를 추정하지 않고 `unknown`, 관측 시각 0으로 초기화한다. SQLite DB 구조 버전 1과 이벤트 JSON 버전 1은 유지한다. 상태 백업과 이전 host/WASM은 `.local/backup-before-pane-presence-66b3d2a4de/`에 보존했다.

`./scripts/check.sh`에서 코어 24개와 호스트 40개, 총 64개 테스트, 포맷, 네이티브/WASI Clippy와 셸/Python 구문 검사를 통과했다. `./scripts/build.sh`로 네이티브 host와 wasm32-wasip1 release 플러그인을 빌드했다. 추가 테스트는 app-server 오탐, 종료·부재·미확인 pane의 목록 제외, 숨긴 부모의 고정 상속 방지, pane 부재에도 작업 상태 보존, 조회 실패 시 기존 pane 확인 유지, pane 조회의 역순·실행 세대 교체, 스키마 2 이관, 프로세스가 살아 있어도 부재 pane에 대한 이동·입력 차단을 포함한다.

`python3 scripts/smoke.py --real-claude`를 임시 Zellij 세션 두 개에서 통과했다. 결과는 `.local/smoke-04fb77e98d/result.json`이며 세션과 검증용 백그라운드 프로세스는 종료했다. pane ID 999999를 상속한 살아 있는 Codex fixture를 발견하되 pane을 `missing`으로 확인하고 resolve를 거절했다. `app-server` fixture는 발견되지 않았고, 테스트 세션으로 검색한 대시보드에서는 종료·부재 pane을 선택할 수 없었다. 입력·종료·중복 요청·세션 이동·detach·reload·SQLite 보존과 실제 Claude SessionStart 훅도 통과했다. 전체 세션을 수집하는 특성을 고려해 목록 제외 검증은 테스트 세션으로 검색 범위를 제한한다.

사용자가 테스트하던 `study` 세션의 collector를 reload하고 대시보드 `plugin_11`을 열었다. 다른 운영 세션에는 설치·reload를 적용하지 않았다.

## 2026-10-08 코어 상태 변경 경계 정리

`Store`는 읽기 전용 `StoreData` 접근과 명시적인 복원 경계를 제공하며, 호스트의 직접적인 도메인 필드 변경을 코어 메서드로 옮겼다. 별칭·종료 보호와 결과·부모 연결·요청 결과·pane/cwd·화면 시도 시각·수집 예약을 코어에서 처리한다. 종료 보호는 요청 접수와 종료 직전에 같은 함수를 사용한다. 늦은 화면 시도 기록은 훅 전환·실행 세대 교체 뒤 반영하지 않는다. 예약 해제는 토큰을 확인하며 만료된 자기 예약도 정리할 수 있다. 외부 JSON과 DB 구조 버전은 유지한다.

`./scripts/check.sh`에서 코어 27개와 호스트 40개, 총 67개 테스트와 포맷·네이티브/WASI Clippy·스크립트 구문 검사를 통과했다. `./scripts/build.sh`로 host와 wasm32-wasip1 release 플러그인을 빌드했다. 추가 테스트는 별칭의 세대 검증·멱등성, 공통 종료 보호, launch 연결의 세대 거부·멱등성, 요청 결과의 재완료 거부와 경로 보존, 교체된 예약 보호, 역순·훅 전환 뒤 화면 시도 거부를 포함한다.

`python3 scripts/smoke.py`를 임시 세션 두 개에서 통과했다. 결과는 `.local/smoke-71c2c4ee73/result.json`이다. 수집기 수명·멀티 클라이언트·화면 상태·훅 출처 전환·호스트 입력/종료/실행·중복 요청·고정/별칭 보존·세션 이동·detach·reload·SQLite 무결성을 확인했다. 실제 Claude 옵션은 이 단계에서 실행하지 않았다. 테스트가 소유한 세션과 프로세스는 정리했으며 운영 세션에 설치·reload를 적용하지 않았다.

## 2026-10-08 용도별 Repository 조회와 요청 인덱스

스냅샷·단일 에이전트·단일 요청·최근 요청·실행 메뉴·runtime 조회를 분리했다. 스냅샷과 runtime은 요청 이력을 읽지 않으며 최근 요청은 `at_ms DESC, id ASC` 인덱스와 LIMIT으로 조회한다. 갱신은 이 단계에서 전체 Store 비교를 유지해 부분 조회 결과의 누락을 삭제로 해석하지 않도록 했다.

DB 구조 버전은 2이며 버전 1의 요청 JSON에서 시각을 채우고 인덱스를 추가한다. 도메인·스냅샷은 3, 이벤트 JSON은 1을 유지한다. 이관 실패는 컬럼·인덱스·버전까지 롤백하고 원본 JSON을 보존한다. `./scripts/check.sh`에서 코어 27개·호스트 44개, 총 71개 테스트와 포맷·네이티브/WASI Clippy·구문 검사를 통과했다. 추가 검증은 버전 1 이관과 실패 후 재시도, 인덱스 사용, 관련 없는 손상 payload의 조회 제외, 커밋된 스냅샷, 최신 요청 개수 제한과 동일 시각의 순서를 포함한다. 미래 도메인 스키마가 용도별 조회에서도 거부되는 테스트를 추가 확인했다. `./scripts/build.sh`로 release host/WASM을 빌드했다.

`python3 scripts/smoke.py`를 임시 세션 두 개에서 통과했다. 결과는 `.local/smoke-8891ad352e/result.json`이며 기존 수집·상태·조작·중복·이동·detach·reload 시나리오와 DB 버전 2·WAL·무결성을 확인했다. 실제 Claude 옵션은 다음 통합 검증에서 실행한다. 운영 세션에 설치·reload를 적용하지 않았다.

프로세스 시작을 포함한 CLI 스냅샷 조회를 12회씩 측정했다. 에이전트 없는 상태에서 요청 0개/4,000개(각 메시지 4 KiB)의 중앙값은 이전 host 3.603/16.305ms, 조회 분리 host 3.752/4.284ms였다. 로컬 측정으로 절대 성능 보장을 의미하지 않는다. 결과는 `.local/repository-benchmark-30e95cc48d/result.json`이다.

## 2026-10-08 작업별 트랜잭션과 변경분 저장

코어가 변경된 agent/request/launch ID, 삭제된 agent ID와 메타데이터·활동·최근 경로 변경을 추적한다. SQLite는 이 변경분만 직렬화·저장하며 트랜잭션 시작/커밋의 전체 Store 복제와 전체 JSON 비교를 제거했다. 활동·최근 경로는 각각 최대 50개·100개인 목록 단위로 교체한다. 메타데이터는 컬렉션을 복제하지 않고 별도로 구성한다. 변경 추적과 부분 조회의 요청 총개수는 저장 JSON에 포함하지 않는다.

수집·훅·대상 확인은 runtime 트랜잭션으로 요청 이력을 읽지 않는다. 요청 접수는 runtime 상태·해당 요청과 SQL 전체 개수를 읽어 4,096개 제한 및 중복을 함께 검사한다. 결과·launch 기록은 해당 요청과 launch·최근 경로를 읽는다. 도메인 이관이 필요한 요청 트랜잭션은 에이전트 기본값도 함께 이관한다. 전체 복원은 명시적인 full 트랜잭션에서만 허용하며 부분 조회의 누락을 삭제로 해석하지 않는다. 명시적으로 정리한 agent ID만 삭제한다.

`./scripts/check.sh`에서 코어 27개·호스트 48개, 총 75개 테스트를 통과했으며 별도의 수동 성능 측정 테스트 1개는 기본 검사에서 제외한다. 포맷·네이티브/WASI Clippy·스크립트 구문 검사와 `./scripts/build.sh` release host/WASM 빌드도 통과했다. 검증은 부분 조회의 관련 없는 손상 payload 제외와 행 보존, 범위 밖 요청 변경·부분 트랜잭션의 전체 복원 거부, 전체 요청 한도와 기존 요청 재사용, 동시 8개 claim 중 1개만 접수, action/request 범위의 도메인 이관, 명시적 agent 삭제, 결과 저장 실패 시 메타데이터와 pending 결과 보존을 포함한다. 코어 종료 보호는 부모 고정에 대해서도 접수와 종료 직전의 동일한 거부를 확인했다.

`python3 scripts/smoke.py --real-claude`를 임시 세션 두 개에서 통과했다. 결과는 `.local/smoke-04ab713396/result.json`이다. 화면 감지·훅 출처 전환·입력/종료/실행·중복 요청·고정/별칭 보존·멀티 클라이언트·이동·detach·reload·DB 버전 2·무결성과 실제 Claude SessionStart 훅을 확인했다. 모델 요청은 하지 않았고 테스트 세션과 프로세스는 정리했다. 운영 세션에 설치·reload를 적용하지 않았다.

수동 실험 `cargo test -p dashboard-host repository_transaction_cost_with_request_history -- --ignored --nocapture`에서 요청 0개/4,000개(메시지 4 KiB), 수집 예약 획득+해제 두 트랜잭션의 중앙값을 12회씩 측정했다. debug 빌드의 전체 조회 scope는 0.729/192.637ms, runtime scope는 0.657/1.268ms였다. 현재 코드의 두 scope를 비교한 값이며 이전 바이너리와의 성능 배수로 해석하지 않는다. 측정 로그는 `.local/benchmark-transactions.log`다.

release CLI 스냅샷 조회도 다시 12회씩 측정했다. 이전 host의 요청 0개/4,000개 중앙값은 4.455/16.527ms, 최종 host는 3.678/3.538ms였다. 에이전트가 없는 격리 상태이며 프로세스 시작을 포함한 로컬 측정이다. 결과는 `.local/repository-benchmark-04495774b9/result.json`이다.

## 2026-10-08 h/l 고정·일반 영역 이동

`h`/왼쪽 방향키는 PINNED, `l`/오른쪽 방향키는 UNPINNED 영역을 선택한다. 같은 방향을 반복 입력해도 해당 영역에 머물며 영역별 선택을 유지한다. `Tab` 전환은 유지하고 좁은 화면에서도 선택 영역을 표시한다. 검색·메뉴 입력 중에는 기존 문자 입력 처리를 유지한다.

`./scripts/check.sh`에서 코어 27개·호스트 48개, 총 75개 테스트와 포맷·네이티브/WASI Clippy·스크립트 구문 검사를 통과했다. 영역별 선택 복원 테스트에 같은 영역 재선택을 추가했고 `./scripts/build.sh`로 release host/WASM을 빌드했다.

`python3 scripts/smoke.py`를 임시 Zellij 세션 두 개에서 통과했다. 실제 키 입력으로 `hh`의 고정 영역 유지, `ll`의 빈 일반 영역 유지, `h`의 선택 복원과 고정 해제 후 `l`의 일반 영역 선택을 확인했다. 기존 입력·종료·세션 이동·detach·reload·SQLite 검증도 통과했다. 결과는 `.local/smoke-70e0af5fcf/result.json`이다. 테스트 세션과 프로세스는 정리했으며 운영 세션에는 설치·reload를 적용하지 않았다.

## 2026-10-08 전역 agent next와 Tab working 순환

별도 bridge를 추가하지 않고 같은 WASM의 collector가 `agent-next`를 처리하도록 구현했다. 네 전역 키 필터와 `all`·`working-only`를 지원한다. 순환 선택은 표시 순서·부모 고정 상속을 공유하고 호스트에서 정확한 실행 세대와 pane을 재검증한다. 대시보드 `Tab`은 검색 결과의 양쪽 패널에서 살아 있는 working 에이전트만 선택하며 패널 이동은 `h/l`·`←/→`로 제공한다. 상태 파일 구조 변경은 없다.

`./scripts/check.sh`에서 코어 29개·호스트 49개, 총 78개 테스트와 포맷·네이티브/WASI Clippy·스크립트 구문 검사를 통과했다. 수동 성능 테스트 1개는 기존처럼 제외했다. `./scripts/build.sh`의 release host·wasm32-wasip1 빌드를 통과했다. 새 테스트는 정확한 idle, 부모 고정 상속, working 패널 간 순환과 빈 결과 보존, 종료·미확인·부재 pane 제외, 후보의 실행 세대 교체와 부재 시 이동 거부를 확인한다.

Zellij 0.45.0에서 `python3 scripts/smoke.py`를 통과했다. 결과는 `.local/smoke-6e5b9f09b0/result.json`, 로그는 `.local/agent-next-smoke-final.log`다. 실제 Tab 입력으로 working 선택·패널 이동·순환을 확인했다. 대시보드 없이 Alt+u의 순환, Alt+i의 idle 고정 필터, 같은 세션과 세션 경계를 포함한 연속 키, 빈 필터의 초점 보존, 다른 클라이언트 초점 유지, 전체 detach·재연결·collector reload 뒤 전역 키의 세션 간 이동을 확인했다. unpinned·idle-unpinned는 코어 및 실제 호스트 `next` 조회로 검증했으며 Alt+o/p의 실제 키 입력은 이 실행에서 별도로 검증하지 않았다. 기존 수집·상태·입력·종료·중복·세션 이동·SQLite 검증도 통과했고 임시 세션과 프로세스를 정리했다. 실제 Claude 옵션은 이번 변경에서 실행하지 않았다.

초기 검증에서 collector의 ModeUpdate에는 세션 이름이 없었고 전역 키의 private 메시지가 여러 클라이언트 인스턴스에 전달되는 것을 확인했다. 실제 세션은 세션 목록에서, 초점 pane과 연결된 클라이언트는 ListClients로 조회한다. 연결된 최소 ID의 클라이언트 하나만 요청을 처리하며 detach된 인스턴스와 다른 클라이언트의 큐는 버린다. 키 입력자 ID는 Zellij의 PipeMessage 계약에 없어 다중 클라이언트에서는 처리 클라이언트의 초점만 이동한다. 세션 이동 중 연속 키가 유실되지 않도록 대기 중인 순환을 끝낸 뒤 최종 대상으로 초점을 이동한다.

명시적으로 요청한 `/Users/in05908_mac/.config/zellij/config.kdl`의 Alt+u/i/o/p와 load_plugins를 이 프로젝트의 dist WASM·host·상태 경로로 교체하고 설정 검사를 통과했다. 백업은 `/Users/in05908_mac/.config/zellij/config.kdl.backup-20261008-agent-next-4ef2b83e`다. 실행 중 운영 세션에 강제 reload는 적용하지 않았다. 기존 Alt+q의 CLI 대시보드 실행 설정은 이번 bridge 교체 범위에 포함하지 않았다.

## 2026-10-08 Alt+q 최초 창 크기와 임시 창 제거

`agent-dashboard-open` private 메시지를 collector가 처리하고 연결된 최소 ID 클라이언트가 유한한 `dashboard-host open-dashboard SESSION` 명령을 실행한다. 호스트 어댑터는 기존 floating dashboard가 있으면 재사용하고, 없으면 `zellij plugin --width 90% --height 90% --x 5% --y 5%`로 생성한다. terminal 보조 pane과 생성 후 resize 단계를 제거했다. 세션별 파일 잠금과 collector의 진행 중 요청 억제로 중복 생성을 제한한다. 상태 파일 구조는 변경하지 않았다.

`./scripts/check.sh`에서 코어 29개·호스트 50개 테스트, 포맷·네이티브/WASI Clippy와 스크립트 구문 검사를 통과했고 release host/WASM을 빌드했다. Zellij 0.45.0 임시 세션 `zad-window-manual-test`의 80×24 화면에서 Alt+q로 72×21 창을 생성하고 반복 입력 시 같은 pane 재사용, terminal pane 추가 없음, 두 클라이언트에서 중복 없음, collector reload 뒤 재열기를 확인했다. 테스트 세션은 종료했다.

전체 `scripts/smoke.py` 재실행은 임시 클라이언트 시작·권한 승인 자동화에서 시간 초과되어 완료되지 않았다. 기록은 `.local/smoke-6f748963cf`와 `.local/smoke-80659fec63`이다. 이 실행을 전체 회귀 검증 통과로 기록하지 않는다. 사용자 요청에 따라 전역 Alt+q 설정을 교체하고 현재 세션 `운영`의 collector에만 reload를 적용했다. 연결된 클라이언트가 없어 검증용 클라이언트를 잠시 붙여 reload와 permissions=true 응답을 확인한 뒤 detach했다.
