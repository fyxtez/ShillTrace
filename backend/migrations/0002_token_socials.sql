-- DEX Screener supplies official project links with pair metadata. Keeping
-- them on tokens makes social controls available without a second API call.
ALTER TABLE tokens ADD COLUMN IF NOT EXISTS website_url TEXT;
ALTER TABLE tokens ADD COLUMN IF NOT EXISTS twitter_url TEXT;
ALTER TABLE tokens ADD COLUMN IF NOT EXISTS telegram_url TEXT;
