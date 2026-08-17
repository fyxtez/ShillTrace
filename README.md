# ShillTrace

ShillTrace is a private, self-hosted Telegram token-call tracker. It listens to selected Telegram channels, extracts EVM and Solana contract addresses, resolves market data, and records how each call performs over time.

## Features

- Telegram MTProto authentication and real-time channel ingestion
- EVM and Solana contract detection from messages and common trading links
- DEX Screener discovery and live market-cap resolution
- GeckoTerminal historical lookup for the initial market cap
- Separate tokens, tracking periods, shills, messages, and market-cap samples
- New Shills review inbox and chronological All Tokens archive
- Channel search, pinning, ignoring, and per-channel shill history
- Live UI refresh through server-sent events with polling fallback
- Retry and stop-tracking controls without deleting historical records

## Stack

- **Backend:** Rust, Tokio, Axum, SQLx, PostgreSQL, `grammers`
- **Frontend:** React, TypeScript, Vite, Lucide
- **Market data:** DEX Screener and GeckoTerminal

## Prerequisites

- Rust toolchain compatible with edition 2024
- Node.js 20 or newer and npm
- Docker with Docker Compose, or a local PostgreSQL instance
- Telegram API credentials from `my.telegram.org`

## Configuration

Create `backend/.env` with the following values:

```dotenv
DATABASE_URL=postgres://signal_ledger:signal_ledger@127.0.0.1:5433/signal_ledger
TELEGRAM_API_ID=your_api_id
TELEGRAM_API_HASH=your_api_hash
TELEGRAM_PHONE_NUMBER=+381...

# Optional
TELEGRAM_PASSWORD=
API_BIND_ADDR=127.0.0.1:3001
FRONTEND_ORIGIN=http://localhost:5173
TELEGRAM_SESSION_PATH=signal_ledger.session
PHOTOS_DIR=storage/channel-photos
MARKET_POLL_SECONDS=15
```

Never commit `.env`, Telegram session files, or downloaded channel photos.

## Run locally

Start PostgreSQL from the repository root:

```bash
docker compose up -d postgres
```

Start the backend:

```bash
cd backend
cargo run
```

On the first run, complete Telegram authentication in the backend terminal. Then start the frontend in another terminal:

```bash
cd frontend
npm install
npm run dev
```

Open `http://localhost:5173`.

## Verification

```bash
cd backend && cargo check
cd ../frontend && npm run build
```

## Product behavior

- **Seen** removes a shill from the inbox while its token stays tracked.
- **Remove token** closes the active tracking period but preserves history.
- A later call for a stopped token automatically starts a new tracking period.
- Calls from different channels remain separate and keep their own initial and maximum multipliers.
- Later messages from the same channel attach to its existing shill.
- Unavailable historical market data is informational and can be retried from the UI.

## Repository layout

```text
signal-ledger/
├── backend/          Rust API, Telegram ingestion, tracker, and migrations
├── frontend/         React dashboard and local visual assets
├── docker-compose.yml
├── LICENSE
└── SECURITY.md
```

## Security and license

See [SECURITY.md](SECURITY.md) before deploying or reporting a vulnerability. This is private proprietary software; see [LICENSE](LICENSE).
