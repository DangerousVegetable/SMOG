use common::RELATIVE_MAPS_PATH;
use itertools::Itertools;
use log::{error, info};
use map_editor::map::{Map as GameMap, Spawn};
use packet_tools::{game_packets::PACKET_SIZE, server_packets::ServerPacket, UnsizedPacketWrite};
use server::{lobby::Player, server::{GameServer, LobbyServer}};
use text_io::try_scan;
use std::{collections::HashMap, io::{stdout, Write}, time::Duration};

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

    let map = GameMap::init_from_file(&map, RELATIVE_MAPS_PATH).unwrap();
    let spawns = map.spawns.clone();
    let lobby_server = LobbyServer::new(addr, map).await?;
    info!("Press enter to adjust the lobby");
    let mut input = String::new();
    let _ = std::io::stdin().read_line(&mut input);

    let mut lobby  = lobby_server.get_lobby().await;
    loop {
        print!(">>> ");
        stdout().flush().unwrap();
        let mut input = String::new();
        let _ = std::io::stdin().read_line(&mut input);

        if let Ok((i, j)) = parse_swap(&input) {
            swap_ids(&mut lobby, i, j).await;
            display_players(&lobby, &spawns);
        }

        if input.starts_with("teams") {
            display_players(&lobby, &spawns);
        }
        if input.starts_with("start") {
            break;
        }
        if input.starts_with("stop") {
            return Ok(());
        }
    }

    let mut server = GameServer::new(
        lobby,
        Duration::from_nanos(2300000), // 2.3ms per PHYSICS TICK ~ 55 fps client
        16,
    )
    .await;

    server.run::<PACKET_SIZE>().await;

    loop {
        print!(">>> ");
        stdout().flush().unwrap();
        let mut input = String::new();
        let _ = std::io::stdin().read_line(&mut input);

        if input.starts_with("stop") {
            break;
        }
    }

    Ok(())
}

fn parse_swap(input: &String) -> Result<(u8, u8), Box<dyn std::error::Error>> {
    let i: u8;
    let j: u8;
    try_scan!(input.bytes() => "swap {} {}", i, j);
    Ok((i, j))
}

async fn swap_ids(players: &mut Vec<Player>, i: u8, j: u8) {
    for player in players {
        if player.id == i {
            player.id = j;
            let _  = player.stream.write_packet(&ServerPacket::SetId(j)).await;
        } else if player.id == j {
            player.id = i;
            let _  = player.stream.write_packet(&ServerPacket::SetId(i)).await;
        }
    }
}

fn display_players(players: &Vec<Player>, spawns: &Vec<Spawn>) {
    let mut spawn_ids = HashMap::<usize, Vec<usize>>::new();
    let mut player_ids = HashMap::<usize, String>::new();

    for (i, spawn) in spawns.iter().enumerate() {
        if spawn_ids.get(&spawn.team).is_none() {
            spawn_ids.insert(spawn.team, Vec::new());
        }

        spawn_ids.get_mut(&spawn.team).map(|v| v.push(i));
    }

    for player in players {
        player_ids.insert(player.id as usize, player.name.clone());
    }

    println!("Displaying teams:\n");
    for (team, ids) in spawn_ids.iter().sorted_by_key(|s| s.0) {
        println!("Team #{team}:");
        for id in ids {
            let str = player_ids.get(id).map_or(format!("{id}: ______\n"), |name| format!("{id}: {name}\n"));
            print!("{str}");
        }
        println!("\n");
    }
}
