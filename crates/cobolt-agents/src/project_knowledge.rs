// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

//! Project-local Knowledge Base retrieval backed by an embedded vector index.
//!
//! The store is a single key→record table (path → content + embedding), read by
//! full scan and scored by dot product — no SQL was ever involved beyond an
//! upsert, a delete and a `SELECT *`. It runs on `redb`, a pure-Rust embedded
//! store the workspace already uses, rather than on bundled SQLite, whose C
//! amalgamation made a C toolchain a prerequisite for building the IDE.

use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use redb::{Database, ReadableTable, TableDefinition};
use serde::{Deserialize, Serialize};

const VECTOR_DIMENSIONS: usize = 384;
const DATABASE_RELATIVE_PATH: &str = "data/project-knowledge.redb";
/// Path → [`DocumentRecord`]. One table, same shape as the old `project_documents`.
const DOCUMENTS: TableDefinition<&str, &[u8]> = TableDefinition::new("project_documents");

/// One indexed document. `dimensions` is kept (rather than inferred from
/// `embedding.len()`) so a record written by a build with a different vector
/// width is recognised as stale instead of silently mis-scored.
#[derive(Serialize, Deserialize)]
struct DocumentRecord {
    content: String,
    embedding: Vec<f32>,
    dimensions: u32,
    updated_unix: i64,
}
pub const KNOWLEDGE_BASE_ROOT: &str = "Knowledge Base";
const LEGACY_KNOWLEDGE_ROOTS: [&str; 2] = ["Documentation", "docs"];

#[derive(Debug, Clone, PartialEq)]
pub struct KnowledgeHit {
    pub path: String,
    pub score: f32,
    pub excerpt: String,
}

pub fn database_path(project_root: &Path) -> PathBuf {
    project_root.join(DATABASE_RELATIVE_PATH)
}

fn open(project_root: &Path) -> Result<Database, String> {
    let path = database_path(project_root);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    Database::create(path).map_err(|e| e.to_string())
}

