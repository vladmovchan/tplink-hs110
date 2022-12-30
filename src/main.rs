use clap::Parser;
use std::{
    io::{Error, Read, Write},
    net,
};

#[derive(Parser)]
#[clap(disable_help_flag = true)]
struct Args {
    #[arg(short, long, default_value_t = String::from("192.168.0.102"))]
    hostname: String,

    #[arg(short, long, default_value_t = 9999)]
    port: u16,

    #[arg(short, long, default_value_t = String::from(r#"{"system":{"get_sysinfo":{}}}"#))]
    command: String,
}

fn main() -> Result<(), Error> {
    let args = Args::parse();

    println!("Hostname: {}", args.hostname);
    println!("Command: {}", args.command);

    let encrypted = encrypt(args.command);

    let mut stream = net::TcpStream::connect(format!("{}:{}", args.hostname, args.port))?;
    stream.write(&encrypted)?;

    let buf = &mut [0u8; 1024];
    let nread = stream.read(buf)?;
    println!("{}", decrypt(&buf[..nread]));

    Ok(())
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

fn decrypt(data: &[u8]) -> String {
    let mut key = 171;
    assert!(data.len() >= 4);

    let len = u32::from_be_bytes(data[..4].try_into().unwrap());
    assert_eq!(data.len() - 4, len as usize);

    let mut result = String::new();

    for b in data[4..].iter() {
        result.push((key ^ b) as char);
        key = *b;
    }
    result
}
