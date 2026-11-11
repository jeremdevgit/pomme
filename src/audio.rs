// SPDX-License-Identifier: GPL-3.0-or-later — Copyright (C) 2026 jeremdevgit — voir LICENSE
//! Moteur audio des fichiers locaux : décodage sans perte (FLAC, ALAC, WAV,
//! AIFF) et compressé (MP3, AAC, Vorbis), égaliseur 10 bandes, rééchantillonnage
//! vers la carte son. Un fil décode par avance (~0,5 s), la sortie WASAPI lit.
//!
//! Le fil de décodage dort quand le tampon est plein : en lecture, la charge
//! processeur reste de l'ordre de 1 %.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rubato::audioadapter_buffers::direct::SequentialSliceOfVecs;
use rubato::audioadapter_buffers::owned::InterleavedOwned;
use rubato::{Async, FixedAsync, Indexing, Resampler, SincInterpolationParameters, SincInterpolationType, WindowFunction};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{Decoder, DecoderOptions};
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::core::units::Time;

/// Fréquences centrales de l'égaliseur (Hz).
pub const EQ_FREQS: [f32; 10] = [31.0, 62.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0];

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct EqSettings {
    pub enabled: bool,
    pub preamp: f32,
    pub gains: [f32; 10],
}

pub enum Cmd {
    /// gain : correction ReplayGain en dB (0 si désactivée)
    Load { path: String, start: f64, play: bool, gain: f32 },
    /// sortie audio choisie (None = celle de Windows)
    Device(Option<String>),
    Play,
    Pause,
    Stop,
    Seek(f64),
    Volume(f32),
    Eq(EqSettings),
}

/// Événements renvoyés à l'interface.
pub enum AudioEvent {
    Loaded { duration: f64, rate: u32, bits: u32, channels: u32, codec: String },
    Position(f64),
    Playing(bool),
    Ended,
    Error(String),
}

struct Shared {
    buf: Mutex<VecDeque<f32>>,
    /// gain linéaire du morceau (normalisation)
    gain: Mutex<f32>,
    /// trames jouées depuis le dernier repère
    played: AtomicU64,
    paused: AtomicBool,
    volume: Mutex<f32>,
    /// sortie perdue (casque débranché, carte son désactivée) : à rouvrir
    lost: AtomicBool,
}

const OUT_CH_STEREO: usize = 2;

pub struct Engine {
    tx: Sender<Cmd>,
}

impl Engine {
    pub fn start(events: impl Fn(AudioEvent) + Send + 'static) -> Engine {
        let (tx, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("pomme-audio".into())
            .spawn(move || run(rx, events))
            .expect("fil audio");
        Engine { tx }
    }

    pub fn send(&self, c: Cmd) {
        let _ = self.tx.send(c);
    }
}

// ── égaliseur ───────────────────────────────────────────────────────────

#[derive(Clone, Copy, Default)]
struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
}

impl Biquad {
    /// Filtre en cloche (RBJ), Q ≈ 1,41 : une octave par bande.
    fn peaking(rate: f32, freq: f32, gain_db: f32) -> Biquad {
        let a = 10f32.powf(gain_db / 40.0);
        let w0 = 2.0 * std::f32::consts::PI * freq.min(rate * 0.45) / rate;
        let alpha = w0.sin() / (2.0 * 1.41);
        let cos = w0.cos();
        let a0 = 1.0 + alpha / a;
        Biquad {
            b0: (1.0 + alpha * a) / a0,
            b1: (-2.0 * cos) / a0,
            b2: (1.0 - alpha * a) / a0,
            a1: (-2.0 * cos) / a0,
            a2: (1.0 - alpha / a) / a0,
        }
    }
}

struct Eq {
    on: bool,
    pre: f32,
    filters: Vec<Biquad>,
    /// état (x1, x2, y1, y2) par filtre et par canal
    state: Vec<[[f32; 4]; OUT_CH_STEREO]>,
}

impl Eq {
    fn new(s: &EqSettings, rate: u32) -> Eq {
        let filters: Vec<Biquad> = EQ_FREQS
            .iter()
            .zip(s.gains.iter())
            .filter(|(_, g)| g.abs() > 0.05)
            .map(|(&f, &g)| Biquad::peaking(rate as f32, f, g.clamp(-12.0, 12.0)))
            .collect();
        Eq {
            on: s.enabled && (!filters.is_empty() || s.preamp.abs() > 0.05),
            pre: 10f32.powf(s.preamp.clamp(-12.0, 12.0) / 20.0),
            state: vec![[[0.0; 4]; OUT_CH_STEREO]; filters.len()],
            filters,
        }
    }

