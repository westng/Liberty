#!/usr/bin/env python3
from __future__ import annotations

import argparse
import gzip
import os
import shutil
import subprocess
import sys
import tarfile
import tempfile
import urllib.request
import zipfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
REQUIREMENTS_PATH = ROOT / "scripts" / "runtime_requirements.txt"
VALIDATE_PATH = ROOT / "scripts" / "runtime_validate.py"

PYTHON_BUNDLE_NAME = "python-runtime.tar.gz"
FFMPEG_BUNDLE_NAME = "ffmpeg-runtime.tar.gz"
PLATFORM_CONFIGS = {
    "darwin-aarch64": {
        "python_url": "https://github.com/astral-sh/python-build-standalone/releases/download/20251031/cpython-3.9.25+20251031-aarch64-apple-darwin-install_only.tar.gz",
        "python_candidates": ["python/bin/python3", "python/bin/python"],
        "ffmpeg_url": "https://github.com/eugeneware/ffmpeg-static/releases/download/b6.1.1/ffmpeg-darwin-arm64.gz",
        "ffmpeg_mode": "single-gz",
        "ffmpeg_executable": "ffmpeg",
    },
    "darwin-x64": {
        "python_url": "https://github.com/astral-sh/python-build-standalone/releases/download/20251031/cpython-3.9.25+20251031-x86_64-apple-darwin-install_only.tar.gz",
        "python_candidates": ["python/bin/python3", "python/bin/python"],
        "ffmpeg_url": "https://github.com/eugeneware/ffmpeg-static/releases/download/b6.1.1/ffmpeg-darwin-x64.gz",
        "ffmpeg_mode": "single-gz",
        "ffmpeg_executable": "ffmpeg",
    },
    "windows-x64": {
        "python_url": "https://github.com/astral-sh/python-build-standalone/releases/download/20251031/cpython-3.9.25+20251031-x86_64-pc-windows-msvc-install_only.tar.gz",
        "python_candidates": ["python/python.exe", "python/python3.exe"],
        "ffmpeg_url": "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-gpl.zip",
        "ffmpeg_mode": "zip-bin-dir",
        "ffmpeg_executable": "ffmpeg.exe",
    },
}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Prepare bundled Liberty runtime resources")
    parser.add_argument("--platform-id", required=True)
    parser.add_argument("--output-dir", required=True)
    return parser.parse_args()


def log(message: str) -> None:
    print(f"[prepare-runtime] {message}", flush=True)


def run(cmd: list[str], env: dict[str, str] | None = None) -> None:
    log("running: " + " ".join(str(part) for part in cmd))
    merged_env = os.environ.copy()
    if env:
        merged_env.update(env)
    subprocess.run(cmd, check=True, env=merged_env)


def download(url: str, destination: Path) -> None:
    destination.parent.mkdir(parents=True, exist_ok=True)
    log(f"downloading {url}")
    request = urllib.request.Request(
        url,
        headers={"User-Agent": "Liberty Runtime Bundler/1.0"},
    )
    with urllib.request.urlopen(request) as response, destination.open("wb") as output:
        shutil.copyfileobj(response, output, length=1024 * 1024)


def extract_tar_gz(archive_path: Path, destination: Path) -> None:
    destination.mkdir(parents=True, exist_ok=True)
    with tarfile.open(archive_path, "r:gz") as archive:
        archive.extractall(destination)


def extract_single_gzip(archive_path: Path, output_path: Path) -> None:
    output_path.parent.mkdir(parents=True, exist_ok=True)
    with gzip.open(archive_path, "rb") as source, output_path.open("wb") as target:
        shutil.copyfileobj(source, target, length=1024 * 1024)
    try:
        output_path.chmod(0o755)
    except OSError:
        pass


def resolve_python_executable(root: Path, candidates: list[str]) -> Path:
    for candidate in candidates:
        path = root / candidate
        if path.is_file():
            return path
    raise FileNotFoundError(f"Unable to locate Python executable under {root}")


