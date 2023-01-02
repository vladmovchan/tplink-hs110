use anyhow::anyhow;
use clap::{arg, Command};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    io::{Read, Write},
    net,
};

/*
 *          'info'     : '{"system":{"get_sysinfo":{}}}',
            'on'       : '{"system":{"set_relay_state":{"state":1}}}',
            'off'      : '{"system":{"set_relay_state":{"state":0}}}',
 *          'ledoff'   : '{"system":{"set_led_off":{"off":1}}}',
 *          'ledon'    : '{"system":{"set_led_off":{"off":0}}}',
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
            let request = encrypt(json!({"system": {"get_sysinfo": {} }}).to_string());
            let mut stream = net::TcpStream::connect(addr)?;
            stream.write(&request)?;

            let buf = &mut [0u8; 1024];
            let nread = stream.read(buf)?;
            let response = decrypt(&buf[..nread])?;

            let response = serde_json::from_str::<HashMap<String, Value>>(&response)?;
            println!("{:#?}", response)
        }
        Some(("led", sub_matches)) => {
            let switch_on = sub_matches.get_flag("on");
            let switch_off = sub_matches.get_flag("off");

            // Clap disallows to set both flags at the same time:
            if switch_on ^ switch_off {
                let request = encrypt(
                    json!({"system": {"set_led_off": {"off": switch_off as u8 }}}).to_string(),
                );
                let mut stream = net::TcpStream::connect(addr.clone())?;

                stream.write(&request)?;
                let buf = &mut [0u8; 1024];
                let nread = stream.read(buf)?;
                let response = decrypt(&buf[..nread])?;

                let response = serde_json::from_str::<HashMap<String, Value>>(&response)?;
                let status = response
                    .get("system")
                    .ok_or_else(|| {
                        eprintln!("Response: {:#?}", &response);
                        anyhow!("`system` object is not available in the response")
                    })?
                    .get("set_led_off")
                    .ok_or_else(|| {
                        eprintln!("Response: {:#?}", &response);
                        anyhow!("`set_led_off` object in not available in the response")
                    })?
                    .get("err_code")
                    .ok_or_else(|| {
                        eprintln!("Response: {:#?}", &response);
                        anyhow!("`err_code` field in not available in the response")
                    })?;
                println!(
                    "Operation has {}",
                    if status == 0 { "succeeded" } else { "failed" }
                );
            }

            let request = encrypt(json!({"system": {"get_sysinfo": {} }}).to_string());
            let mut stream = net::TcpStream::connect(addr)?;

            stream.write(&request)?;
            let buf = &mut [0u8; 1024];
            let nread = stream.read(buf)?;
            let response = decrypt(&buf[..nread])?;

            let response = serde_json::from_str::<HashMap<String, Value>>(&response)?;
            let sysinfo = response
                .get("system")
                .ok_or_else(|| {
                    eprintln!("Response: {:#?}", &response);
                    anyhow!("`system` object is not available in the response")
                })?
                .get("get_sysinfo")
                .ok_or_else(|| {
                    eprintln!("Response: {:#?}", &response);
                    anyhow!("`get_sysinfo` object in not available in the response")
                })?;
            let led_off = sysinfo.get("led_off").ok_or_else(|| {
                eprintln!("get_sysinfo: {:#?}", &sysinfo);
                anyhow!("`led_off` field in not available in the response")
            })?;
            println!("LED is {}", if led_off == 0 { "ON" } else { "OFF" });
            /*
             * {
             *   "system": {
             *     "get_sysinfo": {
             *       "err_code": 0,
             *       "sw_ver": "1.2.6 Build 200727 Rel.120821",
             *       "hw_ver": "1.0",
             *       "type": "IOT.SMARTPLUGSWITCH",
             *       "model": "HS110(EU)",
             *       "mac": "70:4F:57:58:CD:5D",
             *       "deviceId": "8006EB1F5E6DADEFC7F38360A3640B1D1910D881",
             *       "hwId": "45E29DA8382494D2E82688B52A0B2EB5",
             *       "fwId": "00000000000000000000000000000000",
             *       "oemId": "3D341ECE302C0642C99E31CE2430544B",
             *       "alias": "Fan and vacuum",
             *       "dev_name": "Wi-Fi Smart Plug With Energy Monitoring",
             *       "icon_hash": "",
             *       "relay_state": 1,
             *       "on_time": 247235,
             *       "active_mode": "schedule",
             *       "feature": "TIM:ENE",
             *       "updating": 0,
             *       "rssi": -45,
             *       "led_off": 1,
             *       "latitude": 47.784857,
             *       "longitude": 35.184122
             *     }
             *   }
             * }
             */
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
            let plain_char = (key ^ byte) as char;
            key = *byte;
            plain_char
        })
        .collect();

    Ok(decrypted)
}
