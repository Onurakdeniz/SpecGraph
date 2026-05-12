use serde::{Deserialize, Serialize};
use serde_json::json;
use sg_adapter_api::{CODE_INDEXER_ADAPTER_ID, SOURCE_TRUST_OBSERVATION, TRUST_STATE_OBSERVED};
use sg_codegraph::{
    code_file_node_id, code_import_node_id, code_route_node_id, code_symbol_node_id, SourceLocation,
};
use sg_model::{Edge, Finding, FindingSeverity, GraphDelta, Node};
use sg_validation::{CORE_VALIDATOR_VERSION, VALIDATOR_ADAPTER_TRUST};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

pub const CODE_INDEX_CACHE_SCHEMA_VERSION: &str = "specgraph.code-index-cache/v1";
pub const CODE_INDEXER_CONTRACT_SCHEMA_VERSION: &str = "specgraph.semantic-code-indexer/v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeIndexObservation {
    pub file: String,
    pub language: String,
    #[serde(default)]
    pub provenance: IndexerProvenance,
    #[serde(default)]
    pub framework: Option<String>,
    #[serde(default)]
    pub generated: bool,
    #[serde(default)]
    pub symbols: Vec<CodeSymbolObservation>,
    #[serde(default)]
    pub imports: Vec<CodeImportObservation>,
    #[serde(default)]
    pub routes: Vec<CodeRouteObservation>,
    #[serde(default)]
    pub config_accesses: Vec<ConfigAccessObservation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeSymbolObservation {
    pub name: String,
    pub kind: String,
    #[serde(default)]
    pub visibility: Option<String>,
    #[serde(default)]
    pub line: Option<u32>,
    #[serde(default)]
    pub location: Option<SourceLocation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeImportObservation {
    pub imported: String,
    #[serde(default)]
    pub specifier: Option<String>,
    #[serde(default)]
    pub location: Option<SourceLocation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeRouteObservation {
    pub method: String,
    pub path: String,
    #[serde(default)]
    pub handler_symbol: Option<String>,
    #[serde(default)]
    pub framework: Option<String>,
    #[serde(default)]
    pub location: Option<SourceLocation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigAccessObservation {
    pub name: String,
    pub kind: String,
    pub access_pattern: String,
    #[serde(default)]
    pub location: Option<SourceLocation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexerProvenance {
    pub contract_schema_version: String,
    pub language_id: String,
    pub indexer_version: String,
    pub supported_file_extensions: Vec<String>,
    pub content_hash: String,
    pub deterministic: bool,
    #[serde(default)]
    pub language_pack: Option<String>,
}

impl Default for IndexerProvenance {
    fn default() -> Self {
        Self {
            contract_schema_version: CODE_INDEXER_CONTRACT_SCHEMA_VERSION.to_string(),
            language_id: "unknown".to_string(),
            indexer_version: "unknown".to_string(),
            supported_file_extensions: Vec::new(),
            content_hash: "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
                .to_string(),
            deterministic: true,
            language_pack: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeIndexCacheEntry {
    pub schema_version: String,
    pub file: String,
    pub content_hash: String,
    pub ontology_version: String,
    pub language_pack: Option<String>,
    pub indexer_language: String,
    pub indexer_version: String,
    pub observation: CodeIndexObservation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedCodeIndexObservation {
    pub observation: CodeIndexObservation,
    pub cache_hit: bool,
    pub cache_path: PathBuf,
}

pub trait SemanticCodeIndexer {
    fn language_id(&self) -> &'static str;
    fn indexer_version(&self) -> &'static str;
    fn supported_file_extensions(&self) -> &'static [&'static str];
    fn index_file_semantic(&self, path: &str, source: &str) -> CodeIndexObservation;

    fn provenance(&self, source: &str, language_pack: Option<&str>) -> IndexerProvenance {
        IndexerProvenance {
            contract_schema_version: CODE_INDEXER_CONTRACT_SCHEMA_VERSION.to_string(),
            language_id: self.language_id().to_string(),
            indexer_version: self.indexer_version().to_string(),
            supported_file_extensions: self
                .supported_file_extensions()
                .iter()
                .map(|value| (*value).to_string())
                .collect(),
            content_hash: content_hash(source),
            deterministic: true,
            language_pack: language_pack.map(ToOwned::to_owned),
        }
    }
}

pub trait CodeIndexer {
    fn language(&self) -> &'static str;
    fn indexer_version(&self) -> &'static str {
        "legacy"
    }
    fn supported_file_extensions(&self) -> &'static [&'static str] {
        &[]
    }
    fn index_file(&self, path: &str, source: &str) -> Vec<CodeIndexObservation>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct LightweightCodeIndexer;

impl CodeIndexer for LightweightCodeIndexer {
    fn language(&self) -> &'static str {
        "multi"
    }

    fn index_file(&self, path: &str, source: &str) -> Vec<CodeIndexObservation> {
        vec![index_source_file(path, source)]
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct FrameworkAwareCodeIndexer;

impl CodeIndexer for FrameworkAwareCodeIndexer {
    fn language(&self) -> &'static str {
        "multi-framework"
    }

    fn index_file(&self, path: &str, source: &str) -> Vec<CodeIndexObservation> {
        vec![index_source_file(path, source)]
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct RustSemanticIndexer;

#[derive(Debug, Clone, Copy, Default)]
pub struct TypeScriptSemanticIndexer;

#[derive(Debug, Clone, Copy, Default)]
pub struct PythonSemanticIndexer;

impl SemanticCodeIndexer for RustSemanticIndexer {
    fn language_id(&self) -> &'static str {
        "rust"
    }

    fn indexer_version(&self) -> &'static str {
        "semantic-rust/v1"
    }

    fn supported_file_extensions(&self) -> &'static [&'static str] {
        &["rs"]
    }

    fn index_file_semantic(&self, path: &str, source: &str) -> CodeIndexObservation {
        semantic_index_source_file(path, "rust", self.provenance(source, None), source)
    }
}

impl SemanticCodeIndexer for TypeScriptSemanticIndexer {
    fn language_id(&self) -> &'static str {
        "typescript-javascript"
    }

    fn indexer_version(&self) -> &'static str {
        "semantic-typescript-javascript/v1"
    }

    fn supported_file_extensions(&self) -> &'static [&'static str] {
        &["ts", "tsx", "js", "jsx", "mjs", "cjs"]
    }

    fn index_file_semantic(&self, path: &str, source: &str) -> CodeIndexObservation {
        let language = language_for_path(path).unwrap_or("typescript");
        semantic_index_source_file(path, language, self.provenance(source, None), source)
    }
}

impl SemanticCodeIndexer for PythonSemanticIndexer {
    fn language_id(&self) -> &'static str {
        "python"
    }

    fn indexer_version(&self) -> &'static str {
        "semantic-python/v1"
    }

    fn supported_file_extensions(&self) -> &'static [&'static str] {
        &["py"]
    }

    fn index_file_semantic(&self, path: &str, source: &str) -> CodeIndexObservation {
        semantic_index_source_file(path, "python", self.provenance(source, None), source)
    }
}

