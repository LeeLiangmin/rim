use super::errors::InstallationErrors;
use super::InstallConfiguration;
use crate::core::components::ToolchainComponent;
use crate::core::directories::RimDir;
use crate::core::os::add_to_path;
use crate::core::rustup::ToolchainInstaller;
use anyhow::Result;
use log::info;
use rim_common::utils::ProgressHandler;

impl<'a, T: ProgressHandler + Clone + 'static> InstallConfiguration<'a, T> {
    /// Install Rust toolchain with a list of components
    pub(crate) async fn install_rust(
        &mut self,
        components: &[ToolchainComponent],
        errors: &mut InstallationErrors,
    ) -> Result<()> {
        info!("{}", tl!("install_toolchain"));

        let manifest = self.manifest;

        match ToolchainInstaller::init(&*self)
            .insecure(self.insecure)
            .rustup_dist_server(Some(self.rustup_dist_server().clone()))
            .install(self, components)
            .await
        {
            Ok(()) => {
                if let Err(e) = add_to_path(&*self, self.cargo_bin()) {
                    errors.add_step_error("添加到PATH".to_string(), e);
                } else {
                    self.toolchain_is_installed = true;
                }

                self.install_record
                    .add_rust_record(&manifest.toolchain.channel, components);
                self.install_record
                    .clone_toolkit_meta_from_manifest(manifest);
                if let Err(e) = self.install_record.write() {
                    errors.add_step_error("保存安装记录".to_string(), e);
                }

                self.inc_progress(30)?;
            }
            Err(e) => {
                errors.add_rust_error(e);
                self.inc_progress(30)?;
            }
        }

        Ok(())
    }

    /// Add toolchain components separately, typically used in `component add`.
    pub async fn install_toolchain_components(
        &mut self,
        components: &[ToolchainComponent],
    ) -> Result<()> {
        ToolchainInstaller::init(&*self)
            .insecure(self.insecure)
            .rustup_dist_server(Some(self.rustup_dist_server().clone()))
            .add_components(self, components)
            .await?;

        self.install_record
            .add_rust_record(&self.manifest.toolchain.channel, components);
        self.install_record.write()?;
        Ok(())
    }

    /// Update the Rust toolchain.
    pub(crate) async fn update_toolchain(
        &mut self,
        components: &[ToolchainComponent],
    ) -> Result<()> {
        info!("{}", tl!("update_toolchain"));

        ToolchainInstaller::init(&*self)
            .insecure(self.insecure)
            .update(self, components)
            .await?;

        let record = &mut self.install_record;
        record.add_rust_record(&self.manifest.toolchain.channel, components);
        record.clone_toolkit_meta_from_manifest(self.manifest);
        record.write()?;

        self.inc_progress(60)?;
        Ok(())
    }
}
