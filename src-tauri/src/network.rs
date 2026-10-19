use serde::de::DeserializeOwned;

pub const PAGE_LIMIT: usize = 8 * 1024 * 1024;
pub const API_LIMIT: usize = 16 * 1024 * 1024;
pub const LYRICS_LIMIT: usize = 2 * 1024 * 1024;

/// Bound decoded bytes too: Content-Length alone does not cover chunked responses.
pub async fn bytes(mut response: reqwest::Response, limit: usize) -> Result<Vec<u8>, String> {
    if !response.status().is_success() {
        return Err(format!("Сервис вернул HTTP {}", response.status()));
    }
    if response.content_length().is_some_and(|n| n > limit as u64) {
        return Err("Ответ сервиса слишком большой".into());
    }
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "Не удалось прочитать ответ сервиса")?
    {
        if chunk.len() > limit.saturating_sub(body.len()) {
            return Err("Ответ сервиса слишком большой".into());
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}
pub async fn text(response: reqwest::Response, limit: usize) -> Result<String, String> {
    String::from_utf8(bytes(response, limit).await?)
        .map_err(|_| "Некорректная кодировка ответа сервиса".into())
}
pub async fn json<T: DeserializeOwned>(
    response: reqwest::Response,
    limit: usize,
) -> Result<T, String> {
    serde_json::from_slice(&bytes(response, limit).await?)
        .map_err(|_| "Некорректный JSON в ответе сервиса".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    async fn response(raw: &'static [u8]) -> reqwest::Response {
        let server = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = server.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut stream, _) = server.accept().await.unwrap();
            let mut request = [0; 1024];
            let _ = stream.read(&mut request).await;
            stream.write_all(raw).await.unwrap();
        });
        reqwest::get(format!("http://{address}/")).await.unwrap()
    }
    #[tokio::test]
    async fn caps_both_declared_and_chunked_bodies() {
        let r = response(
            b"HTTP/1.1 200 OK\r\nContent-Length: 12\r\nConnection: close\r\n\r\nabcdefghijkl",
        )
        .await;
        assert!(bytes(r, 8).await.is_err());
        let r=response(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\nc\r\nabcdefghijkl\r\n0\r\n\r\n").await;
        assert!(bytes(r, 8).await.is_err());
        let r =
            response(b"HTTP/1.1 200 OK\r\nContent-Length: 7\r\nConnection: close\r\n\r\n{\"a\":1}")
                .await;
        assert_eq!(json::<serde_json::Value>(r, 8).await.unwrap()["a"], 1);
    }
}
