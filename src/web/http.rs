use std::collections::HashMap;

/// HTTP 请求方法
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
    Put,
    Delete,
    Options,
    Other(String),
}

impl Method {
    pub fn from_str(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "GET" => Method::Get,
            "POST" => Method::Post,
            "PUT" => Method::Put,
            "DELETE" => Method::Delete,
            "OPTIONS" => Method::Options,
            other => Method::Other(other.to_string()),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Method::Get => "GET",
            Method::Post => "POST",
            Method::Put => "PUT",
            Method::Delete => "DELETE",
            Method::Options => "OPTIONS",
            Method::Other(s) => s.as_str(),
        }
    }
}

/// HTTP 请求结构
#[derive(Debug, Clone)]
pub struct HttpRequest {
    pub method: Method,
    pub path: String,
    pub raw_query: String,
    pub query_params: HashMap<String, String>,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

impl HttpRequest {
    /// 从原始字节流解析 HTTP/1.1 请求
    pub fn parse(raw: &[u8]) -> Result<Self, String> {
        let text = match std::str::from_utf8(raw) {
            Ok(s) => s,
            Err(_) => return Err("Invalid UTF-8 in HTTP header".to_string()),
        };

        let mut lines = text.lines();
        let request_line = lines.next().ok_or("Empty HTTP request")?;
        let mut parts = request_line.split_whitespace();

        let method_str = parts.next().ok_or("Missing HTTP method")?;
        let full_path = parts.next().ok_or("Missing HTTP path")?;
        let _version = parts.next().unwrap_or("HTTP/1.1");

        let method = Method::from_str(method_str);

        // 解析 path 与 query
        let (path, raw_query) = if let Some(idx) = full_path.find('?') {
            (&full_path[..idx], &full_path[idx + 1..])
        } else {
            (full_path, "")
        };

        let mut query_params = HashMap::new();
        if !raw_query.is_empty() {
            for pair in raw_query.split('&') {
                if pair.is_empty() {
                    continue;
                }
                let mut kv = pair.splitn(2, '=');
                let k = kv.next().unwrap_or("").to_string();
                let v = kv.next().unwrap_or("").to_string();
                query_params.insert(k, v);
            }
        }

        let mut headers = HashMap::new();
        for line in lines {
            if line.is_empty() {
                break;
            }
            if let Some(idx) = line.find(':') {
                let key = line[..idx].trim().to_lowercase();
                let value = line[idx + 1..].trim().to_string();
                headers.insert(key, value);
            }
        }

        // 寻找 body 分隔符 \r\n\r\n 或 \n\n
        let body_offset = if let Some(pos) = raw.windows(4).position(|w| w == b"\r\n\r\n") {
            pos + 4
        } else if let Some(pos) = raw.windows(2).position(|w| w == b"\n\n") {
            pos + 2
        } else {
            raw.len()
        };

        let body = if body_offset < raw.len() {
            raw[body_offset..].to_vec()
        } else {
            Vec::new()
        };

        Ok(Self {
            method,
            path: path.to_string(),
            raw_query: raw_query.to_string(),
            query_params,
            headers,
            body,
        })
    }

    /// 获取 Body 转换为 UTF-8 字符串
    pub fn body_as_str(&self) -> Result<&str, std::str::Utf8Error> {
        std::str::from_utf8(&self.body)
    }

