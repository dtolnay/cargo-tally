use crate::arena::Slice;
use crate::cratemap::CrateMap;
use crate::id::{CrateId, QueryId};
use crate::version::VersionReq;
use anyhow::{bail, Result};
use std::convert::TryFrom;
use std::fmt::{self, Display};
use std::str::FromStr;

#[derive(Copy, Clone, Debug)]
pub struct Query {
    pub id: QueryId,
    pub predicates: Slice<Predicate>,
}

#[derive(Copy, Clone, Debug)]
pub struct Predicate {
    pub crate_id: CrateId,
    pub req: Option<VersionReq>,
}

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
    let mut predicates = Vec::new();

    for predicate in string.split('+') {
        let predicate = predicate.trim();

        let (name, req) = if let Some((name, req)) = predicate.split_once(':') {
            let req = VersionReq::from_str(req)?;
            (name, Some(req))
        } else {
            (predicate, None)
        };

        let crate_id = match crates.id_normalized(name) {
            Some(crate_id) => crate_id,
            None => bail!("no crate named {}", name),
        };

        predicates.push(Predicate { crate_id, req });
    }

    Ok(Slice::new(&predicates))
}

impl Query {
    pub fn display<'a>(&'a self, crates: &'a CrateMap) -> DisplayQuery<'a> {
        DisplayQuery {
            query: self,
            crates,
        }
    }
}

pub struct DisplayQuery<'a> {
    query: &'a Query,
    crates: &'a CrateMap,
}

impl<'a> Display for DisplayQuery<'a> {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        for (i, pred) in self.query.predicates.iter().enumerate() {
            if i > 0 {
                formatter.write_str(" or ")?;
            }
            formatter.write_str(self.crates.name(pred.crate_id).unwrap())?;
            if let Some(req) = pred.req {
                formatter.write_str(":")?;
                write!(formatter, "{}", req)?;
            }
        }
        Ok(())
    }
}
