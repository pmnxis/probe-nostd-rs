use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use serde::Deserialize;
use std::collections::HashMap;
use std::fmt::Write as FmtWrite;
use std::fs;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// YAML deserialization structs (build-time only, NOT the runtime types)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct YamlChipFamily {
    #[allow(dead_code)]
    name: String,
    #[serde(default)]
    variants: Vec<YamlChip>,
    #[serde(default)]
    flash_algorithms: Vec<YamlFlashAlgorithm>,
}

#[derive(Deserialize)]
struct YamlChip {
    name: String,
    #[serde(default)]
    cores: Vec<YamlCore>,
    #[serde(default)]
    memory_map: Vec<YamlMemoryRegion>,
    #[serde(default)]
    flash_algorithms: Vec<String>,
}

#[derive(Deserialize)]
struct YamlCore {
    name: String,
    #[serde(rename = "type")]
    core_type: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
enum YamlMemoryRegion {
    Ram(YamlMemRegionInner),
    Nvm(YamlMemRegionInner),
    Generic(YamlMemRegionInner),
}

#[derive(Deserialize)]
struct YamlMemRegionInner {
    #[serde(default)]
    name: Option<String>,
    range: YamlRange,
}

#[derive(Deserialize)]
struct YamlRange {
    start: u64,
    end: u64,
}

#[derive(Deserialize)]
struct YamlFlashAlgorithm {
    name: String,
    #[serde(default)]
    instructions: Option<String>,
    #[serde(default)]
    pc_init: Option<u64>,
    #[serde(default)]
    pc_uninit: Option<u64>,
    #[serde(default)]
    pc_program_page: Option<u64>,
    #[serde(default)]
    pc_erase_sector: Option<u64>,
    #[serde(default)]
    pc_erase_all: Option<u64>,
    #[serde(default)]
    data_section_offset: Option<u64>,
    #[serde(default)]
    load_address: Option<u64>,
    #[serde(default)]
    stack_size: Option<u32>,
    #[serde(default)]
    flash_properties: Option<YamlFlashProperties>,
}

#[derive(Deserialize)]
struct YamlFlashProperties {
    address_range: YamlRange,
    #[serde(default = "default_page_size")]
    page_size: u32,
    #[serde(default = "default_erased_byte")]
    erased_byte_value: u8,
    #[serde(default = "default_program_timeout")]
    program_page_timeout: u32,
    #[serde(default = "default_erase_timeout")]
    erase_sector_timeout: u32,
    #[serde(default)]
    sectors: Vec<YamlSector>,
}

#[derive(Deserialize)]
struct YamlSector {
    size: u64,
    address: u64,
}

fn default_page_size() -> u32 {
    256
}
fn default_erased_byte() -> u8 {
    0xff
}
fn default_program_timeout() -> u32 {
    300
}
fn default_erase_timeout() -> u32 {
    3000
}

// ---------------------------------------------------------------------------
// Feature name <-> YAML file mapping
// ---------------------------------------------------------------------------

struct YamlSource {
    feature_name: String,
    yaml_path: PathBuf,
}

fn yaml_filename_to_feature(filename: &str) -> String {
    let mut name = filename
        .strip_suffix(".yaml")
        .unwrap_or(filename)
        .to_string();
    for suffix in &["_Series", "-Series"] {
        if let Some(stripped) = name.strip_suffix(suffix) {
            name = stripped.to_string();
        }
    }
    let mut feat = String::new();
    for c in name.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            feat.push(c);
        } else {
            feat.push('-');
        }
    }
    // collapse consecutive dashes
    while feat.contains("--") {
        feat = feat.replace("--", "-");
    }
    let feat = feat.trim_matches('-').to_string();
    format!("target-{feat}")
}

fn feature_to_env_var(feature: &str) -> String {
    format!("CARGO_FEATURE_{}", feature.to_uppercase().replace('-', "_"))
}

