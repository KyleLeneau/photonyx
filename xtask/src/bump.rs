use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use toml_edit::{DocumentMut, value};
use xshell::Shell;

use crate::{flags, project_root};

impl flags::Bump {
    pub(crate) fn run(&self, _sh: &Shell) -> Result<()> {
        let new_version = self.version.trim();
        if new_version.is_empty() {
            bail!("version must not be empty");
        }

        let root = project_root();
        let root_manifest_path = root.join("Cargo.toml");
        let mut root_doc = read_document(&root_manifest_path)?;

        let members = workspace_members(&root_doc, &root_manifest_path)?;

        let mut bumped = Vec::new();
        for member_dir in expand_members(&root, &members)? {
            let manifest_path = member_dir.join("Cargo.toml");
            if !manifest_path.exists() {
                continue;
            }

            let mut doc = read_document(&manifest_path)?;
            let Some(package) = doc.get_mut("package").and_then(|p| p.as_table_like_mut()) else {
                continue;
            };

            // Crates that opt out of publishing (e.g. xtask itself) are build
            // tooling, not part of the versioned release train.
            if package.get("publish").and_then(|v| v.as_bool()) == Some(false) {
                continue;
            }

            let Some(name) = package
                .get("name")
                .and_then(|v| v.as_str())
                .map(str::to_owned)
            else {
                continue;
            };

            if package.get("version").is_none() {
                continue;
            }
            package.insert("version", value(new_version));

            fs::write(&manifest_path, doc.to_string())
                .with_context(|| format!("writing {}", manifest_path.display()))?;
            bumped.push(name);
        }

        if bumped.is_empty() {
            bail!("no local workspace crates found to bump");
        }

        let mut bumped_deps = Vec::new();
        if let Some(deps) = root_doc
            .get_mut("workspace")
            .and_then(|w| w.get_mut("dependencies"))
            .and_then(|d| d.as_table_like_mut())
        {
            for (dep_name, item) in deps.iter_mut() {
                let Some(dep_table) = item.as_table_like_mut() else {
                    continue;
                };
                // Only local path dependencies track the workspace release version.
                if dep_table.get("path").is_none() {
                    continue;
                }
                if dep_table.get("version").is_none() {
                    continue;
                }
                dep_table.insert("version", value(new_version));
                bumped_deps.push(dep_name.to_string());
            }
        }

        fs::write(&root_manifest_path, root_doc.to_string())
            .with_context(|| format!("writing {}", root_manifest_path.display()))?;

        bumped.sort();
        println!("Bumped {} crate(s) to {new_version}:", bumped.len());
        for name in &bumped {
            println!("  - {name}");
        }

        bumped_deps.sort();
        println!(
            "Updated {} workspace.dependencies entr(y/ies) in {}:",
            bumped_deps.len(),
            root_manifest_path.display()
        );
        for name in &bumped_deps {
            println!("  - {name}");
        }

        Ok(())
    }
}

fn read_document(path: &Path) -> Result<DocumentMut> {
    let contents =
        fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    contents
        .parse::<DocumentMut>()
        .with_context(|| format!("parsing {}", path.display()))
}

/// Reads `[workspace].members` from the root manifest, e.g. `["xtask/", "lib/*", "crates/*"]`.
fn workspace_members(doc: &DocumentMut, manifest_path: &Path) -> Result<Vec<String>> {
    let members = doc
        .get("workspace")
        .and_then(|w| w.get("members"))
        .and_then(|m| m.as_array())
        .with_context(|| {
            format!(
                "no [workspace].members found in {}",
                manifest_path.display()
            )
        })?;

    Ok(members
        .iter()
        .filter_map(|m| m.as_str())
        .map(str::to_owned)
        .collect())
}

/// Expands workspace member patterns (plain dirs or a single trailing `*` glob)
/// into concrete directories that exist on disk.
fn expand_members(root: &Path, members: &[String]) -> Result<Vec<PathBuf>> {
    let mut dirs = Vec::new();

    for member in members {
        let member = member.trim_end_matches('/');

        if let Some(parent) = member.strip_suffix("/*") {
            let parent_dir = root.join(parent);
            if !parent_dir.is_dir() {
                continue;
            }
            for entry in fs::read_dir(&parent_dir)
                .with_context(|| format!("reading directory {}", parent_dir.display()))?
            {
                let entry = entry?;
                if entry.file_type()?.is_dir() {
                    dirs.push(entry.path());
                }
            }
        } else if member == "*" {
            bail!("bare `*` workspace member glob is not supported");
        } else {
            dirs.push(root.join(member));
        }
    }

    dirs.sort();
    Ok(dirs)
}