    fn process(&mut self, planar: &mut [Vec<f32>]) {
        if !self.on {
            return;
        }
        for (ch, samples) in planar.iter_mut().enumerate().take(OUT_CH_STEREO) {
            for s in samples.iter_mut() {
                let mut x = *s * self.pre;
                for (f, st) in self.filters.iter().zip(self.state.iter_mut()) {
                    let z = &mut st[ch];
                    let y = f.b0 * x + f.b1 * z[0] + f.b2 * z[1] - f.a1 * z[2] - f.a2 * z[3];
                    z[1] = z[0];
                    z[0] = x;
                    z[3] = z[2];
                    z[2] = y;
                    x = y;
                }
                // limiteur doux : un gain positif ne doit pas faire saturer
                *s = if x.abs() > 0.95 { x.signum() * (0.95 + (x.abs() - 0.95).tanh() * 0.05) } else { x };
            }
        }
    }
}

// ── lecture d'un fichier ────────────────────────────────────────────────

struct Track {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn Decoder>,
    track_id: u32,
    rate: u32,
    resampler: Option<Async<f32>>,
    /// échantillons en attente du rééchantillonneur (par canal)
    pending: Vec<Vec<f32>>,
    eq: Eq,
    done: bool,
}

fn open(path: &str, out_rate: u32, eq: &EqSettings) -> Result<(Track, AudioEvent), String> {
    let file = std::fs::File::open(path).map_err(|e| format!("Fichier illisible : {e}"))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = std::path::Path::new(path).extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let fmt_opts = FormatOptions {
        enable_gapless: true,
        ..Default::default()
    };
    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &fmt_opts, &MetadataOptions::default())
        .map_err(|e| format!("Format non pris en charge : {e}"))?;
    let format = probed.format;
    let track = format.default_track().ok_or("Aucune piste audio")?;
    let p = &track.codec_params;
    let rate = p.sample_rate.unwrap_or(44100);
    let duration = match (p.n_frames, p.time_base) {
        (Some(n), Some(tb)) => {
            let t = tb.calc_time(n);
            t.seconds as f64 + t.frac
        }
        (Some(n), None) => n as f64 / rate as f64,
        _ => 0.0,
    };
    let codec = symphonia::default::get_codecs()
        .get_codec(p.codec)
        .map(|c| c.short_name.to_uppercase())
        .unwrap_or_default();
    let info = AudioEvent::Loaded {
        duration,
        rate,
        bits: p.bits_per_sample.unwrap_or(0),
        channels: p.channels.map(|c| c.count() as u32).unwrap_or(2),
        codec,
    };
    let decoder = symphonia::default::get_codecs()
        .make(p, &DecoderOptions::default())
        .map_err(|e| format!("Décodeur indisponible : {e}"))?;
    let track_id = track.id;
    let resampler = (rate != out_rate).then(|| make_resampler(rate, out_rate));
    Ok((
        Track {
            format,
            decoder,
            track_id,
            rate,
            resampler,
            pending: vec![Vec::new(), Vec::new()],
            eq: Eq::new(eq, rate),
            done: false,
        },
        info,
    ))
}

fn make_resampler(from: u32, to: u32) -> Async<f32> {
    let params = SincInterpolationParameters {
        sinc_len: 128,
        f_cutoff: Some(0.95),
        interpolation: SincInterpolationType::Linear,
        oversampling_factor: 128,
        window: WindowFunction::BlackmanHarris2,
    };
    Async::<f32>::new_sinc(to as f64 / from as f64, 1.0, &params, 1024, OUT_CH_STEREO, FixedAsync::Input).expect("rééchantillonneur")
}

/// Rééchantillonne un bloc stéréo planaire ; renvoie des trames entrelacées.
/// `partial` : dernier bloc, plus court que demandé (complété par du silence).
fn resample(rs: &mut Async<f32>, chunk: &[Vec<f32>], partial: bool) -> Option<Vec<f32>> {
    let frames = chunk[0].len();
    let input = SequentialSliceOfVecs::new(chunk, OUT_CH_STEREO, frames).ok()?;
    let mut out = InterleavedOwned::new(0.0f32, OUT_CH_STEREO, rs.output_frames_max());
    let indexing = partial.then(|| Indexing::new().partial_len(frames));
    let (_, written) = rs.process_into_buffer(&input, &mut out, indexing.as_ref()).ok()?;
    let mut data = out.take_data();
    data.truncate(written * OUT_CH_STEREO);
    Some(data)
}

