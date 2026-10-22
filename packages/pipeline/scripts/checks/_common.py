"""Shared helpers for the number-check scripts: every check writes a TSV (metric, value, note)."""
from pathlib import Path


def write_tsv(path, rows):
    """rows: iterable of (metric, value, note). Overwrites; creates the directory."""
    Path(path).parent.mkdir(parents=True, exist_ok=True)
    with open(path, "w") as f:
        f.write("metric\tvalue\tnote\n")
        for metric, value, note in rows:
            f.write(f"{metric}\t{value}\t{note}\n")


def config_value(config_path, *keys):
    import yaml
    node = yaml.safe_load(Path(config_path).read_text())
    for k in keys:
        node = node[k]
    return node
