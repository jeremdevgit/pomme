// SPDX-License-Identifier: GPL-3.0-or-later — Copyright (C) 2026 jeremdevgit
//! Diffusion volontaire sur le réseau local. Aucun service au lancement.
//! Capture limitée à l'arbre de processus de Pomme (Windows 11 / build 20348+).
//! Les contenus protégés peuvent être rendus silencieux par Windows : aucun contournement DRM.
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

type Res<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;
const TIMEOUT: Duration = Duration::from_secs(5);
const SAMPLE_RATE: u32 = 44100;
const MAX_PCM: usize = SAMPLE_RATE as usize * 4 / 2;
#[derive(Clone, Serialize)]
pub struct Device {
    id: String,
    name: String,
    protocol: String,
    address: Ipv4Addr,
    port: u16,
}
#[derive(Default)]
struct State {
    devices: Vec<Device>,
    active: Option<Arc<AtomicBool>>,
    scanning: bool,
}
static STATE: OnceLock<Mutex<State>> = OnceLock::new();
fn state() -> &'static Mutex<State> {
    STATE.get_or_init(|| Mutex::new(State::default()))
}
fn emit(v: Value) {
    crate::post(crate::UserEvent::ToUi(v));
}
fn local_address(ip: Ipv4Addr) -> bool {
    ip.is_private() || ip.is_link_local()
}
fn status(s: &str, message: &str) {
    emit(json!({"type":"network.status", "state":s, "message":message}));
}

