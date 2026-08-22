use domain::ports::repositories::TrackRepository;

use diesel::prelude::*;
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use shared::{
    models::{Reference, Track},
    types::SoundgnomeResult,
};

use crate::{
    delete_with_relations,
    entities::{
        AlbumEntity, ArtistEntity, NewTrackEntity, NewTrackRefEntity, TrackEntity, TrackRefEntity,
        UpdateTrackEntity,
    },
    schema,
};

/// Attach albums, artists, and references to a batch of track rows with three
/// bulk queries, instead of the per-row N+1 fan-out that made large lists (the
/// Validations page with ~1k pending tracks) slow.
fn hydrate_tracks(
    conn: &mut SqliteConnection,
    tracks: Vec<TrackEntity>,
) -> SoundgnomeResult<Vec<Track>> {
    use std::collections::HashMap;

    if tracks.is_empty() {
        return Ok(Vec::new());
    }

    let track_ids: Vec<i32> = tracks.iter().map(|t| t.id).collect();
    let album_ids: Vec<i32> = tracks.iter().filter_map(|t| t.album_id).collect();

    let albums: HashMap<i32, AlbumEntity> = schema::album::table
        .filter(schema::album::id.eq_any(&album_ids))
        .load::<AlbumEntity>(conn)
        .map_err(|err| shared::errors::Error::Database(format!("Failed to load albums: {}", err)))?
        .into_iter()
        .map(|a| (a.id, a))
        .collect();

    let artist_rows: Vec<(i32, ArtistEntity)> = schema::artist_tracks::table
        .inner_join(
            schema::artist::table.on(schema::artist_tracks::artist_id.eq(schema::artist::id)),
        )
        .filter(schema::artist_tracks::track_id.eq_any(&track_ids))
        .select((schema::artist_tracks::track_id, schema::artist::all_columns))
        .load::<(i32, ArtistEntity)>(conn)
        .map_err(|err| {
            shared::errors::Error::Database(format!("Failed to load artists: {}", err))
        })?;
    let mut artists_by_track: HashMap<i32, Vec<ArtistEntity>> = HashMap::new();
    for (tid, artist) in artist_rows {
        artists_by_track.entry(tid).or_default().push(artist);
    }

    let ref_rows: Vec<TrackRefEntity> = schema::track_ref::table
        .filter(schema::track_ref::track_id.eq_any(&track_ids))
        .load::<TrackRefEntity>(conn)
        .map_err(|err| {
            shared::errors::Error::Database(format!("Failed to load references: {}", err))
        })?;
    let mut refs_by_track: HashMap<i32, Vec<TrackRefEntity>> = HashMap::new();
    for r in ref_rows {
        refs_by_track.entry(r.track_id).or_default().push(r);
    }

    let mut result = Vec::with_capacity(tracks.len());
    for track in tracks {
        let album = track.album_id.and_then(|aid| albums.get(&aid).cloned());
        let artists = artists_by_track.remove(&track.id).unwrap_or_default();
        let references = refs_by_track.remove(&track.id).unwrap_or_default();
        result.push(TrackEntity::convert_to_domain(
            track, album, artists, references,
        ));
    }
    Ok(result)
}
#[derive(Default)]
pub struct DieselTrackRepository {}

impl DieselTrackRepository {
    pub fn new() -> Self {
        Self {}
    }
}

impl TrackRepository for DieselTrackRepository {
    // =================================================================================
    // Custom
    // =================================================================================

    fn get_recent(&self, conn: &mut SqliteConnection, limit: i64) -> SoundgnomeResult<Vec<Track>> {
        let tracks: Vec<TrackEntity> = schema::track::table
            .order(schema::track::id.desc())
            .limit(limit)
            .load(conn)
            .map_err(|err| {
                shared::errors::Error::Database(format!("Failed to get recent tracks: {}", err))
            })?;

        hydrate_tracks(conn, tracks)
    }

    fn get_pending_validations(&self, conn: &mut SqliteConnection) -> SoundgnomeResult<Vec<Track>> {
        let tracks: Vec<TrackEntity> = schema::track::table
            .filter(schema::track::needs_validation.eq(true))
            .load(conn)
            .map_err(|err| {
                shared::errors::Error::Database(format!(
                    "Failed to get pending validations: {}",
                    err
                ))
            })?;

        hydrate_tracks(conn, tracks)
    }

