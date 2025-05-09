use uuid::Uuid;

pub struct Request {
    pub request_id: Uuid,
    pub client_ip: String,
    pub uri: String,
    pub request: http::Request<Vec<u8>>,
}

impl Request {
    pub fn new(mut uri: String, client_ip: String, mut request: http::Request<Vec<u8>>) -> Request {
        // get the uri from the first line
        if uri == "/favicon.ico" {
            uri = "/".to_string();
        }

        let request_id = Uuid::new_v4();

        // add the request ID to the headers
        request
            .headers_mut()
            .insert("X-Request-ID", request_id.to_string().parse().unwrap());

        Request {
            request_id,
            client_ip,
            uri,
            request,
        }
    }
}

pub fn buffer_to_request(
    buffer: Vec<u8>,
    client_ip: String,
    request_id: i64,
) -> Result<http::Request<Vec<u8>>, String> {
    let http_request: HttpRequest = match HttpRequest::new(&buffer, client_ip, request_id) {
        Ok(r) => r,
        Err(e) => return Err(e),
    };

    println!("{http_request}");

    let body: Vec<u8> = Vec::new();
    return Ok(http::Request::builder()
        .method("GET")
        .uri("/")
        .body(body)
        .unwrap());
}

/*pub fn buffer_to_request(buffer: Vec<u8>) -> Result<http::Request<Vec<u8>>, String> {
    println!("{}", String::from_utf8_lossy(&buffer));
    // Find the position of the headers-body delimiter (\r\n\r\n)
    let delimiter = match buffer.windows(4).position(|window| window == b"\r\n\r\n") {
        Some(pos) => pos,
        None => return Err("Malformed request: missing headers-body delimiter".to_string()),
    };

    let (header_part, body) = buffer.split_at(delimiter + 4);

    let request_str: String = match std::str::from_utf8(header_part) {
        Ok(s) => s.to_string(),
        Err(_) => return Err("Invalid UTF-8 sequence in buffer".to_string()),
    };

    let mut lines = request_str.split("\r\n");

    let request_line: String = match lines.next() {
        Some(l) => l.to_string(),
        None => return Err("Missing request line".to_string()),
    };

    let mut request_line = request_line.split_whitespace();

    let method: Method = match request_line.next() {
        Some(m) => match m.parse::<Method>() {
            Ok(m) => m,
            _ => return Err("Invalid HTTP method".to_string()),
        },
        _ => return Err("Missing HTTP method".to_string()),
    };

    let uri: Uri = match request_line.next() {
        Some(u) => match u.parse::<Uri>() {
            Ok(u) => u,
            _ => return Err("Invalid URI".to_string()),
        },
        _ => return Err("Missing URI".to_string()),
    };

    let version: Version = match request_line.next() {
        Some(v) => match v {
            "HTTP/1.0" => Version::HTTP_10,
            "HTTP/1.1" => Version::HTTP_11,
            _ => return Err("Invalid HTTP Version".to_string()),
        },
        None => return Err("Missing HTTP version".to_string()),
    };

    let mut request_builder = http::Request::builder()
        .method(method)
        .uri(uri)
        .version(version);

    for line in lines.by_ref() {
        if line.is_empty() {
            break;
        }

        let mut header_parts = line.splitn(2, ": ");

        let name = match header_parts.next() {
            Some(n) => n,
            _ => return Err("Malformed Header".to_string()),
        };

        let value = match header_parts.next() {
            Some(n) => n,
            _ => return Err("Malformed Header".to_string()),
        };

        request_builder = request_builder.header(name, value);
    }

    let request = match request_builder.body(body.to_vec()) {
        Ok(r) => r,
        _ => return Err("Failed to build request".to_string()),
    };

    Ok(request)
}*/

use core::str;
use std::fmt::Display;

#[derive(Debug)]
pub struct Clock {
    lamport_timestamp: i64,
}

impl Clock {
    pub fn new() -> Self {
        return Clock {
            lamport_timestamp: 0,
        };
    }
    pub fn increment_time(&mut self) -> i64 {
        let temp: i64 = self.lamport_timestamp;
        self.lamport_timestamp += 1;
        return temp;
    }
}

#[derive(Debug)]
pub enum Protocol {
    Http,
}

impl Display for Protocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Protocol::Http => write!(f, "HTTP/1.1"),
        }
    }
}

#[derive(Debug)]
pub struct Header {
    pub title: String,
    pub value: String,
}

impl Display for Header {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} : {}", self.title, self.value)
    }
}

#[derive(Debug)]
pub enum ContentType {
    Text,
    Html,
    Json,
}

impl Display for ContentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContentType::Text => write!(f, "text/plain"),
            ContentType::Html => write!(f, "text/html"),
            ContentType::Json => write!(f, "application/json"),
        }
    }
}

pub struct HttpRequest {
    pub request_id: i64,
    pub client_ip: String,
    pub headers: Vec<String>,
    pub body: String,
    pub method: HttpMethod,
    pub uri: String,
}

impl Display for HttpRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Request: {{method: {}, path: {}, request_id: {},client_ip: {}}}",
            self.method, self.uri, self.request_id, self.client_ip
        )
    }
}

impl HttpRequest {
    pub fn print(&self) {
        println!("{} New Request:", ">>");
        println!("{}{}", self.method.to_string(), self.uri);
    }

