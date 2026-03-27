use std::path::PathBuf;

use anyhow::Ok;
use regex::Regex;
use xshell::{Shell, cmd};

use crate::{flags, project_root};

impl flags::ExportSirilCommands {
    pub(crate) fn run(self, sh: &Shell) -> anyhow::Result<()> {
        let root = project_root();
        let doc_dir = root.join("xtask/siril-doc");
        let generated_dir = root.join("xtask/generated/commands");

        if self.clean {
            cmd!(&sh, "rm -rf {doc_dir} {generated_dir}").run()?;
        }

        // Make the directories if needed
        cmd!(&sh, "mkdir -p {doc_dir} {generated_dir}").run()?;
        let siril_doc = &doc_dir.join("siril-doc-1.4");
        if !sh.path_exists(siril_doc) {
            sh.change_dir(doc_dir);
            cmd!(&sh, "curl https://gitlab.com/free-astro/siril-doc/-/archive/1.4/siril-doc-1.4.zip -o siril-doc-1.4.zip").run()?;
            cmd!(&sh, "unzip siril-doc-1.4.zip -d .").run()?;
        }

        let commands_rst = &siril_doc.join("doc/Commands.rst");
        let commands = parse_commands(sh, commands_rst)?;
        println!("{:?}", commands[0]);

        println!("{:?}", commands[0].to_builder_string());

        let scriptable: Vec<&CommandInfo> = commands.iter().filter(|c| c.scriptable).collect();

        for command in &scriptable {
            let file_name = format!("{}.rs", command.name.to_lowercase());
            let out_file = generated_dir.join(&file_name);
            sh.write_file(out_file, command.to_builder_string())?;
        }

        let mod_contents = scriptable
            .iter()
            .map(|c| {
                let mod_name = c.name.to_lowercase();
                let struct_name = to_pascal_case(&c.name);
                format!("pub mod {mod_name};\npub use {mod_name}::{struct_name};")
            })
            .collect::<Vec<_>>()
            .join("\n");
        sh.write_file(generated_dir.join("mod.rs"), mod_contents)?;

        // cmd!(&sh, "cargo --version").run()?;
        // cmd!(&sh, "ls -ls {root}").run()?;

        Ok(())
    }
}

// for every implementation in siril-sys build it's generated mod.rs block
// for every implementation in siril-sys merge it's updated header from the generated folder
// report on total generated, total implementated, remaining to be implemented
impl flags::MergeSirilCommands {
    pub(crate) fn run(self, sh: &Shell) -> anyhow::Result<()> {
        let root = project_root();
        let dest_dir = root.join("crates/siril-sys/src/commands");
        let mod_file = dest_dir.join("mod.rs");

        let struct_re = Regex::new(r"pub struct (\w+)").unwrap();

        // Collect (module_name, struct_name) for every command file
        let mut modules: Vec<(String, String)> = sh
            .read_dir(&dest_dir)?
            .into_iter()
            .filter_map(|path| {
                let name = path.file_name()?.to_str()?.to_string();
                if name == "mod.rs" || !name.ends_with(".rs") {
                    return None;
                }
                let mod_name = name.strip_suffix(".rs")?.to_string();
                let contents = sh.read_file(&path).ok()?;
                let struct_name = struct_re
                    .captures(&contents)
                    .and_then(|caps| caps.get(1))
                    .map(|m| m.as_str().to_string())?;
                Some((mod_name, struct_name))
            })
            .collect();

        modules.sort_by(|a, b| a.0.cmp(&b.0));

        let generated_block = modules
            .iter()
            .map(|(mod_name, struct_name)| {
                format!("pub mod {mod_name};\npub use {mod_name}::{struct_name};")
            })
            .collect::<Vec<_>>()
            .join("\n");

        let start_marker = "// @generated start by xtask merge-siril-commands";
        let end_marker = "// @generated end by xtask merge-siril-commands";

        let mod_contents = sh.read_file(&mod_file)?;
        let start = mod_contents
            .find(start_marker)
            .ok_or_else(|| anyhow::anyhow!("start marker not found in mod.rs"))?;
        let end = mod_contents
            .find(end_marker)
            .ok_or_else(|| anyhow::anyhow!("end marker not found in mod.rs"))?;

        let new_contents = format!(
            "{}{}\n{}\n{}{}",
            &mod_contents[..start],
            start_marker,
            generated_block,
            end_marker,
            &mod_contents[end + end_marker.len()..],
        );

        sh.write_file(&mod_file, new_contents)?;

        println!("\nUpdated {:?} with {} modules", mod_file, modules.len());

        // Update doc comments in each implementation file from its generated counterpart.
        // Only the /// block immediately before #[derive(Builder)] is replaced.
        let doc_re = Regex::new(r"(?m)((?:^///[^\n]*\n)*)#\[derive\(Builder\)\]").unwrap();
        let generated_dir = root.join("xtask/generated/commands");

        let mut updated = 0usize;
        let mut skipped = 0usize;

        for (mod_name, _) in &modules {
            let generated_file = generated_dir.join(format!("{mod_name}.rs"));
            if !generated_file.exists() {
                skipped += 1;
                continue;
            }

            let generated = sh.read_file(&generated_file)?;
            let new_docs = match doc_re.captures(&generated) {
                Some(caps) => caps[1].to_string(),
                None => {
                    skipped += 1;
                    continue;
                }
            };

            let impl_file = dest_dir.join(format!("{mod_name}.rs"));
            let impl_contents = sh.read_file(&impl_file)?;

            let replaced = doc_re.replace(&impl_contents, format!("{new_docs}#[derive(Builder)]"));
            if replaced != impl_contents {
                print!("Updatings docs on: {:?}", &impl_file);
                sh.write_file(&impl_file, replaced.as_ref())?;
                updated += 1;
            } else {
                skipped += 1;
            }
        }

        println!("\nDocs updated: {updated}, unchanged/skipped: {skipped}");

        // Report: implemented vs generated vs missing
        let implemented: std::collections::HashSet<String> =
            modules.iter().map(|(name, _)| name.clone()).collect();

        let generated: std::collections::HashSet<String> = sh
            .read_dir(&generated_dir)?
            .into_iter()
            .filter_map(|path| {
                let name = path.file_name()?.to_str()?.to_string();
                if name == "mod.rs" || !name.ends_with(".rs") {
                    return None;
                }
                name.strip_suffix(".rs").map(|s| s.to_string())
            })
            .collect();

        let mut missing: Vec<String> = generated.difference(&implemented).cloned().collect();
        missing.sort();

        println!(
            "\nReport: {} implemented, {} generated, {} missing",
            implemented.len(),
            generated.len(),
            missing.len(),
        );

        if self.show_missing {
            for name in &missing {
                println!("  missing: {name}");
            }
        }

        Ok(())
    }
}

