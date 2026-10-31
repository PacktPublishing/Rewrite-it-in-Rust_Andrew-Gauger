use cairn_auth::config::Config;
use cairn_auth::entra::Entra;
use sqlx::postgres::PgPoolOptions;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().with_target(false).init();
    let cfg = Config::load()?;
    let db = PgPoolOptions::new()
        .max_connections(4)
        .connect(&cfg.database_url)
        .await?;
    let entra = Entra::discover(
        &cfg.issuer,
        &cfg.client_id,
        &cfg.client_secret,
        &cfg.redirect_uri,
    )
    .await?;
    let app = cairn_auth::server::router(db, entra, cfg.session_secs);
    let listener = tokio::net::TcpListener::bind(&cfg.listen).await?;
    tracing::info!(
        "cairn-auth on http://{}, logins last {} seconds",
        cfg.listen,
        cfg.session_secs
    );
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            tokio::signal::ctrl_c().await.ok();
        })
        .await?;
    Ok(())
}
