-- Un-flag finalized library tracks that were wrongly re-flagged for validation.
--
-- A re-sync of a source already in the library re-derived a partial/no-match from
-- the raw source metadata and folded `needs_validation` onto the finalized row.
-- A track with a `soundome_id` is a finalized, user-reviewed library entry and
-- must never sit in the validation queue, so clear the flag for those rows.
UPDATE track
SET needs_validation = 0, validation_reason = NULL
WHERE needs_validation = 1
  AND soundome_id IS NOT NULL;