fn sanitize_rust_ident(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            out.push(c);
        } else {
            out.push('_');
        }
    }
    if out.starts_with(|c: char| c.is_ascii_digit()) {
        out.insert(0, '_');
    }
    if out.is_empty() {
        out = "_unnamed".to_string();
    }
    out
}

fn map_core_type(s: &str) -> &'static str {
    match s.to_lowercase().as_str() {
        "armv6m" => "CoreType::Armv6m",
        "armv7m" => "CoreType::Armv7m",
        "armv7em" => "CoreType::Armv7em",
        "armv8m" => "CoreType::Armv8m",
        "armv7a" => "CoreType::Armv7a",
        "armv8a" => "CoreType::Armv8a",
        "riscv" => "CoreType::Riscv",
        "xtensa" => "CoreType::Xtensa",
        _ => "CoreType::Armv7m", // fallback
    }
}

// ---------------------------------------------------------------------------
// Code generation
// ---------------------------------------------------------------------------

fn generate_algo_code(
    algo: &YamlFlashAlgorithm,
    algo_idx: usize,
    mod_name: &str,
    out_dir: &Path,
) -> Option<String> {
    let instructions_b64 = algo.instructions.as_ref()?;
    let decoded = BASE64.decode(instructions_b64).ok()?;

    // Write binary blob
    let bin_filename = format!("{}_{}.bin", mod_name, algo_idx);
    let bin_path = out_dir.join("algos").join(&bin_filename);
    fs::write(&bin_path, &decoded).ok()?;

    let fp = algo.flash_properties.as_ref();

    let mut code = String::new();
    writeln!(
        code,
        "    static ALGO_{idx}_INSTRUCTIONS: &[u8] = include_bytes!(concat!(env!(\"OUT_DIR\"), \"/algos/{bin}\"));",
        idx = algo_idx,
        bin = bin_filename,
    )
    .unwrap();
    writeln!(code).unwrap();

    let sectors_code = if let Some(fp) = fp {
        let mut s = String::from("&[\n");
        for sec in &fp.sectors {
            writeln!(
                s,
                "            SectorDescription {{ size: {}, address: {} }},",
                sec.size, sec.address
            )
            .unwrap();
        }
        s.push_str("        ]");
        s
    } else {
        "&[]".to_string()
    };

    let (ar_start, ar_end, page_size, erased, prog_to, erase_to) = if let Some(fp) = fp {
        (
            fp.address_range.start,
            fp.address_range.end,
            fp.page_size,
            fp.erased_byte_value,
            fp.program_page_timeout,
            fp.erase_sector_timeout,
        )
    } else {
        (0, 0, 256, 0xff, 300, 3000)
    };

    writeln!(
        code,
        "    pub static ALGO_{idx}: FlashAlgoDef = FlashAlgoDef {{",
        idx = algo_idx
    )
    .unwrap();
    writeln!(code, "        name: {:?},", algo.name).unwrap();
    writeln!(
        code,
        "        instructions: ALGO_{idx}_INSTRUCTIONS,",
        idx = algo_idx
    )
    .unwrap();
    writeln!(
        code,
        "        load_address: {},",
        algo.load_address.unwrap_or(0)
    )
    .unwrap();
    writeln!(
        code,
        "        data_section_offset: {},",
        algo.data_section_offset.unwrap_or(0)
    )
    .unwrap();
    match algo.pc_init {
        Some(v) => writeln!(code, "        pc_init: Some({v}),").unwrap(),
        None => writeln!(code, "        pc_init: None,").unwrap(),
    }
    match algo.pc_uninit {
        Some(v) => writeln!(code, "        pc_uninit: Some({v}),").unwrap(),
        None => writeln!(code, "        pc_uninit: None,").unwrap(),
    }
    writeln!(
        code,
        "        pc_program_page: {},",
        algo.pc_program_page.unwrap_or(0)
    )
    .unwrap();
    writeln!(
        code,
        "        pc_erase_sector: {},",
        algo.pc_erase_sector.unwrap_or(0)
    )
    .unwrap();
    match algo.pc_erase_all {
        Some(v) => writeln!(code, "        pc_erase_all: Some({v}),").unwrap(),
        None => writeln!(code, "        pc_erase_all: None,").unwrap(),
    }
    writeln!(
        code,
        "        stack_size: {},",
        algo.stack_size.unwrap_or(256)
    )
    .unwrap();
    writeln!(code, "        flash_properties: FlashProperties {{").unwrap();
    writeln!(code, "            address_range_start: {ar_start},").unwrap();
    writeln!(code, "            address_range_end: {ar_end},").unwrap();
    writeln!(code, "            page_size: {page_size},").unwrap();
    writeln!(code, "            erased_byte_value: {erased},").unwrap();
    writeln!(code, "            program_page_timeout: {prog_to},").unwrap();
    writeln!(code, "            erase_sector_timeout: {erase_to},").unwrap();
    writeln!(code, "            sectors: {sectors_code},").unwrap();
    writeln!(code, "        }},").unwrap();
    writeln!(code, "    }};").unwrap();

    Some(code)
}

