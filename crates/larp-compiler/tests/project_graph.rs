use std::collections::HashMap;

use larp_compiler::project::{
    self, MAX_IMPORT_EDGES, MAX_MODULES, MAX_SOURCE_BYTES, MAX_SOURCE_LINES, ModuleSource, Project,
    ProjectFailure,
};
use larp_source::SourceId;

fn source(identity: &str, text: &str) -> ModuleSource {
    ModuleSource {
        identity: identity.into(),
        name: format!("{identity}.larp"),
        text: text.into(),
    }
}

fn graph(entry: &str, libraries: &[(&str, &str)]) -> Result<Project, ProjectFailure> {
    let sources: HashMap<_, _> = libraries.iter().copied().collect();
    project::load(
        source("entry", entry),
        &mut |_importer: &str, request: &str| {
            sources
                .get(request)
                .map(|text| source(request, text))
                .ok_or_else(|| format!("Cannot resolve {request}"))
        },
    )
}

fn assert_labels_resolve(failure: &ProjectFailure) {
    assert!(!failure.diagnostics.is_empty());
    for diagnostic in &failure.diagnostics {
        for label in std::iter::once(&diagnostic.primary).chain(&diagnostic.related) {
            let file = &failure.sources[label.span.source().0 as usize];
            assert!(
                file.slice(label.span).is_some(),
                "Unresolvable label: {label:?}"
            );
        }
    }
}

#[test]
fn diamond_imports_have_deterministic_breadth_first_ids_and_one_unit_per_identity() {
    let entry = "import \"left\" as l; import \"right\" as r; main_character {}";
    let left = "import \"common\" as c; sidequest left() -> mf { return c::value(); }";
    let right = "import \"common\" as c; sidequest right() -> mf { return c::value(); }";
    let common = "highkey sidequest value() -> mf { return 49; }";
    let mut requests = Vec::new();
    let mut resolver = |importer: &str, request: &str| {
        requests.push((importer.to_string(), request.to_string()));
        Ok(source(
            request,
            match request {
                "left" => left,
                "right" => right,
                "common" => common,
                _ => unreachable!(),
            },
        ))
    };
    let project = project::load(source("entry", entry), &mut resolver).unwrap();
    assert_eq!(
        requests,
        [
            ("entry".into(), "left".into()),
            ("entry".into(), "right".into()),
            ("left".into(), "common".into()),
            ("right".into(), "common".into())
        ]
    );
    assert_eq!(project.entry().index(), 0);
    assert_eq!(project.sources().len(), 4);
    assert_eq!(
        project
            .modules()
            .iter()
            .map(|module| module.identity.as_str())
            .collect::<Vec<_>>(),
        ["entry", "left", "right", "common"]
    );
    for (index, module) in project.modules().iter().enumerate() {
        assert_eq!(module.source, SourceId(index as u32));
    }
    let shared = project.modules()[1].imports[0].target;
    assert_eq!(shared.index(), 3);
    assert_eq!(shared, project.modules()[2].imports[0].target);
    assert_eq!(project.module(shared).unwrap().unit.functions.len(), 1);
    assert!(project.module(shared).unwrap().unit.functions[0].public);
    assert_eq!(
        project.module(shared).unwrap().unit.functions[0]
            .item
            .name
            .span
            .source(),
        SourceId(3)
    );
    let import = &project.modules()[0].imports[0];
    assert_eq!(
        project.sources()[0].slice(import.path_span),
        Some("\"left\"")
    );
    assert_eq!(project.sources()[0].slice(import.alias.span), Some("l"));
    assert_eq!(
        project.sources()[0].slice(import.span),
        Some("import \"left\" as l;")
    );
}

