# Cargo tally

`cargo tally` is a Cargo subcommand for drawing graphs of the number of
dependencies of a crate over time. **Scroll down for some graphs!**

```
Usage: cargo tally --init
       cargo tally [options] <crate>...

Options:
    --graph TITLE     Display line graph using gnuplot, rather than dump csv
    --exclude REGEX   Ignore a dependency coming from any crates matching regex
```

## Installation

```
cargo install cargo-tally
cargo tally --init
```

- There is a one-time setup step that downloads and extracts a 15 MB tarball of
  crates.io metadata into a directory called `tally` within the current
  directory. Subsequent queries read from this cached data and do not query
  crates.io.

- By default `cargo tally` prints out a CSV with a timestamp column and one
  column for each crate being tallied. Pass the `--graph` flag with a title,
  like `--graph "Exponential growth!"`, to pop open `gnuplot` with a line graph.
  Requires `gnuplot` to be present in your $PATH. On Ubuntu I was able to
  install this with `sudo apt install gnuplot`. If you can't get that working,
  you can always run without `--graph` and make your own graphs in Excel or
  Gnumeric.

- The tally command accepts a list of which crates to tally. This can either be
  the name of a crate like `serde` or a name with major+minor version like
  `serde:0.9`. If a major+minor version is not specified, dependencies on all
  versions of the crate are tallied together.

- **If you come up with an interesting graph, please [open an issue] and just
  drop the picture in there! I would love to see what you find!**

[open an issue]: https://github.com/dtolnay/cargo-tally/issues/new

---

### `cargo tally --graph "Serde vs rustc-serialize direct dependencies" rustc-serialize serde`

![Serde vs rustc-serialize direct dependencies][serde-rustc-serialize]

---

### `cargo tally --exclude '^google-' --graph "Number of direct dependencies by Serde version" serde:0.5 serde:0.6 serde:0.7 serde:0.8 serde:0.9 serde:1.0`

![Serde versions][serde-versions]

---

[serde-rustc-serialize]: https://user-images.githubusercontent.com/1940490/33064453-910b0754-ce5a-11e7-8cf3-8352ee4e0eca.png
[serde-versions]: https://user-images.githubusercontent.com/1940490/33064449-8df822e0-ce5a-11e7-9863-1ada8ae8c0eb.png

## License

Licensed under either of

 * Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
 * MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this project by you, as defined in the Apache-2.0 license,
shall be dual licensed as above, without any additional terms or conditions.
