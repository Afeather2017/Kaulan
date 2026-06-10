//! FFmpeg/FFprobe-backed helpers for audio conversion and metadata extraction.
//!
//! Documentation: [docs/ffmpeg-audio-pipeline.md](../../docs/ffmpeg-audio-pipeline.md)

use crate::file_ops::{get_file_reader, resolve_path, PathKind};
use futures::StreamExt;
use rusty_ffmpeg::ffi;
use serde::Deserialize;
use std::ffi::{CStr, CString};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::ptr;
use tokio::io::AsyncWriteExt;

const STREAM_CHUNK_SIZE: usize = 64 * 1024;

#[repr(C)]
struct FormatContextView {
    av_class: *const ffi::AVClass,
    iformat: *const ffi::AVInputFormat,
    oformat: *const ffi::AVOutputFormat,
    priv_data: *mut std::ffi::c_void,
    pb: *mut ffi::AVIOContext,
    ctx_flags: i32,
    nb_streams: u32,
    streams: *mut *mut ffi::AVStream,
}

pub struct PreparedInput {
    path: PathBuf,
    _temp_dir: Option<tempfile::TempDir>,
}

impl PreparedInput {
    pub fn path(&self) -> &Path {
        &self.path
    }
}

pub async fn prepare_input(file_path: &str) -> Result<PreparedInput, String> {
    let resolved = resolve_path(file_path).map_err(|e| {
        format!(
            "failed to resolve source path for ffmpeg input {}: {}",
            file_path, e
        )
    })?;

    if resolved.path_kind == PathKind::StdFs {
        return Ok(PreparedInput {
            path: PathBuf::from(resolved.normalized_path),
            _temp_dir: None,
        });
    }

    let extension = Path::new(file_path)
        .extension()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("bin");

    let temp_dir = tempfile::tempdir().map_err(|e| format!("failed to create temp dir: {e}"))?;
    let temp_path = temp_dir.path().join(format!("input.{extension}"));
    let mut output = tokio::fs::File::create(&temp_path)
        .await
        .map_err(|e| format!("failed to create temp input {}: {e}", temp_path.display()))?;
    let mut stream = get_file_reader()
        .read_stream(file_path, STREAM_CHUNK_SIZE)
        .await
        .map_err(|e| format!("failed to open source stream {}: {e}", file_path))?;

    while let Some(chunk) = stream.next().await {
        let bytes =
            chunk.map_err(|e| format!("failed to read source stream {}: {e}", file_path))?;
        output
            .write_all(&bytes)
            .await
            .map_err(|e| format!("failed to write temp input {}: {e}", temp_path.display()))?;
    }
    output
        .flush()
        .await
        .map_err(|e| format!("failed to flush temp input {}: {e}", temp_path.display()))?;

    Ok(PreparedInput {
        path: temp_path,
        _temp_dir: Some(temp_dir),
    })
}

pub async fn calculate_lufs_for_source(file_path: &str) -> Result<Option<f64>, String> {
    let input = prepare_input(file_path).await?;
    calculate_lufs(input.path())
}

