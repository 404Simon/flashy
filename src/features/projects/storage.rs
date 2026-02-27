#[cfg(feature = "ssr")]
use s3::{AddressingStyle, Auth, Client, Credentials};

#[cfg(feature = "ssr")]
#[derive(Clone, Debug)]
pub struct MinioSettings {
    pub endpoint: String,
    pub region: String,
    pub access_key: String,
    pub secret_key: String,
    pub bucket: String,
    pub force_path_style: bool,
    pub key_prefix: String,
}

#[cfg(feature = "ssr")]
impl MinioSettings {
    pub fn from_env() -> Self {
        let endpoint = std::env::var("MINIO_ENDPOINT").unwrap_or_else(|_| "http://localhost:9000".to_string());
        let region = std::env::var("MINIO_REGION").unwrap_or_else(|_| "us-east-1".to_string());
        let access_key =
            std::env::var("MINIO_ACCESS_KEY").unwrap_or_else(|_| "minioadmin".to_string());
        let secret_key =
            std::env::var("MINIO_SECRET_KEY").unwrap_or_else(|_| "minioadmin".to_string());
        let bucket = std::env::var("MINIO_BUCKET").unwrap_or_else(|_| "flashy".to_string());
        let force_path_style = std::env::var("MINIO_FORCE_PATH_STYLE")
            .unwrap_or_else(|_| "true".to_string())
            .to_lowercase()
            != "false";
        let key_prefix = std::env::var("MINIO_KEY_PREFIX").unwrap_or_else(|_| "projects".to_string());

        Self {
            endpoint,
            region,
            access_key,
            secret_key,
            bucket,
            force_path_style,
            key_prefix,
        }
    }
}

#[cfg(feature = "ssr")]
pub async fn build_minio_client(settings: &MinioSettings) -> Result<Client, String> {
    let creds = Credentials::new(&settings.access_key, &settings.secret_key)
        .map_err(|e| format!("Failed to build MinIO credentials: {e}"))?;

    let mut builder = Client::builder(&settings.endpoint)
        .map_err(|e| format!("Failed to build MinIO client: {e}"))?
        .region(settings.region.clone())
        .auth(Auth::Static(creds));

    if settings.force_path_style {
        builder = builder.addressing_style(AddressingStyle::Path);
    }

    let client = builder
        .build()
        .map_err(|e| format!("Failed to finalize MinIO client: {e}"))?;

    let head_result = client.buckets().head(&settings.bucket).send().await;
    let needs_create = match head_result {
        Ok(_) => false,
        Err(err) => {
            if matches!(err, s3::Error::Api { status, .. } if status.as_u16() == 404) {
                true
            } else {
                return Err(format!("Failed to check MinIO bucket: {err}"));
            }
        }
    };

    if needs_create {
        client
            .buckets()
            .create(&settings.bucket)
            .send()
            .await
            .map_err(|e| format!("Failed to create MinIO bucket: {e}"))?;
    }

    Ok(client)
}
