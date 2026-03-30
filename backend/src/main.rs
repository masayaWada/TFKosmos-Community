use clap::Parser;

#[cfg(feature = "gui-server")]
use axum::{http::header, http::Method, middleware, response::Json, routing::get, Router};
#[cfg(feature = "gui-server")]
use serde_json::{json, Value};
#[cfg(feature = "gui-server")]
use std::net::SocketAddr;
#[cfg(feature = "gui-server")]
use std::sync::Arc;
#[cfg(feature = "gui-server")]
use tower::ServiceBuilder;
#[cfg(feature = "gui-server")]
use tower_governor::{governor::GovernorConfigBuilder, GovernorLayer};
#[cfg(feature = "gui-server")]
use tower_http::cors::{AllowOrigin, Any, CorsLayer};
#[cfg(feature = "gui-server")]
use tower_http::trace::TraceLayer;
#[cfg(feature = "gui-server")]
use utoipa::OpenApi;
#[cfg(feature = "gui-server")]
use utoipa_swagger_ui::SwaggerUi;

mod api;
mod cli;
mod config;
mod domain;
mod infra;
#[cfg(feature = "license-manager")]
mod license;
mod models;
mod services;

#[cfg(feature = "gui-server")]
use api::audit_middleware::audit_middleware;
#[cfg(feature = "gui-server")]
use api::routes;
#[cfg(feature = "gui-server")]
use api::security_headers::security_headers_middleware;
#[cfg(feature = "gui-server")]
use config::Config;
#[cfg(feature = "gui-server")]
use services::audit_service::AuditService;
#[cfg(feature = "gui-server")]
use services::config_management_service::ConfigManagementService;
#[cfg(feature = "gui-server")]
use services::connection_service::{ConnectionService, RealCloudConnectionTester};
#[cfg(feature = "gui-server")]
use services::drift_service::DriftService;
#[cfg(feature = "gui-server")]
use services::generation_service::GenerationService;
#[cfg(feature = "gui-server")]
use services::resource_service::ResourceService;
#[cfg(feature = "gui-server")]
use services::scan_service::{RealScannerFactory, ScanService};
#[cfg(feature = "gui-server")]
use services::template_service::TemplateService;

