-- Per-track like/dislike curation flag, set by the user in the web UI.
-- Nullable: existing rows and unrated tracks are NULL. Values: 'liked' | 'disliked'.
-- Kept off the metadata pipeline (fetch/tag/organize) since it is purely a
-- user-facing curation signal, not audio metadata.
ALTER TABLE track ADD COLUMN rating TEXT;
