use std::collections::HashMap;
use std::str;
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime};
use tokio::{task, time};

use anyhow::Result;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

pub mod db;
use crate::db::{State, StateMap};

const NULL_BULK_STRING: &str = "$-1\r\n";
const OK_BULK_STRING: &str = "+OK\r\n";
const PONG_BULK_STRING: &str = "+PONG\r\n";

#[tokio::main]
async fn main() -> Result<()> {
    let data_store: StateMap = Arc::new(RwLock::new(HashMap::new()));

    // implement key expiry algorithm described in 'How Redis expires keys': https://redis.io/commands/expire/
    let _key_expiry_task = task::spawn(async {
        let mut interval = time::interval(Duration::from_millis(100));

        loop {
            interval.tick().await;
            // run_expiry().await;
        }
    });

    let listener = TcpListener::bind("127.0.0.1:6379").await?;

    loop {
        match listener.accept().await {
            Ok((socket, _)) => {
                let store = Arc::clone(&data_store);

                tokio::spawn(async move {
                    handle_client(socket, &store).await;
                });
            }
            Err(err) => {
                println!("error: {err}");
            }
        };
    }

    // shutting down tokip:
    // https://tokio.rs/tokio/topics/shutdown
    // should be done on a clean shutdown
    //key_expiry_task.await;
}

async fn handle_client(mut socket: TcpStream, store: &StateMap) {
    let mut buf = [0; 512];
    loop {
        // TODO handle input longer than 512 bytes
        let bytes_read = socket.read(&mut buf).await.unwrap();
        let line = parse_message(&buf, bytes_read);

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
            "shutdown" => {
                // send the shutdown message to initiate a clean shutdown
            }
            _ => {
                let res = "-Error Unknown command\r\n";
                socket.write_all(res.as_bytes()).await.unwrap();
            }
        }
    }
}

fn parse_message(line: &[u8], length: usize) -> Vec<&str> {
    let mut lines = Vec::new();
    let mut start = 0;
    let end = length - 1;

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
    fn test_parse_message_ping() {
        assert_eq!(
            parse_message(b"*1\r\n$4\r\nping\r\n", 14),
            ["*1", "$4", "ping"]
        );
    }

    #[test]
    fn test_parse_message_echo() {
        assert_eq!(
            parse_message(b"*2\r\n$4\r\necho\r\n$5\r\nhello world\r\n", 31),
            ["*2", "$4", "echo", "$5", "hello world"]
        );
    }

    #[test]
    fn test_parse_message_get() {
        assert_eq!(
            parse_message(b"*2\r\n$3\r\nget\r\n$3\r\nkey\r\n", 22),
            ["*2", "$3", "get", "$3", "key"]
        );
    }
}
