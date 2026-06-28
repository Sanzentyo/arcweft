use arcweft_bundle::release::ReleaseFetchPolicy;
use std::{
    io::{ErrorKind, Read, Write},
    net::{TcpStream, ToSocketAddrs},
    time::Duration,
};

pub(super) fn network_policy_rejection(
    fetch_policy: &ReleaseFetchPolicy,
    scheme: &str,
) -> Option<String> {
    let policy = &fetch_policy.network_policy;
    if policy.require_https && scheme == "http" {
        return Some("network policy requires HTTPS; plain HTTP mirror is not allowed".to_owned());
    }
    if let Some(proxy_profile) = &policy.proxy_profile {
        return Some(format!(
            "network policy requires proxy profile `{proxy_profile}`, but this cache adapter has no proxy provider"
        ));
    }
    if let Some(auth_profile) = &policy.auth_profile {
        return Some(format!(
            "network policy requires auth profile `{auth_profile}`, but this cache adapter has no credential provider"
        ));
    }
    if let Some(client_profile) = &policy.client_profile {
        return Some(format!(
            "network policy requires client profile `{client_profile}`, but this cache adapter is using the default client"
        ));
    }
    None
}

pub(super) fn read_http_mirror(
    uri: &str,
    fetch_policy: &ReleaseFetchPolicy,
) -> Result<Vec<u8>, String> {
    let url = HttpMirrorUrl::parse(uri)?;
    let mut stream = connect_http(&url, fetch_policy.cancel_after_millis)?;
    let host_header = url.host_header();
    let user_agent = &fetch_policy.network_policy.user_agent;
    write!(
        stream,
        "GET {} HTTP/1.1\r\nHost: {}\r\nUser-Agent: {}\r\nAccept: application/octet-stream\r\nConnection: close\r\n\r\n",
        url.target, host_header, user_agent
    )
    .map_err(|error| format!("failed to write HTTP request: {error}"))?;
    stream
        .flush()
        .map_err(|error| format!("failed to flush HTTP request: {error}"))?;
    let response = read_http_response(&mut stream, fetch_policy.candidate_byte_budget)?;
    decode_http_response(&response, fetch_policy.candidate_byte_budget)
}

pub(super) fn read_https_mirror(
    uri: &str,
    fetch_policy: &ReleaseFetchPolicy,
) -> Result<Vec<u8>, String> {
    if !uri.starts_with("https://") {
        return Err("HTTPS mirror URI must start with https://".to_owned());
    }
    let mut config = ureq::Agent::config_builder();
    if let Some(cancel_after_millis) = fetch_policy.cancel_after_millis {
        let timeout = Duration::from_millis(cancel_after_millis);
        config = config
            .timeout_global(Some(timeout))
            .timeout_connect(Some(timeout));
    }
    let agent = ureq::Agent::new_with_config(config.build());
    let mut response = agent
        .get(uri)
        .header("User-Agent", &fetch_policy.network_policy.user_agent)
        .header("Accept", "application/octet-stream")
        .call()
        .map_err(|error| format!("failed to fetch HTTPS mirror: {error}"))?;
    response
        .body_mut()
        .with_config()
        .limit(fetch_policy.candidate_byte_budget)
        .read_to_vec()
        .map_err(|error| format!("failed to read HTTPS mirror response: {error}"))
}

fn connect_http(
    url: &HttpMirrorUrl,
    cancel_after_millis: Option<u64>,
) -> Result<TcpStream, String> {
    let addrs = (url.host.as_str(), url.port)
        .to_socket_addrs()
        .map_err(|error| format!("failed to resolve HTTP mirror host: {error}"))?;
    let timeout = cancel_after_millis.map(Duration::from_millis);
    let mut last_error = None;
    for addr in addrs {
        let stream = match timeout {
            Some(timeout) => TcpStream::connect_timeout(&addr, timeout),
            None => TcpStream::connect(addr),
        };
        match stream {
            Ok(stream) => {
                stream
                    .set_read_timeout(timeout)
                    .map_err(|error| format!("failed to set HTTP read timeout: {error}"))?;
                stream
                    .set_write_timeout(timeout)
                    .map_err(|error| format!("failed to set HTTP write timeout: {error}"))?;
                return Ok(stream);
            }
            Err(error) => last_error = Some(error),
        }
    }
    Err(last_error.map_or_else(
        || "HTTP mirror host resolved to no socket addresses".to_owned(),
        |error| format!("failed to connect HTTP mirror: {error}"),
    ))
}

