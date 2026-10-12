# Zellij 플러그인 로딩, 별칭, Pipe 인터페이스

> 조사 범위: Zellij 최신 온라인 공식 문서와 공식 `zellij-utils` 0.45.1 API/CLI 소스 문서.
>
> **실행 검증은 하지 않았다.** 아래 명령, 플래그 조합, STDIN 프레이밍, 종료 코드는 실제 설치된 `zellij --version`, `zellij plugin --help`, `zellij pipe --help`, `zellij action pipe --help`로 다시 확인해야 한다.
>
> 표기 규칙: **[공식 사실]**은 공식 문서에 명시된 내용이고, **[추천 패턴]**은 운영상 제안이다.

## 1. 핵심 구분

| 인터페이스 | 용도 | 중요한 차이 |
|---|---|---|
| `zellij plugin` | 플러그인 WASM을 새 pane으로 로드 | top-level 명령. URL/alias와 표시 위치를 지정한다. |
| `zellij pipe` | CLI에서 플러그인으로 메시지 전송 | 목적지를 생략하면 수신 중인 모든 플러그인에 broadcast한다. 목적지가 없으면 첫 메시지 때 로드할 수 있다. |
| `zellij action launch-plugin` | 현재/지정 session 안에서 새 plugin pane 생성 | 항상 새 인스턴스를 만들고 pane ID를 반환한다. |
| `zellij action launch-or-focus-plugin` | 기존 인스턴스를 찾으면 focus, 없으면 생성 | 단일 UI 도구를 열거나 focus할 때 적합하다. |
| `zellij action pipe` | action 하위의 확장 Pipe 명령 | top-level `pipe`보다 launch 모양, cwd, title, cache 등을 더 세밀하게 지정한다. |
| plugin `pipe()` lifecycle | WASM 플러그인이 PipeMessage 수신 | source/name/payload/args/private 여부를 받으며 `bool`로 render 여부를 반환한다. |
| plugin `pipe_message_to_plugin()` | 플러그인에서 다른 플러그인으로 pipe 시작 | URL 또는 내부 plugin ID를 대상으로 하고, 미실행 대상은 로드할 수 있다. |

## 2. 플러그인 URL과 로딩

### 2.1 지원 URL 스키마

**[공식 사실]** 플러그인은 다음 네 가지 방식으로 참조된다.

1. 로컬 파일: `file:/absolute/path/to/plugin.wasm`
2. 내장 플러그인: `zellij:tab-bar`, `zellij:strider` 등
3. 원격 URL: `http://...` 또는 `https://...`
4. bare alias: `filepicker`, `compact-bar` 등

로딩 경로는 layout, CLI, keybinding, config의 `load_plugins`, built-in plugin manager를 포함한다.

```kdl
load_plugins {
    https://example.com/my-plugin.wasm
    file:/absolute/path/to/my-plugin.wasm
    my-plugin-alias
}
```

`load_plugins` 항목은 session 시작 시 background에서 로드된다. 권한이 필요하면 사용자에게 권한 요청 UI가 나타날 수 있다.

### 2.2 별칭과 identity

**[공식 사실]** alias는 config의 `plugins` 블록에서 `alias -> location + 선택적 configuration`으로 정의한다.

```kdl
plugins {
    tab-bar location="zellij:tab-bar"
    status-bar location="zellij:status-bar"
    strider location="zellij:strider"
    compact-bar location="zellij:compact-bar"
    session-manager location="zellij:session-manager"
    welcome-screen location="zellij:session-manager" {
        welcome_screen true
    }
    filepicker location="zellij:strider" {
        cwd "/"
    }
}
```

- bare alias는 layout, CLI, keybinding, 다른 plugin의 pipe 목적지에서 URL로 해석된다.
- 같은 URL이라도 configuration이 다르면 Pipe 목적지 관점에서 서로 다른 플러그인이다.
- 기본 alias를 제거하거나 기본 contract를 지키지 않는 구현으로 교체하면 Zellij가 기대대로 동작하지 않을 수 있다.
- alias에 `cwd`가 정의된 경우, Zellij는 focused pane의 cwd를 `caller_cwd` configuration으로 추가한다. alias의 원래 `cwd`를 덮어쓰지 않는다.
- 실행 중 configuration 변경 이벤트가 발생해도 Pipe identity는 최초 로드 시 configuration에 묶여 있다. 이후 목적지 지정에는 변경된 값이 아니라 원래 값을 사용해야 한다.

