#!/usr/bin/env python3
from __future__ import annotations

import argparse
import os
import sys
from pathlib import Path


def parse_args():
    parser = argparse.ArgumentParser(description="Warm up Liberty managed runtime models")
    parser.add_argument("--models-root", required=True)
    return parser.parse_args()


def log(message: str):
    sys.stdout.write(f"{message}\n")
    sys.stdout.flush()


def main():
    args = parse_args()
    models_root = Path(args.models_root)
    models_root.mkdir(parents=True, exist_ok=True)

    os.environ.setdefault("MODELSCOPE_CACHE", str(models_root / "modelscope"))
    os.environ.setdefault("HF_HOME", str(models_root / "huggingface"))
    os.environ.setdefault("TORCH_HOME", str(models_root / "torch"))

    log("Importing FunASR runtime...")
    from funasr import AutoModel

    common_kwargs = {
        "model": os.getenv("FUNASR_MODEL", "paraformer-zh"),
        "vad_model": os.getenv("FUNASR_VAD_MODEL", "fsmn-vad"),
        "punc_model": os.getenv("FUNASR_PUNC_MODEL", "ct-punc"),
        "device": "cpu",
        "disable_update": True,
    }

    log("Downloading default ASR/VAD/PUNC models...")
    AutoModel(**common_kwargs)

    log("Downloading default speaker model...")
    AutoModel(
        **common_kwargs,
        spk_model=os.getenv("FUNASR_SPK_MODEL", "cam++"),
    )

    log("Managed runtime warmup completed.")


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        sys.stderr.write(f"{error}\n")
        sys.exit(1)
