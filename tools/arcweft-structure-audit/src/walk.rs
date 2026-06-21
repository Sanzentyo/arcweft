use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const IGNORED_DIRECTORIES: &[&str] = &[
    ".git",
    ".idea",
    ".vscode",
    "node_modules",
    "target",
    "vendor",
];

pub fn collect_files(root: &Path) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    visit(root, &mut files)?;
    files.sort();
    Ok(files)
}

fn visit(path: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
    let mut entries = fs::read_dir(path)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(std::fs::DirEntry::file_name);

    for entry in entries {
        let file_type = entry.file_type()?;
        let path = entry.path();
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            if !is_ignored_directory(&path) {
                visit(&path, files)?;
            }
        } else if file_type.is_file() {
            files.push(path);
        }
    }
    Ok(())
}

fn is_ignored_directory(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| IGNORED_DIRECTORIES.contains(&name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_build_directories_are_ignored() {
        assert!(is_ignored_directory(Path::new("target")));
        assert!(is_ignored_directory(Path::new("repo/.git")));
        assert!(!is_ignored_directory(Path::new("crates")));
    }
}
