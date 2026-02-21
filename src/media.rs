use std::path::{Path, PathBuf};

use tokio::io::AsyncWriteExt;

pub async fn download_image(url: &str, download_dir: &Path) -> anyhow::Result<PathBuf> {
    tokio::fs::create_dir_all(download_dir).await?;

    let file_name = url
        .rsplit('/')
        .next()
        .unwrap_or("image.iso")
        .to_string();

    let dest = download_dir.join(&file_name);

    // If already downloaded, reuse
    if dest.exists() {
        return Ok(dest);
    }

    let response = reqwest::get(url).await?;
    if !response.status().is_success() {
        anyhow::bail!(
            "Failed to download {}: HTTP {}",
            url,
            response.status()
        );
    }

    let tmp_path = download_dir.join(format!(".{file_name}.tmp"));
    let mut file = tokio::fs::File::create(&tmp_path).await?;

    let bytes = response.bytes().await?;
    file.write_all(&bytes).await?;
    file.flush().await?;
    drop(file);

    tokio::fs::rename(&tmp_path, &dest).await?;

    Ok(dest)
}
