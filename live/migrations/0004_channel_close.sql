-- The local user leaving a channel closes it (kind "left"); any newer
-- activity on the same key reopens it. Null means open.
ALTER TABLE channels ADD COLUMN closed_at INTEGER;
