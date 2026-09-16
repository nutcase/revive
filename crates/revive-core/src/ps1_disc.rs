//! Disc archives are extracted into an owned, temporary directory for the core.
use crate::system::Result;
use std::{
    fs::{self, File},
    io,
    path::{Path, PathBuf},
};
use tempfile::TempDir;
use zip::ZipArchive;

pub(crate) struct Ps1Disc {
    pub path: PathBuf,
    _extracted: Option<TempDir>,
}
const MAX_FILES: usize = 4096;
const MAX_BYTES: u64 = 4 * 1024 * 1024 * 1024;

pub(crate) fn zip_has_cue(path: &Path) -> Result<bool> {
    let mut zip = open_zip(path)?;
    check_archive(&mut zip)?;
    Ok((0..zip.len()).any(|i| zip.by_index(i).is_ok_and(|f| is_cue(Path::new(f.name())))))
}
fn open_zip(path: &Path) -> Result<ZipArchive<File>> {
    ZipArchive::new(File::open(path).map_err(|e| e.to_string())?)
        .map_err(|e| format!("Invalid disc ZIP: {e}"))
}
fn is_cue(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("cue"))
}
fn check_archive(zip: &mut ZipArchive<File>) -> Result<()> {
    if zip.len() > MAX_FILES {
        return Err("Disc ZIP has too many entries".into());
    }
    let mut size = 0u64;
    let mut paths = std::collections::HashSet::new();
    for i in 0..zip.len() {
        let f = zip.by_index(i).map_err(|e| e.to_string())?;
        let path = f.enclosed_name().ok_or("Unsafe path in disc ZIP")?;
        if path.as_os_str().is_empty() || f.name().contains('\\') || f.name().contains(':') {
            return Err("Unsafe path in disc ZIP".into());
        }
        if !paths.insert(path.to_string_lossy().to_lowercase()) {
            return Err("Duplicate path in disc ZIP".into());
        }
        if f.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000) {
            return Err("Symlinks are not allowed in disc ZIPs".into());
        }
        size = size.checked_add(f.size()).ok_or("Disc ZIP is too large")?;
        if size > MAX_BYTES {
            return Err("Disc ZIP exceeds the 4 GiB extraction limit".into());
        }
    }
    Ok(())
}
impl Ps1Disc {
    pub fn open(path: &Path) -> Result<Self> {
        if !path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("zip"))
        {
            let path = fs::canonicalize(path).map_err(|e| e.to_string())?;
            if is_cue(&path) {
                validate_cue(&path, None)?;
            }
            return Ok(Self {
                path,
                _extracted: None,
            });
        }
        let mut zip = open_zip(path)?;
        check_archive(&mut zip)?;
        let cues: Vec<_> = (0..zip.len())
            .filter_map(|i| {
                let f = zip.by_index(i).ok()?;
                if !f.is_dir() && is_cue(Path::new(f.name())) {
                    f.enclosed_name()
                } else {
                    None
                }
            })
            .collect();
        if cues.len() != 1 {
            return Err(format!(
                "Disc ZIP must contain exactly one CUE (found {}); extract it and select a CUE",
                cues.len()
            ));
        }
        let temp = tempfile::Builder::new()
            .prefix("revive-ps1-")
            .tempdir()
            .map_err(|e| e.to_string())?;
        for i in 0..zip.len() {
            let mut f = zip.by_index(i).map_err(|e| e.to_string())?;
            let dest = temp
                .path()
                .join(f.enclosed_name().ok_or("Unsafe ZIP path")?);
            if f.is_dir() {
                fs::create_dir_all(&dest).map_err(|e| e.to_string())?;
                continue;
            }
            fs::create_dir_all(dest.parent().unwrap()).map_err(|e| e.to_string())?;
            let mut output = File::create(dest).map_err(|e| e.to_string())?;
            io::copy(&mut f, &mut output)
                .map_err(|e| format!("Disc ZIP extraction failed: {e}"))?;
        }
        let path = temp.path().join(&cues[0]);
        validate_cue(&path, Some(temp.path()))?;
        Ok(Self {
            path,
            _extracted: Some(temp),
        })
    }
}
fn validate_cue(path: &Path, root: Option<&Path>) -> Result<()> {
    if fs::metadata(path).map_err(|e| e.to_string())?.len() > 1024 * 1024 {
        return Err("CUE exceeds the 1 MiB size limit".into());
    }
    let cue = fs::read_to_string(path).map_err(|e| format!("Cannot read CUE: {e}"))?;
    let root = root
        .map(fs::canonicalize)
        .transpose()
        .map_err(|e| e.to_string())?;
    let mut tracks = 0;
    for line in cue.lines() {
        let line = line.trim_start();
        let Some((keyword, rest)) = line.split_once(char::is_whitespace) else {
            continue;
        };
        if !keyword.eq_ignore_ascii_case("FILE") {
            continue;
        }
        let rest = rest.trim_start();
        let name = if let Some(quoted) = rest.strip_prefix('"') {
            quoted.split_once('"').map(|(name, _)| name)
        } else {
            rest.split_whitespace().next()
        }
        .ok_or("Invalid FILE entry in CUE")?;
        if name.is_empty() {
            return Err("Empty track name in CUE".into());
        }
        if root.is_some()
            && (Path::new(name).is_absolute() || name.contains('\\') || name.contains(':'))
        {
            return Err("Archived CUE tracks must use relative paths with forward slashes".into());
        }
        let track = path.parent().unwrap().join(name);
        let track =
            fs::canonicalize(&track).map_err(|e| format!("Missing CUE track '{name}': {e}"))?;
        if root.as_ref().is_some_and(|root| !track.starts_with(root)) {
            return Err("CUE track escapes the disc archive".into());
        }
        if !track.is_file() {
            return Err(format!("CUE track '{name}' is not a file"));
        }
        tracks += 1;
    }
    if tracks == 0 {
        return Err("CUE contains no FILE entries".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    fn archive(entries: &[(&str, &[u8])]) -> (TempDir, PathBuf) {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("disc.zip");
        let mut zip = zip::ZipWriter::new(File::create(&path).unwrap());
        for (name, bytes) in entries {
            zip.start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip.finish().unwrap();
        (temp, path)
    }
    #[test]
    fn extracts_nested_cue_and_tracks_and_removes_temporary_files() {
        let (_temp, path) = archive(&[
            (
                "disc/game.cue",
                b"FILE \"track.bin\" BINARY\n TRACK 01 MODE2/2352\n INDEX 01 00:00:00\n",
            ),
            ("disc/track.bin", b"track"),
        ]);
        assert!(zip_has_cue(&path).unwrap());
        let disc = Ps1Disc::open(&path).unwrap();
        assert_eq!(
            fs::read(disc.path.with_file_name("track.bin")).unwrap(),
            b"track"
        );
        let extracted = disc.path.clone();
        drop(disc);
        assert!(!extracted.exists());
    }
    #[test]
    fn rejects_missing_tracks_ambiguous_discs_and_path_traversal() {
        for entries in [
            vec![("game.cue", b"FILE \"missing.bin\" BINARY".as_slice())],
            vec![("a.cue", b"".as_slice()), ("b.cue", b"".as_slice())],
            vec![("../escape.cue", b"".as_slice())],
            vec![("a.cue", b"".as_slice()), ("A.CUE", b"".as_slice())],
        ] {
            let (_temp, path) = archive(&entries);
            assert!(Ps1Disc::open(&path).is_err());
        }
    }
    #[test]
    fn does_not_identify_unrelated_zip_as_ps1() {
        let (_temp, path) = archive(&[("game.nes", b"NES")]);
        assert!(!zip_has_cue(&path).unwrap());
    }
}