#[test]
fn distinct_identities_keep_distinct_units_even_for_equal_contents_and_display_names() {
    let text = "highkey archetype Bro { init() {} }";
    let mut resolver = |_importer: &str, request: &str| {
        Ok(ModuleSource {
            identity: request.into(),
            name: "shared-display.larp".into(),
            text: text.into(),
        })
    };
    let project = project::load(
        source(
            "entry",
            "import \"one\" as a; import \"two\" as b; main_character {}",
        ),
        &mut resolver,
    )
    .unwrap();
    assert_eq!(project.sources().len(), 3);
    assert_eq!(project.sources()[1].name(), project.sources()[2].name());
    assert_ne!(project.modules()[1].source, project.modules()[2].source);
    assert_ne!(
        project.modules()[1].unit.classes[0].item.name.span.source(),
        project.modules()[2].unit.classes[0].item.name.span.source()
    );
}

#[test]
fn aliases_may_repeat_between_files_and_same_target_may_have_two_local_aliases() {
    let project = graph(
        "import \"lib\" as a; import \"lib\" as b; main_character {}",
        &[("lib", "import \"leaf\" as a;"), ("leaf", "")],
    )
    .unwrap();
    assert_eq!(project.sources().len(), 3);
    assert_eq!(
        project.modules()[0].imports[0].target,
        project.modules()[0].imports[1].target
    );
    assert_eq!(project.modules()[1].imports[0].alias.text, "a");
}

#[test]
fn duplicate_alias_error_retains_both_exact_alias_spans_without_resolving_second() {
    let mut count = 0;
    let mut resolver = |_importer: &str, request: &str| {
        count += 1;
        Ok(source(request, ""))
    };
    let failure = project::load(
        source(
            "entry",
            "import \"one\" as bro; import \"two\" as bro; main_character {}",
        ),
        &mut resolver,
    )
    .unwrap_err();
    assert_eq!(count, 1);
    assert_eq!(failure.diagnostics[0].code.id, "LARP-M003");
    let diagnostic = &failure.diagnostics[0];
    assert_eq!(
        failure.sources[0].slice(diagnostic.primary.span),
        Some("bro")
    );
    assert_eq!(
        failure.sources[0].slice(diagnostic.related[0].span),
        Some("bro")
    );
    assert!(diagnostic.related[0].span.range().start < diagnostic.primary.span.range().start);
    assert_labels_resolve(&failure);
}

#[test]
fn closing_cycle_edge_and_preceding_edges_retain_cross_file_spans() {
    let failure = graph(
        "import \"a\" as a; main_character {}",
        &[("a", "import \"b\" as b;"), ("b", "import \"a\" as a;")],
    )
    .unwrap_err();
    let diagnostic = &failure.diagnostics[0];
    assert_eq!(diagnostic.code.id, "LARP-M002");
    assert_eq!(diagnostic.primary.span.source(), SourceId(2));
    assert_eq!(
        failure.sources[2].slice(diagnostic.primary.span),
        Some("\"a\"")
    );
    assert_eq!(diagnostic.related.len(), 1);
    assert_eq!(diagnostic.related[0].span.source(), SourceId(1));
    assert!(diagnostic.message.contains("a -> b -> a"));
    assert_labels_resolve(&failure);
}

#[test]
fn root_self_import_reports_cycle_instead_of_library_entry_point() {
    let entry = "import \"self\" as myself; main_character {}";
    let failure = project::load(
        source("entry", entry),
        &mut |_importer: &str, _request: &str| Ok(source("entry", entry)),
    )
    .unwrap_err();
    assert_eq!(failure.diagnostics[0].code.id, "LARP-M002");
    assert_eq!(failure.sources.len(), 1);
    assert!(failure.diagnostics[0].related.is_empty());
    assert_labels_resolve(&failure);
}

#[test]
fn cycle_through_entry_is_not_mistaken_for_duplicate_main() {
    let entry = "import \"lib\" as lib; main_character {}";
    let mut resolver = |_importer: &str, request: &str| {
        Ok(match request {
            "lib" => source("lib", "import \"entry\" as entry;"),
            "entry" => source("entry", entry),
            _ => unreachable!(),
        })
    };
    let failure = project::load(source("entry", entry), &mut resolver).unwrap_err();
    assert_eq!(failure.diagnostics[0].code.id, "LARP-M002");
    assert_labels_resolve(&failure);
}