pub fn transcode_audio_to_mp3(input: &Path, output: &Path) -> Result<(), String> {
    let input = path_to_cstring(input)?;
    let output = path_to_cstring(output)?;

    unsafe {
        let mut input_ctx = InputContext::open(&input)?;
        let (stream_index, decoder) = input_ctx.best_audio_stream()?;
        let input_stream = (*(*input_ctx.view()).streams.add(stream_index as usize))
            .as_ref()
            .unwrap();
        let mut decoder_ctx = CodecContext::decoder(decoder, input_stream.codecpar)?;
        normalize_channel_layout(decoder_ctx.as_mut_ptr(), 2)?;

        let mut output_ctx = OutputContext::create(&output)?;
        let encoder = find_mp3_encoder()?;
        let mut encoder_ctx = CodecContext::encoder(encoder)?;
        configure_mp3_encoder(encoder, decoder_ctx.as_ptr(), encoder_ctx.as_mut_ptr())?;

        let out_stream = output_ctx.new_stream(encoder)?;
        if (*output_ctx.view().oformat).flags & ffi::AVFMT_GLOBALHEADER as i32 != 0 {
            (*encoder_ctx.as_mut_ptr()).flags |= ffi::AV_CODEC_FLAG_GLOBAL_HEADER as i32;
        }

        ffmpeg_call(
            ffi::avcodec_open2(encoder_ctx.as_mut_ptr(), encoder, ptr::null_mut()),
            "failed to open MP3 encoder",
        )?;

        (*out_stream).time_base = (*encoder_ctx.as_ptr()).time_base;
        ffmpeg_call(
            ffi::avcodec_parameters_from_context((*out_stream).codecpar, encoder_ctx.as_ptr()),
            "failed to export encoder parameters",
        )?;

        output_ctx.open_io(&output)?;
        ffmpeg_call(
            ffi::avformat_write_header(output_ctx.as_mut_ptr(), ptr::null_mut()),
            "failed to write output header",
        )?;

        let mut swr = Resampler::new(decoder_ctx.as_ptr(), encoder_ctx.as_ptr())?;
        let mut packet = Packet::new()?;
        let mut decoded = Frame::new()?;
        let mut converted = Frame::new()?;
        let mut next_pts = 0_i64;

        while ffi::av_read_frame(input_ctx.as_mut_ptr(), packet.as_mut_ptr()) >= 0 {
            if (*packet.as_ptr()).stream_index == stream_index {
                decode_and_encode(
                    decoder_ctx.as_mut_ptr(),
                    encoder_ctx.as_mut_ptr(),
                    swr.as_mut_ptr(),
                    decoded.as_mut_ptr(),
                    converted.as_mut_ptr(),
                    packet.as_mut_ptr(),
                    output_ctx.as_mut_ptr(),
                    out_stream,
                    input_stream,
                    &mut next_pts,
                )?;
            }
            ffi::av_packet_unref(packet.as_mut_ptr());
        }

        ffmpeg_call(
            ffi::avcodec_send_packet(decoder_ctx.as_mut_ptr(), ptr::null()),
            "failed to flush decoder",
        )?;
        receive_decoded_frames(
            decoder_ctx.as_mut_ptr(),
            encoder_ctx.as_mut_ptr(),
            swr.as_mut_ptr(),
            decoded.as_mut_ptr(),
            converted.as_mut_ptr(),
            output_ctx.as_mut_ptr(),
            out_stream,
            input_stream,
            &mut next_pts,
        )?;
        flush_encoder(
            encoder_ctx.as_mut_ptr(),
            output_ctx.as_mut_ptr(),
            out_stream,
            packet.as_mut_ptr(),
        )?;

        ffmpeg_call(
            ffi::av_write_trailer(output_ctx.as_mut_ptr()),
            "failed to finalize output file",
        )?;
    }

    Ok(())
}

pub fn extract_cover_art(input: &Path) -> Result<Option<(String, Vec<u8>)>, String> {
    if !has_embedded_cover(input)? {
        return Ok(None);
    }

    let temp_dir = tempfile::tempdir().map_err(|e| format!("failed to create temp dir: {e}"))?;
    let output = temp_dir.path().join("cover.png");
    let result = Command::new("ffmpeg")
        .args(["-v", "error", "-nostdin", "-y", "-i"])
        .arg(input)
        .args(["-an", "-map", "0:v:0", "-frames:v", "1"])
        .arg(&output)
        .output()
        .map_err(|e| format!("failed to run ffmpeg for cover extraction: {e}"))?;

    if !result.status.success() {
        return Err(format!(
            "ffmpeg cover extraction failed: {}",
            stderr_message(&result.stderr)
        ));
    }

    let bytes =
        std::fs::read(&output).map_err(|e| format!("failed to read extracted cover image: {e}"))?;
    if bytes.is_empty() {
        return Ok(None);
    }

    Ok(Some(("image/png".to_string(), bytes)))
}

pub fn calculate_lufs(input: &Path) -> Result<Option<f64>, String> {
    let result = Command::new("ffmpeg")
        .args(["-hide_banner", "-nostdin", "-i"])
        .arg(input)
        .args(["-filter:a", "ebur128=framelog=verbose", "-f", "null", "-"])
        .output()
        .map_err(|e| format!("failed to run ffmpeg for LUFS calculation: {e}"))?;

    let stderr = String::from_utf8_lossy(&result.stderr);
    let parsed = parse_integrated_lufs(stderr.as_ref());

    if result.status.success() {
        return Ok(parsed);
    }

    if parsed.is_some() {
        return Ok(parsed);
    }

    let message = stderr_message(&result.stderr);
    if message.contains("Invalid data found")
        || message.contains("Output file #0 does not contain any stream")
    {
        return Ok(None);
    }

    Err(format!("ffmpeg LUFS calculation failed: {message}"))
}

