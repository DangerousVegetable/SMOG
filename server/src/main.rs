use std::time::Duration;
use log::{error, warn};
use server::{tcp::TcpSyncServer, PACKET_SIZE};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let env = env_logger::Env::default()
        .filter_or("MY_LOG_LEVEL", "info")
        .write_style_or("MY_LOG_STYLE", "always");

    env_logger::init_from_env(env);

    let args: Vec<_> = std::env::args().collect();
    if args.len() < 1 {
        warn!("provide an ip of the server as a command line argument");
    }

    let addr = &args[1];

    let server = TcpSyncServer::new(
        addr, 
        Duration::from_millis(2),
        16
    ).await?;
    
    server.listen_for_connections();
    loop {
        let mut input_text = String::new();
        std::io::stdin()
            .read_line(&mut input_text)
            .expect("failed to read from stdin");

        if input_text.starts_with("run") {
            server.stop_listening_for_new_connections();
            std::thread::sleep(Duration::from_secs(1));
            server.run::<PACKET_SIZE>();
        }

        if input_text.starts_with("stop") {
            server.stop();
            break;
        }
    }

    std::thread::sleep(Duration::from_secs(2));
    Ok(())
}


