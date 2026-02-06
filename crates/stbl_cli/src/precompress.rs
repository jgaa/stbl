use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::Result;
use brotli::CompressorWriter;
use flate2::{Compression, GzBuilder};
use std::io::Write;
use walkdir::WalkDir;

const COMPRESSIBLE_EXTS: &[&str] = &[
    "html",
    "css",
    "js",
    "mjs",
    "xml",
    "json",
    "svg",
    "txt",
    "map",
    "webmanifest",
    "rss",
];

pub fn is_compressible_path(path: &Path) -> bool {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some(ext) => is_compressible_ext(ext),
        None => false,
    }
}

pub fn gzip_path_for(path: &Path) -> Option<PathBuf> {
    let ext = path.extension().and_then(|ext| ext.to_str())?;
    if !is_compressible_ext(ext) {
        return None;
    }
    Some(path.with_extension(format!("{ext}.gz")))
}

pub fn brotli_path_for(path: &Path) -> Option<PathBuf> {
    let ext = path.extension().and_then(|ext| ext.to_str())?;
    if !is_compressible_ext(ext) {
        return None;
    }
    Some(path.with_extension(format!("{ext}.br")))
}

pub fn expected_gzip_outputs(plan: &stbl_core::model::BuildPlan) -> HashSet<String> {
    let mut out = HashSet::new();
    for task in &plan.tasks {
        for output in &task.outputs {
            if let Some(gz) = gzip_path_for(Path::new(&output.path)) {
                if let Some(rel) = gz.to_str() {
                    out.insert(rel.to_string());
                }
            }
        }
    }
    out
}

pub fn expected_brotli_outputs(plan: &stbl_core::model::BuildPlan) -> HashSet<String> {
    let mut out = HashSet::new();
    for task in &plan.tasks {
        for output in &task.outputs {
            if let Some(br) = brotli_path_for(Path::new(&output.path)) {
                if let Some(rel) = br.to_str() {
                    out.insert(rel.to_string());
                }
            }
        }
    }
    out
}

pub fn write_gzip_files(out_dir: &Path, level: u32, verbose: bool) -> Result<usize> {
    let mut count = 0usize;
    for entry in WalkDir::new(out_dir).min_depth(1).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if !is_compressible_path(path) {
            continue;
        }
        let Some(gz_path) = gzip_path_for(path) else {
            continue;
        };
        let raw = std::fs::read(path)?;
        let mut encoder = GzBuilder::new()
            .mtime(0)
            .write(Vec::new(), Compression::new(level));
        encoder.write_all(&raw)?;
        let compressed = encoder.finish()?;
        std::fs::write(&gz_path, compressed)?;
        count += 1;
    }
    if verbose {
        println!("precompress: wrote {} gzip files", count);
    }
    Ok(count)
}

pub fn write_brotli_files(out_dir: &Path, quality: u32, verbose: bool) -> Result<usize> {
    let mut count = 0usize;
    for entry in WalkDir::new(out_dir).min_depth(1).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if !is_compressible_path(path) {
            continue;
        }
        let Some(br_path) = brotli_path_for(path) else {
            continue;
        };
        let raw = std::fs::read(path)?;
        let mut encoder = CompressorWriter::new(Vec::new(), 4096, quality, 22);
        encoder.write_all(&raw)?;
        let compressed = encoder.into_inner();
        std::fs::write(&br_path, compressed)?;
        count += 1;
    }
    if verbose {
        println!("precompress: wrote {} brotli files", count);
    }
    Ok(count)
}

fn is_compressible_ext(ext: &str) -> bool {
    COMPRESSIBLE_EXTS
        .iter()
        .any(|candidate| ext.eq_ignore_ascii_case(candidate))
}
