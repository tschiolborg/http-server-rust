use anyhow::Result;
use std::cmp::min;
use std::collections::HashMap;
use std::fmt::Display;
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::net::{TcpListener, TcpStream};

// header keys
const CONTENT_LENGTH: &str = "Content-Length";
const CONTENT_TYPE: &str = "Content-Type";
const USER_AGENT: &str = "User-Agent";

// header content types
const TEXT_PLAIN: &str = "text/plain";

#[derive(Debug)]
struct Request {
    method: Method,
    path: String,
    version: String,
    headers: HashMap<String, String>,
    body: String,
}

impl Display for Request {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut headers = String::new();
        for (key, value) in &self.headers {
            headers.push_str(&format!("{}: {}\r\n", key, value));
        }

        write!(
            f,
            "{} {} {}\r\n{}\r\n{}",
            self.method.as_str(),
            self.path,
            self.version,
            headers,
            self.body
        )
    }
}

struct Response {
    status: Status,
    headers: HashMap<String, String>,
    body: String,
}

impl Response {
    fn new(status: Status) -> Self {
        Self {
            status,
            headers: HashMap::new(),
            body: String::new(),
        }
    }

    fn with_header(mut self, key: &str, value: &str) -> Self {
        self.headers.insert(key.to_owned(), value.to_owned());
        self
    }

    fn with_body(mut self, body: &str) -> Self {
        self.body = body.to_owned();
        self
    }

    fn with_content_type_and_current_length(self, content_type: &str) -> Self {
        let body_length = self.body.len().to_string();
        self.with_header(CONTENT_TYPE, content_type)
            .with_header(CONTENT_LENGTH, body_length.as_str())
    }
}

#[derive(Debug, PartialEq)]
enum Method {
    Get,
    Post,
    Put,
    Delete,
}

impl Method {
    fn as_str(&self) -> &str {
        match self {
            Method::Get => "GET",
            Method::Post => "POST",
            Method::Put => "PUT",
            Method::Delete => "DELETE",
        }
    }
}

#[derive(Debug, PartialEq)]
enum Status {
    Http200,
    Http400,
    Http404,
    Http405,
}

impl Status {
    fn as_str(&self) -> &str {
        match self {
            Status::Http200 => "200 OK",
            Status::Http400 => "400 Bad Request",
            Status::Http404 => "404 Not Found",
            Status::Http405 => "405 Method Not Allowed",
        }
    }
}

fn parse_to_request(stream: &mut BufReader<&TcpStream>) -> Result<Request> {
    let mut line = String::new();
    stream.read_line(&mut line)?;

    let line = line.trim_end();

    let parts: Vec<_> = line.splitn(3, ' ').collect();
    if parts.len() != 3 {
        return Err(anyhow::anyhow!("invalid request"));
    }

    let method = match parts[0] {
        "GET" => Method::Get,
        "POST" => Method::Post,
        "PUT" => Method::Put,
        "DELETE" => Method::Delete,
        _ => return Err(anyhow::anyhow!("invalid method")), // return 405
    };

    let path = parts[1].to_owned();

    let version = match parts[2] {
        s if s == "HTTP/1.1" => s.to_owned(),
        _ => return Err(anyhow::anyhow!("invalid version")),
    };

    let mut headers = HashMap::new();

    loop {
        let mut line = String::new();
        stream.read_line(&mut line)?;
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        let parts: Vec<_> = line.splitn(2, ": ").collect();
        if parts.len() != 2 {
            return Err(anyhow::anyhow!("invalid header"));
        }
        headers.insert(parts[0].to_owned(), parts[1].to_owned());
    }

    let content_length = headers
        .get(CONTENT_LENGTH)
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(0);

    if content_length > 1024 {
        return Err(anyhow::anyhow!("content too long"));
    }

    // fix dead lock when no body but content-length is set
    let body = if content_length > 0 {
        let mut buf = [0u8; 1024];
        let n = stream.read(&mut buf)?;
        buf[..min(n, content_length)]
            .iter()
            .map(|&c| c as char)
            .collect()
    } else {
        String::new()
    };

    Ok(Request {
        method,
        path,
        version,
        headers,
        body,
    })
}

fn root_handler(request: Request) -> Response {
    if request.method != Method::Get {
        return Response::new(Status::Http405);
    }
    Response::new(Status::Http200).with_body("Hello World")
}

