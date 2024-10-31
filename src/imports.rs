// src/import.rs
use regex::Regex;

#[derive(Debug)]
pub struct ImportBlock {
    pub imports: Vec<String>,
    pub start_line: usize,
    pub end_line: usize,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Copy, Clone)]
pub enum ImportGroup {
    Future,
    StandardLib,
    ThirdParty,
    LocalLib,
}

#[derive(Debug)]
pub struct GroupedImport {
    pub group: ImportGroup,
    pub line: String,
}

pub fn find_import_blocks(lines: &[&str]) -> Vec<ImportBlock> {
    let mut blocks = Vec::new();
    let mut current_block: Option<ImportBlock> = None;
    let import_re = Regex::new(r"^(from\s+\S+\s+import\s+\S+|import\s+\S+)").unwrap();

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if import_re.is_match(trimmed) {
            if let Some(ref mut block) = current_block {
                block.imports.push(trimmed.to_string());
                block.end_line = i;
            } else {
                current_block = Some(ImportBlock {
                    imports: vec![trimmed.to_string()],
                    start_line: i,
                    end_line: i,
                });
            }
        } else if trimmed.is_empty() && current_block.is_some() {
            continue;
        } else if let Some(block) = current_block.take() {
            blocks.push(block);
        }
    }

    if let Some(block) = current_block {
        blocks.push(block);
    }

    blocks
}

pub fn determine_import_group(import: &str) -> ImportGroup {
    if import.contains("__future__") {
        return ImportGroup::Future;
    }

    if import.starts_with("from .") || import.starts_with("from ..") {
        return ImportGroup::LocalLib;
    }

    let module = if import.starts_with("from ") {
        import.split_whitespace().nth(1).unwrap_or("")
    } else {
        import.split_whitespace().nth(1).unwrap_or("")
    };

    let stdlib_modules = [
        "os",
        "sys",
        "time",
        "datetime",
        "collections",
        "random",
        "math",
        "json",
        "re",
        "pathlib",
        "typing", // 添加更多标准库模块...
    ];

    if stdlib_modules.contains(&module.split('.').next().unwrap_or("")) {
        ImportGroup::StandardLib
    } else {
        ImportGroup::ThirdParty
    }
}

pub fn group_and_sort_imports(imports: &[String]) -> Vec<GroupedImport> {
    let mut grouped: Vec<GroupedImport> = imports
        .iter()
        .map(|import| GroupedImport {
            group: determine_import_group(import),
            line: import.clone(),
        })
        .collect();

    // 首先按组排序，然后在每组内按字母顺序排序
    grouped.sort_by(|a, b| {
        match a.group.cmp(&b.group) {
            std::cmp::Ordering::Equal => {
                // 在同一组内，import 优先于 from import
                if a.line.starts_with("import") && b.line.starts_with("from") {
                    std::cmp::Ordering::Less
                } else if a.line.starts_with("from") && b.line.starts_with("import") {
                    std::cmp::Ordering::Greater
                } else {
                    a.line.to_lowercase().cmp(&b.line.to_lowercase())
                }
            }
            other => other,
        }
    });

    grouped
}