#[derive(Debug)]
struct CommandInfo {
    name: String,
    scriptable: bool,
    documentation: String,
}

impl CommandInfo {
    fn to_builder_string(&self) -> String {
        let struct_name = to_pascal_case(&self.name);

        let doc_lines: String = self
            .documentation
            .lines()
            .map(|line| {
                if line.is_empty() {
                    "///\n".to_string()
                } else {
                    format!("/// {line}\n")
                }
            })
            .collect();

        format!(
            r#"use bon::Builder;

use crate::commands::{{Argument, Command}};

{doc_lines}#[derive(Builder)]
pub struct {struct_name} {{}}

impl Command for {struct_name} {{
    fn name() -> &'static str {{
        "{name}"
    }}

    fn args(&self) -> Vec<Argument> {{
        vec![]
    }}
}}
"#,
            doc_lines = doc_lines,
            struct_name = struct_name,
            name = self.name,
        )
    }
}

fn to_pascal_case(s: &str) -> String {
    s.split(['-', '_', '.'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect()
}

fn parse_commands(sh: &Shell, commands_rst: &PathBuf) -> anyhow::Result<Vec<CommandInfo>> {
    let contents = sh.read_file(commands_rst)?;
    let re1 = Regex::new(r"([^\n]+)\n   :scriptable: ([01])").unwrap();
    let re2 = Regex::new(r"\.\. include:: ([^\n]+)").unwrap();

    let mut results: Vec<CommandInfo> = vec![];
    contents.split(".. command:: ").skip(1).for_each(|block| {
        // println!("----{}", block);
        let caps1 = re1.captures(block).unwrap();

        let includes: Vec<PathBuf> = re2
            .captures_iter(block)
            .map(|caps| commands_rst.parent().unwrap().join(caps[1].trim()))
            .collect();

        let docs = combine_documentation(sh, includes).unwrap();

        results.push(CommandInfo {
            name: caps1[1].to_string(),
            scriptable: &caps1[2] == "1",
            documentation: docs,
        });
    });

    Ok(results)
}

fn combine_documentation(sh: &Shell, includes: Vec<PathBuf>) -> anyhow::Result<String> {
    let mut documentation = String::new();
    let re_pipe = Regex::new(r"(?m)^\| ").unwrap();
    let re_code_block = Regex::new(r"(?s)\.\. code-block:: (\w+)\n\n((?:    [^\n]*\n?)+)").unwrap();

    for doc_path in &includes {
        if !doc_path.exists() {
            eprintln!(
                "Warning: Main documentation file not found: {}",
                doc_path.display()
            );
        } else {
            let content = sh.read_file(doc_path)?;
            let include_doc = re_pipe.replace_all(&content, "");
            let include_doc = re_code_block.replace_all(&include_doc, |caps: &regex::Captures| {
                let lang = &caps[1];
                let body: String = caps[2]
                    .lines()
                    .map(|line| line.strip_prefix("    ").unwrap_or(line))
                    .collect::<Vec<_>>()
                    .join("\n");
                format!("```{lang}\n{body}\n```")
            });
            let include_doc = include_doc.trim().to_string();
            documentation.push_str(&include_doc);
            documentation.push_str("\n\n");
        }
    }

    Ok(documentation)
}
