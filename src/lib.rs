use anyhow::anyhow;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    io::{Read, Write},
    net,
};

pub struct HS110 {
    addr: String,
}

impl HS110 {
    pub fn new(addr: String) -> Self {
        let addr = match addr.find(':') {
            None => format!("{addr}:9999"),
            _ => addr,
        };
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
        let header_len = std::mem::size_of::<u32>();
        if data.len() < header_len {
            return Err(anyhow!("Encrypted response is too short: {}", data.len()));
        }

        let payload_len = u32::from_be_bytes(data[..header_len].try_into()?);
        if data.len() - header_len != payload_len as usize {
            return Err(
                anyhow!(
                    "Encrypted response payload size ({}), differs from the payload size specified in header ({})",
                    data.len() - header_len,
                    payload_len)
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

    fn request(&self, request: Value) -> anyhow::Result<String> {
        let request = Self::encrypt(request.to_string());
        let mut stream = net::TcpStream::connect(self.addr.clone())?;

        stream.write_all(&request)?;
        let buf = &mut [0u8; 8192];
        let nread = stream.read(buf)?;
        Self::decrypt(&buf[..nread])
    }

    pub fn info_raw(&self) -> anyhow::Result<String> {
        self.request(json!({"system": {"get_sysinfo": {}}}))
    }

    pub fn info_parsed(&self) -> anyhow::Result<HashMap<String, Value>> {
        Ok(serde_json::from_str::<HashMap<String, Value>>(
            &self.info_raw()?,
        )?)
    }

    fn info_field_value(&self, field: &str) -> anyhow::Result<Value> {
        let response = self.info_parsed()?;
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
        let value = sysinfo.get(field).ok_or_else(|| {
            eprintln!("get_sysinfo: {:#?}", &sysinfo);
            anyhow!(format!("`{field}` field in not available in the response"))
        })?;

        Ok(value.clone())
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

    fn set_led_state_raw(&self, on: bool) -> anyhow::Result<String> {
        self.request(json!({"system": {"set_led_off": {"off": !on as u8 }}}))
    }

    pub fn set_led_state_parsed(&self, on: bool) -> anyhow::Result<bool> {
        let response =
            serde_json::from_str::<HashMap<String, Value>>(&self.set_led_state_raw(on)?)?;
        let err_code = response
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
        Ok(err_code == 0)
    }

    pub fn power_state(&self) -> anyhow::Result<bool> {
        Ok(self.info_field_value("relay_state")? == 1)
    }

    fn set_power_state_raw(&self, state: bool) -> anyhow::Result<String> {
        self.request(json!({"system": {"set_relay_state": {"state": state as u8 }}}))
    }

    pub fn set_power_state_parsed(&self, state: bool) -> anyhow::Result<bool> {
        let response =
            serde_json::from_str::<HashMap<String, Value>>(&self.set_power_state_raw(state)?)?;
        println!("{:#?}", response);
        let err_code = response
            .get("system")
            .ok_or_else(|| {
                eprintln!("Response: {:#?}", &response);
                anyhow!("`system` object is not available in the response")
            })?
            .get("set_relay_state")
            .ok_or_else(|| {
                eprintln!("Response: {:#?}", &response);
                anyhow!("`set_relay_state` object in not available in the response")
            })?
            .get("err_code")
            .ok_or_else(|| {
                eprintln!("Response: {:#?}", &response);
                anyhow!("`err_code` field in not available in the response")
            })?;
        Ok(err_code == 0)
    }

    fn cloudinfo_raw(&self) -> anyhow::Result<String> {
        self.request(json!({"cnCloud": {"get_info": {}}}))
    }

    pub fn cloudinfo_parsed(&self) -> anyhow::Result<Value> {
        let response = serde_json::from_str::<HashMap<String, Value>>(&self.cloudinfo_raw()?)?;
        let cloudinfo = response
            .get("cnCloud")
            .ok_or_else(|| {
                eprintln!("Response: {:#?}", &response);
                anyhow!("`cnCloud` object is not available in the response")
            })?
            .get("get_info")
            .ok_or_else(|| {
                eprintln!("Response: {:#?}", &response);
                anyhow!("`get_info` object in not available in the response")
            })?;

        Ok(cloudinfo.clone())
    }

    fn ap_list_raw(&self, refresh: bool) -> anyhow::Result<String> {
        self.request(json!({"netif": {"get_scaninfo": {"refresh": refresh as u8}}}))
    }

    pub fn ap_list_parsed(&self, refresh: bool) -> anyhow::Result<Value> {
        let response = serde_json::from_str::<HashMap<String, Value>>(&self.ap_list_raw(refresh)?)?;
        let ap_list = response
            .get("netif")
            .ok_or_else(|| {
                eprintln!("Response: {:#?}", &response);
                anyhow!("`netif` object is not available in the response")
            })?
            .get("get_scaninfo")
            .ok_or_else(|| {
                eprintln!("Response: {:#?}", &response);
                anyhow!("`get_scaninfo` object in not available in the response")
            })?
            .get("ap_list")
            .ok_or_else(|| {
                eprintln!("Response: {:#?}", &response);
                anyhow!("`ap_list` field in not available in the response")
            })?;

        Ok(ap_list.clone())
    }

    fn emeter_raw(&self) -> anyhow::Result<String> {
        self.request(json!({"emeter":{"get_realtime":{}}}))
    }

    pub fn emeter_parsed(&self) -> anyhow::Result<Value> {
        let response = serde_json::from_str::<HashMap<String, Value>>(&self.emeter_raw()?)?;
        let emeter = response
            .get("emeter")
            .ok_or_else(|| {
                eprintln!("Response: {:#?}", &response);
                anyhow!("`emeter` object is not available in the response")
            })?
            .get("get_realtime")
            .ok_or_else(|| {
                eprintln!("Response: {:#?}", &response);
                anyhow!("`get_realtime` object in not available in the response")
            })?;

        Ok(emeter.clone())
    }
}
