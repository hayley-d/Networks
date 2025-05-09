/// This module provides the core functionality for handling HTTP requests and
/// generating appropriate responses in an asynchronous server.
///
/// It includes:
/// - Utility functions for reading files.
/// - Handlers for various HTTP methods (GET, POST, PUT, PATCH, DELETE).
/// - Integration with shared state for caching and user management.
use crate::response::{MyDefault, Response};
use crate::server::SharedState;
use crate::{ContentType, HttpCode, HttpMethod, Request};
use log::{error, info};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs::{self, File};
use tokio::io::AsyncReadExt;
use tokio::sync::Mutex;
use uuid::Uuid;

/// Reads the content of a file located at `path` and returns it as a `Vec<u8>`.
///
/// # Arguments
/// - `path`: A string slice that holds the file path.
///
/// # Returns
/// A vector of bytes representing the file content.
///
/// # Panics
/// Panics if the file cannot be opened or read.
pub async fn read_file_to_bytes(path: &str) -> Vec<u8> {
    let metadata = fs::metadata(path).await.unwrap();
    let mut file = File::open(path).await.unwrap();
    let mut buffer: Vec<u8> = Vec::with_capacity(metadata.len() as usize);
    file.read_to_end(&mut buffer).await.unwrap();
    buffer
}

/// Fetches the byte content of a file from the cache or reads and caches it if not present.
///
/// # Arguments
/// - `state`: A shared, thread-safe state used for caching.
/// - `file_path`: The path to the file to read.
/// - `route_name`: The name of the route associated with the file.
///
/// # Returns
/// A vector of bytes representing the file content.
async fn get_bytes(
    state: Arc<Mutex<SharedState>>,
    file_path: PathBuf,
    route_name: &str,
) -> Vec<u8> {
    let mut state = state.lock().await;
    return match state.get_cached_content(route_name).await {
        Some(b) => b,
        None => state.read_and_cache_page(&file_path, route_name).await,
    };
}

/// Handles incoming HTTP requests and routes them to the appropriate method-specific handler.
///
/// # Arguments
/// - `request`: The incoming HTTP request.
/// - `state`: A shared, thread-safe state used for managing server data and caching.
///
/// # Returns
/// A `Response` object generated based on the request.
pub async fn handle_response(
    request: Request,
    state: Arc<Mutex<SharedState>>,
    session_id: Uuid,
) -> Response {
    match request.method {
        HttpMethod::GET => handle_get(request, state, session_id).await,
        HttpMethod::POST => handle_post(request, state).await,
        HttpMethod::PUT => handle_put(request).await,
        HttpMethod::PATCH => handle_patch(request).await,
        HttpMethod::DELETE => handle_delete(request, state).await,
    }
}

/// Handles HTTP GET requests, serving static files and handling special routes.
///
/// # Arguments
/// - `request`: The incoming GET request.
/// - `state`: A shared, thread-safe state used for managing server data and caching.
///
/// # Returns
/// A `Response` object with the appropriate content and status code.
async fn handle_get(
    request: Request,
    state: Arc<Mutex<SharedState>>,
    session_id: Uuid,
) -> Response {
    let mut response = Response::default()
        .await
        .compression(request.is_compression_supported());

    match request.uri.as_str() {
        "/" => {
            info!(target: "request_logger","GET / from status 200");
            response.add_body(get_bytes(state, PathBuf::from(r"static/index.html"), "/").await);
        }
        "/home" => {
            info!(target: "request_logger","GET /home status: 200");
            response.add_body(get_bytes(state, PathBuf::from(r"static/home.html"), "/home").await);
        }
        "/coffee" => {
            info!(target: "request_logger","GET /coffee status: 418");
            return response
                .code(HttpCode::Teapot)
                .body(get_bytes(state, PathBuf::from(r"static/teapot.html"), "/coffee").await);
        }
        uri if uri.starts_with("/calculate") => {
            let mut state = state.lock().await;
            let user_state: &mut crate::server::UserState =
                match state.user_states.get_mut(&session_id) {
                    Some(s) => s,
                    None => {
                        return response
                            .code(HttpCode::BadRequest)
                            .content_type(ContentType::Text)
                            .body(read_file_to_bytes("static/404.html").await);
                    }
                };

            let params = parse_query_params(uri);
            if let Some(input) = params.get("input") {
                info!(target: "request_logger", "GET /calculate?input={} status: 200", input);
                match input.as_str() {
                    "%2B" | "%2D" | "%2A" | "%2F" | "*" | "/" | "-" | "+" => {
                        println!("Buffer operator");
                        user_state.buffer(String::from(input));
                    }
                    _ => {
                        let operator: Option<String> = user_state.pop();
                        let input: f64 = match input.parse() {
                            Ok(v) => v,
                            Err(_) => 0.0,
                        };
                        match operator {
                            Some(op) => match op.as_str() {
                                "%2B" | "+" => {
                                    let val: f64 = user_state.value + input;
                                    user_state.value = val;
                                }
                                "%2D" | "-" => {
                                    let val: f64 = user_state.value - input;
                                    user_state.value = val;
                                }
                                "%2A" | "*" => {
                                    let val: f64 = user_state.value * input;
                                    user_state.value = val;
                                }
                                "%2F" | "/" => {
                                    let val: f64 = user_state.value / input;
                                    user_state.value = val;
                                }
                                _ => {}
                            },
                            None => {
                                user_state.value = input;
                            }
                        }
                    }
                }

                println!("Current value: {}", user_state.value);
                return response
                    .code(HttpCode::Ok)
                    .content_type(ContentType::Text)
                    .body(user_state.value.to_string().as_bytes().to_vec());
            }
        }
        "/cal" => {
            let content: String = match fs::read_to_string("static/calculator.html").await {
                Ok(content) => content,
                Err(_) => fs::read_to_string("static/404.html")
                    .await
                    .expect("404 Not Found"),
            };

            return response
                .code(HttpCode::Ok)
                .body(content.as_bytes().to_vec());
        }
        _ => {
            error!(target: "error_logger","Failed to serve request GET {}", request.uri);
            info!(target: "request_logger","GET {} status: 404", request.uri);
            return response
                .code(HttpCode::BadRequest)
                .content_type(ContentType::Text)
                .body(get_bytes(state, PathBuf::from(r"static/404.html"), "/404").await);
        }
    }

    response
}

