"""Write tests/golden/satslice_windows.csv from the current tracks of the satslice run.

Run `make pipeline-satslice` first. Regenerate only after reviewing WHY the numbers changed; the CSV is the recorded
current behaviour (hecate: model class markov, upstream 76b4836 + streaming patch), not an independent truth.
"""
from pathlib import Path

import duckdb

ROOT = Path(__file__).resolve().parents[2]
T = ROOT / "data/tracks/satslice/all"
QUERY = f"""
SELECT g.start_pos, g.end_pos,
       round(g.gc_pct, 3) AS gc_pct, round(g.gc_skew, 3) AS gc_skew,
       round(s.shannon_entropy, 4) AS shannon_entropy, s.n_acgt,
       round(h.mean_entropy, 4) AS hecate_mean_entropy, h.n_valid
FROM read_parquet('{T}/gc_content.parquet') g
JOIN read_parquet('{T}/shannon_entropy.parquet') s USING (start_pos, end_pos)
JOIN read_parquet('{T}/hecate_mprob.parquet') h USING (start_pos, end_pos)
ORDER BY g.start_pos
"""
if __name__ == "__main__":
    out = ROOT / "tests/golden/satslice_windows.csv"
    duckdb.sql(f"COPY ({QUERY}) TO '{out}' (HEADER, DELIMITER ',')")
    print("wrote", out)
