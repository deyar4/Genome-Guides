import importlib.util
from pathlib import Path

SPEC = importlib.util.spec_from_file_location("benchmark_table", Path(__file__).resolve().parents[1] / "checks" / "benchmark_table.py")
bt = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(bt)


def test_declared_memory_and_over_flag(tmp_path):
    rules = tmp_path / "rules"
    rules.mkdir()
    (rules / "a.smk").write_text(
        'rule big:\n    resources:\n        mem_mb=config["resources"]["hecate"]["mem_mb"]\n'
        'rule small:\n    resources:\n        mem_mb=512\n')
    cfg = {"resources": {"hecate": {"mem_mb": 8192}}}
    assert bt.declared_mem(rules, cfg) == {"big": 8192, "small": 512}
    bench = tmp_path / "b"
    bench.mkdir()
    head = "s\th:m:s\tmax_rss\tmax_vms\n"
    (bench / "big.tsv").write_text(head + "10.5\t0:00:10\t6000.4\t1\n")
    (bench / "small.tsv").write_text(head + "2.0\t0:00:02\t900.0\t1\n")
    rows = {r["rule"]: r for r in bt.table(bench, bt.declared_mem(rules, cfg))}
    assert rows["big"]["over_declared"] == "False" and rows["big"]["max_rss_mb"] == "6000"
    assert rows["small"]["over_declared"] == "True" and rows["small"]["seconds"] == "2.0"