**[추천 패턴]** 자동화에서는 장기적으로 사람이 바꿀 수 있는 alias보다 고정 URL과 명시적 configuration을 사용한다. alias를 쓴다면 시작 시 `zellij list-aliases` 결과와 예상 location을 기록해 공급망 및 설정 드리프트를 탐지한다.

## 3. `zellij plugin` 전체 플래그

### 3.1 온라인 가이드에 명시된 문법

```text
zellij plugin [OPTIONS] [--] <URL>
```

`<URL>`은 `http(s)`, `file:`, `zellij:` 또는 bare alias다.

| 짧은/긴 옵션 | 값 | 의미 및 제약 |
|---|---:|---|
| `-c, --configuration` | `<CONFIGURATION>` | 플러그인 configuration |
| `-f, --floating` | 없음 | floating pane으로 열기 |
| `-h, --help` | 없음 | 도움말 |
| `--height` | `<HEIGHT>` | floating 높이. 정수 또는 `%` |
| `-i, --in-place` | 없음 | 현재 pane을 일시 정지하고 그 자리에 열기 |
| `--no-focus` | 없음 | 어떤 client의 focus도 바꾸지 않음 |
| `-s, --skip-plugin-cache` | 없음 | 메모리/디스크 cache를 건너뛰고 다시 로드/컴파일 |
| `--tab-id` | `<TAB_ID>` | 지정 tab에 열기. `--in-place`와 충돌 |
| `--width` | `<WIDTH>` | floating 너비. 정수 또는 `%` |
| `-x, --x` | `<X>` | floating x. 정수 또는 `%` |
| `-y, --y` | `<Y>` | floating y. 정수 또는 `%` |

### 3.2 0.45.1 공식 CLI 소스에 추가로 존재하는 옵션

공식 `zellij-utils` 0.45.1 `cli.rs`에는 온라인 `zellij-plugin-and-pipe.html` 표에 없는 다음 옵션도 정의되어 있다.

| 옵션 | 값 | 의미 및 제약 |
|---|---:|---|
| `--close-replaced-pane` | 없음 | `--in-place`로 대체되는 pane을 suspend하지 않고 닫음. `--in-place` 필요 |
| `--pinned` | `<true|false>` | floating pane을 항상 위에 유지. `--floating` 필요 |
| `-b, --borderless` | `<true|false>` | borderless pane으로 시작 |

0.45.1 소스상 전체 집합은 다음과 같다.

```text
zellij plugin [OPTIONS] [--] <URL>
  -c, --configuration <CONFIGURATION>
  -f, --floating
  -i, --in-place
      --close-replaced-pane
  -s, --skip-plugin-cache
  -x, --x <X>
  -y, --y <Y>
      --width <WIDTH>
      --height <HEIGHT>
      --pinned <true|false>
      --no-focus
  -b, --borderless <true|false>
      --tab-id <TAB_ID>
  -h, --help
```

**[추천 패턴]** 온라인 표와 소스가 다르므로 실제 binary의 `zellij plugin --help`를 최종 계약으로 취급한다. 특히 `--pinned`, `--borderless`, `--close-replaced-pane`를 배포 스크립트에서 사용하기 전에 최소 버전을 고정한다.

## 4. `zellij pipe` 전체 플래그

### 4.1 top-level `zellij pipe`

```text
zellij pipe [OPTIONS] [--] <PAYLOAD>
```

`PAYLOAD`를 생략하면 STDIN을 읽는다.

| 짧은/긴 옵션 | 값 | 의미 |
|---|---:|---|
| `-n, --name` | `<NAME>` | pipe 이름. 생략하면 Zellij가 random UUID를 부여한다. |
| `-a, --args` | `<ARGS>` | pipe argument map. plugin user configuration 형식 |
| `-p, --plugin` | `<PLUGIN>` | 특정 URL/alias 목적지. 생략 시 broadcast. 미실행이면 로드 가능 |
| `-c, --plugin-configuration` | `<PLUGIN_CONFIGURATION>` | 목적지 identity 및 신규 로드 configuration |
| `-h, --help` | 없음 | 도움말 |

