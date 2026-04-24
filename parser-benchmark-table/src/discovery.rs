use crate::model::{
    CONJURE_REPO_URL, ESSENCE_CATALOG_REPO_URL, InputGroup, ParserSelection,
    REPO_CACHE_DIR, RepoSelection,
};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

struct ScanTarget {
    repo_name: &'static str,
    repo_root: PathBuf,
    scan_root: PathBuf,
}

pub fn discover_input_groups(
    _parser_selection: ParserSelection,
    repo_selection: RepoSelection,
) -> Vec<InputGroup> {
    // Get the directories to scan based on the selected repositories, cloning or updating them as necessary
    let Some(scan_targets) = build_scan_targets(repo_selection) else {
        return Vec::new();
    };

    // For each target, scan for candidate files and group them into input groups
    let mut all_groups = Vec::new();
    for target in scan_targets {
        println!(
            "Scanning {} at {}",
            target.repo_name,
            target.scan_root.display()
        );

        let grouped = group_repo_files(target.repo_name, &target.repo_root, &target.scan_root);

        println!(
            "- {}: {} input groups",
            target.repo_name,
            grouped.len()
        );
        all_groups.extend(grouped);
    }

    all_groups.sort_by(|a, b| {
        let left = format!("{}:{}", a.repo_name, a.primary_file.display());
        let right = format!("{}:{}", b.repo_name, b.primary_file.display());
        left.cmp(&right)
    });

    all_groups
}

fn build_scan_targets(repo_selection: RepoSelection) -> Option<Vec<ScanTarget>> {
    // Ensure the cache directory exists
    let cache_root = PathBuf::from(REPO_CACHE_DIR);
    if let Err(err) = fs::create_dir_all(&cache_root) {
        println!(
            "Could not create repo cache directory at {}: {}",
            cache_root.display(),
            err
        );
        return None;
    }

    // Determine which repositories to scan based on the selection, ensuring they are checked out in the cache directory
    let mut targets = Vec::new();

    if repo_selection.conjure_oxide {
        targets.push(ScanTarget {
            repo_name: "conjure-oxide",
            repo_root: PathBuf::from("."),
            scan_root: PathBuf::from("tests-integration/tests"),
        });
    }

    if repo_selection.conjure {
        let conjure_target = cache_root.join("conjure");
        let _ = ensure_repo_checkout(CONJURE_REPO_URL, &conjure_target);
        let repo = cache_root.join("conjure");
        targets.push(ScanTarget {
            repo_name: "Conjure",
            repo_root: repo.clone(),
            scan_root: repo,
        });
    }

    if repo_selection.essence_catalog {
        let essence_catalog_target = cache_root.join("EssenceCatalog");
        let _ = ensure_repo_checkout(
            ESSENCE_CATALOG_REPO_URL,
            &essence_catalog_target,
        );
        let repo = cache_root.join("EssenceCatalog");
        targets.push(ScanTarget {
            repo_name: "EssenceCatalog",
            repo_root: repo.clone(),
            scan_root: repo,
        });
    }

    Some(targets)
}

fn ensure_repo_checkout(repo_url: &str, target_path: &Path) -> Option<PathBuf> {
    // If the target path already exists, attempt to pull the latest changes
    if target_path.exists() {
        let pull = Command::new("git")
            .arg("-C")
            .arg(target_path)
            .arg("pull")
            .arg("--ff-only")
            .status();

        match pull {
            Ok(status) if status.success() => return Some(target_path.to_path_buf()),
            _ => return None
        }
    }

    // Otherwise, clone the repository into the target path
    let clone = Command::new("git")
        .arg("clone")
        .arg(repo_url)
        .arg(target_path)
        .status();

    match clone {
        Ok(status) if status.success() => return Some(target_path.to_path_buf()),
        _ => return None
    }
}

