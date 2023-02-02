use std::{cell::RefCell, time::Duration};

use futures::{stream::FuturesUnordered, StreamExt};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
    select,
    task::LocalSet,
};
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

#[derive(Debug)]
struct TcpWrapper {
    listener: TcpListener,
    messages: RefCell<Vec<String>>,
}

impl TcpWrapper {
    async fn bind() -> anyhow::Result<Self> {
        Ok(Self {
            listener: TcpListener::bind("127.0.0.1:5000").await?,
            messages: RefCell::new(vec![]),
        })
    }

    async fn listen(&self) -> anyhow::Result<()> {
        let mut futs = FuturesUnordered::new();
        let sleep = tokio::time::sleep(Duration::from_millis(2000));
        tokio::pin!(sleep);
        loop {
            select! {
                Ok((stream, _addr)) = self.listener.accept() => {
                    info!("Got tcp connection");
                    futs.push(self.handle_stream(stream));
                }
                Some(res) = futs.next() => {
                    info!(?res, "Future complete");
                }
                _ = &mut sleep => {
                    info!("Exiting listen");
                    break;
                }
            };
        }

        dbg!(futs.len());

        Ok(())
    }

    async fn handle_stream(&self, stream: TcpStream) -> anyhow::Result<()> {
        let mut reader = BufReader::new(stream);

        loop {
            let mut buf = String::new();
            let bytes_read = reader.read_line(&mut buf).await?;
            self.messages.borrow_mut().push(buf.clone());
            info!(bytes_read, messages = ?self.messages);
        }
    }
}

#[tracing::instrument]
#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::TRACE)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    let local = LocalSet::new();

    local
        .run_until(async {
            local.spawn_local(async {
                let listener = TcpWrapper::bind().await?;
                listener.listen().await?;

                Ok::<(), anyhow::Error>(())
            });

            let _ = local.spawn_local(send_on_tcp()).await?;

            Ok::<(), anyhow::Error>(())
        })
        .await?;

    Ok(())
}

#[tracing::instrument(err)]
async fn send_on_tcp() -> anyhow::Result<()> {
    let mut client = TcpStream::connect("127.0.0.1:5000").await?;
    loop {
        client.write_all(b"hello\n").await?;
        client.flush().await?;
        tokio::time::sleep(Duration::from_millis(1000)).await;
    }
}
