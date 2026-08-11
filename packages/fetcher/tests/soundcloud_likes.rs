//! Live check for the "my likes" pseudo-playlist.
//!
//! Ignored by default: it needs a real connected SoundCloud account, so it can
//! only pass on a machine where a session token has been stored through the UI.
//!
//! Run it against a repo checkout with:
//!
//! ```sh
//! SOUNDOME_CONFIG_PATH=$PWD/config.toml \
//! SOUNDOME__DATABASE__URL=$PWD/data/soundome.db \
//!   cargo test -p fetcher --test soundcloud_likes -- --ignored --nocapture
//! ```

use fetcher::{soundcloud::Soundcloud, Source};

#[test]
fn likes_url_is_recognised_as_a_playlist() {
    // Pure URL matching, so this one runs everywhere and guards the routing
    // that sends the likes URL down the playlist path in `Fetcher`.
    for url in [
        "https://soundcloud.com/you/likes",
        "https://soundcloud.com/you/likes/",
        "http://www.soundcloud.com/you/likes",
        "https://soundcloud.com/you/likes?ref=sidebar",
        "https://soundcloud.com/you/favorites",
    ] {
        assert!(Soundcloud::is_likes_url(url), "should match: {}", url);
        assert!(
            Soundcloud::is_valid_playlist_url(url),
            "should route as playlist: {}",
            url
        );
    }

    for url in [
        "https://soundcloud.com/someartist/sets/mixtape",
        "https://soundcloud.com/someartist/some-track",
        "https://soundcloud.com/someartist",
    ] {
        assert!(!Soundcloud::is_likes_url(url), "should not match: {}", url);
    }
}

#[tokio::test]
#[ignore = "requires a connected SoundCloud account"]
async fn fetches_every_liked_track() {
    shared::init_globals().expect("failed to init globals");

    let soundcloud = Soundcloud::new().await.expect("failed to build client");

    let playlist = soundcloud
        .get_playlist_from_url(Soundcloud::LIKES_URL)
        .await
        .expect("failed to build likes playlist");
    assert_eq!(playlist.name, "SoundCloud Likes");

    let tracks = soundcloud
        .get_playlist_tracks_from_url(Soundcloud::LIKES_URL)
        .await
        .expect("failed to fetch likes");

    println!("fetched {} liked tracks", tracks.len());
    if let Some(first) = tracks.first() {
        println!("first: {}", first.track.display());
    }

    // Pagination is the thing under test: a single unpaginated page caps at 50.
    assert!(
        tracks.len() > 50,
        "expected pagination past the first page, got {}",
        tracks.len()
    );

    // Positions must be dense and ordered, the playlist writer relies on it.
    for (i, pt) in tracks.iter().enumerate() {
        assert_eq!(pt.position, Some(i as u32));
        assert!(!pt.track.title.trim().is_empty(), "empty title at {}", i);
    }
}