impl<T: SemanticCodeIndexer> CodeIndexer for T {
    fn language(&self) -> &'static str {
        self.language_id()
    }

    fn indexer_version(&self) -> &'static str {
        SemanticCodeIndexer::indexer_version(self)
    }

    fn supported_file_extensions(&self) -> &'static [&'static str] {
        SemanticCodeIndexer::supported_file_extensions(self)
    }

    fn index_file(&self, path: &str, source: &str) -> Vec<CodeIndexObservation> {
        vec![self.index_file_semantic(path, source)]
    }
}

pub fn index_source_file(path: &str, source: &str) -> CodeIndexObservation {
    let language = language_for_path(path).unwrap_or("unknown").to_string();
    let indexer = semantic_indexer_for_language(&language);
    let provenance = indexer
        .as_ref()
        .map(|indexer| indexer.provenance(source, None))
        .unwrap_or_else(|| IndexerProvenance {
            language_id: language.clone(),
            indexer_version: "semantic-generic/v1".to_string(),
            supported_file_extensions: Path::new(path)
                .extension()
                .and_then(|value| value.to_str())
                .map(|value| vec![value.to_string()])
                .unwrap_or_default(),
            content_hash: content_hash(source),
            ..IndexerProvenance::default()
        });
    semantic_index_source_file(path, &language, provenance, source)
}

fn semantic_index_source_file(
    path: &str,
    language: &str,
    provenance: IndexerProvenance,
    source: &str,
) -> CodeIndexObservation {
    let framework = framework_for_source(path, language, source).map(str::to_string);
    let symbols = extract_symbols(path, language, source);
    let imports = extract_imports(path, language, source);
    let routes = extract_routes(path, language, framework.as_deref(), source);
    let config_accesses = extract_config_accesses(path, language, source);
    CodeIndexObservation {
        file: path.to_string(),
        language: language.to_string(),
        provenance,
        framework,
        generated: is_generated_source(path, source),
        symbols,
        imports,
        routes,
        config_accesses,
    }
}

fn semantic_indexer_for_language(language: &str) -> Option<Box<dyn SemanticCodeIndexer>> {
    match language {
        "rust" => Some(Box::new(RustSemanticIndexer)),
        "typescript" | "javascript" => Some(Box::new(TypeScriptSemanticIndexer)),
        "python" => Some(Box::new(PythonSemanticIndexer)),
        _ => None,
    }
}

pub fn index_source_file_with_cache(
    root: &Path,
    path: &str,
    source: &str,
    ontology_version: &str,
    language_pack: Option<&str>,
) -> Result<CachedCodeIndexObservation, String> {
    let language = language_for_path(path).unwrap_or("unknown").to_string();
    let indexer = semantic_indexer_for_language(&language);
    let mut observation = indexer
        .as_ref()
        .map(|indexer| {
            let mut observation = indexer.index_file_semantic(path, source);
            observation.provenance = indexer.provenance(source, language_pack);
            observation
        })
        .unwrap_or_else(|| {
            let mut observation = index_source_file(path, source);
            observation.provenance.language_pack = language_pack.map(ToOwned::to_owned);
            observation
        });
    let cache_path = code_index_cache_path(
        root,
        path,
        &observation.provenance.content_hash,
        &observation.provenance.indexer_version,
        ontology_version,
        language_pack,
    );
    if let Ok(bytes) = fs::read(&cache_path) {
        if let Ok(entry) = serde_json::from_slice::<CodeIndexCacheEntry>(&bytes) {
            if entry.file == path
                && entry.content_hash == observation.provenance.content_hash
                && entry.ontology_version == ontology_version
                && entry.language_pack.as_deref() == language_pack
                && entry.indexer_version == observation.provenance.indexer_version
            {
                return Ok(CachedCodeIndexObservation {
                    observation: entry.observation,
                    cache_hit: true,
                    cache_path,
                });
            }
        }
    }

    let entry = CodeIndexCacheEntry {
        schema_version: CODE_INDEX_CACHE_SCHEMA_VERSION.to_string(),
        file: path.to_string(),
        content_hash: observation.provenance.content_hash.clone(),
        ontology_version: ontology_version.to_string(),
        language_pack: language_pack.map(ToOwned::to_owned),
        indexer_language: observation.provenance.language_id.clone(),
        indexer_version: observation.provenance.indexer_version.clone(),
        observation: observation.clone(),
    };
    if let Some(parent) = cache_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create code index cache directory {}: {error}",
                parent.display()
            )
        })?;
    }
    let bytes = serde_json::to_vec_pretty(&entry)
        .map_err(|error| format!("failed to serialize code index cache entry: {error}"))?;
    fs::write(&cache_path, bytes).map_err(|error| {
        format!(
            "failed to write code index cache {}: {error}",
            cache_path.display()
        )
    })?;
    observation = entry.observation;
    Ok(CachedCodeIndexObservation {
        observation,
        cache_hit: false,
        cache_path,
    })
}

pub fn code_index_cache_path(
    root: &Path,
    path: &str,
    content_hash: &str,
    indexer_version: &str,
    ontology_version: &str,
    language_pack: Option<&str>,
) -> PathBuf {
    let key = format!(
        "{}:{}:{}:{}:{}",
        path,
        content_hash,
        indexer_version,
        ontology_version,
        language_pack.unwrap_or("default")
    );
    root.join(".specgraph")
        .join("index")
        .join("code")
        .join(format!("{}.json", stable_fragment(&key)))
}

pub fn content_hash(source: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(source.as_bytes());
    format!("sha256:{:x}", hasher.finalize())
}

pub fn framework_for_source(path: &str, language: &str, source: &str) -> Option<&'static str> {
    match language {
        "javascript" | "typescript" => {
            if source.contains("express()")
                || source.contains("require('express')")
                || source.contains("require(\"express\")")
                || source.contains(" from 'express'")
                || source.contains(" from \"express\"")
            {
                Some("express")
            } else if path.contains("/pages/") || path.contains("/app/") {
                Some("nextjs")
            } else {
                None
            }
        }
        "rust" => {
            if source.contains("axum::") || source.contains("Router::new()") {
                Some("axum")
            } else if source.contains("actix_web::") {
                Some("actix-web")
            } else {
                None
            }
        }
        "python" => {
            if source.contains("FastAPI(") {
                Some("fastapi")
            } else if source.contains("Flask(") {
                Some("flask")
            } else {
                None
            }
        }
        _ => None,
    }
}

