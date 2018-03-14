#[macro_use]
extern crate serde_derive;

#[macro_use]
extern crate failure_derive;

extern crate chrono;
extern crate failure;
extern crate fnv;
extern crate reqwest;
extern crate semver;
extern crate serde;
extern crate serde_json;
extern crate url;

use chrono::Utc;

use failure::Error;

use fnv::FnvHashMap as Map;

use semver::{Version, VersionReq};

use serde::de::{Deserialize, DeserializeOwned, Deserializer};
use serde_json::Value;

use url::Url;

use std::env;
use std::fmt::{self, Display};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

const PER_PAGE: usize = 100;
const RETRIES: usize = 32;

pub type DateTime = chrono::DateTime<Utc>;

#[derive(Deserialize, Debug)]
pub struct IndexPage {
    pub crates: Vec<IndexCrate>,
    pub meta: Meta,
}

#[derive(Deserialize, Debug)]
pub struct IndexCrate {
    pub name: String,
}

#[derive(Deserialize, Debug)]
pub struct Meta {
    pub total: usize,
}

#[derive(Deserialize, Debug)]
pub struct Crate {
    #[serde(rename = "crate")] pub index: IndexCrate,
    pub versions: Vec<CrateVersion>,
}

#[derive(Deserialize, Debug)]
pub struct Dependencies {
    pub dependencies: Vec<Dependency>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Dependency {
    #[serde(rename = "crate_id")] pub name: String,
    pub kind: DependencyKind,
    pub req: VersionReq,
    pub optional: bool,
    pub default_features: bool,
    pub features: Vec<String>,
}

#[derive(Deserialize, PartialEq, Clone, Copy, Debug)]
#[serde(rename_all = "lowercase")]
pub enum DependencyKind {
    Normal,
    Build,
    Dev,
}

#[derive(Deserialize, Debug)]
pub struct CrateVersion {
    pub num: Version,
    pub created_at: DateTime,
    pub features: Map<String, Vec<Feature>>,
}

#[derive(Clone, Debug)]
pub enum Feature {
    Current(String),
    Dependency(String, String),
}

impl<'de> Deserialize<'de> for Feature {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let mut s = String::deserialize(deserializer)?;
        Ok(match s.find('/') {
            Some(slash) => {
                let feature = s[slash + 1..].to_owned();
                s.truncate(slash);
                Feature::Dependency(s, feature)
            }
            None => Feature::Current(s),
        })
    }
}

pub fn cache_index(n: usize) -> Result<IndexPage, Error> {
    let endpoint = format!("/api/v1/crates?page={}", n);
    let location = format!("index/{}.json", n);
    cache(endpoint, location)
}

pub fn cache_crate(name: &str) -> Result<Crate, Error> {
    let endpoint = format!("/api/v1/crates/{}", name);
    let location = format!("crate/{}.json", name);
    cache(endpoint, location)
}

pub fn cache_dependencies(name: &str, num: &Version) -> Result<Dependencies, Error> {
    let endpoint = format!("/api/v1/crates/{}/{}/dependencies", name, num);
    let location = format!("dependencies/{}/{}.json", name, num);
    cache(endpoint, location)
}

pub fn num_pages() -> Result<usize, Error> {
    let total = cache_index(1)?.meta.total;
    Ok((total + PER_PAGE - 1) / PER_PAGE)
}

pub fn total_crates() -> Result<usize, Error> {
    Ok(cache_index(1)?.meta.total)
}

#[derive(Debug, Fail)]
#[fail(display = "download did not return success: {}", url)]
struct DownloadError {
    url: Url,
}

#[derive(Debug, Fail)]
struct FileNotFoundError {
    path: PathBuf,
}

impl Display for FileNotFoundError {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(
            formatter,
            "file not found, run `cargo tally --init` to download: {}",
            self.path.display()
        )
    }
}

impl Display for DependencyKind {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            DependencyKind::Normal => write!(formatter, "normal"),
            DependencyKind::Build => write!(formatter, "build"),
            DependencyKind::Dev => write!(formatter, "dev"),
        }
    }
}

impl Display for Feature {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Feature::Current(ref feature) => write!(formatter, "{}", feature),
            Feature::Dependency(ref name, ref feature) => write!(formatter, "{}/{}", name, feature),
        }
    }
}

fn cache<U, P, T>(endpoint: U, location: P) -> Result<T, Error>
where
    U: AsRef<str>,
    P: AsRef<Path>,
    T: DeserializeOwned,
{
    let location = location.as_ref();
    assert!(location.is_relative());

    let mut path = PathBuf::from("tally");
    path.push(location);

    if path.exists() {
        let mut file = File::open(&path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        if let Ok(de) = serde_json::from_str(&contents) {
            return Ok(de);
        }
    }

    if env::var("ALLOW_DOWNLOAD").is_err() {
        return Err(Error::from(FileNotFoundError { path }));
    }

    let parent = path.parent().unwrap();
    fs::create_dir_all(parent)?;

    let data: Value = download(endpoint.as_ref())?;
    let de = T::deserialize(&data)?;

    let j = serde_json::to_string_pretty(&data)?;
    let mut file = File::create(&path)?;
    file.write_all(j.as_bytes())?;

    Ok(de)
}

fn download<T>(endpoint: &str) -> Result<T, Error>
where
    T: DeserializeOwned,
{
    let mut url = Url::parse("https://crates.io").unwrap().join(endpoint)?;
    url.query_pairs_mut()
        .append_pair("per_page", &PER_PAGE.to_string());

    let mut resp = retry(|| {
        let resp = reqwest::get(url.clone())?;
        if !resp.status().is_success() {
            return Err(Error::from(DownloadError { url: url.clone() }));
        }
        Ok(resp)
    })?;

    let data = resp.json()?;
    Ok(data)
}

#[cfg(feature = "cargo-clippy")]
fn retry<F, R>(_f: F) -> R
where
    F: Fn() -> R,
{
    let _ = RETRIES;
    unimplemented!()
}

#[cfg(not(feature = "cargo-clippy"))]
fn retry<F, T, E>(f: F) -> Result<T, E>
where
    F: Fn() -> Result<T, E>,
{
    #[allow(dead_code)]
    enum StaticAssert {
        False = false as isize,
        True = (RETRIES > 0) as isize,
    }

    for i in 1usize.. {
        let result = f();
        if result.is_ok() || i == RETRIES {
            return result;
        }
    }

    unreachable!()
}
