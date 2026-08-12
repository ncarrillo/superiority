-- Superiority Live v1. All timestamps are unix milliseconds.
--
-- Every table is partitioned by feed_id: a feed is one registered install of
-- the app, and its secret token is the only thing that can write into it.
-- Client retries are at-least-once, so idempotency lives here: append tables
-- are UNIQUE(feed_id, session_id, seq) + INSERT OR IGNORE
-- (feed_id included so one feed can never mask another feed's rows), and the
-- roster is a current-state table with a last-write-wins guard.

CREATE TABLE feeds (
  id             TEXT PRIMARY KEY,          -- public slug, part of the live link
  token_hash     TEXT NOT NULL UNIQUE,      -- sha256 hex of the secret token
  client_version TEXT NOT NULL DEFAULT '',
  created_at     INTEGER NOT NULL,
  last_seen_at   INTEGER NOT NULL,
  revoked        INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE sessions (
  feed_id        TEXT NOT NULL,
  id             TEXT NOT NULL,             -- client-random per Battle.net connection
  client_version TEXT NOT NULL DEFAULT '',
  started_at     INTEGER NOT NULL,
  last_seen_at   INTEGER NOT NULL,
  last_seq       INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (feed_id, id)
);
CREATE INDEX idx_sessions_feed_seen ON sessions (feed_id, last_seen_at DESC);

CREATE TABLE channels (
  feed_id        TEXT NOT NULL,
  key            TEXT NOT NULL,             -- 'public:1033' | 'private:Op Test' | 'club:5322'
  kind           TEXT NOT NULL,             -- 'public' | 'private' | 'club'
  name           TEXT NOT NULL DEFAULT '',
  first_seen_at  INTEGER NOT NULL,
  last_event_at  INTEGER NOT NULL,
  PRIMARY KEY (feed_id, key)
);
CREATE INDEX idx_channels_feed_recent ON channels (feed_id, last_event_at DESC);

CREATE TABLE messages (
  id             INTEGER PRIMARY KEY,       -- rowid alias: the poll cursor
  feed_id        TEXT NOT NULL,
  session_id     TEXT NOT NULL,
  seq            INTEGER NOT NULL,
  channel_key    TEXT NOT NULL,
  ts             INTEGER NOT NULL,
  sender_handle  INTEGER NOT NULL,
  sender_name    TEXT,
  sender_clan    TEXT,
  body           TEXT NOT NULL,
  UNIQUE (feed_id, session_id, seq)
);
CREATE INDEX idx_messages_feed_channel_cursor ON messages (feed_id, channel_key, id);

CREATE TABLE member_events (
  id             INTEGER PRIMARY KEY,
  feed_id        TEXT NOT NULL,
  session_id     TEXT NOT NULL,
  seq            INTEGER NOT NULL,
  channel_key    TEXT NOT NULL,
  ts             INTEGER NOT NULL,
  kind           TEXT NOT NULL,             -- 'joined' | 'left'
  user_handle    INTEGER NOT NULL,
  user_name      TEXT,
  user_clan      TEXT,
  UNIQUE (feed_id, session_id, seq)
);
CREATE INDEX idx_member_events_feed_channel_cursor ON member_events (feed_id, channel_key, id);

-- Current channel membership. present=0 rows are tombstones kept so a stale
-- redelivered 'member_joined' cannot resurrect someone who already left.
CREATE TABLE roster (
  feed_id        TEXT NOT NULL,
  channel_key    TEXT NOT NULL,
  user_handle    INTEGER NOT NULL,
  name           TEXT,
  clan_tag       TEXT,
  presence       TEXT NOT NULL DEFAULT '',
  present        INTEGER NOT NULL DEFAULT 1,
  session_id     TEXT NOT NULL,             -- session that last wrote the row
  last_seq       INTEGER NOT NULL,          -- that session's seq (in-session LWW)
  updated_at     INTEGER NOT NULL,          -- event ts (cross-session LWW)
  PRIMARY KEY (feed_id, channel_key, user_handle)
);
CREATE INDEX idx_roster_feed_present ON roster (feed_id, channel_key, present, name);

-- ── Social: opt-in data. No read endpoint touches these tables in v1. ───────

CREATE TABLE whispers (
  id             INTEGER PRIMARY KEY,
  feed_id        TEXT NOT NULL,
  session_id     TEXT NOT NULL,
  seq            INTEGER NOT NULL,
  ts             INTEGER NOT NULL,
  peer           TEXT NOT NULL,
  outgoing       INTEGER NOT NULL,          -- 0 | 1
  body           TEXT NOT NULL,
  UNIQUE (feed_id, session_id, seq)
);
CREATE INDEX idx_whispers_feed_ts ON whispers (feed_id, ts);

CREATE TABLE friends_snapshots (
  id             INTEGER PRIMARY KEY,
  feed_id        TEXT NOT NULL,
  session_id     TEXT NOT NULL,
  seq            INTEGER NOT NULL,
  ts             INTEGER NOT NULL,
  friends_json   TEXT NOT NULL,
  UNIQUE (feed_id, session_id, seq)
);
CREATE INDEX idx_friends_feed_ts ON friends_snapshots (feed_id, ts);

-- ── Forward compatibility: kinds this Worker version doesn't understand. ────

CREATE TABLE raw_events (
  id             INTEGER PRIMARY KEY,
  feed_id        TEXT NOT NULL,
  session_id     TEXT NOT NULL,
  seq            INTEGER NOT NULL,
  ts             INTEGER NOT NULL,
  kind           TEXT NOT NULL,
  payload_json   TEXT NOT NULL,
  UNIQUE (feed_id, session_id, seq)
);
CREATE INDEX idx_raw_events_feed_kind ON raw_events (feed_id, kind, id);
