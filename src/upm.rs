use miette::{Context, IntoDiagnostic, Result};
use std::io::{self, prelude::*};
use zip::{write::FileOptions, ZipWriter};

pub struct PackageBuilder<F>
where
    F: Write + Seek,
{
    zip: ZipWriter<F>,
    prefix: String,
}

impl<F> PackageBuilder<F>
where
    F: Write + Seek,
{
    pub fn new(writer: F, prefix: String) -> Self {
        Self {
            zip: ZipWriter::new(writer),
            prefix,
        }
    }

    pub fn append<R>(&mut self, path: &str, reader: &mut R) -> Result<()>
    where
        R: Read,
    {
        self.zip
            .start_file(format!("{}/{}", self.prefix, path), FileOptions::default())
            .into_diagnostic()
            .wrap_err("Failed to create zip header")?;
        io::copy(reader, &mut self.zip)
            .into_diagnostic()
            .wrap_err("Failed to write data")?;

        Ok(())
    }

    pub fn finish(mut self) -> Result<()> {
        self.zip
            .finish()
            .into_diagnostic()
            .wrap_err("Failed to finish zip archive")?;
        Ok(())
    }
}
