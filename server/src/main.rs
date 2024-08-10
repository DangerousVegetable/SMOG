use log::{error, info, warn};
use packet_tools::game_packets::PACKET_SIZE;
use server::server::{GameServer, LobbyServer};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let env = env_logger::Env::default()
        .filter_or("MY_LOG_LEVEL", "info")
        .write_style_or("MY_LOG_STYLE", "always");

    env_logger::init_from_env(env);

    let args: Vec<_> = std::env::args().collect();
    if args.len() < 1 {
        error!("Provide an ip of the server as a command line argument");
        return Ok(());
    }

    let addr = &args[1];
    let map = "default".to_string();
    let map = args.get(2).unwrap_or(&map);

    let lobby_server = LobbyServer::new(addr, map).await?;
    info!("Press enter to start the lobby");

    let mut input = String::new();
    let _ = std::io::stdin().read_line(&mut input);

    let mut server = GameServer::new(
        lobby_server.get_lobby().await,
        Duration::from_nanos(2300000), // 2.3ms per PHYSICS TICK ~ 55 fps client
        16,
    )
    .await;

    server.run::<PACKET_SIZE>().await;

    loop {
        let mut input_text = String::new();
        std::io::stdin()
            .read_line(&mut input_text)
            .expect("failed to read from stdin");

        if input_text.starts_with("stop") {
            break;
        }
    }

    Ok(())
}