pub fn language_for_path(path: &str) -> Option<&'static str> {
    let extension = Path::new(path)
        .extension()
        .and_then(|value| value.to_str())?
        .to_ascii_lowercase();
    match extension.as_str() {
        "ts" | "tsx" => Some("typescript"),
        "js" | "jsx" | "mjs" | "cjs" => Some("javascript"),
        "rs" => Some("rust"),
        "py" => Some("python"),
        "go" => Some("go"),
        "java" => Some("java"),
        "kt" | "kts" => Some("kotlin"),
        "swift" => Some("swift"),
        _ => None,
    }
}

pub fn observations_to_delta(observations: &[CodeIndexObservation]) -> GraphDelta {
    let mut create_nodes = Vec::new();
    let mut create_edges = Vec::new();
    let mut seen_nodes = BTreeSet::new();
    let mut seen_edges = BTreeSet::new();

    for observation in observations {
        let file_id = code_file_node_id(&observation.file);
        if seen_nodes.insert(file_id.clone()) {
            let mut attributes = observed_attributes(BTreeMap::from([
                ("path".to_string(), json!(observation.file)),
                ("language".to_string(), json!(observation.language)),
                ("framework".to_string(), json!(observation.framework)),
                ("generated".to_string(), json!(observation.generated)),
                (
                    "indexerLanguage".to_string(),
                    json!(observation.provenance.language_id.clone()),
                ),
                (
                    "indexerVersion".to_string(),
                    json!(observation.provenance.indexer_version.clone()),
                ),
                (
                    "contentHash".to_string(),
                    json!(observation.provenance.content_hash.clone()),
                ),
                (
                    "indexerDeterministic".to_string(),
                    json!(observation.provenance.deterministic),
                ),
                ("symbolCount".to_string(), json!(observation.symbols.len())),
                ("importCount".to_string(), json!(observation.imports.len())),
                ("routeCount".to_string(), json!(observation.routes.len())),
                (
                    "configAccessCount".to_string(),
                    json!(observation.config_accesses.len()),
                ),
            ]));
            attributes.insert("sourceFile".to_string(), json!(observation.file));
            create_nodes.push(Node {
                id: file_id.clone(),
                stable_key: format!("code-file:{}", observation.file),
                node_type: "CodeFile".to_string(),
                attributes,
            });
        }

        for symbol in &observation.symbols {
            let symbol_id = code_symbol_node_id(&observation.file, &symbol.kind, &symbol.name);
            if seen_nodes.insert(symbol_id.clone()) {
                let mut attributes = observed_attributes(BTreeMap::from([
                    ("file".to_string(), json!(observation.file)),
                    ("language".to_string(), json!(observation.language)),
                    ("framework".to_string(), json!(observation.framework)),
                    ("name".to_string(), json!(symbol.name)),
                    ("kind".to_string(), json!(symbol.kind)),
                    ("visibility".to_string(), json!(symbol.visibility)),
                    ("line".to_string(), json!(symbol.line)),
                ]));
                insert_location(&mut attributes, &symbol.location);
                create_nodes.push(Node {
                    id: symbol_id.clone(),
                    stable_key: format!(
                        "code-symbol:{}/{}/{}",
                        observation.file, symbol.kind, symbol.name
                    ),
                    node_type: "CodeSymbol".to_string(),
                    attributes,
                });
            }
            push_edge(
                &mut create_edges,
                &mut seen_edges,
                observed_edge(&file_id, "DEFINES_SYMBOL", &symbol_id),
            );
        }

        for import in &observation.imports {
            let import_id = code_import_node_id(&observation.file, &import.imported);
            if seen_nodes.insert(import_id.clone()) {
                let mut attributes = observed_attributes(BTreeMap::from([
                    ("file".to_string(), json!(observation.file)),
                    ("imported".to_string(), json!(import.imported)),
                    ("specifier".to_string(), json!(import.specifier)),
                    ("language".to_string(), json!(observation.language)),
                    ("framework".to_string(), json!(observation.framework)),
                ]));
                insert_location(&mut attributes, &import.location);
                create_nodes.push(Node {
                    id: import_id.clone(),
                    stable_key: format!("code-import:{}->{}", observation.file, import.imported),
                    node_type: "CodeImport".to_string(),
                    attributes,
                });
            }
            push_edge(
                &mut create_edges,
                &mut seen_edges,
                observed_edge(&file_id, "HAS_IMPORT", &import_id),
            );
            push_edge(
                &mut create_edges,
                &mut seen_edges,
                observed_edge(
                    &file_id,
                    "IMPORTS_FILE",
                    &code_file_node_id(&import.imported),
                ),
            );
        }

        for route in &observation.routes {
            let route_id = code_route_node_id(&route.method, &route.path);
            if seen_nodes.insert(route_id.clone()) {
                let mut attributes = observed_attributes(BTreeMap::from([
                    ("file".to_string(), json!(observation.file)),
                    (
                        "method".to_string(),
                        json!(route.method.to_ascii_uppercase()),
                    ),
                    ("path".to_string(), json!(route.path)),
                    ("handlerSymbol".to_string(), json!(route.handler_symbol)),
                    ("language".to_string(), json!(observation.language)),
                    ("framework".to_string(), json!(route.framework)),
                ]));
                insert_location(&mut attributes, &route.location);
                create_nodes.push(Node {
                    id: route_id.clone(),
                    stable_key: format!(
                        "code-route:{}-{}",
                        route.method.to_ascii_uppercase(),
                        route.path
                    ),
                    node_type: "CodeRoute".to_string(),
                    attributes,
                });
            }
            push_edge(
                &mut create_edges,
                &mut seen_edges,
                observed_edge(&file_id, "DECLARES_ROUTE", &route_id),
            );
            if let Some(handler) = &route.handler_symbol {
                push_edge(
                    &mut create_edges,
                    &mut seen_edges,
                    observed_edge(
                        &route_id,
                        "HANDLED_BY_SYMBOL",
                        &code_symbol_node_id(&observation.file, "function", handler),
                    ),
                );
            }
        }
        for config in &observation.config_accesses {
            let usage_id = config_usage_node_id(&observation.file, &config.name);
            if seen_nodes.insert(usage_id.clone()) {
                let mut attributes = observed_attributes(BTreeMap::from([
                    ("file".to_string(), json!(observation.file)),
                    ("name".to_string(), json!(config.name)),
                    ("kind".to_string(), json!(config.kind)),
                    ("accessPattern".to_string(), json!(config.access_pattern)),
                    ("language".to_string(), json!(observation.language)),
                    ("framework".to_string(), json!(observation.framework)),
                ]));
                insert_location(&mut attributes, &config.location);
                create_nodes.push(Node {
                    id: usage_id.clone(),
                    stable_key: format!("config-usage:{}/{}", observation.file, config.name),
                    node_type: "ConfigUsage".to_string(),
                    attributes,
                });
            }
            push_edge(
                &mut create_edges,
                &mut seen_edges,
                observed_edge(&file_id, "FILE_READS_CONFIG", &usage_id),
            );
            if config.kind == "secret" {
                push_edge(
                    &mut create_edges,
                    &mut seen_edges,
                    observed_edge(&file_id, "FILE_READS_SECRET", &usage_id),
                );
            }
        }
    }

    GraphDelta {
        create_nodes,
        create_edges,
        ..GraphDelta::default()
    }
}