    pub fn new(buffer: &[u8], client_ip: String, request_id: i64) -> Result<HttpRequest, String> {
        // unwrap is safe as request has been parsed for any issues before this is called
        let request = String::from_utf8(buffer.to_vec()).unwrap();

        // split the request by line
        let request: Vec<&str> = request.lines().collect();

        if request.len() < 3 {
            eprintln!("Recieved invalid request");
            return Err(String::from("Invalid request"));
        }

        // get the http method from the first line
        let method: HttpMethod =
            HttpMethod::new(request[0].split_whitespace().collect::<Vec<&str>>()[0]);

        // get the uri from the first line
        let mut uri: String = request[0].split_whitespace().collect::<Vec<&str>>()[1].to_string();
        if uri == "/favicon.ico" {
            uri = "/".to_string();
        }

        // headers are the rest of the
        let mut headers: Vec<String> = Vec::with_capacity(request.len() - 1);
        let mut body: String = String::new();
        let mut flag = false;
        for line in &request[1..] {
            if line.is_empty() {
                flag = true;
                continue;
            }
            if flag {
                body.push_str(line);
            } else {
                headers.push(line.to_string());
            }
        }

        return Ok(HttpRequest {
            request_id,
            client_ip,
            headers,
            body,
            method,
            uri,
        });
    }

    pub fn is_compression_supported(&self) -> bool {
        for header in &self.headers {
            let header = header.to_lowercase();

            if header.contains("firefox") {
                return false;
            }

            if header.contains("accept-encoding") {
                if header.contains(',') {
                    // multiple compression types
                    let mut encodings: Vec<&str> =
                        header.split(", ").map(|m| m.trim()).collect::<Vec<&str>>();
                    encodings[0] = &encodings[0].split_whitespace().collect::<Vec<&str>>()[1];

                    for encoding in encodings {
                        if encoding == "gzip" || encoding.contains("gzip") {
                            return true;
                        }
                    }
                } else {
                    if header
                        .to_lowercase()
                        .split_whitespace()
                        .collect::<Vec<&str>>()[1]
                        == "gzip"
                    {
                        return true;
                    }
                }
            }
        }
        return false;
    }
}

#[derive(Debug)]
pub enum HttpCode {
    Ok,
    Created,
    BadRequest,
    Unauthorized,
    NotFound,
    MethodNotAllowed,
    RequestTimeout,
    Teapot,
    InternalServerError,
}

impl Display for HttpCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HttpCode::Ok => write!(f, "200 OK"),
            HttpCode::Created => write!(f, "201 Created"),
            HttpCode::BadRequest => write!(f, "400 Bad Request"),
            HttpCode::Unauthorized => write!(f, "401 Unauthorized"),
            HttpCode::NotFound => write!(f, "404 Not Found"),
            HttpCode::MethodNotAllowed => write!(f, "405 Method Not Allowed"),
            HttpCode::RequestTimeout => write!(f, "408 Request Timeout"),
            HttpCode::Teapot => write!(f, "418 I'm a teapot"),
            HttpCode::InternalServerError => write!(f, "500 Internal Server Error"),
        }
    }
}

impl PartialEq for HttpCode {
    fn eq(&self, other: &Self) -> bool {
        match self {
            HttpCode::Ok => match other {
                HttpCode::Ok => true,
                _ => false,
            },
            HttpCode::Created => match other {
                HttpCode::Created => true,
                _ => false,
            },
            HttpCode::BadRequest => match other {
                HttpCode::BadRequest => true,
                _ => false,
            },
            HttpCode::Unauthorized => match other {
                HttpCode::Unauthorized => true,
                _ => false,
            },
            HttpCode::NotFound => match other {
                HttpCode::NotFound => true,
                _ => false,
            },
            HttpCode::MethodNotAllowed => match other {
                HttpCode::MethodNotAllowed => true,
                _ => false,
            },
            HttpCode::RequestTimeout => match other {
                HttpCode::RequestTimeout => true,
                _ => false,
            },
            HttpCode::Teapot => match other {
                HttpCode::Teapot => true,
                _ => false,
            },
            HttpCode::InternalServerError => match other {
                HttpCode::InternalServerError => true,
                _ => false,
            },
        }
    }
}

#[derive(Debug)]
pub enum HttpMethod {
    GET,
    POST,
    PUT,
    PATCH,
    DELETE,
}

impl HttpMethod {
    pub fn new(method: &str) -> HttpMethod {
        if method.to_uppercase().contains("GET") {
            HttpMethod::GET
        } else if method.to_uppercase().contains("POST") {
            HttpMethod::POST
        } else if method.to_uppercase().contains("PUT") {
            HttpMethod::PUT
        } else if method.to_uppercase().contains("DELETE") {
            HttpMethod::DELETE
        } else {
            HttpMethod::PATCH
        }
    }
}

impl Display for HttpMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HttpMethod::GET => write!(f, "GET"),
            HttpMethod::POST => write!(f, "POST"),
            HttpMethod::PUT => write!(f, "PUT"),
            HttpMethod::PATCH => write!(f, "PATCH"),
            HttpMethod::DELETE => write!(f, "DELETE"),
        }
    }
}

impl PartialEq for HttpMethod {
    fn eq(&self, other: &Self) -> bool {
        match self {
            HttpMethod::GET => match other {
                HttpMethod::GET => true,
                _ => false,
            },
            HttpMethod::POST => match other {
                HttpMethod::POST => true,
                _ => false,
            },
            HttpMethod::PUT => match other {
                HttpMethod::PUT => true,
                _ => false,
            },
            HttpMethod::PATCH => match other {
                HttpMethod::PATCH => true,
                _ => false,
            },
            HttpMethod::DELETE => match other {
                HttpMethod::DELETE => true,
                _ => false,
            },
        }
    }
}
