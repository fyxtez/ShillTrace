# Security Policy

## Private disclosure

Do not open a public issue for a suspected vulnerability. Report it privately to `fyxtez@gmail.com` and include:

- the affected component and version or commit;
- clear reproduction steps;
- the expected and observed behavior;
- the potential impact; and
- a minimal proof of concept, if one is safe to share.

Please allow reasonable time for investigation and remediation before any public disclosure.

## Sensitive local data

ShillTrace handles credentials and session material that must never be committed, shared, logged, or included in bug reports:

- `backend/.env` and Telegram API credentials;
- `*.session`, `*.session-shm`, and `*.session-wal` files;
- PostgreSQL credentials and production database exports; and
- downloaded Telegram channel photos or private message contents.

Run the API on a private interface unless authentication and an appropriate reverse-proxy configuration have been added. Keep PostgreSQL bound to localhost for local development, rotate any exposed credential immediately, and revoke a Telegram session if its file may have leaked.

## Telegram session warning

Telegram session files are authentication material, not cache files. Anyone who obtains a valid session may be able to act through that Telegram account/session. If a session file was ever published, revoke the affected Telegram session and generate a new local session.