pub fn stop() {
    if let Some(cancel) = state().lock().unwrap().active.take() {
        cancel.store(true, Ordering::Relaxed);
    }
    status("stopped", "");
}
pub fn scan() {
    {
        let mut s = state().lock().unwrap();
        if s.scanning {
            return;
        }
        s.scanning = true;
    }
    std::thread::spawn(|| {
        let result = discover();
        let mut s = state().lock().unwrap();
        s.scanning = false;
        match result {
            Ok(devices) => {
                s.devices = devices;
                emit(json!({"type":"network.devices", "devices":s.devices}));
            }
            Err(e) => {
                drop(s);
                status("error", &e.to_string());
            }
        }
    });
}
fn discover() -> Res<Vec<Device>> {
    use mdns_sd::{ServiceDaemon, ServiceEvent};
    let daemon = ServiceDaemon::new()?;
    let kinds = ["_airplay._tcp.local.", "_googlecast._tcp.local."];
    let result = (|| -> Res<Vec<Device>> {
        let receivers = [daemon.browse(kinds[0])?, daemon.browse(kinds[1])?];
        let mut devices: Vec<Device> = vec![];
        let until = Instant::now() + Duration::from_secs(6);
        while Instant::now() < until {
            for (i, receiver) in receivers.iter().enumerate() {
                while let Ok(event) = receiver.try_recv() {
                    if let ServiceEvent::ServiceResolved(info) = event {
                        let Some(address) = info
                            .get_addresses_v4()
                            .into_iter()
                            .find(|ip| local_address(*ip))
                        else {
                            continue;
                        };
                        let id = info.get_fullname().to_string();
                        if devices.len() >= 64
                            || devices.iter().any(|d| d.id == id)
                            || info.get_port() == 0
                        {
                            continue;
                        }
                        let name = info
                            .get_property_val_str("fn")
                            .or_else(|| info.get_property_val_str("name"))
                            .unwrap_or_else(|| info.get_hostname().trim_end_matches(".local."))
                            .chars()
                            .take(100)
                            .collect();
                        devices.push(Device {
                            id,
                            name,
                            address,
                            port: info.get_port(),
                            protocol: if i == 0 { "airplay" } else { "cast" }.into(),
                        });
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        devices.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(devices)
    })();
    for kind in kinds {
        let _ = daemon.stop_browse(kind);
    }
    let _ = daemon.shutdown();
    result
}
pub fn start(id: String) {
    let device = {
        let s = state().lock().unwrap();
        s.devices.iter().find(|d| d.id == id).cloned()
    };
    let Some(device) = device else {
        status("error", "Appareil introuvable. Relancez la recherche.");
        return;
    };
    stop();
    let cancel = Arc::new(AtomicBool::new(false));
    state().lock().unwrap().active = Some(cancel.clone());
    status("connecting", &device.name);
    std::thread::spawn(move || {
        let pcm = Arc::new(Mutex::new(VecDeque::with_capacity(MAX_PCM)));
        let capture_cancel = cancel.clone();
        let capture_pcm = pcm.clone();
        let capture_error = Arc::new(Mutex::new(None::<String>));
        let error_for_capture = capture_error.clone();
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        let capture = std::thread::spawn(move || {
            let result = capture(capture_pcm, capture_cancel.clone(), tx);
            if let Err(e) = result {
                *error_for_capture.lock().unwrap() = Some(format!(
                    "Capture audio indisponible (Windows 11 requis) : {e}"
                ));
                capture_cancel.store(true, Ordering::Relaxed);
            }
        });
        // Ne pas annoncer une connexion si Windows refuse la capture de Pomme.
        let result = match rx.recv_timeout(TIMEOUT) {
            Ok(()) if !cancel.load(Ordering::Relaxed) => {
                if device.protocol == "airplay" {
                    airplay(&device, &cancel, &pcm)
                } else {
                    cast(&device, &cancel, &pcm)
                }
            }
            _ => Err("Capture audio indisponible".into()),
        };
        cancel.store(true, Ordering::Relaxed);
        let _ = capture.join();
        // Une ancienne session ne remplace jamais le statut de la nouvelle.
        let mut s = state().lock().unwrap();
        if s.active.as_ref().is_some_and(|a| Arc::ptr_eq(a, &cancel)) {
            s.active = None;
            drop(s);
            if let Some(error) = capture_error.lock().unwrap().take() {
                status("error", &error);
            } else if let Err(e) = result {
                status("error", &e.to_string());
            } else {
                status("stopped", "");
            }
        }
    });
}
fn capture(
    pcm: Arc<Mutex<VecDeque<u8>>>,
    cancel: Arc<AtomicBool>,
    ready: std::sync::mpsc::SyncSender<()>,
) -> Res<()> {
    use wasapi::*;
    initialize_mta().ok().map_err(|e| format!("COM : {e}"))?;
    struct Com;
    impl Drop for Com {
        fn drop(&mut self) {
            deinitialize();
        }
    }
    let _com = Com;
    let mut client = AudioClient::new_application_loopback_client(std::process::id(), true)?;
    let format = WaveFormat::new(16, 16, SampleType::Int, SAMPLE_RATE as usize, 2, None);
    client.initialize_client(
        &format,
        Direction::Capture,
        StreamMode::EventsShared {
            autoconvert: true,
            buffer_duration_hns: 0,
        },
        None,
    )?;
    let event = client.set_get_eventhandle()?;
    let reader = client.get_audiocaptureclient()?;
    client.start_stream()?;
    let _ = ready.send(());
    let mut bytes = VecDeque::new();
    while !cancel.load(Ordering::Relaxed) {
        if reader.get_next_packet_size()?.unwrap_or(0) > 0 {
            reader.read_from_device_to_deque(&mut bytes)?;
            let mut queue = pcm.lock().unwrap();
            // Tampon borné à 0,5 s, aligné sur des trames stéréo de 4 octets.
            queue.append(&mut bytes);
            if queue.len() > MAX_PCM {
                let discard = (queue.len() - MAX_PCM + 3) / 4 * 4;
                queue.drain(..discard);
            }
        }
        let _ = event.wait_for_event(50);
    }
    client.stop_stream()?;
    Ok(())
}
fn samples(pcm: &Mutex<VecDeque<u8>>, n: usize) -> Vec<u8> {
    let mut queue = pcm.lock().unwrap();
    (0..n).map(|_| queue.pop_front().unwrap_or(0)).collect()
}
fn airplay(device: &Device, cancel: &AtomicBool, pcm: &Mutex<VecDeque<u8>>) -> Res<()> {
    use cidermill_airplay::audio::{
        Action, AudioSendConfig, AudioSendState, StreamFormat, StreamType, TimingMode,
    };
    use cidermill_airplay::pairing::TransientStyle;
    use cidermill_airplay::{Auth, ControlChannel, NtpTimestamp, OsRng};
    let host = device.address.to_string();
    let mut channel = ControlChannel::builder(&host)
        .port(device.port)
        .timeout(TIMEOUT)
        .connect()?;
    channel.authenticate(Auth::Transient(TransientStyle::Ios26), &mut OsRng)
        .map_err(|e| format!("AirPlay : cet appareil refuse l'association temporaire. Les appareils exigeant un PIN ou une association Apple TV ne sont pas encore pris en charge : {e}"))?;
    let _session = channel.start_media_session()?;
    let local = channel.connection().local_ip();
    let data = UdpSocket::bind((local, 0))?;
    let control = UdpSocket::bind((local, 0))?;
    data.set_write_timeout(Some(TIMEOUT))?;
    control.set_write_timeout(Some(TIMEOUT))?;
    let format = StreamFormat::ALAC_44100_16_STEREO;
    let config = AudioSendConfig::generate(
        channel.connection(),
        StreamType::Realtime,
        format,
        control.local_addr()?.port(),
        TimingMode::Ntp,
        &mut OsRng,
    );
    let mut machine = AudioSendState::new(config);
    let mut packetizer = machine.packetizer()?;
    let clock = Instant::now();
    let mut streaming: Option<Instant> = None;
    let mut syncs = 0;
    let setup_deadline = Instant::now() + Duration::from_secs(20);
    while !cancel.load(Ordering::Relaxed) {
        match machine.poll(clock.elapsed()) {
            Action::Send { request, .. } => {
                let response = channel.session_request(request)?;
                machine.on_response(&response)?;
            }
            Action::ConnectData {
                data_port,
                control_port,
                ..
            } => {
                data.connect((device.address, data_port))?;
                control.connect((device.address, control_port))?;
                machine.data_connected();
            }
            Action::StartStreaming { .. } => {
                streaming = Some(Instant::now());
                status("streaming", &device.name);
            }
            Action::Failed { reason, .. } => return Err(reason.into()),
            Action::Finished => return Ok(()),
            _ => {
                if let Some(started) = streaming {
                    let elapsed = started.elapsed();
                    if elapsed.as_secs() >= syncs {
                        control.send(
                            &packetizer
                                .sync_packet(syncs == 0, 0, NtpTimestamp::now())
                                .encode(),
                        )?;
                        syncs += 1;
                    }
                    let interval =
                        f64::from(format.frames_per_packet) / f64::from(format.sample_rate);
                    // Un paquet par tour : aucune boucle de rattrapage non bornée.
                    let raw = samples(pcm, format.frames_per_packet as usize * 4);
                    let values: Vec<i32> = raw
                        .chunks_exact(2)
                        .map(|b| i16::from_le_bytes([b[0], b[1]]) as i32)
                        .collect();
                    data.send(&packetizer.packet(&values)?)?;
                    let next = Duration::from_secs_f64(packetizer.packets_sent() as f64 * interval);
                    std::thread::sleep(
                        next.saturating_sub(started.elapsed())
                            .min(Duration::from_millis(20)),
                    );
                } else {
                    if Instant::now() > setup_deadline {
                        return Err("Délai de connexion AirPlay dépassé".into());
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
        }
    }
    machine.stop();
    // TEARDOWN avec les mêmes délais bornés que la connexion.
    for _ in 0..4 {
        if let Action::Send { request, .. } = machine.poll(clock.elapsed()) {
            let response = channel.session_request(request)?;
            machine.on_response(&response)?;
        } else {
            break;
        }
    }
    Ok(())
}

// Cast v2 : le certificat du récepteur est auto-signé. Cette exception reste
// limitée à l'adresse privée découverte puis sélectionnée, sans jeton Apple.
fn cast(device: &Device, cancel: &Arc<AtomicBool>, pcm: &Arc<Mutex<VecDeque<u8>>>) -> Res<()> {
    let tcp = TcpStream::connect_timeout(
        &SocketAddr::new(IpAddr::V4(device.address), device.port),
        TIMEOUT,
    )?;
    tcp.set_read_timeout(Some(TIMEOUT))?;
    tcp.set_write_timeout(Some(TIMEOUT))?;
    let local = tcp.local_addr()?.ip();
    let connector = native_tls::TlsConnector::builder()
        .danger_accept_invalid_certs(true)
        .danger_accept_invalid_hostnames(true)
        .build()?;
    let mut tls = connector.connect(&device.address.to_string(), tcp)?;
    let listener = TcpListener::bind((local, 0))?;
    listener.set_nonblocking(true)?;
    let port = listener.local_addr()?.port();
    let mut random = [0u8; 32];
    getrandom::fill(&mut random)?;
    let token: String = random.iter().map(|b| format!("{b:02x}")).collect();
    let route = format!("/{token}/audio.wav");
    let url = format!("http://{local}:{port}{route}");
    send_cast(
        &mut tls,
        "receiver-0",
        "connection",
        json!({"type":"CONNECT"}),
    )?;
    send_cast(
        &mut tls,
        "receiver-0",
        "receiver",
        json!({"type":"LAUNCH","appId":"CC1AD845","requestId":1}),
    )?;
    let response = wait_cast(&mut tls, 1, cancel)?;
    let app = response["status"]["applications"]
        .as_array()
        .and_then(|a| a.iter().find(|v| v["appId"] == "CC1AD845"))
        .ok_or("Récepteur Chromecast indisponible")?;
    let transport = app["transportId"]
        .as_str()
        .ok_or("Transport Chromecast absent")?
        .to_string();
    let session = app["sessionId"]
        .as_str()
        .ok_or("Session Chromecast absente")?
        .to_string();
    send_cast(
        &mut tls,
        &transport,
        "connection",
        json!({"type":"CONNECT"}),
    )?;
    let http_cancel = cancel.clone();
    let http_pcm = pcm.clone();
    let receiver_connected = Arc::new(AtomicBool::new(false));
    let http_connected = receiver_connected.clone();
    let address = device.address;
    let server = std::thread::spawn(move || {
        serve_pcm(
            listener,
            address,
            route,
            http_cancel,
            http_pcm,
            http_connected,
        )
    });
    let result = (|| -> Res<()> {
        send_cast(
            &mut tls,
            &transport,
            "media",
            json!({"type":"LOAD","requestId":2,"sessionId":session,"autoplay":true,"media":{"contentId":url,"contentType":"audio/wav","streamType":"LIVE","duration":-1}}),
        )?;
        let loaded = wait_cast(&mut tls, 2, cancel)?;
        if loaded["type"] != "MEDIA_STATUS" {
            return Err(format!("Chromecast refuse le flux audio : {}", loaded["type"]).into());
        }
        let deadline = Instant::now() + Duration::from_secs(10);
        while !receiver_connected.load(Ordering::Relaxed) && Instant::now() < deadline {
            if cancel.load(Ordering::Relaxed) {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        if !receiver_connected.load(Ordering::Relaxed) {
            return Err("Chromecast n'a pas ouvert le flux audio local".into());
        }
        status("streaming", &device.name);
        while !cancel.load(Ordering::Relaxed) {
            send_cast(&mut tls, "receiver-0", "heartbeat", json!({"type":"PING"}))?;
            match read_cast(&mut tls) {
                Ok(message) if message["type"] == "PING" => {
                    send_cast(&mut tls, "receiver-0", "heartbeat", json!({"type":"PONG"}))?
                }
                Ok(_) => {}
                Err(e) => return Err(e),
            }
            for _ in 0..20 {
                if cancel.load(Ordering::Relaxed) {
                    break;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        }
        Ok(())
    })();
    let _ = send_cast(
        &mut tls,
        "receiver-0",
        "receiver",
        json!({"type":"STOP","sessionId":session,"requestId":3}),
    );
    cancel.store(true, Ordering::Relaxed);
    let _ = server.join();
    result
}
fn wait_cast(stream: &mut impl ReadWrite, id: u32, cancel: &AtomicBool) -> Res<Value> {
    let until = Instant::now() + Duration::from_secs(20);
    while Instant::now() < until && !cancel.load(Ordering::Relaxed) {
        let value = read_cast(stream)?;
        if value["type"] == "PING" {
            send_cast(stream, "receiver-0", "heartbeat", json!({"type":"PONG"}))?;
        }
        if value["requestId"] == id {
            return Ok(value);
        }
    }
    Err("Délai de réponse Chromecast dépassé".into())
}
trait ReadWrite: Read + Write {}
impl<T: Read + Write> ReadWrite for T {}
fn varint(mut n: usize, out: &mut Vec<u8>) {
    while n >= 128 {
        out.push((n as u8 & 127) | 128);
        n >>= 7;
    }
    out.push(n as u8);
}
fn field(tag: u8, value: &str, out: &mut Vec<u8>) {
    out.push(tag << 3 | 2);
    varint(value.len(), out);
    out.extend(value.as_bytes());
}
fn send_cast(stream: &mut impl Write, destination: &str, namespace: &str, value: Value) -> Res<()> {
    let mut message = vec![8, 0];
    field(2, "sender-pomme", &mut message);
    field(3, destination, &mut message);
    field(
        4,
        &format!("urn:x-cast:com.google.cast.{namespace}"),
        &mut message,
    );
    message.extend([40, 0]);
    field(6, &value.to_string(), &mut message);
    stream.write_all(&(message.len() as u32).to_be_bytes())?;
    stream.write_all(&message)?;
    stream.flush()?;
    Ok(())
}
fn read_varint(bytes: &[u8], at: &mut usize) -> Res<usize> {
    let mut n = 0usize;
    for shift in (0..35).step_by(7) {
        let b = *bytes.get(*at).ok_or("Protobuf tronqué")?;
        *at += 1;
        n |= usize::from(b & 127) << shift;
        if b & 128 == 0 {
            return Ok(n);
        }
    }
    Err("Protobuf invalide".into())
}
fn cast_payload(bytes: &[u8]) -> Res<Value> {
    let mut at = 0;
    while at < bytes.len() {
        let tag = read_varint(bytes, &mut at)?;
        match tag & 7 {
            0 => {
                read_varint(bytes, &mut at)?;
            }
            2 => {
                let len = read_varint(bytes, &mut at)?;
                let end = at
                    .checked_add(len)
                    .filter(|n| *n <= bytes.len())
                    .ok_or("Trame Cast tronquée")?;
                if tag >> 3 == 6 {
                    return Ok(serde_json::from_slice(&bytes[at..end])?);
                }
                at = end;
            }
            _ => return Err("Trame Cast non prise en charge".into()),
        }
    }
    Ok(Value::Null)
}
fn read_cast(stream: &mut impl Read) -> Res<Value> {
    let mut prefix = [0; 4];
    stream.read_exact(&mut prefix)?;
    let len = u32::from_be_bytes(prefix) as usize;
    if len > 65536 {
        return Err("Trame Cast trop grande".into());
    }
    let mut data = vec![0; len];
    stream.read_exact(&mut data)?;
    cast_payload(&data)
}
fn wav_header() -> Vec<u8> {
    let mut h = Vec::with_capacity(44);
    h.extend(b"RIFF");
    h.extend(0xfffffff0u32.to_le_bytes());
    h.extend(b"WAVEfmt ");
    h.extend(16u32.to_le_bytes());
    h.extend(1u16.to_le_bytes());
    h.extend(2u16.to_le_bytes());
    h.extend(SAMPLE_RATE.to_le_bytes());
    h.extend((SAMPLE_RATE * 4).to_le_bytes());
    h.extend(4u16.to_le_bytes());
    h.extend(16u16.to_le_bytes());
    h.extend(b"data");
    h.extend(0xffffffccu32.to_le_bytes());
    h
}
fn serve_pcm(
    listener: TcpListener,
    address: Ipv4Addr,
    route: String,
    cancel: Arc<AtomicBool>,
    pcm: Arc<Mutex<VecDeque<u8>>>,
    receiver_connected: Arc<AtomicBool>,
) -> Res<()> {
    // Un seul flux ; aucune requête ne choisit un chemin disque ni une URL distante.
    while !cancel.load(Ordering::Relaxed) {
        let (mut stream, peer) = match listener.accept() {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(50));
                continue;
            }
            Err(e) => return Err(e.into()),
        };
        if peer.ip() != IpAddr::V4(address) {
            continue;
        }
        stream.set_read_timeout(Some(Duration::from_secs(2)))?;
        stream.set_write_timeout(Some(Duration::from_secs(2)))?;
        let mut request = Vec::new();
        let mut byte = [0u8];
        let deadline = Instant::now() + Duration::from_secs(3);
        while request.len() < 4096
            && !request.ends_with(b"\r\n\r\n")
            && Instant::now() < deadline
            && !cancel.load(Ordering::Relaxed)
        {
            if stream.read_exact(&mut byte).is_err() {
                break;
            }
            request.push(byte[0]);
        }
        let text = String::from_utf8_lossy(&request);
        let line = text.lines().next().unwrap_or("");
        let mut parts = line.split_ascii_whitespace();
        let method = parts.next().unwrap_or("");
        let path = parts.next().unwrap_or("");
        let head = method == "HEAD";
        if !(head || method == "GET") || path != route {
            let _ = stream.write_all(
                b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            );
            continue;
        }
        if stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: audio/wav\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n").is_err(){continue;}
        if head {
            continue;
        }
        if stream.write_all(&wav_header()).is_err() {
            continue;
        }
        receiver_connected.store(true, Ordering::Relaxed);
        let start = Instant::now();
        let mut sent = 0u64;
        while !cancel.load(Ordering::Relaxed) {
            let data = samples(&pcm, 3528);
            if stream.write_all(&data).is_err() {
                break;
            }
            sent += 3528;
            let due = Duration::from_secs_f64(sent as f64 / (SAMPLE_RATE * 4) as f64);
            std::thread::sleep(
                due.saturating_sub(start.elapsed())
                    .min(Duration::from_millis(20)),
            );
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_private_receivers() {
        assert!(local_address(Ipv4Addr::new(192, 168, 1, 2)));
        for ip in [
            Ipv4Addr::LOCALHOST,
            Ipv4Addr::UNSPECIFIED,
            Ipv4Addr::new(8, 8, 8, 8),
            Ipv4Addr::BROADCAST,
        ] {
            assert!(!local_address(ip));
        }
    }
    #[test]
    fn cast_bounds_and_roundtrip() {
        let mut frame = vec![];
        send_cast(
            &mut frame,
            "receiver-0",
            "heartbeat",
            json!({"type":"PING"}),
        )
        .unwrap();
        assert_eq!(read_cast(&mut frame.as_slice()).unwrap()["type"], "PING");
        assert!(read_cast(&mut [0xff; 4].as_slice()).is_err());
        for raw in [&[0x32, 127][..], &[0xff; 10][..], &[0x32, 3, 1][..]] {
            assert!(cast_payload(raw).is_err());
        }
    }
    #[test]
    fn valid_wav() {
        let h = wav_header();
        assert_eq!(h.len(), 44);
        assert_eq!(&h[..4], b"RIFF");
        assert_eq!(&h[36..40], b"data");
        assert_eq!(u32::from_le_bytes(h[24..28].try_into().unwrap()), 44100);
    }
}
