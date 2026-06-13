use std::{
    collections::HashMap,
    sync::Arc,
};

use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
    sync::Mutex,
};

type Db = Arc<Mutex<HashMap<String, String>>>;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind("127.0.0.1:6380").await?;

    println!("Mini Redis running on 127.0.0.1:6379");

    let db: Db = Arc::new(Mutex::new(HashMap::new()));

    loop {
        let (socket, addr) = listener.accept().await?;

        println!("Client connected: {}", addr);

        let db_clone = db.clone();

        tokio::spawn(async move {
            if let Err(e) = handle_client(socket, db_clone).await {
                eprintln!("Error: {}", e);
            }
        });
    }
}

async fn handle_client(
    socket: TcpStream,
    db: Db,
) -> Result<(), Box<dyn std::error::Error>> {
    let (reader, mut writer) = socket.into_split();

    let reader = BufReader::new(reader);
    let mut lines = reader.lines();

    while let Some(line) = lines.next_line().await? {
        let parts: Vec<&str> = line.trim().split_whitespace().collect();

        if parts.is_empty() {
            continue;
        }

        let command = parts[0].to_uppercase();

        match command.as_str() {
            "SET" => {
                if parts.len() < 3 {
                    writer
                        .write_all(b"ERR Usage: SET key value\n")
                        .await?;
                    continue;
                }

                let key = parts[1].to_string();
                let value = parts[2..].join(" ");

                let mut store = db.lock().await;
                store.insert(key, value);

                writer.write_all(b"OK\n").await?;
            }

            "GET" => {
                if parts.len() != 2 {
                    writer
                        .write_all(b"ERR Usage: GET key\n")
                        .await?;
                    continue;
                }

                let key = parts[1];

                let store = db.lock().await;

                match store.get(key) {
                    Some(value) => {
                        writer
                            .write_all(format!("{}\n", value).as_bytes())
                            .await?;
                    }
                    None => {
                        writer.write_all(b"NULL\n").await?;
                    }
                }
            }

            "DEL" => {
                if parts.len() != 2 {
                    writer
                        .write_all(b"ERR Usage: DEL key\n")
                        .await?;
                    continue;
                }

                let key = parts[1];

                let mut store = db.lock().await;

                if store.remove(key).is_some() {
                    writer.write_all(b"1\n").await?;
                } else {
                    writer.write_all(b"0\n").await?;
                }
            }

            "KEYS" => {
                let store = db.lock().await;

                let keys = store
                    .keys()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ");

                writer
                    .write_all(format!("{}\n", keys).as_bytes())
                    .await?;
            }

            "PING" => {
                writer.write_all(b"PONG\n").await?;
            }

            "EXIT" => {
                writer.write_all(b"BYE\n").await?;
                break;
            }

            _ => {
                writer
                    .write_all(b"ERR Unknown command\n")
                    .await?;
            }
        }
    }

    Ok(())
}