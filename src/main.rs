use anyhow::anyhow;
use clap::{arg, Command};
use std::{
    io::{Read, Write},
    net,
};

/*
            'info'     : '{"system":{"get_sysinfo":{}}}',
            'on'       : '{"system":{"set_relay_state":{"state":1}}}',
            'off'      : '{"system":{"set_relay_state":{"state":0}}}',
            'ledoff'   : '{"system":{"set_led_off":{"off":1}}}',
            'ledon'    : '{"system":{"set_led_off":{"off":0}}}',
            'cloudinfo': '{"cnCloud":{"get_info":{}}}',
            'wlanscan' : '{"netif":{"get_scaninfo":{"refresh":0}}}',
            'time'     : '{"time":{"get_time":{}}}',
            'schedule' : '{"schedule":{"get_rules":{}}}',
            'countdown': '{"count_down":{"get_rules":{}}}',
            'antitheft': '{"anti_theft":{"get_rules":{}}}',
            'reboot'   : '{"system":{"reboot":{"delay":1}}}',
            'reset'    : '{"system":{"reset":{"delay":1}}}',
            'energy'   : '{"emeter":{"get_realtime":{}}}'
*/

fn main() -> anyhow::Result<()> {
    let matches = cli().get_matches();

    let hostname = matches.get_one::<String>("HOST").expect("required");
    let port = matches.get_one::<u16>("port").expect("defaulted in clap");
    let addr = format!("{hostname}:{port}");

    match matches.subcommand() {
        Some(("info", _)) => {
            let request = encrypt(r#"{"system":{"get_sysinfo":{}}}"#.to_string());
            let mut stream = net::TcpStream::connect(addr)?;
            stream.write(&request)?;

            let response = &mut [0u8; 1024];
            let nread = stream.read(response)?;
            println!("{}", decrypt(&response[..nread])?);
        }
        Some(("led", sub_matches)) => {
            let on = sub_matches.get_flag("on");
            let off = sub_matches.get_flag("off");

            let request = match (on, off) {
                (true, false) => r#"{"system":{"set_led_off":{"off":0}}}"#,
                (false, true) => r#"{"system":{"set_led_off":{"off":1}}}"#,
                _ => r#"{"system":{"get_sysinfo":{}}}"#,
            };
            let request = encrypt(request.to_string());
            let mut stream = net::TcpStream::connect(addr)?;

            stream.write(&request)?;

            let response = &mut [0u8; 1024];
            let nread = stream.read(response)?;
            println!("{}", decrypt(&response[..nread])?);
        }
        Some((_ext, _sub_matches)) => {
            unimplemented!()
        }
        None => {}
    }

    Ok(())
}

fn cli() -> Command {
    Command::new("kasa-client")
        .about("TP-Link Kasa HS110 client")
        .arg(arg!(<HOST> "Hostname or an IP address of the smartplug"))
        .arg_required_else_help(true)
        .arg(
            arg!(--port <NUMBER> "TCP port number")
                .short('p')
                .value_parser(clap::value_parser!(u16))
                .num_args(1)
                .default_value("9999"),
        )
        .subcommand_required(true)
        .arg_required_else_help(true)
        .allow_external_subcommands(true)
        .subcommand(Command::new("info").about("Get smartplug system information"))
        .subcommand(
            Command::new("led")
                .about("Manage LED state")
                .arg(
                    arg!(--on "Turn LED on")
                        .short('1')
                        .num_args(0)
                        .conflicts_with("off"),
                )
                .arg(
                    arg!(--off "Turn LED off")
                        .short('0')
                        .num_args(0)
                        .conflicts_with("on"),
                ),
        )
}

fn encrypt(string: String) -> Vec<u8> {
    let mut key = 171;
    let mut result = (string.len() as u32).to_be_bytes().to_vec();
    for b in string.into_bytes() {
        key = key ^ b;
        result.push(key);
    }
    result
}

fn decrypt(data: &[u8]) -> anyhow::Result<String> {
    let header_len = 4;
    if data.len() < header_len {
        return Err(anyhow!("Encrypted response is too short: {}", data.len()));
    }

    let payload_len = u32::from_be_bytes(data[..header_len].try_into()?);
    if data.len() - header_len != payload_len as usize {
        return Err(anyhow!(
            "Encrypted response payload size ({}), differs from the payload size specified in header ({})",
            data.len() - header_len,
            payload_len
        ));
    }

    let mut key = 171;

    let decrypted: String = data[4..]
        .iter()
        .map(|byte| {
            let symbol = (key ^ byte) as char;
            key = *byte;
            symbol
        })
        .collect();

    Ok(decrypted)
}