#[cfg(feature = "gui-server")]
#[derive(OpenApi)]
#[openapi(
    info(
        title = "TFKosmos API",
        version = "0.1.0",
        description = "TFKosmos - Cloud Infrastructure to Terraform Code Generator API",
    ),
    paths(
        routes::export::export_resources,
        routes::config::export_config,
        routes::config::import_config,
        routes::config::list_saved_configs,
        routes::config::save_config,
        routes::config::load_config,
        routes::config::delete_config,
        routes::audit::list_audit_logs,
    ),
    components(schemas(
        models::ScanConfig,
        models::GenerationConfig,
        models::ScanResponse,
        models::GenerationResponse,
        models::ResourceListResponse,
        models::ConnectionTestResponse,
        models::AzureSubscription,
        models::AzureResourceGroup,
        models::ValidationError,
        models::TemplateValidationResponse,
        models::DependencyNode,
        models::DependencyEdge,
        models::DependencyGraph,
        models::CloudProvider,
        api::error::ErrorResponse,
        api::error::ErrorDetail,
        models::drift::DriftDetectionRequest,
        models::drift::DriftDetectionResponse,
        models::drift::DriftSummary,
        models::drift::DriftItem,
        models::drift::DriftType,
        models::drift::ChangedField,
        routes::export::ExportRequest,
        services::audit_service::AuditEntry,
        services::audit_service::AuditAction,
        services::audit_service::AuditStatus,
        services::audit_service::AuditQuery,
    )),
    tags(
        (name = "connection", description = "Cloud provider connection testing"),
        (name = "scan", description = "Resource scanning operations"),
        (name = "resources", description = "Scanned resource management"),
        (name = "generate", description = "Terraform code generation"),
        (name = "templates", description = "Template management"),
        (name = "drift", description = "Drift detection between Terraform state and cloud resources"),
        (name = "export", description = "Resource export (CSV/JSON)"),
        (name = "config", description = "Configuration management"),
        (name = "audit", description = "Audit log management"),
    )
)]
struct ApiDoc;

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_target(false)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .init();

    // Parse CLI arguments
    let cli_args = cli::Cli::parse();

    // Dispatch CLI subcommands (non-server modes)
    match cli_args.command {
        Some(cli::Commands::Scan { config, output }) => {
            if let Err(e) = cli::commands::run_scan(&config, &output).await {
                eprintln!("エラー: {:#}", e);
                std::process::exit(1);
            }
            return;
        }
        Some(cli::Commands::Drift {
            scan_id,
            state_file,
            output,
        }) => {
            if let Err(e) = cli::commands::run_drift(&scan_id, &state_file, &output).await {
                eprintln!("エラー: {:#}", e);
                std::process::exit(1);
            }
            return;
        }
        Some(cli::Commands::Generate {
            scan_id,
            output_dir,
        }) => {
            if let Err(e) = cli::commands::run_generate(&scan_id, &output_dir).await {
                eprintln!("エラー: {:#}", e);
                std::process::exit(1);
            }
            return;
        }
        #[cfg(feature = "license-manager")]
        Some(cli::Commands::License { action }) => {
            let result = match action {
                cli::LicenseAction::Activate { key } => cli::license_cmd::run_activate(&key).await,
                cli::LicenseAction::Status => cli::license_cmd::run_status().await,
                cli::LicenseAction::Deactivate => cli::license_cmd::run_deactivate().await,
            };
            if let Err(e) = result {
                eprintln!("エラー: {:#}", e);
                std::process::exit(1);
            }
            return;
        }
        #[cfg(feature = "gui-server")]
        Some(cli::Commands::Serve { bind: _ }) | None => {
            // Fall through to server mode below
        }
        #[cfg(not(feature = "gui-server"))]
        None => {
            eprintln!("TFKosmos Community Edition (CLI専用)");
            eprintln!();
            eprintln!("APIサーバーは TFKosmos Pro 以上で利用可能です。");
            eprintln!("CLIコマンドを使用してください:");
            eprintln!("  tfkosmos scan --config <file>       リソーススキャン");
            eprintln!("  tfkosmos generate --scan-id <id>    Terraformコード生成");
            eprintln!("  tfkosmos drift --scan-id <id> --state-file <file>  ドリフト検出");
            eprintln!();
            eprintln!("詳細: tfkosmos --help");
            std::process::exit(0);
        }
    }

    // Server mode (Pro+ only)
    #[cfg(feature = "gui-server")]
    {
        run_server().await;
    }

    #[cfg(not(feature = "gui-server"))]
    {
        // Community ビルドでは到達しないが、コンパイラ向けの安全ガード
        unreachable!("Community Edition does not support server mode");
    }
}