fn generate_chip_code(
    chip: &YamlChip,
    family_algos: &HashMap<String, usize>,
    _mod_name: &str,
) -> String {
    let chip_ident = sanitize_rust_ident(&chip.name.to_uppercase());
    let mut code = String::new();

    // Cores
    writeln!(code, "    static {chip_ident}_CORES: &[CoreDef] = &[").unwrap();
    for core in &chip.cores {
        let ct = map_core_type(&core.core_type);
        writeln!(
            code,
            "        CoreDef {{ name: {:?}, core_type: {ct} }},",
            core.name
        )
        .unwrap();
    }
    writeln!(code, "    ];").unwrap();
    writeln!(code).unwrap();

    // Memory map
    writeln!(
        code,
        "    static {chip_ident}_MEMORY_MAP: &[MemoryRegion] = &["
    )
    .unwrap();
    for region in &chip.memory_map {
        match region {
            YamlMemoryRegion::Ram(inner) => {
                let name = inner.name.as_deref().unwrap_or("Ram");
                writeln!(
                    code,
                    "        MemoryRegion::Ram {{ name: {:?}, start: {}, end: {} }},",
                    name, inner.range.start, inner.range.end,
                )
                .unwrap();
            }
            YamlMemoryRegion::Nvm(inner) => {
                let name = inner.name.as_deref().unwrap_or("Nvm");
                writeln!(
                    code,
                    "        MemoryRegion::Nvm {{ name: {:?}, start: {}, end: {} }},",
                    name, inner.range.start, inner.range.end,
                )
                .unwrap();
            }
            YamlMemoryRegion::Generic(inner) => {
                let name = inner.name.as_deref().unwrap_or("Generic");
                writeln!(
                    code,
                    "        MemoryRegion::Generic {{ name: {:?}, start: {}, end: {} }},",
                    name, inner.range.start, inner.range.end,
                )
                .unwrap();
            }
        }
    }
    writeln!(code, "    ];").unwrap();
    writeln!(code).unwrap();

    // Flash algorithm references
    writeln!(code, "    static {chip_ident}_ALGOS: &[&FlashAlgoDef] = &[").unwrap();
    for algo_name in &chip.flash_algorithms {
        if let Some(&idx) = family_algos.get(algo_name) {
            writeln!(code, "        &ALGO_{idx},").unwrap();
        }
    }
    writeln!(code, "    ];").unwrap();
    writeln!(code).unwrap();

    // ChipDef
    writeln!(code, "    pub static {chip_ident}: ChipDef = ChipDef {{").unwrap();
    writeln!(code, "        name: {:?},", chip.name).unwrap();
    writeln!(code, "        cores: {chip_ident}_CORES,").unwrap();
    writeln!(code, "        memory_map: {chip_ident}_MEMORY_MAP,").unwrap();
    writeln!(code, "        flash_algorithms: {chip_ident}_ALGOS,").unwrap();
    writeln!(code, "    }};").unwrap();

    code
}

