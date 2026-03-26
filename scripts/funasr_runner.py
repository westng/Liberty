#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import os
import sys
import time
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
):
    write_json(
        job_dir / "progress.json",
        {
            "stage": stage,
            "statusMessage": status_message,
            "failureReason": failure_reason,
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
    model_kwargs = {
        "model": os.getenv("FUNASR_MODEL", "paraformer-zh"),
        "vad_model": os.getenv("FUNASR_VAD_MODEL", "fsmn-vad"),
        "punc_model": os.getenv("FUNASR_PUNC_MODEL", "ct-punc"),
    }
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


def main():
    args = parse_args()
    job_dir = Path(args.job_dir)
    input_path = Path(args.input)
    wants_speaker = args.speaker.lower() == "true"

    if not input_path.exists():
        write_progress(job_dir, "failed", "输入文件不存在。", "输入文件不存在。")
        raise FileNotFoundError(f"Input file not found: {input_path}")

    write_progress(job_dir, "transcribing", "正在加载本地 FunASR 模型。")

    try:
        from funasr import AutoModel
    except Exception as error:
        write_progress(job_dir, "failed", "导入 funasr 失败。", str(error))
        raise

    model, resolved_device = build_model(AutoModel, wants_speaker)
    batch_size_s = parse_positive_int(os.getenv("FUNASR_BATCH_SIZE_S"), 300)
    log(
        "FunASR runtime:"
        f" device={resolved_device}"
        f", batch_size_s={batch_size_s}"
        f", threads={os.getenv('OMP_NUM_THREADS', '1')}"
        f", speaker={'true' if wants_speaker else 'false'}"
    )

    write_progress(job_dir, "transcribing", "正在进行本地转写。")
    result = model.generate(
        input=str(input_path),
        batch_size_s=batch_size_s,
        hotword=args.hotwords or None,
    )

    payload = result[0] if isinstance(result, list) and result else result
    if not isinstance(payload, dict):
        raise RuntimeError("FunASR 返回结果格式不可识别。")

    if wants_speaker:
        write_progress(job_dir, "speaker_processing", "正在整理说话人结果。")

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
    write_progress(job_dir, "completed", "本地处理已完成。")


if __name__ == "__main__":
    current_job_dir = None
    try:
        if "--job-dir" in sys.argv:
            current_job_dir = Path(sys.argv[sys.argv.index("--job-dir") + 1])
        main()
    except Exception as error:
        if current_job_dir is not None:
            write_progress(current_job_dir, "failed", str(error), str(error))
        sys.stderr.write(f"{error}\n")
        sys.exit(1)
