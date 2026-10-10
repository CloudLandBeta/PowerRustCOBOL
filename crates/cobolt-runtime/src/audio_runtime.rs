// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Sound from COBOL — spec 092 R15–R18.
//!
//! | CALL name            | Arguments                                         |
//! |----------------------|---------------------------------------------------|
//! | `COBOL-AUDIO-PLAY`   | file in, [status out]                             |
//! | `COBOL-AUDIO-PAUSE`  | [status out]                                      |
//! | `COBOL-AUDIO-RESUME` | [status out]                                      |
//! | `COBOL-AUDIO-STOP`   | [status out]                                      |
//! | `COBOL-AUDIO-STATE`  | state out, [position-ms out], [duration-ms out]   |
//!
//! `AUDIO-PLAY` starts an MP3, OGG Vorbis, WAV or FLAC file on the default
//! output device, replacing whatever was playing. There is **one** sound per
//! application: every program and every form talks to the same player, so a
//! form can read the progress of a sound another form started.
//!
//! A status is one word — [`Status`]: `OK`, `NOFILE`, `BADFORMAT`, `NODEVICE`,
//! `NOFEATURE` or `ERROR`. A state is one word — [`PlayState`]: `IDLE`,
//! `PLAYING`, `PAUSED`, `ENDED` or `ERROR`. Positions and lengths are whole
//! milliseconds; a length the file does not declare reads `0` until the sound
//! has played to its end.
//!
//! # One thread owns the device
//!
//! An output stream is not `Send` on every platform, so the device is opened,
//! used and closed by one dedicated thread (`cobol-audio`), started by the
//! first `AUDIO-…` call — a program that never makes one never starts it. The
//! interpreter only sends that thread commands and reads the snapshot it
//! publishes. A missing device, an unreadable file and an undecodable one are
//! status words, never a panic; a command the thread does not answer within
//! [`REPLY_TIMEOUT`] reads `ERROR` rather than hanging the program.
//!
//! The device is held only while a sound plays or is paused: it is released
//! at the end of the sound, on `AUDIO-STOP`, and when the program that
//! started the sound ends (its interpreter is dropped). The thread itself
//! sleeps on its channel when nothing plays and dies with the process.
//!
//! # The null output — `COBOLT_AUDIO=null`
//!
//! With `COBOLT_AUDIO=null` in the environment, nothing is sent to a sound
//! card: the file is still opened and decoded exactly as for real playback
//! (so `NOFILE` and `BADFORMAT` mean the same), and a monotonic clock "plays"
//! it for its length. The state machine, the position and the end of the
//! sound behave as they do with a device. Tests and CI use it; a developer can
//! use it to exercise a program on a machine without sound. Without it, a
//! machine with no output device answers `NODEVICE` — the null output is never
//! substituted silently, because a program told `OK` would believe it played.
//!
//! # The `audio` feature
//!
//! Decoding and the device live behind it (`rodio`: `cpal` for the device,
//! `symphonia` for the formats). On Linux `cpal` reaches the sound card
//! through ALSA, so a build with the feature needs the ALSA development
//! package and a machine running it needs `libasound2`. A build without it
//! still compiles and still answers: every command says `NOFEATURE`, the way an
//! HTTP verb in a build without `http` reports status 0. The compiler turns the
//! feature on for any program that names one of the verbs, so `NOFEATURE` is
//! reachable only through a CALL target assembled at run time — which links
//! everything anyway.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// How long a command waits for the audio thread to answer before it reads
/// `ERROR`. Generous: the first `AUDIO-PLAY` opens the device, which can take
/// a moment on some systems, and the null output measures a file whose length
/// is not declared by decoding it.
pub const REPLY_TIMEOUT: Duration = Duration::from_secs(15);

/// The environment variable that selects the null output (`null`).
pub const OUTPUT_VAR: &str = "COBOLT_AUDIO";

/// What a command answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Done.
    Ok,
    /// The file does not exist, is not a regular file, or cannot be opened.
    NoFile,
    /// The file is not sound in one of the four formats, or it is damaged.
    BadFormat,
    /// There is no output device, or it cannot be opened.
    NoDevice,
    /// This program was built without the `audio` feature.
    NoFeature,
    /// Nothing to pause or resume, or the audio thread is gone.
    Error,
}

impl Status {
    /// The word COBOL receives.
    pub fn as_word(self) -> &'static str {
        match self {
            Status::Ok => "OK",
            Status::NoFile => "NOFILE",
            Status::BadFormat => "BADFORMAT",
            Status::NoDevice => "NODEVICE",
            Status::NoFeature => "NOFEATURE",
            Status::Error => "ERROR",
        }
    }
}