예:

```bash
zellij pipe --name agent.event --plugin file:/opt/agent-dashboard.wasm -- '{"kind":"started","agent":"a1"}'

printf '%s\n' '{"kind":"log","text":"one"}' '{"kind":"log","text":"two"}' \
  | zellij pipe --name agent.events --plugin agent-dashboard
```

위 예시는 설명용이며 실행하지 않았다.

### 4.2 `zellij action pipe` 확장 플래그

공식 CLI Actions 문서의 전체 옵션은 다음과 같다.

```text
zellij action pipe [OPTIONS] [--] [PAYLOAD]
  -n, --name <NAME>
  -a, --args <ARGS>
  -p, --plugin <PLUGIN>
  -c, --plugin-configuration <CONFIG>
  -l, --force-launch-plugin
  -s, --skip-plugin-cache
  -f, --floating-plugin <BOOL>
  -i, --in-place-plugin <BOOL>
  -w, --plugin-cwd <CWD>
  -t, --plugin-title <TITLE>
```

- `--force-launch-plugin`: 같은 목적지가 이미 실행 중이어도 새 인스턴스를 시작한다.
- `--floating-plugin`: 신규 목적지가 로드될 때 floating 여부. 소스 설명상 기본은 floating이다.
- `--in-place-plugin`: 신규 목적지를 현재 pane 자리에 로드한다. `--floating-plugin`과 충돌한다.
- `--plugin-cwd`, `--plugin-title`: 첫 메시지로 새 플러그인을 띄울 때 사용할 cwd/title이다.

**문서상 차이:** top-level `zellij pipe` 가이드는 기본 네 옵션만 제공하며, 확장 launch 옵션은 `zellij action pipe`에만 문서화되어 있다. 둘을 같은 명령으로 간주하지 말아야 한다.

## 5. Pipe 전달 모델

### 5.1 이름, payload, args

**[공식 사실]** Pipe는 모두 선택적인 다음 요소를 가진 단방향 메시지 채널이다.

- `name: String`: 생략 시 random UUID
- `payload: Option<String>`: 임의의 직렬화 가능한 text
- `args: BTreeMap<String, String>`: 임의 문자열 key/value
- destination: plugin URL + configuration, 또는 plugin 내부 ID

목적지가 없는 Pipe는 수신하는 모든 plugin에 broadcast된다. 특정 URL+configuration 목적지가 실행 중이 아니면 첫 메시지 때 plugin을 로드하며, 첫 메시지는 로딩이 끝날 때까지 기다린다.

같은 목적지 plugin의 instance가 여러 개이면, 예를 들어 여러 사용자가 같은 session에 attach해 instance가 복수라면 모든 instance가 메시지를 받는다.

### 5.2 정확한 `PipeMessage` 구조

공식 `zellij-tile`/`zellij-utils` 0.45.1 타입:

```rust
pub struct PipeMessage {
    pub source: PipeSource,
    pub name: String,
    pub payload: Option<String>,
    pub args: BTreeMap<String, String>,
    pub is_private: bool,
}

pub enum PipeSource {
    Cli(String),    // CLI pipe ID(UUID 문자열)
    Plugin(u32),   // source plugin의 내부 ID
    Keybind,
}
```

- `is_private == true`: 특정 plugin으로 직접 보낸 메시지
- `is_private == false`: broadcast 메시지
- `PipeSource::Cli(id)`: backpressure와 CLI STDOUT 출력에 쓰는 내부 pipe ID
- `PipeSource::Plugin(id)`: 응답 Pipe를 만들 때 source plugin을 대상으로 사용할 수 있는 내부 ID

### 5.3 plugin lifecycle

플러그인은 `pipe()`를 구현한다.

