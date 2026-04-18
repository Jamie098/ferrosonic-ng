use httpmock::prelude::*;

use crate::error::SubsonicError;
use crate::subsonic::SubsonicClient;

fn make_client(server: &MockServer) -> SubsonicClient {
    SubsonicClient::new(&server.base_url(), "testuser", "testpass").unwrap()
}

// ── ping ─────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn ping_ok() {
    let server = MockServer::start_async().await;
    let mock = server
        .mock_async(|when, then| {
            when.method(GET).path("/rest/ping");
            then.status(200).body(
                r#"{"subsonic-response":{"status":"ok","version":"1.16.1"}}"#,
            );
        })
        .await;

    make_client(&server).ping().await.unwrap();
    mock.assert_async().await;
}

#[tokio::test]
async fn ping_api_error_with_error_object() {
    let server = MockServer::start_async().await;
    server
        .mock_async(|when, then| {
            when.method(GET).path("/rest/ping");
            then.status(200).body(
                r#"{"subsonic-response":{"status":"failed","version":"1.16.1","error":{"code":40,"message":"Wrong username or password"}}}"#,
            );
        })
        .await;

    let err = make_client(&server).ping().await.unwrap_err();
    match err {
        SubsonicError::Api { code, message } => {
            assert_eq!(code, 40);
            assert_eq!(message, "Wrong username or password");
        }
        other => panic!("expected Api error, got {other:?}"),
    }
}

#[tokio::test]
async fn ping_api_error_without_error_object() {
    let server = MockServer::start_async().await;
    server
        .mock_async(|when, then| {
            when.method(GET).path("/rest/ping");
            then.status(200).body(
                r#"{"subsonic-response":{"status":"failed","version":"1.16.1"}}"#,
            );
        })
        .await;

    // ping does not return Err when there is no error object — it silently succeeds.
    // This documents the current (lenient) behavior so any future change is visible.
    make_client(&server).ping().await.unwrap();
}

#[tokio::test]
async fn ping_malformed_json_returns_parse_error() {
    let server = MockServer::start_async().await;
    server
        .mock_async(|when, then| {
            when.method(GET).path("/rest/ping");
            then.status(200).body("not json");
        })
        .await;

    let err = make_client(&server).ping().await.unwrap_err();
    assert!(matches!(err, SubsonicError::Parse(_)));
}

// ── get_artists ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn get_artists_ok() {
    let server = MockServer::start_async().await;
    let mock = server
        .mock_async(|when, then| {
            when.method(GET).path("/rest/getArtists");
            then.status(200).body(
                r#"{
                    "subsonic-response": {
                        "status": "ok",
                        "version": "1.16.1",
                        "artists": {
                            "index": [
                                {"name": "A", "artist": [{"id": "1", "name": "Artist One"}]},
                                {"name": "B", "artist": [{"id": "2", "name": "Artist Two", "albumCount": 3}]}
                            ]
                        }
                    }
                }"#,
            );
        })
        .await;

    let artists = make_client(&server).get_artists().await.unwrap();
    assert_eq!(artists.len(), 2);
    assert_eq!(artists[0].id, "1");
    assert_eq!(artists[1].name, "Artist Two");
    assert_eq!(artists[1].album_count, Some(3));
    mock.assert_async().await;
}

#[tokio::test]
async fn get_artists_empty_index() {
    let server = MockServer::start_async().await;
    server
        .mock_async(|when, then| {
            when.method(GET).path("/rest/getArtists");
            then.status(200).body(
                r#"{"subsonic-response":{"status":"ok","version":"1.16.1","artists":{"index":[]}}}"#,
            );
        })
        .await;

    let artists = make_client(&server).get_artists().await.unwrap();
    assert!(artists.is_empty());
}

#[tokio::test]
async fn get_artists_api_error() {
    let server = MockServer::start_async().await;
    server
        .mock_async(|when, then| {
            when.method(GET).path("/rest/getArtists");
            then.status(200).body(
                r#"{"subsonic-response":{"status":"failed","version":"1.16.1","error":{"code":10,"message":"Required parameter is missing"}}}"#,
            );
        })
        .await;

    let err = make_client(&server).get_artists().await.unwrap_err();
    match err {
        SubsonicError::Api { code, message } => {
            assert_eq!(code, 10);
            assert_eq!(message, "Required parameter is missing");
        }
        other => panic!("expected Api error, got {other:?}"),
    }
}