    fn get_by_url(&self, conn: &mut SqliteConnection, url: &str) -> SoundgnomeResult<Track> {
        let track_ref = schema::track_ref::table
            .filter(schema::track_ref::external_url.eq(url))
            .first::<TrackRefEntity>(conn)
            .map_err(|err| {
                shared::errors::Error::Database(format!("Failed to get resource by url: {}", err))
            })?;

        self.get_by_id(conn, track_ref.track_id)
    }

    fn set_rating(
        &self,
        conn: &mut SqliteConnection,
        id: i32,
        rating: Option<shared::models::Rating>,
    ) -> SoundgnomeResult<()> {
        let value: Option<String> = rating.map(|r| r.as_db().to_string());
        diesel::update(schema::track::table.find(id))
            .set(schema::track::rating.eq(value))
            .execute(conn)
            .map_err(|err| {
                shared::errors::Error::Database(format!("Failed to set track rating: {}", err))
            })?;
        Ok(())
    }

    fn get_ratings(
        &self,
        conn: &mut SqliteConnection,
    ) -> SoundgnomeResult<Vec<(i32, shared::models::Rating)>> {
        let rows: Vec<(i32, Option<String>)> = schema::track::table
            .filter(schema::track::rating.is_not_null())
            .select((schema::track::id, schema::track::rating))
            .load::<(i32, Option<String>)>(conn)
            .map_err(|err| {
                shared::errors::Error::Database(format!("Failed to load ratings: {}", err))
            })?;
        Ok(rows
            .into_iter()
            .filter_map(|(id, r)| {
                r.and_then(|s| shared::models::Rating::from_db(&s)).map(|r| (id, r))
            })
            .collect())
    }

    fn fingerprint_candidates(
        &self,
        conn: &mut SqliteConnection,
        min_secs: i32,
        max_secs: i32,
    ) -> SoundgnomeResult<Vec<(i32, String)>> {
        // Encoded fingerprints are stored as Metadata references whose URL carries
        // this prefix (see download_service::CHROMAPRINT_PREFIX). Narrow by track
        // duration so only acoustically plausible candidates are compared; rows with
        // an unknown duration are always included.
        let rows: Vec<(i32, Option<String>)> = schema::track_ref::table
            .inner_join(schema::track::table)
            .filter(schema::track_ref::external_url.like("soundome:chromaprint:%"))
            .filter(
                schema::track::duration
                    .between(min_secs, max_secs)
                    .or(schema::track::duration.is_null()),
            )
            .select((schema::track_ref::track_id, schema::track_ref::external_url))
            .load::<(i32, Option<String>)>(conn)
            .map_err(|err| {
                shared::errors::Error::Database(format!(
                    "Failed to load fingerprint candidates: {}",
                    err
                ))
            })?;

        Ok(rows
            .into_iter()
            .filter_map(|(id, url)| url.map(|u| (id, u)))
            .collect())
    }

    fn create_references(
        &self,
        conn: &mut SqliteConnection,
        track_id: i32,
        references: &[Reference],
    ) -> SoundgnomeResult<()> {
        for reference in references {
            let new_track_ref = NewTrackRefEntity::convert_from_domain(reference, track_id);

            diesel::insert_into(schema::track_ref::table)
                .values(&new_track_ref)
                .execute(conn)
                .map_err(|err| {
                    shared::errors::Error::Database(format!(
                        "Failed to create track reference: {}",
                        err
                    ))
                })?;
        }
        Ok(())
    }

