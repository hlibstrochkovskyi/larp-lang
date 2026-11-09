use larp_source::{SourceFile, SourceId};
use std::io::Read;
use std::path::{Component, Path, PathBuf};

const MAX_MANIFEST_BYTES: u64 = 64 * 1024;

pub(super) struct Project {
    pub root: PathBuf,
    pub manifest: PathBuf,
    pub entry: PathBuf,
    pub name: String,
}

pub(super) struct Error {
    pub code: &'static str,
    pub message: String,
    pub source: Option<(Box<SourceFile>, std::ops::Range<usize>)>,
}

impl Error {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            source: None,
        }
    }
}

pub(super) fn load(selected: Option<&str>) -> Result<Project, Error> {
    let path = match selected {
        Some(path) => PathBuf::from(path),
        None => discover()?,
    };
    if path.file_name().is_none_or(|name| name != "larp.toml") {
        return Err(Error::new("LARP-J001", "manifest path must name larp.toml"));
    }
    let manifest = path.canonicalize().map_err(|error| {
        Error::new(
            "LARP-J001",
            format!("cannot open manifest {}: {error}", path.display()),
        )
    })?;
    if manifest.to_str().is_none() {
        return Err(Error::new(
            "LARP-J001",
            "manifest paths must be valid UTF-8",
        ));
    }
    let root = manifest
        .parent()
        .ok_or_else(|| Error::new("LARP-J001", "manifest must have a parent directory"))?
        .to_path_buf();
    // Inspect before opening so a nonregular file (for example a FIFO) cannot
    // turn manifest discovery into a blocking read under ordinary filesystem use.
    if !manifest
        .metadata()
        .map_err(|error| Error::new("LARP-J001", error.to_string()))?
        .is_file()
    {
        return Err(Error::new(
            "LARP-J001",
            format!("manifest {} is not a regular file", manifest.display()),
        ));
    }
    let file = std::fs::File::open(&manifest).map_err(|error| {
        Error::new(
            "LARP-J001",
            format!("cannot open manifest {}: {error}", manifest.display()),
        )
    })?;
    if !file
        .metadata()
        .map_err(|error| Error::new("LARP-J001", error.to_string()))?
        .is_file()
    {
        return Err(Error::new(
            "LARP-J001",
            format!("manifest {} is not a regular file", manifest.display()),
        ));
    }
    let mut bytes = Vec::new();
    file.take(MAX_MANIFEST_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| {
            Error::new(
                "LARP-J001",
                format!("cannot read manifest {}: {error}", manifest.display()),
            )
        })?;
    if bytes.len() as u64 > MAX_MANIFEST_BYTES {
        return Err(Error::new("LARP-J001", "manifest exceeds the 64 KiB limit"));
    }
    let text = String::from_utf8(bytes)
        .map_err(|_| Error::new("LARP-J001", "manifest must contain valid UTF-8"))?;
    let source = SourceFile::new(SourceId(0), manifest.to_str().unwrap(), text);
    let (name, request) = decode(source.text()).map_err(|(range, message)| Error {
        code: "LARP-J002",
        message,
        source: Some((
            Box::new(SourceFile::new(SourceId(0), source.name(), source.text())),
            range,
        )),
    })?;
    let entry = entry_path(&root, &request).map_err(|message| Error {
        code: "LARP-J003",
        message,
        source: Some((Box::new(source), 0..0)),
    })?;
    Ok(Project {
        root,
        manifest,
        entry,
        name,
    })
}

type FieldError = (std::ops::Range<usize>, String);

