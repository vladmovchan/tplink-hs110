use clap::{arg, Command};
use serde_json::to_string_pretty;
use tplink_hs110::{error::TpLinkHs110Error, HS110};

fn main() -> Result<(), TpLinkHs110Error> {
    let matches = cli().get_matches();

    let hostname = matches
        .get_one::<String>("HOST")
        .ok_or(TpLinkHs110Error::HostIsNotProvided)?;
    let port = matches
        .get_one::<u16>("port")
        .ok_or(TpLinkHs110Error::PortIsNotProvided)?;
    let smartplug = HS110::new(&format!("{hostname}:{port}"))?;

    match matches.subcommand() {
        Some(("info", _)) => {
            println!("{}", to_string_pretty(&smartplug.info()?)?)
        }
        Some(("led", sub_matches)) => {
            let switch_on = sub_matches.get_flag("on");
            let switch_off = sub_matches.get_flag("off");

            // Clap disallows to set both flags at the same time:
            if switch_on ^ switch_off {
                let led: bool = smartplug.led_state()?.into();
                if led && switch_on || (!led && switch_off) {
                    println!("LED is already {}", if led { "ON" } else { "OFF" });
                    return Ok(());
                }

                smartplug.set_led_state(switch_on.into())?;
                println!("Operation completed successfully");
            }

            let led_state = smartplug.led_state()?;
            println!("LED is {led_state}");
        }
        Some(("power", sub_matches)) => {
            let switch_on = sub_matches.get_flag("on");
            let switch_off = sub_matches.get_flag("off");

            // Clap disallows to set both flags at the same time:
            if switch_on ^ switch_off {
                let power: bool = smartplug.power_state()?.into();
                if power && switch_on || (!power && switch_off) {
                    println!("Power is already {}", if power { "ON" } else { "OFF" });
                    return Ok(());
                }

                smartplug.set_power_state(switch_on.into())?;
                println!("Operation completed successfully");
            }

            let power_state = smartplug.power_state()?;
            println!("Power is {power_state}");
        }
        Some(("cloudinfo", _)) => {
            println!("{}", to_string_pretty(&smartplug.cloudinfo()?)?)
        }
        Some(("wifi", sub_matches)) => match sub_matches.subcommand() {
            Some(("scan", _)) => {
                println!("{}", to_string_pretty(&smartplug.ap_list(true)?)?);
            }
            Some(("list", _)) => {
                println!("{}", to_string_pretty(&smartplug.ap_list(false)?)?)
            }
            _ => {
                unreachable!()
            }
        },
        Some(("emeter", _)) => {
            println!("{}", to_string_pretty(&smartplug.emeter()?)?)
        }
        Some(("reboot", sub_matches)) => {
            let delay = sub_matches.get_one::<u32>("delay").copied();

            smartplug.reboot(delay)?;
            println!("Operation completed successfully");
        }
        Some(("factory-reset", sub_matches)) => {
            let delay = sub_matches.get_one::<u32>("delay").copied();

            smartplug.factory_reset(delay)?;
            println!("Operation completed successfully");
        }
        _ => {
            unreachable!()
        }
    }

    Ok(())
}

fn cli() -> Command {
    Command::new("tplink-hs110")
        .about("TP-Link Kasa HS110 client")
        .arg_required_else_help(true)
        .arg(arg!(<HOST> "Hostname or an IP address of the smartplug"))
        .arg(
            arg!(--port <NUMBER> "TCP port number")
                .short('p')
                .value_parser(clap::value_parser!(u16))
                .num_args(1)
                .default_value("9999"),
        )
        .subcommand_required(true)
        .allow_external_subcommands(true)
        .subcommand(Command::new("info").about("Get smartplug system information"))
        .subcommand(
            Command::new("led")
                .about("Get and manage LED state")
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
        .subcommand(
            Command::new("power")
                .about("Get and manage power state")
                .arg(
                    arg!(--on "Turn power on")
                        .short('1')
                        .num_args(0)
                        .conflicts_with("off"),
                )
                .arg(
                    arg!(--off "Turn power off")
                        .short('0')
                        .num_args(0)
                        .conflicts_with("on"),
                ),
        )
        .subcommand(Command::new("cloudinfo").about("Get cloud information"))
        .subcommand(
            Command::new("wifi")
                .about("Scan and list available wifi stations")
                .arg_required_else_help(true)
                .subcommand_required(true)
                .subcommand(
                    Command::new("scan").about("Scan and list available wifi access points"),
                )
                .subcommand(
                    Command::new("list")
                        .about("List available wifi access points without performing a scan"),
                ),
        )
        .subcommand(
            Command::new("reboot")
                .about("Reboot a smart plug (causes power interruption for connected devices)")
                .arg(
                    arg!(--delay <NUMBER> "Delay a reboot by NUMBER of seconds")
                        .short('d')
                        .value_parser(clap::value_parser!(u32))
                        .num_args(1),
                ),
        )
        .subcommand(
            Command::new("factory-reset")
                .about("Reset device to factory settings")
                .arg(
                    arg!(--delay <NUMBER> "Delay a factory-reset by NUMBER of seconds")
                        .short('d')
                        .value_parser(clap::value_parser!(u32))
                        .num_args(1),
                ),
        )
        .subcommand(
            Command::new("emeter").about("Get energy meter readings (voltage, current, power)"),
        )
}
