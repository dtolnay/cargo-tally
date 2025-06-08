# Cargo tally

<img alt="Number of crates that depend directly on each regex version" src="https://user-images.githubusercontent.com/1940490/122184090-bc75d600-ce40-11eb-856b-affc568d2e15.png" width="30%"> <img alt="Fraction of crates that depend on failure vs anyhow and thiserror" src="https://user-images.githubusercontent.com/1940490/122184103-bf70c680-ce40-11eb-890c-988cd96f4428.png" width="30%"> <img alt="Fraction of crates.io that depends transitively on libc" src="https://user-images.githubusercontent.com/1940490/122184112-c13a8a00-ce40-11eb-8bdb-a7f6f03d2d91.png" width="30%">

**`cargo tally` is a Cargo subcommand for drawing graphs of the number of crates
that depend directly or indirectly on a crate over time.**

```
Usage: cargo tally [options] queries...

Options:
    --db <PATH>       Path to crates.io's database dump [default: ./db-dump.tar.gz]
    --jobs, -j <N>    Number of threads to run differential dataflow
    --relative        Display as a fraction of total crates, not absolute number
    --transitive      Count transitive dependencies, not just direct dependencies
```

[<img alt="github" src="https://img.shields.io/badge/github-dtolnay/cargo--tally-8da0cb?style=for-the-badge&labelColor=555555&logo=github" height="20">](https://github.com/dtolnay/cargo-tally)
[<img alt="crates.io" src="https://img.shields.io/crates/v/cargo-tally.svg?style=for-the-badge&color=fc8d62&logo=rust" height="20">](https://crates.io/crates/cargo-tally)
[<img alt="build status" src="https://img.shields.io/github/actions/workflow/status/dtolnay/cargo-tally/ci.yml?branch=master&style=for-the-badge" height="20">](https://github.com/dtolnay/cargo-tally/actions?query=branch%3Amaster)

<br>

## Installation

```console
$ wget https://static.crates.io/db-dump.tar.gz
$ cargo install cargo-tally
```

- Data is drawn from crates.io database dumps, which are published nightly by
  automation running on crates.io. You can download a new dump whenever you feel
  like having fresh data.

- The tally command accepts a list of which crates to tally. This can either be
  the name of a crate like `serde` or a name with arbitrary semver version
  specification like `serde:1.0`. If a version is not specified, dependencies on
  all versions of the crate are tallied together.

- The generated graphs use [D3](https://d3js.org/); the cargo tally command
  should pop open a browser showing your graph. It uses the same mechanism that
  `cargo doc --open` uses so hopefully it works well on various systems.

---

<br>

## Examples

- Number of crates that depend directly on each major version of the regex
  crate.

  **`$ cargo tally regex:0.1 regex:0.2 regex:1.0`**

![Number of crates that depend directly on each major version of regex][regex]

---

<br>

- Fraction of crates.io that depends directly on each major version of the regex
  crate. This is the same graph as the previous, but scaled to the exponentially
  growing total number of crates on crates.io.


  **`$ cargo tally regex:0.1 regex:0.2 regex:1.0 --relative`**

![Fraction of crates.io that depends directly on each major version of regex][regex-relative]

---

<br>

- Fraction of crates.io that depends directly on various error handling
  libraries. Note that crates are not double-counted; a crate that depends on
  *both* `anyhow` and `thiserror` counts as only one for the purpose of the
  `anyhow+thiserror` curve.

  **`$ cargo tally --relative quick-error failure anyhow+thiserror snafu eyre+color-eyre`**

![Fraction of crates.io that depends directly on various error handling libraries][failure-anyhow-thiserror]

---

<br>

- Fraction of crates.io that depends transitively on libc.

  **`$ cargo tally --relative --transitive libc`**

![Fraction of crates.io that depends transitively on libc][libc]

[regex]: https://user-images.githubusercontent.com/1940490/122184090-bc75d600-ce40-11eb-856b-affc568d2e15.png
[regex-relative]: https://user-images.githubusercontent.com/1940490/122184174-d31c2d00-ce40-11eb-8c17-bde6f3015c28.png
[failure-anyhow-thiserror]: https://github.com/user-attachments/assets/6c648998-30fe-43d0-9e9f-a2616881fbfa
[libc]: https://user-images.githubusercontent.com/1940490/122184112-c13a8a00-ce40-11eb-8bdb-a7f6f03d2d91.png

---

<br>

## Credits

The implementation is powered by [differential-dataflow].

<img src="https://raw.github.com/dtolnay/cargo-tally/72612d2290b0ab564fdc6e332bb69f556e1bb41b/ddshow.svg">

[differential-dataflow]: https://github.com/TimelyDataflow/differential-dataflow

<br>

#### License

<sup>
Licensed under either of <a href="LICENSE-APACHE">Apache License, Version
2.0</a> or <a href="LICENSE-MIT">MIT license</a> at your option.
</sup>

<br>

<sub>
Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this project by you, as defined in the Apache-2.0 license,
shall be dual licensed as above, without any additional terms or conditions.
</sub>