fn fnv1a(text: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in text.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn vectorize(text: &str) -> Vec<f32> {
    let mut vector = vec![0.0_f32; VECTOR_DIMENSIONS];
    for token in text
        .split(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
        .filter(|token| token.len() > 1)
    {
        let token = token.to_lowercase();
        let hash = fnv1a(&token);
        let index = (hash as usize) % VECTOR_DIMENSIONS;
        let sign = if hash & (1_u64 << 63) == 0 { 1.0 } else { -1.0 };
        vector[index] += sign;
    }
    let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
    if norm > f32::EPSILON {
        for value in &mut vector {
            *value /= norm;
        }
    }
    vector
}

fn encode_record(record: &DocumentRecord) -> Result<Vec<u8>, String> {
    bincode::serialize(record).map_err(|e| e.to_string())
}

/// `None` for anything unreadable or written with a different vector width — the
/// caller skips it, so a stale or corrupt record degrades one document's
/// searchability instead of failing the whole query.
fn decode_record(bytes: &[u8]) -> Option<DocumentRecord> {
    let record: DocumentRecord = bincode::deserialize(bytes).ok()?;
    (record.dimensions as usize == VECTOR_DIMENSIONS
        && record.embedding.len() == VECTOR_DIMENSIONS)
        .then_some(record)
}

fn is_documentation_root(component: &str) -> bool {
    component.eq_ignore_ascii_case(KNOWLEDGE_BASE_ROOT)
        || LEGACY_KNOWLEDGE_ROOTS
            .iter()
            .any(|root| component.eq_ignore_ascii_case(root))
}

fn unique_legacy_destination(path: &Path) -> PathBuf {
    if !path.exists() {
        return path.to_path_buf();
    }
    let parent = path.parent().unwrap_or_else(|| Path::new(""));
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("document");
    let extension = path.extension().and_then(|value| value.to_str());
    for number in 1.. {
        let name = match extension {
            Some(extension) => format!("{stem}-legacy-{number}.{extension}"),
            None => format!("{stem}-legacy-{number}"),
        };
        let candidate = parent.join(name);
        if !candidate.exists() {
            return candidate;
        }
    }
    path.to_path_buf()
}

fn move_legacy_tree(source: &Path, destination: &Path) -> Result<usize, String> {
    if !source.exists() {
        return Ok(0);
    }
    std::fs::create_dir_all(destination).map_err(|error| error.to_string())?;
    let mut moved = 0;
    for entry in std::fs::read_dir(source).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        if source_path.is_dir() {
            moved += move_legacy_tree(&source_path, &destination_path)?;
        } else {
            let destination_path = if destination_path.exists() {
                let same =
                    std::fs::read(&source_path).ok() == std::fs::read(&destination_path).ok();
                if same {
                    std::fs::remove_file(&source_path).map_err(|error| error.to_string())?;
                    moved += 1;
                    continue;
                }
                unique_legacy_destination(&destination_path)
            } else {
                destination_path
            };
            if std::fs::rename(&source_path, &destination_path).is_err() {
                std::fs::copy(&source_path, &destination_path)
                    .map_err(|error| error.to_string())?;
                std::fs::remove_file(&source_path).map_err(|error| error.to_string())?;
            }
            moved += 1;
        }
    }
    let _ = std::fs::remove_dir(source);
    Ok(moved)
}

/// Create the canonical project Knowledge Base and migrate legacy
/// `Documentation/` and `docs/` content into it without overwriting conflicts.
pub fn ensure_knowledge_base(project_root: &Path) -> Result<usize, String> {
    let destination = project_root.join(KNOWLEDGE_BASE_ROOT);
    std::fs::create_dir_all(&destination).map_err(|error| error.to_string())?;
    let mut moved = 0;
    for legacy in LEGACY_KNOWLEDGE_ROOTS {
        moved += move_legacy_tree(&project_root.join(legacy), &destination)?;
    }
    Ok(moved)
}

/// Normalize a model-supplied Knowledge Base path and reject traversal or paths
/// outside the project's Knowledge Base. Legacy roots are redirected to the
/// canonical `Knowledge Base/` folder.
pub fn normalize_document_path(raw: &str) -> Result<PathBuf, String> {
    let trimmed = raw.trim().trim_start_matches(['/', '\\']);
    if trimmed.is_empty() {
        return Err("documentation path is empty".into());
    }
    let candidate = Path::new(trimmed);
    let mut clean = PathBuf::new();
    for component in candidate.components() {
        match component {
            Component::Normal(part) => clean.push(part),
            _ => {
                return Err(
                    "documentation path must not contain traversal or a drive prefix".into(),
                )
            }
        }
    }
    let first = clean
        .components()
        .next()
        .and_then(|component| match component {
            Component::Normal(part) => part.to_str(),
            _ => None,
        })
        .ok_or_else(|| "documentation path is not valid UTF-8".to_string())?;
    if first.eq_ignore_ascii_case(KNOWLEDGE_BASE_ROOT) {
        // Already canonical.
    } else if is_documentation_root(first) {
        clean =
            Path::new(KNOWLEDGE_BASE_ROOT).join(clean.components().skip(1).collect::<PathBuf>());
    } else {
        clean = Path::new(KNOWLEDGE_BASE_ROOT).join(clean);
    }
    if clean.file_name().is_none() {
        return Err("documentation path must name a file".into());
    }
    Ok(clean)
}

fn normalize_knowledge_folder(raw: &str) -> Result<PathBuf, String> {
    let trimmed = raw.trim().trim_start_matches(['/', '\\']);
    let candidate = if trimmed.is_empty() {
        PathBuf::from(KNOWLEDGE_BASE_ROOT)
    } else {
        let mut clean = PathBuf::new();
        for component in Path::new(trimmed).components() {
            match component {
                Component::Normal(part) => clean.push(part),
                _ => return Err("Knowledge Base folder must not contain traversal".into()),
            }
        }
        let first = clean
            .components()
            .next()
            .and_then(|component| match component {
                Component::Normal(part) => part.to_str(),
                _ => None,
            })
            .ok_or_else(|| "Knowledge Base folder is not valid UTF-8".to_string())?;
        if first.eq_ignore_ascii_case(KNOWLEDGE_BASE_ROOT) {
            clean
        } else if is_documentation_root(first) {
            Path::new(KNOWLEDGE_BASE_ROOT).join(clean.components().skip(1).collect::<PathBuf>())
        } else {
            Path::new(KNOWLEDGE_BASE_ROOT).join(clean)
        }
    };
    Ok(candidate)
}

pub fn create_knowledge_subfolder(
    project_root: &Path,
    parent: &Path,
    name: &str,
) -> Result<PathBuf, String> {
    ensure_knowledge_base(project_root)?;
    let name = name.trim();
    let mut components = Path::new(name).components();
    if name.is_empty()
        || name.starts_with('.')
        || !matches!(components.next(), Some(Component::Normal(_)))
        || components.next().is_some()
    {
        return Err("Folder name must be one non-hidden path component".into());
    }
    let parent = normalize_knowledge_folder(&relative_string(parent))?;
    let absolute_parent = project_root.join(&parent);
    if !absolute_parent.is_dir() {
        return Err(format!(
            "Knowledge Base parent does not exist: {}",
            parent.display()
        ));
    }
    let relative = parent.join(name);
    let absolute = project_root.join(&relative);
    if absolute.exists() {
        return Err(format!(
            "Knowledge Base folder already exists: {}",
            relative.display()
        ));
    }
    std::fs::create_dir(&absolute).map_err(|error| error.to_string())?;
    Ok(relative)
}

pub fn delete_knowledge_subfolder(project_root: &Path, folder: &Path) -> Result<PathBuf, String> {
    ensure_knowledge_base(project_root)?;
    let relative = normalize_knowledge_folder(&relative_string(folder))?;
    if relative == Path::new(KNOWLEDGE_BASE_ROOT) {
        return Err("The project Knowledge Base root cannot be deleted".into());
    }
    let absolute = project_root.join(&relative);
    if !absolute.is_dir() {
        return Err(format!(
            "Knowledge Base folder does not exist: {}",
            relative.display()
        ));
    }
    std::fs::remove_dir_all(&absolute).map_err(|error| error.to_string())?;
    sync_documentation(project_root)?;
    Ok(relative)
}

fn relative_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Write (or overwrite) one document inside an open table. The insert replaces
/// any existing row for the same path, which is what the old `ON CONFLICT DO
/// UPDATE` did.
fn index_document_into(
    table: &mut redb::Table<&'static str, &'static [u8]>,
    relative_path: &Path,
    content: &str,
) -> Result<(), String> {
    let path = relative_string(relative_path);
    let vector = vectorize(&format!("{path}\n{content}"));
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0);
    let record = DocumentRecord {
        content: content.to_owned(),
        embedding: vector,
        dimensions: VECTOR_DIMENSIONS as u32,
        updated_unix: now,
    };
    let encoded = encode_record(&record)?;
    table
        .insert(path.as_str(), encoded.as_slice())
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn index_document(
    project_root: &Path,
    relative_path: &Path,
    content: &str,
) -> Result<(), String> {
    let relative_path = normalize_document_path(&relative_string(relative_path))?;
    let database = open(project_root)?;
    let write = database.begin_write().map_err(|e| e.to_string())?;
    {
        let mut table = write.open_table(DOCUMENTS).map_err(|e| e.to_string())?;
        index_document_into(&mut table, &relative_path, content)?;
    }
    write.commit().map_err(|e| e.to_string())
}

fn is_text_document(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "md" | "markdown"
                    | "txt"
                    | "rst"
                    | "adoc"
                    | "html"
                    | "htm"
                    | "json"
                    | "yaml"
                    | "yml"
                    | "toml"
                    | "csv"
            )
        })
        .unwrap_or(false)
}

