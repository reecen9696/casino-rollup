use anyhow::Result;
use axum::{routing::post, Router};
use clap::Parser;
use std::net::SocketAddr;
use tracing::info;

#[derive(Parser)]
#[command(name = "sequencer")]
#[command(about = "ZK Casino Sequencer Service")]
pub struct Args {
    #[arg(short, long, default_value = "3000")]
    pub port: u16,
}

pub fn create_app() -> Router {
    Router::new()
        .route("/health", axum::routing::get(health_check))
        .route("/v1/bet", post(|| async { "Bet endpoint placeholder" }))
}

pub async fn health_check() -> &'static str {
    "OK"
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let args = Args::parse();

    let app = create_app();

    let addr = SocketAddr::from(([127, 0, 0, 1], args.port));
    info!("Sequencer listening on {}", addr);
    
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt; // for `oneshot`

    #[tokio::test]
    async fn test_health_check() {
        let app = create_app();

        let response = app
            .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert_eq!(&body[..], b"OK");
    }

    #[tokio::test]
    async fn test_bet_endpoint_placeholder() {
        let app = create_app();

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/bet")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert_eq!(&body[..], b"Bet endpoint placeholder");
    }

    #[tokio::test]
    async fn test_nonexistent_route() {
        let app = create_app();

        let response = app
            .oneshot(Request::builder().uri("/nonexistent").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn test_args_parsing() {
        let args = Args::parse_from(&["sequencer", "--port", "8080"]);
        assert_eq!(args.port, 8080);

        let args = Args::parse_from(&["sequencer"]);
        assert_eq!(args.port, 3000); // default value
    }

    #[test]
    fn test_health_check_function() {
        let result = tokio_test::block_on(health_check());
        assert_eq!(result, "OK");
    }
}