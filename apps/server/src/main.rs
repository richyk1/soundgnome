use std::sync::Arc;

use config::Config;
use database::repositories;
use domain::{ports::repositories::RepositoryLayer, services::ServiceLayer};
use rocket::{catchers, fs::FileServer, launch, routes};
use rocket_okapi::{
    openapi_get_routes,
    swagger_ui::{make_swagger_ui, SwaggerUIConfig},
};

use shared::{init_globals, utils::logs::init_logger};
use soundgnome_server::utils::{
    cancellation::CancellationRegistry, database::Db, task_executor::TaskExecutor,
};
use soundgnome_server::{
    middlewares::{cache_control::CacheControl, cors::Cors},
    routes::{self, errors},
};

fn get_docs() -> SwaggerUIConfig {
    SwaggerUIConfig {
        url: "../api/openapi.json".to_string(),
        ..Default::default()
    }
}

/// Precompute and cache waveform peaks for every finalized track not already
/// cached, so the scrubber is instant on first play. Deliberately gentle: starts
/// after a delay and decodes one track at a time with a breather between each, so
/// ffmpeg never contends with foreground audio/waveform requests on a busy or
/// shared box. Anything played before the backfill reaches it is computed on
/// demand instead, so a slow warm-up costs nothing.
fn backfill_waveforms(services: &ServiceLayer, db_url: &str) {
    use std::time::Duration;

    // Let the server settle: a reload + play right after launch must not compete
    // with the initial decode work.
    std::thread::sleep(Duration::from_secs(20));

    let mut conn = database::init_connection(db_url);
    let tracks = match services.track_service.get_all_finalized(&mut conn) {
        Ok(tracks) => tracks,
        Err(e) => {
            tracing::warn!("Waveform backfill: could not list tracks: {e}");
            return;
        }
    };
    drop(conn);

    // Only decode what is missing; a stat per track is cheap on later runs.
    let pending: Vec<(i32, std::path::PathBuf)> = tracks
        .into_iter()
        .filter_map(|t| Some((t.id?, t.file_path?)))
        .filter(|(id, path)| !routes::tracks::waveform_is_cached(*id, path))
        .collect();
    if pending.is_empty() {
        tracing::info!("Waveform backfill: all tracks already cached");
        return;
    }

    let total = pending.len();
    let mut ok = 0usize;
    for (id, path) in &pending {
        match routes::tracks::waveform_peaks_cached(*id, path) {
            Ok(_) => ok += 1,
            Err(e) => tracing::debug!("Waveform backfill: track {id} failed: {e}"),
        }
        // One decode at a time, with a breather, keeps a single ffmpeg well below
        // saturation and yields the CPU to playback between tracks.
        std::thread::sleep(Duration::from_millis(150));
    }

    tracing::info!("Waveform backfill complete: {ok}/{total} newly cached");
}

/// Precompute and cache the small cover thumbnail for every track with a local
/// file that carries embedded art, so the Library and Validations lists (which
/// render hundreds of covers at once) show artwork instantly instead of pulling
/// the multi-megabyte raw pictures. Gentle like the waveform backfill: a delayed
/// start and a breather between tracks so image decoding never contends with
/// foreground requests. Anything viewed first is computed on demand and cached.
fn backfill_covers(services: &ServiceLayer, db_url: &str) {
    use std::time::Duration;

    // Start after the waveform backfill has a head start so the two never
    // saturate the CPU together right after launch.
    std::thread::sleep(Duration::from_secs(30));

    let mut conn = database::init_connection(db_url);
    let tracks = match services.track_service.get_all(&mut conn) {
        Ok(tracks) => tracks,
        Err(e) => {
            tracing::warn!("Cover backfill: could not list tracks: {e}");
            return;
        }
    };
    drop(conn);

    let px = routes::tracks::COVER_THUMB_PX;
    // Only build what is missing; a stat per track is cheap on later runs.
    let pending: Vec<(i32, std::path::PathBuf)> = tracks
        .into_iter()
        .filter_map(|t| Some((t.id?, t.file_path?)))
        .filter(|(id, path)| !routes::tracks::cover_is_cached(*id, path, px))
        .collect();
    if pending.is_empty() {
        tracing::info!("Cover backfill: all tracks already cached");
        return;
    }

    let total = pending.len();
    let mut ok = 0usize;
    for (id, path) in &pending {
        if routes::tracks::cover_cached(*id, path, px).is_some() {
            ok += 1;
        }
        std::thread::sleep(Duration::from_millis(120));
    }

    tracing::info!("Cover backfill complete: {ok}/{total} newly cached");
}

