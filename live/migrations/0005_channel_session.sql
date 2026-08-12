ALTER TABLE channels ADD COLUMN session_id TEXT NOT NULL DEFAULT '';
ALTER TABLE channels ADD COLUMN session_started_at INTEGER NOT NULL DEFAULT 0;

WITH activity AS (
  SELECT feed_id, channel_key, session_id, ts FROM messages
  UNION ALL
  SELECT feed_id, channel_key, session_id, ts FROM member_events
  UNION ALL
  SELECT feed_id, channel_key, session_id, updated_at AS ts FROM roster
), ranked AS (
  SELECT feed_id, channel_key, session_id,
         ROW_NUMBER() OVER (PARTITION BY feed_id, channel_key ORDER BY ts DESC) AS rank
  FROM activity
)
UPDATE channels
SET session_id = COALESCE((
  SELECT ranked.session_id
  FROM ranked
  WHERE ranked.feed_id = channels.feed_id
    AND ranked.channel_key = channels.key
    AND ranked.rank = 1
), '');

UPDATE channels
SET session_started_at = COALESCE((
  SELECT sessions.started_at
  FROM sessions
  WHERE sessions.feed_id = channels.feed_id
    AND sessions.id = channels.session_id
), 0);

CREATE INDEX idx_channels_feed_session
ON channels (feed_id, session_started_at DESC, first_seen_at ASC);

CREATE INDEX idx_messages_feed_session_channel_cursor
ON messages (feed_id, session_id, channel_key, id);

CREATE INDEX idx_member_events_feed_session_channel_cursor
ON member_events (feed_id, session_id, channel_key, id);
