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
        stream.flush()?;

        let mut received = vec![];
        let mut rx_buf = [0u8; NET_BUFFER_SIZE];
        loop {
            let nread = stream.read(&mut rx_buf)?;
            received.extend_from_slice(&rx_buf[..nread]);
            if nread < NET_BUFFER_SIZE {
                break;
            }
        }
        Self::decrypt(&received)
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

        // Smart plugs of HW version 1 and HW version 2 provide results via different json fields and use different units.
        // I.e. one uses "voltage" in Volts and another "voltage_mv" in milliVolts.
        // As it is hard to decide which version is "better" or more widely used - calculate and provide both fields
        // for both hardware versions:
        #[rustfmt::skip]
        [
            ("voltage_mv", "voltage",    1000f64),
            ("current_ma", "current",    1000f64),
            ("power_mw",   "power",      1000f64),
            ("total_wh",   "total",      1000f64),
            ("voltage",    "voltage_mv", 0.001f64),
            ("current",    "current_ma", 0.001f64),
            ("power",      "power_mw",   0.001f64),
            ("total",      "total_wh",   0.001f64),
        ]
        .iter()
        .for_each(|(from, to, divider)| {
            if let Some(from) = emeter.get(from) {
                if emeter.get(to).is_none() {
                    emeter[to] = Value::from(from.as_f64().unwrap_or(0f64) / divider);
                }
            }
        });

        Ok(emeter)
    }

    fn reboot_raw(&self, delay: Option<u32>) -> anyhow::Result<String> {
        self.request(&json!({"system": {"reboot": {"delay": delay.unwrap_or(0) }}}))
    }

    pub fn reboot_parsed(&self, delay: Option<u32>) -> anyhow::Result<bool> {
        let response = serde_json::from_str::<Value>(&self.reboot_raw(delay)?)?;
        let err_code = extract_hierarchical(&response, &["system", "reboot", "err_code"])?;

        Ok(err_code == 0)
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

#[cfg(test)]
mod tests {
    use crate::*;
    use once_cell::sync::Lazy;
    use serial_test::serial;

    static TEST_TARGET_ADDR: Lazy<String> =
        Lazy::new(|| std::env::var("TEST_TARGET_ADDR").expect("TEST_TARGET_ADDR env variable"));

    #[test]
    #[serial]
    fn basic() {
        let hs110 = HS110::new(&*TEST_TARGET_ADDR).with_timeout(Duration::from_secs(3));
        assert_ne!("unknown", hs110.hostname().unwrap());

        let hs110 = HS110::new(&*TEST_TARGET_ADDR);
        assert_ne!("unknown", hs110.hostname().unwrap());

        assert!(matches!(
            hs110.hw_version(),
            Ok(HwVersion::Version1) | Ok(HwVersion::Version2)
        ));
    }

    #[test]
    fn led_on_off() {
        let hs110 = HS110::new(&*TEST_TARGET_ADDR);

        let original_state = hs110.led_status().unwrap();
        assert!(hs110.set_led_state_parsed(!original_state).is_ok());
        assert_eq!(hs110.led_status().unwrap(), !original_state);
        assert!(hs110.set_led_state_parsed(original_state).is_ok());
        assert_eq!(hs110.led_status().unwrap(), original_state);
    }

    #[test]
    #[serial]
    #[ignore] // Power-cycles devices connected to the plug
    fn power_on_off() {
        let hs110 = HS110::new(&*TEST_TARGET_ADDR);

        let original_state = hs110.power_state().unwrap();
        assert!(hs110.set_power_state_parsed(!original_state).is_ok());
        assert_eq!(hs110.power_state().unwrap(), !original_state);
        assert!(hs110.set_power_state_parsed(original_state).is_ok());
        assert_eq!(hs110.power_state().unwrap(), original_state);
    }

    #[test]
    fn cloudinfo() {
        let hs110 = HS110::new(&*TEST_TARGET_ADDR);

        assert!(hs110.cloudinfo_parsed().is_ok());
    }

    #[test]
    #[serial]
    fn ap_list_scan() {
        let hs110 = HS110::new(&*TEST_TARGET_ADDR);

        hs110
            .ap_list_parsed(false)
            .unwrap()
            .as_array()
            .expect("json array");
        assert!(!hs110
            .ap_list_parsed(true)
            .unwrap()
            .as_array()
            .expect("json array")
            .is_empty());
    }

    #[test]
    #[serial]
    #[ignore] // Power-cycles devices connected to the plug
    fn reboot() {
        let hs110 = HS110::new(&*TEST_TARGET_ADDR);
        assert_eq!(hs110.reboot_parsed(None).unwrap(), true);

        let hs110 = HS110::new(&*TEST_TARGET_ADDR).with_timeout(Duration::from_secs(1));
        // Device is expected to be unreachable after the previous reboot
        assert!(hs110.reboot_parsed(Some(1)).is_err());

        // Wait till device is back online before the end of the test
        for _ in 0..20 {
            let hs110 = HS110::new(&*TEST_TARGET_ADDR).with_timeout(Duration::from_secs(10));
            if hs110.hostname().is_ok() {
                return;
            }
        }
        panic!("Device didn't back online after reboot");
    }
}
