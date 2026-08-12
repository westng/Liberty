from __future__ import annotations

import importlib.util
from pathlib import Path

import pytest


@pytest.fixture(scope="session")
def repository_root() -> Path:
    return Path(__file__).resolve().parents[3]


@pytest.fixture(scope="session")
def runner_module(repository_root: Path):
    runner_path = repository_root / "python" / "funasr-runner" / "runner.py"
    spec = importlib.util.spec_from_file_location("liberty_funasr_runner", runner_path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"Unable to load Runner module from {runner_path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module