fn group_repo_files(repo_name: &str, repo_root: &Path, scan_root: &Path) -> Vec<InputGroup> {
    let mut groups = Vec::new();
    let mut group_indexes: BTreeMap<String, usize> = BTreeMap::new();

    // Recursively scan the target directory for candidate files, grouping them as we go
    collect_and_group_files(
        scan_root,
        repo_name,
        repo_root,
        &mut groups,
        &mut group_indexes,
    );

    groups
}

fn collect_and_group_files(
    dir: &Path,
    repo_name: &str,
    repo_root: &Path,
    groups: &mut Vec<InputGroup>,
    group_indexes: &mut BTreeMap<String, usize>,
) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();

        // If directory, recursively collect files from it
        if path.is_dir() {
            collect_and_group_files(&path, repo_name, repo_root, groups, group_indexes);
            continue;
        }

        if !is_candidate_file(&path) {
            continue;
        }

        // Use the parent directory as the grouping key, so files in the same directory are grouped together
        let key = path
            .parent()
            .map(|p| p.display().to_string())
            .unwrap_or_default();

        if let Some(idx) = group_indexes.get(&key).copied() {
            // If we've already seen a file from the same group, apply this file to the existing group
            apply_file_to_group(&mut groups[idx], path);
        } else {
            // Otherwise, create a new group for this file
            let text = path.to_string_lossy();
            let group_kind = if text.ends_with(".eprime-param") {
                "eprime-param-only"
            } else if text.ends_with(".param") {
                "param-only"
            } else if text.ends_with(".eprime") {
                "eprime"
            } else {
                "essence"
            };

            groups.push(InputGroup {
                repo_name: repo_name.to_string(),
                repo_root: repo_root.to_path_buf(),
                primary_file: path,
                param_file: None,
                group_kind,
            });
            group_indexes.insert(key, groups.len() - 1);
        }
    }
}


fn apply_file_to_group(group: &mut InputGroup, path: PathBuf) {
    // Determine the role of the file based on its name and extension, and update the group accordingly
    let text = path.to_string_lossy();

    if text.ends_with(".essence") {
        if group.group_kind == "param-only" {
            let param_path = group.primary_file.clone();
            group.primary_file = path;
            group.param_file = Some(param_path);
            group.group_kind = "essence+param";
        } else {
            group.primary_file = path;
            group.group_kind = if group.param_file.is_some() {
                "essence+param"
            } else {
                "essence"
            };
        }
        return;
    }

    if text.ends_with(".param") {
        match group.group_kind {
            "essence" => {
                group.param_file = Some(path);
                group.group_kind = "essence+param";
            }
            "essence+param" => {
                group.param_file = Some(path);
            }
            "param-only" => {
                group.primary_file = path;
            }
            _ => {}
        }
        return;
    }

    if text.ends_with(".eprime") {
        if group.group_kind == "eprime-param-only" {
            let param_path = group.primary_file.clone();
            group.primary_file = path;
            group.param_file = Some(param_path);
            group.group_kind = "eprime+eprime-param";
        } else {
            group.primary_file = path;
            group.group_kind = if group.param_file.is_some() {
                "eprime+eprime-param"
            } else {
                "eprime"
            };
        }
        return;
    }

    if text.ends_with(".eprime-param") {
        match group.group_kind {
            "eprime" => {
                group.param_file = Some(path);
                group.group_kind = "eprime+eprime-param";
            }
            "eprime+eprime-param" => {
                group.param_file = Some(path);
            }
            "eprime-param-only" => {
                group.primary_file = path;
            }
            _ => {}
        }
    }
}

fn is_candidate_file(path: &Path) -> bool {
    let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };

    // Exclude files that contain 'expected' or 'generated' in their name
    if file_name.contains("expected") || file_name.contains("generated") {
        return false;
    }

    // Only consider files with relevant extensions
    file_name.ends_with(".essence")
        || file_name.ends_with(".eprime")
        || file_name.ends_with(".param")
        || file_name.ends_with(".eprime-param")
}

