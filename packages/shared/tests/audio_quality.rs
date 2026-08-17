//! Verifies `Track::audio_quality` against real encoded files.
//!
//! Fixtures are generated with ffmpeg from the same synthetic tone, so the
//! three files differ only in codec/bitrate. Skipped when ffmpeg is absent.

use std::path::{Path, PathBuf};
use std::process::Command;

use shared::models::Track;

fn ffmpeg_available() -> bool {
    Command::new("ffmpeg")
        .arg("-version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Encode 5 seconds of a 440 Hz tone into `path` with the given codec args.
fn encode(dir: &Path, name: &str, args: &[&str]) -> PathBuf {
    let path = dir.join(name);
    let status = Command::new("ffmpeg")
        .args(["-y", "-f", "lavfi", "-i", "sine=frequency=440:duration=5"])
        .args(args)
        .arg(&path)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .expect("failed to run ffmpeg");
    assert!(status.success(), "ffmpeg failed to encode {}", name);
    path
}

fn track_at(path: PathBuf) -> Track {
    Track {
        id: None,
        needs_validation: false,
        validation_reason: None,
        soundome_id: None,
        title: "tone".to_string(),
        artists: vec![],
        album: None,
        date: None,
        genre: None,
        cover: None,
        duration: None,
        track_number: None,
        disc_number: None,
        label: None,
        file_path: Some(path),
        references: vec![],
    }
}

#[test]
fn ranks_lossless_above_lossy_and_higher_bitrate_above_lower() {
    if !ffmpeg_available() {
        eprintln!("skipping: ffmpeg not installed");
        return;
    }

    let dir = std::env::temp_dir().join("soundgnome-audio-quality-test");
    std::fs::create_dir_all(&dir).expect("failed to create fixture dir");

    let mp3_128 = track_at(encode(&dir, "tone-128.mp3", &["-b:a", "128k"]));
    let mp3_320 = track_at(encode(&dir, "tone-320.mp3", &["-b:a", "320k"]));
    let flac = track_at(encode(&dir, "tone.flac", &[]));
    let m4a = track_at(encode(&dir, "tone.m4a", &["-c:a", "aac", "-b:a", "128k"]));

    // The original bug: mp3/m4a were not probeable at all, so every comparison
    // silently returned "not better" and nothing was ever replaced.
    let q_128 = mp3_128.audio_quality().expect("mp3 must be probeable");
    let q_320 = mp3_320.audio_quality().expect("mp3 must be probeable");
    let q_flac = flac.audio_quality().expect("flac must be probeable");
    let q_m4a = m4a.audio_quality().expect("m4a must be probeable");

    assert!(!q_128.lossless);
    assert!(!q_m4a.lossless);
    assert!(q_flac.lossless);

    // Bitrate ordering within the same codec.
    assert!(q_320.bitrate_bps > q_128.bitrate_bps);

    // Lossless outranks lossy regardless of the measured bitrates.
    assert!(q_flac > q_320);

    // A missing file is not comparable.
    assert!(track_at(dir.join("nope.mp3")).audio_quality().is_none());

    std::fs::remove_dir_all(&dir).ok();
}