// ── get_artist ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn get_artist_ok() {
    let server = MockServer::start_async().await;
    let mock = server
        .mock_async(|when, then| {
            when.method(GET).path("/rest/getArtist");
            then.status(200).body(
                r#"{
                    "subsonic-response": {
                        "status": "ok",
                        "version": "1.16.1",
                        "artist": {
                            "id": "ar-1",
                            "name": "The Beatles",
                            "album": [
                                {"id": "al-1", "name": "Abbey Road"},
                                {"id": "al-2", "name": "Let It Be"}
                            ]
                        }
                    }
                }"#,
            );
        })
        .await;

    let (artist, albums) = make_client(&server).get_artist("ar-1").await.unwrap();
    assert_eq!(artist.id, "ar-1");
    assert_eq!(artist.name, "The Beatles");
    assert_eq!(artist.album_count, Some(2));
    assert_eq!(albums.len(), 2);
    assert_eq!(albums[0].name, "Abbey Road");
    mock.assert_async().await;
}

#[tokio::test]
async fn get_artist_api_error() {
    let server = MockServer::start_async().await;
    server
        .mock_async(|when, then| {
            when.method(GET).path("/rest/getArtist");
            then.status(200).body(
                r#"{"subsonic-response":{"status":"failed","version":"1.16.1","error":{"code":70,"message":"Artist not found"}}}"#,
            );
        })
        .await;

    let err = make_client(&server).get_artist("missing").await.unwrap_err();
    assert!(matches!(err, SubsonicError::Api { code: 70, .. }));
}

// ── get_album ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn get_album_ok() {
    let server = MockServer::start_async().await;
    let mock = server
        .mock_async(|when, then| {
            when.method(GET).path("/rest/getAlbum");
            then.status(200).body(
                r#"{
                    "subsonic-response": {
                        "status": "ok",
                        "version": "1.16.1",
                        "album": {
                            "id": "al-1",
                            "name": "Abbey Road",
                            "artist": "The Beatles",
                            "artistId": "ar-1",
                            "year": 1969,
                            "song": [
                                {"id": "s-1", "title": "Come Together", "isDir": false},
                                {"id": "s-2", "title": "Something", "isDir": false}
                            ]
                        }
                    }
                }"#,
            );
        })
        .await;

    let (album, songs) = make_client(&server).get_album("al-1").await.unwrap();
    assert_eq!(album.id, "al-1");
    assert_eq!(album.name, "Abbey Road");
    assert_eq!(album.artist.as_deref(), Some("The Beatles"));
    assert_eq!(album.year, Some(1969));
    assert_eq!(album.song_count, Some(2));
    assert_eq!(songs.len(), 2);
    assert_eq!(songs[0].title, "Come Together");
    mock.assert_async().await;
}

#[tokio::test]
async fn get_album_api_error() {
    let server = MockServer::start_async().await;
    server
        .mock_async(|when, then| {
            when.method(GET).path("/rest/getAlbum");
            then.status(200).body(
                r#"{"subsonic-response":{"status":"failed","version":"1.16.1","error":{"code":70,"message":"Album not found"}}}"#,
            );
        })
        .await;

    let err = make_client(&server).get_album("missing").await.unwrap_err();
    assert!(matches!(err, SubsonicError::Api { code: 70, .. }));
}

// ── get_playlists / get_playlist ─────────────────────────────────────────────

