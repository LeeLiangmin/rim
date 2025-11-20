use crate::core::tools::VSCODE_FAMILY;
use anyhow::Result;
use rim_common::utils;
use std::{
    env,
    path::{Path, PathBuf},
};

/// Export an example `cargo` project to the specified path.
/// Returns the path to the exported project directory.
pub fn export_demo_project(path: Option<&Path>) -> Result<PathBuf> {
    let path_to_init = if let Some(p) = path {
        p.to_path_buf()
    } else {
        env::current_dir()?
    };

    let example_sources = ExampleTemplate::load();
    // Export the example to user selected location
    let example_dir = example_sources.export(&path_to_init)?;
    info!(
        "{}",
        t!("demo_project_exported", dir = example_dir.display())
    );
    
    Ok(example_dir)
}

/// Open a project directory with `VSCode` editor or file explorer.
/// This function attempts to open the directory with VS-Code first,
/// if that didn't work, it opens the directory using file explorer.
pub fn open_project(project_dir: &Path) -> Result<()> {
    #[cfg(target_os = "windows")]
    let file_explorer = "explorer.exe";
    #[cfg(target_os = "linux")]
    let file_explorer = "xdg-open";
    #[cfg(target_os = "macos")]
    let file_explorer = "open";

    let program = VSCODE_FAMILY
        .iter()
        .find_map(|p| utils::cmd_exist(p).then_some(p.as_str()))
        .unwrap_or(file_explorer);
    // Try to open the project, but don't do anything if it fails cuz it's not critical.
    _ = run!(program, project_dir);
    Ok(())
}

/// Export an example `cargo` project, then optionally open it with `VSCode` editor or `file explorer`.
/// 
/// # Arguments
/// * `path` - Optional path to export the project to. If None, uses current directory.
/// * `open_editor` - Whether to open the project with an editor after export.
pub fn try_it(path: Option<&Path>, open_editor: bool) -> Result<PathBuf> {
    let example_dir = export_demo_project(path)?;
    
    if open_editor {
        open_project(&example_dir)?;
    }
    
    Ok(example_dir)
}

struct ExampleTemplate<'a> {
    src_main: &'a str,
    cargo_toml: &'a str,
    vscode_config: &'a str,
}

impl ExampleTemplate<'_> {
    fn load() -> Self {
        Self {
            src_main: include_str!("../../resources/example/src/main.rs"),
            cargo_toml: include_str!("../../resources/example/Cargo.toml"),
            vscode_config: include_str!("../../resources/example/.vscode/launch.json"),
        }
    }

    fn export(&self, dest: &Path) -> Result<PathBuf> {
        let root = dest.join("example_project");
        let src_dir = root.join("src");
        let vscode_dir = root.join(".vscode");
        utils::ensure_dir(&src_dir)?;
        utils::ensure_dir(&vscode_dir)?;

        let main_fp = src_dir.join("main.rs");
        let cargo_toml_fp = root.join("Cargo.toml");
        let vscode_config_fp = vscode_dir.join("launch.json");

        // write source files
        utils::write_file(main_fp, self.src_main, false)?;
        utils::write_file(cargo_toml_fp, self.cargo_toml, false)?;
        utils::write_file(vscode_config_fp, self.vscode_config, false)?;

        Ok(root)
    }
}