#[test]
fn library_main_is_reported_at_original_body_with_import_context() {
    let failure = graph(
        "import \"lib\" as lib; main_character {}",
        &[("lib", "main_character {}")],
    )
    .unwrap_err();
    let error = &failure.diagnostics[0];
    assert_eq!(error.code.id, "LARP-M004");
    assert_eq!(error.primary.span.source(), SourceId(1));
    assert_eq!(error.related[0].span.source(), SourceId(0));
    assert_labels_resolve(&failure);
}

#[test]
fn root_requires_main_but_empty_imported_library_is_valid() {
    let entry = "sidequest helper() -> unit {}";
    let failure = graph(entry, &[]).unwrap_err();
    assert_eq!(failure.diagnostics[0].code.id, "LARP-M004");
    assert_eq!(
        failure.diagnostics[0].primary.span.range(),
        entry.len()..entry.len()
    );
    assert_labels_resolve(&failure);
    assert!(
        graph(
            "import \"empty\" as empty; main_character {}",
            &[("empty", "")]
        )
        .is_ok()
    );
}

#[test]
fn parser_errors_keep_the_imported_source_and_its_nonzero_id() {
    let failure = graph(
        "import \"broken\" as bro; main_character {}",
        &[("broken", "highkey sidequest broken( { 🦀")],
    )
    .unwrap_err();
    assert_eq!(failure.sources.len(), 2);
    assert_eq!(failure.sources[1].text(), "highkey sidequest broken( { 🦀");
    assert_eq!(failure.diagnostics[0].primary.span.source(), SourceId(1));
    assert_labels_resolve(&failure);
}

#[test]
fn resolver_receives_decoded_request_and_authoritative_importer_identity() {
    let mut seen = Vec::new();
    let mut resolver = |importer: &str, request: &str| {
        seen.push((importer.to_string(), request.to_string()));
        if importer == "entry" {
            Ok(source("canonical:lib", "import \"leaf\" as leaf;"))
        } else {
            Err("Leaf is unavailable".into())
        }
    };
    let failure = project::load(
        source("entry", "import \"lib\\tfile\" as lib; main_character {}"),
        &mut resolver,
    )
    .unwrap_err();
    assert_eq!(
        seen,
        [
            ("entry".into(), "lib\tfile".into()),
            ("canonical:lib".into(), "leaf".into())
        ]
    );
    assert_eq!(failure.diagnostics[0].code.id, "LARP-M001");
    assert_eq!(
        failure.sources[1].slice(failure.diagnostics[0].primary.span),
        Some("\"leaf\"")
    );
    assert_labels_resolve(&failure);
}

#[test]
fn reused_identity_requires_consistent_text_and_display_name() {
    for change_name in [false, true] {
        let mut count = 0;
        let mut resolver = |_importer: &str, _request: &str| {
            count += 1;
            Ok(ModuleSource {
                identity: "same".into(),
                name: if change_name && count == 2 {
                    "changed.larp"
                } else {
                    "same.larp"
                }
                .into(),
                text: if !change_name && count == 2 {
                    "// changed"
                } else {
                    ""
                }
                .into(),
            })
        };
        let failure = project::load(
            source(
                "entry",
                "import \"a\" as a; import \"b\" as b; main_character {}",
            ),
            &mut resolver,
        )
        .unwrap_err();
        assert_eq!(failure.diagnostics[0].code.id, "LARP-M001");
        assert_eq!(failure.diagnostics[0].related[0].span.source(), SourceId(1));
        assert_labels_resolve(&failure);
    }
}

