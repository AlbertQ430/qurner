#!/usr/bin/env python3
"""M6B 性能基准：1,000 本书冷启动与搜索。

可重复执行：
    python3 scripts/m6b_benchmark.py all --app <Qurner 可执行文件> [--fixtures DIR] [--home DIR]

默认路径相对仓库根与系统临时目录，不依赖个人路径；仅终止本脚本启动的 Qurner 进程。
冷启动失败或任一 p95 超限时退出码为 1。
"""

from __future__ import annotations

import argparse
import json
import math
import os
import shutil
import signal
import sqlite3
import subprocess
import sys
import tempfile
import time
import uuid
import zipfile
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
DEFAULT_APP = REPO / "target/release/bundle/macos/Qurner.app/Contents/MacOS/qurner"
DEFAULT_ROOT = Path(tempfile.gettempdir()) / "qurner-m6b"
COLDSTART_THRESHOLD = 3000.0
SEARCH_THRESHOLD = 200.0


def percentile(samples: list[float], percent: float) -> float:
    """nearest-rank：p95 等于排序后第 ceil(0.95*n) 个样本。"""
    ordered = sorted(samples)
    rank = math.ceil((percent / 100.0) * len(ordered))
    return ordered[min(max(rank - 1, 0), len(ordered) - 1)]


def build_fixtures(directory: Path, count: int = 1000) -> None:
    directory.mkdir(parents=True, exist_ok=True)
    for index in range(count):
        title = f"规模书 {index:04d}"
        opf = (
            '<?xml version="1.0" encoding="UTF-8"?>'
            '<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="id">'
            '<metadata xmlns:dc="http://purl.org/dc/elements/1.1/">'
            f"<dc:title>{title}</dc:title><dc:creator>规模作者</dc:creator>"
            f'<dc:identifier id="id">urn:uuid:scale-{index:04d}</dc:identifier>'
            '<meta property="dcterms:modified">2026-11-16T00:00:00Z</meta></metadata>'
            '<manifest><item id="c1" href="chapter1.xhtml" media-type="application/xhtml+xml"/></manifest>'
            '<spine><itemref idref="c1"/></spine></package>'
        )
        chapter = (
            '<?xml version="1.0" encoding="UTF-8"?>'
            '<html xmlns="http://www.w3.org/1999/xhtml"><body>'
            f"<h1>{title}</h1><p>规模测试正文。</p></body></html>"
        )
        path = directory / f"book-{index:04d}.epub"
        if path.exists():
            continue
        with zipfile.ZipFile(path, "w", zipfile.ZIP_DEFLATED) as archive:
            archive.writestr("mimetype", "application/epub+zip", compress_type=zipfile.ZIP_STORED)
            archive.writestr(
                "META-INF/container.xml",
                '<?xml version="1.0"?><container version="1.0" '
                'xmlns="urn:oasis:names:tc:opendocument:xmlns:container">'
                '<rootfiles><rootfile full-path="OEBPS/content.opf" '
                'media-type="application/oebps-package+xml"/></rootfiles></container>',
            )
            archive.writestr("OEBPS/content.opf", opf)
            archive.writestr("OEBPS/chapter1.xhtml", chapter)
    print(f"fixtures={directory} count={count}")


def run_checked(command: list[str], **kwargs) -> subprocess.CompletedProcess[str]:
    result = subprocess.run(command, capture_output=True, text=True, **kwargs)
    if result.returncode != 0:
        sys.stderr.write(result.stdout + result.stderr)
        raise SystemExit(f"command failed ({result.returncode}): {' '.join(command)}")
    return result


def prepare(app: Path, fixtures: Path, home: Path, scale_jsonl: Path, reset: bool) -> None:
    db_dir = home / "Library/Application Support/com.qurner.reader"
    if home.exists() and any(home.iterdir()):
        if not reset:
            raise SystemExit(
                f"refusing to overwrite existing home: {home} (choose a new --home or pass --reset)"
            )
        managed_root = Path(tempfile.gettempdir()).resolve() / "qurner-m6b"
        if managed_root not in home.resolve().parents:
            raise SystemExit(
                f"refusing to delete non-managed directory: {home} (only {managed_root} may be reset)"
            )
        shutil.rmtree(home)
    db_dir.mkdir(parents=True, exist_ok=True)
    env = dict(os.environ, HOME=str(home))
    process = subprocess.Popen(
        [str(app)], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL
    )
    try:
        time.sleep(5)
    finally:
        process.send_signal(signal.SIGTERM)
        process.wait(timeout=10)
    time.sleep(1.5)
    run_checked(
        [
            "cargo",
            "run",
            "--release",
            "-q",
            "-p",
            "epub-core",
            "--example",
            "scale_manifest",
            "--",
            str(fixtures),
            str(scale_jsonl),
        ],
        cwd=REPO,
        env=dict(os.environ, PATH=os.environ["PATH"]),
    )
    rows = [json.loads(line) for line in scale_jsonl.open(encoding="utf-8")]
    database = db_dir / "reader.sqlite3"
    connection = sqlite3.connect(database)
    columns = [row[1] for row in connection.execute("PRAGMA table_info(books)")]
    for row in rows:
        title = row["title"] or f"规模书 {row['index']:04d}"
        author = row["author"] or "规模作者"
        values = {
            "id": str(uuid.uuid4()),
            "fingerprint": row["fingerprint"],
            "source_path": row["path"],
            "title": title,
            "author": author,
            "title_search": title.lower(),
            "author_search": author.lower(),
            "title_sort_key": b"",
            "author_sort_key": b"",
            "cover_cache_key": None,
            "manifest_json": json.dumps(row["manifest"], ensure_ascii=False, separators=(",", ":")),
            "manifest_version": row["schema_version"],
            "lifecycle": "active",
            "imported_at": "2026-11-16T00:00:%02dZ" % (row["index"] % 60),
            "last_opened_at": None,
        }
        connection.execute(
            f"insert into books ({','.join(columns)}) values ({','.join(['?'] * len(columns))})",
            [values[column] for column in columns],
        )
    connection.commit()
    count = connection.execute("select count(*) from books").fetchone()[0]
    connection.close()
    print(f"prepared home={home} books={count}")


