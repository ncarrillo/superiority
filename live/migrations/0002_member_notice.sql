-- Member join/leave events always arrive (the roster is built from them);
-- `notice` records whether the sharer's app showed the event as a chat line
-- ("Join / leave notifications" setting), so the viewer's transcript can
-- honor the same choice without starving the roster.
ALTER TABLE member_events ADD COLUMN notice INTEGER NOT NULL DEFAULT 1;