fn stderr_message(stderr: &[u8]) -> String {
    let text = String::from_utf8_lossy(stderr).trim().to_string();
    if text.is_empty() {
        "unknown ffmpeg error".to_string()
    } else {
        text
    }
}

fn path_to_cstring(path: &Path) -> Result<CString, String> {
    CString::new(
        path.to_str()
            .ok_or_else(|| format!("path is not valid UTF-8: {}", path.display()))?,
    )
    .map_err(|e| format!("path contains interior NUL byte: {e}"))
}

fn ffmpeg_call(code: i32, context: &str) -> Result<(), String> {
    if code < 0 {
        Err(format!("{context}: {}", ffmpeg_error_string(code)))
    } else {
        Ok(())
    }
}

fn ffmpeg_error_string(code: i32) -> String {
    let mut buffer = [0i8; ffi::AV_ERROR_MAX_STRING_SIZE as usize];
    unsafe {
        ffi::av_strerror(code, buffer.as_mut_ptr(), buffer.len());
        CStr::from_ptr(buffer.as_ptr())
            .to_string_lossy()
            .into_owned()
    }
}

unsafe fn find_mp3_encoder() -> Result<*const ffi::AVCodec, String> {
    let codec_name = CString::new("libmp3lame").unwrap();
    let encoder = ffi::avcodec_find_encoder_by_name(codec_name.as_ptr());
    if !encoder.is_null() {
        return Ok(encoder);
    }

    let fallback = ffi::avcodec_find_encoder(ffi::AV_CODEC_ID_MP3);
    if fallback.is_null() {
        Err("failed to find an MP3 encoder in the linked FFmpeg libraries".to_string())
    } else {
        Ok(fallback)
    }
}

unsafe fn normalize_channel_layout(
    codec_ctx: *mut ffi::AVCodecContext,
    fallback_channels: i32,
) -> Result<(), String> {
    if (*codec_ctx).ch_layout.nb_channels > 0
        && ffi::av_channel_layout_check(&(*codec_ctx).ch_layout) == 1
    {
        return Ok(());
    }

    ffi::av_channel_layout_uninit(&mut (*codec_ctx).ch_layout);
    ffi::av_channel_layout_default(&mut (*codec_ctx).ch_layout, fallback_channels);
    if ffi::av_channel_layout_check(&(*codec_ctx).ch_layout) == 1 {
        Ok(())
    } else {
        Err("failed to create default channel layout".to_string())
    }
}

unsafe fn configure_mp3_encoder(
    encoder: *const ffi::AVCodec,
    decoder_ctx: *const ffi::AVCodecContext,
    encoder_ctx: *mut ffi::AVCodecContext,
) -> Result<(), String> {
    let sample_rate = select_sample_rate(encoder, (*decoder_ctx).sample_rate);
    let sample_fmt = select_sample_fmt(encoder)?;
    let channel_layout = select_channel_layout(encoder, &(*decoder_ctx).ch_layout)?;

    (*encoder_ctx).sample_rate = sample_rate;
    (*encoder_ctx).sample_fmt = sample_fmt;
    ffmpeg_call(
        ffi::av_channel_layout_copy(&mut (*encoder_ctx).ch_layout, &channel_layout),
        "failed to copy encoder channel layout",
    )?;
    (*encoder_ctx).bit_rate = 320_000;
    (*encoder_ctx).time_base = ffi::AVRational {
        num: 1,
        den: sample_rate,
    };

    Ok(())
}

unsafe fn select_sample_rate(encoder: *const ffi::AVCodec, preferred: i32) -> i32 {
    let rates = (*encoder).supported_samplerates;
    if rates.is_null() {
        return if preferred > 0 { preferred } else { 44_100 };
    }

    let mut index = 0;
    let mut fallback = 44_100;
    while *rates.add(index) != 0 {
        let value = *rates.add(index);
        if index == 0 {
            fallback = value;
        }
        if value == preferred {
            return value;
        }
        index += 1;
    }
    fallback
}

