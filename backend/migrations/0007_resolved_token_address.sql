-- Keep the canonical base-token CA separate from the address posted on Telegram.
-- A caller may post a pair address, and migration recovery still needs the real
-- token address after that original pool disappears from DEX Screener.
ALTER TABLE tokens
    ADD COLUMN IF NOT EXISTS resolved_token_address TEXT;