/// Precompute and cache each track's integrated loudness so the player can
/// normalize volume from the first play instead of decoding the file on demand.
/// Gentle like the other backfills: a delayed start and a breather between tracks
/// so ffmpeg never contends with foreground playback.
fn backfill_loudness(services: &ServiceLayer, db_url: &str) {
    use std::time::Duration;

    // Start last of the three backfills so they never saturate the CPU together.
    std::thread::sleep(Duration::from_secs(40));

    let mut conn = database::init_connection(db_url);
    let tracks = match services.track_service.get_all(&mut conn) {
        Ok(tracks) => tracks,
        Err(e) => {
            tracing::warn!("Loudness backfill: could not list tracks: {e}");
            return;
        }
    };
    drop(conn);

    let pending: Vec<(i32, std::path::PathBuf)> = tracks
        .into_iter()
        .filter_map(|t| Some((t.id?, t.file_path?)))
        .filter(|(id, path)| !routes::tracks::loudness_is_cached(*id, path))
        .collect();
    if pending.is_empty() {
        tracing::info!("Loudness backfill: all tracks already measured");
        return;
    }

    let total = pending.len();
    let mut ok = 0usize;
    for (id, path) in &pending {
        if routes::tracks::loudness_cached(*id, path).is_some() {
            ok += 1;
        }
        std::thread::sleep(Duration::from_millis(150));
    }

    tracing::info!("Loudness backfill complete: {ok}/{total} newly measured");
}

