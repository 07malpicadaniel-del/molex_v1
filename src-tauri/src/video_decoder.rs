use std::io::Read;
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Once};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use ffmpeg_next::codec::{self, Id};
use ffmpeg_next::ffi;
use ffmpeg_next::format::Pixel;
use ffmpeg_next::frame::Video as VideoFrame;
use ffmpeg_next::software::scaling::{Context as ScalerContext, Flags as ScalerFlags};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tokio::sync::watch;
use tokio_tungstenite::accept_hdr_async;
use tokio_tungstenite::tungstenite::handshake::server::{Request, Response};
use tokio_tungstenite::tungstenite::Message;

static FFMPEG_INIT: Once = Once::new();

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StreamStats {
    pub active: bool,
    pub connected: bool,
    pub host: String,
    pub port: u16,
    pub width: u32,
    pub height: u32,
    pub fps: f32,
    pub decode_ms: f32,
    pub kbps: u32,
    pub frame_seq: u64,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoEndpointInfo {
    pub ws_url: String,
    pub token: String,
    pub stats: StreamStats,
}

struct ParserGuard {
    ptr: *mut ffi::AVCodecParserContext,
}

impl ParserGuard {
    fn new_h264() -> Result<Self, String> {
        let ptr = unsafe { ffi::av_parser_init(ffi::AVCodecID::AV_CODEC_ID_H264) };
        if ptr.is_null() {
            return Err("No se pudo inicializar AVCodecParserContext para H.264".into());
        }
        Ok(Self { ptr })
    }
}

impl Drop for ParserGuard {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe { ffi::av_parser_close(self.ptr) };
            self.ptr = std::ptr::null_mut();
        }
    }
}

struct ActiveStreamHandle {
    running: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

pub struct VideoPipeline {
    ws_port: u16,
    token: String,
    latest_frame: Arc<Mutex<Option<Arc<Vec<u8>>>>>,
    frame_seq_tx: watch::Sender<u64>,
    stats: Arc<Mutex<StreamStats>>,
    active_handle: Mutex<Option<ActiveStreamHandle>>,
}

impl VideoPipeline {
    pub fn start_server() -> Arc<Self> {
        FFMPEG_INIT.call_once(|| {
            let _ = ffmpeg_next::init();
        });

        let token = uuid::Uuid::new_v4().to_string();
        let latest_frame: Arc<Mutex<Option<Arc<Vec<u8>>>>> = Arc::new(Mutex::new(None));
        let (frame_seq_tx, _) = watch::channel(0u64);
        let stats = Arc::new(Mutex::new(StreamStats::default()));

        let listener = std::net::TcpListener::bind("127.0.0.1:0")
            .expect("No se pudo enlazar el servidor WebSocket local de video");
        listener
            .set_nonblocking(true)
            .expect("No se pudo configurar nonblocking en listener WebSocket");
        let ws_port = listener.local_addr().unwrap().port();

        let pipeline = Arc::new(Self {
            ws_port,
            token: token.clone(),
            latest_frame: latest_frame.clone(),
            frame_seq_tx: frame_seq_tx.clone(),
            stats: stats.clone(),
            active_handle: Mutex::new(None),
        });

        tauri::async_runtime::spawn(async move {
            let tokio_listener = match TcpListener::from_std(listener) {
                Ok(l) => l,
                Err(e) => {
                    eprintln!("❌ Error convirtiendo listener WebSocket: {}", e);
                    return;
                }
            };

            loop {
                let Ok((stream, _peer)) = tokio_listener.accept().await else {
                    continue;
                };
                let _ = stream.set_nodelay(true);

                let expected_token = token.clone();
                let frames_ref = latest_frame.clone();
                let mut seq_rx = frame_seq_tx.subscribe();

                tokio::spawn(async move {
                    let ws_stream = match accept_hdr_async(
                        stream,
                        |req: &Request, resp: Response| {
                            let authorized = req
                                .uri()
                                .query()
                                .map(|q| {
                                    q.split('&')
                                        .any(|kv| kv == format!("token={}", expected_token))
                                })
                                .unwrap_or(false);
                            if authorized {
                                Ok(resp)
                            } else {
                                Err(Response::builder()
                                    .status(401)
                                    .body(Some("Unauthorized".to_string()))
                                    .unwrap())
                            }
                        },
                    )
                    .await
                    {
                        Ok(ws) => ws,
                        Err(_) => return,
                    };

                    let (mut ws_tx, _ws_rx) = ws_stream.split();

                    // Enviar último frame disponible inmediatamente al conectar
                    let initial_frame = frames_ref.lock().ok().and_then(|g| g.clone());
                    if let Some(frame) = initial_frame {
                        if ws_tx
                            .send(Message::Binary(frame.as_slice().to_vec().into()))
                            .await
                            .is_err()
                        {
                            return;
                        }
                    }

                    while seq_rx.changed().await.is_ok() {
                        let frame_opt = frames_ref.lock().ok().and_then(|g| g.clone());
                        if let Some(frame) = frame_opt {
                            if ws_tx
                                .send(Message::Binary(frame.as_slice().to_vec().into()))
                                .await
                                .is_err()
                            {
                                break;
                            }
                        }
                    }
                });
            }
        });

        pipeline
    }

