#!/usr/bin/env python3
"""Check resource validation and malformed-track failure behavior."""

import argparse
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "common"))
from common.resource_args import add_resource_args, effective_memory_mb
from common.tsv_to_parquet import convert_tsv_to_parquet


class ComplianceTests(unittest.TestCase):
    def test_resource_values_are_positive(self):
        parser = argparse.ArgumentParser()
        add_resource_args(parser)
        with self.assertRaises(SystemExit):
            parser.parse_args(["--threads", "0"])
        with self.assertRaises(ValueError):
            effective_memory_mb(0)

    def test_malformed_tsv_fails_explicitly(self):
        with tempfile.TemporaryDirectory() as directory:
            input_path = Path(directory) / "bad.tsv"
            output_path = Path(directory) / "track.parquet"
            input_path.write_text("chr1\tnot-a-number\n")
            with self.assertRaises(SystemExit) as error:
                convert_tsv_to_parquet(str(input_path), str(output_path), "homopolymers")
            self.assertIn("Could not parse TSV", str(error.exception))


if __name__ == "__main__":
    unittest.main()
