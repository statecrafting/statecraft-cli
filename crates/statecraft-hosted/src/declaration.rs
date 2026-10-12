//! The project's committed hosted declaration, `.statecraft/hosted.json`
//! (spec 038 section 3.1 rule 3, section 5 O-1): the cell's origin, the
//! repository id the cell knows the project by, and the public native client
//! id and exact scopes the cell's manifest declares.
//!
//! Committed, so two checkouts of one commit present the same client; read,
//! never written, by this product.

use crate::{Error, Origin};
use serde::Deserialize;
use std::path::Path;

/// Where the declaration lives, relative to the project root.
pub const PATH: &str = ".statecraft/hosted.json";

/// The one schema version this reader accepts.
pub const SCHEMA_VERSION: u32 = 1;

/// The declaration as written.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Raw {
    schema_version: u32,
    cell: String,
    repository: String,
    client_id: String,
    scopes: Vec<String>,
}

/// A validated declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Declaration {
    /// The cell.
    pub cell: Origin,
    /// The repository id, lowercase hexadecimal.
    pub repository: String,
    /// The public native client id.
    pub client_id: String,
    /// The exact scopes requested, in declared order.
    pub scopes: Vec<String>,
}

impl Declaration {
    /// Read `<root>/.statecraft/hosted.json`. Absent, malformed, or missing
    /// the client id or the scopes refuses (exit 2), naming which.
    pub fn read(root: &Path) -> Result<Declaration, Error> {
        let path = root.join(PATH);
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(Error::Refused(format!(
                    "no hosted declaration: {PATH} is absent; it names the cell, the repository id, the native client id and the scopes"
                )));
            }
            Err(e) => return Err(Error::Refused(format!("{PATH} cannot be read: {e}"))),
        };
        Declaration::parse(&bytes)
    }

    /// Parse the declaration's bytes.
    pub fn parse(bytes: &[u8]) -> Result<Declaration, Error> {
        let raw: Raw = serde_json::from_slice(bytes)
            .map_err(|e| Error::Refused(format!("{PATH} is malformed: {e}")))?;
        if raw.schema_version != SCHEMA_VERSION {
            return Err(Error::Refused(format!(
                "{PATH} has schemaVersion {}, and this reader knows {SCHEMA_VERSION}",
                raw.schema_version
            )));
        }
        let cell = Origin::parse(&raw.cell)?;
        if raw.repository.len() != 64
            || !raw
                .repository
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(Error::Refused(format!(
                "{PATH}: repository is not a lowercase 64-character hexadecimal id"
            )));
        }
        if raw.client_id.trim().is_empty() {
            return Err(Error::Refused(format!("{PATH} declares no clientId")));
        }
        if raw.scopes.is_empty() {
            return Err(Error::Refused(format!("{PATH} declares no scopes")));
        }
        for s in &raw.scopes {
            if s.is_empty()
                || !s
                    .chars()
                    .all(|c| c.is_ascii_graphic() && c != '"' && c != '\\')
            {
                return Err(Error::Refused(format!(
                    "{PATH}: scope `{s}` is not a scope token"
                )));
            }
        }
        Ok(Declaration {
            cell,
            repository: raw.repository,
            client_id: raw.client_id,
            scopes: raw.scopes,
        })
    }
}
