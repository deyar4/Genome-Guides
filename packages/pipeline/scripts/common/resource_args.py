"""Shared resource flags and conservative host-memory detection."""

import argparse
import os


def available_memory_mb() -> int:
    try:
        pages = os.sysconf("SC_AVPHYS_PAGES")
        page_size = os.sysconf("SC_PAGE_SIZE")
        return max(256, int(pages * page_size / 1_000_000))
    except (AttributeError, OSError, ValueError):
        return 16_000


def add_resource_args(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--threads", type=int, default=1, help="Worker threads")
    parser.add_argument(
        "--memory-mb",
        type=int,
        default=None,
        help="Available memory budget in MB (defaults to detected available RAM)",
    )


def effective_memory_mb(requested: int | None) -> int:
    return requested if requested is not None else available_memory_mb()