fn read_http_response(stream: &mut TcpStream, body_budget: u64) -> Result<Vec<u8>, String> {
    const HEADER_BUDGET: u64 = 16 * 1024;
    let response_budget = body_budget.saturating_add(HEADER_BUDGET);
    let mut response = Vec::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let read = match stream.read(&mut buffer) {
            Ok(read) => read,
            Err(error) if error.kind() == ErrorKind::ConnectionReset && !response.is_empty() => {
                break;
            }
            Err(error) => return Err(format!("failed to read HTTP response: {error}")),
        };
        if read == 0 {
            break;
        }
        response.extend_from_slice(&buffer[..read]);
        if u64::try_from(response.len()).unwrap_or(u64::MAX) > response_budget {
            return Err(format!(
                "HTTP response exceeds candidate byte budget: response > {body_budget} byte(s) plus header allowance"
            ));
        }
    }
    Ok(response)
}

fn decode_http_response(response: &[u8], body_budget: u64) -> Result<Vec<u8>, String> {
    let header_end = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| "HTTP response is missing header terminator".to_owned())?;
    let header = std::str::from_utf8(&response[..header_end])
        .map_err(|error| format!("HTTP response headers are not UTF-8: {error}"))?;
    let status = header.lines().next().unwrap_or_default();
    if !status.starts_with("HTTP/1.1 200 ") && !status.starts_with("HTTP/1.0 200 ") {
        return Err(format!("HTTP mirror returned non-200 status: {status}"));
    }
    if header.lines().skip(1).any(|line| {
        line.split_once(':').is_some_and(|(name, value)| {
            name.eq_ignore_ascii_case("transfer-encoding")
                && value.to_ascii_lowercase().contains("chunked")
        })
    }) {
        return Err("HTTP chunked transfer encoding is not supported by this adapter".to_owned());
    }
    let body = &response[header_end + 4..];
    if u64::try_from(body.len()).unwrap_or(u64::MAX) > body_budget {
        return Err(format!(
            "candidate byte budget exceeded: {} byte(s) > {} byte(s)",
            body.len(),
            body_budget
        ));
    }
    Ok(body.to_vec())
}

struct HttpMirrorUrl {
    host: String,
    port: u16,
    target: String,
}

impl HttpMirrorUrl {
    fn parse(uri: &str) -> Result<Self, String> {
        let rest = uri
            .strip_prefix("http://")
            .ok_or_else(|| "HTTP mirror URI must start with http://".to_owned())?;
        let (authority, path) = rest
            .split_once('/')
            .map_or((rest, "/"), |(authority, path)| (authority, path));
        if authority.is_empty() {
            return Err("HTTP mirror URI is missing a host".to_owned());
        }
        if authority.contains('@') {
            return Err("HTTP mirror URI userinfo is not supported".to_owned());
        }
        let (host, port) = parse_http_authority(authority)?;
        if host.is_empty() {
            return Err("HTTP mirror URI is missing a host".to_owned());
        }
        Ok(Self {
            host,
            port,
            target: if path == "/" {
                "/".to_owned()
            } else {
                format!("/{path}")
            },
        })
    }

    fn host_header(&self) -> String {
        if self.port == 80 {
            self.host.clone()
        } else if self.host.contains(':') {
            format!("[{}]:{}", self.host, self.port)
        } else {
            format!("{}:{}", self.host, self.port)
        }
    }
}

fn parse_http_authority(authority: &str) -> Result<(String, u16), String> {
    if let Some(rest) = authority.strip_prefix('[') {
        let (host, after_host) = rest
            .split_once(']')
            .ok_or_else(|| "HTTP IPv6 host is missing closing bracket".to_owned())?;
        let port = if after_host.is_empty() {
            80
        } else {
            after_host
                .strip_prefix(':')
                .ok_or_else(|| "HTTP IPv6 host has invalid port separator".to_owned())?
                .parse::<u16>()
                .map_err(|error| format!("HTTP mirror port is invalid: {error}"))?
        };
        return Ok((host.to_owned(), port));
    }
    match authority.rsplit_once(':') {
        Some((host, port)) if !host.contains(':') => Ok((
            host.to_owned(),
            port.parse::<u16>()
                .map_err(|error| format!("HTTP mirror port is invalid: {error}"))?,
        )),
        _ => Ok((authority.to_owned(), 80)),
    }
}
