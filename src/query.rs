use crate::cratemap::CrateMap;
use anyhow::{bail, format_err, Error, Result};
use cargo_tally::arena::Slice;
use cargo_tally::id::QueryId;
use cargo_tally::version::VersionReq;
use cargo_tally::{Predicate, Query};
use std::convert::TryFrom;
use std::fmt::{self, Display};
use std::str::{FromStr, Split};

// for example &["serde:1.0", "anyhow:^1.0 + thiserror"]
pub fn parse<'a>(
    queries: impl IntoIterator<Item = &'a str>,
    crates: &CrateMap,
) -> Result<Vec<Query>> {
    queries
        .into_iter()
        .enumerate()
        .map(|(i, query)| {
            let id = QueryId(u8::try_from(i).unwrap());
            match parse_predicates(query, crates) {
                Ok(predicates) => Ok(Query { id, predicates }),
                Err(err) => bail!("failed to parse query {:?}: {}", query, err),
            }
        })
        .collect()
}

fn parse_predicates(string: &str, crates: &CrateMap) -> Result<Slice<Predicate>> {
    let predicates = IterPredicates::new(string, crates).collect::<Result<Vec<Predicate>>>()?;
    Ok(Slice::new(&predicates))
}

pub fn format(query: &str, crates: &CrateMap) -> String {
    DisplayQuery { query, crates }.to_string()
}

struct DisplayQuery<'a> {
    query: &'a str,
    crates: &'a CrateMap,
}

impl<'a> Display for DisplayQuery<'a> {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        for (i, predicate) in IterPredicates::new(self.query, self.crates).enumerate() {
            if i > 0 {
                formatter.write_str(" or ")?;
            }

            let predicate = predicate.unwrap();
            let original_name = self.crates.name(predicate.crate_id).unwrap();
            formatter.write_str(original_name)?;

            if let Some(req) = predicate.req {
                formatter.write_str(":")?;
                write!(formatter, "{}", req)?;
            }
        }
        Ok(())
    }
}

struct IterPredicates<'a> {
    split: Split<'a, char>,
    crates: &'a CrateMap,
}

impl<'a> IterPredicates<'a> {
    fn new(query: &'a str, crates: &'a CrateMap) -> Self {
        IterPredicates {
            split: query.split('+'),
            crates,
        }
    }
}

impl<'a> Iterator for IterPredicates<'a> {
    type Item = Result<Predicate>;

    fn next(&mut self) -> Option<Self::Item> {
        let predicate = self.split.next()?.trim();

        let (name, req) = if let Some((name, req)) = predicate.split_once(':') {
            match VersionReq::from_str(req) {
                Ok(req) => (name, Some(req)),
                Err(err) => return Some(Err(Error::new(err))),
            }
        } else {
            (predicate, None)
        };

        let crate_id = match self.crates.id(name) {
            Some(crate_id) => crate_id,
            None => return Some(Err(format_err!("no crate named {}", name))),
        };

        Some(Ok(Predicate { crate_id, req }))
    }
}
