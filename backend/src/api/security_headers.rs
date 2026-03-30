//! セキュリティヘッダーミドルウェア
//!
//! すべてのHTTPレスポンスにセキュリティ関連ヘッダーを付加する。
//! OWASP推奨のセキュリティヘッダーを実装している。

use axum::{body::Body, extract::Request, http::Response, middleware::Next};

/// セキュリティヘッダーをレスポンスに付加するミドルウェア
///
/// 付加されるヘッダー:
/// - `X-Content-Type-Options: nosniff` — MIMEタイプスニッフィング防止
/// - `X-Frame-Options: DENY` — クリックジャッキング防止
/// - `X-XSS-Protection: 0` — 古いXSSフィルターを無効化（CSPを使用）
/// - `Referrer-Policy: strict-origin-when-cross-origin` — Refererヘッダー制御
/// - `Permissions-Policy: ...` — ブラウザ機能のアクセス制限
/// - `Cache-Control: no-store` — APIレスポンスのキャッシュ禁止
pub async fn security_headers_middleware(request: Request, next: Next) -> Response<Body> {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();

    // MIMEタイプスニッフィングを防止
    headers.insert(
        "X-Content-Type-Options",
        "nosniff".parse().expect("valid header value"),
    );

    // クリックジャッキング攻撃を防止（iframe埋め込みを禁止）
    headers.insert(
        "X-Frame-Options",
        "DENY".parse().expect("valid header value"),
    );

    // 古いブラウザの XSS フィルターを無効化（CSP が優先）
    headers.insert("X-XSS-Protection", "0".parse().expect("valid header value"));

    // Referer ヘッダーの送信範囲を制限
    headers.insert(
        "Referrer-Policy",
        "strict-origin-when-cross-origin"
            .parse()
            .expect("valid header value"),
    );

    // ブラウザ機能へのアクセスを制限
    headers.insert(
        "Permissions-Policy",
        "camera=(), microphone=(), geolocation=(), payment=()"
            .parse()
            .expect("valid header value"),
    );

    // APIレスポンスをキャッシュしない
    headers.insert(
        "Cache-Control",
        "no-store, max-age=0".parse().expect("valid header value"),
    );

    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{middleware, routing::get, Router};
    use axum_test::TestServer;

    #[tokio::test]
    async fn test_security_headers_present() {
        let app = Router::new()
            .route("/test", get(|| async { "ok" }))
            .layer(middleware::from_fn(security_headers_middleware));
        let server = TestServer::new(app.into_make_service()).unwrap();

        let response = server.get("/test").await;

        assert_eq!(response.status_code().as_u16(), 200);

        let headers = response.headers();
        assert_eq!(
            headers
                .get("x-content-type-options")
                .map(|v| v.to_str().unwrap()),
            Some("nosniff")
        );
        assert_eq!(
            headers.get("x-frame-options").map(|v| v.to_str().unwrap()),
            Some("DENY")
        );
        assert_eq!(
            headers.get("x-xss-protection").map(|v| v.to_str().unwrap()),
            Some("0")
        );
        assert_eq!(
            headers.get("referrer-policy").map(|v| v.to_str().unwrap()),
            Some("strict-origin-when-cross-origin")
        );
        assert!(headers.get("permissions-policy").is_some());
        assert_eq!(
            headers.get("cache-control").map(|v| v.to_str().unwrap()),
            Some("no-store, max-age=0")
        );
    }
}
