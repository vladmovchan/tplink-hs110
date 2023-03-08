## A CLI tool and a library to control TP-Link HS110 (and HS100) SmartPlugs over WiFi ##

### Build ###
`cargo build`

### Usage examples ###

#### Top level commands ####
```
$ cargo run -q
TP-Link Kasa HS110 client

Usage: tplink-hs110 [OPTIONS] <HOST> <COMMAND>

Commands:
  info           Get smartplug system information
  led            Get and manage LED state
  power          Get and manage power state
  cloudinfo      Get cloud information
  wifi           Scan and list available wifi stations
  reboot         Reboot a smart plug (causes power interruption for connected devices)
  factory-reset  Reset device to factory settings
  emeter         Get energy meter readings (voltage, current, power)
  help           Print this message or the help of the given subcommand(s)

Arguments:
  <HOST>  Hostname or an IP address of the smartplug

Options:
  -p, --port <NUMBER>  TCP port number [default: 9999]
  -h, --help           Print help
```

#### General info ####
```
$ cargo run -q 192.168.0.155 info
{
  "system": {
    "get_sysinfo": {
      "active_mode": "schedule",
      "alias": "Bathroom",
      "dev_name": "Wi-Fi Smart Plug With Energy Monitoring",
      "deviceId": "700644160CBBBB3AC78D5DFB15345D6C191F906B",
      "err_code": 0,
      "feature": "TIM:ENE",
      "fwId": "00000000000000000000000000000000",
      "hwId": "75E20DA8182494D2E82677B52A0B2EB6",
      "hw_ver": "1.0",
      "icon_hash": "",
      "latitude": 48.784857,
      "led_off": 0,
      "longitude": 34.184122,
      "mac": "70:4F:57:58:C6:FA",
      "model": "HS110(EU)",
      "oemId": "3D301ECA121C0642C12E31CE2430347D",
      "on_time": 2262647,
      "relay_state": 1,
      "rssi": -69,
      "sw_ver": "1.2.6 Build 200727 Rel.120821",
      "type": "IOT.SMARTPLUGSWITCH",
      "updating": 0
    }
  }
}
```

#### LED lights ####
```
$ cargo run -q 192.168.0.155 led --help
Get and manage LED state

Usage: tplink-hs110 <HOST> led [OPTIONS]

Options:
  -1, --on    Turn LED on
  -0, --off   Turn LED off
  -h, --help  Print help
$ cargo run -q 192.168.0.155 led
LED is ON
$ cargo run -q 192.168.0.155 led --off
Operation has succeeded
LED is OFF
$ cargo run -q 192.168.0.155 led --on
Operation has succeeded
LED is ON
```

#### Power state ####
```
$ cargo run -q 192.168.0.155 power --help
Get and manage power state

Usage: tplink-hs110 <HOST> power [OPTIONS]

Options:
  -1, --on    Turn power on
  -0, --off   Turn power off
  -h, --help  Print help
$ cargo run -q 192.168.0.155 power
Power is OFF
$ cargo run -q 192.168.0.155 power --on
Operation has succeeded
Power is ON
$ cargo run -q 192.168.0.155 power --off
Operation has succeeded
Power is OFF
```

#### Cloud info ####
```
$ cargo run -q 192.168.0.122 cloudinfo
{
  "binded": 1,
  "cld_connection": 1,
  "err_code": 0,
  "fwDlPage": "",
  "fwNotifyType": 0,
  "illegalType": 0,
  "server": "n-devs.tplinkcloud.com",
  "stopConnect": 0,
  "tcspInfo": "",
  "tcspStatus": 1,
  "username": "mailbox@domain.com"
}
```

#### Scan and list nearby WiFi access points ####
```
$ cargo run -q 192.168.0.155 wifi --help
Scan and list available wifi stations

Usage: tplink-hs110 <HOST> wifi <COMMAND>

Commands:
  scan  Scan and list available wifi access points
  list  List available wifi access points without performing a scan
  help  Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help
$ cargo run -q 192.168.0.155 wifi scan
[
  {
    "key_type": 3,
    "ssid": "HomeKyiv"
  },
  {
    "key_type": 3,
    "ssid": "HUAWEI AX3"
  },
  {
    "key_type": 3,
    "ssid": "Keenetic-8785"
  },
  {
    "key_type": 3,
    "ssid": "Kyivstar-7C80"
  },
  {
    "key_type": 2,
    "ssid": "Kyivstar242"
  },
  {
    "key_type": 3,
    "ssid": "netis_2.4G_CC0EA8"
  },
  {
    "key_type": 2,
    "ssid": "TP-LINK_245"
  },
  {
    "key_type": 3,
    "ssid": "TP-Link_C1F3"
  }
]
```

#### Reboot ####
```
$ cargo run -q 192.168.0.155 reboot --help
Reboot a smart plug (causes power interruption for connected devices)

Usage: tplink-hs110 <HOST> reboot [OPTIONS]

Options:
  -d, --delay <NUMBER>  Delay a reboot by NUMBER of seconds
  -h, --help            Print help
$ cargo run -q 192.168.0.155 reboot
Operation has succeeded
```

#### Get energy meter readings ####
```
$ cargo run -q 192.168.0.155 emeter
{
  "current": 0.027566,
  "current_ma": 27.566,
  "err_code": 0,
  "power": 0.775979,
  "power_mw": 775.9789999999999,
  "total": 188.23,
  "total_wh": 188230.0,
  "voltage": 232.835569,
  "voltage_mv": 232835.569
}
```

### Extending list of commands ###
A full list of commands supported by HS110/HS100 smartplugs is available in [tplink-smarthome-commands.txt](https://github.com/softScheck/tplink-smartplug/blob/2e4b5e76bda0ebcc031f18e0532f63a294a29345/tplink-smarthome-commands.txt)

Only a limited set of these commands is implemented in the library at the moment.