unsafe fn select_sample_fmt(encoder: *const ffi::AVCodec) -> Result<ffi::AVSampleFormat, String> {
    let formats = (*encoder).sample_fmts;
    if formats.is_null() {
        return Err("mp3 encoder did not expose supported sample formats".to_string());
    }

    let mut index = 0;
    while *formats.add(index) != ffi::AV_SAMPLE_FMT_NONE {
        let value = *formats.add(index);
        if value == ffi::AV_SAMPLE_FMT_FLTP {
            return Ok(value);
        }
        if index == 0 {
            return Ok(value);
        }
        index += 1;
    }

    Err("mp3 encoder did not expose any usable sample format".to_string())
}

unsafe fn select_channel_layout(
    encoder: *const ffi::AVCodec,
    input_layout: *const ffi::AVChannelLayout,
) -> Result<ffi::AVChannelLayout, String> {
    let mut selected = std::mem::zeroed::<ffi::AVChannelLayout>();
    let supported = (*encoder).ch_layouts;

    if !supported.is_null() {
        let input_channels = (*input_layout).nb_channels;
        let mut index = 0;
        let first = supported.add(0);
        while (*supported.add(index)).nb_channels != 0 {
            let candidate = supported.add(index);
            if (*candidate).nb_channels == input_channels {
                ffmpeg_call(
                    ffi::av_channel_layout_copy(&mut selected, candidate),
                    "failed to copy matching encoder channel layout",
                )?;
                return Ok(selected);
            }
            index += 1;
        }
        ffmpeg_call(
            ffi::av_channel_layout_copy(&mut selected, first),
            "failed to copy fallback encoder channel layout",
        )?;
        return Ok(selected);
    }

    ffmpeg_call(
        ffi::av_channel_layout_copy(&mut selected, input_layout),
        "failed to copy input channel layout",
    )?;
    if ffi::av_channel_layout_check(&selected) == 1 {
        return Ok(selected);
    }

    ffi::av_channel_layout_uninit(&mut selected);
    ffi::av_channel_layout_default(&mut selected, 2);
    if ffi::av_channel_layout_check(&selected) == 1 {
        Ok(selected)
    } else {
        Err("failed to allocate stereo channel layout".to_string())
    }
}

unsafe fn decode_and_encode(
    decoder_ctx: *mut ffi::AVCodecContext,
    encoder_ctx: *mut ffi::AVCodecContext,
    swr: *mut ffi::SwrContext,
    decoded: *mut ffi::AVFrame,
    converted: *mut ffi::AVFrame,
    packet: *mut ffi::AVPacket,
    output_ctx: *mut ffi::AVFormatContext,
    out_stream: *mut ffi::AVStream,
    input_stream: *const ffi::AVStream,
    next_pts: &mut i64,
) -> Result<(), String> {
    ffmpeg_call(
        ffi::avcodec_send_packet(decoder_ctx, packet),
        "failed to send audio packet to decoder",
    )?;
    receive_decoded_frames(
        decoder_ctx,
        encoder_ctx,
        swr,
        decoded,
        converted,
        output_ctx,
        out_stream,
        input_stream,
        next_pts,
    )
}

unsafe fn receive_decoded_frames(
    decoder_ctx: *mut ffi::AVCodecContext,
    encoder_ctx: *mut ffi::AVCodecContext,
    swr: *mut ffi::SwrContext,
    decoded: *mut ffi::AVFrame,
    converted: *mut ffi::AVFrame,
    output_ctx: *mut ffi::AVFormatContext,
    out_stream: *mut ffi::AVStream,
    input_stream: *const ffi::AVStream,
    next_pts: &mut i64,
) -> Result<(), String> {
    loop {
        let code = ffi::avcodec_receive_frame(decoder_ctx, decoded);
        if code == ffi::AVERROR(ffi::EAGAIN) || code == ffi::AVERROR_EOF {
            return Ok(());
        }
        ffmpeg_call(code, "failed to receive decoded audio frame")?;

        ffi::av_frame_unref(converted);
        (*converted).format = (*encoder_ctx).sample_fmt as i32;
        (*converted).sample_rate = (*encoder_ctx).sample_rate;
        ffmpeg_call(
            ffi::av_channel_layout_copy(&mut (*converted).ch_layout, &(*encoder_ctx).ch_layout),
            "failed to copy converted frame channel layout",
        )?;
        (*converted).nb_samples =
            ffi::swr_get_out_samples(swr, (*decoded).nb_samples).max((*decoded).nb_samples);
        ffmpeg_call(
            ffi::swr_convert_frame(swr, converted, decoded),
            "failed to convert decoded audio frame",
        )?;
        (*converted).pts = *next_pts;
        *next_pts += (*converted).nb_samples as i64;

        encode_converted_frame(encoder_ctx, converted, output_ctx, out_stream)?;
        ffi::av_frame_unref(decoded);
        ffi::av_frame_unref(converted);

        let _ = input_stream;
    }
}