fn collect_documents(
    project_root: &Path,
    directory: &Path,
    documents: &mut Vec<(PathBuf, String)>,
) -> Result<(), String> {
    if !directory.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(directory).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            collect_documents(project_root, &path, documents)?;
        } else if is_text_document(&path) {
            let content = match std::fs::read_to_string(&path) {
                Ok(content) => content,
                Err(_) => continue,
            };
            let relative = path
                .strip_prefix(project_root)
                .map_err(|e| e.to_string())?
                .to_path_buf();
            documents.push((relative, content));
        }
    }
    Ok(())
}

/// Synchronize every textual file under the project Knowledge Base, including
/// files the developer added outside Grace. Missing files are removed from the
/// index. Returns the number of indexed documents.
pub fn sync_documentation(project_root: &Path) -> Result<usize, String> {
    ensure_knowledge_base(project_root)?;
    let mut documents = Vec::new();
    collect_documents(
        project_root,
        &project_root.join(KNOWLEDGE_BASE_ROOT),
        &mut documents,
    )?;
    let database = open(project_root)?;
    let write = database.begin_write().map_err(|e| e.to_string())?;
    {
        let mut table = write.open_table(DOCUMENTS).map_err(|e| e.to_string())?;
        let mut current = HashSet::new();
        for (path, content) in &documents {
            current.insert(relative_string(path));
            index_document_into(&mut table, path, content)?;
        }
        // Documents that vanished from disk leave the index with them. Collect
        // first: removing while iterating would borrow the table twice.
        let stale: Vec<String> = table
            .iter()
            .map_err(|e| e.to_string())?
            .filter_map(Result::ok)
            .map(|(key, _)| key.value().to_owned())
            .filter(|path| !current.contains(path))
            .collect();
        for path in stale {
            table.remove(path.as_str()).map_err(|e| e.to_string())?;
        }
    }
    write.commit().map_err(|e| e.to_string())?;
    Ok(documents.len())
}

