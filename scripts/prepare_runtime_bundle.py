#!/usr/bin/env python3
from __future__ import annotations

import argparse
import gzip
import os
import shutil
import ssl
import subprocess
import sys
import tarfile
import tempfile
import time
import urllib.request
import zipfile
from pathlib import Path

WINDOWS_NO_WINDOW = 0x08000000


ROOT = Path(__file__).resolve().parents[1]
RUNNER_ROOT = ROOT / "python" / "funasr-runner"
REQUIREMENTS_PATH = RUNNER_ROOT / "requirements.txt"
VALIDATE_PATH = RUNNER_ROOT / "runtime_validate.py"

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
    "windows-x86": {
        "python_url": "https://github.com/astral-sh/python-build-standalone/releases/download/20251031/cpython-3.9.25+20251031-i686-pc-windows-msvc-install_only.tar.gz",
        "python_candidates": ["python/python.exe", "python/python3.exe"],
        "ffmpeg_url": "https://github.com/sudo-nautilus/FFmpeg-Builds-Win32/releases/download/latest/ffmpeg-master-latest-win32-gpl.zip",
        "ffmpeg_mode": "zip-bin-dir",
        "ffmpeg_executable": "ffmpeg.exe",
        "asr_backend": "sherpa-onnx",
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
    kwargs = {"check": True, "env": merged_env}
    if os.name == "nt":
        kwargs["creationflags"] = WINDOWS_NO_WINDOW
    subprocess.run(cmd, **kwargs)


def can_execute_binary(path: Path) -> bool:
    suffix = path.suffix.lower()
    if os.name != "nt" and suffix in {".exe", ".bat", ".cmd"}:
        return False
    return os.name == "nt" or os.access(path, os.X_OK)


def download_with_curl(url: str, destination: Path) -> None:
    curl_path = shutil.which("curl")
    if curl_path is None:
        raise FileNotFoundError("curl is not available")

    run(
        [
            curl_path,
            "--fail",
            "--location",
            "--silent",
            "--show-error",
            "--retry",
            "3",
            "--retry-all-errors",
            "--output",
            str(destination),
            url,
        ]
    )


def download(url: str, destination: Path, retries: int = 3) -> None:
    destination.parent.mkdir(parents=True, exist_ok=True)
    temp_destination = destination.with_suffix(destination.suffix + ".part")
    if temp_destination.exists():
        temp_destination.unlink()

    last_error: Exception | None = None
    for attempt in range(1, retries + 1):
        try:
            log(f"downloading {url} (attempt {attempt}/{retries})")
            request = urllib.request.Request(
                url,
                headers={"User-Agent": "Liberty Runtime Bundler/1.0"},
            )
            with urllib.request.urlopen(request, timeout=120) as response, temp_destination.open("wb") as output:
                shutil.copyfileobj(response, output, length=1024 * 1024)
            temp_destination.replace(destination)
            return
        except Exception as error:
            last_error = error
            if temp_destination.exists():
                temp_destination.unlink()
            log(f"download failed: {error}")
            if attempt < retries:
                time.sleep(min(attempt * 2, 5))

    should_try_curl = isinstance(last_error, ssl.SSLError) or "SSL:" in str(last_error)
    if should_try_curl:
        log("falling back to curl after SSL download failure")
        download_with_curl(url, temp_destination)
        temp_destination.replace(destination)
        return

    raise last_error if last_error is not None else RuntimeError(f"Unable to download {url}")


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
    config = PLATFORM_CONFIGS[platform_id]
    backend = config.get("asr_backend", "funasr")
    if not can_execute_binary(python_executable):
        log(
            f"skipping dependency installation because {python_executable.name} "
            "cannot run on this build host"
        )
        return

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
    if backend == "sherpa-onnx":
        run(
            [
                str(python_executable),
                "-m",
                "pip",
                "install",
                "--only-binary=:all:",
                "--prefer-binary",
                "--retries",
                "2",
                "--timeout",
                "120",
                "numpy<2",
                "sherpa-onnx==1.13.1",
            ],
            env=env,
        )
    else:
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
        run(
            [
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
                "numpy<2",
            ],
            env=env,
        )


def validate_python_runtime(python_executable: Path, platform_id: str) -> None:
    if not can_execute_binary(python_executable):
        log(
            f"skipping runtime validation because {python_executable.name} "
            "cannot run on this build host"
        )
        return

    run(
        [str(python_executable), str(VALIDATE_PATH)],
        env={
            "PYTHONUTF8": "1",
            "LIBERTY_ASR_BACKEND": PLATFORM_CONFIGS[platform_id].get("asr_backend", "funasr"),
        },
    )


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

    unsupported_reason = config.get("unsupported_reason")
    if unsupported_reason:
        log(f"managed runtime unavailable for {platform_id}: {unsupported_reason}")
        (output_dir / "unsupported-runtime.txt").write_text(unsupported_reason + "\n", encoding="utf-8")
        log(f"runtime bundle placeholder ready at {output_dir}")
        return

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
        validate_python_runtime(python_executable, platform_id)
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
