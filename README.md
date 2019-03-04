# Cargo tally

<img alt="Number of crates that depend directly on Serde vs rustc-serialize" src="https://user-images.githubusercontent.com/1940490/47555689-528c1500-d8c1-11e8-84e0-43b3efc14b08.png" width="30%"> <img alt="Number of crates that depend directly on each Serde version" src="https://user-images.githubusercontent.com/1940490/47555685-5029bb00-d8c1-11e8-84c4-6eaf8601f07b.png" width="30%"> <img alt="Fraction of crates.io that depends transitively on libc" src="https://user-images.githubusercontent.com/1940490/47555693-5455d880-d8c1-11e8-812e-ed38785c27c3.png" width="30%">

**`cargo tally` is a Cargo subcommand for drawing graphs of the number of crates
that depend directly or indirectly on a crate over time.**

```
Usage: cargo tally --init
       cargo tally [options] <crate>...

Options:
    --graph TITLE     Display line graph using gnuplot, rather than dump csv
    --relative        Display as a fraction of total crates, not absolute number
    --transitive      Count transitive dependencies, not just direct dependencies
    --exclude REGEX   Ignore a dependency coming from any crates matching regex
```

## Installation

```
cargo install cargo-tally
cargo tally --init
```

- There is a one-time setup step that downloads a 6 MB json file of crates.io
  metadata into a file called `tally.json.gz` within the current directory.
  Subsequent queries read from this cached data and do not query crates.io.

- By default `cargo tally` prints out a CSV with a timestamp column and one
  column for each crate being tallied. Pass the `--graph` flag with a title,
  like `--graph "Exponential growth!"`, to pop open `gnuplot` with a line graph.
  Requires `gnuplot` to be present in your $PATH. On Ubuntu I was able to
  install this with `sudo apt install gnuplot`. On macOS you want `brew install
  gnuplot --with-qt`. If you can't get that working, you can always run without
  `--graph` and make your own graphs in Excel or Gnumeric.

- The tally command accepts a list of which crates to tally. This can either be
  the name of a crate like `serde` or a name with arbitrary semver version
  specification like `serde:0.9`. If a version is not specified, dependencies on
  all versions of the crate are tallied together.

- **If you come up with an interesting graph, please [open an issue] and just
  drop the picture in there! I would love to see what you find! Also @mention
  the crates' authors if you would like to share with them.**

[open an issue]: https://github.com/dtolnay/cargo-tally/issues/new

---

### `cargo tally --graph "Number of crates that depend directly on Serde vs rustc-serialize" rustc-serialize serde`

![Number of crates that depend directly on Serde vs rustc-serialize][serde-rustc-serialize]

---

### `cargo tally --exclude '^google-' --graph "Number of crates that depend directly on each Serde version" serde:0.5 serde:0.6 serde:0.7 serde:0.8 serde:0.9 serde:1.0`

![Number of crates that depend directly on each Serde version][serde-versions]

---

### `cargo tally --graph "Fraction of crates.io that depends transitively on libc" --relative --transitive libc`

![Fraction of crates.io that depends transitively on libc][transitive-libc]

---

[serde-rustc-serialize]: https://user-images.githubusercontent.com/1940490/47555689-528c1500-d8c1-11e8-84e0-43b3efc14b08.png
[serde-versions]: https://user-images.githubusercontent.com/1940490/47555685-5029bb00-d8c1-11e8-84c4-6eaf8601f07b.png
[transitive-libc]: https://user-images.githubusercontent.com/1940490/47555693-5455d880-d8c1-11e8-812e-ed38785c27c3.png

## License

Licensed under either of

 * Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
 * MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this project by you, as defined in the Apache-2.0 license,
shall be dual licensed as above, without any additional terms or conditions.