fn extract_symbols(path: &str, language: &str, source: &str) -> Vec<CodeSymbolObservation> {
    if language == "rust" {
        return extract_rust_symbols(path, source);
    }
    let mut symbols = Vec::new();
    let mut seen = BTreeSet::new();

    for (index, line) in source.lines().enumerate() {
        let line_number = (index + 1) as u32;
        for (kind, name) in symbols_from_line(language, line) {
            let key = (kind.clone(), name.clone());
            if seen.insert(key) {
                symbols.push(CodeSymbolObservation {
                    name,
                    kind,
                    visibility: symbol_visibility(language, line),
                    line: Some(line_number),
                    location: Some(SourceLocation {
                        file: path.to_string(),
                        start_line: Some(line_number),
                        end_line: Some(line_number),
                        start_column: None,
                        end_column: None,
                    }),
                });
            }
        }
    }

    symbols
}

fn extract_rust_symbols(path: &str, source: &str) -> Vec<CodeSymbolObservation> {
    let mut symbols = Vec::new();
    let mut seen = BTreeSet::new();
    let mut brace_depth = 0usize;
    let mut impl_stack: Vec<(String, usize)> = Vec::new();

    for (index, raw_line) in source.lines().enumerate() {
        let line_number = (index + 1) as u32;
        let line = strip_line_comment(raw_line, "//");
        while impl_stack
            .last()
            .is_some_and(|(_, depth)| brace_depth < *depth)
        {
            impl_stack.pop();
        }
        let current_impl = impl_stack.last().map(|(name, _)| name.clone());
        for (mut kind, mut name) in rust_symbols_from_line(line) {
            if kind == "function" {
                if let Some(parent) = &current_impl {
                    kind = "method".to_string();
                    name = format!("{parent}::{name}");
                }
            }
            let key = (kind.clone(), name.clone());
            if seen.insert(key) {
                symbols.push(CodeSymbolObservation {
                    name,
                    kind,
                    visibility: symbol_visibility("rust", line),
                    line: Some(line_number),
                    location: Some(SourceLocation {
                        file: path.to_string(),
                        start_line: Some(line_number),
                        end_line: Some(line_number),
                        start_column: None,
                        end_column: None,
                    }),
                });
            }
        }
        if let Some(impl_name) = rust_impl_target(line) {
            let impl_body_depth = brace_depth + line.matches('{').count();
            if impl_body_depth > brace_depth {
                impl_stack.push((impl_name, impl_body_depth));
            }
        }
        brace_depth = brace_depth.saturating_add(line.matches('{').count());
        brace_depth = brace_depth.saturating_sub(line.matches('}').count());
    }

    symbols
}

fn symbol_visibility(language: &str, line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    match language {
        "rust" => Some(
            if trimmed.starts_with("pub") {
                "public"
            } else {
                "private"
            }
            .to_string(),
        ),
        "typescript" | "javascript" => Some(
            if trimmed.starts_with("export ")
                || trimmed.starts_with("export\t")
                || trimmed.starts_with("export default")
            {
                "public"
            } else {
                "private"
            }
            .to_string(),
        ),
        "python" => Some(
            if trimmed
                .strip_prefix("def ")
                .or_else(|| trimmed.strip_prefix("async def "))
                .or_else(|| trimmed.strip_prefix("class "))
                .and_then(clean_identifier)
                .is_some_and(|name| name.starts_with('_'))
            {
                "private"
            } else {
                "public"
            }
            .to_string(),
        ),
        _ => None,
    }
}

fn extract_imports(path: &str, language: &str, source: &str) -> Vec<CodeImportObservation> {
    let mut imports = Vec::new();
    let mut seen = BTreeSet::new();
    for (index, line) in source.lines().enumerate() {
        let line_number = (index + 1) as u32;
        for (specifier, imported) in imports_from_line(path, language, line) {
            if seen.insert(imported.clone()) {
                imports.push(CodeImportObservation {
                    imported,
                    specifier: Some(specifier),
                    location: Some(SourceLocation {
                        file: path.to_string(),
                        start_line: Some(line_number),
                        end_line: Some(line_number),
                        start_column: None,
                        end_column: None,
                    }),
                });
            }
        }
    }
    imports
}

fn extract_routes(
    path: &str,
    language: &str,
    framework: Option<&str>,
    source: &str,
) -> Vec<CodeRouteObservation> {
    let mut routes = Vec::new();
    let mut seen = BTreeSet::new();
    for (index, line) in source.lines().enumerate() {
        let line_number = (index + 1) as u32;
        for (method, route_path, handler) in routes_from_line(language, framework, line) {
            let key = (method.clone(), route_path.clone());
            if seen.insert(key) {
                routes.push(CodeRouteObservation {
                    method,
                    path: route_path,
                    handler_symbol: handler,
                    framework: framework.map(str::to_string),
                    location: Some(SourceLocation {
                        file: path.to_string(),
                        start_line: Some(line_number),
                        end_line: Some(line_number),
                        start_column: None,
                        end_column: None,
                    }),
                });
            }
        }
    }
    routes
}

fn extract_config_accesses(
    path: &str,
    language: &str,
    source: &str,
) -> Vec<ConfigAccessObservation> {
    let mut accesses = Vec::new();
    let mut seen = BTreeSet::new();
    for (index, line) in source.lines().enumerate() {
        let line_number = (index + 1) as u32;
        for (name, pattern) in config_accesses_from_line(language, line) {
            if seen.insert((name.clone(), pattern.clone())) {
                accesses.push(ConfigAccessObservation {
                    kind: if looks_like_secret_name(&name) {
                        "secret".to_string()
                    } else {
                        "config".to_string()
                    },
                    name,
                    access_pattern: pattern,
                    location: Some(SourceLocation {
                        file: path.to_string(),
                        start_line: Some(line_number),
                        end_line: Some(line_number),
                        start_column: None,
                        end_column: None,
                    }),
                });
            }
        }
    }
    accesses
}

