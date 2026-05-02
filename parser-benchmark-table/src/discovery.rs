use crate::model::{
    CONJURE_REPO_URL, ESSENCE_CATALOG_REPO_URL, InputGroup, ParserSelection, REPO_CACHE_DIR,
    RepoSelection,
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

#[derive(Default)]
struct GroupBucket {
    essence_models: Vec<PathBuf>,
    essence_params: Vec<PathBuf>,
    eprime_models: Vec<PathBuf>,
    eprime_params: Vec<PathBuf>,
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

        println!("- {}: {} input groups", target.repo_name, grouped.len());
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
        let _ = ensure_repo_checkout(ESSENCE_CATALOG_REPO_URL, &essence_catalog_target);
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
            _ => return None,
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
        _ => return None,
    }
}

fn group_repo_files(repo_name: &str, repo_root: &Path, scan_root: &Path) -> Vec<InputGroup> {
    let mut buckets: BTreeMap<String, GroupBucket> = BTreeMap::new();
    collect_and_group_files(scan_root, repo_name, &mut buckets);

    let mut groups = Vec::new();
    // For each bucket of related files, emit one or more input groups based on the presence of model and param files
    for bucket in buckets.values_mut() {
        bucket.essence_models.sort();
        bucket.essence_params.sort();
        bucket.eprime_models.sort();
        bucket.eprime_params.sort();

        emit_groups(repo_name, repo_root, bucket, &mut groups);
    }

    groups
}

fn collect_and_group_files(
    dir: &Path,
    repo_name: &str,
    buckets: &mut BTreeMap<String, GroupBucket>,
) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();

        // If directory, recursively collect files from it
        if path.is_dir() {
            collect_and_group_files(&path, repo_name, buckets);
            continue;
        }

        if !is_candidate_file(&path) {
            continue;
        }

        // Get the grouping key for this file and add it to the appropriate bucket based on its extension
        let key = grouping_key(repo_name, &path);
        let bucket = buckets.entry(key).or_default();

        let text = path.to_string_lossy();
        if text.ends_with(".eprime-param") {
            bucket.eprime_params.push(path);
        } else if text.ends_with(".param") {
            bucket.essence_params.push(path);
        } else if text.ends_with(".eprime") {
            bucket.eprime_models.push(path);
        } else if text.ends_with(".essence") {
            bucket.essence_models.push(path);
        }
    }
}

fn emit_groups(
    repo_name: &str,
    repo_root: &Path,
    bucket: &GroupBucket,
    groups: &mut Vec<InputGroup>,
) {
    // Get the input file (should only be one essence or eprime model file)
    let model = bucket
        .essence_models
        .first()
        .cloned()
        .or_else(|| bucket.eprime_models.first().cloned());

    if let Some(model_path) = model {
        if bucket.essence_params.is_empty() {
            // Model file with no params
            groups.push(InputGroup {
                repo_name: repo_name.to_string(),
                repo_root: repo_root.to_path_buf(),
                primary_file: model_path,
                param_file: None,
                group_kind: "essence",
            });
        } else {
            // Add a group for each param file paired with the model file
            for param in &bucket.essence_params {
                groups.push(InputGroup {
                    repo_name: repo_name.to_string(),
                    repo_root: repo_root.to_path_buf(),
                    primary_file: model_path.clone(),
                    param_file: Some(param.clone()),
                    group_kind: "essence+param",
                });
            }
        }
    } else {
        // Only param files with no model file - add each as its own group
        for param in &bucket.essence_params {
            groups.push(InputGroup {
                repo_name: repo_name.to_string(),
                repo_root: repo_root.to_path_buf(),
                primary_file: param.clone(),
                param_file: None,
                group_kind: "param-only",
            });
        }
    }
}

fn grouping_key(repo_name: &str, path: &Path) -> String {
    // Group files by their parent directory by default
    let parent = path.parent();
    let parent_name = parent
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or_default();

    // EssenceCatalog commonly uses a test file in a problem directory
    // and params under a sibling params/ directory. Normalize params/ files
    // to the parent problem directory key so each param pairs with that test file.
    if repo_name == "EssenceCatalog" && parent_name == "params" {
        return parent
            .and_then(|p| p.parent())
            .map(|p| p.display().to_string())
            .unwrap_or_default();
    }

    parent.map(|p| p.display().to_string()).unwrap_or_default()
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
