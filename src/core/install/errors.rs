use log::{error, warn};

/// 收集安装过程中的错误信息
#[derive(Debug, Default)]
pub(crate) struct InstallationErrors {
    /// 工具安装错误: (工具名, 错误信息)
    tool_errors: Vec<(String, String)>,
    /// Rust工具链安装错误
    rust_error: Option<String>,
    /// 其他步骤的错误: (步骤名, 错误信息)
    step_errors: Vec<(String, String)>,
}

impl InstallationErrors {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn add_tool_error(&mut self, tool_name: String, error: anyhow::Error) {
        let error_msg = format!("{error:?}");
        error!("{}", tl!("install_tool_failed", name = tool_name, err = error_msg));
        self.tool_errors.push((tool_name, error_msg));
    }

    pub(crate) fn add_rust_error(&mut self, error: anyhow::Error) {
        let error_msg = format!("{error:?}");
        error!("{}", tl!("install_toolchain_failed", err = error_msg));
        self.rust_error = Some(error_msg);
    }

    pub(crate) fn add_step_error(&mut self, step_name: String, error: anyhow::Error) {
        let error_msg = format!("{error:?}");
        warn!("{}", tl!("step_failed", step = step_name, err = error_msg));
        self.step_errors.push((step_name, error_msg));
    }

    pub(crate) fn has_errors(&self) -> bool {
        !self.tool_errors.is_empty() || self.rust_error.is_some() || !self.step_errors.is_empty()
    }

    pub(crate) fn report(&self) {
        if !self.has_errors() {
            return;
        }

        error!("{}", tl!("install_error_summary"));
        
        if !self.tool_errors.is_empty() {
            error!("{}", tl!("install_errors_tools", count = self.tool_errors.len()));
            for (name, err) in &self.tool_errors {
                error!("  - {}: {}", name, err);
            }
        }

        if let Some(ref err) = self.rust_error {
            error!("{}", tl!("install_errors_rust", err = err));
        }

        if !self.step_errors.is_empty() {
            error!("{}", tl!("install_errors_other_steps", count = self.step_errors.len()));
            for (step, err) in &self.step_errors {
                error!("  - {}: {}", step, err);
            }
        }
    }
}
