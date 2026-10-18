//! One fresh-process resource probe; the Python driver measures process RSS.

use larp_source::{SourceFile, SourceId};
use larp_vm::{Limits, execute_with_stats};
use std::fmt::Write;
use std::time::Instant;

fn json_string(value: &str) -> String {
    let mut output = String::from("\"");
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            c if c.is_control() => write!(output, "\\u{:04x}", u32::from(c)).unwrap(),
            c => output.push(c),
        }
    }
    output.push('"');
    output
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    let [path, fuel, frames, mode] = arguments.as_slice() else {
        return Err("Usage: resource_probe FILE FUEL FRAMES normal|stress".into());
    };
    let gc_stress = match mode.as_str() {
        "normal" => false,
        "stress" => true,
        _ => return Err("Expected normal or stress GC mode".into()),
    };
    let limits = Limits {
        instructions: fuel.parse()?,
        call_frames: frames.parse()?,
        gc_stress,
    };
    let source = SourceFile::new(SourceId(0), path, std::fs::read_to_string(path)?);
    let compilation = larp_compiler::compile(&source).map_err(|diagnostics| {
        diagnostics
            .iter()
            .map(|diagnostic| larp_diagnostics::render(diagnostic, &source))
            .collect::<String>()
    })?;
    let mut output = Vec::new();
    let start = Instant::now();
    let execution = execute_with_stats(&compilation.bytecode, &mut output, limits);
    let elapsed = start.elapsed().as_nanos();
    let output = json_string(std::str::from_utf8(&output)?);
    match execution {
        Ok(stats) => println!(
            "{{\"schema_version\":1,\"status\":\"ok\",\"code\":null,\"vm_elapsed_ns\":{elapsed},\"output\":{output},\"stats\":{{\"instructions\":{},\"allocations\":{},\"collections\":{},\"reclaimed\":{},\"peak_live_objects\":{}}}}}",
            stats.instructions,
            stats.allocations,
            stats.collections,
            stats.reclaimed,
            stats.peak_live_objects
        ),
        Err(diagnostic) => println!(
            "{{\"schema_version\":1,\"status\":\"runtime_error\",\"code\":{},\"message\":{},\"vm_elapsed_ns\":{elapsed},\"output\":{output},\"stats\":null}}",
            json_string(diagnostic.code.id),
            json_string(&diagnostic.message)
        ),
    }
    Ok(())
}
