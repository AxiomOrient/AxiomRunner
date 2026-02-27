# axiomAi 배포 가이드

## 필수 환경변수

아래 변수들은 `AXIOM_PROFILE`, `AXIOM_ENDPOINT` 등 CLI args, 환경변수, 설정 파일 순으로 우선순위가 적용됩니다.

| 변수                            | 기본값                       | 설명                                                            |
| ------------------------------- | ---------------------------- | --------------------------------------------------------------- |
| `AXIOM_PROFILE`                 | `prod`                       | 실행 프로파일 이름                                              |
| `AXIOM_ENDPOINT`                | `http://127.0.0.1:8080`      | 게이트웨이 엔드포인트 URL                                       |
| `AXIOM_RUNTIME_PROVIDER`        | `mock-local`                 | 프로바이더 ID (`mock-local` / `openai` / `openrouter`)          |
| `AXIOM_AGENT_ID`                | `coclai`                     | `agent` 커맨드 백엔드 ID (`coclai` / `mock`)                    |
| `AXIOM_ALLOW_MOCK_AGENT`        | —                            | `AXIOM_AGENT_ID=mock` 허용 플래그 (`1`/`true`/`yes`)            |
| `OPENAI_API_KEY`                | —                            | OpenAI API 키 (`AXIOM_RUNTIME_PROVIDER=openai` 시 필수)         |
| `OPENROUTER_API_KEY`            | —                            | OpenRouter API 키 (`AXIOM_RUNTIME_PROVIDER=openrouter` 시 필수) |
| `AXIOM_RUNTIME_PROVIDER_MODEL`  | `gpt-4o-mini`                | 모델명 (프로바이더별 모델 ID)                                   |
| `AXIOM_RUNTIME_MAX_TOKENS`      | `4096`                       | 응답 최대 토큰 수                                               |
| `AXIOM_RUNTIME_MEMORY_PATH`     | `~/.axiom/memory.db`         | 메모리 DB 경로 (없으면 자동 생성)                               |
| `AXIOM_RUNTIME_TOOL_WORKSPACE`  | `~/.axiom/workspace`         | 툴 워크스페이스 디렉토리 (없으면 자동 생성)                     |
| `AXIOM_RUNTIME_TOOL_LOG_PATH`   | `.axiom/runtime-compose.log` | 툴 실행 로그 경로                                               |
| `AXIOM_RUNTIME_BOOTSTRAP_ROOT`  | —                            | 에이전트 컨텍스트 bootstrap 루트 디렉토리                       |
| `AXIOM_RUNTIME_CHANNEL`         | —                            | 활성화할 채널 ID                                                |
| `AXIOM_RUNTIME_TOOLS`           | —                            | 활성화할 툴 목록 (쉼표 구분)                                    |
| `AXIOM_CONTEXT_ROOT`            | —                            | AxiomMe RAG 컨텍스트 루트 디렉토리 (없으면 RAG 비활성화)        |
| `AXIOM_GATEWAY_SECRET`          | —                            | HTTP 게이트웨이 HMAC 서명 시크릿 (설정 시 서명 검증 활성화)     |
| `AXIOM_OTP_SECRET`              | —                            | CLI OTP 시크릿 (base32 인코딩, 설정 시 TOTP 강제)               |
| `AXIOM_OTP_CODE`                | —                            | TOTP 인증 코드 (`AXIOM_OTP_SECRET` 설정 시 필요)                |
| `AXIOM_CHANNEL_STORE_PATH`      | `~/.axiom/channel/store.db`  | 채널 레코드 저장 경로                                           |
| `AXIOM_CHANNEL_TELEGRAM_TOKEN`  | —                            | Telegram 봇 토큰 (Telegram 채널 사용 시 필수)                   |
| `AXIOM_CHANNEL_DISCORD_WEBHOOK` | —                            | Discord Webhook URL (Discord 채널 사용 시 필요)                 |
| `AXIOM_CHANNEL_SLACK_WEBHOOK`   | —                            | Slack Incoming Webhook URL (Slack 채널 사용 시 필요)            |
| `AXIOM_GATEWAY_REQUESTS`        | —                            | 게이트웨이 요청 설정 경로                                       |
| `COMPOSIO_API_KEY`              | —                            | Composio platform API 키 (`integrations info composio` 참조)    |
| `H2_ALLOWED_DIFF`               | `0`                          | h2_verify 바이너리의 허용 헤더 차이 임계값                      |

> **주의**: `anthropic` 프로바이더는 직접 지원하지 않습니다. Anthropic 모델을 사용하려면 `openrouter` 프로바이더와 함께 Anthropic 모델 ID를 지정하세요.

---

## 첫 실행 체크리스트

### 1. 빌드

```bash
cargo build --release
```

### 2. 시스템 진단

