#!/usr/bin/env python3
# pyright: reportAny=false, reportUnknownArgumentType=false, reportUnknownMemberType=false
"""Verify rubc-ng test-manifest.toml covers every reference ROM exactly once."""
from __future__ import annotations
import sys
import tomllib
from collections import Counter
from pathlib import Path

repo = Path(__file__).resolve().parents[1]
manifest_path = repo / "rubc-ng-data" / "test-manifest.toml"
manifest = tomllib.loads(manifest_path.read_text())
rom_entries = manifest.get("rom", [])
paths = [entry["path"] for entry in rom_entries]
counts = Counter(paths)
duplicates = sorted(path for path, count in counts.items() if count != 1)
expected = sorted(
    path.relative_to(repo).as_posix()
    for path in (repo / "reference" / "test-suites").rglob("*")
    if path.suffix.lower() in {".gb", ".gbc"}
)
missing = sorted(set(expected) - set(paths))
extra = sorted(set(paths) - set(expected))
vector_suites = manifest.get("vector_suite", [])
errors = []
if missing:
    errors.append(f"missing={len(missing)}")
if extra:
    errors.append(f"extra={len(extra)}")
if duplicates:
    errors.append(f"duplicate_or_nonunique={len(duplicates)}")
if not vector_suites:
    errors.append("missing_vector_suite")
if errors:
    print("manifest coverage FAIL " + " ".join(errors))
    if missing:
        print("missing paths:")
        print("\n".join(missing))
    if extra:
        print("extra paths:")
        print("\n".join(extra))
    if duplicates:
        print("non-unique paths:")
        print("\n".join(duplicates))
    sys.exit(1)
by_suite = Counter(entry["suite"] for entry in rom_entries)
by_status = Counter(entry["current_old_core_status"] for entry in rom_entries)
by_expectation = Counter(entry["expectation"]["kind"] for entry in rom_entries)
model_counts = Counter(model for entry in rom_entries for model in entry["intended_models"])
print(f"manifest coverage OK: roms={len(paths)} expected={len(expected)} unique={len(counts)}")
print("suites: " + ", ".join(f"{k}={v}" for k, v in sorted(by_suite.items())))
print("statuses: " + ", ".join(f"{k}={v}" for k, v in sorted(by_status.items())))
print("expectations: " + ", ".join(f"{k}={v}" for k, v in sorted(by_expectation.items())))
print("models: " + ", ".join(f"{k}={v}" for k, v in sorted(model_counts.items())))
for suite in vector_suites:
    exp = suite["expectation"]
    print(f"vectors: {suite['suite']} files={exp['files']} cases={exp['cases']} status={suite['current_old_core_status']}")
