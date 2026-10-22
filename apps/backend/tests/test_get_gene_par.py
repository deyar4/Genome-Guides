"""get_gene must not mix PAR copies: one gene_id on two contigs (chrX/chrY) is selected by chrom,
and without chrom the first copy in (contig, start) order is returned with the other copies listed."""
import duckdb
import pytest
from fastapi.testclient import TestClient

from app.api.dependencies import get_track_service
from app.main import app
from app.services.duckdb_track_service import DuckDBTrackService


@pytest.fixture(scope="module")
def client(tmp_path_factory):
    root = tmp_path_factory.mktemp("data")
    out = root / "tracks" / "synth" / "all"
    out.mkdir(parents=True)
    con = duckdb.connect()
    con.execute("""CREATE TABLE g AS SELECT * FROM (VALUES
        ('PAR1', 'PARGENE', 'protein_coding', 'chrX', 100, 200, '+'),
        ('PAR1', 'PARGENE', 'protein_coding', 'chrY', 5000, 5100, '+')
    ) t(gene_id, gene_name, biotype, contig, start_pos, end_pos, strand)""")
    con.execute(f"COPY g TO '{out}/gene_models.parquet' (FORMAT PARQUET)")
    con.execute("""CREATE TABLE t AS SELECT * FROM (VALUES
        ('TX', 'PAR1', 'PARGENE', 'protein_coding', 'chrX', 100, 200, '+'),
        ('TY', 'PAR1', 'PARGENE', 'protein_coding', 'chrY', 5000, 5100, '+')
    ) t(transcript_id, gene_id, gene_name, biotype, contig, start_pos, end_pos, strand)""")
    con.execute(f"COPY t TO '{out}/transcripts.parquet' (FORMAT PARQUET)")
    service = DuckDBTrackService(data_dir=str(root))
    app.dependency_overrides[get_track_service] = lambda: service
    yield TestClient(app, raise_server_exceptions=False)
    app.dependency_overrides.clear()


def test_chrom_selects_copy_and_its_transcripts(client):
    for chrom, tx in (("chrX", "TX"), ("chrY", "TY")):
        r = client.get("/api/v1/genes/PAR1", params={"build": "synth", "chrom": chrom}).json()
        assert r["chrom"] == chrom
        assert [t["transcript_id"] for t in r["transcripts"]] == [tx]
        assert r["other_copies"] == []


def test_without_chrom_is_deterministic_and_lists_other_copy(client):
    r = client.get("/api/v1/genes/PAR1", params={"build": "synth"}).json()
    assert r["chrom"] == "chrX"
    assert [t["transcript_id"] for t in r["transcripts"]] == ["TX"]
    assert [c["chrom"] for c in r["other_copies"]] == ["chrY"]


def test_unknown_chrom_is_404(client):
    assert client.get("/api/v1/genes/PAR1", params={"build": "synth", "chrom": "chr1"}).status_code == 404