unsafe fn encode_converted_frame(
    encoder_ctx: *mut ffi::AVCodecContext,
    converted: *mut ffi::AVFrame,
    output_ctx: *mut ffi::AVFormatContext,
    out_stream: *mut ffi::AVStream,
) -> Result<(), String> {
    ffmpeg_call(
        ffi::avcodec_send_frame(encoder_ctx, converted),
        "failed to send audio frame to encoder",
    )?;
    let mut packet = Packet::new()?;
    loop {
        let code = ffi::avcodec_receive_packet(encoder_ctx, packet.as_mut_ptr());
        if code == ffi::AVERROR(ffi::EAGAIN) || code == ffi::AVERROR_EOF {
            return Ok(());
        }
        ffmpeg_call(code, "failed to receive encoded MP3 packet")?;

        ffi::av_packet_rescale_ts(
            packet.as_mut_ptr(),
            (*encoder_ctx).time_base,
            (*out_stream).time_base,
        );
        (*packet.as_mut_ptr()).stream_index = (*out_stream).index;
        ffmpeg_call(
            ffi::av_interleaved_write_frame(output_ctx, packet.as_mut_ptr()),
            "failed to write encoded MP3 packet",
        )?;
        ffi::av_packet_unref(packet.as_mut_ptr());
    }
}

unsafe fn flush_encoder(
    encoder_ctx: *mut ffi::AVCodecContext,
    output_ctx: *mut ffi::AVFormatContext,
    out_stream: *mut ffi::AVStream,
    packet: *mut ffi::AVPacket,
) -> Result<(), String> {
    ffmpeg_call(
        ffi::avcodec_send_frame(encoder_ctx, ptr::null()),
        "failed to flush encoder",
    )?;

    loop {
        let code = ffi::avcodec_receive_packet(encoder_ctx, packet);
        if code == ffi::AVERROR(ffi::EAGAIN) || code == ffi::AVERROR_EOF {
            return Ok(());
        }
        ffmpeg_call(code, "failed to drain encoded MP3 packet")?;

        ffi::av_packet_rescale_ts(packet, (*encoder_ctx).time_base, (*out_stream).time_base);
        (*packet).stream_index = (*out_stream).index;
        ffmpeg_call(
            ffi::av_interleaved_write_frame(output_ctx, packet),
            "failed to write flushed MP3 packet",
        )?;
        ffi::av_packet_unref(packet);
    }
}

struct InputContext(*mut ffi::AVFormatContext);

impl InputContext {
    unsafe fn open(path: &CString) -> Result<Self, String> {
        let mut ctx = ptr::null_mut();
        ffmpeg_call(
            ffi::avformat_open_input(&mut ctx, path.as_ptr(), ptr::null_mut(), ptr::null_mut()),
            "failed to open input file",
        )?;
        ffmpeg_call(
            ffi::avformat_find_stream_info(ctx, ptr::null_mut()),
            "failed to read input stream information",
        )?;
        Ok(Self(ctx))
    }

    fn as_mut_ptr(&mut self) -> *mut ffi::AVFormatContext {
        self.0
    }

    unsafe fn view(&self) -> &FormatContextView {
        &*(self.0.cast::<FormatContextView>())
    }

    unsafe fn best_audio_stream(&mut self) -> Result<(i32, *const ffi::AVCodec), String> {
        let mut decoder: *const ffi::AVCodec = ptr::null();
        let index = ffi::av_find_best_stream(
            self.as_mut_ptr(),
            ffi::AVMEDIA_TYPE_AUDIO,
            -1,
            -1,
            &mut decoder,
            0,
        );
        if index < 0 {
            return Err(format!(
                "failed to find an audio stream in the input file: {}",
                ffmpeg_error_string(index)
            ));
        }

        Ok((index, decoder))
    }
}