#[test]
fn empty_resolver_identity_or_display_name_is_an_error_with_retained_source() {
    for change_identity in [false, true] {
        let mut entry = source("entry", "main_character {}");
        if change_identity {
            entry.identity.clear();
        } else {
            entry.name.clear();
        }
        let failure = project::load(entry, &mut |_importer: &str, _request: &str| unreachable!())
            .unwrap_err();
        assert_eq!(failure.diagnostics[0].code.id, "LARP-M001");
        assert_eq!(failure.sources.len(), 1);
        assert_labels_resolve(&failure);
    }
}

#[test]
fn module_count_limit_accepts_boundary_and_retains_excess_file_without_parsing_it() {
    for modules in [MAX_MODULES, MAX_MODULES + 1] {
        let mut resolver = |_importer: &str, request: &str| {
            let index: usize = request.parse().unwrap();
            Ok(source(
                request,
                &if index + 1 < modules {
                    format!("import \"{}\" as next;", index + 1)
                } else if modules > MAX_MODULES {
                    "@".into() // The graph limit must precede this lexical failure.
                } else {
                    String::new()
                },
            ))
        };
        let result = project::load(
            source("entry", "import \"1\" as next; main_character {}"),
            &mut resolver,
        );
        if modules == MAX_MODULES {
            assert_eq!(result.unwrap().modules().len(), modules);
        } else {
            let failure = result.unwrap_err();
            assert_eq!(failure.diagnostics[0].code.id, "LARP-M005");
            assert_eq!(failure.sources.len(), modules);
            assert_labels_resolve(&failure);
        }
    }
}

#[test]
fn aggregate_byte_limit_counts_utf8_bytes_once_per_unique_source() {
    let entry = "import \"lib\" as lib; import \"lib\" as same; main_character {}";
    let prefix = "// 🦀";
    let at_limit = format!(
        "{prefix}{}",
        "x".repeat(MAX_SOURCE_BYTES - entry.len() - prefix.len())
    );
    assert!(graph(entry, &[("lib", &at_limit)]).is_ok());
    let excessive = format!("{at_limit}x");
    let failure = graph(entry, &[("lib", &excessive)]).unwrap_err();
    assert_eq!(failure.diagnostics[0].code.id, "LARP-M005");
    assert_eq!(failure.sources.len(), 2);
    assert_labels_resolve(&failure);
}

#[test]
fn aggregate_line_limit_counts_empty_last_lines_and_duplicate_imports_once() {
    let entry = "import \"lib\" as lib; import \"lib\" as same; main_character {}";
    let at_limit = "\n".repeat(MAX_SOURCE_LINES - 2);
    assert!(graph(entry, &[("lib", &at_limit)]).is_ok());
    let excessive = format!("{at_limit}\n");
    let failure = graph(entry, &[("lib", &excessive)]).unwrap_err();
    assert_eq!(failure.diagnostics[0].code.id, "LARP-M005");
    assert_labels_resolve(&failure);
}

#[test]
fn import_edge_limit_counts_repeated_targets_but_stops_before_excess_resolution() {
    for edges in [MAX_IMPORT_EDGES, MAX_IMPORT_EDGES + 1] {
        let mut entry = String::new();
        for index in 0..edges {
            entry.push_str(&format!("import \"lib\" as lib{index};\n"));
        }
        entry.push_str("main_character {}");
        let mut count = 0;
        let result = project::load(
            source("entry", &entry),
            &mut |_importer: &str, _request: &str| {
                count += 1;
                Ok(source("lib", ""))
            },
        );
        assert_eq!(count, MAX_IMPORT_EDGES);
        if edges == MAX_IMPORT_EDGES {
            let project = result.unwrap();
            assert_eq!(project.sources().len(), 2);
            assert_eq!(project.modules()[0].imports.len(), edges);
        } else {
            let failure = result.unwrap_err();
            assert_eq!(failure.diagnostics[0].code.id, "LARP-M005");
            assert_labels_resolve(&failure);
        }
    }
}