fn parse_query_params(uri: &str) -> HashMap<String, String> {
    let mut params = HashMap::new();
    if let Some(query_start) = uri.find('?') {
        let query_string = &uri[query_start + 1..];
        for pair in query_string.split('&') {
            if let Some((key, value)) = pair.split_once('=') {
                params.insert(key.to_string(), value.to_string());
            }
        }
    }
    params
}

/// Handles HTTP POST requests for specific routes like `/signup` and `/login`.
///
/// # Arguments
/// - `request`: The incoming POST request.
/// - `state`: A shared, thread-safe state used for managing server data and caching.
///
/// # Returns
/// A `Response` object with the appropriate content and status code.
async fn handle_post(request: Request, _state: Arc<Mutex<SharedState>>) -> Response {
    let response = Response::default()
        .await
        .compression(request.is_compression_supported())
        .body(read_file_to_bytes("static/index.html").await)
        .content_type(ContentType::Text);

    error!("Failed to parse invalid POST request");
    response
        .body(String::from("Invalid post URI.").into())
        .code(HttpCode::BadRequest)
}

/// Handles HTTP PUT requests which are currently unsupported and return a `405` Method Not
/// Allowed.
///
/// # Arguments
/// - `request`: The incoming POST request.
///
/// # Returns
/// A `Response` object with the appropriate content and status code.
async fn handle_put(request: Request) -> Response {
    info!(target: "request_logger","PUT {} status 405", request.uri);

    Response::default()
        .await
        .compression(request.is_compression_supported())
        .body(read_file_to_bytes("static/index.html").await)
        .code(HttpCode::MethodNotAllowed)
}

/// Handles HTTP PATCH requests which are currently unsupported and return a `405` Method Not
/// Allowed.
///
/// # Arguments
/// - `request`: The incoming POST request.
///
/// # Returns
/// A `Response` object with the appropriate content and status code.
async fn handle_patch(request: Request) -> Response {
    info!(target: "request_logger","PATCH {} status 405", request.uri);
    Response::default()
        .await
        .compression(request.is_compression_supported())
        .body(read_file_to_bytes("static/index.html").await)
        .code(HttpCode::MethodNotAllowed)
}

/// Handles HTTP DELETE requests which are currently unsupported and return a `405` Method Not
/// Allowed.
///
/// # Arguments
/// - `request`: The incoming POST request.
///
/// # Returns
/// A `Response` object with the appropriate content and status code.

async fn handle_delete(request: Request, _state: Arc<Mutex<SharedState>>) -> Response {
    info!(target: "request_logger","Delete {} status 405", request.uri);
    Response::default()
        .await
        .compression(request.is_compression_supported())
        .body(read_file_to_bytes("static/index.html").await)
        .code(HttpCode::MethodNotAllowed)
}
