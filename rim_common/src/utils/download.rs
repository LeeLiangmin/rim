use std::cmp::min;
use std::path::Path;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use reqwest::{header, Client};
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use url::Url;

use crate::types::Proxy as CrateProxy;
use crate::utils::{ProgressHandler, ProgressKind};
use crate::{build_config, setter};

const COPY_BUFFER_SIZE: usize = 1024 * 1024;
/// Throttle progress callback updates to reduce UI/log overhead.
const DOWNLOAD_PROGRESS_THRESHOLD: u64 = 1024 * 1024;
/// Maximum number of download retry attempts
const MAX_RETRY_ATTEMPTS: u32 = 3;
/// Base delay for exponential backoff (in milliseconds)
const RETRY_BASE_DELAY_MS: u64 = 1000;

fn default_proxy() -> reqwest::Proxy {
    reqwest::Proxy::custom(|url| env_proxy::for_url(url).to_url())
        .no_proxy(reqwest::NoProxy::from_env())
}

pub struct DownloadOpt {
    /// The verbose name of the file to download.
    pub name: String,
    /// Download progress handler, aka a progress bar.
    pub progress_handler: Box<dyn ProgressHandler>,
    /// Option to skip SSL certificate verification when downloading.
    pub insecure: bool,
    /// Proxy configurations for download.
    pub proxy: Option<CrateProxy>,
    /// Whether or not to resuming previous download.
    resume: bool,
    /// Cached HTTP client for connection reuse.
    client: Option<Client>,
}

impl DownloadOpt {
    pub fn new<S: ToString>(name: S, handler: Box<dyn ProgressHandler>) -> Self {
        Self {
            name: name.to_string(),
            progress_handler: handler,
            insecure: false,
            proxy: None,
            resume: true,
            client: None,
        }
    }

    setter!(with_proxy(self.proxy, Option<CrateProxy>));
    setter!(insecure(self.insecure, bool));
    setter!(resume(self.resume, bool));
    setter!(with_client(self.client, Option<Client>));

    /// Build and return a client for download.
    ///
    /// If a cached client is available, it will be returned directly;
    /// otherwise a new one is built from the current configuration.
    fn client(&self) -> Result<Client> {
        if let Some(ref client) = self.client {
            return Ok(client.clone());
        }
        let user_agent = format!(
            "{}/{}",
            &build_config().identifier,
            env!("CARGO_PKG_VERSION")
        );
        let proxy = if let Some(p) = &self.proxy {
            p.try_into()?
        } else {
            default_proxy()
        };
        let client = Client::builder()
            .user_agent(user_agent)
            .connect_timeout(Duration::from_secs(30))
            .danger_accept_invalid_certs(self.insecure)
            .proxy(proxy)
            .build()?;
        Ok(client)
    }

    /// Build and return a reusable HTTP client based on the current configuration.
    pub fn build_client(&self) -> Result<Client> {
        self.client()
    }

    /// Consume self, and retrieve text response by sending request to a given url.
    ///
    /// If the `url` is a local file, this will use [`read_to_string`](fs::read_to_string) to
    /// get the text instead.
    pub async fn read(self, url: &Url) -> Result<String> {
        if url.scheme() == "file" {
            let file_url = url
                .to_file_path()
                .map_err(|_| anyhow!("file url does not exist"))?;
            return fs::read_to_string(&file_url).await.with_context(|| {
                format!(
                    "unable to read {} located in {}",
                    self.name,
                    file_url.display()
                )
            });
        }

        if self.insecure {
            warn!("{}", crate::tl!("insecure_download"));
        }

        let resp = self
            .client()?
            .get(url.as_ref())
            .send()
            .await
            .with_context(|| format!("failed to receive server response from '{url}'"))?;
        if resp.status().is_success() {
            Ok(resp.text().await?)
        } else {
            bail!(
                "unable to get text content of url '{url}': server responded with error {}",
                resp.status()
            );
        }
    }

    async fn copy(mut self, src: &Path, dest: &Path) -> Result<()> {
        let mut src_file = fs::File::open(src).await?;
        let mut dst_file = fs::File::create(dest).await?;

        let mut buf = vec![0u8; COPY_BUFFER_SIZE];
        let total_size = src_file.metadata().await?.len();
        self.progress_handler.start(
            t!("downloading", file = &self.name).into(),
            ProgressKind::Bytes(total_size),
        )?;

        let mut copied: u64 = 0;
        let mut last_reported: u64 = 0;
        loop {
            let bytes = src_file.read(&mut buf).await?;
            if bytes == 0 {
                break; // EOF
            }
            dst_file.write_all(&buf[..bytes]).await?;
            copied += bytes as u64;
            if copied.saturating_sub(last_reported) >= DOWNLOAD_PROGRESS_THRESHOLD {
                self.progress_handler.update(Some(copied))?;
                last_reported = copied;
            }
        }

        self.progress_handler.update(Some(total_size))?;
        dst_file.flush().await?;
        self.progress_handler
            .finish(t!("download_success", file = &self.name).into())?;
        Ok(())
    }