/// Where the player is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayState {
    /// Nothing has been played, or the sound was stopped.
    Idle,
    /// A sound is playing.
    Playing,
    /// A sound is paused; its position holds.
    Paused,
    /// The sound played to its end.
    Ended,
    /// The last `AUDIO-PLAY` failed (its status says why).
    Error,
}

impl PlayState {
    /// The word COBOL receives.
    pub fn as_word(self) -> &'static str {
        match self {
            PlayState::Idle => "IDLE",
            PlayState::Playing => "PLAYING",
            PlayState::Paused => "PAUSED",
            PlayState::Ended => "ENDED",
            PlayState::Error => "ERROR",
        }
    }
}

/// What `AUDIO-STATE` reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Snapshot {
    pub state: PlayState,
    /// How far into the sound, in milliseconds.
    pub position_ms: u64,
    /// The sound's length in milliseconds; `0` while it is not known.
    pub duration_ms: u64,
}

impl Snapshot {
    /// Nothing playing.
    pub const IDLE: Snapshot = Snapshot { state: PlayState::Idle, position_ms: 0, duration_ms: 0 };
}

/// Distinguishes the programs that use the player, so that the end of one
/// stops only the sound it started.
static NEXT_OWNER: AtomicU64 = AtomicU64::new(1);

/// One interpreter's door to the application's player.
///
/// Dropping it — the program ended — stops the sound **this** program
/// started, if it is still playing or paused; a sound another program started
/// plays on.
pub struct AudioHandle {
    owner: u64,
    /// Off-build only: the last command was a refused `AUDIO-PLAY`, so the
    /// state reads `ERROR`, exactly as after any other failed `AUDIO-PLAY`.
    #[cfg(not(feature = "audio"))]
    refused: bool,
}

impl Default for AudioHandle {
    fn default() -> Self {
        Self::new()
    }
}

// ── The real player ─────────────────────────────────────────────────────────

#[cfg(feature = "audio")]
impl AudioHandle {
    pub fn new() -> Self {
        Self { owner: NEXT_OWNER.fetch_add(1, Ordering::Relaxed) }
    }

    /// `AUDIO-PLAY`: start `path`, replacing whatever was playing.
    pub fn play(&mut self, path: &str) -> Status {
        engine::global().play(self.owner, std::path::Path::new(path.trim()))
    }

    /// `AUDIO-PAUSE`.
    pub fn pause(&mut self) -> Status {
        engine::global().pause()
    }

    /// `AUDIO-RESUME`.
    pub fn resume(&mut self) -> Status {
        engine::global().resume()
    }

    /// `AUDIO-STOP`.
    pub fn stop(&mut self) -> Status {
        engine::global().stop()
    }

    /// `AUDIO-STATE`. Never starts the audio thread just to say nothing plays.
    pub fn state(&self) -> Snapshot {
        engine::running().map_or(Snapshot::IDLE, |e| e.snapshot())
    }
}

#[cfg(feature = "audio")]
impl Drop for AudioHandle {
    fn drop(&mut self) {
        if let Some(engine) = engine::running() {
            engine.release(self.owner);
        }
    }
}

// ── The shim, when the `audio` feature is off ────────────────────────────────
//
// Adjacent to the real one on purpose (the `http_runtime` rule): a command
// added above must be added here too, and a shim kept elsewhere rots unseen.
#[cfg(not(feature = "audio"))]
impl AudioHandle {
    pub fn new() -> Self {
        Self { owner: NEXT_OWNER.fetch_add(1, Ordering::Relaxed), refused: false }
    }
    pub fn play(&mut self, _path: &str) -> Status {
        let _ = self.owner;
        self.refused = true;
        Status::NoFeature
    }
    pub fn pause(&mut self) -> Status {
        Status::NoFeature
    }
    pub fn resume(&mut self) -> Status {
        Status::NoFeature
    }
    pub fn stop(&mut self) -> Status {
        self.refused = false;
        Status::NoFeature
    }
    pub fn state(&self) -> Snapshot {
        if self.refused {
            Snapshot { state: PlayState::Error, ..Snapshot::IDLE }
        } else {
            Snapshot::IDLE
        }
    }
}

/// Whether `COBOLT_AUDIO` asks for the null output.
pub fn null_output_requested(value: Option<&str>) -> bool {
    value.is_some_and(|v| v.trim().eq_ignore_ascii_case("null"))
}

