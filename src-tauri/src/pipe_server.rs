use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};
use tokio::sync::{Mutex, Notify};

const PIPE_NAME: &str = r"\\.\pipe\LexiInputMethod";
const BUFFER_SIZE: usize = 4096;

pub struct PipeServer {
    running: Mutex<bool>,
    notify: Notify,
}

impl PipeServer {
    pub fn new() -> Self {
        PipeServer {
            running: Mutex::new(false),
            notify: Notify::new(),
        }
    }

    pub async fn start<F>(&self, handler: F) -> Result<(), String>
    where
        F: Fn(String) -> Option<String> + Send + Sync + 'static,
    {
        {
            let mut running = self.running.lock().await;
            if *running {
                return Ok(());
            }
            *running = true;
        }

        let handler = Arc::new(handler);

        loop {
            let server = ServerOptions::new()
                .create(PIPE_NAME)
                .map_err(|e| format!("Failed to create pipe: {}", e))?;

            let connect_result = tokio::select! {
                r = server.connect() => r,
                _ = self.notify.notified() => {
                    let mut running = self.running.lock().await;
                    *running = false;
                    return Ok(());
                }
            };

            match connect_result {
                Ok(()) => {
                    let handler = handler.clone();
                    tokio::spawn(async move {
                        handle_client(server, handler).await;
                    });
                }
                Err(e) => {
                    eprintln!("Pipe connect failed: {}", e);
                }
            }
        }
    }

    pub async fn stop(&self) {
        {
            let mut running = self.running.lock().await;
            if !*running {
                return;
            }
            *running = false;
        }
        self.notify.notify_one();
    }
}

async fn handle_client<F>(mut server: NamedPipeServer, handler: Arc<F>)
where
    F: Fn(String) -> Option<String> + Send + Sync + 'static,
{
    let mut buffer = vec![0u8; BUFFER_SIZE];
    let mut read_offset = 0usize;

    loop {
        match server.read(&mut buffer[read_offset..]).await {
            Ok(0) => break,
            Ok(n) => {
                read_offset += n;

                let request = String::from_utf8_lossy(&buffer[..read_offset]).to_string();
                let request = request.trim_end_matches('\0').to_string();

                let response = handler(request);

                let data = match &response {
                    Some(resp) => resp.as_bytes(),
                    None => br#"{"handled":false}"#,
                };

                if let Err(e) = server.write_all(data).await {
                    eprintln!("Pipe write error: {}", e);
                    break;
                }

                read_offset = 0;

                let mut remaining = vec![0u8; BUFFER_SIZE];
                match server.read(&mut remaining).await {
                    Ok(0) => break,
                    Ok(n) => {
                        buffer[..n].copy_from_slice(&remaining[..n]);
                        read_offset = n;
                    }
                    Err(e) => {
                        eprintln!("Pipe read error: {}", e);
                        break;
                    }
                }
            }
            Err(e) => {
                eprintln!("Pipe read error: {}", e);
                break;
            }
        }
    }
}