pub fn validate_code_index_observations(observations: &[CodeIndexObservation]) -> Vec<Finding> {
    let mut findings = Vec::new();
    for observation in observations {
        for symbol in &observation.symbols {
            if symbol.location.is_none() {
                findings.push(finding(
                    "code_indexer.symbol_location_required",
                    format!(
                        "Symbol `{}` in `{}` must include a source location.",
                        symbol.name, observation.file
                    ),
                ));
            }
        }
        for route in &observation.routes {
            if route.location.is_none() {
                findings.push(finding(
                    "code_indexer.route_location_required",
                    format!(
                        "Route `{}` `{}` in `{}` must include a source location.",
                        route.method, route.path, observation.file
                    ),
                ));
            }
        }
        for import in &observation.imports {
            if import.location.is_none() {
                findings.push(finding(
                    "code_indexer.import_location_required",
                    format!(
                        "Import `{}` in `{}` must include a source location.",
                        import.imported, observation.file
                    ),
                ));
            }
        }
        for config in &observation.config_accesses {
            if config.location.is_none() {
                findings.push(finding(
                    "code_indexer.config_location_required",
                    format!(
                        "Config access `{}` in `{}` must include a source location.",
                        config.name, observation.file
                    ),
                ));
            }
        }
    }
    findings
}

fn config_accesses_from_line(language: &str, line: &str) -> Vec<(String, String)> {
    match language {
        "javascript" | "typescript" => javascript_config_accesses_from_line(line),
        "rust" => rust_config_accesses_from_line(line),
        "python" => python_config_accesses_from_line(line),
        _ => Vec::new(),
    }
}

fn javascript_config_accesses_from_line(line: &str) -> Vec<(String, String)> {
    let line = strip_line_comment(line, "//");
    let mut out = Vec::new();
    for marker in ["process.env.", "import.meta.env."] {
        let mut rest = line;
        while let Some((_, after)) = rest.split_once(marker) {
            if let Some(name) = clean_identifier(after) {
                out.push((name, marker.trim_end_matches('.').to_string()));
            }
            rest = after;
        }
    }
    if line.contains("readFileSync(") {
        if let Some(path) = quoted_value(line).and_then(config_file_access_name) {
            out.push((path, "fs.readFileSync".to_string()));
        }
    }
    out
}

fn rust_config_accesses_from_line(line: &str) -> Vec<(String, String)> {
    let line = strip_line_comment(line, "//");
    let mut out = ["std::env::var(", "env::var("]
        .iter()
        .filter_map(|marker| {
            line.split_once(marker)
                .and_then(|(_, rest)| quoted_value(rest))
                .map(|name| (name, marker.trim_end_matches('(').to_string()))
        })
        .collect::<Vec<_>>();
    if line.contains("read_to_string(") {
        if let Some(path) = quoted_value(line).and_then(config_file_access_name) {
            out.push((path, "fs::read_to_string".to_string()));
        }
    }
    out
}

fn python_config_accesses_from_line(line: &str) -> Vec<(String, String)> {
    let line = strip_line_comment(line, "#");
    let mut out = Vec::new();
    for marker in ["os.environ[", "os.getenv("] {
        if let Some((_, rest)) = line.split_once(marker) {
            if let Some(name) = quoted_value(rest) {
                out.push((name, marker.trim_end_matches(['[', '(']).to_string()));
            }
        }
    }
    if line.contains("open(") {
        if let Some(path) = quoted_value(line).and_then(config_file_access_name) {
            out.push((path, "open".to_string()));
        }
    }
    out
}

fn config_file_access_name(path: String) -> Option<String> {
    let lower = path.to_ascii_lowercase();
    if lower.contains("config") || lower.contains(".env") {
        Some(path)
    } else {
        None
    }
}

fn looks_like_secret_name(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    ["SECRET", "TOKEN", "PASSWORD", "PRIVATE_KEY", "API_KEY"]
        .iter()
        .any(|marker| upper.contains(marker))
}

fn imports_from_line(path: &str, language: &str, line: &str) -> Vec<(String, String)> {
    match language {
        "javascript" | "typescript" => javascript_imports_from_line(path, line),
        "rust" => rust_imports_from_line(line),
        "python" => python_imports_from_line(line),
        _ => Vec::new(),
    }
}

fn javascript_imports_from_line(path: &str, line: &str) -> Vec<(String, String)> {
    let line = strip_line_comment(line, "//").trim();
    let specifier = if let Some((_, rest)) = line.split_once(" from ") {
        quoted_value(rest)
    } else if let Some((_, rest)) = line.split_once("require(") {
        quoted_value(rest)
    } else {
        None
    };
    specifier
        .map(|specifier| {
            let imported = resolve_javascript_import(path, &specifier);
            vec![(specifier, imported)]
        })
        .unwrap_or_default()
}

fn rust_imports_from_line(line: &str) -> Vec<(String, String)> {
    let line = strip_line_comment(line, "//").trim();
    if let Some(rest) = line.strip_prefix("use ") {
        let specifier = rest.trim_end_matches(';').trim().to_string();
        vec![(specifier.clone(), specifier.replace("::", "/"))]
    } else if let Some(rest) = line.strip_prefix("mod ") {
        let specifier = rest.trim_end_matches(';').trim().to_string();
        vec![(specifier.clone(), format!("{specifier}.rs"))]
    } else {
        Vec::new()
    }
}

fn python_imports_from_line(line: &str) -> Vec<(String, String)> {
    let line = strip_line_comment(line, "#").trim();
    if let Some(rest) = line.strip_prefix("from ") {
        let module = rest.split_whitespace().next().unwrap_or_default();
        vec![(module.to_string(), module.replace('.', "/"))]
    } else if let Some(rest) = line.strip_prefix("import ") {
        let module = rest.split(',').next().unwrap_or_default().trim();
        vec![(module.to_string(), module.replace('.', "/"))]
    } else {
        Vec::new()
    }
}

fn routes_from_line(
    language: &str,
    framework: Option<&str>,
    line: &str,
) -> Vec<(String, String, Option<String>)> {
    match (language, framework) {
        ("javascript" | "typescript", Some("express")) => express_routes_from_line(line),
        ("python", Some("fastapi") | Some("flask")) => python_routes_from_line(line),
        ("rust", Some("axum")) => axum_routes_from_line(line),
        _ => Vec::new(),
    }
}

