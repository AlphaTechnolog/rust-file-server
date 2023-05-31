use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::thread;
use std::fs;

struct Headers {
    content_type: String,
    content_length: usize,
}

impl Headers {
    fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(format!("Content-Type: {}\r\n", self.content_type).as_bytes());
        buf.extend_from_slice(format!("Content-Length: {}\r\n", self.content_length).as_bytes());
        buf.extend_from_slice("\r\n".as_bytes());
        buf
    }
}

struct Response<'a> {
    headers: &'a Headers,
    statuscode: &'a str,
    message: String
}

impl<'a> Response<'a> {
    fn new(statuscode: &'static str, message: String, headers: &'a Headers) -> Self {
        Self {
            headers,
            statuscode,
            message
        }
    }

    fn as_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(format!("HTTP/1.1 {}\r\n", self.statuscode).as_bytes());
        buf.extend_from_slice(&self.headers.to_bytes());
        buf.extend_from_slice(self.message.as_bytes());
        buf
    }
}

fn main() {
    let listener = TcpListener::bind("127.0.0.1:8080").unwrap();
    println!("Server is listening on port 8080");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                thread::spawn(move || {
                    handle_client(stream);
                });
            }
            Err(err) => {
                println!("Error: {}", err);
            }
        }
    }
}

fn is_dir(path: &Path) -> bool {
    if let Ok(metadata) = fs::metadata(path) {
        return !metadata.is_file();
    }

    false
}

fn files_listing(dirname: &str) -> Option<String> {
    let path = Path::new(dirname);
    if let Ok(entries) = fs::read_dir(path) {
        let mut files = String::new();
        for entry in entries {
            if let Ok(entry) = entry {
                let file_name = entry.file_name();
                if let Some(file_name) = file_name.to_str() {
                    let mut file_path = PathBuf::new();

                    file_path.push(path);
                    file_path.push(file_name);

                    files += &[
                        if is_dir(file_path.as_path()) { "DIR  " }
                        else { "FILE " },
                        file_name
                    ].concat();

                    files += "\n";
                }
            }
        }

        return Some(files);
    }

    None
}

fn handle_client(mut stream: TcpStream) {
    let mut buffer = [0; 1024];
    stream.read(&mut buffer).unwrap();
    let request = String::from_utf8_lossy(&buffer[..]);

    debug_printing(&request);

    let filename: Option<String> = filename_by_path(get_path(&request));

    if filename.is_none() {
        return match files_listing("./") {
            Some(content) => {
                let headers = Headers {
                    content_type: "text/plain".to_string(),
                    content_length: content.len(),
                };

                let response = Response::new("200 OK", content.to_string(), &headers);
                stream.write_all(&response.as_bytes()).unwrap();
            },
            None => {
                let headers = Headers {
                    content_type: "text/plain".to_string(),
                    content_length: 123,
                };

                let response = Response::new("500 Internal server error", "Cannot list directory".to_string(), &headers);
                stream.write_all(&response.as_bytes()).unwrap();
            },
        };
    }

    if let Some(v) = filename {
        let path = Path::new(&v);
        let mimetype = get_mimetype(&path);
        if let Ok(metadata) = fs::metadata(path) {
            if !metadata.is_file() {
                return match files_listing(&v) {
                    Some(content) => {
                        let headers = Headers {
                            content_type: "text/plain".to_string(),
                            content_length: content.len(),
                        };
        
                        let response = Response::new("200 OK", content.to_string(), &headers);
                        stream.write_all(&response.as_bytes()).unwrap();
                    },
                    None => {
                        let headers = Headers {
                            content_type: "text/plain".to_string(),
                            content_length: 123,
                        };
        
                        let response = Response::new("500 Internal server error", "Cannot list directory".to_string(), &headers);
                        stream.write_all(&response.as_bytes()).unwrap();
                    },
                };
            } else if let Some(content) = get_content(path) {
                let headers = Headers {
                    content_type: mimetype,
                    content_length: content.len(),
                };

                let response = Response::new("200 OK", content.to_string(), &headers);
                stream.write_all(&response.as_bytes()).unwrap();
                return;
            } else {
                let headers = Headers {
                    content_type: "text/plain".to_string(),
                    content_length: 512,
                };

                let response = Response::new("500 Internal server error", "Cannot retrieve file content".to_string(), &headers);
                stream.write_all(&response.as_bytes()).unwrap();
                return;
            }
        } else {
            let headers = Headers {
                content_type: "text/plain".to_string(),
                content_length: 512,
            };

            let response = Response::new("404 Not Found", "Cannot retrieve file content".to_string(), &headers);
            stream.write_all(&response.as_bytes()).unwrap();
            return;
        }
    }

    stream.flush().unwrap();
}

fn get_content(path: &Path) -> Option<String> {
    if let Ok(file_contents) = fs::read_to_string(path) {
        return Some(file_contents.to_string());
    }

    None
}

fn get_mimetype(path: &Path) -> String {
    let filename: &str = path.file_name().unwrap().to_str().unwrap();
    let parts: Vec<&str> = filename.split('.').collect();

    let res = match parts.last() {
        Some(v) => {
            match *v {
                "png" => mime::IMAGE_PNG,
                "jpg" => mime::IMAGE_JPEG,
                "json" => mime::APPLICATION_JSON,
                "js" => mime::APPLICATION_JAVASCRIPT,
                &_ => mime::TEXT_PLAIN,
            }
        },
        None => mime::TEXT_PLAIN
    };

    res.to_string()
}

fn filename_by_path(url: &str) -> Option<String> {
    let rest_url = url.trim_start_matches('/');
    let path = Path::new(rest_url);
    let mut new_path_buf = PathBuf::new();

    new_path_buf.push(".");
    new_path_buf.push(path);

    let new_path = new_path_buf.as_path();

    let filename: Option<String> = match new_path.to_str() {
        Some(filename_str) => Some(filename_str.to_owned()),
        None => None
    };

    if let Some(v) = filename {
        if v == "./".to_string() {
            return None;
        }

        return Some(v);
    }

    None
}

fn debug_printing(request: &str) {
    println!("{}", request.lines().next().unwrap_or(""));
}

fn get_path(request: &str) -> &str {
    &request.split_whitespace().nth(1)
        .expect("Can't extract the route path of the request!")
}