fn echo_handler(request: Request) -> Response {
    let body = match request.method {
        Method::Post => {
            if request.path != "/echo" {
                return Response::new(Status::Http405);
            }
            request.body.as_str()
        }
        Method::Get => {
            let parts: Vec<_> = request.path.splitn(3, '/').collect();
            if parts.len() > 2 {
                parts[2]
            } else {
                ""
            }
        }
        _ => return Response::new(Status::Http405),
    };

    Response::new(Status::Http200)
        .with_body(body)
        .with_content_type_and_current_length(TEXT_PLAIN)
}

fn user_agent_handler(request: Request) -> Response {
    if request.method != Method::Get {
        return Response::new(Status::Http405);
    }

    if request.headers.get(USER_AGENT).is_none() {
        return Response::new(Status::Http400);
    };

    let body = request.headers.get(USER_AGENT).unwrap();

    Response::new(Status::Http200)
        .with_body(body.as_str())
        .with_content_type_and_current_length(TEXT_PLAIN)
}

fn handle_request(request: Request) -> Response {
    match request.path.as_str() {
        "/" => root_handler(request),
        "/user-agent" => user_agent_handler(request),
        s if s.starts_with("/echo") => echo_handler(request),
        _ => Response::new(Status::Http404),
    }
}

fn write_response(response: Response, stream: &mut BufWriter<&TcpStream>) -> Result<()> {
    stream.write_all(format!("HTTP/1.1 {}\r\n", response.status.as_str()).as_bytes())?;

    for (key, value) in response.headers {
        stream.write_all(format!("{}: {}\r\n", key, value).as_bytes())?;
    }

    stream.write_all(b"\r\n")?;
    stream.write_all(response.body.as_bytes())?;

    Ok(())
}

fn handle_connection(stream: TcpStream) {
    let mut reader = BufReader::new(&stream);
    let request = parse_to_request(&mut reader);

    let response = match request {
        Ok(request) => {
            println!("{}", request);
            handle_request(request)
        }
        Err(_) => Response::new(Status::Http400),
    };
    let mut writer = BufWriter::new(&stream);
    write_response(response, &mut writer).unwrap();
}

fn main() {
    let listener = TcpListener::bind("127.0.0.1:4221").unwrap();

    println!("listening started, ready to accept");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                println!("accepted new connection");
                handle_connection(stream);
            }
            Err(e) => {
                println!("error: {}", e);
            }
        }
    }
}

#[cfg(test)]
impl Request {
    fn new(method: Method, path: &str) -> Self {
        Self {
            method,
            path: path.to_owned(),
            version: "HTTP/1.1".to_owned(),
            headers: HashMap::new(),
            body: String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_root() {
        let req = Request::new(Method::Get, "/");
        let res = root_handler(req);
        assert_eq!(res.status, Status::Http200);

        let req = Request::new(Method::Post, "/");
        let res = root_handler(req);
        assert_eq!(res.status, Status::Http405);
    }

    #[test]
    fn test_echo() {
        let req = Request::new(Method::Get, "/echo");
        let res = echo_handler(req);
        assert_eq!(res.status, Status::Http200);
        assert_eq!(res.body, "");

        let req = Request::new(Method::Get, "/echo/abc");
        let res = echo_handler(req);
        assert_eq!(res.status, Status::Http200);
        assert_eq!(res.body, "abc");

        let req = Request::new(Method::Post, "/echo");
        let res = echo_handler(req);
        assert_eq!(res.status, Status::Http200);
        assert_eq!(res.body, "");

        let mut req = Request::new(Method::Post, "/echo");
        req.body = "abc".to_owned(); // todo: add with_body to Request, maybe a trait?
        let res = echo_handler(req);
        assert_eq!(res.status, Status::Http200);
        assert_eq!(res.body, "abc");

        let req = Request::new(Method::Post, "/echo/abc");
        let res = echo_handler(req);
        assert_eq!(res.status, Status::Http405);

        let req = Request::new(Method::Put, "/echo");
        let res = echo_handler(req);
        assert_eq!(res.status, Status::Http405);
    }

    #[test]
    fn test_user_agent() {
        let req = Request::new(Method::Get, "/user-agent");
        let res = user_agent_handler(req);
        assert_eq!(res.status, Status::Http400);

        let mut req = Request::new(Method::Get, "/user-agent");
        req.headers
            .insert(USER_AGENT.to_owned(), "curl/7.64.1".to_owned()); // todo: add with_header
        let res = user_agent_handler(req);
        assert_eq!(res.status, Status::Http200);
        assert_eq!(res.body, "curl/7.64.1");

        let req = Request::new(Method::Post, "/user-agent");
        let res = user_agent_handler(req);
        assert_eq!(res.status, Status::Http405);
    }
}
