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
        error!("Provide an ip of the server as a command line argument");
        return Ok(())
    }

    let addr = &args[1];

    let mut server = TcpSyncServer::new(
        addr, 
        Duration::from_nanos(2300000), // 2.3ms per PHYSICS TICK ~ 55 fps client
        16
    ).await?;
    
    server.accept_connections();
    loop {
        let mut input_text = String::new();
        std::io::stdin()
            .read_line(&mut input_text)
            .expect("failed to read from stdin");

        if input_text.starts_with("run") {
            server.decline_connections();
            std::thread::sleep(Duration::from_millis(100));
            server.run::<PACKET_SIZE>();
        }

        if input_text.starts_with("stop") {
            server.stop();
            break;
        }
    }
    
    Ok(())
}