fn main() {
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let workspace_root = manifest_dir
        .parent()
        .expect("failed to find workspace root");

    // Create algos dir
    let algos_dir = out_dir.join("algos");
    fs::create_dir_all(&algos_dir).unwrap();

    // Collect YAML sources
    let yaml_dirs = [
        workspace_root.join("probe-rs").join("targets"),
        workspace_root.join("probe-rs-espressif").join("targets"),
    ];

    for dir in &yaml_dirs {
        println!("cargo:rerun-if-changed={}", dir.display());
    }

    let mut sources: Vec<YamlSource> = Vec::new();
    for dir in &yaml_dirs {
        if !dir.is_dir() {
            continue;
        }
        let mut entries: Vec<_> = fs::read_dir(dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .map(|ext| ext == "yaml")
                    .unwrap_or(false)
            })
            .collect();
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let filename = entry.file_name().to_string_lossy().to_string();
            let feature = yaml_filename_to_feature(&filename);
            sources.push(YamlSource {
                feature_name: feature,
                yaml_path: entry.path(),
            });
        }
    }

    // Filter to enabled features
    let enabled: Vec<&YamlSource> = sources
        .iter()
        .filter(|s| std::env::var(feature_to_env_var(&s.feature_name)).is_ok())
        .collect();

    // Generate code
    let mut output = String::new();
    writeln!(output, "// Auto-generated by build.rs -- do not edit").unwrap();
    writeln!(output).unwrap();
    writeln!(output, "#[allow(unused_imports)]").unwrap();
    writeln!(output, "use crate::types::*;").unwrap();
    writeln!(output).unwrap();

    // Track all chip refs for ALL_TARGETS
    let mut all_chip_refs: Vec<(String, String, String)> = Vec::new(); // (feature, mod_name, chip_ident)

    for source in &enabled {
        let yaml_content = match fs::read_to_string(&source.yaml_path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Warning: cannot read {}: {}", source.yaml_path.display(), e);
                continue;
            }
        };

        let family: YamlChipFamily = match serde_yaml::from_str(&yaml_content) {
            Ok(f) => f,
            Err(e) => {
                eprintln!(
                    "Warning: cannot parse {}: {}",
                    source.yaml_path.display(),
                    e
                );
                continue;
            }
        };

        let mod_name = sanitize_rust_ident(&source.feature_name.replace('-', "_"));

        writeln!(output, "#[cfg(feature = {:?})]", source.feature_name).unwrap();
        writeln!(output, "mod {mod_name} {{").unwrap();
        writeln!(output, "    use super::*;").unwrap();
        writeln!(output).unwrap();

        // Generate flash algorithms
        let mut algo_map: HashMap<String, usize> = HashMap::new();
        for (idx, algo) in family.flash_algorithms.iter().enumerate() {
            if let Some(code) = generate_algo_code(algo, idx, &mod_name, &out_dir) {
                algo_map.insert(algo.name.clone(), idx);
                writeln!(output, "{code}").unwrap();
            }
        }

        // Generate chip variants
        for chip in &family.variants {
            let chip_code = generate_chip_code(chip, &algo_map, &mod_name);
            writeln!(output, "{chip_code}").unwrap();

            let chip_ident = sanitize_rust_ident(&chip.name.to_uppercase());
            all_chip_refs.push((source.feature_name.clone(), mod_name.clone(), chip_ident));
        }

        writeln!(output, "}}").unwrap();
        writeln!(output).unwrap();
    }

    // Generate ALL_TARGETS
    writeln!(output, "pub static ALL_TARGETS: &[&ChipDef] = &[").unwrap();
    for (feature, mod_name, chip_ident) in &all_chip_refs {
        writeln!(output, "    #[cfg(feature = {feature:?})]").unwrap();
        writeln!(output, "    &{mod_name}::{chip_ident},").unwrap();
    }
    writeln!(output, "];").unwrap();

    let targets_path = out_dir.join("targets.rs");
    fs::write(&targets_path, &output).unwrap();
}