impl Track {
    /// Décode un paquet ; renvoie des trames stéréo entrelacées au débit de sortie.
    fn next(&mut self) -> Option<Vec<f32>> {
        loop {
            let packet = match self.format.next_packet() {
                Ok(p) => p,
                Err(_) => {
                    self.done = true;
                    return self.flush();
                }
            };
            if packet.track_id() != self.track_id {
                continue;
            }
            let decoded = match self.decoder.decode(&packet) {
                Ok(d) => d,
                Err(SymError::DecodeError(_)) => continue,
                Err(_) => {
                    self.done = true;
                    return self.flush();
                }
            };
            let spec = *decoded.spec();
            let ch = spec.channels.count().max(1);
            let mut sb = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
            sb.copy_interleaved_ref(decoded);
            let s = sb.samples();
            let frames = s.len() / ch;
            // vers la stéréo : mono dupliqué, multicanal réduit aux deux premiers
            let mut planar = vec![Vec::with_capacity(frames), Vec::with_capacity(frames)];
            for f in 0..frames {
                let l = s[f * ch];
                let r = if ch > 1 { s[f * ch + 1] } else { l };
                planar[0].push(l);
                planar[1].push(r);
            }
            self.eq.process(&mut planar);
            match self.resampler.as_mut() {
                None => return Some(interleave(&planar)),
                Some(_) => {
                    self.pending[0].extend_from_slice(&planar[0]);
                    self.pending[1].extend_from_slice(&planar[1]);
                    if let Some(out) = self.resample_ready() {
                        return Some(out);
                    }
                }
            }
        }
    }

    fn resample_ready(&mut self) -> Option<Vec<f32>> {
        let rs = self.resampler.as_mut()?;
        let mut out: Vec<f32> = Vec::new();
        loop {
            let need = rs.input_frames_next();
            if self.pending[0].len() < need {
                break;
            }
            let chunk: Vec<Vec<f32>> = self.pending.iter_mut().map(|c| c.drain(..need).collect()).collect();
            if let Some(r) = resample(rs, &chunk, false) {
                out.extend_from_slice(&r);
            }
        }
        (!out.is_empty()).then_some(out)
    }

    fn flush(&mut self) -> Option<Vec<f32>> {
        let rs = self.resampler.as_mut()?;
        if self.pending[0].is_empty() {
            return None;
        }
        let chunk = std::mem::take(&mut self.pending);
        self.pending = vec![Vec::new(), Vec::new()];
        resample(rs, &chunk, true)
    }

    fn seek(&mut self, secs: f64) {
        let t = Time::new(secs.max(0.0) as u64, secs.max(0.0).fract());
        let _ = self.format.seek(
            SeekMode::Accurate,
            SeekTo::Time {
                time: t,
                track_id: Some(self.track_id),
            },
        );
        self.decoder.reset();
        if let Some(rs) = self.resampler.as_mut() {
            rs.reset();
        }
        self.pending = vec![Vec::new(), Vec::new()];
        self.done = false;
    }
}

fn interleave(p: &[Vec<f32>]) -> Vec<f32> {
    let n = p[0].len().min(p[1].len());
    let mut v = Vec::with_capacity(n * 2);
    for i in 0..n {
        v.push(p[0][i]);
        v.push(p[1][i]);
    }
    v
}

// ── sortie WASAPI ───────────────────────────────────────────────────────

/// Noms des sorties audio disponibles.
pub fn devices() -> Vec<String> {
    let host = cpal::default_host();
    host.output_devices()
        .map(|it| it.filter_map(|d| d.description().ok().map(|d| d.name().to_string())).collect())
        .unwrap_or_default()
}

fn default_name() -> Option<String> {
    cpal::default_host().default_output_device().and_then(|d| d.description().ok().map(|d| d.name().to_string()))
}

