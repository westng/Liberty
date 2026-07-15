#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import time
import traceback
from pathlib import Path
from typing import Optional

os.environ.setdefault("OMP_NUM_THREADS", "1")
os.environ.setdefault("MKL_NUM_THREADS", os.environ["OMP_NUM_THREADS"])
os.environ.setdefault("KMP_DUPLICATE_LIB_OK", "TRUE")


def parse_args():
    parser = argparse.ArgumentParser(description="Local FunASR runner for Liberty")
    parser.add_argument("--job-dir", required=True)
    parser.add_argument("--input", required=True)
    parser.add_argument("--lang", required=True)
    parser.add_argument("--speaker", required=True)
    parser.add_argument("--hotwords", default="")
    return parser.parse_args()


def write_json(path: Path, payload: dict):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")


def log(message: str):
    sys.stdout.write(f"{message}\n")
    sys.stdout.flush()


def write_progress(
    job_dir: Path,
    stage: str,
    status_message: str,
    failure_reason: Optional[str] = None,
    progress_percent: Optional[int] = None,
):
    write_json(
        job_dir / "progress.json",
        {
            "stage": stage,
            "statusMessage": status_message,
            "failureReason": failure_reason,
            "progressPercent": progress_percent,
            "updatedAt": str(time.time_ns()),
        },
    )


def normalize_timestamp(value):
    if isinstance(value, (int, float)):
        return int(value)
    return 0


def normalize_sentence_items(raw: dict) -> list[dict]:
    for key in ("sentence_info", "sentenceInfo", "sentence_info_list", "sentenceInfoList"):
        value = raw.get(key)
        if isinstance(value, list):
            return value
    return []


def parse_positive_int(value: str | None, default: int) -> int:
    try:
        parsed = int(str(value or "").strip())
    except (TypeError, ValueError):
        return default
    return parsed if parsed > 0 else default


def resolve_model_profile() -> str:
    profile = str(os.getenv("FUNASR_PROFILE", "") or "").strip().lower()
    if profile in {"sensevoice", "sensevoice-small"}:
        return "sensevoice"
    model_name = str(os.getenv("FUNASR_MODEL", "") or "").strip().lower()
    if "sensevoice" in model_name:
        return "sensevoice"
    return "paraformer"


def resolve_requested_device() -> str:
    requested = str(os.getenv("FUNASR_DEVICE", "auto") or "auto").strip().lower()
    if requested in {"cpu", "mps", "cuda"}:
        return requested
    return "auto"


def detect_best_device() -> str:
    try:
        import torch
    except Exception:
        return "cpu"

    try:
        if torch.cuda.is_available():
            return "cuda:0"
    except Exception:
        pass

    try:
        if getattr(torch.backends, "mps", None) and torch.backends.mps.is_available():
            return "mps"
    except Exception:
        pass

    return "cpu"


def build_model(auto_model_cls, wants_speaker: bool) -> tuple[object, str]:
    profile = resolve_model_profile()
    default_model = "iic/SenseVoiceSmall" if profile == "sensevoice" else "paraformer-zh"
    model_kwargs = {
        "model": os.getenv("FUNASR_MODEL", default_model),
        "vad_model": os.getenv("FUNASR_VAD_MODEL", "fsmn-vad"),
        "disable_update": True,
    }
    if profile != "sensevoice":
        model_kwargs["punc_model"] = os.getenv("FUNASR_PUNC_MODEL", "ct-punc")
    if wants_speaker:
        model_kwargs["spk_model"] = os.getenv("FUNASR_SPK_MODEL", "cam++")

    requested_device = resolve_requested_device()
    base_device = detect_best_device() if requested_device == "auto" else requested_device
    device_candidates: list[str | None] = []

    for candidate in [base_device, "cpu", None]:
        if candidate not in device_candidates:
            device_candidates.append(candidate)

    last_error: Exception | None = None
    for candidate in device_candidates:
        try:
            kwargs = dict(model_kwargs)
            if candidate is not None:
                kwargs["device"] = "cuda:0" if candidate == "cuda" else candidate
            model = auto_model_cls(**kwargs)
            return model, candidate or "default"
        except Exception as error:
            last_error = error

    if last_error is not None:
        raise last_error

    raise RuntimeError("FunASR 模型初始化失败。")


def resolve_ffmpeg_binary() -> Optional[str]:
    configured = str(os.getenv("LIBERTY_FFMPEG_PATH", "") or "").strip()
    if configured and Path(configured).exists():
        return configured

    return shutil.which("ffmpeg")


def prepare_input_media(job_dir: Path, input_path: Path) -> Path:
    force_convert = os.getenv("LIBERTY_FORCE_AUDIO_CONVERT", "").strip().lower() == "true"
    suffix = input_path.suffix.lower()
    if suffix == ".wav" and not force_convert:
        return input_path

    ffmpeg_binary = resolve_ffmpeg_binary()
    if not ffmpeg_binary:
        raise RuntimeError("当前文件需要 ffmpeg 进行音频解码，但本地运行环境中未找到 ffmpeg。")

    prepared_path = job_dir / "prepared-input.wav"
    if prepared_path.exists():
        prepared_path.unlink()

    log(f"Preparing media via ffmpeg: {input_path.name} -> {prepared_path.name}")
    subprocess.run(
        [
            ffmpeg_binary,
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-i",
            str(input_path),
            "-vn",
            "-ac",
            "1",
            "-ar",
            "16000",
            str(prepared_path),
        ],
        check=True,
    )

    if not prepared_path.exists():
        raise RuntimeError("ffmpeg 已执行，但未生成可用的 wav 文件。")

    return prepared_path


