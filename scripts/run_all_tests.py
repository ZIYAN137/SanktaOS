#!/usr/bin/env python3
from __future__ import annotations

import argparse
import os
import shlex
import subprocess
import sys
import time
from datetime import datetime
from pathlib import Path
from typing import Optional, Tuple


_STDOUT_OK = True


def _safe_print(s: str) -> None:
    global _STDOUT_OK
    if not _STDOUT_OK:
        return
    try:
        print(s, flush=True)
    except BrokenPipeError:
        _STDOUT_OK = False
        try:
            sys.stdout = open(os.devnull, "w")
        except Exception:
            pass


def _safe_write(line: str) -> None:
    global _STDOUT_OK
    if not _STDOUT_OK:
        return
    try:
        sys.stdout.write(line)
        sys.stdout.flush()
    except BrokenPipeError:
        _STDOUT_OK = False
        try:
            sys.stdout = open(os.devnull, "w")
        except Exception:
            pass


def _run_capture(cmd: list[str], cwd: Path) -> str:
    p = subprocess.run(
        cmd,
        cwd=str(cwd),
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
    )
    if p.returncode != 0:
        return "unknown"
    return p.stdout.strip()


def _log_section(summary_fp, title: str) -> None:
    summary_fp.write("\n== {} ==\n".format(title))
    summary_fp.flush()


def _tail_file(path: Path, max_lines: int) -> str:
    if max_lines <= 0 or not path.exists():
        return ""
    try:
        with path.open("r", encoding="utf-8", errors="replace") as fp:
            lines = fp.readlines()
        lines = lines[-max_lines:]
        return "".join(lines)
    except Exception:
        return ""


def _run_step(
    summary_fp,
    name: str,
    logfile: Path,
    cmd: list[str],
    cwd: Path,
    *,
    stream: bool,
) -> Tuple[bool, int, int]:
    _log_section(summary_fp, name)
    cmd_str = shlex.join(cmd)
    summary_fp.write("cmd: {}\n".format(cmd_str))
    summary_fp.write("cwd: {}\n".format(cwd))
    summary_fp.write("log: {}\n".format(logfile))
    summary_fp.flush()

    _safe_print("")
    _safe_print("== {} ==".format(name))
    _safe_print("cmd: {}".format(cmd_str))
    _safe_print("log: {}".format(logfile))
    _safe_print("running...")

    logfile.parent.mkdir(parents=True, exist_ok=True)

    start = time.time()
    rc = 0
    with logfile.open("w", encoding="utf-8", newline="\n") as fp:
        fp.write("cmd: {}\n".format(shlex.join(cmd)))
        fp.write("cwd: {}\n".format(cwd))
        fp.write("time: {}\n\n".format(datetime.now().isoformat(timespec="seconds")))
        fp.flush()

        if stream:
            p = subprocess.Popen(
                cmd,
                cwd=str(cwd),
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                text=True,
                encoding='utf-8',
                errors='replace',
                bufsize=1,
            )
            assert p.stdout is not None
            for line in p.stdout:
                fp.write(line)
                fp.flush()
                _safe_write(line)
            rc = p.wait()
        else:
            p = subprocess.run(cmd, cwd=str(cwd), stdout=fp, stderr=subprocess.STDOUT)
            rc = p.returncode

    dur = int(time.time() - start)
    if rc == 0:
        summary_fp.write("result: PASS ({}s)\n".format(dur))
        summary_fp.flush()
        _safe_print("result: PASS ({}s)".format(dur))
        return True, rc, dur
    summary_fp.write("result: FAIL (exit={}, {}s)\n".format(rc, dur))
    summary_fp.flush()
    _safe_print("result: FAIL (exit={}, {}s)".format(rc, dur))
    return False, rc, dur


def _print_result_table(rows: list[tuple[str, str, int, str]]) -> None:
    """
    rows: (status, name, duration_s, log_path)
    """
    if not rows:
        return

    status_w = max(len(r[0]) for r in rows + [("STATUS", "", 0, "")])
    dur_w = max(len(str(r[2])) for r in rows + [("", "", 0, "")])
    name_w = max(len(r[1]) for r in rows + [("", "STEP", 0, "")])

    _safe_print("")
    _safe_print("Results:")
    _safe_print(
        "{:<{sw}}  {:>4}  {:<{nw}}  {}".format(
            "STATUS", "TIME", "STEP", "LOG", sw=status_w, nw=name_w
        )
    )
    _safe_print(
        "{:<{sw}}  {:>4}  {:<{nw}}  {}".format(
            "-" * status_w, "-" * 4, "-" * name_w, "-" * 3, sw=status_w, nw=name_w
        )
    )
    for status, name, dur_s, log_path in rows:
        _safe_print(
            "{:<{sw}}  {:>3}s  {:<{nw}}  {}".format(
                status, dur_s, name, log_path, sw=status_w, nw=name_w
            )
        )


