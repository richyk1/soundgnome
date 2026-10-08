-- Re-bucket SoundCloud DRM tracks that were mislabeled as metadata match/no-match.
--
-- When a SoundCloud download is DRM-blocked the track keeps no staged audio file
-- (file_path IS NULL). If enrichment had already flagged the track as a weak
-- metadata match, the DRM reason was suppressed, so these file-less tracks were
-- stranded under "Partial match" / "No match" where they can never be approved:
-- Select there only re-tags an existing staged file, but there is none, and no
-- download URL is supplied. They belong in the DRM tab, where a YouTube source can
-- be picked to actually fetch the audio.
--
-- A file-less needs_validation track can only originate from the DRM branch of the
-- download workflow, so this signature uniquely identifies the affected rows.
UPDATE track
SET validation_reason = 'soundcloud_drm_protected'
WHERE needs_validation = 1
  AND file_path IS NULL
  AND validation_reason IN ('metadata_partial_match', 'metadata_no_match');
