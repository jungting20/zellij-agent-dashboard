# Zellij CLI 조사 문서

공식 [Zellij User Guide](https://zellij.dev/documentation/introduction.html)를 기반으로 CLI 사용법과 CLI 자동화에 필요한 설정·레이아웃·세션·플러그인 지식을 한국어로 분류했다.

- 조사일: **2026-10-08**
- 기준: 조사 당시 공식 온라인 가이드, 보완 자료인 **zellij-utils 0.45.0** 공개 CLI 정의.
- 성격: 공식 문서 재구성과 안전한 활용 패턴. Zellij의 공식 문서 자체는 아니다.
- 검증 한계: 조사 환경의 PATH에서 `zellij`를 찾지 못했다. 명령/KDL 예제는 로컬 실행 검증하지 않았다. 온라인 문서가 설치 버전보다 앞설 수 있으므로 `zellij --version`과 각 명령의 `--help`를 우선한다.
- 프로젝트 경계: 프로젝트 구현의 기능/의존성/테스트 통과를 주장하지 않는다. 문서 예시는 프로젝트 파일에 실제로 존재하는 실행 명령이 아니다.

## 목차

1. [설치와 CLI 기본 구조](01-installation-cli-basics.md): 설치, 전역 옵션, 최상위 명령·별칭, setup 전체
2. [세션 관리](02-sessions.md): 생성, attach, 백그라운드, watch, 목록, 종료·삭제, 복원, 중첩
3. [Action 명령 전체](03-actions.md): pane, tab, 입력, 화면·스크롤, 레이아웃, 조회, 기타 action
4. [Run과 Edit](04-run-edit.md): command pane, 편집기, floating, in-place, blocking
5. [설정과 CLI options](05-configuration.md): KDL, 경로, 우선순위, 설정 옵션
6. [레이아웃](06-layouts.md): KDL 문법, pane·tab, cwd, template, floating, stack, swap
7. [셸 통합](07-shell-integration.md): completions, 자동 시작, 환경변수, OSC 133
8. [플러그인과 Pipe](08-plugins-pipes.md): 로딩, 별칭, configuration, 메시지, stdin/stdout
9. [Subscribe와 프로그래밍 제어](09-subscribe-programmatic-control.md): ID, JSON 조회, 화면 구독, 동기화
10. [CLI 레시피와 대시보드 패턴](10-recipes-dashboard.md): 워크플로, 자동화 설계, 오류·종료 처리
11. [Web와 원격 접속](11-web-remote.md): 서버, 토큰, HTTPS, 원격 attach
12. [문제 해결과 안전 수칙](12-troubleshooting-safety.md): 버전, 터미널 호환성, 진단, 민감정보
13. [출처·범위·갱신 방법](13-sources-coverage.md)

## 빠른 시작

아래는 설명용 예시이며 실제 환경에 맞게 이름·명령을 수정한다.

```sh
zellij --version
zellij --help
zellij setup --check

# 없으면 새 세션을 만들고 연결
zellij attach --create agent-dashboard

# 다른 터미널에서 대상 세션을 명시적으로 제어
zellij --session agent-dashboard run --name logs -- sh -c 'printf "ready\n"; sleep 30'
zellij --session agent-dashboard action list-panes --help
```

자동화 구현 순서는 **기능 지원 확인 → 세션 지정 → ID 발견 → 구조화된 상태 조회 → 대상 ID로 변경 → 결과 재조회**를 권장한다. focused pane이나 UI 키바인드에 의존하지 않는다.

## 범위와 완전성의 의미

공식 가이드에서 다루는 공개 CLI 명령군과 관련 옵션을 범위로 삼았다. action 목록은 `03-actions.md`, 최상위 명령은 `01-installation-cli-basics.md`에서 찾는다. 관련 설정, 레이아웃, shell integration, resurrection, web/remote도 포함한다.

플러그인 **개발 API 전체**, 테마 이름 전체, 가능한 키바인드 조합 전체, Zellij 내부 IPC 구현은 CLI 사용법과 구별하여 범위 밖으로 두었다. 필요한 공식 링크는 출처 문서에 남겼다. 숨겨진 내부 서버 옵션은 공개 API로 권장하지 않는다.

온라인 최신 가이드와 0.45.0 소스가 일치하지 않는 항목은 해당 문서에서 표시했다. “조사 범위를 다뤘다”는 것이 모든 Zellij 버전에서 동일하게 실행된다는 뜻은 아니다.

## 특히 주의할 사항

- `detach`, `kill-session`, `delete-session`은 서로 다르다.
- 전체 세션 종료/삭제 명령을 자동 정리로 사용하지 않는다.
- `--force-run-commands`는 복원된 명령을 바로 실행한다.
- `run`의 blocking과 렌더 화면 구독은 프로세스 종료 코드·stdout 캡처와 별개로 이해한다.
- 화면/scrollback, 셸 history, Web 토큰에는 비밀이 포함될 수 있다.
- 가이드의 `kill-sessions`와 `options --clean` 표기, 일부 옵션/예제는 공식 자료 사이에 불일치가 있어 설치된 도움말로 확인해야 한다.
