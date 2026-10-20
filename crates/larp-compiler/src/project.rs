//! Deterministic, source-preserving import graphs supplied by an explicit resolver.
//!
//! This layer performs no filesystem or host I/O. Resolver identities define
//! deduplication; path policy belongs to the resolver, not to the graph builder.

use std::collections::HashMap;

use larp_diagnostics::{Diagnostic, Label};
use larp_source::{SourceFile, SourceId, Span};
use larp_syntax::{Name, SourceUnit};

pub const MAX_MODULES: usize = 64;
pub const MAX_IMPORT_EDGES: usize = 4096;
pub const MAX_SOURCE_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_SOURCE_LINES: usize = 131_072;

#[derive(Debug)]
pub struct ModuleSource {
    pub identity: String,
    pub name: String,
    pub text: String,
}

/// Resolve requests against an importer's opaque identity. Returning the same
/// identity requires identical display names and source text for this load.
pub trait Resolver {
    fn resolve(&mut self, importer: &str, request: &str) -> Result<ModuleSource, String>;
}

impl<F> Resolver for F
where
    F: FnMut(&str, &str) -> Result<ModuleSource, String>,
{
    fn resolve(&mut self, importer: &str, request: &str) -> Result<ModuleSource, String> {
        self(importer, request)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct ModuleId(usize);

impl ModuleId {
    pub fn index(self) -> usize {
        self.0
    }
}

#[derive(Debug)]
pub struct ImportBinding {
    pub alias: Name,
    pub target: ModuleId,
    pub span: Span,
    pub path_span: Span,
}

#[derive(Debug)]
pub struct Module {
    pub identity: String,
    pub source: SourceId,
    pub unit: SourceUnit,
    pub imports: Vec<ImportBinding>,
}

#[derive(Debug)]
pub struct Project {
    sources: Vec<SourceFile>,
    modules: Vec<Module>,
    entry: ModuleId,
}

impl Project {
    pub fn sources(&self) -> &[SourceFile] {
        &self.sources
    }

    pub fn modules(&self) -> &[Module] {
        &self.modules
    }

    pub fn entry(&self) -> ModuleId {
        self.entry
    }

    pub fn module(&self, id: ModuleId) -> Option<&Module> {
        self.modules.get(id.0)
    }
}

#[derive(Debug)]
pub struct ProjectFailure {
    /// Original loaded files, including the file responsible for a parse error.
    pub sources: Vec<SourceFile>,
    pub diagnostics: Vec<Diagnostic>,
}

struct Builder {
    sources: Vec<SourceFile>,
    modules: Vec<Module>,
    identities: HashMap<String, ModuleId>,
    discovered_from: Vec<Option<Span>>,
    source_bytes: usize,
    source_lines: usize,
}

impl Builder {
    fn add(
        &mut self,
        source: ModuleSource,
        import_site: Option<Span>,
    ) -> Result<ModuleId, Vec<Diagnostic>> {
        if let Some(&id) = self.identities.get(&source.identity) {
            let original = &self.sources[id.0];
            if original.name() != source.name || original.text() != source.text {
                let span = import_site.unwrap_or_else(|| original.span(0..0).unwrap());
                let mut error = Diagnostic::error(
                    "LARP-M001",
                    span,
                    format!(
                        "Resolver returned inconsistent source for identity {:?}",
                        source.identity
                    ),
                );
                error.related.push(Label {
                    span: original.span(0..0).unwrap(),
                    message: "This identity was first loaded here".into(),
                });
                return Err(vec![error]);
            }
            return Ok(id);
        }

        let id = ModuleId(self.sources.len());
        // Successful graphs contain at most MAX_MODULES files; one failed load
        // may retain an additional resolved file so its diagnostics are usable.
        let file = SourceFile::new(SourceId(id.0 as u32), source.name, source.text);
        let span = import_site.unwrap_or_else(|| file.span(0..0).unwrap());
        self.source_bytes = self.source_bytes.saturating_add(file.text().len());
        self.source_lines = self
            .source_lines
            .saturating_add(file.text().bytes().filter(|&byte| byte == b'\n').count() + 1);
        self.sources.push(file);
        if source.identity.is_empty() || self.sources[id.0].name().is_empty() {
            return Err(vec![Diagnostic::error(
                "LARP-M001",
                span,
                "Resolver identities and source display names must be nonempty",
            )]);
        }
        let exceeded = if self.sources.len() > MAX_MODULES {
            Some(format!(
                "Project exceeds {MAX_MODULES} distinct source modules"
            ))
        } else if self.source_bytes > MAX_SOURCE_BYTES {
            Some(format!(
                "Project exceeds {MAX_SOURCE_BYTES} aggregate UTF-8 source bytes"
            ))
        } else if self.source_lines > MAX_SOURCE_LINES {
            Some(format!(
                "Project exceeds {MAX_SOURCE_LINES} aggregate source lines"
            ))
        } else {
            None
        };
        if let Some(message) = exceeded {
            return Err(vec![Diagnostic::error("LARP-M005", span, message)]);
        }
        let unit = larp_syntax::parse_unit(&self.sources[id.0])?;
        self.identities.insert(source.identity.clone(), id);
        self.discovered_from.push(import_site);
        self.modules.push(Module {
            identity: source.identity,
            source: SourceId(id.0 as u32),
            unit,
            imports: Vec::new(),
        });
        Ok(id)
    }

    fn failure(self, diagnostics: Vec<Diagnostic>) -> ProjectFailure {
        ProjectFailure {
            sources: self.sources,
            diagnostics,
        }
    }
}

/// Parse each distinct source once in breadth-first import order, then validate
/// aliases, acyclicity, and entry-point placement. No semantic linking occurs.
pub fn load(
    entry: ModuleSource,
    resolver: &mut (impl Resolver + ?Sized),
) -> Result<Project, ProjectFailure> {
    let mut builder = Builder {
        sources: Vec::new(),
        modules: Vec::new(),
        identities: HashMap::new(),
        discovered_from: Vec::new(),
        source_bytes: 0,
        source_lines: 0,
    };
    let entry = match builder.add(entry, None) {
        Ok(entry) => entry,
        Err(errors) => return Err(builder.failure(errors)),
    };
    let mut cursor = 0;
    let mut edge_count = 0;
    while cursor < builder.modules.len() {
        let mut aliases = HashMap::new();
        for import_index in 0..builder.modules[cursor].unit.imports.len() {
            let import = &builder.modules[cursor].unit.imports[import_index];
            if let Some(first) = aliases.insert(import.alias.text.clone(), import.alias.span) {
                let mut error = Diagnostic::error(
                    "LARP-M003",
                    import.alias.span,
                    format!("Duplicate module alias {}", import.alias.text),
                );
                error.related.push(Label {
                    span: first,
                    message: "Alias first declared here".into(),
                });
                return Err(builder.failure(vec![error]));
            }
            edge_count += 1;
            if edge_count > MAX_IMPORT_EDGES {
                let error = Diagnostic::error(
                    "LARP-M005",
                    import.span,
                    format!("Project exceeds {MAX_IMPORT_EDGES} import edges"),
                );
                return Err(builder.failure(vec![error]));
            }
            let alias = import.alias.clone();
            let span = import.span;
            let path_span = import.path_span;
            let resolved = resolver.resolve(&builder.modules[cursor].identity, &import.path);
            let source = match resolved {
                Ok(source) => source,
                Err(message) => {
                    let error = Diagnostic::error("LARP-M001", path_span, message);
                    return Err(builder.failure(vec![error]));
                }
            };
            let target = match builder.add(source, Some(path_span)) {
                Ok(target) => target,
                Err(errors) => return Err(builder.failure(errors)),
            };
            builder.modules[cursor].imports.push(ImportBinding {
                alias,
                target,
                span,
                path_span,
            });
        }
        cursor += 1;
    }

    // Cycles take precedence over library-main errors, including a root importing
    // itself. The explicit DFS stack does not consume the host call stack.
    if let Some(error) = cycle(&builder.modules, entry) {
        return Err(builder.failure(vec![error]));
    }
    for (index, module) in builder.modules.iter().enumerate() {
        let error = if index == entry.0 && module.unit.main.is_none() {
            let source = &builder.sources[index];
            Some(Diagnostic::error(
                "LARP-M004",
                source
                    .span(source.text().len()..source.text().len())
                    .unwrap(),
                "The entry module requires exactly one main_character block",
            ))
        } else if index != entry.0 {
            module.unit.main.as_ref().map(|main| {
                let mut error = Diagnostic::error(
                    "LARP-M004",
                    main.span,
                    "An imported module cannot declare main_character",
                );
                if let Some(span) = builder.discovered_from[index] {
                    error.related.push(Label {
                        span,
                        message: "Module imported here".into(),
                    });
                }
                error
            })
        } else {
            None
        };
        if let Some(error) = error {
            return Err(builder.failure(vec![error]));
        }
    }
    Ok(Project {
        sources: builder.sources,
        modules: builder.modules,
        entry,
    })
}

fn cycle(modules: &[Module], entry: ModuleId) -> Option<Diagnostic> {
    let mut colors = vec![0u8; modules.len()];
    colors[entry.0] = 1;
    let mut stack = vec![(entry, 0usize)];
    while let Some(&(id, next)) = stack.last() {
        if next == modules[id.0].imports.len() {
            colors[id.0] = 2;
            stack.pop();
            continue;
        }
        stack.last_mut().unwrap().1 += 1;
        let edge = &modules[id.0].imports[next];
        match colors[edge.target.0] {
            0 => {
                colors[edge.target.0] = 1;
                stack.push((edge.target, 0));
            }
            1 => {
                let start = stack
                    .iter()
                    .position(|&(node, _)| node == edge.target)
                    .unwrap();
                let mut names: Vec<_> = stack[start..]
                    .iter()
                    .map(|&(node, _)| modules[node.0].identity.as_str())
                    .collect();
                names.push(&modules[edge.target.0].identity);
                let mut error = Diagnostic::error(
                    "LARP-M002",
                    edge.path_span,
                    format!("Import cycle: {}", names.join(" -> ")),
                );
                for &(node, next_edge) in &stack[start..stack.len() - 1] {
                    let preceding = &modules[node.0].imports[next_edge - 1];
                    error.related.push(Label {
                        span: preceding.path_span,
                        message: "Earlier import in this cycle".into(),
                    });
                }
                return Some(error);
            }
            _ => {}
        }
    }
    None
}
