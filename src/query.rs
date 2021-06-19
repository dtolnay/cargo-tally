use crate::cratemap::CrateMap;
use crate::user::UserQuery;
use anyhow::{bail, format_err, Error, Result};
use cargo_tally::arena::Slice;
use cargo_tally::id::QueryId;
use cargo_tally::version::VersionReq;
use cargo_tally::{Predicate, Query};
use ref_cast::RefCast;
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
    let mut predicates = Vec::new();

    for predicate in IterPredicates::new(string, crates) {
        let predicate = predicate?;
        match predicate {
            RawPredicate::Crate(predicate) => predicates.push(predicate),
            RawPredicate::User(username) => {
                let user_id = match crates.users.get(username) {
                    Some(user_id) => user_id,
                    None => {
                        let kind = if username.is_team() { "team" } else { "user" };
                        bail!("no crates owned by {} @{}", kind, username);
                    }
                };
                predicates.extend(
                    crates
                        .owners
                        .get(user_id)
                        .map(Vec::as_slice)
                        .unwrap_or_default()
                        .iter()
                        .map(|&crate_id| Predicate {
                            crate_id,
                            req: None,
                        }),
                );
            }
        }
    }

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
            match predicate {
                RawPredicate::Crate(predicate) => {
                    let original_name = self.crates.name(predicate.crate_id).unwrap();
                    formatter.write_str(original_name)?;
                    if let Some(req) = predicate.req {
                        write!(formatter, ":{}", req)?;
                    }
                }
                RawPredicate::User(username) => {
                    let (username, _user_id) = self.crates.users.get_key_value(username).unwrap();
                    write!(formatter, "@{}", username)?;
                }
            }
        }
        Ok(())
    }
}

enum RawPredicate<'a> {
    Crate(Predicate),
    User(&'a UserQuery),
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
    type Item = Result<RawPredicate<'a>>;

    fn next(&mut self) -> Option<Self::Item> {
        let predicate = self.split.next()?.trim();

        if let Some(username) = predicate.strip_prefix('@') {
            return Some(Ok(RawPredicate::User(UserQuery::ref_cast(username))));
        }

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

        Some(Ok(RawPredicate::Crate(Predicate { crate_id, req })))
    }
}
