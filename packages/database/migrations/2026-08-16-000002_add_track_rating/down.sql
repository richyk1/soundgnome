-- Remove the user-facing curation flag when reverting this migration.
ALTER TABLE track DROP COLUMN rating;