    /// 解析 JSON Body
    pub fn json<T: serde::de::DeserializeOwned>(&self) -> Result<T, String> {
        let s = self.body_as_str().map_err(|e| e.to_string())?;
        serde_json::from_str(s).map_err(|e| e.to_string())
    }
}

/// HTTP 状态码
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatusCode(pub u16, pub &'static str);

impl StatusCode {
    pub const OK: StatusCode = StatusCode(200, "OK");
    pub const CREATED: StatusCode = StatusCode(201, "Created");
    pub const NO_CONTENT: StatusCode = StatusCode(204, "No Content");
    pub const BAD_REQUEST: StatusCode = StatusCode(400, "Bad Request");
    pub const UNAUTHORIZED: StatusCode = StatusCode(401, "Unauthorized");
    pub const FORBIDDEN: StatusCode = StatusCode(403, "Forbidden");
    pub const NOT_FOUND: StatusCode = StatusCode(404, "Not Found");
    pub const METHOD_NOT_ALLOWED: StatusCode = StatusCode(405, "Method Not Allowed");
    pub const CONFLICT: StatusCode = StatusCode(409, "Conflict");
    pub const INTERNAL_SERVER_ERROR: StatusCode = StatusCode(500, "Internal Server Error");
}

/// HTTP 响应结构
#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status: StatusCode,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

impl HttpResponse {
    pub fn new(status: StatusCode) -> Self {
        let mut headers = HashMap::new();
        headers.insert("Server".to_string(), "AgentSteward-Web/0.3.2".to_string());
        headers.insert("Access-Control-Allow-Origin".to_string(), "*".to_string());
        headers.insert("Access-Control-Allow-Methods".to_string(), "GET, POST, OPTIONS".to_string());
        headers.insert("Access-Control-Allow-Headers".to_string(), "Content-Type, Authorization".to_string());
        Self {
            status,
            headers,
            body: Vec::new(),
        }
    }

    pub fn ok() -> Self {
        Self::new(StatusCode::OK)
    }

    pub fn not_found(msg: &str) -> Self {
        Self::json(StatusCode::NOT_FOUND, &serde_json::json!({
            "code": 404,
            "error": "Not Found",
            "message": msg
        }))
    }

    pub fn bad_request(msg: &str) -> Self {
        Self::json(StatusCode::BAD_REQUEST, &serde_json::json!({
            "code": 400,
            "error": "Bad Request",
            "message": msg
        }))
    }

    pub fn server_error(msg: &str) -> Self {
        Self::json(StatusCode::INTERNAL_SERVER_ERROR, &serde_json::json!({
            "code": 500,
            "error": "Internal Server Error",
            "message": msg
        }))
    }

    pub fn header(mut self, key: &str, value: &str) -> Self {
        self.headers.insert(key.to_string(), value.to_string());
        self
    }

    pub fn body_bytes(mut self, bytes: Vec<u8>) -> Self {
        self.headers.insert("Content-Length".to_string(), bytes.len().to_string());
        self.body = bytes;
        self
    }

    pub fn body_str(self, text: &str) -> Self {
        self.body_bytes(text.as_bytes().to_vec())
    }

    pub fn json<T: serde::Serialize>(status: StatusCode, value: &T) -> Self {
        match serde_json::to_vec(value) {
            Ok(bytes) => Self::new(status)
                .header("Content-Type", "application/json; charset=utf-8")
                .body_bytes(bytes),
            Err(e) => Self::new(StatusCode::INTERNAL_SERVER_ERROR)
                .header("Content-Type", "application/json; charset=utf-8")
                .body_str(&format!(r#"{{"code":500,"error":"JSON serialization error: {}"}}"#, e)),
        }
    }

    pub fn html(html: &str) -> Self {
        Self::new(StatusCode::OK)
            .header("Content-Type", "text/html; charset=utf-8")
            .body_str(html)
    }

    pub fn css(css: &str) -> Self {
        Self::new(StatusCode::OK)
            .header("Content-Type", "text/css; charset=utf-8")
            .body_str(css)
    }

    pub fn js(js: &str) -> Self {
        Self::new(StatusCode::OK)
            .header("Content-Type", "application/javascript; charset=utf-8")
            .body_str(js)
    }

    /// 序列化为标准的 HTTP/1.1 响应报文
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut resp = Vec::new();
        let status_line = format!("HTTP/1.1 {} {}\r\n", self.status.0, self.status.1);
        resp.extend_from_slice(status_line.as_bytes());

        for (k, v) in &self.headers {
            resp.extend_from_slice(format!("{}: {}\r\n", k, v).as_bytes());
        }
        resp.extend_from_slice(b"\r\n");
        resp.extend_from_slice(&self.body);
        resp
    }
}