    fn set_references(
        &self,
        conn: &mut SqliteConnection,
        track_id: i32,
        references: &[Reference],
    ) -> SoundgnomeResult<()> {
        // Semantics:
        // - Source/Provider: replace (ensure single row): delete existing of that type then insert.
        // - Metadata/Reference: merge (insert missing only), preserving existing ids.
        if references.is_empty() {
            return Ok(());
        }

        // Handle Source and Provider replacement first
        for reference in references {
            let ref_type = reference.ref_type.as_ref().to_string().to_lowercase();
            if ref_type == "source" || ref_type == "provider" {
                diesel::delete(
                    schema::track_ref::table
                        .filter(schema::track_ref::track_id.eq(track_id))
                        .filter(schema::track_ref::type_.eq(&ref_type)),
                )
                .execute(conn)
                .map_err(|err| {
                    shared::errors::Error::Database(format!(
                        "Failed to replace track {} reference: {}",
                        ref_type, err
                    ))
                })?;

                if reference.external_id.is_none() && reference.external_url.is_none() {
                    continue;
                }

                let new_track_ref = NewTrackRefEntity::convert_from_domain(reference, track_id);
                diesel::insert_into(schema::track_ref::table)
                    .values(&new_track_ref)
                    .execute(conn)
                    .map_err(|err| {
                        shared::errors::Error::Database(format!(
                            "Failed to create track {} reference: {}",
                            ref_type, err
                        ))
                    })?;
            }
        }

        // Then merge everything else
        let existing: Vec<TrackRefEntity> = schema::track_ref::table
            .filter(schema::track_ref::track_id.eq(track_id))
            .load(conn)
            .map_err(|err| {
                shared::errors::Error::Database(format!("Failed to load track references: {}", err))
            })?;

        for reference in references {
            let ref_type = reference.ref_type.as_ref().to_string().to_lowercase();
            if ref_type == "source" || ref_type == "provider" {
                continue;
            }
            if reference.external_id.is_none() && reference.external_url.is_none() {
                continue;
            }

            let platform = reference.platform.as_ref().to_string().to_lowercase();

            let already_exists = existing.iter().any(|r| {
                r.ref_type.to_lowercase() == ref_type
                    && r.platform.to_lowercase() == platform
                    && r.external_id == reference.external_id
                    && r.external_url == reference.external_url
            });

            if !already_exists {
                let new_track_ref = NewTrackRefEntity::convert_from_domain(reference, track_id);
                diesel::insert_into(schema::track_ref::table)
                    .values(&new_track_ref)
                    .execute(conn)
                    .map_err(|err| {
                        shared::errors::Error::Database(format!(
                            "Failed to create track reference: {}",
                            err
                        ))
                    })?;
            }
        }

        Ok(())
    }

    // fn find_by_unique_fields(&self, conn: &mut SqliteConnection, track: &Track) -> SoundgnomeResult<Option<Track>> {
    //     use diesel::prelude::*;
    //     use crate::schema;
    //     use crate::schema::track::dsl::*;
    //     let mut query = track.into_boxed();
    //     query = query.filter(title.eq(&track.));
    //     if let Some(album) = &track.album {
    //         if let Some(album_id_val) = album.id {
    //             query = query.filter(album_id.eq(album_id_val));
    //         }
    //     }
    //     let found: Option<TrackEntity> = query
    //         .first::<TrackEntity>(conn)
    //         .optional()
    //         .map_err(|err| shared::errors::Error::Database(format!("Failed to find track by unique fields: {}", err)))?;
    //     if let Some(entity) = found {
    //         let album = super::album::find_one(conn, entity.album_id.unwrap_or_default()).ok();
    //         let artists: Vec<ArtistEntity> = schema::artist_tracks::table
    //             .inner_join(schema::artist::table.on(schema::artist_tracks::artist_id.eq(schema::artist::id)))
    //             .filter(schema::artist_tracks::track_id.eq(entity.id))
    //             .select(schema::artist::all_columns)
    //             .load(conn)
    //             .unwrap_or_default();
    //         let references: Vec<TrackRefEntity> = schema::track_ref::table
    //             .filter(schema::track_ref::track_id.eq(entity.id))
    //             .load(conn)
    //             .unwrap_or_default();
    //         Ok(Some(TrackEntity::convert_to_domain(entity, album, artists, references)))
    //     } else {
    //         Ok(None)
    //     }
    // }

    // =================================================================================
    // CRUD
    // =================================================================================