def main() -> int:
    repo_root = (Path(__file__).resolve().parent / "..").resolve()

    # Env defaults
    env_arch = os.environ.get("ARCH", "riscv")
    env_out_dir = os.environ.get("OUT_DIR")
    env_skip_os = os.environ.get("SKIP_OS") == "1"
    env_skip_crates = os.environ.get("SKIP_CRATES") == "1"
    env_crates = os.environ.get("CRATES")

    parser = argparse.ArgumentParser(
        description="Run all tests (kernel QEMU tests + host crate tests) and collect logs."
    )
    parser.add_argument("--arch", default=env_arch, help="riscv or loongarch (default: env ARCH)")
    parser.add_argument("--out-dir", default=env_out_dir, help="output directory")
    parser.add_argument("--skip-os", action="store_true", default=env_skip_os)
    parser.add_argument("--skip-crates", action="store_true", default=env_skip_crates)
    parser.add_argument(
        "--stream",
        action="store_true",
        default=False,
        help="stream command output to the terminal (always saved to log files too)",
    )
    parser.add_argument(
        "--tail-fail",
        type=int,
        default=80,
        help="print last N lines of failing logs at the end (default: 80; 0 to disable)",
    )
    parser.add_argument(
        "--crates",
        nargs="*",
        default=None,
        help='override crate list (default: env CRATES or "device fs klog mm net sync uapi vfs")',
    )
    args = parser.parse_args()

    ts = datetime.now().strftime("%Y%m%d-%H%M%S")
    # Keep test artifacts out of Cargo's `target/` to avoid polluting builds.
    out_dir = Path(args.out_dir) if args.out_dir else (repo_root / "test-reports" / ts)
    out_dir.mkdir(parents=True, exist_ok=True)

    summary_path = out_dir / "summary.txt"
    git_short = _run_capture(["git", "rev-parse", "--short", "HEAD"], cwd=repo_root)

    crates_default = ["device", "fs", "klog", "mm", "net", "sync", "uapi", "vfs"]
    if args.crates is not None and len(args.crates) > 0:
        crates = args.crates
    elif env_crates:
        crates = env_crates.split()
    else:
        crates = crates_default

    failed = False
    result_rows: list[tuple[str, str, int, str]] = []
    failure_logs: list[tuple[str, Path, int]] = []
    with summary_path.open("w", encoding="utf-8", newline="\n") as summary_fp:
        header = [
            "root: {}".format(repo_root),
            "arch: {}".format(args.arch),
            "time: {}".format(datetime.now().isoformat(timespec="seconds")),
            "git:  {}".format(git_short),
            "out_dir: {}".format(out_dir),
            "summary: {}".format(summary_path),
        ]
        summary_fp.write("\n".join(header) + "\n")
        summary_fp.flush()

        _safe_print("\n".join(header))

        if not args.skip_os:
            ok, rc, dur = _run_step(
                summary_fp,
                "os: make -C os test",
                out_dir / "os.make-test.log",
                ["make", "-C", str(repo_root / "os"), "test", "ARCH={}".format(args.arch)],
                cwd=repo_root,
                stream=args.stream,
            )
            failed = failed or (not ok)
            result_rows.append(
                ("PASS" if ok else "FAIL", "os: make -C os test", dur, str(out_dir / "os.make-test.log"))
            )
            if not ok:
                failure_logs.append(("os: make -C os test", out_dir / "os.make-test.log", rc))
        else:
            _log_section(summary_fp, "os: skipped (--skip-os / SKIP_OS=1)")
            _safe_print("\n== os: skipped (--skip-os / SKIP_OS=1) ==")
            result_rows.append(("SKIP", "os: make -C os test", 0, "-"))

        if not args.skip_crates:
            for c in crates:
                log_path = out_dir / "crate.{}.log".format(c)
                ok, rc, dur = _run_step(
                    summary_fp,
                    "crate: {}".format(c),
                    log_path,
                    [
                        "cargo",
                        "test",
                        "--manifest-path",
                        str(repo_root / "crates" / c / "Cargo.toml"),
                    ],
                    cwd=repo_root,
                    stream=args.stream,
                )
                failed = failed or (not ok)
                result_rows.append(("PASS" if ok else "FAIL", "crate: {}".format(c), dur, str(log_path)))
                if not ok:
                    failure_logs.append(("crate: {}".format(c), log_path, rc))
        else:
            _log_section(summary_fp, "crates: skipped (--skip-crates / SKIP_CRATES=1)")
            _safe_print("\n== crates: skipped (--skip-crates / SKIP_CRATES=1) ==")
            result_rows.append(("SKIP", "crates: all", 0, "-"))

        _log_section(summary_fp, "done")
        summary_fp.write("out_dir: {}\n".format(out_dir))
        summary_fp.write("summary: {}\n".format(summary_path))
        summary_fp.write("overall: {}\n".format("FAIL" if failed else "PASS"))
        summary_fp.flush()

        _print_result_table(result_rows)

        if failed and args.tail_fail > 0:
            _safe_print("")
            _safe_print("Failures (tail):")
            for name, log_path, rc in failure_logs:
                _safe_print("")
                _safe_print("== {} (exit={}) ==".format(name, rc))
                _safe_print("log: {}".format(log_path))
                tail = _tail_file(log_path, args.tail_fail)
                if tail.strip():
                    _safe_print(tail.rstrip("\n"))
                else:
                    _safe_print("(no log content)")

        _safe_print("")
        _safe_print("out_dir: {}".format(out_dir))
        _safe_print("summary: {}".format(summary_path))
        _safe_print("overall: {}".format("FAIL" if failed else "PASS"))

    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
