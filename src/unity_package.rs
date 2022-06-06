use flate2::read::GzDecoder;
use miette::{Context, IntoDiagnostic, Result};
use std::{
    ascii,
    borrow::Borrow,
    collections::HashMap,
    ffi::OsString,
    io::{self, prelude::*, SeekFrom},
};
use tar::{Archive, Entries, Entry};
use tempfile::{spooled_tempfile, SpooledTempFile};
use tracing::{debug_span, warn};

const MAX_ASSET_MEM: usize = 32 * 1024 * 1024;

#[derive(Default)]
struct AssetParts {
    asset: Option<SpooledTempFile>,
    metadata: Option<SpooledTempFile>,
}

enum ItemStatus {
    AwaitingPath(AssetParts),
    KnownPath(String),
    Error,
}

impl Default for ItemStatus {
    fn default() -> Self {
        ItemStatus::AwaitingPath(AssetParts::default())
    }
}

pub struct Package<R>
where
    R: Read,
{
    tar: Archive<R>,
    paths: HashMap<OsString, ItemStatus>,
}

impl<F> Package<GzDecoder<F>>
where
    F: Read,
{
    pub fn new(reader: F) -> Self {
        Self {
            tar: Archive::new(GzDecoder::new(reader)),
            paths: HashMap::new(),
        }
    }
}

impl<R> Package<R>
where
    R: Read,
{
    pub fn entries(&mut self) -> Result<PackageEntries<'_, R>> {
        Ok(PackageEntries {
            entries: self
                .tar
                .entries()
                .into_diagnostic()
                .wrap_err("Failed to read tar header")?,
            paths: &mut self.paths,
            late: None,
        })
    }
}

pub enum PackageEntryContent<'a, R>
where
    R: Read,
{
    Buffer(SpooledTempFile),
    Direct(Box<Entry<'a, R>>),
}

impl<'a, R> Read for PackageEntryContent<'a, R>
where
    R: Read,
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            PackageEntryContent::Buffer(temp) => temp.read(buf),
            PackageEntryContent::Direct(r) => r.read(buf),
        }
    }
}

pub struct PackageEntry<'a, R>
where
    R: Read,
{
    pub path: String,
    pub content: PackageEntryContent<'a, R>,
}

pub struct PackageEntries<'a, R>
where
    R: Read,
{
    paths: &'a mut HashMap<OsString, ItemStatus>,
    entries: Entries<'a, R>,
    late: Option<(String, SpooledTempFile)>,
}

fn buffer_reader<R>(reader: &mut R) -> Result<SpooledTempFile>
where
    R: Read,
{
    let mut temp = spooled_tempfile(MAX_ASSET_MEM);

    io::copy(reader, &mut temp)
        .into_diagnostic()
        .wrap_err("Failed to extract")?;

    temp.seek(SeekFrom::Start(0))
        .into_diagnostic()
        .wrap_err("Failed to reset temp file")?;

    Ok(temp)
}

impl<'a, R> Iterator for PackageEntries<'a, R>
where
    R: Read,
{
    type Item = Result<PackageEntry<'a, R>>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some((path, content)) = self.late.take() {
            return Some(Ok(PackageEntry {
                path,
                content: PackageEntryContent::Buffer(content),
            }));
        }

        loop {
            let mut entry = match self
                .entries
                .next()?
                .into_diagnostic()
                .wrap_err("Failed to read entry header")
            {
                Ok(entry) => entry,
                Err(error) => return Some(Err(error)),
            };

            if !entry.header().entry_type().is_file() {
                continue;
            }

            let entry_path = match entry.path().into_diagnostic().wrap_err_with(|| {
                format!(
                    "Failed to read package entry path {:?}",
                    entry
                        .path_bytes()
                        .iter()
                        .copied()
                        .flat_map(ascii::escape_default)
                        .map(char::from)
                        .collect::<String>()
                )
            }) {
                Ok(entry_path) => entry_path.into_owned(),
                Err(error) => return Some(Err(error)),
            };

            let _ = debug_span!("Inspecting entry {entry}", entry = ?entry_path).enter();

            let mut components = entry_path.components();
            let (id, part) = match (components.next(), components.next(), components.next()) {
                (Some(id), Some(part), None) => (
                    id.as_os_str().to_owned(),
                    part.as_os_str().to_string_lossy(),
                ),
                _ => {
                    warn!("Skipping entry because it is not expected.");
                    continue;
                }
            };

            if part == "preview.png" {
                continue;
            }

            match part.borrow() {
                "asset" | "asset.meta" => match self.paths.entry(id).or_default() {
                    ItemStatus::AwaitingPath(ref mut parts) => {
                        let temp = match buffer_reader(&mut entry)
                            .wrap_err_with(|| format!("Failed to buffer {entry_path:?}"))
                        {
                            Ok(temp) => temp,
                            Err(error) => return Some(Err(error)),
                        };

                        match part.borrow() {
                            "asset" => parts.asset = Some(temp),
                            "asset.meta" => parts.metadata = Some(temp),
                            _ => unreachable!(),
                        }
                    }
                    ItemStatus::KnownPath(path) => {
                        let path = match part.borrow() {
                            "asset" => path.clone(),
                            "asset.meta" => format!("{path}.meta"),
                            _ => unreachable!(),
                        };
                        return Some(Ok(PackageEntry {
                            path,
                            content: PackageEntryContent::Direct(Box::new(entry)),
                        }));
                    }
                    ItemStatus::Error => {}
                },
                "pathname" => {
                    let mut name = String::new();

                    if let Err(error) = entry
                        .read_to_string(&mut name)
                        .into_diagnostic()
                        .wrap_err_with(|| format!("Failed to read asset name from {entry_path:?}"))
                    {
                        self.paths.insert(id, ItemStatus::Error);
                        return Some(Err(error));
                    }

                    if name.starts_with("Assets/") {
                        name.drain(.."Assets/".len());
                    } else {
                        warn!("Ignoring non-asset path {name:?}");
                    }

                    if let Some(ItemStatus::AwaitingPath(unknowns)) = self
                        .paths
                        .insert(id, ItemStatus::KnownPath(name.to_owned()))
                    {
                        match unknowns {
                            AssetParts {
                                asset: Some(asset),
                                metadata: Some(metadata),
                            } => {
                                self.late = Some((format!("{name}.meta"), metadata));
                                return Some(Ok(PackageEntry {
                                    path: name,
                                    content: PackageEntryContent::Buffer(asset),
                                }));
                            }
                            AssetParts {
                                asset: Some(asset),
                                metadata: None,
                            } => {
                                return Some(Ok(PackageEntry {
                                    path: name,
                                    content: PackageEntryContent::Buffer(asset),
                                }));
                            }
                            AssetParts {
                                asset: None,
                                metadata: Some(metadata),
                            } => {
                                return Some(Ok(PackageEntry {
                                    path: format!("{name}.meta"),
                                    content: PackageEntryContent::Buffer(metadata),
                                }));
                            }
                            AssetParts {
                                asset: None,
                                metadata: None,
                            } => {}
                        }
                    }
                }
                _ => {
                    warn!("Skipping unrecognized asset component {part:?}");
                    continue;
                }
            }
        }
    }
}