/// Write and immediately index a documentation file. If indexing fails, restore
/// the previous file contents so Grace cannot report an unindexed document.
pub fn write_document(
    project_root: &Path,
    raw_path: &str,
    content: &str,
) -> Result<PathBuf, String> {
    ensure_knowledge_base(project_root)?;
    let relative = normalize_document_path(raw_path)?;
    let absolute = project_root.join(&relative);
    if let Some(parent) = absolute.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let previous = std::fs::read(&absolute).ok();
    std::fs::write(&absolute, content).map_err(|e| e.to_string())?;
    if let Err(error) = index_document(project_root, &relative, content) {
        match previous {
            Some(bytes) => {
                let _ = std::fs::write(&absolute, bytes);
            }
            None => {
                let _ = std::fs::remove_file(&absolute);
            }
        }
        return Err(format!(
            "document indexing failed; file write was rolled back: {error}"
        ));
    }
    Ok(relative)
}

pub fn documentation_paths(project_root: &Path) -> Result<Vec<String>, String> {
    ensure_knowledge_base(project_root)?;
    fn collect_paths(
        project_root: &Path,
        directory: &Path,
        paths: &mut Vec<String>,
    ) -> Result<(), String> {
        for entry in std::fs::read_dir(directory).map_err(|error| error.to_string())? {
            let path = entry.map_err(|error| error.to_string())?.path();
            if path.is_dir() {
                collect_paths(project_root, &path, paths)?;
            } else {
                let relative = path
                    .strip_prefix(project_root)
                    .map_err(|error| error.to_string())?;
                paths.push(relative_string(relative));
            }
        }
        Ok(())
    }

    let mut paths = Vec::new();
    collect_paths(
        project_root,
        &project_root.join(KNOWLEDGE_BASE_ROOT),
        &mut paths,
    )?;
    paths.sort();
    paths.dedup();
    Ok(paths)
}