    fn get_by_id(&self, conn: &mut SqliteConnection, id: i32) -> SoundgnomeResult<Track> {
        let (track, album): (TrackEntity, Option<AlbumEntity>) = schema::track::table
            .left_join(
                schema::album::table.on(schema::album::id.nullable().eq(schema::track::album_id)),
            )
            .filter(schema::track::id.eq(id))
            .first(conn)
            .map_err(|err| {
                shared::errors::Error::Database(format!("Failed to get resource by id: {}", err))
            })?;

        let artists: Vec<ArtistEntity> = schema::artist_tracks::table
            .inner_join(
                schema::artist::table.on(schema::artist_tracks::artist_id.eq(schema::artist::id)),
            )
            .filter(schema::artist_tracks::track_id.eq(track.id))
            .select(schema::artist::all_columns)
            .load(conn)
            .map_err(|err| {
                shared::errors::Error::Database(format!("Failed to get resource by id: {}", err))
            })?;

        let references: Vec<TrackRefEntity> = schema::track_ref::table
            .filter(schema::track_ref::track_id.eq(track.id))
            .load(conn)
            .map_err(|err| {
                shared::errors::Error::Database(format!("Failed to get resource by id: {}", err))
            })?;

        Ok(TrackEntity::convert_to_domain(
            track, album, artists, references,
        ))
    }

    fn get_all(&self, conn: &mut SqliteConnection) -> SoundgnomeResult<Vec<Track>> {
        let tracks: Vec<TrackEntity> = schema::track::table.load(conn).map_err(|err| {
            shared::errors::Error::Database(format!("Failed to get all resources: {}", err))
        })?;

        hydrate_tracks(conn, tracks)
    }

    fn create(&self, conn: &mut SqliteConnection, new_track: &Track) -> SoundgnomeResult<Track> {
        let new_track_entity = NewTrackEntity::convert_from_domain(new_track);
        let inserted_track = diesel::insert_into(schema::track::table)
            .values(&new_track_entity)
            .execute(conn)
            .and_then(|_| {
                schema::track::table
                    .order(schema::track::id.desc())
                    .first::<TrackEntity>(conn)
            })
            .map_err(|err| {
                shared::errors::Error::Database(format!("Failed to create resource: {}", err))
            })?;

        Ok(TrackEntity::convert_to_domain(
            inserted_track,
            None,
            vec![],
            vec![],
        ))
    }

    fn update(
        &self,
        conn: &mut SqliteConnection,
        id: i32,
        updated_track: &Track,
    ) -> SoundgnomeResult<Track> {
        let updated_track_entity = UpdateTrackEntity::convert_from_domain(updated_track);
        let updated_track = diesel::update(schema::track::table.filter(schema::track::id.eq(id)))
            .set(&updated_track_entity)
            .execute(conn)
            .and_then(|_| {
                schema::track::table
                    .filter(schema::track::id.eq(id))
                    .first::<TrackEntity>(conn)
            })
            .map_err(|err| {
                shared::errors::Error::Database(format!("Failed to update resource: {}", err))
            })?;

        Ok(TrackEntity::convert_to_domain(
            updated_track,
            None,
            vec![],
            vec![],
        ))
    }

    fn delete(&self, conn: &mut SqliteConnection, id: i32) -> SoundgnomeResult<()> {
        delete_with_relations!(
            conn,
            id,
            [
                (
                    schema::track_ref::table,
                    schema::track_ref::track_id,
                    "Failed to delete associated track references"
                ),
                (
                    schema::artist_tracks::table,
                    schema::artist_tracks::track_id,
                    "Failed to delete associated artist-track relationships"
                ),
                (
                    schema::track::table,
                    schema::track::id,
                    "Failed to delete resource"
                ),
            ]
        )?;
        Ok(())
    }

    fn count(&self, conn: &mut SqliteConnection) -> SoundgnomeResult<i64> {
        schema::track::table
            .count()
            .get_result(conn)
            .map_err(|err| {
                shared::errors::Error::Database(format!("Failed to count tracks: {}", err))
            })
    }

    fn count_pending_validations(&self, conn: &mut SqliteConnection) -> SoundgnomeResult<i64> {
        schema::track::table
            .filter(schema::track::needs_validation.eq(true))
            .count()
            .get_result(conn)
            .map_err(|err| {
                shared::errors::Error::Database(format!(
                    "Failed to count pending validations: {}",
                    err
                ))
            })
    }

    fn count_by_file_path(
        &self,
        conn: &mut SqliteConnection,
        file_path: &str,
        exclude_id: Option<i32>,
    ) -> SoundgnomeResult<i64> {
        let mut query = schema::track::table
            .filter(schema::track::file_path.eq(file_path))
            .into_boxed();
        if let Some(id) = exclude_id {
            query = query.filter(schema::track::id.ne(id));
        }
        query.count().get_result(conn).map_err(|err| {
            shared::errors::Error::Database(format!("Failed to count tracks by file_path: {}", err))
        })
    }

