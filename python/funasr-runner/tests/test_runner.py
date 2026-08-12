from __future__ import annotations

import json


def test_extract_segments_preserves_real_speaker_labels(runner_module):
    transcript, speakers = runner_module.extract_segments(
        {
            "sentence_info": [
                {"text": "第一句", "start": 10, "end": 20, "speaker": "speaker-0"},
                {"text": "第二句", "start": 21, "end": 30, "speaker": "speaker-1"},
            ]
        },
        True,
    )

    assert [segment["text"] for segment in transcript] == ["第一句", "第二句"]
    assert [segment["speaker"] for segment in speakers] == ["speaker-0", "speaker-1"]


def test_extract_segments_does_not_invent_missing_speaker(runner_module):
    transcript, speakers = runner_module.extract_segments(
        {"sentence_info": [{"text": "没有标签", "start": 0, "end": 100}]},
        True,
    )

    assert transcript == [
        {"id": "segment-1", "startMs": 0, "endMs": 100, "text": "没有标签"}
    ]
    assert speakers == []


def test_extract_segments_uses_full_text_when_sentences_are_missing(runner_module):
    transcript, speakers = runner_module.extract_segments({"text": "完整逐字稿"}, False)

    assert transcript == [
        {"id": "segment-1", "startMs": 0, "endMs": 0, "text": "完整逐字稿"}
    ]
    assert speakers == []


def test_extract_segments_rejects_partial_speaker_projection(runner_module):
    transcript, speakers = runner_module.extract_segments(
        {
            "sentence_info": [
                {"text": "有标签", "start": 0, "end": 100, "speaker": 0},
                {"text": "无标签", "start": 101, "end": 200},
            ]
        },
        True,
    )

    assert len(transcript) == 2
    assert speakers == []


def test_write_json_atomically_replaces_complete_document(runner_module, tmp_path, monkeypatch):
    target = tmp_path / "result.json"
    runner_module.write_json(target, {"revision": 1})
    observed = []
    original_replace = runner_module.os.replace

    def observe_before_replace(source, destination):
        observed.append(json.loads(target.read_text(encoding="utf-8")))
        original_replace(source, destination)

    monkeypatch.setattr(runner_module.os, "replace", observe_before_replace)
    runner_module.write_json(target, {"revision": 2, "payload": "new"})

    assert observed == [{"revision": 1}]
    assert json.loads(target.read_text(encoding="utf-8")) == {"revision": 2, "payload": "new"}
    assert list(tmp_path.glob("*.tmp")) == []


def test_progress_is_v2_and_revision_is_monotonic(runner_module, tmp_path, capsys):
    runner_module.write_progress(tmp_path, "transcribing", "first", progress_percent=10)
    first = json.loads((tmp_path / "progress.json").read_text(encoding="utf-8"))
    runner_module.write_progress(tmp_path, "completed", "done", progress_percent=100)
    second = json.loads((tmp_path / "progress.json").read_text(encoding="utf-8"))

    assert first["protocolVersion"] == 2
    assert second["revision"] > first["revision"]
    for line in capsys.readouterr().out.splitlines():
        event = json.loads(line)
        assert event["protocolVersion"] == 2
        assert event["type"] == "progress"