def wait_library_ready(pid: int, home: Path, deadline: float) -> bool:
    script = (
        'tell application "System Events" to tell (first process whose unix id is %d) '
        "to get entire contents of window 1"
    ) % pid
    while time.time() < deadline:
        try:
            result = subprocess.run(
                ["osascript", "-e", script],
                capture_output=True,
                text=True,
                timeout=3,
                env=dict(os.environ, HOME=str(home)),
            )
            text = result.stdout + result.stderr
            if "规模书" in text:
                return True
        except subprocess.TimeoutExpired:
            pass
        time.sleep(0.15)
    return False


def coldstart(app: Path, home: Path, runs: int) -> list[float]:
    samples: list[float] = []
    for index in range(runs):
        env = dict(os.environ, HOME=str(home))
        start = time.perf_counter()
        process = subprocess.Popen(
            [str(app)], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL
        )
        ready = False
        try:
            ready = wait_library_ready(process.pid, home, deadline=time.time() + 15)
        finally:
            process.send_signal(signal.SIGTERM)
            process.wait(timeout=10)
        if not ready:
            raise SystemExit(f"coldstart run {index} timed out waiting for library")
        samples.append(round((time.perf_counter() - start) * 1000, 1))
        print(f"coldstart sample={samples[-1]} ms")
        time.sleep(0.8)
    return samples


def search(database: Path, query: str, runs: int) -> dict:
    result = run_checked(
        [
            "cargo",
            "run",
            "--release",
            "-q",
            "-p",
            "storage-core",
            "--example",
            "list_bench",
            "--",
            str(database),
            query,
            str(runs),
        ],
        cwd=REPO,
    )
    lines = [line for line in result.stdout.splitlines() if line.startswith("{")]
    if not lines:
        raise SystemExit("list_bench produced no JSON")
    return json.loads(lines[-1])


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=["fixtures", "prepare", "coldstart", "search", "all"])
    parser.add_argument("--app", default=os.environ.get("QURNER_APP", str(DEFAULT_APP)))
    parser.add_argument("--fixtures", default=str(DEFAULT_ROOT / "fixtures"))
    parser.add_argument("--home", default=str(DEFAULT_ROOT / "home"))
    parser.add_argument("--runs", type=int, default=10)
    parser.add_argument("--reset", action="store_true", help="reset a script-managed home")
    args = parser.parse_args()

    app = Path(args.app)
    fixtures = Path(args.fixtures)
    home = Path(args.home)
    scale_jsonl = home.parent / "scale.jsonl"
    if not app.exists():
        print(f"Qurner executable not found: {app}", file=sys.stderr)
        return 2
    if args.command == "fixtures":
        build_fixtures(fixtures)
        return 0
    if args.command in ("prepare", "all"):
        build_fixtures(fixtures)
        prepare(app, fixtures, home, scale_jsonl, args.reset)
    database = home / "Library/Application Support/com.qurner.reader/reader.sqlite3"
    if not database.exists():
        print(f"missing 1000-book database: {database} (run prepare first)", file=sys.stderr)
        return 2
    results: list[dict] = []
    failed = False
    if args.command in ("coldstart", "all"):
        samples = coldstart(app, home, args.runs)
        p50 = percentile(samples, 50)
        p95 = percentile(samples, 95)
        passed = p95 <= COLDSTART_THRESHOLD
        failed |= not passed
        results.append(
            {
                "label": "coldstart",
                "unit": "ms",
                "samples": samples,
                "p50": round(p50, 2),
                "p95": round(p95, 2),
                "threshold": COLDSTART_THRESHOLD,
                "pass": passed,
            }
        )
    if args.command in ("search", "all"):
        summary = search(database, "0999", args.runs)
        failed |= not summary["pass"]
        results.append(summary)
    if results:
        print(json.dumps(results, ensure_ascii=False, indent=2))
        print("ALL_PASS" if all(item["pass"] for item in results) else "SOME_FAIL")
    return 0 if not failed else 1


if __name__ == "__main__":
    raise SystemExit(main())