def extract_segments(result: dict, with_speaker: bool) -> tuple[list[dict], list[dict]]:
    transcript_segments: list[dict] = []
    speaker_segments: list[dict] = []
    sentence_items = normalize_sentence_items(result)

    for index, item in enumerate(sentence_items, start=1):
        text = str(item.get("text") or item.get("sentence") or item.get("content") or "").strip()
        if not text:
            continue

        start_ms = normalize_timestamp(item.get("start") or item.get("start_ms") or item.get("begin"))
        end_ms = normalize_timestamp(item.get("end") or item.get("end_ms") or item.get("stop"))
        if isinstance(item.get("timestamp"), list) and item["timestamp"]:
            first = item["timestamp"][0]
            last = item["timestamp"][-1]
            if isinstance(first, (list, tuple)) and len(first) >= 2:
                start_ms = normalize_timestamp(first[0])
            if isinstance(last, (list, tuple)) and len(last) >= 2:
                end_ms = normalize_timestamp(last[1])

        base_segment = {
            "id": f"segment-{index}",
            "startMs": start_ms,
            "endMs": end_ms,
            "text": text,
        }
        transcript_segments.append(base_segment)

        speaker = item.get("speaker") or item.get("spk") or item.get("speaker_id")
        if speaker is not None:
            speaker_segments.append({**base_segment, "speaker": str(speaker)})
        elif with_speaker:
            speaker_segments.append({**base_segment, "speaker": "Speaker 1"})

    if transcript_segments:
        if not speaker_segments and with_speaker:
            speaker_segments = [{**segment, "speaker": "Speaker 1"} for segment in transcript_segments]
        return transcript_segments, speaker_segments

    full_text = str(result.get("text") or "").strip()
    if not full_text:
        return [], []

    fallback_segment = {
        "id": "segment-1",
        "startMs": 0,
        "endMs": 0,
        "text": full_text,
    }
    transcript_segments = [fallback_segment]
    if with_speaker:
        speaker_segments = [{**fallback_segment, "speaker": "Speaker 1"}]
    return transcript_segments, speaker_segments


def resolve_sherpa_model_root() -> Path:
    configured = str(os.getenv("SHERPA_ONNX_MODEL_DIR", "") or "").strip()
    if configured and Path(configured).is_dir():
        return Path(configured)

    for env_name in ("MODELSCOPE_CACHE", "HF_HOME", "TORCH_HOME"):
        value = str(os.getenv(env_name, "") or "").strip()
        if not value:
            continue
        root = Path(value)
        models_root = root.parent if root.name in {"modelscope", "huggingface", "torch"} else root
        candidate = models_root / "sherpa-onnx"
        if candidate.is_dir():
            return candidate

    raise RuntimeError("未找到 Sherpa-ONNX 模型目录，请重新安装内置本地运行环境。")


def find_required_file(root: Path, names: tuple[str, ...]) -> Path:
    for name in names:
        direct = root / name
        if direct.is_file():
            return direct
    for name in names:
        matches = sorted(root.rglob(name))
        if matches:
            return matches[0]
    raise RuntimeError(f"模型目录中缺少必要文件：{', '.join(names)}")


def build_sherpa_recognizer(model_root: Path, lang: str):
    import sherpa_onnx

    model_path = find_required_file(
        model_root,
        ("model.int8.onnx", "model.onnx", "encoder.int8.onnx", "encoder.onnx"),
    )
    tokens_path = find_required_file(model_root, ("tokens.txt",))
    num_threads = parse_positive_int(os.getenv("OMP_NUM_THREADS"), 1)

    recognizer = sherpa_onnx.OfflineRecognizer.from_paraformer(
        paraformer=str(model_path),
        tokens=str(tokens_path),
        num_threads=max(1, num_threads),
        sample_rate=16000,
        feature_dim=80,
        decoding_method="greedy_search",
        debug=False,
        provider="cpu",
    )
    log(
        "Sherpa-ONNX runtime:"
        f" model={model_path.name}"
        f", lang={lang}"
        f", threads={num_threads}"
    )
    return recognizer


def read_wav_mono_16k(path: Path):
    import wave
    import numpy as np

    with wave.open(str(path), "rb") as wav_file:
        channels = wav_file.getnchannels()
        sample_width = wav_file.getsampwidth()
        sample_rate = wav_file.getframerate()
        frames = wav_file.readframes(wav_file.getnframes())

    if channels != 1 or sample_width != 2 or sample_rate != 16000:
        raise RuntimeError("Sherpa-ONNX 输入必须是 16kHz 单声道 16-bit wav。")

    samples = np.frombuffer(frames, dtype=np.int16).astype(np.float32) / 32768.0
    return samples, sample_rate


