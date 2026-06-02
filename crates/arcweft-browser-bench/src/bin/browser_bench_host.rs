use std::{
    env,
    ffi::OsString,
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

const WASM_BINDGEN_VERSION: &str = "0.2.122";
const OUT_DIR: &str = "target/arcweft-browser-webgpu";
const WASM_INPUT: &str = "target/wasm32-unknown-unknown/release/arcweft_browser_bench.wasm";

fn main() {
    if let Err(error) = run() {
        eprintln!("browser bench host error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args_os();
    let _bin = args.next();
    match args
        .next()
        .and_then(|arg| arg.into_string().ok())
        .as_deref()
    {
        Some("build") => build(),
        Some("serve") => {
            let args = args.collect::<Vec<_>>();
            serve(&args)
        }
        Some("build-and-serve") => {
            build()?;
            let args = args.collect::<Vec<_>>();
            serve(&args)
        }
        Some(other) => Err(format!(
            "unknown command `{other}`; expected build, serve, or build-and-serve"
        )),
        None => Err("missing command; expected build, serve, or build-and-serve".to_owned()),
    }
}

fn build() -> Result<(), String> {
    run_command(
        Command::new("cargo")
            .args([
                "build",
                "-p",
                "arcweft-browser-bench",
                "--release",
                "--target",
                "wasm32-unknown-unknown",
                "--all-features",
            ])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit()),
    )?;
    ensure_wasm_bindgen()?;
    fs::create_dir_all(OUT_DIR).map_err(|error| error.to_string())?;
    run_command(
        Command::new(wasm_bindgen_path())
            .args([
                "--target",
                "web",
                "--out-dir",
                OUT_DIR,
                "--out-name",
                "arcweft_browser_webgpu",
                WASM_INPUT,
            ])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit()),
    )?;
    fs::write(
        Path::new(OUT_DIR).join("index.html"),
        include_str!("../../web/index.html"),
    )
    .map_err(|error| error.to_string())?;
    fs::write(
        Path::new(OUT_DIR).join("run.js"),
        include_str!("../../web/run.js"),
    )
    .map_err(|error| error.to_string())?;
    println!("browser bench bundle written to {OUT_DIR}");
    Ok(())
}

fn ensure_wasm_bindgen() -> Result<(), String> {
    if wasm_bindgen_path().exists() {
        return Ok(());
    }
    run_command(
        Command::new("cargo")
            .args([
                "install",
                "wasm-bindgen-cli",
                "--version",
                WASM_BINDGEN_VERSION,
                "--locked",
                "--root",
                "target/tools",
            ])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit()),
    )
}

fn serve(args: &[OsString]) -> Result<(), String> {
    let mut port = 8787_u16;
    let mut root = PathBuf::from(OUT_DIR);
    let mut index = 0;
    while index < args.len() {
        match args[index].to_string_lossy().as_ref() {
            "--port" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| "--port requires a value".to_owned())?
                    .to_string_lossy();
                port = value
                    .parse()
                    .map_err(|error| format!("invalid port `{value}`: {error}"))?;
            }
            "--root" => {
                index += 1;
                root = PathBuf::from(
                    args.get(index)
                        .ok_or_else(|| "--root requires a value".to_owned())?,
                );
            }
            value => return Err(format!("unknown serve option `{value}`")),
        }
        index += 1;
    }
    let listener = TcpListener::bind(("127.0.0.1", port)).map_err(|error| error.to_string())?;
    println!("serving browser bench at http://127.0.0.1:{port}/");
    for stream in listener.incoming() {
        let stream = stream.map_err(|error| error.to_string())?;
        if let Err(error) = handle_connection(stream, &root) {
            eprintln!("browser bench request error: {error}");
        }
    }
    Ok(())
}

fn handle_connection(mut stream: TcpStream, root: &Path) -> Result<(), String> {
    let mut request = [0_u8; 2048];
    let len = stream
        .read(&mut request)
        .map_err(|error| error.to_string())?;
    let request = String::from_utf8_lossy(&request[..len]);
    let raw_path = request
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap_or("/");
    let path = raw_path.split(['?', '#']).next().unwrap_or(raw_path);
    let relative = match path {
        "/" => "index.html",
        path if path.starts_with('/') && is_safe_relative_path(&path[1..]) => &path[1..],
        _ => return write_response(&mut stream, 404, "text/plain", b"not found"),
    };
    let file_path = root.join(relative);
    let Ok(body) = fs::read(&file_path) else {
        return write_response(&mut stream, 404, "text/plain", b"not found");
    };
    let content_type = content_type(file_path.extension().and_then(|value| value.to_str()));
    write_response(&mut stream, 200, content_type, &body)
}

fn write_response(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &[u8],
) -> Result<(), String> {
    let reason = if status == 200 { "OK" } else { "Not Found" };
    let headers = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCross-Origin-Opener-Policy: same-origin\r\nCross-Origin-Embedder-Policy: require-corp\r\nCache-Control: no-store\r\n\r\n",
        body.len()
    );
    stream
        .write_all(headers.as_bytes())
        .and_then(|()| stream.write_all(body))
        .map_err(|error| error.to_string())
}

fn is_safe_relative_path(path: &str) -> bool {
    !path.contains("..")
        && !path.contains('\\')
        && path
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b'/'))
}

fn content_type(extension: Option<&str>) -> &'static str {
    match extension {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("wasm") => "application/wasm",
        Some("json") => "application/json; charset=utf-8",
        _ => "application/octet-stream",
    }
}

fn wasm_bindgen_path() -> PathBuf {
    let exe = if cfg!(windows) {
        "wasm-bindgen.exe"
    } else {
        "wasm-bindgen"
    };
    Path::new("target").join("tools").join("bin").join(exe)
}

fn run_command(command: &mut Command) -> Result<(), String> {
    let status = command.status().map_err(|error| error.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("command exited with status {status}"))
    }
}