```rust
fn pipe(&mut self, pipe_message: PipeMessage) -> bool {
    match pipe_message.source {
        PipeSource::Cli(pipe_id) => {
            if let Some(payload) = pipe_message.payload {
                self.consume(payload);
            }
            // true이면 이 메시지 뒤 render() 요청
            true
        }
        PipeSource::Plugin(source_plugin_id) => {
            self.record_sender(source_plugin_id);
            false
        }
        PipeSource::Keybind => false,
    }
}
```

반환값이 `true`이면 Zellij가 plugin을 render한다. `false`이면 이 메시지 때문에 render하지 않는다.

### 5.4 plugin에서 plugin으로 전송

```rust
fn pipe_message_to_plugin(message_to_plugin: MessageToPlugin)
```

공식 API 예:

```rust
pipe_message_to_plugin(MessageToPlugin {
    plugin_url: Some("file:/path/to/other-plugin.wasm".to_string()),
    destination_plugin_id: None,
    plugin_config: BTreeMap::new(),
    message_name: "process_data".to_string(),
    message_payload: Some("payload content".to_string()),
    message_args: BTreeMap::new(),
    new_plugin_args: None,
    floating_pane_coordinates: None,
});
```

필요 권한은 `MessageAndLaunchOtherPlugins`다.

`zellij:OWN_URL` 목적지는 source plugin 자신의 URL로 치환된다. 새 자기 인스턴스에 보낼 때 현재 인스턴스와 다른 configuration을 줘야 한다. 동일 URL+동일 configuration이면 메시지가 원래 plugin으로 되돌아올 수 있다.

## 6. STDIN, STDOUT, backpressure

### 6.1 공식 동작

- CLI Pipe는 payload argument 대신 STDIN을 읽어 같은 Pipe에 여러 메시지를 보낼 수 있다.
- plugin render가 끝나거나 plugin이 render하지 않기로 한 뒤에 다음 STDIN buffer가 release된다.
- plugin은 CLI pipe ID를 알고 있으면 입력을 명시적으로 block/unblock할 수 있다.
- 입력이 block되어도 plugin은 같은 Pipe의 STDOUT으로 데이터를 출력할 수 있다.
- 여러 plugin 또는 여러 instance가 같은 CLI pipe의 STDOUT에 출력할 수 있다.
- plugin을 거치는 파이프는 일반 Unix 프로그램 간 pipe보다 보통 느리다. plugin이 메시지마다 render할 수 있기 때문이다.

관련 API:

```rust
fn block_cli_pipe_input(pipe_name: &str)
fn unblock_cli_pipe_input(pipe_name: &str)
fn cli_pipe_output(pipe_name: &str, output: &str)
```

공식 Plugin API Commands 문서는 인자 이름을 `pipe_name`으로 쓰지만, Plugin Pipes 설명과 `PipeSource::Cli(String)`은 이 값을 내부 CLI pipe ID로 설명한다. 즉 애플리케이션 수준의 `--name`과 혼동하지 말아야 한다.

필요 권한은 `ReadCliPipes`다.

### 6.2 명시되지 않은 부분

공식 문서는 STDIN byte stream이 plugin `pipe()` 호출로 어떻게 segment되는지, newline 단위인지, 임의 chunk 단위인지 명확한 framing 계약을 제시하지 않는다.

**[추천 패턴]**

1. 가장 강한 계약이 필요하면 payload argument로 한 메시지씩 호출한다.
2. 장기 STDIN stream을 쓸 때는 length-prefix 또는 NDJSON 같은 애플리케이션 framing을 정의하고 plugin에서 partial buffer를 누적한다.
3. `pipe()` 한 번이 논리 record 하나라고 가정하지 않는다.
4. CLI STDOUT을 여러 plugin이 쓸 수 있으므로 broadcast 요청/응답에는 `request_id`, `producer_id`를 포함한다.
5. 사람이 읽는 log와 기계 응답을 같은 STDOUT에 섞지 않는다. plugin debug log는 Zellij log/STDERR 경로를 사용한다.

## 7. Agent dashboard에 쓰는 Pipe protocol 제안

아래는 **[추천 패턴]**이며 Zellij 자체 표준이 아니다.

### 7.1 명명

```text
agent.command
agent.event
agent.heartbeat
agent.snapshot.request
agent.snapshot.response
```

