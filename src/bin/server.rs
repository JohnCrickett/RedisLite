use anyhow::Result;
use bytes::Bytes;
use std::str;
use std::time::Duration;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

use redis_lite::db::{Db, DbHandle};

static NULL_BULK_STRING: Bytes = Bytes::from_static(b"$-1\r\n");
static OK_BULK_STRING: Bytes = Bytes::from_static(b"+OK\r\n");
static PONG_BULK_STRING: Bytes = Bytes::from_static(b"+PONG\r\n");

#[tokio::main]
async fn main() -> Result<()> {
    let data_store = DbHandle::new();

    let listener = TcpListener::bind("127.0.0.1:6379").await?;

    loop {
        match listener.accept().await {
            Ok((socket, _)) => {
                let db = data_store.db();

                tokio::spawn(async move {
                    handle_client(socket, db).await;
                });
            }
            Err(err) => {
                println!("error: {err}");
            }
        };
    }
}

async fn handle_client(mut socket: TcpStream, store: Db) {
    let mut buf = [0; 512];
    loop {
        // TODO handle input longer than 512 bytes
        let bytes_read = socket.read(&mut buf).await.unwrap();
        let line = parse_message(&buf, bytes_read);

        if bytes_read == 0 {
            break;
        }

        match line[2].to_lowercase().as_str() {
            "echo" => {
                let echo = line[4].to_string();
                let res = Bytes::from(format!("+{echo}\r\n"));
                socket.write_all(&res).await.unwrap();
            }
            "ping" => {
                let res = PONG_BULK_STRING.clone();
                socket.write_all(&res).await.unwrap();
            }
            "get" => {
                let key = line[4].to_string();
                let val = match store.get(&key) {
                    None => NULL_BULK_STRING.clone(),
                    Some(d) => d,
                };

                let len = &val.len();
                let v = val.to_vec();
                let value = str::from_utf8(&v).unwrap();
                let ret = format!("${len}\r\n{value}\r\n");
                socket.write_all(ret.as_bytes()).await.unwrap();
            }
            "set" => {
                let key = line[4].to_string();
                let value = line[6].to_string();
                let expiry = line
                    .get(8)
                    .filter(|s| s.to_string() == "px")
                    .and_then(|_| line.get(10))
                    .map(|s| Duration::from_millis(s.parse::<u64>().unwrap()));
                store.set(key, value.into(), expiry);

                // no error
                let res = OK_BULK_STRING.clone();
                socket.write_all(&res).await.unwrap();
            }
            _ => {
                let res = Bytes::from("-Error Unknown command\r\n");
                socket.write_all(&res).await.unwrap();
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