#[dotenvy::load(path = "./.env", required = false)]
#[launch]
fn rocket() -> _ {
    init_globals().unwrap_or_else(|err| {
        eprintln!("Failed to initialize globals: {}", err);
        std::process::exit(1);
    });

    init_logger();

    tracing::info!("Starting server...");

    // Mint Spotify Web API tokens from the librespot session instead of a
    // fragile OAuth refresh token (avoids Spotify revoking it).
    downloader::spotify::auth::register_token_minter();

    // Read Liked Songs natively via the librespot session (spclient collection)
    // instead of the throttle-prone /me/tracks Web API.
    downloader::spotify::register_liked_provider();

    // Initialize database and run migrations
    let db_url = Config::get().database.url.clone();
    if let Err(e) = database::init_database(&db_url) {
        tracing::error!("Failed to initialize database: {}", e);
        std::process::exit(1);
    }

    let track_repo = Arc::new(repositories::track::DieselTrackRepository::new());
    let album_repo = Arc::new(repositories::album::DieselAlbumRepository::new());
    let artist_repo = Arc::new(repositories::artist::DieselArtistRepository::new());
    let playlist_repo = Arc::new(repositories::playlist::DieselPlaylistRepository::new());
    let task_repo = Arc::new(repositories::task::DieselTaskRepository::new());
    let sync_schedule_repo =
        Arc::new(repositories::sync_schedule::DieselSyncScheduleRepository::new());

    let repositories = Arc::new(RepositoryLayer {
        track: track_repo.clone(),
        album: album_repo.clone(),
        artist: artist_repo.clone(),
        playlist: playlist_repo.clone(),
        task: task_repo.clone(),
        sync_schedule: sync_schedule_repo.clone(),
    });

    let services = Arc::new(ServiceLayer::new(repositories));
    let cancellation_registry = Arc::new(CancellationRegistry::new());
    // Start the serial task executor (single background worker). Every job that
    // needs the shared SQLite DB or long-running network I/O must be enqueued
    // here, so at most one runs at a time. See `utils/task_executor.rs`.
    let task_executor = Arc::new(TaskExecutor::start(
        services.clone(),
        cancellation_registry.clone(),
    ));

    // Automatic recovery of stale tasks (Pending/Running from previous run) is disabled.
    // Stale tasks can be retried manually via the /api/tasks/{id}/retry endpoint or the UI.
    // This ensures operators have full control over task resumption and prevents unexpected
    // behavior after server restarts.
    //
    // To re-enable automatic recovery, uncomment the block below and recompile:
    /*
    {
        let db_url = Config::get().database.url.clone();
        let conn = &mut database::init_connection(&db_url);
        match services.task_service.get_stale_running(conn) {
            Ok(stale_tasks) if !stale_tasks.is_empty() => {
                tracing::warn!(
                    "Found {} stale Running task(s) from previous run, re-enqueueing",
                    stale_tasks.len()
                );
                for task in stale_tasks {
                    let task_id = match task.id {
                        Some(id) => id,
                        None => continue,
                    };
                    let url = task.payload.clone();
                    let url = serde_json::from_str::<serde_json::Value>(&url)
                        .ok()
                        .and_then(|v| v.get("url")?.as_str().map(String::from));
                    let Some(url) = url else {
                        tracing::warn!("Task {} has no url in payload, marking as failed", task_id);
                        let _ =
                            services
                                .task_service
                                .set_failed(conn, task_id, "no url in payload");
                        continue;
                    };

                    if let Err(e) = services.task_service.reset_for_retry(conn, task_id) {
                        tracing::error!("Failed to reset task {} for retry: {}", task_id, e);
                        continue;
                    }

                    let cancel_flag = cancellation_registry.register(task_id);
                    tracing::info!("Re-enqueueing stale task {} for URL {}", task_id, url);
                    match task.task_type {
                        shared::models::TaskType::SyncArtist => {
                            task_executor.enqueue_artist_sync(task_id, url, cancel_flag);
                        }
                        shared::models::TaskType::SyncAlbum => {
                            task_executor.enqueue_album_sync(task_id, url, cancel_flag);
                        }
                        _ => {
                            task_executor.enqueue_playlist_sync(task_id, url, cancel_flag);
                        }
                    }
                }
            }
            Ok(_) => {} // no stale tasks
            Err(e) => tracing::error!("Failed to check for stale tasks at boot: {}", e),
        }
    }
    */

    // Maintenance backfills (fingerprint / artwork) cannot resume across a restart
    // and would otherwise wedge their Tools page showing "Running" forever. Mark any
    // left over from a previous run as failed so the page frees up; both are
    // idempotent and cheap to re-run from the button.
    {
        let db_url = Config::get().database.url.clone();
        let conn = &mut database::init_connection(&db_url);
        if let Ok(stale) = services.task_service.get_stale_running(conn) {
            for task in stale {
                let is_backfill = matches!(
                    task.task_type,
                    shared::models::TaskType::EmbedArtworkBackfill
                        | shared::models::TaskType::FingerprintBackfill
                );
                if is_backfill {
                    if let Some(id) = task.id {
                        let _ = services.task_service.set_failed(
                            conn,
                            id,
                            "Interrupted by a server restart",
                        );
                    }
                }
            }
        }
    }

    // Warm the audio-quality cache in the background. The library list probes every
    // track's file for its format/bitrate; on a cold cache that scales with the
    // library and is the dominant cost of the first Tracks page load. Doing it here
    // moves that work off the request path so the page is fast for the user.
    {
        let db_url = Config::get().database.url.clone();
        let services_for_warm = services.clone();
        std::thread::spawn(move || {
            let conn = &mut database::init_connection(&db_url);
            match services_for_warm.track_service.get_all_finalized(conn) {
                Ok(tracks) => {
                    for track in &tracks {
                        let _ = soundgnome_server::utils::quality_cache::probe(track);
                    }
                    tracing::info!("Quality cache warmed for {} track(s)", tracks.len());
                }
                Err(e) => tracing::warn!("Quality cache warm-up failed: {}", e),
            }
        });
    }

    // Spawn the background sync scheduler (checks every 60 seconds)
    {
        let db_url = Config::get().database.url.clone();
        let services_for_scheduler = services.clone();
        let registry_for_scheduler = cancellation_registry.clone();
        let executor_for_scheduler = task_executor.clone();
        std::thread::spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_secs(60));

            let conn = &mut database::init_connection(&db_url);
            let due = match services_for_scheduler.sync_schedule_service.get_due(conn) {
                Ok(v) => v,
                Err(e) => {
                    tracing::error!("Scheduler: failed to query due schedules: {}", e);
                    continue;
                }
            };
            for schedule in due {
                let schedule_id = match schedule.id {
                    Some(id) => id,
                    None => continue,
                };
                let url = schedule.playlist_url.clone();
                let label = schedule.label.clone();
                if let Err(e) = services_for_scheduler
                    .sync_schedule_service
                    .mark_ran(conn, schedule_id)
                {
                    tracing::error!(
                        "Scheduler: failed to mark schedule {} as ran: {}",
                        schedule_id,
                        e
                    );
                    continue;
                }
                let task = match services_for_scheduler
                    .task_service
                    .create_playlist_sync(conn, &url, label)
                {
                    Ok(t) => t,
                    Err(e) => {
                        tracing::error!(
                            "Scheduler: failed to create task for schedule {}: {}",
                            schedule_id,
                            e
                        );
                        continue;
                    }
                };
                let task_id = match task.id {
                    Some(id) => id,
                    None => continue,
                };
                let cancel_flag = registry_for_scheduler.register(task_id);
                tracing::info!(
                    "Scheduler: enqueueing sync for schedule {} (url={})",
                    schedule_id,
                    url
                );
                executor_for_scheduler.enqueue_playlist_sync(task_id, url, cancel_flag);
            }
        });
    }

    // Rocket — build a figment from the standard Rocket.toml / ROCKET_* sources,
    // then layer any SOUNDGNOME__SERVER__* overrides on top.
    let figment = {
        let soundgnome_cfg = Config::get();
        let mut f = rocket::Config::figment();
        // host
        if let Some(host) = &soundgnome_cfg.server.host {
            f = f.merge(("address", host.as_str()));
        }
        // port
        if let Some(port) = soundgnome_cfg.server.port {
            f = f.merge(("port", port));
        }
        // rocket database
        // let db: rocket::figment::value::Map<_, rocket::figment::value::Value>  = rocket::figment::util::map! {
        //     "url" => soundgnome_cfg.database.url.as_str().into(),
        //     "pool_size" => 10.into(),
        //     "timeout" => 5.into(),
        // };
        // f = f.merge(("databases.sqlite", db));
        f = f.merge(("databases.sqlite.url", soundgnome_cfg.database.url.as_str()));

        f
    };

    let waveform_services = services.clone();
    let waveform_db_url = db_url.clone();
    let cover_services = services.clone();
    let cover_db_url = db_url.clone();
    let loudness_services = services.clone();
    let loudness_db_url = db_url.clone();

    rocket::custom(figment)
        .attach(Cors)
        .attach(CacheControl)
        .attach(Db::fairing())
        .manage(services)
        .manage(cancellation_registry)
        .manage(task_executor)
        .attach(rocket::fairing::AdHoc::on_liftoff(
            "waveform-backfill",
            move |_rocket| {
                Box::pin(async move {
                    rocket::tokio::task::spawn_blocking(move || {
                        backfill_waveforms(&waveform_services, &waveform_db_url);
                    });
                })
            },
        ))
        .attach(rocket::fairing::AdHoc::on_liftoff(
            "cover-backfill",
            move |_rocket| {
                Box::pin(async move {
                    rocket::tokio::task::spawn_blocking(move || {
                        backfill_covers(&cover_services, &cover_db_url);
                    });
                })
            },
        ))
        .attach(rocket::fairing::AdHoc::on_liftoff(
            "loudness-backfill",
            move |_rocket| {
                Box::pin(async move {
                    rocket::tokio::task::spawn_blocking(move || {
                        backfill_loudness(&loudness_services, &loudness_db_url);
                    });
                })
            },
        ))
        .register("/", catchers![errors::default])
        .mount(
            "/api",
            openapi_get_routes![
                routes::misc::index,
                routes::misc::get_all,
                routes::misc::get_providers,
                routes::misc::get_version,
                routes::validations::get_pending,
                routes::validations::get_recent,
                routes::validations::approve_validation,
                routes::validations::reject_validation,
                routes::validations::get_match_candidates,
                routes::validations::get_youtube_provider_candidates,
                routes::download::download,
                routes::tasks::get_all,
                routes::tasks::get_by_id,
                routes::tasks::retry,
                routes::tasks::cancel,
                routes::sync_schedules::get_all,
                routes::sync_schedules::get_by_id,
                routes::sync_schedules::create,
                routes::sync_schedules::update,
                routes::sync_schedules::delete,
                routes::sync_schedules::trigger,
                routes::tracks::get_all,
                routes::tracks::get,
                routes::tracks::update,
                routes::tracks::delete,
                routes::tracks::set_rating,
                routes::tracks::ai_clean,
                routes::tracks::download_file,
                routes::tracks::get_references,
                routes::tracks::add_reference,
                routes::tracks::delete_reference,
                routes::albums::get_all,
                routes::albums::get,
                routes::albums::update,
                routes::albums::delete,
                routes::albums::merge,
                routes::albums::get_references,
                routes::albums::add_reference,
                routes::albums::delete_reference,
                routes::images::fetch_album_cover,
                routes::artists::get_all,
                routes::artists::get,
                routes::artists::update,
                routes::artists::delete,
                routes::artists::merge,
                routes::artists::get_references,
                routes::artists::add_reference,
                routes::artists::delete_reference,
                routes::images::fetch_artist_icon,
                routes::playlists::get_all,
                routes::playlists::get_tracks,
                routes::playlists::export,
                routes::playlists::delete,
                routes::library::scan,
                routes::library::ingest,
                routes::library::list_ingest_files,
                routes::library::ingest_all,
                routes::library::upload,
                routes::library::ingest_session,
                routes::library::embed_artwork,
                routes::library::backfill_fingerprints,
                routes::library::missing_files,
                routes::library::resync_track,
                routes::storage::storage_stats,
                routes::soundcloud::get_status,
                routes::soundcloud::connect,
                routes::soundcloud::disconnect,
                routes::soundcloud::stream_url,
                routes::spotify::list_likes,
                routes::spotify_audio::get_status,
                routes::spotify_audio::login,
                routes::spotify_audio::callback,
                routes::spotify_audio::disconnect,
                routes::lastfm::get_status,
                routes::lastfm::set_credentials,
                routes::lastfm::login,
                routes::lastfm::callback,
                routes::lastfm::disconnect,
                routes::lastfm::now_playing,
                routes::lastfm::scrobble,
                routes::audio::stream,
            ],
        )
        .mount(
            "/api",
            routes![
                routes::images::upload_artist_image,
                routes::images::upload_album_image,
                routes::images::upload_track_image,
                routes::images::batch_fetch_artist_icons,
                routes::images::batch_fetch_album_covers,
                routes::tracks::waveform,
                routes::tracks::cover,
                routes::tracks::loudness,
                routes::library::dedupe,
            ],
        )
        .mount("/", routes![routes::metrics::metrics])
        .mount("/swagger", make_swagger_ui(&get_docs()))
        .mount("/", FileServer::from("data/web"))
}
