# ShillTrace

ShillTrace is a self-hosted Telegram token-call intelligence dashboard. It ingests selected Telegram channels in real time, detects token contracts across supported chains, resolves market data, and measures how every call performs after it appears.

## Highlights

- Real-time Telegram MTProto ingestion with channel discovery
- Contract detection for EVM, Solana and supported trading links
- DEX Screener market discovery with GeckoTerminal historical lookup
- Initial/current market cap, current X and max X tracking
- New-shill review inbox and chronological token archive
- Per-channel history, pinning, ignoring and visibility controls
- SSE-driven UI updates with polling fallback and audible alerts
- PostgreSQL-backed history with retry and destructive cleanup controls

## Stack

**Backend:** Rust, Tokio, Axum, SQLx, PostgreSQL, grammers  
**Frontend:** React, TypeScript, Vite, Lucide  
**Data:** DEX Screener, GeckoTerminal  
**Runtime:** Docker Compose for local PostgreSQL

## Repository structure

```text
backend/
  migrations/       SQLx schema evolution
  src/
    telegram/       MTProto setup, dialogs and update ingestion
    api.rs           Axum routes and HTTP/SSE boundary
    detection.rs     contract extraction
    market.rs        market-data clients
    tracker.rs       background market sampling
frontend/
  src/
    components/      dashboard UI
    hooks/           application state and live-update orchestration
    utils/           pure formatting helpers
docs/
  ARCHITECTURE.md
```

## Local development

Copy `backend/.env.example` to `backend/.env` and fill in your Telegram credentials. Then:

```bash
docker compose up -d postgres

cd backend
cargo run
```

Complete Telegram authentication in the backend terminal on first run. In another terminal:

```bash
cd frontend
npm install
npm run dev
```

The frontend runs at `http://localhost:5173` and the API defaults to `http://127.0.0.1:3001`.

## Verification

```bash
cd backend && cargo check
cd ../frontend && npm run build
```

## Security

ShillTrace is intended for private/self-hosted operation and currently has no application authentication layer. Do not expose the API or PostgreSQL directly to the public internet.

Never commit `.env`, Telegram `*.session` files, downloaded channel data, or database exports. See [SECURITY.md](SECURITY.md).

## Documentation

- [Architecture](docs/ARCHITECTURE.md)
- [Security policy](SECURITY.md)
- [Contributing](CONTRIBUTING.md)
- [Changelog](CHANGELOG.md)

## License

MIT — see [LICENSE](LICENSE).
