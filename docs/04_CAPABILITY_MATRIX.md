# 04. Capability Matrix

## 1. 원칙

제품 capability는 3종류로만 분류한다.

- **Core**: release blocker
- **Experimental**: 빌드 가능하더라도 제품 약속에 포함되지 않음
- **Removed from product surface**: 저장소에 남아도 제품 문서/README/doctor에서 기본 노출하지 않음

## 2. v1 Core Capability

| 영역 | capability | 상태 | release 기준 |
|---|---|---|---|
| Task | `run` | Core | must pass |
| Task | `doctor` | Core | must pass |
| Task | `replay` | Core | must pass |
| Workspace | list/read/search | Core | must pass |
| Editing | atomic patch apply | Core | must pass |
| Commands | allowlisted execution | Core | must pass |
| Backend | `codek` | Core | must pass |
| Backend | `mock` | Core | must pass |
| Trace | sqlite/jsonl event log | Core | must pass |
| Safety | path boundary / shell=false / timeout | Core | must pass |
| Validation | golden tasks / regression suite | Core | must pass |

## 3. Experimental Capability

| 영역 | capability | 이유 |
|---|---|---|
| Provider | direct openai-compatible provider | product default를 흐림 |
| Provider | anthropic direct adapter | backend 다변화는 v1 목표 아님 |
| Tools | browser | 제품 핵심과 거리 있음 |
| Tools | composio | scope 과확장 |
| Tools | delegate | multi-agent로 범위 확장 |
| Memory | markdown long-term memory | 정합성/드리프트 위험 |
| Memory | hybrid/axiomme/context | 외부 의존과 복잡성 증가 |
| Ops | metrics HTTP | 없어도 제품 핵심 가능 |
| Ops | cron | 장기 자동화는 v1 목표 아님 |
| Ops | daemon/service | 제품 경로를 흐림 |
| Channels | telegram/discord/slack/irc/matrix/whatsapp | 작업 완료 제품과 다른 면 |
| Gateway | signing/webhook ingress | 제품 핵심 경로가 아님 |

## 4. Removed From Product Surface

아래는 저장소에 남아도 release README/CLI 도움말/doctor default output에서는 숨긴다.

- `channel serve`
- `daemon`
- `gateway`
- `service`
- `cron`
- `skills`
- `integrations` 카탈로그 전면 노출
- `metrics_http`

## 5. CLI Surface Contract

### 허용

```bash
axonrunner run ...
axonrunner doctor ...
axonrunner replay ...
```

### 제품 도움말에서 숨김 또는 feature-gated

```bash
axonrunner experimental channel ...
axonrunner experimental gateway ...
axonrunner experimental daemon ...
```

## 6. Tool Surface Contract

v1 tool surface는 명시적으로 아래만 보장한다.

| tool | 기능 |
|---|---|
| `workspace.list` | 파일 인벤토리 |
| `workspace.read` | 파일 읽기 |
| `workspace.search` | 문자열 검색 |
| `workspace.write_patch` | atomic patch apply |
| `command.run` | allowlisted command 실행 |

핵심은 tool 수가 아니라, 이 다섯 개가 **정확하게 잘 동작하는 것**이다.

## 7. Backend Matrix

| backend | 역할 | status |
|---|---|---|
| `codek` | production backend | Core |
| `mock` | deterministic tests | Core |
| `openai-direct` | optional adapter | Experimental |
| `anthropic-direct` | optional adapter | Experimental |

## 8. Documentation Truth Rules

1. README에 적힌 기능은 반드시 `Core` 또는 명시적 `Experimental`로 표시한다.
2. doctor 출력은 현재 build feature와 정확히 일치해야 한다.
3. hidden fallback backend를 두지 않는다.
4. `available`은 실제 실행 가능함을 뜻한다.
5. `partial`은 제품 약속에서 제외된다.

## 9. Release blocker 조건

아래 중 하나라도 깨지면 release 금지.

- `run` 실패
- `doctor` 오류 진단 불가
- `replay` 불가
- path escape 가능
- allowlist 우회 가능
- `codek` backend session lifecycle 깨짐
- trace 저장/로딩 실패
- 문서/CLI/capability 불일치
