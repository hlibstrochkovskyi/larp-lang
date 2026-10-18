//! Repeat real LARP trial bodies while retaining one root identity per VM run.

use larp_compiler::compile;
use larp_source::{SourceFile, SourceId};
use larp_vm::{ExecutionStats, Limits, execute_with_observers};
use std::collections::BTreeMap;
use std::io::{self, Write};
use std::path::Path;
use std::time::Instant;

const ITERATIONS: usize = 10_000;
const SAMPLES: usize = 7;
const FUEL: u64 = 100_000_000;
const BEGIN: &str = "    // Begin trial.\n";
const END: &str = "    // End trial.";

struct Program {
    name: &'static str,
    pair: &'static str,
    expected: &'static [u8],
    scenario: bool,
}

fn repeat_trial(source: &str, inspections: bool) -> Result<String, String> {
    let (prefix, rest) = source.split_once(BEGIN).ok_or("Missing begin marker")?;
    let (trial, suffix) = rest.split_once(END).ok_or("Missing end marker")?;
    if rest.contains(BEGIN) || suffix.contains(END) {
        return Err("Duplicate trial markers".into());
    }
    let mut repeated =
        format!("{prefix}{BEGIN}    npc loop (mf comparisonIteration in 0..{ITERATIONS}) {{\n");
    for line in trial.lines() {
        if inspections || !line.trim_start().starts_with("palantir(") {
            repeated.push_str("    ");
            repeated.push_str(line);
            repeated.push('\n');
        }
    }
    repeated.push_str("    }\n");
    repeated.push_str(END);
    repeated.push_str(suffix);
    Ok(repeated)
}

fn verify_stats(stats: &ExecutionStats) -> Result<(), String> {
    if stats.allocations != stats.reclaimed {
        return Err("Successful execution retained managed heap objects".into());
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let programs = [
        Program {
            name: "config-tuning",
            pair: "configuration",
            expected: b"8\n1000\n",
            scenario: true,
        },
        Program {
            name: "config-tuning-explicit",
            pair: "configuration",
            expected: b"8\n1000\n",
            scenario: false,
        },
        Program {
            name: "game-training",
            pair: "game",
            expected: b"50010\n0\n",
            scenario: true,
        },
        Program {
            name: "game-training-explicit",
            pair: "game",
            expected: b"50010\n0\n",
            scenario: false,
        },
    ];
    let mut paired = BTreeMap::new();
    let mut csv = io::BufWriter::new(io::stdout().lock());
    writeln!(
        csv,
        "program,pair,inspection,sample,iterations,elapsed_ns,instructions,allocations,collections,reclaimed,peak_live_objects"
    )?;
    for program in programs {
        let path = root.join("examples").join(format!("{}.larp", program.name));
        let original = std::fs::read_to_string(&path)?;
        for inspections in if program.scenario {
            &[false, true][..]
        } else {
            &[false][..]
        } {
            let text = repeat_trial(&original, *inspections)?;
            let source = SourceFile::new(SourceId(0), path.display().to_string(), text);
            let compilation = compile(&source).map_err(|errors| {
                errors
                    .iter()
                    .map(|error| larp_diagnostics::render(error, &source))
                    .collect::<String>()
            })?;
            let limits = Limits {
                instructions: FUEL,
                ..Default::default()
            };
            let mut expected_stats = None;
            // Compile once, then warm up and measure seven independent VM runs.
            for sample in 0..=SAMPLES {
                let mut output = Vec::new();
                let start = Instant::now();
                let stats = execute_with_observers(
                    &compilation.bytecode,
                    &mut output,
                    &mut io::sink(),
                    limits,
                )
                .map_err(|error| larp_diagnostics::render(&error, &source))?;
                let elapsed = start.elapsed().as_nanos();
                verify_stats(&stats)?;
                if output != program.expected {
                    return Err(
                        format!("Unexpected result from {}: {output:?}", program.name).into(),
                    );
                }
                if let Some(previous) = paired.insert(program.pair, output.clone())
                    && previous != output
                {
                    return Err(format!("Pair {} disagrees", program.pair).into());
                }
                let deterministic = (
                    stats.instructions,
                    stats.allocations,
                    stats.collections,
                    stats.reclaimed,
                    stats.peak_live_objects,
                );
                if let Some(previous) = expected_stats.replace(deterministic)
                    && previous != deterministic
                {
                    return Err("VM statistics changed across identical runs".into());
                }
                if sample != 0 {
                    writeln!(
                        csv,
                        "{},{},{},{},{},{},{},{},{},{},{}",
                        program.name,
                        program.pair,
                        if *inspections { "enabled" } else { "disabled" },
                        sample,
                        ITERATIONS,
                        elapsed,
                        stats.instructions,
                        stats.allocations,
                        stats.collections,
                        stats.reclaimed,
                        stats.peak_live_objects
                    )?;
                }
            }
        }
    }
    csv.flush()?;
    Ok(())
}
