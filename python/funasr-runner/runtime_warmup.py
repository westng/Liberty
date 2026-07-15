#!/usr/bin/env python3
from __future__ import annotations

import argparse
import os
import shutil
import tarfile
import tempfile
import urllib.request
import sys
from pathlib import Path


def parse_args():
    parser = argparse.ArgumentParser(description="Warm up Liberty managed runtime models")
    parser.add_argument("--models-root", required=True)
    parser.add_argument("--validate-only", action="store_true")
    return parser.parse_args()


def log(message: str):
    sys.stdout.write(f"{message}\n")
    sys.stdout.flush()


def download(url: str, destination: Path):
    destination.parent.mkdir(parents=True, exist_ok=True)
    request = urllib.request.Request(
        url,
        headers={"User-Agent": "Liberty Runtime Warmup/1.0"},
    )
    with urllib.request.urlopen(request, timeout=120) as response, destination.open("wb") as output:
        shutil.copyfileobj(response, output, length=1024 * 1024)


def warmup_sherpa_onnx_models(models_root: Path, validate_only: bool = False):
    model_root = models_root / "sherpa-onnx"
    marker_path = model_root / "tokens.txt"
    if marker_path.is_file():
        log("Sherpa-ONNX model validation passed." if validate_only else "Sherpa-ONNX models already exist.")
        return
    if validate_only:
        raise RuntimeError("Sherpa-ONNX 模型文件不完整。")

    url = os.getenv(
        "SHERPA_ONNX_MODEL_URL",
        "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/"
        "sherpa-onnx-paraformer-zh-small-2024-03-09.tar.bz2",
    )

    with tempfile.TemporaryDirectory(prefix="liberty-sherpa-model-") as temp_dir_raw:
        temp_dir = Path(temp_dir_raw)
        archive_path = temp_dir / "model.tar.bz2"
        extract_root = temp_dir / "extract"

        log("Downloading default Sherpa-ONNX model...")
        download(url, archive_path)

        log("Extracting default Sherpa-ONNX model...")
        extract_root.mkdir(parents=True, exist_ok=True)
        with tarfile.open(archive_path, "r:bz2") as archive:
            archive.extractall(extract_root)

        candidates = [
            path
            for path in extract_root.rglob("*")
            if path.is_dir() and (path / "tokens.txt").is_file()
        ]
        if not candidates:
            raise RuntimeError("未在 Sherpa-ONNX 模型包中找到 tokens.txt。")

        if model_root.exists():
            shutil.rmtree(model_root)
        shutil.copytree(candidates[0], model_root)

    log("Sherpa-ONNX model warmup completed.")


def resolve_model_profile() -> str:
    profile = str(os.getenv("FUNASR_PROFILE", "") or "").strip().lower()
    if profile in {"sensevoice", "sensevoice-small"}:
        return "sensevoice"
    model_name = str(os.getenv("FUNASR_MODEL", "") or "").strip().lower()
    if "sensevoice" in model_name:
        return "sensevoice"
    return "paraformer"


def main():
    args = parse_args()
    models_root = Path(args.models_root)
    models_root.mkdir(parents=True, exist_ok=True)

    backend = os.getenv("LIBERTY_ASR_BACKEND", "funasr").strip().lower() or "funasr"
    if backend == "sherpa-onnx":
        warmup_sherpa_onnx_models(models_root, args.validate_only)
        return

    os.environ.setdefault("MODELSCOPE_CACHE", str(models_root / "modelscope"))
    os.environ.setdefault("HF_HOME", str(models_root / "huggingface"))
    os.environ.setdefault("TORCH_HOME", str(models_root / "torch"))
    if args.validate_only:
        os.environ["MODELSCOPE_OFFLINE"] = "1"
        os.environ["HF_HUB_OFFLINE"] = "1"
        os.environ["TRANSFORMERS_OFFLINE"] = "1"

    log("Validating FunASR models offline..." if args.validate_only else "Importing FunASR runtime...")
    from funasr import AutoModel

    profile = resolve_model_profile()
    default_model = "iic/SenseVoiceSmall" if profile == "sensevoice" else "paraformer-zh"
    common_kwargs = {
        "model": os.getenv("FUNASR_MODEL", default_model),
        "vad_model": os.getenv("FUNASR_VAD_MODEL", "fsmn-vad"),
        "device": "cpu",
        "disable_update": True,
    }
    if profile != "sensevoice":
        common_kwargs["punc_model"] = os.getenv("FUNASR_PUNC_MODEL", "ct-punc")

    log(
        f"Validating cached FunASR models for profile: {profile}"
        if args.validate_only
        else f"Downloading default FunASR models for profile: {profile}"
    )
    AutoModel(**common_kwargs)

    log("Validating cached speaker model..." if args.validate_only else "Downloading default speaker model...")
    AutoModel(
        **common_kwargs,
        spk_model=os.getenv("FUNASR_SPK_MODEL", "cam++"),
    )

    log("Managed model validation completed." if args.validate_only else "Managed runtime warmup completed.")


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        sys.stderr.write(f"{error}\n")
        sys.exit(1)