    /// Consume self, and download from given `Url` to `Path`.
    pub async fn download(mut self, url: &Url, path: &Path) -> Result<()> {
        // File URL handling stays the same (no retry needed for local files)
        if url.scheme() == "file" {
            let src = url
                .to_file_path()
                .map_err(|_| anyhow!("unable to convert to file path for url '{url}'"))?;
            return self.copy(&src, path).await;
        }

        if self.insecure {
            warn!("{}", crate::tl!("insecure_download"));
        }

        let mut last_error = None;

        for attempt in 0..MAX_RETRY_ATTEMPTS {
            if attempt > 0 {
                let delay = Duration::from_millis(RETRY_BASE_DELAY_MS * 2u64.pow(attempt - 1));
                info!("{}", crate::tl!("download_retry", file = &self.name, attempt = attempt, max = MAX_RETRY_ATTEMPTS, delay_secs = delay.as_secs()));
                tokio::time::sleep(delay).await;
                // Enable resume for retry attempts
                self.resume = true;
            }

            match self.try_download(url, path).await {
                Ok(()) => return Ok(()),
                Err(e) => {
                    warn!("{}", crate::tl!("download_attempt_failed", file = &self.name, attempt = attempt + 1, err = format!("{e:#}")));
                    last_error = Some(e);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow!("download failed after {MAX_RETRY_ATTEMPTS} attempts")))
    }

    /// Attempt a single download from given `Url` to `Path`.
    async fn try_download(&mut self, url: &Url, path: &Path) -> Result<()> {
        let helper = DownloadHelper::new(&self.client()?, url, path, self.resume).await?;
        let (mut resp, mut file, downloaded_bytes) =
            (helper.response, helper.file, helper.downloaded_bytes);

        let content_length = resp.content_length();

        // 如果 resume 模式下服务器返回剩余 0 字节，说明文件已完整下载
        if downloaded_bytes > 0 && content_length == Some(0) {
            // 文件已完整存在，跳过下载
            info!("'{}' already downloaded, skipping.", &self.name);
            return Ok(());
        }

        // When resuming, content_length is the *remaining* bytes, not the full file size.
        // We need the full size for the progress bar, and track only session bytes for position.
        let total_size = content_length.map(|cl| cl + downloaded_bytes);

        if let Some(size) = total_size {
            self.progress_handler.start(
                t!("downloading", file = &self.name).into(),
                ProgressKind::Bytes(size),
            )?;
        } else {
            // Server didn't provide Content-Length, use spinner mode
            info!("Content-Length not available for '{}', using spinner progress", &self.name);
            self.progress_handler.start(
                t!("downloading", file = &self.name).into(),
                ProgressKind::Spinner {
                    auto_tick_duration: Some(Duration::from_millis(100)),
                },
            )?;
        }

        // Track bytes downloaded in this session separately for accurate progress
        let mut session_bytes: u64 = 0;
        let mut last_reported: u64 = downloaded_bytes;
        while let Some(chunk) = resp.chunk().await? {
            file.write_all(&chunk).await?;
            session_bytes += chunk.len() as u64;
            if let Some(size) = total_size {
                let current = min(downloaded_bytes + session_bytes, size);
                if current.saturating_sub(last_reported) >= DOWNLOAD_PROGRESS_THRESHOLD {
                    self.progress_handler.update(Some(current))?;
                    last_reported = current;
                }
            }
        }

        if let Some(size) = total_size {
            self.progress_handler.update(Some(size))?;
        }

        self.progress_handler
            .finish(t!("download_success", file = &self.name).into())?;
        Ok(())
    }

    /// Consume self, and download from given `Url` to `Path`.
    ///
    /// Note: This will block the current thread until the download is finished.
    pub fn blocking_download(self, url: &Url, path: &Path) -> Result<()> {
        crate::blocking!(self.download(url, path))
    }
}

struct DownloadHelper {
    response: reqwest::Response,
    file: fs::File,
    /// The length of bytes that already got downloaded.
    downloaded_bytes: u64,
}

impl DownloadHelper {
    async fn new_without_resume(client: &Client, url: &Url, path: &Path) -> Result<Self> {
        let response = get_response_(client, url, None).await?;
        let file = open_file_(path, true).await?;

        Ok(Self {
            response,
            file,
            downloaded_bytes: 0,
        })
    }

    async fn new(client: &Client, url: &Url, path: &Path, resume: bool) -> Result<Self> {
        let (downloaded_bytes, file) = if resume {
            let file = open_file_(path, false).await?;
            let downloaded = file.metadata().await?.len();
            (downloaded, file)
        } else {
            (0, open_file_(path, true).await?)
        };

        // resume from the next of downloaded byte
        let resume_from = (downloaded_bytes != 0).then_some(downloaded_bytes + 1);
        let response = get_response_(client, url, resume_from).await?;

        let status = response.status();
        if status == 416 {
            // 416: server does not support download range, retry without resuming
            info!("download range not satisfiable, retrying without ranges header");

            return Self::new_without_resume(client, url, path).await;
        } else if !status.is_success() {
            bail!("server returns error when attempting download from '{url}': {status}");
        }

        Ok(Self {
            response,
            file,
            downloaded_bytes,
        })
    }
}

async fn open_file_(path: &Path, truncate: bool) -> Result<fs::File> {
    Ok(fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(truncate)
        .open(path)
        .await?)
}

async fn get_response_(
    client: &Client,
    url: &Url,
    resume_from: Option<u64>,
) -> Result<reqwest::Response> {
    let mut builder = client.get(url.as_ref());
    if let Some(bytes) = resume_from {
        builder = builder.header(header::RANGE, format!("bytes={bytes}-"));
    }
    let resp = builder.send().await.with_context(|| {
        format!("failed to receive server response when downloading from '{url}'")
    })?;
    Ok(resp)
}
