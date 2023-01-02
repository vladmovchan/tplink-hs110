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

struct HS110 {
    addr: String,
}

impl HS110 {
    fn new(addr: String) -> Self {
        Self { addr }
    }

    fn encrypt(string: String) -> Vec<u8> {
        let mut key = 171;
        let mut result = (string.len() as u32).to_be_bytes().to_vec();
        for b in string.into_bytes() {
            key ^= b;
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

    fn info(&self) -> anyhow::Result<String> {
        let request = Self::encrypt(json!({"system": {"get_sysinfo": {} }}).to_string());
        let mut stream = net::TcpStream::connect(self.addr.clone())?;
        stream.write_all(&request)?;

        let buf = &mut [0u8; 1024];
        let nread = stream.read(buf)?;
        Self::decrypt(&buf[..nread])
    }

    fn info_deserialized(&self) -> anyhow::Result<HashMap<String, Value>> {
        Ok(serde_json::from_str::<HashMap<String, Value>>(
            &self.info()?,
        )?)
    }

    fn led_status(&self) -> anyhow::Result<bool> {
        let response = self.info_deserialized()?;
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

        Ok(led_off == 0)
    }

    fn set_led_state(&self, on: bool) -> anyhow::Result<String> {
        let request =
            Self::encrypt(json!({"system": {"set_led_off": {"off": !on as u8 }}}).to_string());
        let mut stream = net::TcpStream::connect(self.addr.clone())?;

        stream.write_all(&request)?;
        let buf = &mut [0u8; 1024];
        let nread = stream.read(buf)?;
        Self::decrypt(&buf[..nread])
    }

    fn set_led_state_deserialized(&self, on: bool) -> anyhow::Result<bool> {
        let response = serde_json::from_str::<HashMap<String, Value>>(&self.set_led_state(on)?)?;
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
        Ok(status == 0)
    }
}

fn main() -> anyhow::Result<()> {
    let matches = cli().get_matches();

    let hostname = matches.get_one::<String>("HOST").expect("required");
    let port = matches.get_one::<u16>("port").expect("defaulted in clap");
    let hs110 = HS110::new(format!("{hostname}:{port}"));

    match matches.subcommand() {
        Some(("info", _)) => {
            println!("{:#?}", hs110.info_deserialized())
        }
        Some(("led", sub_matches)) => {
            let switch_on = sub_matches.get_flag("on");
            let switch_off = sub_matches.get_flag("off");

            // Clap disallows to set both flags at the same time:
            if switch_on ^ switch_off {
                let led = hs110.led_status()?;
                if led && switch_on || (!led && switch_off) {
                    println!("LED is already {}", if led { "ON" } else { "OFF" });
                    return Ok(());
                }

                let status = hs110.set_led_state_deserialized(switch_on)?;
                println!(
                    "Operation has {}",
                    if status { "succeeded" } else { "failed" }
                );
            }

            let led = hs110.led_status()?;
            println!("LED is {}", if led { "ON" } else { "OFF" });
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
