import argparse
import json
import sys
from pathlib import Path

# Must match WikiIndexBenchmarkResult in src-tauri/src/commands/wikipedia.rs
BENCHMARK_KEYS = (
    "model",
    "total_entries_in_zim",
    "benchmark_entries",
    "scanned_entries",
    "accepted_articles",
    "embedded_articles",
    "windows",
    "total_ms",
    "read_ms",
    "parse_ms",
    "embed_ms",
    "entries_per_sec",
    "accepted_per_sec",
    "embedded_per_sec",
)

META_KEYS = frozenset(
    {"version", "captured_at", "notes", "name", "degrade_thresholds", "benchmark", "metrics"}
)


def load_json(path: Path) -> dict:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise RuntimeError(
            f"Invalid JSON at {path}: {exc}\n"
            "If you pasted two JSON objects into one file, keep only one object "
            "(see benchmarks/wikipedia_index_baseline.json schema version 2)."
        ) from exc
    except Exception as exc:
        raise RuntimeError(f"Failed reading {path}: {exc}") from exc


def extract_snapshot(doc: dict) -> dict:
    """Return the benchmark payload: same keys as WikiIndexBenchmarkResult."""
    if "benchmark" in doc and isinstance(doc["benchmark"], dict):
        return doc["benchmark"]
    if "metrics" in doc and isinstance(doc["metrics"], dict):
        # Legacy v1: nested metrics + possibly model/benchmark_entries on root
        snap = dict(doc["metrics"])
        for k in ("model", "benchmark_entries", "total_entries_in_zim", "windows"):
            if k in doc and k not in snap:
                snap[k] = doc[k]
        return snap
    return {k: doc[k] for k in doc if k not in META_KEYS}


def pct_change(current: float, baseline: float) -> float:
    if baseline == 0:
        return 0.0
    return ((current - baseline) / baseline) * 100.0


def fmt(v: float) -> str:
    return f"{v:.2f}"


def require_metric_float(snap: dict, key: str, label: str) -> float:
    """Require a numeric metric; fail loudly instead of defaulting to 0.0."""
    if key not in snap:
        raise ValueError(f"{label} snapshot: missing required metric {key!r}")
    raw = snap[key]
    try:
        return float(raw)
    except (TypeError, ValueError) as exc:
        raise ValueError(
            f"{label} snapshot: metric {key!r} must be numeric, got {raw!r}"
        ) from exc


def report_key_mismatch(baseline_snap: dict, current_snap: dict) -> None:
    b_keys = set(baseline_snap.keys())
    c_keys = set(current_snap.keys())
    missing_in_current = sorted(b_keys - c_keys)
    missing_in_baseline = sorted(c_keys - b_keys)
    if missing_in_current:
        print(f"⚠ Keys in baseline but missing in current: {missing_in_current}")
    if missing_in_baseline:
        print(f"⚠ Keys in current but missing in baseline: {missing_in_baseline}")
    expected = set(BENCHMARK_KEYS)
    b_missing_expected = sorted(expected - b_keys)
    c_missing_expected = sorted(expected - c_keys)
    if b_missing_expected:
        print(f"⚠ Baseline missing standard benchmark fields: {b_missing_expected}")
    if c_missing_expected:
        print(f"⚠ Current missing standard benchmark fields: {c_missing_expected}")
    print("")


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Compare current wikipedia indexing benchmark against baseline "
        "(same schema as WikiIndexBenchmarkResult)."
    )
    parser.add_argument(
        "--baseline",
        default="benchmarks/wikipedia_index_baseline.json",
        help="Path to baseline JSON (v2 flat or legacy metrics/benchmark)",
    )
    parser.add_argument(
        "--current",
        required=True,
        help="Path to current run JSON (paste from Copy result JSON, or flat file)",
    )
    parser.add_argument(
        "--verbose",
        action="store_true",
        help="Print all numeric benchmark fields side by side",
    )
    args = parser.parse_args()

    baseline_path = Path(args.baseline)
    current_path = Path(args.current)

    baseline_doc = load_json(baseline_path)
    current_doc = load_json(current_path)

    b_snap = extract_snapshot(baseline_doc)
    c_snap = extract_snapshot(current_doc)

    thresholds = baseline_doc.get("degrade_thresholds", {})
    checks = [
        ("total_ms", thresholds.get("total_ms_pct", 15.0), "max"),
        ("entries_per_sec", thresholds.get("entries_per_sec_pct", -15.0), "min"),
        ("accepted_per_sec", thresholds.get("accepted_per_sec_pct", -15.0), "min"),
        ("embedded_per_sec", thresholds.get("embedded_per_sec_pct", -15.0), "min"),
    ]

    print("Wikipedia indexing benchmark comparison")
    print(f"Baseline: {baseline_path}")
    print(f"Current:  {current_path}")
    print("")

    report_key_mismatch(b_snap, c_snap)

    if args.verbose:
        print("--- All benchmark scalars ---")
        for key in BENCHMARK_KEYS:
            bv = b_snap.get(key, "—")
            cv = c_snap.get(key, "—")
            print(f"  {key}: baseline={bv!r}  current={cv!r}")
        print("")

    failed = False
    for key, threshold_pct, mode in checks:
        b = require_metric_float(b_snap, key, "baseline")
        c = require_metric_float(c_snap, key, "current")
        delta = pct_change(c, b)

        if mode == "max":
            ok = delta <= float(threshold_pct)
            relation = "<="
        else:
            ok = delta >= float(threshold_pct)
            relation = ">="

        status = "OK" if ok else "FAIL"
        print(
            f"- {key}: baseline={fmt(b)} current={fmt(c)} delta={fmt(delta)}% "
            f"(threshold {relation} {fmt(float(threshold_pct))}%) => {status}"
        )
        if not ok:
            failed = True

    print("")
    if failed:
        print("Result: regression detected.")
        return 1
    print("Result: within configured thresholds.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