```bash
./target/release/axiom_apps doctor
```

모든 항목이 `pass` 또는 `info`로 표시되어야 합니다. `warn` 항목은 선택적 기능의 미설정을 의미하며, 운영에는 지장이 없습니다.

### 3. 신원 초기화

```bash
./target/release/axiom_apps onboard
```

에이전트 ID와 신원 정보를 초기화합니다. 최초 1회만 실행합니다.

### 4. 에이전트 테스트

```bash
AXIOM_AGENT_ID=mock \
AXIOM_ALLOW_MOCK_AGENT=1 \
./target/release/axiom_apps agent --message "health check"
```

mock 에이전트 백엔드로 로컬 실행 경로를 검증합니다.

### 5. coclai 연동 확인 (선택, 네트워크 필요)

```bash
./target/release/axiom_apps agent --message "hello"
```

### 6. 배포 승인 게이트 실행 (필수)

프로덕션 배포 직전에는 아래 통합 게이트를 반드시 통과해야 합니다.

```bash
bash scripts/run_release_approval_gate.sh
```

빠른 로컬 점검(저부하)은 아래처럼 실행합니다.

```bash
RELEASE_GATE_BENCH_ITERATIONS=1 \
RELEASE_GATE_BENCH_RECORDS=200 \
RELEASE_GATE_BENCH_WARMUP=0 \
RELEASE_GATE_BENCH_REQUIRED_CONSECUTIVE_PASSES=1 \
RELEASE_GATE_BENCH_MAX_PASSES=1 \
bash scripts/run_release_approval_gate.sh
```

승인 기준:

- `target/release-approval-gate/report.json`에서 `passed=true`
- `errors=[]`
- `security_gate_debug`, `security_gate_release`, `perf_gate` 모두 `pass`

실패 시에는 배포를 즉시 중지하고, 상세 절차는 [`docs/release-readiness-gate.md`](release-readiness-gate.md)의 장애 대응 절차를 따릅니다.

---

## Docker 설정 예시

### Dockerfile

```dockerfile
FROM rust:1.80-slim AS builder

WORKDIR /build
COPY . .
RUN cargo build --release --locked

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

RUN useradd -r -s /bin/false axiom
WORKDIR /app

COPY --from=builder /build/target/release/axiom_apps /app/axiom_apps

RUN mkdir -p /data/axiom && chown axiom:axiom /data/axiom

USER axiom
ENV AXIOM_RUNTIME_MEMORY_PATH=/data/axiom/memory.db
ENV AXIOM_RUNTIME_TOOL_WORKSPACE=/data/axiom/workspace
ENV AXIOM_CHANNEL_STORE_PATH=/data/axiom/channel/store.db

ENTRYPOINT ["/app/axiom_apps"]
CMD ["serve", "--mode=daemon"]
```

### docker-compose.yml

```yaml
services:
  axiom:
    build: .
    restart: unless-stopped
    environment:
      AXIOM_PROFILE: prod
      AXIOM_RUNTIME_PROVIDER: openai
      OPENAI_API_KEY: ${OPENAI_API_KEY}
      AXIOM_RUNTIME_PROVIDER_MODEL: gpt-4o-mini
      AXIOM_RUNTIME_MEMORY_PATH: /data/axiom/memory.db
      AXIOM_RUNTIME_TOOL_WORKSPACE: /data/axiom/workspace
      AXIOM_CHANNEL_STORE_PATH: /data/axiom/channel/store.db
      # 게이트웨이 서명 시크릿 (선택)
      # AXIOM_GATEWAY_SECRET: ${AXIOM_GATEWAY_SECRET}
      # Telegram 채널 (선택)
      # AXIOM_CHANNEL_TELEGRAM_TOKEN: ${TELEGRAM_BOT_TOKEN}
    volumes:
      - axiom_data:/data/axiom
    # 게이트웨이 모드 사용 시 포트 노출
    # ports:
    #   - "8080:8080"

volumes:
  axiom_data:
```

**시작 방법:**

```bash
# .env 파일 작성
echo "OPENAI_API_KEY=sk-..." > .env

# 첫 실행: 신원 초기화
docker compose run --rm axiom onboard

# 데몬 실행
docker compose up -d

# 로그 확인
docker compose logs -f axiom
```

---

## systemd 서비스 파일 예시

`/etc/systemd/system/axiom-daemon.service`:

