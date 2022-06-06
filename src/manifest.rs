use miette::{Diagnostic, NamedSource, Result, SourceOffset, SourceSpan};
use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

#[derive(Deserialize, Serialize)]
pub struct Manifest<'a> {
    pub name: &'a str,
    pub version: &'a str,
}

#[derive(Error, Debug, Diagnostic)]
pub struct SerdeError {
    serde: serde_json::Error,
    #[source_code]
    src: NamedSource,
    #[label("Around here")]
    bad_bit: SourceSpan,
}

impl fmt::Display for SerdeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.serde.fmt(f)
    }
}

impl<'a> Manifest<'a> {
    pub fn parse<'b>(content: &'a str, file_name: &'b str) -> Result<Manifest<'a>, SerdeError> {
        serde_json::from_str(content).map_err(|source| SerdeError {
            src: NamedSource::new(file_name, content.to_owned()),
            bad_bit: SourceSpan::new(
                SourceOffset::from_location(&content, source.line(), source.column()),
                SourceOffset::from(1),
            ),
            serde: source,
        })
    }
}
