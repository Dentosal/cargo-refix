use std::{
    collections::HashMap,
    fmt::Debug,
    fs::{self},
    ops,
    path::PathBuf,
};

/// A single change to a file
#[derive(Debug, Clone)]
pub struct Change {
    /// The file to change
    pub file: PathBuf,
    /// The actual replacement
    pub patch: Patch,
}

/// File-agnostic change to be applied
#[derive(Clone)]
pub struct Patch {
    /// The range of bytes to replace
    pub location: ops::Range<usize>,
    /// New bytes to replace the range with
    pub bytes: Vec<u8>,
}
impl Debug for Patch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = String::from_utf8_lossy(&self.bytes);
        f.debug_struct("Patch")
            .field("location", &self.location)
            .field("location", &text)
            .finish()
    }
}

/// All changes to a file, ready to be applied
#[derive(Debug, Clone)]
pub struct FileChangeSet {
    /// The file to change
    file: PathBuf,
    /// Changes
    /// Invariants: sorted, non-overlapping
    changes: Vec<Patch>,
}
impl FileChangeSet {
    /// Takes patches in the order they are applied, groups them by file,
    /// and sorts them by location correcting offsets, so they can be applied
    pub fn group(changes: Vec<Change>) -> Vec<FileChangeSet> {
        let mut change_sets: HashMap<PathBuf, Vec<Patch>> = HashMap::new();
        // Sort by file
        for change in changes {
            change_sets
                .entry(change.file)
                .or_default()
                .push(change.patch);
        }

        // Do in-file ordering for each file
        for patches in change_sets.values_mut() {
            // Do a stable sort so we preserve order if it matters
            patches.sort_by_key(|patch| patch.location.start);

            // // Correct offsets
            // let mut displacement: isize = 0;

            // for patch in patches.iter_mut() {
            //     patch.location.start = (patch.location.start as isize - displacement) as usize;
            //     patch.location.end = (patch.location.end as isize - displacement) as usize;
            //     displacement += patch.bytes.len() as isize - patch.location.len() as isize;
            // }

            for [a, b] in patches.array_windows() {
                assert!(
                    a.location.end <= b.location.start,
                    "Overlapping patches are not allowed"
                );
            }
        }
        change_sets
            .into_iter()
            .map(|(file, changes)| FileChangeSet { file, changes })
            .collect()
    }

    /// Actually write the changes to the file
    pub fn write(self) -> std::io::Result<()> {
        let mut buffer = fs::read(&self.file)?;
        for change in self.changes.into_iter().rev() {
            buffer.splice(change.location, change.bytes);
        }
        fs::write(self.file, buffer)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    use tempfile::NamedTempFile;

    #[test]
    fn test_apply_changes() {
        let tmp = NamedTempFile::new().unwrap();

        let changes = vec![
            Change {
                file: tmp.path().to_owned(),
                patch: Patch {
                    location: 7..12,
                    bytes: b"there".to_vec(),
                },
            },
            Change {
                file: tmp.path().to_owned(),
                patch: Patch {
                    location: 1..1,
                    bytes: b"??".to_vec(),
                },
            },
            Change {
                file: tmp.path().to_owned(),
                patch: Patch {
                    location: 1..4,
                    bytes: b"!!".to_vec(),
                },
            },
        ];

        fs::write(tmp.path(), b"Hello, world!").unwrap();
        assert_eq!(fs::read(tmp.path()).unwrap(), b"Hello, world!");

        {
            let grouped = FileChangeSet::group(vec![changes[0].clone()]);
            assert!(grouped.len() == 1);
            let primary = grouped[0].clone();
            assert!(primary.file == tmp.path());

            primary.write().expect("Unable to write");

            assert_eq!(fs::read(tmp.path()).unwrap(), b"Hello, there!");
        }

        fs::write(tmp.path(), b"Hello, world!").unwrap();

        {
            let grouped = FileChangeSet::group(vec![changes[0].clone(), changes[1].clone()]);
            assert!(grouped.len() == 1);
            let primary = grouped[0].clone();
            assert!(primary.file == tmp.path());

            primary.write().expect("Unable to write");

            assert_eq!(fs::read(tmp.path()).unwrap(), b"H??ello, there!");
        }

        fs::write(tmp.path(), b"Hello, world!").unwrap();

        {
            let grouped = FileChangeSet::group(changes);
            assert!(grouped.len() == 1);
            let primary = grouped[0].clone();
            assert!(primary.file == tmp.path());

            primary.write().expect("Unable to write");

            dbg!(String::from_utf8_lossy(&fs::read(tmp.path()).unwrap()));
            assert_eq!(fs::read(tmp.path()).unwrap(), b"H??!!o, there!");
        }
    }
}
