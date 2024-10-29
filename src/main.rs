use std::fs;
use std::path::{Path, PathBuf};
use std::error::Error;
use std::time::{SystemTime, UNIX_EPOCH};
use std::sync::{Arc, Mutex};
use clap::Parser;
use regex::Regex;
use walkdir::WalkDir;
use rayon::prelude::*;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to file or directory
    path: String,

    /// Number of threads for parallel processing
    #[arg(short='t', long, default_value = "4")]
    threads: usize,
}

#[derive(Debug)]
struct ImportBlock {
    imports: Vec<String>,
    start_line: usize,
    end_line: usize,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Copy, Clone)]
enum ImportGroup {
    Future,
    StandardLib,
    ThirdParty,
    LocalLib,
}

#[derive(Debug)]
struct GroupedImport {
    group: ImportGroup,
    line: String,
}

fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let args = Args::parse();
    let path = Path::new(&args.path);
    
    rayon::ThreadPoolBuilder::new()
        .num_threads(args.threads)
        .build_global()?;

    let processed_files = Arc::new(Mutex::new(Vec::new()));

    if path.is_file() {
        if path.extension().map_or(false, |ext| ext == "py") {
            process_file(path, &processed_files)?;
        } else {
            println!("Not a Python file: {}", path.display());
        }
    } else {
        // 收集所有Python文件路径
        let py_files: Vec<_> = WalkDir::new(path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "py"))
            .map(|e| e.path().to_owned())
            .collect();

        // 并行处理文件
        py_files.par_iter().try_for_each(|file_path| {
            process_file(file_path, &processed_files)
        })?;
    }

    println!("\nProcessed files:");
    let files = processed_files.lock().unwrap();
    for file in files.iter() {
        println!("✓ {}", file.display());
    }

    Ok(())
}

fn create_backup(path: &Path) -> Result<(), Box<dyn Error + Send + Sync>> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_secs();
    
    // Create backup in system temp directory
    let temp_dir = std::env::temp_dir();
    let file_name = path.file_name()
        .ok_or("Failed to get file name")?
        .to_string_lossy();
    let backup_path = temp_dir.join(format!("{}.{}.bak", file_name, timestamp));
    
    fs::copy(path, &backup_path)?;
    println!("Created backup: {}", backup_path.display());
    Ok(())
}

fn process_file(path: &Path, processed_files: &Arc<Mutex<Vec<PathBuf>>>) -> Result<(), Box<dyn Error + Send + Sync>> {
    println!("Processing: {}", path.display());
    
    let content = fs::read_to_string(path)?;
    let lines: Vec<&str> = content.lines().collect();
    
    let import_blocks = find_import_blocks(&lines);
    let mut new_content = String::new();
    let mut last_end = 0;

    for block in import_blocks {
        // 添加非导入内容，并删除末尾多余的空行
        let preceding_content = lines[last_end..block.start_line].join("\n").trim_end().to_string();
        if !preceding_content.is_empty() {
            new_content.push_str(&preceding_content);
            new_content.push_str("\n\n"); // 确保有两个空行
        }

        // 对导入进行分组和排序
        let grouped_imports = group_and_sort_imports(&block.imports);
        
        // 按组添加导入语句
        let mut current_group = None;
        for import in grouped_imports {
            if current_group != Some(import.group) {
                if current_group.is_some() {
                    new_content.push('\n');
                }
                current_group = Some(import.group);
            }
            new_content.push_str(&import.line);
            new_content.push('\n');
        }

        // 确保导入块后有两个空行（不多不少）
        new_content.push_str("\n\n");

        last_end = block.end_line + 1;
    }

    // 添加剩余内容，确保开头没有多余的空行
    if last_end < lines.len() {
        let remaining_content = lines[last_end..].join("\n").trim_start().to_string();
        if !remaining_content.is_empty() {
            new_content.push_str(&remaining_content);
        }
    }

    // Only create backup and write file if content has changed
    if content != new_content {
        create_backup(path)?;
        fs::write(path, new_content)?;
        // Record processed file only if it was actually modified
        processed_files.lock().unwrap().push(path.to_owned());
        println!("✓ Updated: {}", path.display());
    } else {
        println!("⚡ No changes needed: {}", path.display());
    }
    
    Ok(())
}

fn find_import_blocks(lines: &[&str]) -> Vec<ImportBlock> {
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

fn determine_import_group(import: &str) -> ImportGroup {
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
        "os", "sys", "time", "datetime", "collections", "random", 
        "math", "json", "re", "pathlib", "typing"
        // 添加更多标准库模块...
    ];

    if stdlib_modules.contains(&module.split('.').next().unwrap_or("")) {
        ImportGroup::StandardLib
    } else {
        ImportGroup::ThirdParty
    }
}

fn group_and_sort_imports(imports: &[String]) -> Vec<GroupedImport> {
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
            },
            other => other,
        }
    });

    grouped
}