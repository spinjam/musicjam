use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use super::Track;

#[derive(Debug, Clone, PartialEq)]
pub struct FolderNode {
    pub path: PathBuf,
    pub name: String,
    pub children: Vec<FolderNode>,
}

/// The folders that hold music, below `root`. Folders without any audio file
/// underneath (artwork, scans) are left out.
pub fn build(root: &Path, tracks: &[Track]) -> FolderNode {
    #[derive(Default)]
    struct Dir(BTreeMap<String, Dir>);
    let mut top = Dir::default();
    for t in tracks {
        let Ok(rel) = t.path.strip_prefix(root) else { continue };
        let mut dir = &mut top;
        if let Some(parent) = rel.parent() {
            for c in parent.iter() {
                dir = dir.0.entry(c.to_string_lossy().into_owned()).or_default();
            }
        }
    }
    fn convert(path: PathBuf, name: String, dir: Dir) -> FolderNode {
        let mut children: Vec<FolderNode> =
            dir.0.into_iter().map(|(n, d)| convert(path.join(&n), n, d)).collect();
        children.sort_by_key(|c| c.name.to_lowercase());
        FolderNode { path, name, children }
    }
    let name = root.file_name().map_or_else(|| root.to_string_lossy().into_owned(), |n| n.to_string_lossy().into_owned());
    convert(root.to_path_buf(), name, top)
}

impl FolderNode {
    /// The rows a tree view shows: this node, then the children of every
    /// expanded node, each with its depth.
    pub fn visible_rows(&self, expanded: &HashSet<PathBuf>) -> Vec<(usize, &FolderNode)> {
        let mut rows = Vec::new();
        fn walk<'a>(n: &'a FolderNode, depth: usize, exp: &HashSet<PathBuf>, rows: &mut Vec<(usize, &'a FolderNode)>) {
            rows.push((depth, n));
            if exp.contains(&n.path) {
                for c in &n.children {
                    walk(c, depth + 1, exp, rows);
                }
            }
        }
        walk(self, 0, expanded, &mut rows);
        rows
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(path: &str) -> Track {
        Track { id: 0, path: path.into(), title: String::new(), artist: String::new(), album: String::new(), track_no: None, duration_secs: 0.0 }
    }

    #[test]
    fn builds_sorted_tree_and_rows() {
        let tracks = [t("/M/blur/B/1.mp3"), t("/M/Arctic/A/1.mp3"), t("/M/blur/A/2.mp3"), t("/M/loose.mp3")];
        let root = build(Path::new("/M"), &tracks);
        let names: Vec<&str> = root.children.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["Arctic", "blur"]);
        assert_eq!(root.children[1].children.len(), 2);

        let mut exp = HashSet::new();
        assert_eq!(root.visible_rows(&exp).len(), 1);
        exp.insert(PathBuf::from("/M"));
        exp.insert(PathBuf::from("/M/blur"));
        let rows: Vec<(usize, &str)> = root.visible_rows(&exp).iter().map(|(d, n)| (*d, n.name.as_str())).collect();
        assert_eq!(rows, [(0, "M"), (1, "Arctic"), (1, "blur"), (2, "A"), (2, "B")]);
    }
}