    pub fn endpoint_info(&self) -> VideoEndpointInfo {
        let stats = self.stats.lock().map(|s| s.clone()).unwrap_or_default();
        VideoEndpointInfo {
            ws_url: format!("ws://127.0.0.1:{}/?token={}", self.ws_port, self.token),
            token: self.token.clone(),
            stats,
        }
    }

    pub fn get_stats(&self) -> StreamStats {
        self.stats.lock().map(|s| s.clone()).unwrap_or_default()
    }

    pub fn get_latest_frame_bytes(&self) -> Option<Vec<u8>> {
        self.latest_frame
            .lock()
            .ok()
            .and_then(|g| g.as_ref().map(|arc| arc.as_ref().clone()))
    }

    pub fn stop_stream(&self) {
        if let Ok(mut guard) = self.active_handle.lock() {
            if let Some(mut handle) = guard.take() {
                handle.running.store(false, Ordering::SeqCst);
                if let Some(join) = handle.join.take() {
                    let _ = join.join();
                }
            }
        }
        if let Ok(mut stats) = self.stats.lock() {
            stats.active = false;
            stats.connected = false;
        }
    }

    pub fn start_stream(&self, host: String, port: u16, decoder_threads: u32) {
        self.stop_stream();

        let running = Arc::new(AtomicBool::new(true));
        let running_thread = running.clone();
        let latest_frame = self.latest_frame.clone();
        let frame_seq_tx = self.frame_seq_tx.clone();
        let stats_ref = self.stats.clone();

        if let Ok(mut s) = stats_ref.lock() {
            *s = StreamStats {
                active: true,
                connected: false,
                host: host.clone(),
                port,
                ..Default::default()
            };
        }

        let join = std::thread::Builder::new()
            .name("molex-h264-decoder".into())
            .spawn(move || {
                run_stream_loop(
                    host,
                    port,
                    decoder_threads,
                    running_thread,
                    latest_frame,
                    frame_seq_tx,
                    stats_ref,
                );
            })
            .ok();

        if let Ok(mut guard) = self.active_handle.lock() {
            *guard = Some(ActiveStreamHandle { running, join });
        }
    }
}

fn connect_tcp_with_timeout(host: &str, port: u16, timeout: Duration) -> Result<TcpStream, String> {
    let target = format!("{}:{}", host.trim(), port);
    let addrs: Vec<SocketAddr> = target
        .to_socket_addrs()
        .map_err(|e| format!("No se pudo resolver {}: {}", target, e))?
        .collect();
    if addrs.is_empty() {
        return Err(format!("Sin direcciones IP para {}", target));
    }

    let mut last_err = String::new();
    for addr in addrs {
        match TcpStream::connect_timeout(&addr, timeout) {
            Ok(stream) => {
                let _ = stream.set_nodelay(true);
                let _ = stream.set_read_timeout(Some(Duration::from_millis(250)));
                return Ok(stream);
            }
            Err(e) => last_err = e.to_string(),
        }
    }
    Err(format!("No se pudo conectar a {}: {}", target, last_err))
}

fn pack_yuv420p_packet(
    frame: &VideoFrame,
    seq: u64,
    decode_us: u32,
    pts_ms: u32,
) -> Vec<u8> {
    let width = frame.width() as usize;
    let height = frame.height() as usize;
    let chroma_w = width.div_ceil(2);
    let chroma_h = height.div_ceil(2);

    let y_len = width * height;
    let u_len = chroma_w * chroma_h;
    let v_len = chroma_w * chroma_h;
    let header_len = 24;

    let mut out = Vec::with_capacity(header_len + y_len + u_len + v_len);
    out.extend_from_slice(b"MOLX");
    out.extend_from_slice(&(width as u32).to_le_bytes());
    out.extend_from_slice(&(height as u32).to_le_bytes());
    out.extend_from_slice(&(seq as u32).to_le_bytes());
    out.extend_from_slice(&decode_us.to_le_bytes());
    out.extend_from_slice(&pts_ms.to_le_bytes());

    let y_stride = frame.stride(0);
    let y_data = frame.data(0);
    for row in 0..height {
        let start = row * y_stride;
        out.extend_from_slice(&y_data[start..start + width]);
    }

    let u_stride = frame.stride(1);
    let u_data = frame.data(1);
    for row in 0..chroma_h {
        let start = row * u_stride;
        out.extend_from_slice(&u_data[start..start + chroma_w]);
    }

    let v_stride = frame.stride(2);
    let v_data = frame.data(2);
    for row in 0..chroma_h {
        let start = row * v_stride;
        out.extend_from_slice(&v_data[start..start + chroma_w]);
    }

    out
}

#[derive(Debug)]
enum StreamSessionError {
    ImmediateZeroBytesEof,
    Other { message: String, received_bytes: usize },
}

fn run_stream_loop(
    host: String,
    port: u16,
    decoder_threads: u32,
    running: Arc<AtomicBool>,
    latest_frame: Arc<Mutex<Option<Arc<Vec<u8>>>>>,
    frame_seq_tx: watch::Sender<u64>,
    stats_ref: Arc<Mutex<StreamStats>>,
) {
    run_stream_loop_with_delay(
        host,
        port,
        decoder_threads,
        running,
        latest_frame,
        frame_seq_tx,
        stats_ref,
        Duration::from_millis(1500),
    );
}

fn run_stream_loop_with_delay(
    host: String,
    port: u16,
    decoder_threads: u32,
    running: Arc<AtomicBool>,
    latest_frame: Arc<Mutex<Option<Arc<Vec<u8>>>>>,
    frame_seq_tx: watch::Sender<u64>,
    stats_ref: Arc<Mutex<StreamStats>>,
    retry_delay: Duration,
) {
    const MAX_ZERO_BYTE_FAILURES: u32 = 3;
    let mut zero_byte_failures: u32 = 0;

    while running.load(Ordering::Relaxed) {
        match connect_tcp_with_timeout(&host, port, Duration::from_secs(4)) {
            Ok(mut tcp) => {
                if let Ok(mut s) = stats_ref.lock() {
                    s.connected = true;
                    s.error = None;
                }

                match decode_tcp_h264_session(
                    &mut tcp,
                    decoder_threads,
                    &running,
                    &latest_frame,
                    &frame_seq_tx,
                    &stats_ref,
                ) {
                    Ok(()) => {
                        zero_byte_failures = 0;
                    }
                    Err(StreamSessionError::ImmediateZeroBytesEof) => {
                        zero_byte_failures += 1;
                        if zero_byte_failures >= MAX_ZERO_BYTE_FAILURES {
                            if let Ok(mut s) = stats_ref.lock() {
                                s.active = false;
                                s.connected = false;
                                s.error = Some(
                                    "El servidor cerró el stream con 0 bytes (posible fallo de codificador VA-API/NVENC en el host)"
                                        .to_string(),
                                );
                            }
                            break;
                        }
                        if running.load(Ordering::Relaxed) {
                            if let Ok(mut s) = stats_ref.lock() {
                                s.connected = false;
                                s.error = Some(format!(
                                    "El servidor cerró el stream con 0 bytes (intento {}/{})",
                                    zero_byte_failures, MAX_ZERO_BYTE_FAILURES
                                ));
                            }
                        }
                    }
                    Err(StreamSessionError::Other {
                        message,
                        received_bytes,
                    }) => {
                        if received_bytes > 0 {
                            zero_byte_failures = 0;
                        }
                        if running.load(Ordering::Relaxed) {
                            if let Ok(mut s) = stats_ref.lock() {
                                s.connected = false;
                                s.error = Some(message);
                            }
                        }
                    }
                }
            }
            Err(err) => {
                if let Ok(mut s) = stats_ref.lock() {
                    s.connected = false;
                    s.error = Some(err);
                }
            }
        }

        // Reintento controlado si la sesión sigue activa
        let step = Duration::from_millis(50);
        let mut waited = Duration::ZERO;
        while waited < retry_delay {
            if !running.load(Ordering::Relaxed) {
                return;
            }
            let chunk = (retry_delay - waited).min(step);
            std::thread::sleep(chunk);
            waited += chunk;
        }
    }
}

fn decode_tcp_h264_session(
    tcp: &mut TcpStream,
    decoder_threads: u32,
    running: &AtomicBool,
    latest_frame: &Arc<Mutex<Option<Arc<Vec<u8>>>>>,
    frame_seq_tx: &watch::Sender<u64>,
    stats_ref: &Arc<Mutex<StreamStats>>,
) -> Result<(), StreamSessionError> {
    let codec = codec::decoder::find(Id::H264).ok_or_else(|| StreamSessionError::Other {
        message: "No se encontró el decodificador H.264 en libavcodec".to_string(),
        received_bytes: 0,
    })?;

    let mut ctx = codec::context::Context::new_with_codec(codec);
    unsafe {
        let raw_ctx = ctx.as_mut_ptr();
        (*raw_ctx).flags |= ffi::AV_CODEC_FLAG_LOW_DELAY as i32;
        (*raw_ctx).thread_count = decoder_threads.clamp(1, 8) as i32;
        (*raw_ctx).thread_type = ffi::FF_THREAD_SLICE as i32;
    }

    let mut decoder = ctx.decoder().video().map_err(|e| StreamSessionError::Other {
        message: format!("Error abriendo decodificador H.264: {}", e),
        received_bytes: 0,
    })?;

    let parser = ParserGuard::new_h264().map_err(|message| StreamSessionError::Other {
        message,
        received_bytes: 0,
    })?;
    let mut decoded_frame = VideoFrame::empty();
    let mut yuv_frame = VideoFrame::empty();
    let mut scaler: Option<(ScalerContext, u32, u32, Pixel)> = None;

    let mut read_buf = vec![0u8; 256 * 1024];
    let mut seq = 0u64;
    let stream_start = Instant::now();
    let mut fps_window_start = Instant::now();
    let mut fps_frames = 0u32;
    let mut bytes_window = 0usize;
    let mut total_bytes_read = 0usize;

    while running.load(Ordering::Relaxed) {
        let n = match tcp.read(&mut read_buf) {
            Ok(0) => {
                if total_bytes_read == 0 {
                    return Err(StreamSessionError::ImmediateZeroBytesEof);
                }
                return Err(StreamSessionError::Other {
                    message: "El servidor cerró el stream de video".into(),
                    received_bytes: total_bytes_read,
                });
            }
            Ok(n) => n,
            Err(ref e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                continue;
            }
            Err(e) => {
                return Err(StreamSessionError::Other {
                    message: format!("Error leyendo stream H.264: {}", e),
                    received_bytes: total_bytes_read,
                });
            }
        };

        total_bytes_read += n;
        bytes_window += n;
        let mut offset = 0usize;

        while offset < n && running.load(Ordering::Relaxed) {
            let mut out_data: *mut u8 = std::ptr::null_mut();
            let mut out_size: i32 = 0;

            let consumed = unsafe {
                ffi::av_parser_parse2(
                    parser.ptr,
                    decoder.as_mut_ptr(),
                    &mut out_data,
                    &mut out_size,
                    read_buf[offset..n].as_ptr(),
                    (n - offset) as i32,
                    ffi::AV_NOPTS_VALUE,
                    ffi::AV_NOPTS_VALUE,
                    0,
                )
            };

            if consumed < 0 {
                return Err(StreamSessionError::Other {
                    message: "Error interno en av_parser_parse2".into(),
                    received_bytes: total_bytes_read,
                });
            }
            offset += consumed as usize;

            if out_size > 0 && !out_data.is_null() {
                let pkt_bytes =
                    unsafe { std::slice::from_raw_parts(out_data, out_size as usize) };
                let pkt = ffmpeg_next::Packet::copy(pkt_bytes);
                let decode_started = Instant::now();

                if decoder.send_packet(&pkt).is_ok() {
                    while decoder.receive_frame(&mut decoded_frame).is_ok() {
                        let w = decoded_frame.width();
                        let h = decoded_frame.height();
                        let fmt = decoded_frame.format();
                        if w == 0 || h == 0 {
                            continue;
                        }

                        let frame_to_pack: &VideoFrame = if fmt == Pixel::YUV420P {
                            &decoded_frame
                        } else {
                            let needs_new_scaler = match &scaler {
                                Some((_, sw, sh, sfmt)) => *sw != w || *sh != h || *sfmt != fmt,
                                None => true,
                            };
                            if needs_new_scaler {
                                let sc = ScalerContext::get(
                                    fmt,
                                    w,
                                    h,
                                    Pixel::YUV420P,
                                    w,
                                    h,
                                    ScalerFlags::FAST_BILINEAR,
                                )
                                .map_err(|e| StreamSessionError::Other {
                                    message: format!("Error creando sws_scale: {}", e),
                                    received_bytes: total_bytes_read,
                                })?;
                                scaler = Some((sc, w, h, fmt));
                            }
                            if let Some((ref mut sc, _, _, _)) = scaler {
                                sc.run(&decoded_frame, &mut yuv_frame).map_err(|e| {
                                    StreamSessionError::Other {
                                        message: format!("Error convirtiendo pixel format: {}", e),
                                        received_bytes: total_bytes_read,
                                    }
                                })?;
                            }
                            &yuv_frame
                        };

                        seq += 1;
                        fps_frames += 1;
                        let decode_us = decode_started.elapsed().as_micros() as u32;
                        let pts_ms = stream_start.elapsed().as_millis() as u32;

                        let packed = Arc::new(pack_yuv420p_packet(
                            frame_to_pack,
                            seq,
                            decode_us,
                            pts_ms,
                        ));

                        if let Ok(mut guard) = latest_frame.lock() {
                            *guard = Some(packed);
                        }
                        let _ = frame_seq_tx.send(seq);

                        let elapsed_win = fps_window_start.elapsed().as_secs_f32();
                        if elapsed_win >= 0.5 {
                            let fps = (fps_frames as f32) / elapsed_win;
                            let kbps = ((bytes_window as f32 * 8.0) / elapsed_win / 1000.0) as u32;
                            if let Ok(mut s) = stats_ref.lock() {
                                s.connected = true;
                                s.width = w;
                                s.height = h;
                                s.fps = (fps * 10.0).round() / 10.0;
                                s.decode_ms = (decode_us as f32 / 100.0).round() / 10.0;
                                s.kbps = kbps;
                                s.frame_seq = seq;
                                s.error = None;
                            }
                            fps_window_start = Instant::now();
                            fps_frames = 0;
                            bytes_window = 0;
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packs_yuv420p_packet_with_molx_header() {
        let mut frame = VideoFrame::new(Pixel::YUV420P, 4, 2);
        frame.data_mut(0)[..4].copy_from_slice(&[10, 20, 30, 40]);
        let y_stride = frame.stride(0);
        frame.data_mut(0)[y_stride..y_stride + 4].copy_from_slice(&[50, 60, 70, 80]);
        frame.data_mut(1)[..2].copy_from_slice(&[128, 129]);
        frame.data_mut(2)[..2].copy_from_slice(&[200, 201]);

        let packet = pack_yuv420p_packet(&frame, 7, 450, 1234);
        assert_eq!(&packet[0..4], b"MOLX");
        assert_eq!(u32::from_le_bytes(packet[4..8].try_into().unwrap()), 4);
        assert_eq!(u32::from_le_bytes(packet[8..12].try_into().unwrap()), 2);
        assert_eq!(u32::from_le_bytes(packet[12..16].try_into().unwrap()), 7);
        assert_eq!(u32::from_le_bytes(packet[16..20].try_into().unwrap()), 450);
        assert_eq!(u32::from_le_bytes(packet[20..24].try_into().unwrap()), 1234);
        assert_eq!(packet.len(), 24 + 8 + 2 + 2);
        assert_eq!(&packet[24..32], &[10, 20, 30, 40, 50, 60, 70, 80]);
        assert_eq!(&packet[32..34], &[128, 129]);
        assert_eq!(&packet[34..36], &[200, 201]);
    }

    #[test]
    fn decodes_real_h264_annex_b_stream_over_tcp() {
        use std::io::Write;
        FFMPEG_INIT.call_once(|| {
            let _ = ffmpeg_next::init();
        });

        // SPS + PPS + IDR + P-frame Annex-B (64x64 YUV420p)
        const H264_SAMPLE: &[u8] = &[
            0x00, 0x00, 0x00, 0x01, 0x67, 0x42, 0xc0, 0x0a, 0xd9, 0x04, 0x26, 0xc0, 0x44, 0x00,
            0x00, 0x03, 0x00, 0x04, 0x00, 0x00, 0x03, 0x00, 0xc8, 0x3c, 0x48, 0x99, 0x20, 0x00,
            0x00, 0x00, 0x01, 0x68, 0xcb, 0x83, 0xcb, 0x20, 0x00, 0x00, 0x01, 0x65, 0x88, 0x84,
            0x0b, 0xf1, 0x18, 0xa0, 0x00, 0x24, 0x73, 0x1c, 0x00, 0x04, 0x43, 0x63, 0x80, 0x00,
            0x9a, 0xd4, 0x9c, 0x9c, 0x9d, 0x75, 0xd7, 0x5d, 0x75, 0xd7, 0x5d, 0x75, 0xd7, 0x5e,
            0x00, 0x00, 0x00, 0x01, 0x41, 0x9a, 0x38, 0x15, 0xe1, 0x18, 0x00, 0x00, 0x00, 0x01,
        ];

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server_thread = std::thread::spawn(move || {
            let (mut conn, _) = listener.accept().unwrap();
            let _ = conn.write_all(H264_SAMPLE);
            let _ = conn.flush();
            std::thread::sleep(Duration::from_millis(120));
        });

        let mut tcp = connect_tcp_with_timeout("127.0.0.1", port, Duration::from_secs(2)).unwrap();
        let running = AtomicBool::new(true);
        let latest_frame = Arc::new(Mutex::new(None));
        let (seq_tx, _seq_rx) = watch::channel(0u64);
        let stats = Arc::new(Mutex::new(StreamStats::default()));

        let _ = decode_tcp_h264_session(&mut tcp, 2, &running, &latest_frame, &seq_tx, &stats);
        let _ = server_thread.join();

        let packet = latest_frame
            .lock()
            .unwrap()
            .clone()
            .expect("Debe haber decodificado al menos un frame H.264 64x64");
        assert_eq!(&packet[0..4], b"MOLX");
        assert_eq!(u32::from_le_bytes(packet[4..8].try_into().unwrap()), 64);
        assert_eq!(u32::from_le_bytes(packet[8..12].try_into().unwrap()), 64);
        assert_eq!(packet.len(), 24 + 64 * 64 + 32 * 32 + 32 * 32);
    }

    #[test]
    fn stops_retrying_after_three_immediate_zero_byte_eofs() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            for _ in 0..3 {
                if let Ok((conn, _)) = listener.accept() {
                    drop(conn);
                }
            }
        });

        let running = Arc::new(AtomicBool::new(true));
        let latest_frame = Arc::new(Mutex::new(None));
        let (seq_tx, _seq_rx) = watch::channel(0u64);
        let stats = Arc::new(Mutex::new(StreamStats {
            active: true,
            ..Default::default()
        }));

        run_stream_loop_with_delay(
            "127.0.0.1".to_string(),
            port,
            2,
            running.clone(),
            latest_frame,
            seq_tx,
            stats.clone(),
            Duration::from_millis(20),
        );
        let _ = server.join();

        let final_stats = stats.lock().unwrap().clone();
        assert!(!final_stats.active);
        assert!(!final_stats.connected);
        assert_eq!(
            final_stats.error.as_deref(),
            Some("El servidor cerró el stream con 0 bytes (posible fallo de codificador VA-API/NVENC en el host)")
        );
    }
}