/// APIサーバーを起動（gui-server フィーチャー有効時のみコンパイル）
#[cfg(feature = "gui-server")]
async fn run_server() {
    // Load configuration from environment
    let config = Config::from_env();

    tracing::info!(
        environment = ?config.environment,
        "Starting TFKosmos server"
    );

    // Build CORS layer based on environment
    //
    // - 開発環境: 全オリジンを許可（開発の利便性のため）
    // - 本番環境: TFKOSMOS_CORS_ORIGINS で指定されたオリジンのみ許可
    //
    // 環境変数の例:
    //   TFKOSMOS_ENV=production
    //   TFKOSMOS_CORS_ORIGINS=https://example.com,https://app.example.com
    let cors = build_cors_layer(&config);

    // Rate limiter for scan endpoints: max 10 requests per second per IP
    let governor_conf = Arc::new(
        GovernorConfigBuilder::default()
            .per_second(10)
            .burst_size(5)
            .use_headers()
            .finish()
            .expect("Failed to build rate limiter config"),
    );

    // Instantiate services with dependency injection
    let connection_service = Arc::new(ConnectionService::new(Arc::new(RealCloudConnectionTester)));
    let scan_service = Arc::new(ScanService::new(Arc::new(RealScannerFactory::new())));
    let resource_service = Arc::new(ResourceService::new(scan_service.clone()));
    let generation_service = Arc::new(GenerationService::new(scan_service.clone()));
    let drift_service = Arc::new(DriftService::new(scan_service.clone()));

    // Build scan router with rate limiting
    let scan_router = routes::scan::router(scan_service.clone()).layer(GovernorLayer {
        config: governor_conf,
    });

    // Determine base directory for template service (directory of the binary / current dir)
    let base_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let template_service = Arc::new(TemplateService::new(base_dir.clone()));

    // Config management service
    let config_dir = base_dir.join("configs");
    let config_management_service = Arc::new(ConfigManagementService::new(config_dir));

    // Audit service
    let audit_log_dir = base_dir.join("audit-logs");
    let audit_service = Arc::new(AuditService::new(audit_log_dir));

    // Build application with routes
    let mut app = Router::new()
        .route("/", get(root))
        .route("/health", get(health))
        .nest(
            "/api/connection",
            routes::connection::router(connection_service),
        )
        .nest("/api/scan", scan_router)
        .nest(
            "/api/resources",
            routes::resources::router(resource_service.clone()),
        )
        .nest(
            "/api/generate",
            routes::generate::router(generation_service.clone()),
        )
        .nest(
            "/api/templates",
            routes::templates::router(template_service),
        )
        .nest(
            "/api/export",
            routes::export::router(resource_service.clone()),
        )
        .nest(
            "/api/config",
            routes::config::router(config_management_service),
        )
        .nest("/api/drift", routes::drift::router(drift_service))
        .nest("/api/audit", routes::audit::router(audit_service.clone()));

    #[cfg(feature = "license-manager")]
    {
        app = app.nest("/api/license", routes::license::router());
    }

    // Pro edition routes
    #[cfg(feature = "multi-account")]
    {
        app = app.nest("/api/scan/multi-account", routes::multi_account::router());
    }
    #[cfg(feature = "import-engine")]
    {
        app = app.nest("/api/import", routes::import_exec::router());
    }

    // Enterprise edition routes
    #[cfg(feature = "compliance")]
    {
        app = app.nest("/api/compliance", routes::compliance::router());
    }

    let mut app = app
        .layer(axum::Extension(audit_service))
        .layer(middleware::from_fn(audit_middleware));

    // Swagger UI（開発環境のみ）
    if !config.is_production() {
        tracing::info!("Swagger UI enabled at /swagger-ui");
        app = app
            .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()));
    }

    let app = app
        .layer(middleware::from_fn(security_headers_middleware))
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(cors),
        );

    // Start periodic cache cleanup tasks
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(300));
        loop {
            interval.tick().await;
            scan_service.cleanup_expired_scans().await;
            generation_service.cleanup_expired_generations().await;
        }
    });

    // Start server
    let bind_address = config.bind_address();
    let listener = tokio::net::TcpListener::bind(&bind_address)
        .await
        .expect("Failed to bind to address");

    tracing::info!("Server listening on http://{}", bind_address);

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .expect("Server failed to start");
}

/// 環境に応じたCORSレイヤーを構築
#[cfg(feature = "gui-server")]
fn build_cors_layer(config: &Config) -> CorsLayer {
    let base_cors = CorsLayer::new()
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION, header::ACCEPT]);

    if config.is_production() && !config.cors_origins.is_empty() {
        // 本番環境: 指定されたオリジンのみ許可
        let origins: Vec<_> = config
            .cors_origins
            .iter()
            .filter_map(|origin| origin.parse().ok())
            .collect();

        tracing::info!(
            origins = ?config.cors_origins,
            "CORS: Allowing specific origins only"
        );

        base_cors.allow_origin(AllowOrigin::list(origins))
    } else {
        // 開発環境 または オリジン未指定: 全オリジンを許可
        if config.is_production() {
            tracing::warn!(
                "CORS: No origins specified in production mode, allowing all origins. \
                 Set TFKOSMOS_CORS_ORIGINS to restrict access."
            );
        } else {
            tracing::info!("CORS: Development mode - allowing all origins");
        }

        base_cors.allow_origin(Any)
    }
}

#[cfg(feature = "gui-server")]
async fn root() -> Json<Value> {
    Json(json!({
        "message": "TFKosmos API",
        "version": "0.1.0"
    }))
}

#[cfg(feature = "gui-server")]
async fn health() -> Json<Value> {
    Json(json!({
        "status": "healthy"
    }))
}
