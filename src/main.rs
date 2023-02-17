use std::collections::HashMap;
use std::str;
use std::sync::{Arc, RwLock};
use std::time::SystemTime;

use anyhow::Result;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

const NULL_BULK_STRING: &str = "$-1\r\n";
const OK_BULK_STRING: &str = "+OK\r\n";
const PONG_BULK_STRING: &str = "+PONG\r\n";


struct State {
    value: String,
    expiration: Option<u128>,
}

type StateMap = Arc<RwLock<HashMap<String, State>>>;


#[tokio::main]
async fn main() -> Result<()> {
    let data_store: StateMap = Arc::new(RwLock::new(HashMap::new()));

    let listener = TcpListener::bind("127.0.0.1:6379").await?;

    loop {
        match listener.accept().await {
            Ok((socket, _)) => {
                let store = Arc::clone(&data_store);

                tokio::spawn(async move {
                    process(socket, &store).await;
                });
            }
            Err(err) => {
                println!("error: {err}");
            }
       };
    }
}

async fn process(mut socket: TcpStream, store: &StateMap) {
    let mut buf = [0; 512];
    loop {
        let bytes_read = socket.read(&mut buf).await.unwrap();
        let line = parse_stream(&buf, bytes_read);

        if bytes_read == 0 {
            break;
        }

        println!("{buf:?}");
        println!("{line:?}");

        match line[2].to_lowercase().as_str() {
            "echo" => {
                let echo = line[4].to_string();
                let res = format!("+{echo}\r\n");
                socket.write_all(res.as_bytes()).await.unwrap();
            }
            "ping" => {
                let res = PONG_BULK_STRING;
                socket.write_all(res.as_bytes()).await.unwrap();
            }
            "get" => {
                let key = line[4].to_string();
                let mut key_has_expired = false;
                let res = match store.read().unwrap().get(key.as_str()) {
                    None => NULL_BULK_STRING.to_string(),
                    Some(s) => {
                        let len = &s.value.len();
                        let val = &s.value;
                        let ret = format!("${len}\r\n{val}\r\n");
                        match &s.expiration {
                            None => ret,
                            Some(exp) => {
                                let now = SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap()
                                    .as_millis();
                                if exp < &now {
                                    key_has_expired = true;
                                    NULL_BULK_STRING.to_string()
                                } else {
                                    ret
                                }
                            }
                        }
                    }
                };
                socket.write_all(res.as_bytes()).await.unwrap();

                if key_has_expired {
                    store.write().unwrap().remove(key.as_str());
                }
            }
            "set" => {
                let key = line[4].to_string();
                let value = line[6].to_string();

                let expiry = line
                    .get(8)
                    .filter(|s| s.to_string() == "px")
                    .and_then(|_| line.get(10))
                    .map(|s| {
                        SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_millis()
                            + s.parse::<u128>().unwrap()
                    });
                store.write().unwrap().insert(
                    key,
                    State {
                        value,
                        expiration: expiry,
                    },
                );

                // no error
                let res = OK_BULK_STRING;
                socket.write_all(res.as_bytes()).await.unwrap();
            }
            _ => {
                let res = "-Error Unknown command\r\n";
                socket.write_all(res.as_bytes()).await.unwrap();
            }
        }
    }
}

fn parse_stream(line: &[u8], l: usize) -> Vec<&str> {
    let mut lines =  Vec::new();
    let mut start = 0;
    let end = l - 1;
    for i in 0..end {
        if line[i] == b'\r' && line[i + 1] == b'\n' {
            lines.push(str::from_utf8(&line[start..i]).unwrap());
            start = i + 2;
        }
    }
lines
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_parse_stream_ping() {
      assert_eq!(parse_stream(b"*1\r\n$4\r\nping\r\n", 14), ["*1", "$4", "ping"]);
  }

  #[test]
  fn test_parse_stream_echo() {
      assert_eq!(parse_stream(b"*2\r\n$4\r\necho\r\n$5\r\nhello world\r\n", 31), ["*2", "$4", "echo", "$5", "hello world"]);
  }
}