fn decode(text: &str) -> Result<(String, String), FieldError> {
    let parsed = toml::de::DeTable::parse(text).map_err(|error| {
        (
            error.span().unwrap_or(0..0),
            format!("invalid TOML: {}", error.message()),
        )
    })?;
    let table = parsed.get_ref();
    for (key, _) in table.iter() {
        if !matches!(key.get_ref().as_ref(), "schema" | "project") {
            return Err((
                key.span(),
                format!("unknown manifest field `{}`", key.get_ref()),
            ));
        }
    }
    let schema = table.get("schema");
    let integer = schema.and_then(|value| value.get_ref().as_integer());
    if integer.and_then(|value| i64::from_str_radix(value.as_str(), value.radix()).ok()) != Some(1)
    {
        return Err((
            schema.map_or(0..0, |value| value.span()),
            "manifest `schema` must be the integer 1".into(),
        ));
    }
    let project_value = table.get("project");
    let project = project_value
        .and_then(|value| value.get_ref().as_table())
        .ok_or_else(|| {
            (
                project_value.map_or(0..0, |value| value.span()),
                "manifest requires a [project] table".into(),
            )
        })?;
    for (key, _) in project.iter() {
        if !matches!(key.get_ref().as_ref(), "name" | "entry") {
            return Err((
                key.span(),
                format!("unknown manifest field `project.{}`", key.get_ref()),
            ));
        }
    }
    let name_value = project.get("name");
    let name = name_value
        .and_then(|value| value.get_ref().as_str())
        .ok_or_else(|| {
            (
                name_value.map_or(0..0, |value| value.span()),
                "`project.name` must be a string".into(),
            )
        })?;
    super::scaffold::validate_name(name).map_err(|message| {
        (
            name_value.unwrap().span(),
            format!("invalid project.name: {message}"),
        )
    })?;
    let entry_value = project.get("entry");
    let entry = entry_value
        .and_then(|value| value.get_ref().as_str())
        .ok_or_else(|| {
            (
                entry_value.map_or(0..0, |value| value.span()),
                "`project.entry` must be a string".into(),
            )
        })?;
    Ok((name.to_owned(), entry.to_owned()))
}

fn discover() -> Result<PathBuf, Error> {
    let mut directory = std::env::current_dir().map_err(|error| {
        Error::new(
            "LARP-J001",
            format!("cannot determine the current directory: {error}"),
        )
    })?;
    loop {
        let candidate = directory.join("larp.toml");
        match std::fs::symlink_metadata(&candidate) {
            Ok(_) => return Ok(candidate),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(Error::new(
                    "LARP-J001",
                    format!("cannot inspect manifest {}: {error}", candidate.display()),
                ));
            }
        }
        if !directory.pop() {
            return Err(Error::new(
                "LARP-J001",
                "cannot find larp.toml in the current directory or its ancestors",
            ));
        }
    }
}

fn entry_path(root: &Path, request: &str) -> Result<PathBuf, String> {
    let bytes = request.as_bytes();
    if request.is_empty()
        || request.starts_with('/')
        || request.contains(['\\', '\0'])
        || (bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':')
        || !request.ends_with(".larp")
    {
        return Err("project.entry must be a relative .larp path using '/' without drive prefixes, backslashes, or NUL".into());
    }
    let mut candidate = root.to_path_buf();
    for component in Path::new(request).components() {
        match component {
            Component::Normal(name) => candidate.push(name),
            Component::CurDir => {}
            Component::ParentDir => {
                if candidate == root || !candidate.pop() {
                    return Err("project.entry leaves the project root".into());
                }
            }
            _ => return Err("project.entry must be relative".into()),
        }
    }
    let entry = candidate.canonicalize().map_err(|error| {
        format!(
            "cannot resolve project.entry {}: {error}",
            candidate.display()
        )
    })?;
    if !entry.starts_with(root) {
        return Err("project.entry resolves outside the project root".into());
    }
    if entry.to_str().is_none() {
        return Err("project.entry paths must be valid UTF-8".into());
    }
    if !entry
        .metadata()
        .map_err(|error| error.to_string())?
        .is_file()
    {
        return Err("project.entry must be a regular source file".into());
    }
    Ok(entry)
}
