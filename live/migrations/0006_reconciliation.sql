ALTER TABLE sessions ADD COLUMN synced_at INTEGER;
ALTER TABLE sessions ADD COLUMN ended_at INTEGER;
ALTER TABLE sessions ADD COLUMN sync_seq INTEGER NOT NULL DEFAULT 0;

UPDATE sessions
SET synced_at = last_seen_at
WHERE EXISTS (
  SELECT 1 FROM channels
  WHERE channels.feed_id = sessions.feed_id
    AND channels.session_id = sessions.id
);

CREATE TABLE channels_next (
  feed_id           TEXT NOT NULL,
  session_id        TEXT NOT NULL,
  key               TEXT NOT NULL,
  kind              TEXT NOT NULL,
  name              TEXT NOT NULL DEFAULT '',
  first_seen_at     INTEGER NOT NULL,
  last_event_at     INTEGER NOT NULL,
  closed_at         INTEGER,
  last_seq          INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (feed_id, session_id, key)
);

INSERT INTO channels_next
  (feed_id, session_id, key, kind, name, first_seen_at, last_event_at, closed_at)
SELECT feed_id, session_id, key, kind, name, first_seen_at, last_event_at, closed_at
FROM channels
WHERE session_id != '';

CREATE TABLE roster_next (
  feed_id           TEXT NOT NULL,
  session_id        TEXT NOT NULL,
  channel_key       TEXT NOT NULL,
  user_handle       INTEGER NOT NULL,
  name              TEXT,
  clan_tag          TEXT,
  presence          TEXT NOT NULL DEFAULT '',
  last_seq          INTEGER NOT NULL,
  updated_at        INTEGER NOT NULL,
  portrait_table    INTEGER,
  portrait_offset   INTEGER,
  PRIMARY KEY (feed_id, session_id, channel_key, user_handle)
);

INSERT INTO roster_next
  (feed_id, session_id, channel_key, user_handle, name, clan_tag, presence,
   last_seq, updated_at, portrait_table, portrait_offset)
SELECT feed_id, session_id, channel_key, user_handle, name, clan_tag, presence,
       last_seq, updated_at, portrait_table, portrait_offset
FROM roster
WHERE present = 1 AND session_id != '';

DROP TABLE channels;
ALTER TABLE channels_next RENAME TO channels;

DROP TABLE roster;
ALTER TABLE roster_next RENAME TO roster;

DROP TABLE member_events;
DROP TABLE whispers;
DROP TABLE friends_snapshots;
DROP TABLE raw_events;

DROP INDEX idx_sessions_feed_seen;
CREATE INDEX idx_sessions_feed_seen
ON sessions (feed_id, last_seen_at DESC, started_at DESC);
CREATE INDEX idx_sessions_feed_synced
ON sessions (feed_id, synced_at DESC, started_at DESC);

CREATE INDEX idx_channels_feed_session_open
ON channels (feed_id, session_id, closed_at, first_seen_at, key);

DROP INDEX idx_messages_feed_session_channel_cursor;

CREATE INDEX idx_roster_feed_session_channel_name
ON roster (feed_id, session_id, channel_key, name COLLATE NOCASE, user_handle);