    fn get_by_soundome_id(
        &self,
        conn: &mut SqliteConnection,
        soundome_id: &str,
    ) -> SoundgnomeResult<Option<Track>> {
        let track: Option<TrackEntity> = schema::track::table
            .filter(schema::track::soundome_id.eq(soundome_id))
            .first::<TrackEntity>(conn)
            .optional()
            .map_err(|err| {
                shared::errors::Error::Database(format!(
                    "Failed to get track by soundome_id: {}",
                    err
                ))
            })?;

        let Some(track) = track else {
            return Ok(None);
        };

        let album = if let Some(album_id) = track.album_id {
            schema::album::table
                .filter(schema::album::id.eq(album_id))
                .first::<AlbumEntity>(conn)
                .ok()
        } else {
            None
        };

        let artists: Vec<ArtistEntity> = schema::artist_tracks::table
            .inner_join(
                schema::artist::table.on(schema::artist_tracks::artist_id.eq(schema::artist::id)),
            )
            .filter(schema::artist_tracks::track_id.eq(track.id))
            .select(schema::artist::all_columns)
            .load(conn)
            .map_err(|err| {
                shared::errors::Error::Database(format!(
                    "Failed to get artists for track by soundome_id: {}",
                    err
                ))
            })?;

        let references: Vec<TrackRefEntity> = schema::track_ref::table
            .filter(schema::track_ref::track_id.eq(track.id))
            .load(conn)
            .map_err(|err| {
                shared::errors::Error::Database(format!(
                    "Failed to get references for track by soundome_id: {}",
                    err
                ))
            })?;

        Ok(Some(TrackEntity::convert_to_domain(
            track, album, artists, references,
        )))
    }

    fn get_all_finalized(&self, conn: &mut SqliteConnection) -> SoundgnomeResult<Vec<Track>> {
        let tracks: Vec<TrackEntity> = schema::track::table
            .filter(schema::track::file_path.is_not_null())
            .load(conn)
            .map_err(|err| {
                shared::errors::Error::Database(format!("Failed to get finalized tracks: {}", err))
            })?;

        hydrate_tracks(conn, tracks)
    }

    fn delete_reference(&self, conn: &mut SqliteConnection, ref_id: i32) -> SoundgnomeResult<()> {
        diesel::delete(schema::track_ref::table.filter(schema::track_ref::id.eq(ref_id)))
            .execute(conn)
            .map_err(|err| {
                shared::errors::Error::Database(format!(
                    "Failed to delete track reference {}: {}",
                    ref_id, err
                ))
            })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use diesel::connection::SimpleConnection;
    use diesel_migrations::MigrationHarness;

    fn migrated_conn() -> SqliteConnection {
        let mut conn = SqliteConnection::establish(":memory:").unwrap();
        conn.run_pending_migrations(crate::MIGRATIONS).unwrap();
        conn
    }

    #[test]
    fn count_by_file_path_detects_rows_sharing_one_file() {
        let mut conn = migrated_conn();
        conn.batch_execute(
            "INSERT INTO track (title, needs_validation, file_path) VALUES ('shared', 0, '/tmp/shared.m4a');\
             INSERT INTO track (title, needs_validation, file_path) VALUES ('shared', 1, '/tmp/shared.m4a');\
             INSERT INTO track (title, needs_validation, file_path) VALUES ('solo', 0, '/tmp/solo.m4a');",
        )
        .unwrap();
        let repo = DieselTrackRepository::new();

        // Two rows point at the same physical file.
        assert_eq!(
            repo.count_by_file_path(&mut conn, "/tmp/shared.m4a", None)
                .unwrap(),
            2
        );
        // Deleting row 2 (the loser) still leaves row 1 holding the file, so the
        // guard sees a positive count and must preserve it - this is the exact
        // case that orphaned a keeper before the fix.
        assert_eq!(
            repo.count_by_file_path(&mut conn, "/tmp/shared.m4a", Some(2))
                .unwrap(),
            1
        );
        // A uniquely-referenced file has no other holder, so deletion is safe.
        assert_eq!(
            repo.count_by_file_path(&mut conn, "/tmp/solo.m4a", Some(3))
                .unwrap(),
            0
        );
    }
}