#[cfg(feature = "audio")]
mod engine {
    use super::{PlayState, Snapshot, Status, OUTPUT_VAR, REPLY_TIMEOUT};
    use std::fs::File;
    use std::io::BufReader;
    use std::path::{Path, PathBuf};
    use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
    use std::sync::{Arc, Mutex, OnceLock};
    use std::thread::JoinHandle;
    use std::time::{Duration, Instant};

    use rodio::Source;

    /// How often the thread refreshes the position while a sound plays.
    const TICK: Duration = Duration::from_millis(20);

    /// A decoded file, ready to play.
    pub(super) struct Track {
        decoder: rodio::Decoder<BufReader<File>>,
        /// The length the file declares, when it declares one.
        duration: Option<Duration>,
    }

    /// One sound being played on an output. Dropping it ends the sound and
    /// releases the output.
    pub(super) trait Sink {
        fn pause(&mut self);
        fn resume(&mut self);
        /// How far into the sound.
        fn position(&self) -> Duration;
        /// The sound has played to its end.
        fn drained(&self) -> bool;
    }

    /// Opens an output and starts a track on it.
    pub(super) type Opener = Box<dyn FnMut(Track) -> Result<Box<dyn Sink>, Status> + Send>;

    /// Open and decode `path`: the status words a file can earn.
    pub(super) fn open_track(path: &Path) -> Result<Track, Status> {
        let meta = std::fs::metadata(path).map_err(|_| Status::NoFile)?;
        if !meta.is_file() {
            return Err(Status::NoFile);
        }
        let file = File::open(path).map_err(|_| Status::NoFile)?;
        let mut builder = rodio::Decoder::builder()
            .with_data(BufReader::new(file))
            .with_byte_len(meta.len())
            .with_seekable(true);
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            builder = builder.with_hint(ext);
        }
        // Every decoder failure — an unknown format, damaged data, a header
        // that ends early — is the file's content, since it opened.
        let decoder = builder.build().map_err(|_| Status::BadFormat)?;
        let duration = decoder.total_duration();
        Ok(Track { decoder, duration })
    }

    // ── The device ───────────────────────────────────────────────────────

    struct DeviceSink {
        // Declared first so it drops first: the player stops, then the
        // stream closes.
        player: rodio::Player,
        _device: rodio::MixerDeviceSink,
    }

    impl Sink for DeviceSink {
        fn pause(&mut self) {
            self.player.pause();
        }
        fn resume(&mut self) {
            self.player.play();
        }
        fn position(&self) -> Duration {
            self.player.get_pos()
        }
        fn drained(&self) -> bool {
            self.player.empty()
        }
    }

    /// The default output device. rodio tries the default device and then
    /// every other real one before giving up.
    pub(super) fn device_opener() -> Opener {
        Box::new(|track: Track| {
            let device = rodio::DeviceSinkBuilder::open_default_sink().map_err(|e| {
                tracing::warn!("audio: no output device could be opened: {e}");
                Status::NoDevice
            })?;
            let player = rodio::Player::connect_new(device.mixer());
            player.append(track.decoder);
            Ok(Box::new(DeviceSink { player, _device: device }) as Box<dyn Sink>)
        })
    }

    // ── The null output ──────────────────────────────────────────────────

    /// A stopwatch that can be paused.
    struct Clock {
        since: Option<Instant>,
        banked: Duration,
    }

    impl Clock {
        fn running() -> Self {
            Self { since: Some(Instant::now()), banked: Duration::ZERO }
        }
        fn elapsed(&self) -> Duration {
            self.banked + self.since.map_or(Duration::ZERO, |t| t.elapsed())
        }
        fn pause(&mut self) {
            if let Some(t) = self.since.take() {
                self.banked += t.elapsed();
            }
        }
        fn resume(&mut self) {
            if self.since.is_none() {
                self.since = Some(Instant::now());
            }
        }
    }

    /// "Plays" a track for its length against a monotonic clock.
    struct NullSink {
        clock: Clock,
        length: Duration,
    }

    impl Sink for NullSink {
        fn pause(&mut self) {
            self.clock.pause();
        }
        fn resume(&mut self) {
            self.clock.resume();
        }
        fn position(&self) -> Duration {
            self.clock.elapsed().min(self.length)
        }
        fn drained(&self) -> bool {
            self.clock.elapsed() >= self.length
        }
    }

    /// The null output (`COBOLT_AUDIO=null`).
    pub(super) fn null_opener() -> Opener {
        Box::new(|track: Track| {
            // A length the file does not declare is measured by decoding it,
            // so the sound still ends when real playback would. What the
            // player REPORTS stays the declared length (or 0), as it would
            // with a device.
            let length = match track.duration {
                Some(d) => d,
                None => measured_length(track.decoder),
            };
            Ok(Box::new(NullSink { clock: Clock::running(), length }) as Box<dyn Sink>)
        })
    }

    fn measured_length(decoder: rodio::Decoder<BufReader<File>>) -> Duration {
        let per_second = decoder.sample_rate().get() as f64 * decoder.channels().get() as f64;
        let samples = decoder.count() as f64;
        Duration::from_secs_f64(samples / per_second)
    }

    // ── The thread ───────────────────────────────────────────────────────

    enum Request {
        Play { owner: u64, path: PathBuf, reply: Sender<Status> },
        Pause { reply: Sender<Status> },
        Resume { reply: Sender<Status> },
        Stop { reply: Sender<Status> },
        /// The program `owner` ended: stop its sound, if it is still the
        /// current one. Nobody waits for an answer.
        Release { owner: u64 },
    }

    /// The player: a channel to the audio thread and the snapshot it
    /// publishes.
    pub(super) struct Engine {
        tx: Option<Sender<Request>>,
        shared: Arc<Mutex<Snapshot>>,
        worker: Option<JoinHandle<()>>,
    }

    static GLOBAL: OnceLock<Engine> = OnceLock::new();

    /// The application's player, started on first use with the output
    /// `COBOLT_AUDIO` selects.
    pub(super) fn global() -> &'static Engine {
        GLOBAL.get_or_init(|| {
            let wanted = std::env::var(OUTPUT_VAR).ok();
            if super::null_output_requested(wanted.as_deref()) {
                Engine::start(null_opener())
            } else {
                Engine::start(device_opener())
            }
        })
    }

    /// The player, if a program has used it.
    pub(super) fn running() -> Option<&'static Engine> {
        GLOBAL.get()
    }

    impl Engine {
        pub(super) fn start(opener: Opener) -> Engine {
            let shared = Arc::new(Mutex::new(Snapshot::IDLE));
            let (tx, rx) = mpsc::channel();
            let published = Arc::clone(&shared);
            // The worker is built ON the thread: an output it opens may not be
            // `Send`, so the worker that holds one never crosses threads.
            match std::thread::Builder::new()
                .name("cobol-audio".into())
                .spawn(move || Worker::new(opener, published).run(rx))
            {
                Ok(handle) => Engine { tx: Some(tx), shared, worker: Some(handle) },
                Err(e) => {
                    tracing::warn!("audio: the audio thread could not be started: {e}");
                    Engine { tx: None, shared, worker: None }
                }
            }
        }

        fn ask(&self, make: impl FnOnce(Sender<Status>) -> Request) -> Status {
            let Some(tx) = &self.tx else { return Status::Error };
            let (reply, answer) = mpsc::channel();
            if tx.send(make(reply)).is_err() {
                return Status::Error;
            }
            // A thread that died mid-command drops `reply`, which ends the
            // wait at once; one that hangs ends it at the timeout.
            answer.recv_timeout(REPLY_TIMEOUT).unwrap_or(Status::Error)
        }

        pub(super) fn play(&self, owner: u64, path: &Path) -> Status {
            self.ask(|reply| Request::Play { owner, path: path.to_path_buf(), reply })
        }
        pub(super) fn pause(&self) -> Status {
            self.ask(|reply| Request::Pause { reply })
        }
        pub(super) fn resume(&self) -> Status {
            self.ask(|reply| Request::Resume { reply })
        }
        pub(super) fn stop(&self) -> Status {
            self.ask(|reply| Request::Stop { reply })
        }
        pub(super) fn release(&self, owner: u64) {
            if let Some(tx) = &self.tx {
                let _ = tx.send(Request::Release { owner });
            }
        }

        pub(super) fn snapshot(&self) -> Snapshot {
            let alive = self.worker.as_ref().is_some_and(|w| !w.is_finished());
            if !alive {
                return Snapshot { state: PlayState::Error, ..Snapshot::IDLE };
            }
            *self.shared.lock().unwrap_or_else(|p| p.into_inner())
        }

        /// The snapshot, as the test that watches the thread end needs it.
        #[cfg(test)]
        pub(super) fn shared(&self) -> std::sync::Weak<Mutex<Snapshot>> {
            Arc::downgrade(&self.shared)
        }
    }

    /// Lives on the audio thread; owns the output.
    struct Worker {
        open: Opener,
        sink: Option<Box<dyn Sink>>,
        owner: u64,
        state: PlayState,
        position: Duration,
        duration: Option<Duration>,
        shared: Arc<Mutex<Snapshot>>,
    }

    impl Worker {
        fn new(open: Opener, shared: Arc<Mutex<Snapshot>>) -> Self {
            Worker {
                open,
                sink: None,
                owner: 0,
                state: PlayState::Idle,
                position: Duration::ZERO,
                duration: None,
                shared,
            }
        }

        fn run(mut self, rx: Receiver<Request>) {
            loop {
                // Only a playing sound moves; otherwise sleep until told.
                let next = if self.state == PlayState::Playing {
                    match rx.recv_timeout(TICK) {
                        Ok(r) => Some(r),
                        Err(RecvTimeoutError::Timeout) => None,
                        Err(RecvTimeoutError::Disconnected) => break,
                    }
                } else {
                    match rx.recv() {
                        Ok(r) => Some(r),
                        Err(_) => break,
                    }
                };
                let answer = next.and_then(|r| self.handle(r));
                self.tick();
                self.publish();
                // Answered after publishing, so the caller's next AUDIO-STATE
                // already shows what the command did.
                if let Some((reply, status)) = answer {
                    let _ = reply.send(status);
                }
            }
            // The player is gone: the sound and the output go with it.
            self.sink = None;
        }

        fn handle(&mut self, request: Request) -> Option<(Sender<Status>, Status)> {
            match request {
                Request::Play { owner, path, reply } => Some((reply, self.play(owner, &path))),
                Request::Pause { reply } => Some((reply, self.pause())),
                Request::Resume { reply } => Some((reply, self.resume())),
                Request::Stop { reply } => Some((reply, self.stop())),
                Request::Release { owner } => {
                    if owner == self.owner && matches!(self.state, PlayState::Playing | PlayState::Paused) {
                        self.stop();
                    }
                    None
                }
            }
        }

        fn play(&mut self, owner: u64, path: &Path) -> Status {
            // Whatever played ends first, whether or not the new file plays.
            self.sink = None;
            self.owner = owner;
            self.position = Duration::ZERO;
            self.duration = None;
            let started = open_track(path).and_then(|track| {
                let declared = track.duration;
                (self.open)(track).map(|sink| (sink, declared))
            });
            match started {
                Ok((sink, declared)) => {
                    self.sink = Some(sink);
                    self.duration = declared;
                    self.state = PlayState::Playing;
                    Status::Ok
                }
                Err(status) => {
                    self.state = PlayState::Error;
                    status
                }
            }
        }

        fn pause(&mut self) -> Status {
            match self.state {
                PlayState::Playing => {
                    if let Some(sink) = self.sink.as_mut() {
                        sink.pause();
                        // The position holds at what it was when paused.
                        let at = sink.position();
                        self.position = self.clamped(at);
                    }
                    self.state = PlayState::Paused;
                    Status::Ok
                }
                PlayState::Paused => Status::Ok,
                _ => Status::Error,
            }
        }

        fn resume(&mut self) -> Status {
            match self.state {
                PlayState::Paused => {
                    if let Some(sink) = self.sink.as_mut() {
                        sink.resume();
                    }
                    self.state = PlayState::Playing;
                    Status::Ok
                }
                PlayState::Playing => Status::Ok,
                _ => Status::Error,
            }
        }

        fn stop(&mut self) -> Status {
            self.sink = None;
            self.state = PlayState::Idle;
            self.position = Duration::ZERO;
            self.duration = None;
            Status::Ok
        }

        fn clamped(&self, position: Duration) -> Duration {
            self.duration.map_or(position, |d| position.min(d))
        }

        /// Follow a playing sound: its position, and its end.
        fn tick(&mut self) {
            if self.state != PlayState::Playing {
                return;
            }
            let Some(sink) = self.sink.as_ref() else { return };
            if sink.drained() {
                let last = sink.position().max(self.position);
                // At the end the length is known, declared or not.
                let length = self.duration.unwrap_or(last);
                self.position = length;
                self.duration = Some(length);
                self.sink = None;
                self.state = PlayState::Ended;
            } else {
                self.position = self.clamped(sink.position());
            }
        }

        fn publish(&self) {
            let snapshot = Snapshot {
                state: self.state,
                position_ms: self.position.as_millis() as u64,
                duration_ms: self.duration.map_or(0, |d| d.as_millis() as u64),
            };
            *self.shared.lock().unwrap_or_else(|p| p.into_inner()) = snapshot;
        }
    }

    // ── Tests: the state machine and the clock, on the null output ─────

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::sync::atomic::{AtomicUsize, Ordering};

        /// A silent PCM WAV of `ms` milliseconds — 8 kHz, 8-bit, mono — with
        /// its RIFF header written by hand.
        fn wav(ms: u32) -> Vec<u8> {
            let rate: u32 = 8000;
            let data_len = rate * ms / 1000;
            let mut b = Vec::with_capacity(44 + data_len as usize);
            b.extend_from_slice(b"RIFF");
            b.extend_from_slice(&(36 + data_len).to_le_bytes());
            b.extend_from_slice(b"WAVEfmt ");
            b.extend_from_slice(&16u32.to_le_bytes()); // fmt chunk size
            b.extend_from_slice(&1u16.to_le_bytes()); // PCM
            b.extend_from_slice(&1u16.to_le_bytes()); // mono
            b.extend_from_slice(&rate.to_le_bytes());
            b.extend_from_slice(&rate.to_le_bytes()); // bytes per second
            b.extend_from_slice(&1u16.to_le_bytes()); // block align
            b.extend_from_slice(&8u16.to_le_bytes()); // bits per sample
            b.extend_from_slice(b"data");
            b.extend_from_slice(&data_len.to_le_bytes());
            b.resize(44 + data_len as usize, 0x80); // unsigned 8-bit silence
            b
        }

        struct Scratch(PathBuf);
        impl Scratch {
            fn new(tag: &str) -> Self {
                static N: AtomicUsize = AtomicUsize::new(0);
                let dir = std::env::temp_dir().join(format!(
                    "prc-audio-{tag}-{}-{}",
                    std::process::id(),
                    N.fetch_add(1, Ordering::Relaxed)
                ));
                std::fs::create_dir_all(&dir).expect("scratch dir");
                Scratch(dir)
            }
            fn file(&self, name: &str, bytes: &[u8]) -> PathBuf {
                let p = self.0.join(name);
                std::fs::write(&p, bytes).expect("write fixture");
                p
            }
        }
        impl Drop for Scratch {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }

        /// Poll until `pred` holds or `limit` passes; the last snapshot either way.
        fn wait_for(engine: &Engine, limit: Duration, pred: impl Fn(&Snapshot) -> bool) -> (Snapshot, Duration) {
            let t0 = Instant::now();
            loop {
                let s = engine.snapshot();
                if pred(&s) || t0.elapsed() > limit {
                    return (s, t0.elapsed());
                }
                std::thread::sleep(Duration::from_millis(5));
            }
        }

        #[test]
        fn a_two_second_wav_plays_advances_and_ends_at_its_length() {
            let dir = Scratch::new("ends");
            let tone = dir.file("tone.wav", &wav(2000));
            let engine = Engine::start(null_opener());
            let t0 = Instant::now();
            assert_eq!(engine.play(1, &tone), Status::Ok);
            let s = engine.snapshot();
            assert_eq!(s.state, PlayState::Playing, "{s:?}");
            assert_eq!(s.duration_ms, 2000, "a WAV declares its length: {s:?}");
            let (s, _) = wait_for(&engine, Duration::from_secs(5), |s| s.position_ms >= 300);
            assert!(s.position_ms >= 300 && s.state == PlayState::Playing, "the position advances: {s:?}");
            let (s, _) = wait_for(&engine, Duration::from_secs(10), |s| s.state != PlayState::Playing);
            let wall = t0.elapsed();
            assert_eq!(s.state, PlayState::Ended, "{s:?}");
            assert_eq!((s.position_ms, s.duration_ms), (2000, 2000), "ENDED sits at the length: {s:?}");
            assert!(wall >= Duration::from_millis(1950), "it played in real time, not instantly: {wall:?}");
            println!("  2000 ms of sound ended after {} ms of wall time", wall.as_millis());
        }

        #[test]
        fn pause_holds_the_position_and_resume_continues_from_it() {
            let dir = Scratch::new("pause");
            let tone = dir.file("tone.wav", &wav(2000));
            let engine = Engine::start(null_opener());
            assert_eq!(engine.play(1, &tone), Status::Ok);
            wait_for(&engine, Duration::from_secs(5), |s| s.position_ms >= 200);
            assert_eq!(engine.pause(), Status::Ok);
            let held = engine.snapshot();
            assert_eq!(held.state, PlayState::Paused, "{held:?}");
            // An observation window, not a wait for an event: the position
            // must not move while paused.
            std::thread::sleep(Duration::from_millis(300));
            let still = engine.snapshot();
            assert_eq!(still, held, "a paused sound holds its position");
            assert_eq!(engine.pause(), Status::Ok, "pausing a paused sound is harmless");
            assert_eq!(engine.resume(), Status::Ok);
            let (s, _) = wait_for(&engine, Duration::from_secs(5), |s| s.position_ms > held.position_ms + 100);
            assert_eq!(s.state, PlayState::Playing, "{s:?}");
            assert!(s.position_ms > held.position_ms, "resume continues from the held position: {s:?} vs {held:?}");
            assert!(s.position_ms < held.position_ms + 1000, "…not from somewhere else: {s:?} vs {held:?}");
        }

        #[test]
        fn stop_returns_to_idle() {
            let dir = Scratch::new("stop");
            let tone = dir.file("tone.wav", &wav(2000));
            let engine = Engine::start(null_opener());
            assert_eq!(engine.play(1, &tone), Status::Ok);
            assert_eq!(engine.stop(), Status::Ok);
            assert_eq!(engine.snapshot(), Snapshot::IDLE);
            assert_eq!(engine.stop(), Status::Ok, "stopping nothing is harmless");
        }

        #[test]
        fn a_new_play_replaces_the_sound_that_was_playing() {
            let dir = Scratch::new("replace");
            let long = dir.file("long.wav", &wav(2000));
            let short = dir.file("short.wav", &wav(500));
            let engine = Engine::start(null_opener());
            assert_eq!(engine.play(1, &long), Status::Ok);
            wait_for(&engine, Duration::from_secs(5), |s| s.position_ms >= 300);
            assert_eq!(engine.play(1, &short), Status::Ok);
            let s = engine.snapshot();
            assert_eq!(s.duration_ms, 500, "the new sound is the current one: {s:?}");
            assert!(s.position_ms < 300, "…and it starts from its beginning: {s:?}");
            let (s, _) = wait_for(&engine, Duration::from_secs(5), |s| s.state != PlayState::Playing);
            assert_eq!((s.state, s.position_ms), (PlayState::Ended, 500), "{s:?}");
        }

        #[test]
        fn a_missing_file_is_nofile_and_a_folder_too() {
            let dir = Scratch::new("missing");
            let engine = Engine::start(null_opener());
            assert_eq!(engine.play(1, &dir.0.join("no-such.mp3")), Status::NoFile);
            assert_eq!(engine.snapshot().state, PlayState::Error);
            assert_eq!(engine.play(1, &dir.0), Status::NoFile, "a folder is not a sound file");
        }

        #[test]
        fn garbage_an_empty_file_and_a_cut_header_are_badformat() {
            let dir = Scratch::new("garbage");
            let engine = Engine::start(null_opener());
            // Printable noise: no 0xFF byte, so not even an MP3 frame sync.
            let noise: Vec<u8> = (0..4096u32).map(|i| (i.wrapping_mul(2654435761) >> 13) as u8 % 94 + 32).collect();
            for (name, bytes) in [
                ("noise.wav", noise.as_slice()),
                ("empty.mp3", &[][..]),
                ("text.ogg", b"IDENTIFICATION DIVISION.".as_slice()),
                ("cut.wav", &wav(1000)[..20]),
            ] {
                let p = dir.file(name, bytes);
                assert_eq!(engine.play(1, &p), Status::BadFormat, "{name}");
                assert_eq!(engine.snapshot().state, PlayState::Error, "{name}");
            }
        }

        #[test]
        fn no_device_is_nodevice_and_never_a_panic() {
            let dir = Scratch::new("nodevice");
            let tone = dir.file("tone.wav", &wav(500));
            let engine = Engine::start(Box::new(|_track: Track| Err(Status::NoDevice)));
            assert_eq!(engine.play(1, &tone), Status::NoDevice);
            assert_eq!(engine.snapshot().state, PlayState::Error);
            assert_eq!(engine.pause(), Status::Error, "nothing to pause");
            assert_eq!(engine.stop(), Status::Ok);
            assert_eq!(engine.snapshot(), Snapshot::IDLE);
            // The file is checked before the device: a bad file says so.
            assert_eq!(engine.play(1, &dir.0.join("gone.wav")), Status::NoFile);
        }

        #[test]
        fn pause_and_resume_with_nothing_playing_are_errors_that_change_nothing() {
            let engine = Engine::start(null_opener());
            assert_eq!(engine.pause(), Status::Error);
            assert_eq!(engine.resume(), Status::Error);
            assert_eq!(engine.snapshot(), Snapshot::IDLE);
        }

        #[test]
        fn the_end_of_a_program_stops_only_the_sound_it_started() {
            let dir = Scratch::new("release");
            let tone = dir.file("tone.wav", &wav(2000));
            let engine = Engine::start(null_opener());
            assert_eq!(engine.play(7, &tone), Status::Ok);
            engine.release(8);
            assert_eq!(engine.snapshot().state, PlayState::Playing, "another program's end is not this sound's");
            engine.release(7);
            let (s, _) = wait_for(&engine, Duration::from_secs(5), |s| s.state != PlayState::Playing);
            assert_eq!(s, Snapshot::IDLE, "the owner ended: its sound stops");
        }

        #[test]
        fn a_thread_that_dies_reads_as_error_not_a_hang() {
            let dir = Scratch::new("panic");
            let tone = dir.file("tone.wav", &wav(500));
            let engine = Engine::start(Box::new(|_track: Track| panic!("a device that panics")));
            let t0 = Instant::now();
            assert_eq!(engine.play(1, &tone), Status::Error);
            assert!(t0.elapsed() < REPLY_TIMEOUT, "the caller is released at once");
            let (s, _) = wait_for(&engine, Duration::from_secs(5), |s| s.state == PlayState::Error);
            assert_eq!(s.state, PlayState::Error);
            assert_eq!(engine.stop(), Status::Error, "the thread is gone");
        }

        #[test]
        fn the_thread_ends_when_the_player_is_dropped() {
            let dir = Scratch::new("drop");
            let tone = dir.file("tone.wav", &wav(2000));
            let engine = Engine::start(null_opener());
            assert_eq!(engine.play(1, &tone), Status::Ok);
            let weak = engine.shared();
            drop(engine);
            let t0 = Instant::now();
            while weak.upgrade().is_some() && t0.elapsed() < Duration::from_secs(5) {
                std::thread::sleep(Duration::from_millis(5));
            }
            assert!(weak.upgrade().is_none(), "the audio thread let go of the player and ended");
        }

        /// An MP3 without a length header: the length reads 0 while it plays,
        /// and is the position it ended at once it has. Driven on the worker
        /// directly, since every fixture a test can write declares its length.
        #[test]
        fn a_length_the_file_does_not_declare_reads_zero_until_the_end() {
            struct Undeclared(Clock);
            impl Sink for Undeclared {
                fn pause(&mut self) {}
                fn resume(&mut self) {}
                fn position(&self) -> Duration {
                    self.0.elapsed().min(Duration::from_millis(150))
                }
                fn drained(&self) -> bool {
                    self.0.elapsed() >= Duration::from_millis(150)
                }
            }
            let shared = Arc::new(Mutex::new(Snapshot::IDLE));
            let mut w = Worker::new(null_opener(), Arc::clone(&shared));
            w.sink = Some(Box::new(Undeclared(Clock::running())));
            w.state = PlayState::Playing;
            w.tick();
            w.publish();
            let s = *shared.lock().unwrap();
            assert_eq!((s.state, s.duration_ms), (PlayState::Playing, 0), "unknown while playing: {s:?}");
            let t0 = Instant::now();
            while w.state == PlayState::Playing && t0.elapsed() < Duration::from_secs(5) {
                std::thread::sleep(Duration::from_millis(5));
                w.tick();
            }
            w.publish();
            let s = *shared.lock().unwrap();
            assert_eq!(s, Snapshot { state: PlayState::Ended, position_ms: 150, duration_ms: 150 });
        }

        #[test]
        fn the_words_are_the_documented_ones() {
            let states: Vec<&str> = [PlayState::Idle, PlayState::Playing, PlayState::Paused, PlayState::Ended, PlayState::Error]
                .iter()
                .map(|s| s.as_word())
                .collect();
            assert_eq!(states, ["IDLE", "PLAYING", "PAUSED", "ENDED", "ERROR"]);
            let statuses: Vec<&str> = [Status::Ok, Status::NoFile, Status::BadFormat, Status::NoDevice, Status::NoFeature, Status::Error]
                .iter()
                .map(|s| s.as_word())
                .collect();
            assert_eq!(statuses, ["OK", "NOFILE", "BADFORMAT", "NODEVICE", "NOFEATURE", "ERROR"]);
            assert!(super::super::null_output_requested(Some("null")));
            assert!(super::super::null_output_requested(Some(" NULL ")));
            assert!(!super::super::null_output_requested(Some("pulse")));
            assert!(!super::super::null_output_requested(None));
        }
    }
}
