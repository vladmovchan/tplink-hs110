use clap::{arg, Command};
use tplink_hs1x0::HS110;

/*
 *          'info'     : '{"system":{"get_sysinfo":{}}}',
 *          'on'       : '{"system":{"set_relay_state":{"state":1}}}',
 *          'off'      : '{"system":{"set_relay_state":{"state":0}}}',
 *          'ledoff'   : '{"system":{"set_led_off":{"off":1}}}',
 *          'ledon'    : '{"system":{"set_led_off":{"off":0}}}',
 *          'cloudinfo': '{"cnCloud":{"get_info":{}}}',
 *          'wlanscan' : '{"netif":{"get_scaninfo":{"refresh":0}}}',
            'time'     : '{"time":{"get_time":{}}}',
            'schedule' : '{"schedule":{"get_rules":{}}}',
            'countdown': '{"count_down":{"get_rules":{}}}',
            'antitheft': '{"anti_theft":{"get_rules":{}}}',
            'reboot'   : '{"system":{"reboot":{"delay":1}}}',
            'reset'    : '{"system":{"reset":{"delay":1}}}',
 *          'energy'   : '{"emeter":{"get_realtime":{}}}'
*/

fn main() -> anyhow::Result<()> {
    let matches = cli().get_matches();

    let hostname = matches.get_one::<String>("HOST").expect("required");
    let port = matches.get_one::<u16>("port").expect("defaulted in clap");
    let hs110 = HS110::new(format!("{hostname}:{port}"));

    match matches.subcommand() {
        Some(("info-raw", _)) => {
            println!("{}", hs110.info_raw()?)
        }
        Some(("info", _)) => {
            println!("{:#?}", hs110.info_parsed()?)
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

                let status = hs110.set_led_state_parsed(switch_on)?;
                println!(
                    "Operation has {}",
                    if status { "succeeded" } else { "failed" }
                );
            }

            let led = hs110.led_status()?;
            println!("LED is {}", if led { "ON" } else { "OFF" });
        }
        Some(("power", sub_matches)) => {
            let switch_on = sub_matches.get_flag("on");
            let switch_off = sub_matches.get_flag("off");

            // Clap disallows to set both flags at the same time:
            if switch_on ^ switch_off {
                let power = hs110.power_state()?;
                if power && switch_on || (!power && switch_off) {
                    println!("Power is already {}", if power { "ON" } else { "OFF" });
                    return Ok(());
                }

                let status = hs110.set_power_state_parsed(switch_on)?;
                println!(
                    "Operation has {}",
                    if status { "succeeded" } else { "failed" }
                );
            }

            let power = hs110.power_state()?;
            println!("Power is {}", if power { "ON" } else { "OFF" });
        }
        Some(("cloudinfo", _)) => {
            println!("{:#?}", hs110.cloudinfo_parsed()?)
        }
        Some(("wifi", sub_matches)) => match sub_matches.subcommand() {
            Some(("scan", _)) => {
                println!("{:#?}", hs110.ap_list_parsed(true)?);
            }
            Some(("list", _)) => {
                println!("{:#?}", hs110.ap_list_parsed(false)?)
            }
            _ => {
                unreachable!()
            }
        },
        Some(("emeter", _)) => {
            println!("{:#?}", hs110.emeter_parsed()?)
        }
        _ => {
            unreachable!()
        }
    }

    Ok(())
}

fn cli() -> Command {
    Command::new("tplink-hs1x0")
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
        .subcommand(Command::new("info-raw").about(
            "Get smartplug system information providing the response as it is, without parsing",
        ))
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
            Command::new("emeter").about("Get energy meter readings (voltage, current, power)"),
        )
}
