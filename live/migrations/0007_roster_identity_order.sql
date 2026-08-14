ALTER TABLE roster ADD COLUMN is_local INTEGER NOT NULL DEFAULT 0;
ALTER TABLE roster ADD COLUMN joined_order INTEGER;

CREATE INDEX idx_roster_feed_channel_order
ON roster (feed_id, session_id, channel_key, joined_order, user_handle);