/// Ouvre la sortie ; renvoie aussi le nom du périphérique réellement utilisé.
fn open_output(shared: Arc<Shared>, wanted: &Option<String>) -> Result<(cpal::Stream, u32, String), String> {
    let host = cpal::default_host();
    let chosen = wanted.as_ref().and_then(|w| host.output_devices().ok()?.find(|d| d.description().ok().is_some_and(|d| d.name() == w.as_str())));
    let device = chosen.or_else(|| host.default_output_device()).ok_or("Aucune sortie audio")?;
    let name = device.description().map(|d| d.name().to_string()).unwrap_or_default();
    let supported = device.default_output_config().map_err(|e| e.to_string())?;
    let fmt = supported.sample_format();
    let config: cpal::StreamConfig = supported.into();
    let out_ch = config.channels as usize;
    let rate = config.sample_rate;
    let lost = shared.clone();
    let err = move |_| lost.lost.store(true, Ordering::Relaxed);
    let sh = shared.clone();
    let stream = match fmt {
        cpal::SampleFormat::F32 => device.build_output_stream(
            config,
            move |out: &mut [f32], _| write_out(&sh, out_ch, out.len(), |i, v| out[i] = v),
            err,
            None,
        ),
        cpal::SampleFormat::I16 => device.build_output_stream(
            config,
            move |out: &mut [i16], _| {
                let n = out.len();
                write_out(&sh, out_ch, n, |i, v| out[i] = (v.clamp(-1.0, 1.0) * 32767.0) as i16)
            },
            err,
            None,
        ),
        other => return Err(format!("Format de sortie non géré : {other:?}")),
    }
    .map_err(|e| e.to_string())?;
    stream.play().map_err(|e| e.to_string())?;
    shared.lost.store(false, Ordering::Relaxed);
    Ok((stream, rate, name))
}

/// Appelé par la carte son : copie du tampon, silence si vide ou en pause.
fn write_out(sh: &Shared, out_ch: usize, len: usize, mut put: impl FnMut(usize, f32)) {
    let frames = len / out_ch.max(1);
    let paused = sh.paused.load(Ordering::Relaxed);
    // volume en courbe perceptive, normalisation (ReplayGain) linéaire
    let vol = *sh.volume.lock().unwrap();
    let g = vol * vol * *sh.gain.lock().unwrap();
    let mut buf = sh.buf.lock().unwrap();
    let mut played = 0u64;
    for f in 0..frames {
        let (l, r) = if paused || buf.len() < 2 {
            (0.0, 0.0)
        } else {
            played += 1;
            (buf.pop_front().unwrap_or(0.0), buf.pop_front().unwrap_or(0.0))
        };
        for c in 0..out_ch {
            let v = match (out_ch, c) {
                (1, _) => (l + r) * 0.5,
                (_, 0) => l,
                (_, 1) => r,
                _ => 0.0,
            };
            put(f * out_ch + c, (v * g).clamp(-1.0, 1.0));
        }
    }
    drop(buf);
    sh.played.fetch_add(played, Ordering::Relaxed);
}

// ── boucle principale du fil audio ──────────────────────────────────────

type Output = Option<(cpal::Stream, u32, String)>;

/// Nouvelle sortie (choix de l'utilisateur, sortie perdue) : le morceau en
/// cours reprend au même endroit.
fn switch_output(shared: &Arc<Shared>, device: &Option<String>, eq: &EqSettings, current: &Option<String>, output: &mut Output, track: &mut Option<Track>, base: &mut f64) {
    let rate = output.as_ref().map(|o| o.1).unwrap_or(48000);
    let pos = *base + shared.played.load(Ordering::Relaxed) as f64 / rate as f64;
    *output = None;
    let Ok(o) = open_output(shared.clone(), device) else { return };
    let r = o.1;
    *output = Some(o);
    shared.buf.lock().unwrap().clear();
    shared.played.store(0, Ordering::Relaxed);
    if let (Some(path), true) = (current, track.is_some()) {
        if let Ok((mut t, _)) = open(path, r, eq) {
            t.seek(pos);
            *base = pos;
            *track = Some(t);
        }
    }
}

