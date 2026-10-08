//! Ogg Vorbis tagging round-trip.
//!
//! Spotify audio arrives as Ogg Vorbis and is kept that way rather than
//! transcoded, so the tag path has to work: an untagged file breaks the
//! `SOUNDOME_ID` anchor the library sync depends on.
//!
//! Fixtures are Opus rather than Vorbis: both are Ogg streams tagged with
//! Vorbis comments through the same code path, and not every ffmpeg build
//! ships a working Vorbis encoder. Skipped when ffmpeg is absent.

use std::path::PathBuf;
use std::process::{Command, Stdio};

use shared::models::{Album, AlbumType, Artist, Track};

fn ffmpeg_available() -> bool {
    Command::new("ffmpeg")
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

fn make_ogg(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("soundgnome-ogg-tag-test");
    std::fs::create_dir_all(&dir).expect("cannot create fixture dir");
    let path = dir.join(name);

    let status = Command::new("ffmpeg")
        .args([
            "-y",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:duration=2",
            "-c:a",
            "libopus",
        ])
        .arg(&path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("cannot run ffmpeg");
    assert!(status.success(), "ffmpeg could not encode {}", name);
    path
}

fn artist(name: &str) -> Artist {
    Artist {
        id: None,
        name: name.to_string(),
        icon: None,
        references: Vec::new(),
    }
}

fn sample_track() -> Track {
    Track {
        id: None,
        needs_validation: false,
        validation_reason: None,
        soundome_id: Some("soundgnome-test-id".to_string()),
        title: "Test Title".to_string(),
        artists: vec![artist("First Artist"), artist("Second Artist")],
        album: Some(Album {
            id: None,
            title: "Test Album".to_string(),
            artists: vec![artist("First Artist")],
            cover: None,
            date: None,
            album_type: AlbumType::Unknown,
            references: Vec::new(),
        }),
        genre: Some("Electronic".to_string()),
        cover: None,
        duration: Some(2),
        track_number: Some(7),
        disc_number: Some(1),
        label: Some("Test Label".to_string()),
        date: Some("2024-06-07".to_string()),
        file_path: None,
        references: Vec::new(),
    }
}

#[test]
fn writes_and_reads_back_vorbis_comments() {
    if !ffmpeg_available() {
        eprintln!("skipping: ffmpeg not installed");
        return;
    }

    let path = make_ogg("tagged.opus");
    let track = sample_track();
    let cover = b"not a real jpeg, only bytes".to_vec();

    tagger::file::tag_file_with_track_and_cover(&path, &track, Some(&cover))
        .expect("tagging failed");

    // Read back through lofty directly: the point is that the bytes landed in
    // the file, not that our own reader agrees with itself.
    use lofty::file::TaggedFileExt;
    use lofty::ogg::{OggPictureStorage, VorbisComments};
    use lofty::probe::Probe;

    let tagged = Probe::open(&path).unwrap().read().unwrap();
    let comments: VorbisComments = tagged
        .primary_tag()
        .cloned()
        .map(VorbisComments::from)
        .expect("no tag written");

    assert_eq!(comments.get("TITLE"), Some("Test Title"));
    assert_eq!(comments.get("ALBUM"), Some("Test Album"));
    assert_eq!(comments.get("GENRE"), Some("Electronic"));
    assert_eq!(comments.get("TRACKNUMBER"), Some("7"));
    assert_eq!(comments.get("DATE"), Some("2024-06-07"));
    assert_eq!(comments.get("LABEL"), Some("Test Label"));

    // Multi-artist must stay separable, not collapse into one string.
    let artists: Vec<&str> = comments.get_all("ARTIST").collect();
    assert_eq!(artists, vec!["First Artist", "Second Artist"]);

    assert_eq!(comments.pictures().len(), 1, "cover art missing");
    assert_eq!(comments.pictures()[0].0.data(), cover.as_slice());

    assert_eq!(comments.get("SOUNDOME_ID"), Some("soundgnome-test-id"));

    // And our own reader must find it, since the library sync relies on it.
    assert_eq!(
        tagger::file::read_soundome_id_from_file(&path).as_deref(),
        Some("soundgnome-test-id")
    );

    std::fs::remove_file(&path).ok();
}

#[test]
fn writes_the_anchor_without_touching_other_tags() {
    if !ffmpeg_available() {
        eprintln!("skipping: ffmpeg not installed");
        return;
    }

    let path = make_ogg("anchor-only.opus");
    let track = sample_track();
    tagger::file::tag_file_with_track_and_cover(&path, &track, None).expect("tagging failed");

    tagger::file::write_soundome_id_tag(&path, "second-id").expect("anchor write failed");

    assert_eq!(
        tagger::file::read_soundome_id_from_file(&path).as_deref(),
        Some("second-id")
    );

    use lofty::file::TaggedFileExt;
    use lofty::ogg::VorbisComments;
    use lofty::probe::Probe;
    let tagged = Probe::open(&path).unwrap().read().unwrap();
    let comments: VorbisComments = tagged
        .primary_tag()
        .cloned()
        .map(VorbisComments::from)
        .unwrap();
    assert_eq!(
        comments.get("TITLE"),
        Some("Test Title"),
        "rewriting the anchor must not clear the rest"
    );

    std::fs::remove_file(&path).ok();
}