impl Drop for InputContext {
    fn drop(&mut self) {
        unsafe {
            if !self.0.is_null() {
                ffi::avformat_close_input(&mut self.0);
            }
        }
    }
}

struct OutputContext(*mut ffi::AVFormatContext);

impl OutputContext {
    unsafe fn create(path: &CString) -> Result<Self, String> {
        let mut ctx = ptr::null_mut();
        ffmpeg_call(
            ffi::avformat_alloc_output_context2(
                &mut ctx,
                ptr::null_mut(),
                ptr::null(),
                path.as_ptr(),
            ),
            "failed to allocate output context",
        )?;
        Ok(Self(ctx))
    }

    fn as_mut_ptr(&mut self) -> *mut ffi::AVFormatContext {
        self.0
    }

    unsafe fn view(&self) -> &FormatContextView {
        &*(self.0.cast::<FormatContextView>())
    }

    unsafe fn view_mut(&mut self) -> &mut FormatContextView {
        &mut *(self.0.cast::<FormatContextView>())
    }

    unsafe fn new_stream(
        &mut self,
        codec: *const ffi::AVCodec,
    ) -> Result<*mut ffi::AVStream, String> {
        let stream = ffi::avformat_new_stream(self.as_mut_ptr(), codec);
        if stream.is_null() {
            Err("failed to allocate output stream".to_string())
        } else {
            Ok(stream)
        }
    }

    unsafe fn open_io(&mut self, path: &CString) -> Result<(), String> {
        if (*self.view().oformat).flags & ffi::AVFMT_NOFILE as i32 != 0 {
            return Ok(());
        }

        ffmpeg_call(
            ffi::avio_open(
                &mut self.view_mut().pb,
                path.as_ptr(),
                ffi::AVIO_FLAG_WRITE as i32,
            ),
            "failed to open output file for writing",
        )
    }
}

impl Drop for OutputContext {
    fn drop(&mut self) {
        unsafe {
            if self.0.is_null() {
                return;
            }
            let view = &mut *(self.0.cast::<FormatContextView>());
            if !view.pb.is_null() && (*view.oformat).flags & ffi::AVFMT_NOFILE as i32 == 0 {
                ffi::avio_closep(&mut view.pb);
            }
            ffi::avformat_free_context(self.0);
        }
    }
}

struct CodecContext(*mut ffi::AVCodecContext);

impl CodecContext {
    unsafe fn decoder(
        codec: *const ffi::AVCodec,
        params: *const ffi::AVCodecParameters,
    ) -> Result<Self, String> {
        let ctx = ffi::avcodec_alloc_context3(codec);
        if ctx.is_null() {
            return Err("failed to allocate decoder context".to_string());
        }
        ffmpeg_call(
            ffi::avcodec_parameters_to_context(ctx, params),
            "failed to copy decoder parameters",
        )?;
        ffmpeg_call(
            ffi::avcodec_open2(ctx, codec, ptr::null_mut()),
            "failed to open audio decoder",
        )?;
        Ok(Self(ctx))
    }

    unsafe fn encoder(codec: *const ffi::AVCodec) -> Result<Self, String> {
        let ctx = ffi::avcodec_alloc_context3(codec);
        if ctx.is_null() {
            Err("failed to allocate encoder context".to_string())
        } else {
            Ok(Self(ctx))
        }
    }

    fn as_ptr(&self) -> *mut ffi::AVCodecContext {
        self.0
    }

    fn as_mut_ptr(&mut self) -> *mut ffi::AVCodecContext {
        self.0
    }
}

impl Drop for CodecContext {
    fn drop(&mut self) {
        unsafe {
            if !self.0.is_null() {
                ffi::avcodec_free_context(&mut self.0);
            }
        }
    }
}

struct Frame(*mut ffi::AVFrame);

impl Frame {
    unsafe fn new() -> Result<Self, String> {
        let frame = ffi::av_frame_alloc();
        if frame.is_null() {
            Err("failed to allocate AVFrame".to_string())
        } else {
            Ok(Self(frame))
        }
    }

    fn as_mut_ptr(&mut self) -> *mut ffi::AVFrame {
        self.0
    }
}

impl Drop for Frame {
    fn drop(&mut self) {
        unsafe {
            if !self.0.is_null() {
                ffi::av_frame_free(&mut self.0);
            }
        }
    }
}

