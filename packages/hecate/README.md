# hecate

hecate - a modular genomic compressor.
Licensed under the terms of GNU GPL version 3 or later - see COPYING.
Report issues to Kamila Szewczyk <k@iczelia.net>.
Project homepage: https://gitlab.com/rahmannlab/hecate

## Building

```
$ cargo build --release
```

## Usage

```
$ target/release/hecate -h
$ target/release/hecate -2fMzstd:22 tests/data/GCF_000005845.2.fna -o ecoli.he
$ target/release/hecate stats ecoli.he
```

## Disclaimer

The file format may change until a stable version is reached.