def run_sherpa_onnx(args, job_dir: Path, input_path: Path, wants_speaker: bool):
    write_progress(job_dir, "transcribing", "正在准备音频文件。", progress_percent=8)
    os.environ["LIBERTY_FORCE_AUDIO_CONVERT"] = "true"
    prepared_input = prepare_input_media(job_dir, input_path)

    write_progress(job_dir, "transcribing", "正在加载本地 Sherpa-ONNX 模型。", progress_percent=18)
    model_root = resolve_sherpa_model_root()
    recognizer = build_sherpa_recognizer(model_root, args.lang)

    write_progress(job_dir, "transcribing", "正在进行本地转写。", progress_percent=32)
    samples, sample_rate = read_wav_mono_16k(prepared_input)
    stream = recognizer.create_stream()
    stream.accept_waveform(sample_rate, samples)
    recognizer.decode_stream(stream)
    text = str(stream.result.text or "").strip()
    if not text:
        raise RuntimeError("Sherpa-ONNX 未返回可用逐字稿内容。")

    duration_ms = int(len(samples) / sample_rate * 1000) if sample_rate > 0 else 0
    transcript_segments = [
        {
            "id": "segment-1",
            "startMs": 0,
            "endMs": duration_ms,
            "text": text,
        }
    ]
    speaker_segments = (
        [{**transcript_segments[0], "speaker": "Speaker 1"}] if wants_speaker else []
    )

    if wants_speaker:
        write_progress(job_dir, "speaker_processing", "32 位离线模式暂不做真实说话人分离，已生成默认说话人。", progress_percent=94)

    write_json(
        job_dir / "result.json",
        {
            "durationMinutes": max(1, int((duration_ms + 59999) / 60000)) if duration_ms > 0 else 0,
            "transcriptSegments": transcript_segments,
            "speakerSegments": speaker_segments,
            "failureReason": None,
        },
    )
    write_progress(job_dir, "completed", "本地处理已完成。", progress_percent=100)


def main():
    args = parse_args()
    job_dir = Path(args.job_dir)
    input_path = Path(args.input)
    wants_speaker = args.speaker.lower() == "true"

    if not input_path.exists():
        write_progress(job_dir, "failed", "输入文件不存在。", "输入文件不存在。", 0)
        raise FileNotFoundError(f"Input file not found: {input_path}")

    backend = os.getenv("LIBERTY_ASR_BACKEND", "funasr").strip().lower() or "funasr"
    if backend == "sherpa-onnx":
        run_sherpa_onnx(args, job_dir, input_path, wants_speaker)
        return

    write_progress(job_dir, "transcribing", "正在准备音频文件。", progress_percent=8)
    prepared_input = prepare_input_media(job_dir, input_path)

    write_progress(job_dir, "transcribing", "正在加载本地 FunASR 模型。", progress_percent=18)

    try:
        from funasr import AutoModel
    except Exception as error:
        write_progress(job_dir, "failed", "导入 funasr 失败。", str(error), 18)
        raise

    model, resolved_device = build_model(AutoModel, wants_speaker)
    batch_size_s = parse_positive_int(os.getenv("FUNASR_BATCH_SIZE_S"), 300)
    log(
        "FunASR runtime:"
        f" profile={resolve_model_profile()}"
        f", device={resolved_device}"
        f", batch_size_s={batch_size_s}"
        f", threads={os.getenv('OMP_NUM_THREADS', '1')}"
        f", speaker={'true' if wants_speaker else 'false'}"
    )

    write_progress(job_dir, "transcribing", "正在进行本地转写。", progress_percent=32)
    result = model.generate(
        input=str(prepared_input),
        batch_size_s=batch_size_s,
        hotword=args.hotwords or None,
    )

    payload = result[0] if isinstance(result, list) and result else result
    if not isinstance(payload, dict):
        raise RuntimeError("FunASR 返回结果格式不可识别。")

    if wants_speaker:
        write_progress(job_dir, "speaker_processing", "正在整理说话人结果。", progress_percent=94)

    transcript_segments, speaker_segments = extract_segments(payload, wants_speaker)
    if not transcript_segments:
        raise RuntimeError("FunASR 未返回可用逐字稿内容。")

    write_json(
        job_dir / "result.json",
        {
            "durationMinutes": 0,
            "transcriptSegments": transcript_segments,
            "speakerSegments": speaker_segments,
            "failureReason": None,
        },
    )
    write_progress(job_dir, "completed", "本地处理已完成。", progress_percent=100)


if __name__ == "__main__":
    current_job_dir = None
    try:
        if "--job-dir" in sys.argv:
            current_job_dir = Path(sys.argv[sys.argv.index("--job-dir") + 1])
        main()
    except Exception as error:
        failure_message = f"{error.__class__.__name__}: {error}"
        if current_job_dir is not None:
            write_progress(current_job_dir, "failed", failure_message, failure_message, 0)
        traceback.print_exc()
        sys.exit(1)
