# Signal Ledger

Private Telegram token-shill tracker built with Rust, PostgreSQL and React.

## What is included

- Telegram MTProto login and `Update::NewMessage` ingestion through `grammers`.
- Channel-only monitoring with an ignored-channel setup and locally cached photos.
- EVM and Solana contract extraction from raw text and common chart/trading links.
- DEX Screener chain discovery and current market-cap lookup.
- GeckoTerminal historical minute lookup for the initial shill market cap.
- Separate token, tracking-period, shill and additional-message records.
- Manual retry for calls that have no DEX market yet.
- 15-second tracking for active tokens and PostgreSQL market-cap samples.
- New Shills inbox, All Tokens table, channel filters, channel management and details.

## Quick start

1. Copy `.env.example` to `.env` and fill in Telegram credentials.
2. Start PostgreSQL:

   ```bash
   docker compose up -d postgres
   ```

3. Start the backend:

   ```bash
   cd backend
   cargo run
   ```

4. Start the frontend:

   ```bash
   cd frontend
   npm install
   npm run dev
   ```

Open `http://localhost:5173`. The first Telegram login asks for the OTP in the
backend terminal. Never commit the generated `.session` file or `.env`.

## Product semantics

- **Seen** removes one shill from the inbox but keeps the token tracked.
- **Remove token** closes its active tracking period without deleting history.
- A later shill automatically creates a fresh tracking period.
- A second channel creates a separate shill with its own initial and maximum X.
- Later messages from the same channel attach to the existing shill.
- Missing market data is informational and retried only through the UI.