fn express_routes_from_line(line: &str) -> Vec<(String, String, Option<String>)> {
    let trimmed = strip_line_comment(line, "//").trim();
    for method in ["get", "post", "put", "patch", "delete"] {
        for prefix in [format!("app.{method}("), format!("router.{method}(")] {
            if let Some(rest) = trimmed.split_once(&prefix).map(|(_, rest)| rest) {
                if let Some(path) = quoted_value(rest) {
                    let handler = rest
                        .split(',')
                        .nth(1)
                        .and_then(|value| clean_identifier(value.trim()));
                    return vec![(method.to_ascii_uppercase(), path, handler)];
                }
            }
        }
    }
    Vec::new()
}

fn python_routes_from_line(line: &str) -> Vec<(String, String, Option<String>)> {
    let trimmed = strip_line_comment(line, "#").trim();
    for method in ["get", "post", "put", "patch", "delete"] {
        for prefix in [format!("@app.{method}("), format!("@router.{method}(")] {
            if let Some(rest) = trimmed.split_once(&prefix).map(|(_, rest)| rest) {
                if let Some(path) = quoted_value(rest) {
                    return vec![(method.to_ascii_uppercase(), path, None)];
                }
            }
        }
    }
    Vec::new()
}

fn axum_routes_from_line(line: &str) -> Vec<(String, String, Option<String>)> {
    let trimmed = strip_line_comment(line, "//").trim();
    let Some(rest) = trimmed.split_once(".route(").map(|(_, rest)| rest) else {
        return Vec::new();
    };
    let Some(path) = quoted_value(rest) else {
        return Vec::new();
    };
    let method = ["get", "post", "put", "patch", "delete"]
        .iter()
        .find(|method| rest.contains(&format!("{method}(")))
        .map(|method| method.to_ascii_uppercase())
        .unwrap_or_else(|| "GET".to_string());
    let handler = rest
        .split_once(&format!("{}(", method.to_ascii_lowercase()))
        .and_then(|(_, value)| clean_identifier(value));
    vec![(method, path, handler)]
}

fn is_generated_source(path: &str, source: &str) -> bool {
    path.contains("/generated/")
        || path.ends_with(".generated.ts")
        || path.ends_with(".generated.js")
        || source.lines().take(5).any(|line| {
            let lower = line.to_ascii_lowercase();
            lower.contains("@generated") || lower.contains("do not edit")
        })
}

fn resolve_javascript_import(path: &str, specifier: &str) -> String {
    if !specifier.starts_with('.') {
        return specifier.to_string();
    }
    let base = Path::new(path).parent().unwrap_or_else(|| Path::new(""));
    let joined = base.join(specifier);
    joined
        .to_string_lossy()
        .trim_start_matches("./")
        .to_string()
}

fn quoted_value(value: &str) -> Option<String> {
    let quote = value.chars().find(|ch| *ch == '\'' || *ch == '"')?;
    let after = value.split_once(quote)?.1;
    let (quoted, _) = after.split_once(quote)?;
    Some(quoted.to_string())
}

fn symbols_from_line(language: &str, line: &str) -> Vec<(String, String)> {
    match language {
        "rust" => rust_symbols_from_line(line),
        "typescript" | "javascript" => javascript_symbols_from_line(line),
        "python" => python_symbols_from_line(line),
        "go" => go_symbols_from_line(line),
        "java" | "kotlin" | "swift" => c_family_symbols_from_line(line),
        _ => Vec::new(),
    }
}

fn rust_symbols_from_line(line: &str) -> Vec<(String, String)> {
    let mut symbols = Vec::new();
    let line = strip_line_comment(line, "//");
    let normalized = normalize_leading_keywords(
        line.trim(),
        &[
            "pub(crate)",
            "pub(super)",
            "pub(self)",
            "pub",
            "async",
            "const",
            "unsafe",
            "extern",
        ],
    );
    for (keyword, kind) in [
        ("fn", "function"),
        ("struct", "struct"),
        ("enum", "enum"),
        ("trait", "trait"),
        ("mod", "module"),
        ("type", "type"),
    ] {
        if let Some(name) = identifier_after_keyword(normalized, keyword) {
            symbols.push((kind.to_string(), name));
        }
    }
    if let Some(name) = identifier_after_keyword(normalized, "impl") {
        symbols.push(("impl".to_string(), name));
    }
    symbols
}

fn rust_impl_target(line: &str) -> Option<String> {
    let line = strip_line_comment(line, "//");
    let normalized = normalize_leading_keywords(line.trim(), &["unsafe", "default"]);
    let mut rest = strip_leading_keyword(normalized, "impl")?.trim_start();
    if rest.starts_with('<') {
        rest = rest.split_once('>')?.1.trim_start();
    }
    if let Some((_, target)) = rest.split_once(" for ") {
        clean_identifier(target)
    } else {
        clean_identifier(rest)
    }
}

fn javascript_symbols_from_line(line: &str) -> Vec<(String, String)> {
    let mut symbols = Vec::new();
    let line = strip_line_comment(line, "//");
    let normalized = normalize_leading_keywords(
        line.trim(),
        &[
            "export",
            "default",
            "declare",
            "abstract",
            "async",
            "public",
            "private",
            "protected",
            "static",
            "readonly",
        ],
    );

    for (keyword, kind) in [
        ("function", "function"),
        ("class", "class"),
        ("interface", "interface"),
        ("type", "type"),
        ("enum", "enum"),
    ] {
        if let Some(name) = identifier_after_keyword(normalized, keyword) {
            symbols.push((kind.to_string(), name));
        }
    }

    for keyword in ["const", "let", "var"] {
        if let Some(name) = identifier_after_keyword(normalized, keyword) {
            let kind = if normalized.contains("=>") || normalized.contains("function") {
                "function"
            } else {
                "variable"
            };
            symbols.push((kind.to_string(), name));
        }
    }

    symbols
}

fn python_symbols_from_line(line: &str) -> Vec<(String, String)> {
    let mut symbols = Vec::new();
    let line = strip_line_comment(line, "#");
    let normalized = normalize_leading_keywords(line.trim(), &["async"]);
    if let Some(name) = identifier_after_keyword(normalized, "def") {
        symbols.push(("function".to_string(), name));
    }
    if let Some(name) = identifier_after_keyword(normalized, "class") {
        symbols.push(("class".to_string(), name));
    }
    symbols
}

fn go_symbols_from_line(line: &str) -> Vec<(String, String)> {
    let mut symbols = Vec::new();
    let line = strip_line_comment(line, "//");
    let normalized = line.trim();
    if let Some(name) = identifier_after_keyword(normalized, "func") {
        symbols.push(("function".to_string(), name));
    }
    if normalized.starts_with("type ") && normalized.contains(" struct") {
        if let Some(name) = identifier_after_keyword(normalized, "type") {
            symbols.push(("struct".to_string(), name));
        }
    }
    if normalized.starts_with("type ") && normalized.contains(" interface") {
        if let Some(name) = identifier_after_keyword(normalized, "type") {
            symbols.push(("interface".to_string(), name));
        }
    }
    symbols
}

