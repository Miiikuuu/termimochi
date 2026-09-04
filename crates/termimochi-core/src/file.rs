use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};

static NEXT_TEMP_FILE: AtomicU64 = AtomicU64::new(0);

/// Replace a file without exposing a partially-written destination.
///
/// Existing Unix permission bits are retained. A read-only destination is
/// deliberately refused instead of being replaced through its writable parent
/// directory.
pub fn write_atomically(path: &Path, contents: &[u8]) -> io::Result<()> {
    write_atomically_with_nonce(path, contents, || {
        NEXT_TEMP_FILE.fetch_add(1, Ordering::Relaxed)
    })
}

fn write_atomically_with_nonce(
    path: &Path,
    contents: &[u8],
    mut next_nonce: impl FnMut() -> u64,
) -> io::Result<()> {
    let existing_permissions = match fs::metadata(path) {
        Ok(metadata) => {
            if metadata.permissions().readonly() {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    format!("{} is read-only", path.display()),
                ));
            }
            Some(metadata.permissions())
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error),
    };

    let parent = non_empty_parent(path);
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy())
        .unwrap_or_else(|| "palette".into());

    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    if let Some(permissions) = &existing_permissions {
        options.mode(permissions.mode() & 0o777);
    }

    let mut owned_temporary = None;
    for _ in 0..128 {
        let nonce = next_nonce();
        let temporary = parent.join(format!(
            ".{file_name}.termimochi-{}-{nonce}.tmp",
            std::process::id()
        ));
        match options.open(&temporary) {
            Ok(file) => {
                owned_temporary = Some((file, temporary));
                break;
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    let (mut file, temporary) = owned_temporary.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!(
                "cannot create a unique temporary file for {}",
                path.display()
            ),
        )
    })?;

    let result = (|| {
        if let Some(permissions) = &existing_permissions {
            #[cfg(unix)]
            file.set_permissions(fs::Permissions::from_mode(permissions.mode() & 0o777))?;
            #[cfg(not(unix))]
            file.set_permissions(permissions.clone())?;
        }
        file.write_all(contents)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary, path)
    })();

    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

/// Determine whether two paths identify the same file, including symlinks and
/// hard links. Lexically-equivalent missing destinations are also detected.
pub fn paths_refer_to_same_file(first: &Path, second: &Path) -> io::Result<bool> {
    if normalized_absolute(first)? == normalized_absolute(second)? {
        return Ok(true);
    }

    let first_metadata = fs::metadata(first)?;
    match fs::metadata(second) {
        Ok(second_metadata) => {
            #[cfg(unix)]
            if first_metadata.dev() == second_metadata.dev()
                && first_metadata.ino() == second_metadata.ino()
            {
                return Ok(true);
            }

            Ok(fs::canonicalize(first)? == fs::canonicalize(second)?)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let first = fs::canonicalize(first)?;
            let Some(file_name) = second.file_name() else {
                return Ok(false);
            };
            let parent = non_empty_parent(second);
            match fs::canonicalize(parent) {
                Ok(parent) => Ok(first == parent.join(file_name)),
                Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
                Err(error) => Err(error),
            }
        }
        Err(error) => Err(error),
    }
}

fn non_empty_parent(path: &Path) -> &Path {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}

fn normalized_absolute(path: &Path) -> io::Result<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir()?.join(path)
    };
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            component => normalized.push(component.as_os_str()),
        }
    }
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "termimochi-file-test-{}-{}-{name}",
            std::process::id(),
            NEXT_TEMP_FILE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        root
    }

    #[test]
    fn atomic_write_replaces_complete_file() {
        let directory = fixture("replace");
        let target = directory.join("theme.palette");
        fs::write(&target, "old").unwrap();

        write_atomically(&target, b"new palette").unwrap();

        assert_eq!(fs::read_to_string(&target).unwrap(), "new palette");
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn atomic_write_does_not_remove_a_colliding_unowned_temp_file() {
        let directory = fixture("temp-collision");
        let target = directory.join("theme.palette");
        let colliding_nonce = 41;
        let available_nonce = 42;
        let colliding = directory.join(format!(
            ".theme.palette.termimochi-{}-{colliding_nonce}.tmp",
            std::process::id()
        ));
        fs::write(&colliding, "owned by another writer").unwrap();

        let mut nonces = [colliding_nonce, available_nonce].into_iter();
        write_atomically_with_nonce(&target, b"new palette", || {
            nonces.next().expect("a free nonce is available")
        })
        .unwrap();

        assert_eq!(fs::read(&target).unwrap(), b"new palette");
        assert_eq!(
            fs::read_to_string(&colliding).unwrap(),
            "owned by another writer"
        );
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 2);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn atomic_write_stops_after_repeated_temp_file_collisions() {
        let directory = fixture("repeated-temp-collision");
        let target = directory.join("theme.palette");
        let colliding_nonce = 73;
        let colliding = directory.join(format!(
            ".theme.palette.termimochi-{}-{colliding_nonce}.tmp",
            std::process::id()
        ));
        fs::write(&colliding, "owned by another writer").unwrap();

        let error = write_atomically_with_nonce(&target, b"new palette", || colliding_nonce)
            .expect_err("repeated collisions must eventually fail");

        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert!(!target.exists());
        assert_eq!(
            fs::read_to_string(&colliding).unwrap(),
            "owned by another writer"
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn atomic_write_preserves_existing_permissions() {
        let directory = fixture("permissions");
        let target = directory.join("private.palette");
        fs::write(&target, "old").unwrap();
        fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).unwrap();

        write_atomically(&target, b"new").unwrap();

        assert_eq!(
            fs::metadata(&target).unwrap().permissions().mode() & 0o777,
            0o600
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn atomic_write_refuses_read_only_destinations() {
        let directory = fixture("read-only");
        let target = directory.join("locked.palette");
        fs::write(&target, "keep").unwrap();
        let mut permissions = fs::metadata(&target).unwrap().permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&target, permissions).unwrap();

        let error = write_atomically(&target, b"replace").unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        assert_eq!(fs::read_to_string(&target).unwrap(), "keep");
        #[cfg(not(unix))]
        {
            let mut permissions = fs::metadata(&target).unwrap().permissions();
            permissions.set_readonly(false);
            fs::set_permissions(&target, permissions).unwrap();
        }
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn same_file_detection_covers_relative_and_distinct_paths() {
        let directory = fixture("identity");
        let source = directory.join("source.palette");
        fs::write(&source, "palette").unwrap();

        assert!(paths_refer_to_same_file(&source, &source).unwrap());
        assert!(!paths_refer_to_same_file(&source, &directory.join("new.conf")).unwrap());
        fs::remove_dir_all(directory).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn same_file_detection_follows_links() {
        use std::os::unix::fs::symlink;

        let directory = fixture("links");
        let source = directory.join("source.palette");
        let symbolic = directory.join("symbolic.conf");
        let hard = directory.join("hard.conf");
        fs::write(&source, "palette").unwrap();
        symlink(&source, &symbolic).unwrap();
        fs::hard_link(&source, &hard).unwrap();

        assert!(paths_refer_to_same_file(&source, &symbolic).unwrap());
        assert!(paths_refer_to_same_file(&source, &hard).unwrap());
        fs::remove_dir_all(directory).unwrap();
    }
}
