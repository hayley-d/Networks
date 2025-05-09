use std::net::SocketAddr;
use std::sync::Arc;

use chrono::Utc;
use rate_limiter::rate_limiter::{rate_limit, State};
use rate_limiter::rate_limiter_proto::rate_limiter_server::RateLimiterServer;
use rate_limiter::rate_limiter_proto::{
    rate_limiter_server::RateLimiter, RateLimitRequest, RateLimitResponse,
};
use tokio::sync::Mutex;
use tonic::transport::Server;

#[derive(Debug)]
struct RateLimiterService {
    pub state: Arc<Mutex<State>>,
}

#[tonic::async_trait]
impl RateLimiter for RateLimiterService {
    async fn check_request(
        &self,
        request: tonic::Request<RateLimitRequest>,
    ) -> Result<tonic::Response<RateLimitResponse>, tonic::Status> {
        let request = request.into_inner();

        let timestamp = Utc::now();
        println!("IP address: {}", &request.ip_address);
        let result = rate_limit(
            self.state.clone(),
            request.ip_address,
            &request.endpoint,
            timestamp,
        )
        .await;

        match result {
            Ok(_) => Ok(tonic::Response::new(RateLimitResponse {
                request_id: request.request_id.clone(),
                allowed: true,
            })),
            Err(_) => Ok(tonic::Response::new(RateLimitResponse {
                request_id: request.request_id.clone(),
                allowed: false,
            })),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = SocketAddr::from(([127, 0, 0, 1], 50051));

    let state: Arc<Mutex<State>> = State::new();

    let rate_limiter_service = RateLimiterService { state };

    println!("Server listening on {}", addr);

    // Run the gRPC server
    Server::builder()
        .add_service(RateLimiterServer::new(rate_limiter_service))
        .serve(addr)
        .await?;

    Ok(())
}