fn c_family_symbols_from_line(line: &str) -> Vec<(String, String)> {
    let mut symbols = Vec::new();
    let line = strip_line_comment(line, "//");
    let normalized = normalize_leading_keywords(
        line.trim(),
        &[
            "public",
            "private",
            "protected",
            "static",
            "final",
            "abstract",
            "open",
            "export",
            "internal",
        ],
    );
    for (keyword, kind) in [
        ("class", "class"),
        ("interface", "interface"),
        ("enum", "enum"),
        ("struct", "struct"),
        ("func", "function"),
        ("fun", "function"),
    ] {
        if let Some(name) = identifier_after_keyword(normalized, keyword) {
            symbols.push((kind.to_string(), name));
        }
    }
    symbols
}

fn strip_line_comment<'a>(line: &'a str, marker: &str) -> &'a str {
    line.split_once(marker)
        .map(|(before, _)| before)
        .unwrap_or(line)
}

fn normalize_leading_keywords<'a>(mut value: &'a str, keywords: &[&str]) -> &'a str {
    loop {
        let mut changed = false;
        for keyword in keywords {
            if let Some(rest) = strip_leading_keyword(value, keyword) {
                value = rest.trim_start();
                changed = true;
                break;
            }
        }
        if !changed {
            return value;
        }
    }
}

fn strip_leading_keyword<'a>(value: &'a str, keyword: &str) -> Option<&'a str> {
    let rest = value.strip_prefix(keyword)?;
    if rest
        .chars()
        .next()
        .is_none_or(|ch| ch.is_whitespace() || ch == '<')
    {
        Some(rest)
    } else {
        None
    }
}

fn identifier_after_keyword(value: &str, keyword: &str) -> Option<String> {
    let rest = strip_leading_keyword(value, keyword)?.trim_start();
    let rest = if keyword == "impl" && rest.starts_with('<') {
        rest.split_once('>')?.1.trim_start()
    } else {
        rest
    };
    let rest = rest.strip_prefix("r#").unwrap_or(rest);
    clean_identifier(rest)
}

fn clean_identifier(value: &str) -> Option<String> {
    let identifier = value
        .trim_start_matches(['*', '&'])
        .chars()
        .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_' || *ch == '$')
        .collect::<String>();
    if identifier.is_empty() || identifier.chars().next()?.is_ascii_digit() {
        None
    } else {
        Some(identifier)
    }
}

fn config_usage_node_id(file: &str, name: &str) -> String {
    format!(
        "node_config_usage_{}",
        stable_fragment(&format!("{file}/{name}"))
    )
}

fn observed_attributes(
    mut attributes: BTreeMap<String, serde_json::Value>,
) -> BTreeMap<String, serde_json::Value> {
    attributes.insert("trustState".to_string(), json!(TRUST_STATE_OBSERVED));
    attributes.insert("sourceTrust".to_string(), json!(SOURCE_TRUST_OBSERVATION));
    attributes.insert("observedBy".to_string(), json!(CODE_INDEXER_ADAPTER_ID));
    attributes
}

fn insert_location(
    attributes: &mut BTreeMap<String, serde_json::Value>,
    location: &Option<SourceLocation>,
) {
    if let Some(location) = location {
        attributes.insert("sourceFile".to_string(), json!(location.file));
        attributes.insert("startLine".to_string(), json!(location.start_line));
        attributes.insert("endLine".to_string(), json!(location.end_line));
        attributes.insert("startColumn".to_string(), json!(location.start_column));
        attributes.insert("endColumn".to_string(), json!(location.end_column));
    }
}

fn observed_edge(from: &str, edge_type: &str, to: &str) -> Edge {
    Edge {
        id: edge_id(from, edge_type, to),
        stable_key: format!("edge:{from}:{edge_type}:{to}"),
        edge_type: edge_type.to_string(),
        from: from.to_string(),
        to: to.to_string(),
        attributes: observed_attributes(BTreeMap::new()),
    }
}

fn push_edge(edges: &mut Vec<Edge>, seen: &mut BTreeSet<String>, edge: Edge) {
    if seen.insert(edge.id.clone()) {
        edges.push(edge);
    }
}

fn edge_id(from: &str, edge_type: &str, to: &str) -> String {
    format!(
        "edge_{}",
        stable_fragment(&format!("{from}:{edge_type}:{to}"))
    )
}

fn finding(code: &str, message: String) -> Finding {
    Finding::new(code, FindingSeverity::Error, message)
        .with_validator(VALIDATOR_ADAPTER_TRUST, CORE_VALIDATOR_VERSION)
}

