#!/usr/bin/env python3

import argparse
import os
import struct


DEFAULT_TARGET_SIZE_MB = 400
DEFAULT_SAMPLE_RATE = 48_000
DEFAULT_CHANNELS = 2
DEFAULT_BITS_PER_SAMPLE = 16
CHUNK_SIZE = 1024 * 1024


def build_wav_header(data_size: int, sample_rate: int, channels: int, bits_per_sample: int) -> bytes:
    byte_rate = sample_rate * channels * bits_per_sample // 8
    block_align = channels * bits_per_sample // 8
    riff_chunk_size = 36 + data_size
    return struct.pack(
        "<4sI4s4sIHHIIHH4sI",
        b"RIFF",
        riff_chunk_size,
        b"WAVE",
        b"fmt ",
        16,
        1,
        channels,
        sample_rate,
        byte_rate,
        block_align,
        bits_per_sample,
        b"data",
        data_size,
    )


def main() -> None:
    parser = argparse.ArgumentParser(description="Generate a silent PCM WAV file filled with zero samples.")
    parser.add_argument("output", help="Output WAV path")
    parser.add_argument("--size-mb", type=int, default=DEFAULT_TARGET_SIZE_MB, help="Approximate total file size in MiB")
    parser.add_argument("--sample-rate", type=int, default=DEFAULT_SAMPLE_RATE, help="PCM sample rate")
    parser.add_argument("--channels", type=int, default=DEFAULT_CHANNELS, help="Channel count")
    parser.add_argument(
        "--bits-per-sample",
        type=int,
        choices=(8, 16, 24, 32),
        default=DEFAULT_BITS_PER_SAMPLE,
        help="PCM bit depth",
    )
    args = parser.parse_args()

    total_size = args.size_mb * 1024 * 1024
    header_size = 44
    if total_size <= header_size:
        raise ValueError("target size must be larger than WAV header size")

    block_align = args.channels * args.bits_per_sample // 8
    data_size = total_size - header_size
    data_size -= data_size % block_align

    zero_chunk = b"\x00" * min(CHUNK_SIZE, data_size)

    with open(args.output, "wb") as handle:
        handle.write(
            build_wav_header(
                data_size=data_size,
                sample_rate=args.sample_rate,
                channels=args.channels,
                bits_per_sample=args.bits_per_sample,
            )
        )

        remaining = data_size
        while remaining > 0:
            chunk = zero_chunk if remaining >= len(zero_chunk) else zero_chunk[:remaining]
            handle.write(chunk)
            remaining -= len(chunk)

    written = os.path.getsize(args.output)
    print(f"wrote {args.output} ({written} bytes)")


if __name__ == "__main__":
    main()