### 7.2 payload 예

```json
{
  "schema_version": 1,
  "message_id": "01J...",
  "request_id": null,
  "agent_id": "worker-3",
  "kind": "state_changed",
  "sequence": 42,
  "timestamp": "2026-10-08T12:34:56+09:00",
  "state": "busy",
  "pane_id": "terminal_7",
  "detail": "running tests"
}
```

권장 필드:

- `schema_version`: protocol 진화
- `message_id`: 중복 제거
- `request_id`: 요청/응답 상관관계
- `agent_id`: 여러 instance 출력 구분
- `sequence`: agent별 단조 증가 번호
- `pane_id`: `terminal_N`/`plugin_N` 형태
- `timestamp`: 관측 시각. 상태 자체의 절대 ordering으로 과신하지 않는다.

### 7.3 전달 전략

- command는 `--plugin <exact-url-or-alias> --plugin-configuration ...`로 private delivery한다.
- event broadcast는 여러 dashboard/notification plugin이 동시에 소비해야 할 때만 쓴다.
- dashboard가 아직 없으면 첫 message로 로드되도록 할 수 있지만, 중요한 초기 상태는 plugin load 완료 후 snapshot handshake로 다시 보낸다.
- plugin instance가 복수일 수 있으므로 receiver는 idempotent하게 처리한다.
- backpressure를 UX pause로 쓸 때 watchdog timeout 및 운영자 취소 경로를 둔다.

## 8. 위험 및 경쟁 상태

| 위험 | 공식 근거 | 완화 권고 |
|---|---|---|
| broadcast fan-out | 목적지 없는 Pipe는 모든 listening plugin에 전달 | 기계 제어 command는 항상 정확한 URL+configuration 지정 |
| 여러 instance 중복 처리 | 같은 목적지의 모든 instance가 메시지 수신 | `message_id` 기반 dedupe, 단일 leader 또는 idempotent action |
| STDOUT interleave | 여러 plugin이 같은 CLI pipe에 출력 가능 | producer envelope 및 NDJSON, 응답 수 기대치 명시 |
| 영구 block/deadlock | plugin이 CLI input을 block하고 나중에 release 가능 | timeout, cancel pipe, finally/unblock 경로 |
| 느린 처리 | render가 다음 STDIN release에 backpressure | batch, render coalescing, `false` 반환, rate limit |
| identity drift | URL+config가 identity이며 runtime config 변경 후에도 최초 config 사용 | immutable identity config와 mutable runtime config 분리 |
| 자기 자신으로 loop | `zellij:OWN_URL` + 동일 config이면 원래 plugin으로 돌아올 수 있음 | 새 role/instance ID를 config에 넣기 |
| 원격 WASM 위험 | HTTPS URL을 직접 load 가능, plugin permission 요청 존재 | checksum/고정 artifact, 신뢰 URL, 최소 권한, cache 정책 명시 |
| alias hijack/drift | alias는 config에서 arbitrary URL로 교체 가능 | automation 시작 시 alias 해석 검증 또는 URL 고정 |
| cache stale | 일반 load는 cache 사용 | 개발에서만 `--skip-plugin-cache`; 운영은 버전 URL/해시로 갱신 |
| pane focus 경쟁 | plugin open이 connected client focus를 바꿀 수 있음 | dashboard background load에 `--no-focus`, 명시적 `--tab-id` |

## 9. 권한

Pipe 관련 핵심 permission:

- `ReadCliPipes`: CLI pipe input block/unblock, CLI pipe output
- `MessageAndLaunchOtherPlugins`: 다른 plugin으로 message 전송 및 필요 시 launch
- `ReadApplicationState`: tab/pane 상태 event를 읽는 dashboard
- `ReadPaneContents`: `PaneRenderReport`로 다른 pane rendered content를 읽는 dashboard
- `OpenTerminalsOrPlugins`, `ChangeApplicationState`, `WriteToStdin`: dashboard가 workspace를 실제 제어할 때

최소 권한 원칙을 적용한다. 상태 표시 전용 dashboard에 command 실행이나 전체 파일시스템 접근 권한을 주지 않는다.

