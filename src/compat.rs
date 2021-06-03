#![allow(dead_code)]

use semver::{Error, Version, VersionReq};

pub(crate) fn version(string: &str) -> Result<Version, Error> {
    match Version::parse(string) {
        Ok(version) => Ok(version),
        Err(err) => {
            let corrected = match string {
                "0.0.1-001" => "0.0.1-1",
                "0.3.0-alpha.01" => "0.3.0-alpha.1",
                "0.4.0-alpha.00" => "0.4.0-alpha.0",
                "0.4.0-alpha.01" => "0.4.0-alpha.1",
                _ => return Err(err),
            };
            Ok(Version::parse(corrected).unwrap())
        }
    }
}

pub(crate) fn version_req(string: &str) -> Result<VersionReq, Error> {
    match VersionReq::parse(string) {
        Ok(req) => Ok(req),
        Err(err) => {
            let corrected = match string {
                "^0-.11.0" => "^0.11.0",
                "^0.1-alpha.0" => "^0.1.0-alpha.0",
                "^0.51-oldsyn" => "^0.51.0-oldsyn",
                "~2.0-2.2" => ">=2.0, <=2.2",
                _ => return Err(err),
            };
            Ok(VersionReq::parse(corrected).unwrap())
        }
    }
}
