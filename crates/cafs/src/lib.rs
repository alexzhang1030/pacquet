#![allow(unused)]

use std::{
    fs,
    path::{Path, PathBuf},
};

use miette::Diagnostic;
use ssri::{Algorithm, IntegrityOpts};
use thiserror::Error;

#[derive(Error, Debug, Diagnostic)]
#[non_exhaustive]
pub enum CafsError {
    #[error(transparent)]
    #[diagnostic(code(pacquet_cafs::io_error))]
    Io(#[from] std::io::Error),
}

enum FileType {
    Exec,
    NonExec,
    Index,
}

fn content_path_from_hex(file_type: FileType, hex: &str) -> PathBuf {
    let mut p = PathBuf::new();
    p.push(&hex[0..2]);

    let extension = match file_type {
        FileType::Exec => "-exec",
        FileType::NonExec => "",
        FileType::Index => "-index.json",
    };

    p.join(format!("{}{}", &hex[2..], extension))
}

pub fn write_sync<P: Into<PathBuf>>(store_dir: P, buffer: &Vec<u8>) -> Result<PathBuf, CafsError> {
    let hex_integrity =
        IntegrityOpts::new().algorithm(Algorithm::Sha512).chain(buffer).result().to_hex().1;
    let content_path = content_path_from_hex(FileType::NonExec, &hex_integrity);
    let file_path = store_dir.into().join(&content_path);

    if !file_path.exists() {
        let parent_dir = file_path.parent().unwrap();
        fs::create_dir_all(parent_dir)?;
        fs::write(&file_path, buffer)?;
    }

    Ok(content_path)
}

pub fn prune_sync<P: Into<PathBuf>>(store_dir: P) -> Result<(), CafsError> {
    // TODO: This should remove unreferenced packages, not all packages.
    // Ref: https://pnpm.io/cli/store#prune
    fs::remove_dir_all(store_dir.into())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{env, str::FromStr};

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn create_content_path_from_hex() {
        assert_eq!(
            content_path_from_hex(FileType::NonExec, "1234567890abcdef1234567890abcdef12345678"),
            PathBuf::from("12/34567890abcdef1234567890abcdef12345678")
        );
        assert_eq!(
            content_path_from_hex(FileType::Exec, "1234567890abcdef1234567890abcdef12345678"),
            PathBuf::from("12/34567890abcdef1234567890abcdef12345678-exec")
        );
        assert_eq!(
            content_path_from_hex(FileType::Index, "1234567890abcdef1234567890abcdef12345678"),
            PathBuf::from("12/34567890abcdef1234567890abcdef12345678-index.json")
        );
    }

    #[test]
    fn should_write_and_clear() {
        let dir = tempdir().unwrap();
        let buffer = vec![0, 1, 2, 3, 4, 5, 6];
        let saved_file_path = write_sync(dir.path(), &buffer).unwrap();
        let store_path = dir.path().join(saved_file_path);
        assert!(store_path.exists());
        prune_sync(dir.path()).unwrap();
        assert!(!store_path.exists());
    }
}
