# Architecture

ShillTrace is a local-first monitoring application with three runtime layers.

## Frontend

The React/Vite client is a dashboard only. `useShillTrace` owns remote state, SSE refreshes, polling fallback, alert audio, selection, and browser-title state. Components under `frontend/src/components` own presentation, while `utils/format.ts` contains pure display helpers. `api.ts` is the HTTP boundary.

## Backend

The Axum service exposes token, shill, channel, health, history, photo, and SSE endpoints. SQLx persists normalized tracking data in PostgreSQL. `tracker.rs` performs periodic market refreshes, while `market.rs` isolates external market-data access.

## Telegram ingestion

`telegram/` owns MTProto initialization, dialog discovery, and incoming updates. Detection is separated into `detection.rs`; accepted calls are persisted and broadcast to the UI. Telegram ingestion runs independently from the HTTP server so API traffic cannot block message processing.

## Data flow

Telegram message → detection → PostgreSQL → market resolution/tracking → SSE event → React refresh.

## Security boundary

The backend is designed for private/self-hosted use. It has no application authentication layer. Keep the API and PostgreSQL private unless authentication and a hardened reverse proxy are added. Telegram `.session` files are credentials and must never enter version control.