struct Packet(*mut ffi::AVPacket);

impl Packet {
    unsafe fn new() -> Result<Self, String> {
        let packet = ffi::av_packet_alloc();
        if packet.is_null() {
            Err("failed to allocate AVPacket".to_string())
        } else {
            Ok(Self(packet))
        }
    }

    fn as_ptr(&self) -> *mut ffi::AVPacket {
        self.0
    }

    fn as_mut_ptr(&mut self) -> *mut ffi::AVPacket {
        self.0
    }
}

impl Drop for Packet {
    fn drop(&mut self) {
        unsafe {
            if !self.0.is_null() {
                ffi::av_packet_free(&mut self.0);
            }
        }
    }
}

struct Resampler(*mut ffi::SwrContext);

impl Resampler {
    unsafe fn new(
        decoder_ctx: *const ffi::AVCodecContext,
        encoder_ctx: *const ffi::AVCodecContext,
    ) -> Result<Self, String> {
        let mut swr = ptr::null_mut();
        ffmpeg_call(
            ffi::swr_alloc_set_opts2(
                &mut swr,
                &(*encoder_ctx).ch_layout,
                (*encoder_ctx).sample_fmt,
                (*encoder_ctx).sample_rate,
                &(*decoder_ctx).ch_layout,
                (*decoder_ctx).sample_fmt,
                (*decoder_ctx).sample_rate,
                0,
                ptr::null_mut(),
            ),
            "failed to configure audio resampler",
        )?;
        ffmpeg_call(ffi::swr_init(swr), "failed to initialize audio resampler")?;
        Ok(Self(swr))
    }

    fn as_mut_ptr(&mut self) -> *mut ffi::SwrContext {
        self.0
    }
}

impl Drop for Resampler {
    fn drop(&mut self) {
        unsafe {
            if !self.0.is_null() {
                ffi::swr_free(&mut self.0);
            }
        }
    }
}

fn parse_integrated_lufs(stderr: &str) -> Option<f64> {
    stderr.lines().rev().find_map(|line| {
        let trimmed = line.trim();
        if !trimmed.starts_with("I:") || !trimmed.contains("LUFS") {
            return None;
        }

        trimmed
            .split_whitespace()
            .nth(1)
            .and_then(|value| value.parse::<f64>().ok())
    })
}

#[derive(Debug, Deserialize)]
struct FfprobeStreams {
    #[serde(default)]
    streams: Vec<FfprobeStream>,
}

#[derive(Debug, Deserialize)]
struct FfprobeStream {
    #[serde(default)]
    disposition: FfprobeDisposition,
}

#[derive(Debug, Default, Deserialize)]
struct FfprobeDisposition {
    #[serde(default)]
    attached_pic: i32,
}

fn has_embedded_cover(input: &Path) -> Result<bool, String> {
    let result = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v",
            "-show_entries",
            "stream=disposition",
            "-of",
            "json",
        ])
        .arg(input)
        .output()
        .map_err(|e| format!("failed to run ffprobe: {e}"))?;

    if !result.status.success() {
        return Err(format!(
            "ffprobe failed while checking cover art: {}",
            stderr_message(&result.stderr)
        ));
    }

    let parsed: FfprobeStreams = serde_json::from_slice(&result.stdout)
        .map_err(|e| format!("failed to parse ffprobe output: {e}"))?;

    Ok(parsed
        .streams
        .iter()
        .any(|stream| stream.disposition.attached_pic == 1))
}

#[cfg(test)]
mod tests {
    use super::{parse_integrated_lufs, transcode_audio_to_mp3};
    use std::path::Path;

    #[test]
    fn parse_integrated_lufs_reads_summary_value() {
        let stderr = r#"
[Parsed_ebur128_0 @ 0x0] Summary:
  Integrated loudness:
    I:         -14.7 LUFS
"#;

        assert_eq!(parse_integrated_lufs(stderr), Some(-14.7));
    }

    #[test]
    fn transcode_audio_to_mp3_reports_open_input_errors() {
        let err = transcode_audio_to_mp3(
            Path::new("/definitely/missing/input.webm"),
            Path::new("/tmp/output.mp3"),
        )
        .unwrap_err();
        assert!(err.contains("failed to open input file"));
    }
}