fn run(rx: Receiver<Cmd>, events: impl Fn(AudioEvent)) {
    let shared = Arc::new(Shared {
        buf: Mutex::new(VecDeque::new()),
        played: AtomicU64::new(0),
        paused: AtomicBool::new(true),
        volume: Mutex::new(0.8),
        gain: Mutex::new(1.0),
        lost: AtomicBool::new(false),
    });
    let mut device: Option<String> = None;
    let mut current: Option<String> = None;
    let mut output: Output = None;
    let mut track: Option<Track> = None;
    let mut eq = EqSettings::default();
    let mut base = 0.0f64;
    let mut last_report = 0.0f64;
    let mut ended_sent = false;

    loop {
        let rate = output.as_ref().map(|o| o.1).unwrap_or(48000);
        let target = (rate as usize / 2) * 2; // ~0,5 s de stéréo
        let playing = !shared.paused.load(Ordering::Relaxed);
        let buffered = shared.buf.lock().unwrap().len();
        let need_decode = track.as_ref().is_some_and(|t| !t.done) && buffered < target;
        let timeout = if need_decode {
            Duration::ZERO
        } else if playing {
            Duration::from_millis(100)
        } else {
            Duration::from_secs(3600)
        };

        match rx.recv_timeout(timeout) {
            Ok(cmd) => match cmd {
                Cmd::Load { path, start, play, gain } => {
                    *shared.gain.lock().unwrap() = 10f32.powf(gain.clamp(-24.0, 12.0) / 20.0);
                    current = Some(path.clone());
                    // sortie perdue, ou sortie par défaut de Windows changée depuis le dernier morceau
                    let moved = device.is_none() && output.as_ref().is_some_and(|o| default_name().is_some_and(|n| n != o.2));
                    if moved || shared.lost.load(Ordering::Relaxed) {
                        output = None;
                    }
                    if output.is_none() {
                        match open_output(shared.clone(), &device) {
                            Ok(o) => output = Some(o),
                            Err(e) => {
                                events(AudioEvent::Error(e));
                                continue;
                            }
                        }
                    }
                    let rate = output.as_ref().unwrap().1;
                    shared.buf.lock().unwrap().clear();
                    shared.played.store(0, Ordering::Relaxed);
                    let opened = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| open(&path, rate, &eq)))
                        .unwrap_or_else(|_| Err("Fichier audio endommagé".into()));
                    match opened {
                        Ok((mut t, info)) => {
                            if start > 0.0 {
                                t.seek(start);
                            }
                            base = start;
                            track = Some(t);
                            ended_sent = false;
                            events(info);
                            shared.paused.store(!play, Ordering::Relaxed);
                            events(AudioEvent::Playing(play));
                        }
                        Err(e) => {
                            track = None;
                            events(AudioEvent::Error(e));
                        }
                    }
                }
                Cmd::Play => {
                    if track.is_some() {
                        shared.paused.store(false, Ordering::Relaxed);
                        events(AudioEvent::Playing(true));
                    }
                }
                Cmd::Pause => {
                    shared.paused.store(true, Ordering::Relaxed);
                    events(AudioEvent::Playing(false));
                }
                Cmd::Stop => {
                    shared.paused.store(true, Ordering::Relaxed);
                    shared.buf.lock().unwrap().clear();
                    track = None;
                    // la carte son est libérée : plus aucun réveil en arrière-plan
                    output = None;
                    events(AudioEvent::Playing(false));
                }
                Cmd::Seek(s) => {
                    if let Some(t) = track.as_mut() {
                        shared.buf.lock().unwrap().clear();
                        t.seek(s);
                        base = s;
                        shared.played.store(0, Ordering::Relaxed);
                        ended_sent = false;
                        events(AudioEvent::Position(s));
                    }
                }
                Cmd::Volume(v) => *shared.volume.lock().unwrap() = v.clamp(0.0, 1.0),
                Cmd::Device(d) => {
                    device = d;
                    if output.is_some() {
                        switch_output(&shared, &device, &eq, &current, &mut output, &mut track, &mut base);
                    }
                }
                Cmd::Eq(s) => {
                    if let Some(t) = track.as_mut() {
                        t.eq = Eq::new(&s, t.rate);
                    }
                    eq = s;
                }
            },
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
        }

        // casque débranché, carte son désactivée : la lecture continue sur la sortie disponible
        if output.is_some() && shared.lost.load(Ordering::Relaxed) {
            switch_output(&shared, &device, &eq, &current, &mut output, &mut track, &mut base);
            if output.is_none() {
                shared.paused.store(true, Ordering::Relaxed);
                events(AudioEvent::Playing(false));
                events(AudioEvent::Error("Sortie audio indisponible".into()));
            }
        }

        // remplissage du tampon
        if let Some(t) = track.as_mut() {
            let mut buffered = shared.buf.lock().unwrap().len();
            while buffered < target && !t.done {
                // un fichier corrompu ne doit jamais faire tomber l'application
                let chunk = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| t.next())).unwrap_or_else(|_| {
                    t.done = true;
                    events(AudioEvent::Error("Fichier audio endommagé".into()));
                    None
                });
                match chunk {
                    Some(chunk) => {
                        let mut b = shared.buf.lock().unwrap();
                        b.extend(chunk);
                        buffered = b.len();
                    }
                    None => break,
                }
            }
            // position et fin de piste
            let pos = base + shared.played.load(Ordering::Relaxed) as f64 / rate as f64;
            if (pos - last_report).abs() >= 0.25 {
                last_report = pos;
                events(AudioEvent::Position(pos));
            }
            if t.done && shared.buf.lock().unwrap().len() < 2 && !ended_sent {
                ended_sent = true;
                events(AudioEvent::Ended);
            }
        }
    }
}
