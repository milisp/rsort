use clap::Parser;
use rayon::prelude::*;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use walkdir::WalkDir;
use ignore::Walk;

mod imports;
use crate::imports::find_import_blocks;
use crate::imports::group_and_sort_imports;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to file or directory
    path: String,

    /// Number of threads for parallel processing
    #[arg(short = 't', long, default_value = "4")]
    threads: usize,
}

fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let args = Args::parse();
    let path = Path::new(&args.path);

    rayon::ThreadPoolBuilder::new()
        .num_threads(args.threads)
        .build_global()?;

    let processed_files = Arc::new(Mutex::new(Vec::new()));

    // Load .gitignore rules
    let gitignore_path = path.join(".gitignore");
    let mut ignore_rules = Vec::new();
    if gitignore_path.exists() {
        let walker = Walk::new(&path);
        for entry in walker {
            if let Ok(entry) = entry {
                if entry.file_type().map_or(false, |ft| ft.is_file()) {
                    ignore_rules.push(entry.path().to_path_buf());
                }
            }
        }
    }

    // Add common Python environment paths to ignore rules
    let python_envs = vec!["venv", ".venv", "env", ".env", "__pypackages__", "envs", ".virtualenvs"];
    for env in python_envs {
        let env_path = path.join(env);
        if env_path.exists() {
            ignore_rules.push(env_path);
        }
    }

    // Check if the file path contains any ignored environment paths
    let is_ignored = |file_path: &Path| {
        ignore_rules.iter().any(|ignore_path| file_path.starts_with(ignore_path))
    };

    if path.is_file() {
        if path.extension().map_or(false, |ext| ext == "py") && !is_ignored(&path) {
            process_file(path, &processed_files)?;
        } else {
            println!("Not a Python file or ignored: {}", path.display());
        }
    } else {
        // 收集所有Python文件路径
        let py_files: Vec<_> = WalkDir::new(path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "py") && !is_ignored(e.path()))
            .map(|e| e.path().to_owned())
            .collect();

        // 并行处理文件
        py_files
            .par_iter()
            .try_for_each(|file_path| process_file(file_path, &processed_files))?;
    }

    println!("\nProcessed files:");
    let files = processed_files.lock().unwrap();
    for file in files.iter() {
        println!("✓ {}", file.display());
    }

    Ok(())
}

fn process_file(
    path: &Path,
    processed_files: &Arc<Mutex<Vec<PathBuf>>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    println!("Processing: {}", path.display());

    let content = fs::read_to_string(path)?;
    let lines: Vec<&str> = content.lines().collect();

    let import_blocks = find_import_blocks(&lines);
    let mut new_content = String::new();
    let mut last_end = 0;

    for block in import_blocks {
        // 添加非导入内容，并删除末尾多余的空行
        let preceding_content = lines[last_end..block.start_line]
            .join("\n")
            .trim_end()
            .to_string();
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

    if content != new_content {
        fs::write(path, new_content)?;
        // Record processed file only if it was actually modified
        processed_files.lock().unwrap().push(path.to_owned());
        println!("✓ Updated: {}", path.display());
    } else {
        println!("⚡ No changes needed: {}", path.display());
    }

    Ok(())
}