def install_python_runtime(python_executable: Path, platform_id: str) -> None:
    env = {
        "PYTHONUTF8": "1",
        "PIP_DISABLE_PIP_VERSION_CHECK": "1",
    }
    run([str(python_executable), "-m", "ensurepip", "--upgrade"], env=env)
    run(
        [
            str(python_executable),
            "-m",
            "pip",
            "install",
            "--upgrade",
            "pip",
            "setuptools",
            "wheel",
            "--retries",
            "2",
            "--timeout",
            "120",
        ],
        env=env,
    )
    run(
        [
            str(python_executable),
            "-m",
            "pip",
            "install",
            "--prefer-binary",
            "--retries",
            "2",
            "--timeout",
            "120",
            "-r",
            str(REQUIREMENTS_PATH),
        ],
        env=env,
    )

    torch_cmd = [
        str(python_executable),
        "-m",
        "pip",
        "install",
        "--force-reinstall",
        "--no-cache-dir",
        "--prefer-binary",
        "--retries",
        "2",
        "--timeout",
        "120",
        "-c",
        str(REQUIREMENTS_PATH),
        "numpy<2",
        "torch==2.2.2",
        "torchvision==0.17.2",
        "torchaudio==2.2.2",
    ]
    if platform_id == "windows-x64":
        torch_cmd.extend(["--index-url", "https://download.pytorch.org/whl/cpu"])
    run(torch_cmd, env=env)


def validate_python_runtime(python_executable: Path) -> None:
    run([str(python_executable), str(VALIDATE_PATH)], env={"PYTHONUTF8": "1"})


def prepare_ffmpeg(config: dict[str, str], downloads_dir: Path, runtime_root: Path) -> None:
    ffmpeg_archive = downloads_dir / Path(config["ffmpeg_url"]).name
    ffmpeg_dir = runtime_root / "ffmpeg"
    ffmpeg_dir.mkdir(parents=True, exist_ok=True)
    download(config["ffmpeg_url"], ffmpeg_archive)

    if config["ffmpeg_mode"] == "single-gz":
        extract_single_gzip(ffmpeg_archive, ffmpeg_dir / config["ffmpeg_executable"])
        return

    if config["ffmpeg_mode"] == "zip-bin-dir":
        extract_root = downloads_dir / "ffmpeg-extract"
        extract_root.mkdir(parents=True, exist_ok=True)
        with zipfile.ZipFile(ffmpeg_archive) as archive:
            archive.extractall(extract_root)

        ffmpeg_executable = None
        for candidate in extract_root.rglob(config["ffmpeg_executable"]):
            ffmpeg_executable = candidate
            break

        if ffmpeg_executable is None:
            raise FileNotFoundError("Unable to locate ffmpeg.exe inside downloaded archive")

        for child in ffmpeg_executable.parent.iterdir():
            target = ffmpeg_dir / child.name
            if child.is_dir():
                shutil.copytree(child, target, dirs_exist_ok=True)
            else:
                shutil.copy2(child, target)
        return

    raise RuntimeError(f"Unsupported ffmpeg extraction mode: {config['ffmpeg_mode']}")


def create_tar_gz(source_dir: Path, archive_path: Path) -> None:
    archive_path.parent.mkdir(parents=True, exist_ok=True)
    if archive_path.exists():
        archive_path.unlink()
    with tarfile.open(archive_path, "w:gz") as archive:
        archive.add(source_dir, arcname=source_dir.name)


def main() -> None:
    args = parse_args()
    platform_id = args.platform_id
    output_dir = Path(args.output_dir).resolve()

    if platform_id not in PLATFORM_CONFIGS:
        raise SystemExit(f"Unsupported platform id: {platform_id}")

    config = PLATFORM_CONFIGS[platform_id]
    if output_dir.exists():
        shutil.rmtree(output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)

    with tempfile.TemporaryDirectory(prefix=f"liberty-runtime-{platform_id}-") as temp_dir_raw:
        temp_dir = Path(temp_dir_raw)
        downloads_dir = temp_dir / "downloads"
        runtime_root = temp_dir / "runtime"
        downloads_dir.mkdir(parents=True, exist_ok=True)
        runtime_root.mkdir(parents=True, exist_ok=True)

        python_archive = downloads_dir / Path(config["python_url"]).name
        download(config["python_url"], python_archive)
        extract_tar_gz(python_archive, runtime_root)

        python_executable = resolve_python_executable(runtime_root, config["python_candidates"])
        install_python_runtime(python_executable, platform_id)
        validate_python_runtime(python_executable)
        prepare_ffmpeg(config, downloads_dir, runtime_root)

        create_tar_gz(runtime_root / "python", output_dir / PYTHON_BUNDLE_NAME)
        create_tar_gz(runtime_root / "ffmpeg", output_dir / FFMPEG_BUNDLE_NAME)

    log(f"runtime bundle ready at {output_dir}")


if __name__ == "__main__":
    try:
        main()
    except subprocess.CalledProcessError as error:
        sys.stderr.write(f"{error}\n")
        sys.exit(error.returncode or 1)
    except Exception as error:
        sys.stderr.write(f"{error}\n")
        sys.exit(1)
