-- Product identity is part of the feed/session contract so viewers can route
-- to the correct presentation instead of assuming every feed is StarCraft II.
-- Existing rows predate multi-product publishing and are therefore SC2.
ALTER TABLE feeds ADD COLUMN product TEXT NOT NULL DEFAULT 'sc2';
ALTER TABLE sessions ADD COLUMN product TEXT NOT NULL DEFAULT 'sc2';
