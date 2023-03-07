use anyhow::anyhow;
use serde_json::{json, Value};
use std::{
    io::{Read, Write},
    net,
    time::Duration,
};

#[derive(Debug)]
pub struct HS110 {
    addr: String,
    timeout: Option<Duration>,
}

#[derive(Debug)]
pub enum HwVersion {
    Version1,
    Version2,
    Unsupported,
}

const NET_BUFFER_SIZE: usize = 8192;

impl HS110 {
    pub fn new(addr: &str) -> Self {
        let addr = match addr.find(':') {
            None => format!("{addr}:9999"),
            _ => addr.to_owned(),
        };
        Self {
            addr,
            timeout: None,
        }
    }

    pub fn with_timeout(mut self, duration: Duration) -> Self {
        self.timeout = Some(duration);
        self
    }

    fn encrypt(request: &Value) -> Vec<u8> {
        let request = request.to_string();
        let mut key = 171;
        let mut encrypted = (request.len() as u32).to_be_bytes().to_vec();
        for b in request.as_bytes() {
            key ^= b;
            encrypted.push(key);
        }
        encrypted
    }

    fn decrypt(data: &[u8]) -> anyhow::Result<String> {
        let header_len = std::mem::size_of::<u32>();
        if data.len() < header_len {
            return Err(anyhow!("Encrypted response is too short: {}", data.len()));
        }

        let payload_len = u32::from_be_bytes(data[..header_len].try_into()?);
        if data.len() - header_len != payload_len as usize {
            return Err(
                anyhow!(
                    "Encrypted response payload size ({}), differs from the payload size specified in header ({payload_len})",
                    data.len() - header_len)
                );
        }

        let mut key = 171;
        let decrypted: String = data[header_len..]
            .iter()
            .map(|byte| {
                let plain_char = (key ^ byte) as char;
                key = *byte;
                plain_char
            })
            .collect();

        Ok(decrypted)
    }

    fn request(&self, request: &Value) -> anyhow::Result<String> {
        let encrypted = Self::encrypt(request);
        let mut stream = match self.timeout {
            None => net::TcpStream::connect(&self.addr)?,
            Some(duration) => {
                let stream = net::TcpStream::connect_timeout(&self.addr.parse()?, duration)?;
                stream.set_read_timeout(self.timeout)?;
                stream.set_write_timeout(self.timeout)?;
                stream
            }
        };

        stream.write_all(&encrypted)?;
        let buf = &mut [0u8; NET_BUFFER_SIZE];
        let nread = stream.read(buf)?;
        Self::decrypt(&buf[..nread])
    }

    pub fn info_raw(&self) -> anyhow::Result<String> {
        self.request(&json!({"system": {"get_sysinfo": {}}}))
    }

    pub fn info_parsed(&self) -> anyhow::Result<Value> {
        Ok(serde_json::from_str::<Value>(&self.info_raw()?)?)
    }

    fn info_field_value(&self, field: &str) -> anyhow::Result<Value> {
        let response = self.info_parsed()?;
        let value = extract_hierarchical(&response, &["system", "get_sysinfo", field])?;

        Ok(value)
    }

    pub fn led_status(&self) -> anyhow::Result<bool> {
        Ok(self.info_field_value("led_off")? == 0)
    }

    pub fn hostname(&self) -> anyhow::Result<String> {
        Ok(self
            .info_field_value("alias")?
            .as_str()
            .unwrap_or("unknown")
            .to_owned())
    }

    pub fn hw_version(&self) -> anyhow::Result<HwVersion> {
        match self.info_field_value("hw_ver")?.as_str() {
            Some("1.0") => Ok(HwVersion::Version1),
            Some("2.0") => Ok(HwVersion::Version2),
            Some(_) => Ok(HwVersion::Unsupported),
            None => Err(anyhow!("hw_version is not available")),
        }
    }

    fn set_led_state_raw(&self, on: bool) -> anyhow::Result<String> {
        self.request(&json!({"system": {"set_led_off": {"off": !on as u8 }}}))
    }

    pub fn set_led_state_parsed(&self, on: bool) -> anyhow::Result<bool> {
        let response = serde_json::from_str::<Value>(&self.set_led_state_raw(on)?)?;

        let err_code = extract_hierarchical(&response, &["system", "set_led_off", "err_code"])?;

        Ok(err_code == 0)
    }

    pub fn power_state(&self) -> anyhow::Result<bool> {
        Ok(self.info_field_value("relay_state")? == 1)
    }

    fn set_power_state_raw(&self, state: bool) -> anyhow::Result<String> {
        self.request(&json!({"system": {"set_relay_state": {"state": state as u8 }}}))
    }

    pub fn set_power_state_parsed(&self, state: bool) -> anyhow::Result<bool> {
        let response = serde_json::from_str::<Value>(&self.set_power_state_raw(state)?)?;
        let err_code = extract_hierarchical(&response, &["system", "set_relay_state", "err_code"])?;

        Ok(err_code == 0)
    }

    fn cloudinfo_raw(&self) -> anyhow::Result<String> {
        self.request(&json!({"cnCloud": {"get_info": {}}}))
    }

    pub fn cloudinfo_parsed(&self) -> anyhow::Result<Value> {
        let response = serde_json::from_str::<Value>(&self.cloudinfo_raw()?)?;
        let cloudinfo = extract_hierarchical(&response, &["cnCloud", "get_info"])?;

        Ok(cloudinfo)
    }

    fn ap_list_raw(&self, refresh: bool) -> anyhow::Result<String> {
        self.request(&json!({"netif": {"get_scaninfo": {"refresh": refresh as u8}}}))
    }

    pub fn ap_list_parsed(&self, refresh: bool) -> anyhow::Result<Value> {
        let response = serde_json::from_str::<Value>(&self.ap_list_raw(refresh)?)?;
        let ap_list = extract_hierarchical(&response, &["netif", "get_scaninfo", "ap_list"])?;

        Ok(ap_list)
    }

    fn emeter_raw(&self) -> anyhow::Result<String> {
        self.request(&json!({"emeter":{"get_realtime":{}}}))
    }

    pub fn emeter_parsed(&self) -> anyhow::Result<Value> {
        let response = serde_json::from_str::<Value>(&self.emeter_raw()?)?;
        let mut emeter = extract_hierarchical(&response, &["emeter", "get_realtime"])?;

        let fields_to_unify = vec![
            ("voltage_mv", "voltage"),
            ("current_ma", "current"),
            ("power_mw", "power"),
            ("total_wh", "total"),
        ];
        fields_to_unify.iter().for_each(|(field_m, field)| {
            if let Some(value_m) = emeter.get(field_m) {
                emeter[field] = Value::from(value_m.as_f64().unwrap_or(0f64) / 1000f64);
            } else if let Some(value) = emeter.get(field) {
                emeter[field_m] = Value::from((value.as_f64().unwrap_or(0f64) * 1000f64) as u64);
            }
        });

        Ok(emeter)
    }
}

fn extract_hierarchical(response: &Value, path: &[&str]) -> anyhow::Result<Value> {
    let mut value = response;
    for next_prefix in path {
        value = value.get(next_prefix).ok_or_else(|| {
            eprintln!("Response: {response:#?}");
            eprintln!("`{next_prefix}` key is not available in the response");
            anyhow!("`{next_prefix}` key is not available in the response")
        })?;
    }

    Ok(value.clone())
}