## 10. 버전 호환성

- Pipes와 Plugin Aliases는 Zellij **0.40.0** release에서 소개됐다.
- CLI automation, `zellij subscribe`, pane/tab 생성 ID 반환, 조건부 blocking은 **0.44.0**에서 크게 확장됐다.
- 조사 시점 최신 공식 release 및 docs.rs latest는 **0.45.1**이다.
- 0.44.0 release는 protobuf 기반 client/server contract 도입을 설명한다. 0.44.0 업그레이드는 기존 session을 마지막으로 orphan할 수 있으며, 이후 새 client가 오래된 session에 연결될 수 있지만 **새 기능은 오래된 session에서 조용히 실패할 수 있다**고 명시한다.
- plugin ABI/API는 `zellij-tile` crate 버전에 의존한다. plugin 빌드 시 Zellij host와 호환되는 crate/version을 고정한다.

**[추천 패턴]**

```bash
zellij --version
zellij plugin --help
zellij pipe --help
zellij action pipe --help
```

을 배포 전 검사하고, 최소 `0.44.0`이 필요한 CLI automation과 최소 `0.40.0`인 Pipe/alias 기능을 분리해 capability gate한다. 오래된 running session을 재사용할 때는 명령 성공처럼 보여도 원하는 새 기능이 적용되지 않았는지 상태 query로 확인한다.

## 11. 발견한 공식 문서 불일치

1. **`zellij plugin` 옵션 누락**
   - 온라인 `zellij-plugin-and-pipe.html`에는 `--close-replaced-pane`, `--pinned`, `--borderless`가 없다.
   - 공식 `zellij-utils` 0.45.1 CLI 소스에는 세 옵션이 있다.
2. **Pipe ID 명칭 혼동**
   - Plugin Pipes 문서는 `PipeSource::Cli`가 제공하는 내부 pipe ID를 block/unblock/output에 사용한다고 설명한다.
   - Plugin API Commands 함수 인자명과 표는 이를 `pipe_name`이라고 부른다. 사용자 지정 `--name`과 동일하다고 단정하면 안 된다.
3. **Permission 목록 누락**
   - Plugin API Commands는 Pipe API에 `ReadCliPipes`, plugin 간 전송에 `MessageAndLaunchOtherPlugins`가 필요하다고 명시한다.
   - 온라인 `plugin-api-permissions.html`의 서술 목록에는 이 두 permission과 `WebAccess`가 누락되어 있지만, 공식 Type Reference의 `PermissionType` enum에는 존재한다.
4. **top-level `pipe`와 `action pipe` 범위 차이**
   - Plugin & Pipe 가이드의 top-level 명령은 네 가지 옵션만 설명한다.
   - CLI Actions의 `action pipe`는 force launch, cache, floating/in-place, cwd, title 옵션을 추가로 제공한다.

## 12. 공식 자료

사용자가 지정한 공식 문서:

- https://zellij.dev/documentation/zellij-plugin-and-pipe.html
- https://zellij.dev/documentation/plugin-loading.html
- https://zellij.dev/documentation/plugin-aliases.html
- https://zellij.dev/documentation/plugin-pipes.html

교차 확인에 사용한 공식 문서/소스:

- https://zellij.dev/documentation/cli-actions.html
- https://zellij.dev/documentation/plugin-api.html
- https://zellij.dev/documentation/plugin-api-commands.html
- https://zellij.dev/documentation/plugin-api-types.html
- https://zellij.dev/documentation/plugin-api-permissions.html
- https://zellij.dev/documentation/plugin-api-events.html
- https://zellij.dev/documentation/plugin-lifecycle.html
- https://docs.rs/zellij-utils/0.45.1/src/zellij_utils/cli.rs.html
- https://docs.rs/zellij-tile/latest/zellij_tile/prelude/struct.PipeMessage.html
- https://docs.rs/zellij-tile/latest/zellij_tile/prelude/enum.PipeSource.html
- https://zellij.dev/news/welcome-screen-pipes-filepicker/
- https://zellij.dev/news/remote-sessions-windows-cli/
- https://github.com/zellij-org/zellij/releases/latest