#[tokio::test]
async fn get_playlists_ok() {
    let server = MockServer::start_async().await;
    let mock = server
        .mock_async(|when, then| {
            when.method(GET).path("/rest/getPlaylists");
            then.status(200).body(
                r#"{
                    "subsonic-response": {
                        "status": "ok",
                        "version": "1.16.1",
                        "playlists": {
                            "playlist": [
                                {"id": "p-1", "name": "Chill Mix", "songCount": 10, "duration": 2400},
                                {"id": "p-2", "name": "Workout", "songCount": 20, "duration": 4800}
                            ]
                        }
                    }
                }"#,
            );
        })
        .await;

    let playlists = make_client(&server).get_playlists().await.unwrap();
    assert_eq!(playlists.len(), 2);
    assert_eq!(playlists[0].id, "p-1");
    assert_eq!(playlists[1].name, "Workout");
    mock.assert_async().await;
}

#[tokio::test]
async fn get_playlist_ok() {
    let server = MockServer::start_async().await;
    let mock = server
        .mock_async(|when, then| {
            when.method(GET).path("/rest/getPlaylist");
            then.status(200).body(
                r#"{
                    "subsonic-response": {
                        "status": "ok",
                        "version": "1.16.1",
                        "playlist": {
                            "id": "p-1",
                            "name": "Chill Mix",
                            "owner": "admin",
                            "songCount": 2,
                            "duration": 420,
                            "entry": [
                                {"id": "s-1", "title": "Track One", "isDir": false},
                                {"id": "s-2", "title": "Track Two", "isDir": false}
                            ]
                        }
                    }
                }"#,
            );
        })
        .await;

    let (playlist, songs) = make_client(&server).get_playlist("p-1").await.unwrap();
    assert_eq!(playlist.id, "p-1");
    assert_eq!(playlist.name, "Chill Mix");
    assert_eq!(playlist.owner.as_deref(), Some("admin"));
    assert_eq!(songs.len(), 2);
    assert_eq!(songs[1].title, "Track Two");
    mock.assert_async().await;
}

#[tokio::test]
async fn get_playlist_api_error() {
    let server = MockServer::start_async().await;
    server
        .mock_async(|when, then| {
            when.method(GET).path("/rest/getPlaylist");
            then.status(200).body(
                r#"{"subsonic-response":{"status":"failed","version":"1.16.1","error":{"code":70,"message":"Playlist not found"}}}"#,
            );
        })
        .await;

    let err = make_client(&server).get_playlist("missing").await.unwrap_err();
    assert!(matches!(err, SubsonicError::Api { code: 70, .. }));
}

// ── get_starred_songs ────────────────────────────────────────────────────────

#[tokio::test]
async fn get_starred_songs_ok() {
    let server = MockServer::start_async().await;
    let mock = server
        .mock_async(|when, then| {
            when.method(GET).path("/rest/getStarred2");
            then.status(200).body(
                r#"{
                    "subsonic-response": {
                        "status": "ok",
                        "version": "1.16.1",
                        "starred2": {
                            "song": [
                                {"id": "s-1", "title": "Starred Song", "isDir": false}
                            ]
                        }
                    }
                }"#,
            );
        })
        .await;

    let songs = make_client(&server).get_starred_songs().await.unwrap();
    assert_eq!(songs.len(), 1);
    assert_eq!(songs[0].title, "Starred Song");
    mock.assert_async().await;
}

#[tokio::test]
async fn get_starred_songs_empty() {
    let server = MockServer::start_async().await;
    server
        .mock_async(|when, then| {
            when.method(GET).path("/rest/getStarred2");
            then.status(200).body(
                r#"{"subsonic-response":{"status":"ok","version":"1.16.1","starred2":{"song":[]}}}"#,
            );
        })
        .await;

    let songs = make_client(&server).get_starred_songs().await.unwrap();
    assert!(songs.is_empty());
}

// ── get_random_songs ─────────────────────────────────────────────────────────

#[tokio::test]
async fn get_random_songs_ok() {
    let server = MockServer::start_async().await;
    let mock = server
        .mock_async(|when, then| {
            when.method(GET).path("/rest/getRandomSongs");
            then.status(200).body(
                r#"{
                    "subsonic-response": {
                        "status": "ok",
                        "version": "1.16.1",
                        "randomSongs": {
                            "song": [
                                {"id": "r-1", "title": "Random One", "isDir": false},
                                {"id": "r-2", "title": "Random Two", "isDir": false}
                            ]
                        }
                    }
                }"#,
            );
        })
        .await;

    let songs = make_client(&server).get_random_songs(2).await.unwrap();
    assert_eq!(songs.len(), 2);
    assert_eq!(songs[0].id, "r-1");
    mock.assert_async().await;
}

