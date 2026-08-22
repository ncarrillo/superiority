-- The transcript carries more than chat now: messages.kind says what a row
-- is ('talk' | 'emote' | 'broadcast' | 'information' | 'member_joined' |
-- 'member_left'). Rows written before this migration were all plain chat.
-- Roster identity grows the SC:R-era fields: a free-form avatar id and the
-- channel-operator flag.
ALTER TABLE messages ADD COLUMN kind TEXT NOT NULL DEFAULT 'talk';
ALTER TABLE roster ADD COLUMN avatar TEXT;
ALTER TABLE roster ADD COLUMN is_operator INTEGER NOT NULL DEFAULT 0;