pub fn search(project_root: &Path, query: &str, limit: usize) -> Result<Vec<KnowledgeHit>, String> {
    if query.trim().is_empty() || limit == 0 {
        return Ok(Vec::new());
    }
    let database = open(project_root)?;
    let query_vector = vectorize(query);
    let read = database.begin_read().map_err(|e| e.to_string())?;
    // A project that has never been indexed has no table yet — that is an empty
    // result, not a failure.
    let table = match read.open_table(DOCUMENTS) {
        Ok(table) => table,
        Err(redb::TableError::TableDoesNotExist(_)) => return Ok(Vec::new()),
        Err(e) => return Err(e.to_string()),
    };
    let mut hits = Vec::new();
    for entry in table.iter().map_err(|e| e.to_string())? {
        let (key, value) = entry.map_err(|e| e.to_string())?;
        let Some(record) = decode_record(value.value()) else {
            continue;
        };
        let score = query_vector
            .iter()
            .zip(record.embedding.iter())
            .map(|(left, right)| left * right)
            .sum::<f32>();
        if score <= 0.0 {
            continue;
        }
        hits.push(KnowledgeHit {
            path: key.value().to_owned(),
            score,
            excerpt: record.content.chars().take(1600).collect(),
        });
    }
    hits.sort_by(|left, right| right.score.total_cmp(&left.score));
    hits.truncate(limit);
    Ok(hits)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "prc-project-knowledge-{tag}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn knowledge_write_is_scoped_indexed_and_retrievable() {
        let root = project("write");
        std::fs::create_dir_all(&root).unwrap();
        let path = write_document(
            &root,
            "/Knowledge Base/Projects/PowerRustERP/plan/overview.md",
            "Accounts payable invoice approval and vendor payment workflow.",
        )
        .unwrap();
        assert_eq!(
            relative_string(&path),
            "Knowledge Base/Projects/PowerRustERP/plan/overview.md"
        );
        assert!(database_path(&root).exists());
        let hits = search(&root, "vendor invoice payment", 3).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].path, relative_string(&path));
        assert!(normalize_document_path("../outside.md").is_err());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn legacy_documentation_is_migrated_without_overwriting_conflicts() {
        let root = project("migration");
        std::fs::create_dir_all(root.join("Documentation/Plans")).unwrap();
        std::fs::create_dir_all(root.join("docs/Plans")).unwrap();
        std::fs::create_dir_all(root.join("Knowledge Base/Plans")).unwrap();
        std::fs::write(root.join("Knowledge Base/Plans/erp.md"), "canonical").unwrap();
        std::fs::write(root.join("Documentation/Plans/erp.md"), "legacy one").unwrap();
        std::fs::write(root.join("docs/Plans/tasks.md"), "legacy tasks").unwrap();

        assert_eq!(ensure_knowledge_base(&root).unwrap(), 2);
        assert!(!root.join("Documentation").exists());
        assert!(!root.join("docs").exists());
        assert_eq!(
            std::fs::read_to_string(root.join("Knowledge Base/Plans/erp.md")).unwrap(),
            "canonical"
        );
        assert_eq!(
            std::fs::read_to_string(root.join("Knowledge Base/Plans/erp-legacy-1.md")).unwrap(),
            "legacy one"
        );
        assert!(root.join("Knowledge Base/Plans/tasks.md").exists());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn knowledge_subfolders_are_scoped_and_deletable() {
        let root = project("folders");
        std::fs::create_dir_all(&root).unwrap();
        ensure_knowledge_base(&root).unwrap();
        let projects =
            create_knowledge_subfolder(&root, Path::new(KNOWLEDGE_BASE_ROOT), "Projects").unwrap();
        let erp = create_knowledge_subfolder(&root, &projects, "ERP").unwrap();
        std::fs::write(root.join(&erp).join("plan.md"), "ERP plan evidence").unwrap();
        assert_eq!(sync_documentation(&root).unwrap(), 1);
        assert_eq!(search(&root, "ERP plan", 2).unwrap().len(), 1);
        assert!(create_knowledge_subfolder(&root, &projects, "../outside").is_err());
        assert!(delete_knowledge_subfolder(&root, Path::new(KNOWLEDGE_BASE_ROOT)).is_err());
        assert_eq!(delete_knowledge_subfolder(&root, &erp).unwrap(), erp);
        assert!(search(&root, "ERP plan", 2).unwrap().is_empty());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn sync_indexes_user_documents_and_removes_deleted_entries() {
        let root = project("sync");
        let docs = root.join(KNOWLEDGE_BASE_ROOT);
        std::fs::create_dir_all(&docs).unwrap();
        let file = docs.join("requirements.md");
        std::fs::write(&file, "warehouse replenishment requirements").unwrap();
        std::fs::write(docs.join("architecture.pdf"), b"%PDF-test").unwrap();
        assert_eq!(sync_documentation(&root).unwrap(), 1);
        assert_eq!(
            documentation_paths(&root).unwrap(),
            vec![
                "Knowledge Base/architecture.pdf".to_string(),
                "Knowledge Base/requirements.md".to_string(),
            ]
        );
        assert_eq!(
            search(&root, "warehouse replenishment", 2).unwrap().len(),
            1
        );
        std::fs::remove_file(file).unwrap();
        assert_eq!(sync_documentation(&root).unwrap(), 0);
        assert!(search(&root, "warehouse replenishment", 2)
            .unwrap()
            .is_empty());
        let _ = std::fs::remove_dir_all(root);
    }
}