// ── search_songs ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn search_songs_ok() {
    let server = MockServer::start_async().await;
    let mock = server
        .mock_async(|when, then| {
            when.method(GET).path("/rest/search3");
            then.status(200).body(
                r#"{
                    "subsonic-response": {
                        "status": "ok",
                        "version": "1.16.1",
                        "searchResult3": {
                            "song": [
                                {"id": "s-1", "title": "Found Song", "isDir": false}
                            ]
                        }
                    }
                }"#,
            );
        })
        .await;

    let songs = make_client(&server)
        .search_songs("Found", 0, 10)
        .await
        .unwrap();
    assert_eq!(songs.len(), 1);
    assert_eq!(songs[0].title, "Found Song");
    mock.assert_async().await;
}

// ── star / unstar ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn star_song_ok() {
    let server = MockServer::start_async().await;
    let mock = server
        .mock_async(|when, then| {
            when.method(GET).path("/rest/star");
            then.status(200)
                .body(r#"{"subsonic-response":{"status":"ok","version":"1.16.1"}}"#);
        })
        .await;

    make_client(&server).star_song("s-1").await.unwrap();
    mock.assert_async().await;
}

#[tokio::test]
async fn unstar_song_ok() {
    let server = MockServer::start_async().await;
    let mock = server
        .mock_async(|when, then| {
            when.method(GET).path("/rest/unstar");
            then.status(200)
                .body(r#"{"subsonic-response":{"status":"ok","version":"1.16.1"}}"#);
        })
        .await;

    make_client(&server).unstar_song("s-1").await.unwrap();
    mock.assert_async().await;
}

// ── scrobble ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn scrobble_submission_ok() {
    let server = MockServer::start_async().await;
    let mock = server
        .mock_async(|when, then| {
            when.method(GET).path("/rest/scrobble");
            then.status(200)
                .body(r#"{"subsonic-response":{"status":"ok","version":"1.16.1"}}"#);
        })
        .await;

    make_client(&server).scrobble("s-1", true).await.unwrap();
    mock.assert_async().await;
}

// ── get_stream_url ────────────────────────────────────────────────────────────

#[test]
fn get_stream_url_contains_song_id() {
    let client = SubsonicClient::new("http://example.com", "user", "pass").unwrap();
    let url = client.get_stream_url("song-42").unwrap();
    assert!(url.contains("id=song-42"));
    assert!(url.contains("/rest/stream"));
}

#[test]
fn get_stream_url_contains_auth_params() {
    let client = SubsonicClient::new("http://example.com", "alice", "secret").unwrap();
    let url = client.get_stream_url("s-1").unwrap();
    assert!(url.contains("u=alice"));
    assert!(url.contains("t="));
    assert!(url.contains("s="));
}

// ── api error without error object (generic path via request()) ───────────────

#[tokio::test]
async fn get_artists_failed_status_no_error_object() {
    let server = MockServer::start_async().await;
    server
        .mock_async(|when, then| {
            when.method(GET).path("/rest/getArtists");
            then.status(200).body(
                r#"{"subsonic-response":{"status":"failed","version":"1.16.1"}}"#,
            );
        })
        .await;

    let err = make_client(&server).get_artists().await.unwrap_err();
    match err {
        SubsonicError::Api { code: 0, message } => {
            assert_eq!(message, "Unknown error");
        }
        other => panic!("expected Api error with code 0, got {other:?}"),
    }
}

// ── malformed JSON ────────────────────────────────────────────────────────────

#[tokio::test]
async fn get_artists_malformed_json_returns_parse_error() {
    let server = MockServer::start_async().await;
    server
        .mock_async(|when, then| {
            when.method(GET).path("/rest/getArtists");
            then.status(200).body("not valid json {{");
        })
        .await;

    let err = make_client(&server).get_artists().await.unwrap_err();
    assert!(matches!(err, SubsonicError::Parse(_)));
}