fn stable_fragment(value: &str) -> String {
    let mut out = String::new();
    let mut last_was_separator = false;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_was_separator = false;
        } else if !last_was_separator {
            out.push('_');
            last_was_separator = true;
        }
    }
    out.trim_matches('_').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexes_typescript_symbols() {
        let observation = index_source_file(
            "src/user.ts",
            r#"
import { repo } from "./repo";
export interface UserRepository {}
export class UserService {}
export const resetPassword = async () => {};
function helper() {}
"#,
        );

        assert_eq!(observation.language, "typescript");
        assert_eq!(observation.imports.len(), 1);
        assert!(observation
            .symbols
            .iter()
            .any(|symbol| symbol.kind == "interface" && symbol.name == "UserRepository"));
        assert!(observation
            .symbols
            .iter()
            .any(|symbol| symbol.kind == "class" && symbol.name == "UserService"));
        assert!(observation
            .symbols
            .iter()
            .any(|symbol| symbol.kind == "function" && symbol.name == "resetPassword"));
        assert!(observation
            .symbols
            .iter()
            .any(|symbol| symbol.kind == "function" && symbol.name == "helper"));
    }

    #[test]
    fn indexes_rust_symbols_and_delta_nodes() {
        let observation = index_source_file(
            "crates/demo/src/lib.rs",
            r#"
pub struct Store {}
pub(crate) struct PrivateStore {}
pub enum Event {}
pub trait Indexer {}
pub fn replay() {}
impl Store {}
"#,
        );
        let delta = observations_to_delta(&[observation.clone()]);

        assert_eq!(observation.language, "rust");
        assert_eq!(observation.symbols.len(), 6);
        assert_eq!(observation.provenance.language_id, "rust");
        assert_eq!(observation.provenance.indexer_version, "semantic-rust/v1");
        assert!(delta
            .create_nodes
            .iter()
            .any(|node| node.node_type == "CodeFile"));
        assert!(sg_adapter_api::validate_adapter_delta(
            &sg_adapter_api::AdapterDescriptor::lightweight_code_indexer(),
            &delta
        )
        .is_empty());
        assert_eq!(
            delta
                .create_nodes
                .iter()
                .filter(|node| node.node_type == "CodeSymbol")
                .count(),
            6
        );
    }

    #[test]
    fn semantic_rust_indexer_extracts_methods_visibility_and_provenance() {
        let indexer = RustSemanticIndexer;
        assert_eq!(indexer.language_id(), "rust");
        assert!(SemanticCodeIndexer::supported_file_extensions(&indexer).contains(&"rs"));
        let observation = indexer.index_file_semantic(
            "crates/demo/src/lib.rs",
            r#"
pub struct Store {}
impl Store {
    pub fn new() -> Self { Store {} }
    fn helper(&self) {}
}
"#,
        );

        assert!(observation.symbols.iter().any(|symbol| {
            symbol.kind == "struct"
                && symbol.name == "Store"
                && symbol.visibility.as_deref() == Some("public")
        }));
        assert!(observation.symbols.iter().any(|symbol| {
            symbol.kind == "method"
                && symbol.name == "Store::new"
                && symbol.visibility.as_deref() == Some("public")
        }));
        assert!(observation.symbols.iter().any(|symbol| {
            symbol.kind == "method"
                && symbol.name == "Store::helper"
                && symbol.visibility.as_deref() == Some("private")
        }));
        assert_eq!(
            observation.provenance.contract_schema_version,
            CODE_INDEXER_CONTRACT_SCHEMA_VERSION
        );
        assert!(observation.provenance.deterministic);
    }

    #[test]
    fn semantic_typescript_and_python_indexers_extract_exports_routes_and_imports() {
        let ts = TypeScriptSemanticIndexer.index_file_semantic(
            "apps/api/src/routes.ts",
            r#"
import express from "express";
export class UserController {}
const app = express();
function resetPassword(req, res) {}
app.post("/password-reset", resetPassword);
"#,
        );
        assert_eq!(ts.language, "typescript");
        assert!(ts.symbols.iter().any(|symbol| {
            symbol.name == "UserController" && symbol.visibility.as_deref() == Some("public")
        }));
        assert!(ts.routes.iter().any(|route| {
            route.method == "POST"
                && route.path == "/password-reset"
                && route.handler_symbol.as_deref() == Some("resetPassword")
        }));

        let py = PythonSemanticIndexer.index_file_semantic(
            "app/main.py",
            r#"
from fastapi import FastAPI
app = FastAPI()
@app.get("/users")
def list_users():
    pass
"#,
        );
        assert_eq!(py.language, "python");
        assert!(py
            .imports
            .iter()
            .any(|import| import.specifier.as_deref() == Some("fastapi")));
        assert!(py
            .routes
            .iter()
            .any(|route| route.method == "GET" && route.path == "/users"));
    }

    #[test]
    fn code_index_cache_reuses_unchanged_semantic_output() {
        let tmp = tempfile::tempdir().unwrap();
        let source = "pub struct Store {}\n";
        let first = index_source_file_with_cache(
            tmp.path(),
            "crates/demo/src/lib.rs",
            source,
            "ontology/v1",
            Some("rust-pack/v1"),
        )
        .unwrap();
        assert!(!first.cache_hit);
        assert!(first.cache_path.exists());
        let first_delta = observations_to_delta(std::slice::from_ref(&first.observation));

        let second = index_source_file_with_cache(
            tmp.path(),
            "crates/demo/src/lib.rs",
            source,
            "ontology/v1",
            Some("rust-pack/v1"),
        )
        .unwrap();
        assert!(second.cache_hit);
        assert_eq!(first.observation, second.observation);
        let second_delta = observations_to_delta(std::slice::from_ref(&second.observation));
        assert_eq!(first_delta, second_delta);
    }

    #[test]
    fn framework_indexer_extracts_express_routes_with_trust_and_locations() {
        let indexer = FrameworkAwareCodeIndexer;
        let observations = indexer.index_file(
            "src/routes/password-reset.js",
            r#"
const express = require("express");
const router = express.Router();
function resetPassword(req, res) {}
router.post("/password-reset", resetPassword);
"#,
        );
        let observation = &observations[0];
        assert_eq!(observation.framework.as_deref(), Some("express"));
        assert!(observation.routes.iter().any(|route| {
            route.method == "POST"
                && route.path == "/password-reset"
                && route
                    .location
                    .as_ref()
                    .and_then(|location| location.start_line)
                    == Some(5)
        }));

        let delta = observations_to_delta(&observations);
        let route = delta
            .create_nodes
            .iter()
            .find(|node| node.node_type == "CodeRoute")
            .expect("route node");
        assert_eq!(
            route
                .attributes
                .get("trustState")
                .and_then(|value| value.as_str()),
            Some(TRUST_STATE_OBSERVED)
        );
        assert!(delta
            .create_edges
            .iter()
            .any(|edge| edge.edge_type == "DECLARES_ROUTE"));
    }

    #[test]
    fn indexes_config_and_secret_accesses() {
        let observation = index_source_file(
            "src/config.ts",
            r#"
const databaseUrl = process.env.DATABASE_URL;
const apiToken = process.env.API_TOKEN;
const rawConfig = readFileSync("config/default.json", "utf8");
"#,
        );

        assert!(observation
            .config_accesses
            .iter()
            .any(|access| { access.name == "DATABASE_URL" && access.kind == "config" }));
        assert!(observation
            .config_accesses
            .iter()
            .any(|access| { access.name == "API_TOKEN" && access.kind == "secret" }));
        assert!(observation
            .config_accesses
            .iter()
            .any(|access| { access.name == "config/default.json" }));
        let delta = observations_to_delta(&[observation]);
        assert!(delta
            .create_nodes
            .iter()
            .any(|node| node.node_type == "ConfigUsage"));
        assert!(delta
            .create_edges
            .iter()
            .any(|edge| edge.edge_type == "FILE_READS_SECRET"));
    }

    #[test]
    fn observation_validation_requires_source_locations() {
        let findings = validate_code_index_observations(&[CodeIndexObservation {
            file: "src/app.js".to_string(),
            language: "javascript".to_string(),
            provenance: IndexerProvenance::default(),
            framework: Some("express".to_string()),
            generated: false,
            symbols: vec![CodeSymbolObservation {
                name: "handler".to_string(),
                kind: "function".to_string(),
                visibility: None,
                line: None,
                location: None,
            }],
            imports: Vec::new(),
            routes: Vec::new(),
            config_accesses: Vec::new(),
        }]);

        assert!(findings
            .iter()
            .any(|finding| finding.code == "code_indexer.symbol_location_required"));
    }
}