```ini
[Unit]
Description=axiomAi Daemon
Documentation=https://github.com/your-org/axiomAi
After=network.target
Wants=network.target

[Service]
Type=simple
User=axiom
Group=axiom
WorkingDirectory=/opt/axiom

# 바이너리 경로
ExecStart=/opt/axiom/axiom_apps serve --mode=daemon

# 재시작 정책
Restart=on-failure
RestartSec=5s
StartLimitIntervalSec=60
StartLimitBurst=3

# 환경변수 파일 (보안: 600 권한 설정 필요)
EnvironmentFile=/etc/axiom/env

# 보안 하드닝
NoNewPrivileges=yes
PrivateTmp=yes
ProtectSystem=strict
ProtectHome=read-only
ReadWritePaths=/var/lib/axiom

# 로그
StandardOutput=journal
StandardError=journal
SyslogIdentifier=axiom-daemon

[Install]
WantedBy=multi-user.target
```

`/etc/axiom/env` (권한: `chmod 600 /etc/axiom/env`):

```bash
AXIOM_PROFILE=prod
AXIOM_RUNTIME_PROVIDER=openai
OPENAI_API_KEY=sk-...
AXIOM_RUNTIME_PROVIDER_MODEL=gpt-4o-mini
AXIOM_RUNTIME_MEMORY_PATH=/var/lib/axiom/memory.db
AXIOM_RUNTIME_TOOL_WORKSPACE=/var/lib/axiom/workspace
AXIOM_CHANNEL_STORE_PATH=/var/lib/axiom/channel/store.db
```

**서비스 등록 및 시작:**

```bash
# 사용자 및 디렉토리 생성
useradd -r -s /bin/false axiom
mkdir -p /var/lib/axiom /etc/axiom
chown axiom:axiom /var/lib/axiom

# 바이너리 설치
cp target/release/axiom_apps /opt/axiom/axiom_apps

# 신원 초기화 (서비스 시작 전 1회)
sudo -u axiom /opt/axiom/axiom_apps onboard

# 서비스 등록
systemctl daemon-reload
systemctl enable axiom-daemon
systemctl start axiom-daemon

# 상태 확인
systemctl status axiom-daemon
journalctl -u axiom-daemon -f
```

---

## Telegram 채널 연동

### 1. Telegram 봇 생성

1. Telegram에서 `@BotFather`와 대화
2. `/newbot` 명령으로 봇 생성
3. 발급된 봇 토큰을 `AXIOM_CHANNEL_TELEGRAM_TOKEN`에 설정

### 2. 채널 등록

```bash
export AXIOM_CHANNEL_TELEGRAM_TOKEN=1234567890:ABCdef...

# 채널 추가
./target/release/axiom_apps channel add telegram my-telegram \
  --token "$AXIOM_CHANNEL_TELEGRAM_TOKEN"

# 채널 목록 확인
./target/release/axiom_apps channel list
```

### 3. 채널 서버 실행

```bash
# 채널 serve (포그라운드)
./target/release/axiom_apps channel serve

# 채널 진단
./target/release/axiom_apps channel doctor
```

### 4. 채널 상태 확인

```bash
./target/release/axiom_apps channel list
```

---

## 운영 점검 커맨드

| 커맨드                      | 설명                                                    |
| --------------------------- | ------------------------------------------------------- |
| `axiom_apps doctor`         | 전체 시스템 진단 (provider, memory, tool, channel 상태) |
| `axiom_apps status`         | 현재 에이전트 상태 요약                                 |
| `axiom_apps health`         | 헬스체크 (게이트웨이/데몬 연결 확인)                    |
| `axiom_apps channel doctor` | 채널 어댑터 진단                                        |
| `axiom_apps channel list`   | 등록된 채널 목록 및 상태                                |
| `axiom_apps cron list`      | 등록된 cron 작업 목록                                   |
| `axiom_apps service status` | systemd 서비스 상태                                     |

### doctor 출력 예시

```
profile     : prod
endpoint    : http://127.0.0.1:8080
mode        : direct
revision    : 1

[pass] provider        mock-local (mock)
[pass] memory          enabled — /home/user/.axiom/memory.db
[pass] tool            enabled — /home/user/.axiom/workspace
[info] bootstrap       not configured
[info] channel         not configured
[warn] gateway-secret  AXIOM_GATEWAY_SECRET not set — HTTP signatures disabled
[warn] otp-secret      AXIOM_OTP_SECRET not set — CLI OTP disabled
```

---

## 보안 권고사항

- `OPENAI_API_KEY`, `AXIOM_GATEWAY_SECRET`, `AXIOM_OTP_SECRET` 등 시크릿은 환경변수 파일(`/etc/axiom/env`)에 저장하고 `chmod 600` 적용
- Docker 사용 시 `.env` 파일을 `.gitignore`에 추가
- `AXIOM_GATEWAY_SECRET` 설정으로 HTTP 엔드포인트 서명 검증 활성화 권장
- `AXIOM_OTP_SECRET` 설정으로 CLI 접근에 TOTP 2FA 적용 권장
- 프로덕션 환경에서는 `mock-local` 프로바이더 사용 금지
