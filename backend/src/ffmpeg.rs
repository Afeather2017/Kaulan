//! FFmpeg/FFprobe-backed helpers for audio conversion and metadata extraction.
//!
//! Documentation: [docs/ffmpeg-audio-pipeline.md](../../docs/ffmpeg-audio-pipeline.md)

use crate::file_ops::{get_file_reader, resolve_path, PathKind};
use futures::StreamExt;
use rusty_ffmpeg::ffi;
use std::ffi::{CStr, CString};
use std::path::{Path, PathBuf};
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

enum PreferredAudioOutput {
    Remux { extension: &'static str },
    TranscodeFlac,
    TranscodeMp3,
}

pub fn export_audio_for_download(
    input: &Path,
    output_dir: &Path,
    output_stem: &str,
) -> Result<(String, PathBuf), String> {
    let codec_id = detect_primary_audio_codec(input)?;
    let (file_name, output_path) = match preferred_output_for_codec(codec_id) {
        PreferredAudioOutput::Remux { extension } => {
            let file_name = format!("{output_stem}.{extension}");
            let output_path = output_dir.join(&file_name);
            remux_audio_stream(input, &output_path)?;
            (file_name, output_path)
        }
        PreferredAudioOutput::TranscodeFlac => {
            let file_name = format!("{output_stem}.flac");
            let output_path = output_dir.join(&file_name);
            transcode_audio_to_flac(input, &output_path)?;
            (file_name, output_path)
        }
        PreferredAudioOutput::TranscodeMp3 => {
            let file_name = format!("{output_stem}.mp3");
            let output_path = output_dir.join(&file_name);
            transcode_audio_to_mp3(input, &output_path)?;
            (file_name, output_path)
        }
    };

    Ok((file_name, output_path))
}

pub fn transcode_audio_to_mp3(input: &Path, output: &Path) -> Result<(), String> {
    transcode_audio(input, output, AudioTranscodeTarget::Mp3)
}

pub fn transcode_audio_to_flac(input: &Path, output: &Path) -> Result<(), String> {
    transcode_audio(input, output, AudioTranscodeTarget::Flac)
}

fn transcode_audio(
    input: &Path,
    output: &Path,
    target: AudioTranscodeTarget,
) -> Result<(), String> {
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
        let encoder = find_encoder(target)?;
        let mut encoder_ctx = CodecContext::encoder(encoder)?;
        configure_encoder(
            target,
            encoder,
            decoder_ctx.as_ptr(),
            encoder_ctx.as_mut_ptr(),
        )?;

        let out_stream = output_ctx.new_stream(encoder)?;
        if (*output_ctx.view().oformat).flags & ffi::AVFMT_GLOBALHEADER as i32 != 0 {
            (*encoder_ctx.as_mut_ptr()).flags |= ffi::AV_CODEC_FLAG_GLOBAL_HEADER as i32;
        }

        let encoder_name = target.display_name();
        ffmpeg_call(
            ffi::avcodec_open2(encoder_ctx.as_mut_ptr(), encoder, ptr::null_mut()),
            &format!("failed to open {encoder_name} encoder"),
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

        let mut swr = if matches!(target, AudioTranscodeTarget::Flac)
            || audio_formats_match(decoder_ctx.as_ptr(), encoder_ctx.as_ptr())
        {
            None
        } else {
            Some(Resampler::new(decoder_ctx.as_ptr(), encoder_ctx.as_ptr())?)
        };
        let mut fifo = AudioFifo::new(encoder_ctx.as_ptr())?;
        let mut packet = Packet::new()?;
        let mut decoded = Frame::new()?;
        let mut converted = Frame::new()?;
        let mut next_pts = 0_i64;

        while ffi::av_read_frame(input_ctx.as_mut_ptr(), packet.as_mut_ptr()) >= 0 {
            if (*packet.as_ptr()).stream_index == stream_index {
                decode_and_encode(
                    decoder_ctx.as_mut_ptr(),
                    encoder_ctx.as_mut_ptr(),
                    swr.as_mut().map_or(ptr::null_mut(), Resampler::as_mut_ptr),
                    decoded.as_mut_ptr(),
                    converted.as_mut_ptr(),
                    packet.as_mut_ptr(),
                    output_ctx.as_mut_ptr(),
                    out_stream,
                    fifo.as_mut_ptr(),
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
            swr.as_mut().map_or(ptr::null_mut(), Resampler::as_mut_ptr),
            decoded.as_mut_ptr(),
            converted.as_mut_ptr(),
            output_ctx.as_mut_ptr(),
            out_stream,
            fifo.as_mut_ptr(),
            &mut next_pts,
        )?;
        if let Some(swr) = swr.as_mut() {
            drain_resampler_into_fifo(
                encoder_ctx.as_mut_ptr(),
                swr.as_mut_ptr(),
                converted.as_mut_ptr(),
                fifo.as_mut_ptr(),
                output_ctx.as_mut_ptr(),
                out_stream,
                &mut next_pts,
            )?;
        }
        flush_audio_fifo(
            encoder_ctx.as_mut_ptr(),
            fifo.as_mut_ptr(),
            output_ctx.as_mut_ptr(),
            out_stream,
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

#[derive(Clone, Copy)]
enum AudioTranscodeTarget {
    Mp3,
    Flac,
}

impl AudioTranscodeTarget {
    fn display_name(self) -> &'static str {
        match self {
            Self::Mp3 => "MP3",
            Self::Flac => "FLAC",
        }
    }
}

pub fn detect_primary_audio_codec(input: &Path) -> Result<ffi::AVCodecID, String> {
    let input = path_to_cstring(input)?;
    unsafe {
        let mut input_ctx = InputContext::open(&input)?;
        let (stream_index, _) = input_ctx.best_audio_stream()?;
        let stream = input_ctx.stream(stream_index as usize)?;
        Ok((*(*stream).codecpar).codec_id)
    }
}

pub fn remux_audio_stream(input: &Path, output: &Path) -> Result<(), String> {
    let input = path_to_cstring(input)?;
    let output = path_to_cstring(output)?;

    unsafe {
        let mut input_ctx = InputContext::open(&input)?;
        let (stream_index, _) = input_ctx.best_audio_stream()?;
        let input_stream = input_ctx.stream(stream_index as usize)?;

        let mut output_ctx = OutputContext::create(&output)?;
        let out_stream = output_ctx.new_stream(ptr::null())?;
        ffmpeg_call(
            ffi::avcodec_parameters_copy((*out_stream).codecpar, (*input_stream).codecpar),
            "failed to copy audio stream parameters for remux",
        )?;
        (*(*out_stream).codecpar).codec_tag = 0;
        (*out_stream).time_base = (*input_stream).time_base;

        output_ctx.open_io(&output)?;
        ffmpeg_call(
            ffi::avformat_write_header(output_ctx.as_mut_ptr(), ptr::null_mut()),
            "failed to write remux output header",
        )?;

        let mut packet = Packet::new()?;
        while ffi::av_read_frame(input_ctx.as_mut_ptr(), packet.as_mut_ptr()) >= 0 {
            if (*packet.as_ptr()).stream_index == stream_index {
                ffi::av_packet_rescale_ts(
                    packet.as_mut_ptr(),
                    (*input_stream).time_base,
                    (*out_stream).time_base,
                );
                (*packet.as_mut_ptr()).stream_index = (*out_stream).index;
                (*packet.as_mut_ptr()).pos = -1;
                ffmpeg_call(
                    ffi::av_interleaved_write_frame(output_ctx.as_mut_ptr(), packet.as_mut_ptr()),
                    "failed to write remuxed packet",
                )?;
            }
            ffi::av_packet_unref(packet.as_mut_ptr());
        }

        ffmpeg_call(
            ffi::av_write_trailer(output_ctx.as_mut_ptr()),
            "failed to finalize remuxed output file",
        )?;
    }

    Ok(())
}

pub fn extract_cover_art(input: &Path) -> Result<Option<(String, Vec<u8>)>, String> {
    let input = path_to_cstring(input)?;

    unsafe {
        let input_ctx = InputContext::open(&input)?;
        let Some((stream, codec_id)) = input_ctx.attached_picture_stream() else {
            return Ok(None);
        };

        let packet = &(*stream).attached_pic;
        if packet.data.is_null() || packet.size <= 0 {
            return Ok(None);
        }

        let bytes = std::slice::from_raw_parts(packet.data, packet.size as usize).to_vec();
        if bytes.is_empty() {
            return Ok(None);
        }

        Ok(Some((cover_mime_type(codec_id).to_string(), bytes)))
    }
}

pub fn calculate_lufs(input: &Path) -> Result<Option<f64>, String> {
    let input = path_to_cstring(input)?;

    unsafe {
        let mut input_ctx = match InputContext::open(&input) {
            Ok(ctx) => ctx,
            Err(err) if is_unsupported_media_error(&err) => return Ok(None),
            Err(err) => return Err(err),
        };
        let (stream_index, decoder) = match input_ctx.best_audio_stream() {
            Ok(stream) => stream,
            Err(err) if err.contains("failed to find an audio stream") => return Ok(None),
            Err(err) => return Err(err),
        };
        let input_stream = input_ctx.stream(stream_index as usize)?;
        let mut decoder_ctx = CodecContext::decoder(decoder, (*input_stream).codecpar)?;
        normalize_channel_layout(decoder_ctx.as_mut_ptr(), 2)?;

        let mut analyzer = LoudnessAnalyzer::new(decoder_ctx.as_ptr())?;
        let mut packet = Packet::new()?;
        let mut decoded = Frame::new()?;
        let mut filtered = Frame::new()?;
        let mut integrated_lufs = None;

        while ffi::av_read_frame(input_ctx.as_mut_ptr(), packet.as_mut_ptr()) >= 0 {
            if (*packet.as_ptr()).stream_index == stream_index {
                decode_and_analyze_loudness(
                    decoder_ctx.as_mut_ptr(),
                    analyzer.as_mut_ptr(),
                    decoded.as_mut_ptr(),
                    filtered.as_mut_ptr(),
                    packet.as_mut_ptr(),
                    &mut integrated_lufs,
                )?;
            }
            ffi::av_packet_unref(packet.as_mut_ptr());
        }

        ffmpeg_call(
            ffi::avcodec_send_packet(decoder_ctx.as_mut_ptr(), ptr::null()),
            "failed to flush decoder",
        )?;
        receive_loudness_frames(
            decoder_ctx.as_mut_ptr(),
            analyzer.as_mut_ptr(),
            decoded.as_mut_ptr(),
            filtered.as_mut_ptr(),
            &mut integrated_lufs,
        )?;
        analyzer.close_source()?;
        analyzer.drain(filtered.as_mut_ptr(), &mut integrated_lufs)?;

        Ok(integrated_lufs)
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
    let mut buffer = [0 as std::ffi::c_char; ffi::AV_ERROR_MAX_STRING_SIZE as usize];
    unsafe {
        ffi::av_strerror(code, buffer.as_mut_ptr(), buffer.len());
        CStr::from_ptr(buffer.as_ptr())
            .to_string_lossy()
            .into_owned()
    }
}

fn preferred_output_for_codec(codec_id: ffi::AVCodecID) -> PreferredAudioOutput {
    match codec_id {
        ffi::AV_CODEC_ID_AAC => PreferredAudioOutput::Remux { extension: "m4a" },
        ffi::AV_CODEC_ID_OPUS | ffi::AV_CODEC_ID_VORBIS => {
            PreferredAudioOutput::Remux { extension: "ogg" }
        }
        ffi::AV_CODEC_ID_MP3 => PreferredAudioOutput::Remux { extension: "mp3" },
        ffi::AV_CODEC_ID_FLAC => PreferredAudioOutput::Remux { extension: "flac" },
        codec_id if is_pcm_codec(codec_id) => PreferredAudioOutput::TranscodeFlac,
        _ => PreferredAudioOutput::TranscodeMp3,
    }
}

fn is_pcm_codec(codec_id: ffi::AVCodecID) -> bool {
    matches!(
        codec_id,
        ffi::AV_CODEC_ID_PCM_S16LE
            | ffi::AV_CODEC_ID_PCM_S16BE
            | ffi::AV_CODEC_ID_PCM_U16LE
            | ffi::AV_CODEC_ID_PCM_U16BE
            | ffi::AV_CODEC_ID_PCM_S8
            | ffi::AV_CODEC_ID_PCM_U8
            | ffi::AV_CODEC_ID_PCM_MULAW
            | ffi::AV_CODEC_ID_PCM_ALAW
            | ffi::AV_CODEC_ID_PCM_S32LE
            | ffi::AV_CODEC_ID_PCM_S32BE
            | ffi::AV_CODEC_ID_PCM_U32LE
            | ffi::AV_CODEC_ID_PCM_U32BE
            | ffi::AV_CODEC_ID_PCM_S24LE
            | ffi::AV_CODEC_ID_PCM_S24BE
            | ffi::AV_CODEC_ID_PCM_U24LE
            | ffi::AV_CODEC_ID_PCM_U24BE
            | ffi::AV_CODEC_ID_PCM_S24DAUD
            | ffi::AV_CODEC_ID_PCM_ZORK
            | ffi::AV_CODEC_ID_PCM_S16LE_PLANAR
            | ffi::AV_CODEC_ID_PCM_DVD
            | ffi::AV_CODEC_ID_PCM_F32BE
            | ffi::AV_CODEC_ID_PCM_F32LE
            | ffi::AV_CODEC_ID_PCM_F64BE
            | ffi::AV_CODEC_ID_PCM_F64LE
            | ffi::AV_CODEC_ID_PCM_BLURAY
            | ffi::AV_CODEC_ID_PCM_LXF
            | ffi::AV_CODEC_ID_PCM_S8_PLANAR
            | ffi::AV_CODEC_ID_PCM_S24LE_PLANAR
            | ffi::AV_CODEC_ID_PCM_S32LE_PLANAR
            | ffi::AV_CODEC_ID_PCM_S16BE_PLANAR
            | ffi::AV_CODEC_ID_PCM_S64LE
            | ffi::AV_CODEC_ID_PCM_S64BE
            | ffi::AV_CODEC_ID_PCM_F16LE
            | ffi::AV_CODEC_ID_PCM_F24LE
    )
}

unsafe fn find_encoder(target: AudioTranscodeTarget) -> Result<*const ffi::AVCodec, String> {
    match target {
        AudioTranscodeTarget::Mp3 => {
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
        AudioTranscodeTarget::Flac => {
            let encoder = ffi::avcodec_find_encoder(ffi::AV_CODEC_ID_FLAC);
            if encoder.is_null() {
                Err("failed to find a FLAC encoder in the linked FFmpeg libraries".to_string())
            } else {
                Ok(encoder)
            }
        }
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

unsafe fn configure_encoder(
    target: AudioTranscodeTarget,
    encoder: *const ffi::AVCodec,
    decoder_ctx: *const ffi::AVCodecContext,
    encoder_ctx: *mut ffi::AVCodecContext,
) -> Result<(), String> {
    let sample_rate = select_sample_rate(encoder, (*decoder_ctx).sample_rate);
    let sample_fmt = select_sample_fmt(encoder, target, (*decoder_ctx).sample_fmt)?;
    let channel_layout = select_channel_layout(encoder, &(*decoder_ctx).ch_layout)?;

    (*encoder_ctx).sample_rate = sample_rate;
    (*encoder_ctx).sample_fmt = sample_fmt;
    ffmpeg_call(
        ffi::av_channel_layout_copy(&mut (*encoder_ctx).ch_layout, &channel_layout),
        "failed to copy encoder channel layout",
    )?;
    if matches!(target, AudioTranscodeTarget::Mp3) {
        (*encoder_ctx).bit_rate = 320_000;
    }
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

unsafe fn select_sample_fmt(
    encoder: *const ffi::AVCodec,
    target: AudioTranscodeTarget,
    decoder_sample_fmt: ffi::AVSampleFormat,
) -> Result<ffi::AVSampleFormat, String> {
    let formats = (*encoder).sample_fmts;
    if formats.is_null() {
        return Err(format!(
            "{} encoder did not expose supported sample formats",
            target.display_name()
        ));
    }

    let mut index = 0;
    let preferred = match target {
        AudioTranscodeTarget::Mp3 => Some(ffi::AV_SAMPLE_FMT_FLTP),
        AudioTranscodeTarget::Flac => Some(decoder_sample_fmt),
    };
    let secondary_preferred = match target {
        AudioTranscodeTarget::Mp3 => None,
        AudioTranscodeTarget::Flac => Some(ffi::AV_SAMPLE_FMT_S16),
    };
    let mut fallback = None;
    while *formats.add(index) != ffi::AV_SAMPLE_FMT_NONE {
        let value = *formats.add(index);
        if Some(value) == preferred {
            return Ok(value);
        }
        if Some(value) == secondary_preferred {
            return Ok(value);
        }
        if index == 0 {
            fallback = Some(value);
        }
        index += 1;
    }

    fallback.ok_or_else(|| {
        format!(
            "{} encoder did not expose any usable sample format",
            target.display_name()
        )
    })
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

unsafe fn audio_formats_match(
    decoder_ctx: *const ffi::AVCodecContext,
    encoder_ctx: *const ffi::AVCodecContext,
) -> bool {
    (*decoder_ctx).sample_fmt == (*encoder_ctx).sample_fmt
        && (*decoder_ctx).sample_rate == (*encoder_ctx).sample_rate
        && ffi::av_channel_layout_compare(&(*decoder_ctx).ch_layout, &(*encoder_ctx).ch_layout) == 0
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
    fifo: *mut AudioFifoContext,
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
        fifo,
        next_pts,
    )
}

unsafe fn decode_and_analyze_loudness(
    decoder_ctx: *mut ffi::AVCodecContext,
    analyzer: *mut LoudnessAnalyzerGraph,
    decoded: *mut ffi::AVFrame,
    filtered: *mut ffi::AVFrame,
    packet: *mut ffi::AVPacket,
    integrated_lufs: &mut Option<f64>,
) -> Result<(), String> {
    ffmpeg_call(
        ffi::avcodec_send_packet(decoder_ctx, packet),
        "failed to send audio packet to decoder",
    )?;
    receive_loudness_frames(decoder_ctx, analyzer, decoded, filtered, integrated_lufs)
}

unsafe fn receive_loudness_frames(
    decoder_ctx: *mut ffi::AVCodecContext,
    analyzer: *mut LoudnessAnalyzerGraph,
    decoded: *mut ffi::AVFrame,
    filtered: *mut ffi::AVFrame,
    integrated_lufs: &mut Option<f64>,
) -> Result<(), String> {
    loop {
        let code = ffi::avcodec_receive_frame(decoder_ctx, decoded);
        if code == ffi::AVERROR(ffi::EAGAIN) || code == ffi::AVERROR_EOF {
            return Ok(());
        }
        ffmpeg_call(code, "failed to receive decoded audio frame")?;

        (*analyzer).push_frame(decoded)?;
        (*analyzer).drain(filtered, integrated_lufs)?;
        ffi::av_frame_unref(decoded);
    }
}

unsafe fn receive_decoded_frames(
    decoder_ctx: *mut ffi::AVCodecContext,
    encoder_ctx: *mut ffi::AVCodecContext,
    swr: *mut ffi::SwrContext,
    decoded: *mut ffi::AVFrame,
    converted: *mut ffi::AVFrame,
    output_ctx: *mut ffi::AVFormatContext,
    out_stream: *mut ffi::AVStream,
    fifo: *mut AudioFifoContext,
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
        (*converted).nb_samples = if swr.is_null() {
            (*decoded).nb_samples
        } else {
            ffi::swr_get_out_samples(swr, (*decoded).nb_samples).max((*decoded).nb_samples)
        };
        ffmpeg_call(
            if swr.is_null() {
                ffi::av_frame_ref(converted, decoded)
            } else {
                ffi::swr_convert_frame(swr, converted, decoded)
            },
            if swr.is_null() {
                "failed to clone decoded audio frame"
            } else {
                "failed to convert decoded audio frame"
            },
        )?;
        (*fifo).write(converted)?;
        encode_fifo_frames(encoder_ctx, fifo, output_ctx, out_stream, next_pts, false)?;
        ffi::av_frame_unref(decoded);
        ffi::av_frame_unref(converted);
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

unsafe fn drain_resampler_into_fifo(
    encoder_ctx: *mut ffi::AVCodecContext,
    swr: *mut ffi::SwrContext,
    converted: *mut ffi::AVFrame,
    fifo: *mut AudioFifoContext,
    output_ctx: *mut ffi::AVFormatContext,
    out_stream: *mut ffi::AVStream,
    next_pts: &mut i64,
) -> Result<(), String> {
    loop {
        ffi::av_frame_unref(converted);
        (*converted).format = (*encoder_ctx).sample_fmt as i32;
        (*converted).sample_rate = (*encoder_ctx).sample_rate;
        ffmpeg_call(
            ffi::av_channel_layout_copy(&mut (*converted).ch_layout, &(*encoder_ctx).ch_layout),
            "failed to copy drained frame channel layout",
        )?;
        (*converted).nb_samples = ffi::swr_get_out_samples(swr, 0);
        if (*converted).nb_samples <= 0 {
            break;
        }

        ffmpeg_call(
            ffi::swr_convert_frame(swr, converted, ptr::null()),
            "failed to drain resampler",
        )?;
        if (*converted).nb_samples <= 0 {
            ffi::av_frame_unref(converted);
            break;
        }

        (*fifo).write(converted)?;
        encode_fifo_frames(encoder_ctx, fifo, output_ctx, out_stream, next_pts, false)?;
        ffi::av_frame_unref(converted);
    }

    Ok(())
}

unsafe fn flush_audio_fifo(
    encoder_ctx: *mut ffi::AVCodecContext,
    fifo: *mut AudioFifoContext,
    output_ctx: *mut ffi::AVFormatContext,
    out_stream: *mut ffi::AVStream,
    next_pts: &mut i64,
) -> Result<(), String> {
    encode_fifo_frames(encoder_ctx, fifo, output_ctx, out_stream, next_pts, true)
}

unsafe fn encode_fifo_frames(
    encoder_ctx: *mut ffi::AVCodecContext,
    fifo: *mut AudioFifoContext,
    output_ctx: *mut ffi::AVFormatContext,
    out_stream: *mut ffi::AVStream,
    next_pts: &mut i64,
    flush_partial: bool,
) -> Result<(), String> {
    let frame_size = (*encoder_ctx).frame_size.max(1);
    loop {
        let available = (*fifo).size();
        if available <= 0 {
            return Ok(());
        }
        if !flush_partial && available < frame_size {
            return Ok(());
        }

        let samples = if flush_partial {
            available.min(frame_size)
        } else {
            frame_size
        };
        let mut frame = allocate_audio_frame(encoder_ctx, samples)?;
        frame.set_pts(*next_pts);
        *next_pts += i64::from(samples);
        (*fifo).read(frame.as_mut_ptr(), samples)?;
        encode_converted_frame(encoder_ctx, frame.as_mut_ptr(), output_ctx, out_stream)?;
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

    unsafe fn stream(&self, index: usize) -> Result<*mut ffi::AVStream, String> {
        let view = self.view();
        if index >= view.nb_streams as usize {
            return Err(format!("stream index {index} out of range"));
        }

        let stream = *view.streams.add(index);
        if stream.is_null() {
            Err(format!("stream index {index} is null"))
        } else {
            Ok(stream)
        }
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

    unsafe fn attached_picture_stream(&self) -> Option<(*mut ffi::AVStream, ffi::AVCodecID)> {
        let view = self.view();
        for index in 0..view.nb_streams as usize {
            let stream = *view.streams.add(index);
            if stream.is_null() {
                continue;
            }

            let has_attached_pic =
                ((*stream).disposition & ffi::AV_DISPOSITION_ATTACHED_PIC as i32) != 0;
            let packet = &(*stream).attached_pic;
            if !has_attached_pic || packet.data.is_null() || packet.size <= 0 {
                continue;
            }

            return Some((stream, (*(*stream).codecpar).codec_id));
        }

        None
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

    fn set_pts(&mut self, pts: i64) {
        unsafe {
            (*self.0).pts = pts;
        }
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

struct AudioFifo(*mut AudioFifoContext);

impl AudioFifo {
    unsafe fn new(encoder_ctx: *const ffi::AVCodecContext) -> Result<Self, String> {
        let fifo = AudioFifoContext::new(encoder_ctx)?;
        Ok(Self(Box::into_raw(Box::new(fifo))))
    }

    fn as_mut_ptr(&mut self) -> *mut AudioFifoContext {
        self.0
    }
}

impl Drop for AudioFifo {
    fn drop(&mut self) {
        unsafe {
            if !self.0.is_null() {
                drop(Box::from_raw(self.0));
            }
        }
    }
}

struct AudioFifoContext {
    fifo: *mut ffi::AVAudioFifo,
    sample_fmt: ffi::AVSampleFormat,
}

impl AudioFifoContext {
    unsafe fn new(encoder_ctx: *const ffi::AVCodecContext) -> Result<Self, String> {
        let fifo = ffi::av_audio_fifo_alloc(
            (*encoder_ctx).sample_fmt,
            (*encoder_ctx).ch_layout.nb_channels,
            (*encoder_ctx).frame_size.max(1),
        );
        if fifo.is_null() {
            return Err("failed to allocate audio fifo".to_string());
        }

        Ok(Self {
            fifo,
            sample_fmt: (*encoder_ctx).sample_fmt,
        })
    }

    unsafe fn write(&mut self, frame: *mut ffi::AVFrame) -> Result<(), String> {
        if (*frame).nb_samples <= 0 {
            return Ok(());
        }

        let written = ffi::av_audio_fifo_write(
            self.fifo,
            (*frame).extended_data.cast(),
            (*frame).nb_samples,
        );
        ffmpeg_call(written, "failed to write samples into audio fifo")?;
        Ok(())
    }

    unsafe fn read(&mut self, frame: *mut ffi::AVFrame, nb_samples: i32) -> Result<(), String> {
        let read = ffi::av_audio_fifo_read(self.fifo, (*frame).extended_data.cast(), nb_samples);
        ffmpeg_call(read, "failed to read samples from audio fifo")?;
        if read != nb_samples {
            return Err(format!(
                "audio fifo returned {read} samples, expected {nb_samples}"
            ));
        }
        Ok(())
    }

    unsafe fn size(&mut self) -> i32 {
        ffi::av_audio_fifo_size(self.fifo)
    }
}

impl Drop for AudioFifoContext {
    fn drop(&mut self) {
        unsafe {
            if !self.fifo.is_null() {
                ffi::av_audio_fifo_free(self.fifo);
            }
            let _ = self.sample_fmt;
        }
    }
}

unsafe fn allocate_audio_frame(
    encoder_ctx: *mut ffi::AVCodecContext,
    nb_samples: i32,
) -> Result<Frame, String> {
    let mut frame = Frame::new()?;
    (*frame.as_mut_ptr()).format = (*encoder_ctx).sample_fmt as i32;
    (*frame.as_mut_ptr()).sample_rate = (*encoder_ctx).sample_rate;
    (*frame.as_mut_ptr()).nb_samples = nb_samples;
    ffmpeg_call(
        ffi::av_channel_layout_copy(
            &mut (*frame.as_mut_ptr()).ch_layout,
            &(*encoder_ctx).ch_layout,
        ),
        "failed to copy allocated frame channel layout",
    )?;
    ffmpeg_call(
        ffi::av_frame_get_buffer(frame.as_mut_ptr(), 0),
        "failed to allocate audio frame buffer",
    )?;
    Ok(frame)
}

struct LoudnessAnalyzer {
    graph: LoudnessAnalyzerGraph,
}

impl LoudnessAnalyzer {
    unsafe fn new(decoder_ctx: *const ffi::AVCodecContext) -> Result<Self, String> {
        Ok(Self {
            graph: LoudnessAnalyzerGraph::new(decoder_ctx)?,
        })
    }

    fn as_mut_ptr(&mut self) -> *mut LoudnessAnalyzerGraph {
        &mut self.graph
    }

    fn close_source(&mut self) -> Result<(), String> {
        unsafe { self.graph.close_source() }
    }

    fn drain(
        &mut self,
        frame: *mut ffi::AVFrame,
        integrated_lufs: &mut Option<f64>,
    ) -> Result<(), String> {
        unsafe { self.graph.drain(frame, integrated_lufs) }
    }
}

struct LoudnessAnalyzerGraph {
    graph: *mut ffi::AVFilterGraph,
    source: *mut ffi::AVFilterContext,
    sink: *mut ffi::AVFilterContext,
}

impl LoudnessAnalyzerGraph {
    unsafe fn new(decoder_ctx: *const ffi::AVCodecContext) -> Result<Self, String> {
        let graph = ffi::avfilter_graph_alloc();
        if graph.is_null() {
            return Err("failed to allocate loudness filter graph".to_string());
        }

        let mut source = ptr::null_mut();
        let mut filter = ptr::null_mut();
        let mut sink = ptr::null_mut();

        let buffer_name = CString::new("abuffer").unwrap();
        let loudness_name = CString::new("ebur128").unwrap();
        let sink_name = CString::new("abuffersink").unwrap();
        let source_instance = CString::new("in").unwrap();
        let filter_instance = CString::new("loudness").unwrap();
        let sink_instance = CString::new("out").unwrap();
        let filter_args = CString::new("metadata=1:video=0").unwrap();

        ffmpeg_call(
            ffi::avfilter_graph_create_filter(
                &mut source,
                ffi::avfilter_get_by_name(buffer_name.as_ptr()),
                source_instance.as_ptr(),
                ptr::null(),
                ptr::null_mut(),
                graph,
            ),
            "failed to create loudness source filter",
        )?;

        let mut params = BufferSrcParameters::new(decoder_ctx)?;
        ffmpeg_call(
            ffi::av_buffersrc_parameters_set(source, params.as_mut_ptr()),
            "failed to configure loudness source filter",
        )?;

        ffmpeg_call(
            ffi::avfilter_graph_create_filter(
                &mut filter,
                ffi::avfilter_get_by_name(loudness_name.as_ptr()),
                filter_instance.as_ptr(),
                filter_args.as_ptr(),
                ptr::null_mut(),
                graph,
            ),
            "failed to create ebur128 filter",
        )?;

        ffmpeg_call(
            ffi::avfilter_graph_create_filter(
                &mut sink,
                ffi::avfilter_get_by_name(sink_name.as_ptr()),
                sink_instance.as_ptr(),
                ptr::null(),
                ptr::null_mut(),
                graph,
            ),
            "failed to create loudness sink filter",
        )?;

        ffmpeg_call(
            ffi::avfilter_link(source, 0, filter, 0),
            "failed to link loudness source filter",
        )?;
        ffmpeg_call(
            ffi::avfilter_link(filter, 0, sink, 0),
            "failed to link ebur128 filter to sink",
        )?;
        ffmpeg_call(
            ffi::avfilter_graph_config(graph, ptr::null_mut()),
            "failed to configure loudness filter graph",
        )?;

        Ok(Self {
            graph,
            source,
            sink,
        })
    }

    unsafe fn push_frame(&mut self, frame: *mut ffi::AVFrame) -> Result<(), String> {
        ffmpeg_call(
            ffi::av_buffersrc_write_frame(self.source, frame),
            "failed to push decoded frame into ebur128 filter",
        )
    }

    unsafe fn close_source(&mut self) -> Result<(), String> {
        ffmpeg_call(
            ffi::av_buffersrc_close(self.source, ffi::AV_NOPTS_VALUE, 0),
            "failed to close loudness filter source",
        )
    }

    unsafe fn drain(
        &mut self,
        frame: *mut ffi::AVFrame,
        integrated_lufs: &mut Option<f64>,
    ) -> Result<(), String> {
        loop {
            let code = ffi::av_buffersink_get_frame(self.sink, frame);
            if code == ffi::AVERROR(ffi::EAGAIN) || code == ffi::AVERROR_EOF {
                return Ok(());
            }
            ffmpeg_call(code, "failed to receive filtered loudness frame")?;

            if let Some(value) = frame_loudness_metadata(frame, "lavfi.r128.I") {
                *integrated_lufs = Some(value);
            }

            ffi::av_frame_unref(frame);
        }
    }
}

impl Drop for LoudnessAnalyzerGraph {
    fn drop(&mut self) {
        unsafe {
            if !self.graph.is_null() {
                ffi::avfilter_graph_free(&mut self.graph);
            }
        }
    }
}

struct BufferSrcParameters(*mut ffi::AVBufferSrcParameters);

impl BufferSrcParameters {
    unsafe fn new(decoder_ctx: *const ffi::AVCodecContext) -> Result<Self, String> {
        let params = ffi::av_buffersrc_parameters_alloc();
        if params.is_null() {
            return Err("failed to allocate buffer source parameters".to_string());
        }

        (*params).format = (*decoder_ctx).sample_fmt as i32;
        (*params).sample_rate = (*decoder_ctx).sample_rate;
        (*params).time_base = ffi::AVRational {
            num: 1,
            den: (*decoder_ctx).sample_rate.max(1),
        };
        ffmpeg_call(
            ffi::av_channel_layout_copy(&mut (*params).ch_layout, &(*decoder_ctx).ch_layout),
            "failed to copy buffer source channel layout",
        )?;

        Ok(Self(params))
    }

    fn as_mut_ptr(&mut self) -> *mut ffi::AVBufferSrcParameters {
        self.0
    }
}

impl Drop for BufferSrcParameters {
    fn drop(&mut self) {
        unsafe {
            if !self.0.is_null() {
                ffi::av_channel_layout_uninit(&mut (*self.0).ch_layout);
                ffi::av_free(self.0.cast());
            }
        }
    }
}

fn cover_mime_type(codec_id: ffi::AVCodecID) -> &'static str {
    match codec_id {
        ffi::AV_CODEC_ID_PNG | ffi::AV_CODEC_ID_APNG => "image/png",
        ffi::AV_CODEC_ID_MJPEG | ffi::AV_CODEC_ID_MJPEGB => "image/jpeg",
        ffi::AV_CODEC_ID_BMP => "image/bmp",
        ffi::AV_CODEC_ID_GIF => "image/gif",
        ffi::AV_CODEC_ID_TIFF => "image/tiff",
        ffi::AV_CODEC_ID_WEBP => "image/webp",
        _ => "application/octet-stream",
    }
}

fn frame_loudness_metadata(frame: *mut ffi::AVFrame, key: &str) -> Option<f64> {
    let key = CString::new(key).ok()?;
    unsafe {
        let entry = ffi::av_dict_get((*frame).metadata, key.as_ptr(), ptr::null(), 0);
        if entry.is_null() || (*entry).value.is_null() {
            return None;
        }

        CStr::from_ptr((*entry).value)
            .to_str()
            .ok()
            .and_then(|value| value.parse::<f64>().ok())
    }
}

fn is_unsupported_media_error(message: &str) -> bool {
    message.contains("Invalid data found")
        || message.contains("End of file")
        || message.contains("No such file or directory")
}

#[cfg(test)]
mod tests {
    use super::{
        cover_mime_type, detect_primary_audio_codec, export_audio_for_download, path_to_cstring,
        transcode_audio_to_mp3, InputContext,
    };
    use rusty_ffmpeg::ffi;
    use std::path::{Path, PathBuf};

    #[test]
    fn transcode_audio_to_mp3_reports_open_input_errors() {
        let err = transcode_audio_to_mp3(
            Path::new("/definitely/missing/input.webm"),
            Path::new("/tmp/output.mp3"),
        )
        .unwrap_err();
        assert!(err.contains("failed to open input file"));
    }

    #[test]
    fn cover_codec_ids_map_to_expected_mime_types() {
        assert_eq!(cover_mime_type(ffi::AV_CODEC_ID_MJPEG), "image/jpeg");
        assert_eq!(cover_mime_type(ffi::AV_CODEC_ID_PNG), "image/png");
    }

    #[test]
    fn transcode_sample_webm_to_mp3() {
        assert_sample_converts_to_mp3("music/1.webm");
    }

    #[test]
    fn transcode_sample_m4s_to_mp3() {
        assert_sample_converts_to_mp3("music/1.m4s");
    }

    #[test]
    fn export_sample_webm_to_ogg_without_transcoding() {
        assert_sample_exports_with_codec("music/1.webm", "exported.ogg", ffi::AV_CODEC_ID_OPUS);
    }

    #[test]
    fn export_sample_m4s_to_m4a_without_transcoding() {
        assert_sample_exports_with_codec("music/1.m4s", "exported.m4a", ffi::AV_CODEC_ID_AAC);
    }

    #[test]
    fn export_sample_wav_to_flac_with_lossless_transcoding() {
        let temp_dir = tempfile::tempdir().unwrap();
        let input = temp_dir.path().join("input.wav");
        write_silent_wav(&input, 48_000, 2, 16, 48_000).unwrap();
        assert_export_with_codec(
            &input,
            temp_dir.path(),
            "exported.flac",
            ffi::AV_CODEC_ID_FLAC,
        );
    }

    fn assert_sample_converts_to_mp3(sample_name: &str) {
        let input = repo_root().join(sample_name);
        if !input.exists() {
            eprintln!(
                "skipping sample conversion test, missing {}",
                input.display()
            );
            return;
        }

        let temp_dir = tempfile::tempdir().unwrap();
        let output = temp_dir.path().join("converted.mp3");

        transcode_audio_to_mp3(&input, &output)
            .unwrap_or_else(|err| panic!("failed to transcode sample {}: {err}", input.display()));

        let metadata = std::fs::metadata(&output).unwrap();
        assert!(metadata.len() > 0, "converted output is empty");
        assert_mp3_stream(&output);
    }

    fn assert_mp3_stream(output: &Path) {
        let output = path_to_cstring(output).unwrap();
        unsafe {
            let mut input_ctx = InputContext::open(&output).unwrap();
            let (stream_index, _) = input_ctx.best_audio_stream().unwrap();
            let stream = input_ctx.stream(stream_index as usize).unwrap();
            let codec_id = (*(*stream).codecpar).codec_id;
            assert_eq!(codec_id, ffi::AV_CODEC_ID_MP3);
        }
    }

    fn assert_sample_exports_with_codec(
        sample_name: &str,
        expected_file_name: &str,
        expected_codec: ffi::AVCodecID,
    ) {
        let input = repo_root().join(sample_name);
        if !input.exists() {
            eprintln!("skipping sample export test, missing {}", input.display());
            return;
        }

        let temp_dir = tempfile::tempdir().unwrap();
        assert_export_with_codec(&input, temp_dir.path(), expected_file_name, expected_codec);
    }

    fn assert_export_with_codec(
        input: &Path,
        output_dir: &Path,
        expected_file_name: &str,
        expected_codec: ffi::AVCodecID,
    ) {
        let (file_name, output) = export_audio_for_download(input, output_dir, "exported")
            .unwrap_or_else(|err| panic!("failed to export sample {}: {err}", input.display()));

        assert_eq!(file_name, expected_file_name);
        assert_eq!(
            output.file_name().and_then(|v| v.to_str()),
            Some(expected_file_name)
        );

        let metadata = std::fs::metadata(&output).unwrap();
        assert!(metadata.len() > 0, "exported output is empty");

        let output_codec = detect_primary_audio_codec(&output).unwrap();
        assert_eq!(output_codec, expected_codec);
    }

    fn write_silent_wav(
        path: &Path,
        sample_rate: u32,
        channels: u16,
        bits_per_sample: u16,
        frames: u32,
    ) -> Result<(), std::io::Error> {
        use std::io::Write;

        let bytes_per_sample = u32::from(bits_per_sample / 8);
        let data_size = frames * u32::from(channels) * bytes_per_sample;
        let byte_rate = sample_rate * u32::from(channels) * bytes_per_sample;
        let block_align = channels * (bits_per_sample / 8);
        let riff_size = 36 + data_size;

        let mut file = std::fs::File::create(path)?;
        file.write_all(b"RIFF")?;
        file.write_all(&riff_size.to_le_bytes())?;
        file.write_all(b"WAVE")?;
        file.write_all(b"fmt ")?;
        file.write_all(&16_u32.to_le_bytes())?;
        file.write_all(&1_u16.to_le_bytes())?;
        file.write_all(&channels.to_le_bytes())?;
        file.write_all(&sample_rate.to_le_bytes())?;
        file.write_all(&byte_rate.to_le_bytes())?;
        file.write_all(&block_align.to_le_bytes())?;
        file.write_all(&bits_per_sample.to_le_bytes())?;
        file.write_all(b"data")?;
        file.write_all(&data_size.to_le_bytes())?;

        let silence = vec![0_u8; data_size as usize];
        file.write_all(&silence)?;
        Ok(())
    }

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_path_buf()
    }
}
