-- Portrait atlas coordinates for roster members: the cell (t = atlas sheet,
-- o = offset) in the app's own portrait atlases, sent by the client as users
-- resolve. Null means the placeholder.
ALTER TABLE roster ADD COLUMN portrait_table INTEGER;
ALTER TABLE roster ADD COLUMN portrait_offset INTEGER;
