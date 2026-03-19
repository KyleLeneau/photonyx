use std::path::PathBuf;

use anyhow::Ok;
use regex::Regex;
use xshell::{Shell, cmd};

use crate::{flags, project_root};

impl flags::ExportSirilCommands {
    pub(crate) fn run(self, sh: &Shell) -> anyhow::Result<()> {
        let root = project_root();
        let doc_dir = root.join("xtask/siril-doc");
        let generated_dir = root.join("xtask/generated");

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

        let out_file = generated_dir.join("commands.rs");
        let out_contents = commands
            .iter()
            .filter(|c| c.scriptable)
            .map(|c| c.to_builder_string())
            .collect::<Vec<_>>()
            .join("\n\n");
        sh.write_file(out_file, out_contents)?;

        // cmd!(&sh, "cargo --version").run()?;
        // cmd!(&sh, "ls -ls {root}").run()?;

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
            r#"{doc_lines}#[derive(Builder)]
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
    let _blocks = contents.split(".. command:: ").skip(1).for_each(|block| {
        // println!("----{}", block);
        let caps1 = re1.captures(block).unwrap();

        let includes: Vec<PathBuf> = re2
            .captures_iter(&block)
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
