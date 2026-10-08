# 화면 상태 어댑터 출처

사용자가 지정한 참고 저장소 `/Users/in05908_mac/zellij-with-codeagent`의 `internal/codingagent`를 기준으로 이관했다. 저장소 원격은 `https://github.com/jungting20/zellij-with-codeagent.git`이며 확인한 HEAD는 `1f49244a9af40d0ae936fb4088eca297810e3a64`다. 이관 작업은 참고 저장소를 수정하지 않는다.

- `detector.go`: 규칙 우선순위, 선언 순서, fallback, 활성 pane 제목 필터를 Rust로 이식했다.
- `matcher.go`: contains/regex/line_regex/all/any/not 의미를 Rust로 이식했다.
- `regions.go`: 화면 영역 선택과 prompt/horizontal-rule 경계를 Rust로 이식했다.
- `manifests/{claude,codex,gemini,cursor}.yaml`: JSON으로 변환해 내장했다. 규칙 ID, 정규식과 우선순위를 보존했다. YAML 파서는 런타임 의존성에 추가하지 않았다.
- `monitor.go`: 시작 3초 유예, 명시적 idle과 fallback 구분, 실행 세대 검증 원칙을 반영했다. Go monitor와 타이머·데몬·runtime API는 복사하지 않았다. fallback idle 확인은 새 화면 표본 3회로 바꿨다.

참고 저장소의 추적 파일과 루트 README에서 별도의 LICENSE/NOTICE 또는 라이선스 선언을 확인하지 못했다. 사용자가 지정한 코드 이관 범위에 따라 출처를 보존하며, 참고 코드에 임의로 오픈소스 라이선스를 부여하지 않는다. 이관한 파일에는 원본 저작권 고지가 없어 제거된 고지는 없다. Rust 정규식 엔진 의존성은 Cargo.lock으로 고정하며 해당 패키지의 라이선스는 원본 규칙의 라이선스와 별개다.

현재 host는 pane 화면과 제목을 읽는다. OSC progress는 제공되지 않아 관련 원본 규칙을 사용하지 않는다. Gemini/Claude/Cursor의 도구별 실제 대화 흐름 전체를 검증했다는 의미는 아니다. 실제 실행 검증 범위는 [실행 검증 기록](runtime-validation.md)을 따른다.
