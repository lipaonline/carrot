use reqwest::{Client, StatusCode};

#[derive(Clone)]
pub struct BunnyStorage {
    client: Client,
    base_url: String,
    access_key: String,
}

impl BunnyStorage {
    pub fn from_env() -> Self {
        let host = std::env::var("BUNNY_STORAGE_HOST")
            .unwrap_or_else(|_| "storage.bunnycdn.com".to_string());
        let zone = std::env::var("BUNNY_STORAGE_ZONE").expect("BUNNY_STORAGE_ZONE must be set");
        let access_key =
            std::env::var("BUNNY_STORAGE_ACCESS_KEY").expect("BUNNY_STORAGE_ACCESS_KEY must be set");

        Self {
            client: Client::new(),
            base_url: format!("https://{host}/{zone}"),
            access_key,
        }
    }

    fn object_url(&self, key: &str) -> String {
        format!("{}/{}", self.base_url, key)
    }

    pub async fn put(&self, key: &str, data: Vec<u8>) -> Result<(), reqwest::Error> {
        self.client
            .put(self.object_url(key))
            .header("AccessKey", &self.access_key)
            .header("Content-Type", "application/octet-stream")
            .body(data)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    pub async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, reqwest::Error> {
        let response = self
            .client
            .get(self.object_url(key))
            .header("AccessKey", &self.access_key)
            .send()
            .await?;

        if response.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }

        let bytes = response.error_for_status()?.bytes().await?;
        Ok(Some(bytes.to_vec()))
    }

    pub async fn delete(&self, key: &str) -> Result<(), reqwest::Error> {
        let response = self
            .client
            .delete(self.object_url(key))
            .header("AccessKey", &self.access_key)
            .send()
            .await?;

        if response.status() == StatusCode::NOT_FOUND {
            return Ok(());
        }

        response.error_for_status()?;
        Ok(())
    }
}
