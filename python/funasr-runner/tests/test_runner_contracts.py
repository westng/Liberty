from __future__ import annotations

import json
from pathlib import Path

import pytest
from jsonschema import Draft202012Validator
from jsonschema.exceptions import ValidationError


def load_json(path: Path):
    return json.loads(path.read_text(encoding="utf-8"))


@pytest.mark.parametrize(
    ("schema_name", "fixture_name"),
    [
        ("result", "result-completed"),
        ("result", "result-unavailable"),
        ("progress", "progress-transcribing"),
        ("event", "event-progress"),
    ],
)
def test_runner_v2_fixtures_match_shared_schema(
    repository_root: Path,
    schema_name: str,
    fixture_name: str,
):
    schema_root = repository_root / "packages" / "shared-types" / "schemas" / "runner" / "v2"
    fixture_root = repository_root / "packages" / "shared-types" / "fixtures" / "runner" / "v2"
    validator = Draft202012Validator(load_json(schema_root / f"{schema_name}.schema.json"))

    validator.validate(load_json(fixture_root / f"{fixture_name}.json"))


def test_completed_diarization_requires_a_real_speaker_segment(repository_root: Path):
    schema_path = (
        repository_root
        / "packages"
        / "shared-types"
        / "schemas"
        / "runner"
        / "v2"
        / "result.schema.json"
    )
    validator = Draft202012Validator(load_json(schema_path))
    invalid_result = {
        "protocolVersion": 2,
        "asrBackend": "funasr",
        "diarizationRequested": True,
        "diarizationStatus": "completed",
        "warnings": [],
        "durationMinutes": 1,
        "transcriptSegments": [
            {"id": "segment-1", "startMs": 0, "endMs": 1000, "text": "逐字稿"}
        ],
        "speakerSegments": [],
    }

    with pytest.raises(ValidationError):
        validator.validate(invalid_result)
