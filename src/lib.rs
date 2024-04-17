//! A library to control TP-Link HS110 (and HS100) SmartPlugs over Wi-Fi.
use error::TpLinkHs110Error;
use serde_json::{json, Value};
use std::{
    fmt::Display,
    io::{Read, Write},
    mem::size_of,
    net::{self, SocketAddr},
    ops::Not,
    time::Duration,
};

pub mod error;

const NET_BUFFER_SIZE: usize = 8192;

/// HS110 smartplug.
#[derive(Debug)]
pub struct HS110 {
    /// Smartplug network address.
    socket_addr: SocketAddr,

    /// Optional timeout for network communication.
    timeout: Option<Duration>,
}

impl HS110 {
    /// Attempts to create a new HS110 instance using given network address.
    pub fn new(addr: &str) -> Result<Self, TpLinkHs110Error> {
        let socket_addr = match addr.find(':') {
            Some(_) => addr.parse(),
            None => (addr.to_string() + ":9999").parse(),
        }?;

        Ok(Self {
            socket_addr,
            timeout: None,
        })
    }

    /// Sets a timeout for network communication with a smartplug.
    pub fn with_timeout(mut self, duration: Duration) -> Self {
        self.timeout = Some(duration);
        self
    }

    /// "Encrypts" a given string (which is usually a command represented as a JSON).
    ///
    /// This way of encryption/scrambling is necessary before sending a command to a smartplug.
    fn encrypt<S>(payload: S) -> Vec<u8>
    where
        S: AsRef<str>,
    {
        let mut key = 171;

        (payload.as_ref().len() as u32)
            .to_be_bytes()
            .into_iter()
            .chain(payload.as_ref().as_bytes().iter().map(|v| {
                key ^= v;
                key
            }))
            .collect()
    }

    /// Attempts to decrypt/unscramble data received from a smartplug.
    fn decrypt(payload: &[u8]) -> Result<String, TpLinkHs110Error> {
        const HEADER_LEN: usize = size_of::<u32>();
        if payload.len() < HEADER_LEN {
            Err(TpLinkHs110Error::ShortEncryptedResponse(payload.len()))?
        }

        let payload_len_from_header = u32::from_be_bytes(payload[..HEADER_LEN].try_into()?);
        let payload_len_actual = payload.len() - HEADER_LEN;
        if payload_len_actual != payload_len_from_header as usize {
            Err(TpLinkHs110Error::EncryptedPayloadLengthMismatch {
                payload_len_actual,
                payload_len_from_header,
            })?;
        }

        let mut key = 171;
        let decrypted: String = payload[HEADER_LEN..]
            .iter()
            .map(|byte| {
                let plain_char = (key ^ byte) as char;
                key = *byte;
                plain_char
            })
            .collect();

        Ok(decrypted)
    }

    /// Attempts to send a provided request to a smartplug, receive a response and represent it as
    /// as plaing text string (usually containing JSON).
    fn request<S>(&self, request: S) -> Result<String, TpLinkHs110Error>
    where
        S: AsRef<str>,
    {
        let mut stream = match self.timeout {
            None => net::TcpStream::connect(self.socket_addr)?,
            Some(duration) => {
                let stream = net::TcpStream::connect_timeout(&self.socket_addr, duration)?;
                stream.set_read_timeout(self.timeout)?;
                stream.set_write_timeout(self.timeout)?;
                stream
            }
        };

        stream.write_all(&Self::encrypt(request))?;
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

    /// Attempts to get a general info from/about a smartplug.
    ///
    /// In case of success a resulting JSON Value looks similar to this:
    /// ```text
    /// {
    ///   "system": {
    ///     "get_sysinfo": {
    ///       "active_mode": "schedule",
    ///       "alias": "Bathroom",
    ///       "dev_name": "Wi-Fi Smart Plug With Energy Monitoring",
    ///       "deviceId": "800644100000BB3AC70000FB15245D6C190F936B",
    ///       "err_code": 0,
    ///       "feature": "TIM:ENE",
    ///       "fwId": "00000000000000000000000000000000",
    ///       "hwId": "47E30DA8382497D2E82691B52A3B2EB3",
    ///       "hw_ver": "1.0",
    ///       "icon_hash": "",
    ///       "latitude": 47.782857,
    ///       "led_off": 0,
    ///       "longitude": 35.186122,
    ///       "mac": "70:4F:57:57:A1:14",
    ///       "model": "HS110(EU)",
    ///       "oemId": "4D345ECE299C0641C96E27CE2430548B",
    ///       "on_time": 8819452,
    ///       "relay_state": 1,
    ///       "rssi": -64,
    ///       "sw_ver": "1.2.6 Build 200727 Rel.120821",
    ///       "type": "IOT.SMARTPLUGSWITCH",
    ///       "updating": 0
    ///     }
    ///   }
    /// }
    /// ```
    pub fn info(&self) -> Result<Value, TpLinkHs110Error> {
        Ok(serde_json::from_str::<Value>(&self.request(
            json!({"system": {"get_sysinfo": {}}}).to_string(),
        )?)?)
    }

    /// Helper function which attempts to extract an object/field under specified hierarchical
    /// path in a JSON obtained with `get_sysinfo` command.
    fn info_field_value(&self, field: &'static str) -> Result<Value, TpLinkHs110Error> {
        self.info()?
            .extract_hierarchical(&["system", "get_sysinfo", field])
    }

    /// Attempts to get current LED state (which could be ON or OFF).
    pub fn led_state(&self) -> Result<LedState, TpLinkHs110Error> {
        Ok((self
            .info_field_value("led_off")?
            .as_u64()
            .ok_or(TpLinkHs110Error::UnexpectedValueRepresentation)?
            == 0)
            .into())
    }

    /// Attempts to switch LED to a specified state (i.e. turn it ON or turn it OFF).
    pub fn set_led_state(&self, led_state: LedState) -> Result<(), TpLinkHs110Error> {
        match serde_json::from_str::<Value>(
            &self.request(
                json!({"system": {"set_led_off": {"off": (led_state == LedState::Off) as u8 }}})
                    .to_string(),
            )?,
        )?
        .extract_hierarchical(&["system", "set_led_off", "err_code"])?
        .as_i64()
        .ok_or(TpLinkHs110Error::UnexpectedValueRepresentation)?
        {
            0 => Ok(()),
            err_code => Err(TpLinkHs110Error::SmartplugErrCode(err_code)),
        }
    }

    /// Attempts to obtain a smartplug name (alias). Name is given during smartplug initial setup,
    /// and it could be changed in companion app (Tapo or Kasa) on a mobile phone.
    pub fn hostname(&self) -> Result<String, TpLinkHs110Error> {
        Ok(self
            .info_field_value("alias")?
            .as_str()
            .ok_or(TpLinkHs110Error::UnexpectedValueRepresentation)?
            .to_string())
    }

    /// Attempts to obtain hardware version (hardware revision) of a smartplug.
    pub fn hw_version(&self) -> Result<HwVersion, TpLinkHs110Error> {
        match self
            .info_field_value("hw_ver")?
            .as_str()
            .ok_or(TpLinkHs110Error::UnexpectedValueRepresentation)?
        {
            "1.0" => Ok(HwVersion::Version1),
            "2.0" => Ok(HwVersion::Version2),
            other => Ok(HwVersion::Unsupported(other.into())),
        }
    }

    /// Attempts to get current power relay state. It is either smartplug powers connected device
    /// (ON) or not (OFF).
    pub fn power_state(&self) -> Result<PowerState, TpLinkHs110Error> {
        Ok((self
            .info_field_value("relay_state")?
            .as_u64()
            .ok_or(TpLinkHs110Error::UnexpectedValueRepresentation)?
            == 1)
            .into())
    }

    /// Attempts to switch power relay on or switch it off.
    pub fn set_power_state(&self, state: PowerState) -> Result<(), TpLinkHs110Error> {
        match serde_json::from_str::<Value>(
            &self.request(
                json!({"system": {"set_relay_state": {"state": (state == PowerState::On) as u8 }}})
                    .to_string(),
            )?,
        )?
        .extract_hierarchical(&["system", "set_relay_state", "err_code"])?
        .as_i64()
        .ok_or(TpLinkHs110Error::UnexpectedValueRepresentation)?
        {
            0 => Ok(()),
            err_code => Err(TpLinkHs110Error::SmartplugErrCode(err_code)),
        }
    }

    /// Attempts to get an information about smartplug connection to TP-Link cloud.
    ///
    /// In case of success resulting JSON Value looks similar to this:
    /// ```text
    /// Object {
    ///     "binded": Number(1),
    ///     "cld_connection": Number(1),
    ///     "err_code": Number(0),
    ///     "fwDlPage": String(""),
    ///     "fwNotifyType": Number(0),
    ///     "illegalType": Number(0),
    ///     "server": String("n-devs.tplinkcloud.com"),
    ///     "stopConnect": Number(0),
    ///     "tcspInfo": String(""),
    ///     "tcspStatus": Number(1),
    ///     "username": String("username@example.com"),
    /// }
    /// ```
    pub fn cloudinfo(&self) -> Result<Value, TpLinkHs110Error> {
        serde_json::from_str::<Value>(
            &self.request(json!({"cnCloud": {"get_info": {}}}).to_string())?,
        )?
        .extract_hierarchical(&["cnCloud", "get_info"])
    }

    /// Attempts to get an information about Wi-Fi access points which smartplug observes in a
    /// radio spectrum.
    /// The `refresh` boolean specifies whether it is necessary to perform scan of Wi-Fi spectrum
    /// (i.e. refresh the list of access points), or not.
    ///
    /// In case of success resulting JSON looks similar to the following:
    /// ```text
    /// Array [
    ///     Object {
    ///         "key_type": Number(3),
    ///         "ssid": String("MERCUSYS_1A04"),
    ///     },
    ///     Object {
    ///         "key_type": Number(3),
    ///         "ssid": String("RADIO"),
    ///     },
    ///     Object {
    ///         "key_type": Number(3),
    ///         "ssid": String("TP-Link_C1F3"),
    ///     },
    ///     Object {
    ///         "key_type": Number(2),
    ///         "ssid": String("ZyXEL_KEENEKTIC_LITE_76FAFB"),
    ///     },
    /// ],
    /// ```
    pub fn ap_list(&self, refresh: bool) -> Result<Value, TpLinkHs110Error> {
        serde_json::from_str::<Value>(
            &self.request(
                json!({"netif": {"get_scaninfo": {"refresh": refresh as u8}}}).to_string(),
            )?,
        )?
        .extract_hierarchical(&["netif", "get_scaninfo", "ap_list"])
    }

    /// Attempts to get values from smartplug's energy meter. Energy meter is present in HS110, and
    /// absent in HS100.
    ///
    /// In case of success resulting JSON looks like this:
    /// ```text
    /// Object {
    ///     "current": Number(0.027824),
    ///     "current_ma": Number(27.824),
    ///     "err_code": Number(0),
    ///     "power": Number(0.770242),
    ///     "power_mw": Number(770.242),
    ///     "total": Number(625.833),
    ///     "total_wh": Number(625833.0),
    ///     "voltage": Number(228.603726),
    ///     "voltage_mv": Number(228603.726),
    /// }
    /// ```
    pub fn emeter(&self) -> Result<Value, TpLinkHs110Error> {
        let mut emeter = serde_json::from_str::<Value>(
            &self.request(json!({"emeter":{"get_realtime":{}}}).to_string())?,
        )?
        .extract_hierarchical(&["emeter", "get_realtime"])?;

        // Smart plugs of HW version 1 and HW version 2 provide results via different JSON fields
        // and use different units.
        // I.e. one uses "voltage" in Volts and another "voltage_mv" in milliVolts.
        //
        // As it not clear which version is "better" or more widely used - calculate and provide
        // both fields for both hardware versions:
        #[rustfmt::skip]
        [
            ("voltage_mv", "voltage",    0.001f64),
            ("current_ma", "current",    0.001f64),
            ("power_mw",   "power",      0.001f64),
            ("total_wh",   "total",      0.001f64),
            ("voltage",    "voltage_mv", 1000f64),
            ("current",    "current_ma", 1000f64),
            ("power",      "power_mw",   1000f64),
            ("total",      "total_wh",   1000f64),
        ]
        .iter()
        .for_each(|(from, to, multiplier)| {
            if let Some(from) = emeter.get(from) {
                if emeter.get(to).is_none() {
                    emeter[to] = Value::from(from.as_f64().unwrap_or(0f64) * multiplier);
                }
            }
        });

        Ok(emeter)
    }

    /// Attempts to reboot a smartplug with an optional delay (in seconds).
    pub fn reboot(&self, delay: Option<u32>) -> Result<(), TpLinkHs110Error> {
        match serde_json::from_str::<Value>(
            &self.request(
                json!({"system": {"reboot": {"delay": delay.unwrap_or(0) }}}).to_string(),
            )?,
        )?
        .extract_hierarchical(&["system", "reboot", "err_code"])?
        .as_i64()
        .ok_or(TpLinkHs110Error::UnexpectedValueRepresentation)?
        {
            0 => Ok(()),
            err_code => Err(TpLinkHs110Error::SmartplugErrCode(err_code)),
        }
    }

    /// Attempts to perform a factory reset with an optional delay (in seconds).
    pub fn factory_reset(&self, delay: Option<u32>) -> Result<(), TpLinkHs110Error> {
        match serde_json::from_str::<Value>(
            &self.request(
                json!({"system": {"reset": {"delay": delay.unwrap_or(0) }}}).to_string(),
            )?,
        )?
        .extract_hierarchical(&["system", "reset", "err_code"])?
        .as_i64()
        .ok_or(TpLinkHs110Error::UnexpectedValueRepresentation)?
        {
            0 => Ok(()),
            err_code => Err(TpLinkHs110Error::SmartplugErrCode(err_code)),
        }
    }
}

trait ExtractHierarchical {
    fn extract_hierarchical(&self, path: &[&'static str]) -> Result<Value, TpLinkHs110Error>;
}

impl ExtractHierarchical for Value {
    /// Attempts to traverse hierarchical structure (JSON object) over provided path and returns
    /// corresponding sub-object/field.
    fn extract_hierarchical(&self, path: &[&'static str]) -> Result<Value, TpLinkHs110Error> {
        let mut current_object = self;
        for key in path {
            current_object =
                current_object
                    .get(key)
                    .ok_or_else(|| TpLinkHs110Error::KeyIsNotAvailable {
                        response: self.clone(),
                        key,
                    })?;
        }

        Ok(current_object.clone())
    }
}

/// Smartplug hardware version (hardware revision).
#[derive(Debug)]
pub enum HwVersion {
    Version1,
    Version2,
    Unsupported(String),
}

/// Smartplug's power relay state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PowerState {
    /// Power relay is ON, i.e. smartplug is powering its outlet (connected device).
    On,

    /// Power relay is OFF, i.e. there is no power on smartplug's outlet.
    Off,
}

impl Display for PowerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                PowerState::On => "ON",
                PowerState::Off => "OFF",
            }
        )
    }
}

impl Not for PowerState {
    type Output = PowerState;

    fn not(self) -> Self::Output {
        match self {
            PowerState::On => PowerState::Off,
            PowerState::Off => PowerState::On,
        }
    }
}

impl From<PowerState> for bool {
    fn from(value: PowerState) -> Self {
        match value {
            PowerState::On => true,
            PowerState::Off => false,
        }
    }
}

impl From<bool> for PowerState {
    fn from(value: bool) -> Self {
        match value {
            true => Self::On,
            false => Self::Off,
        }
    }
}

/// Smartplug LED indicator state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LedState {
    /// LED light is ON.
    On,

    /// LED light is OFF.
    Off,
}

impl Display for LedState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                LedState::On => "ON",
                LedState::Off => "OFF",
            }
        )
    }
}

impl Not for LedState {
    type Output = LedState;

    fn not(self) -> Self::Output {
        match self {
            LedState::On => LedState::Off,
            LedState::Off => LedState::On,
        }
    }
}

impl From<LedState> for bool {
    fn from(value: LedState) -> Self {
        match value {
            LedState::On => true,
            LedState::Off => false,
        }
    }
}

impl From<bool> for LedState {
    fn from(value: bool) -> Self {
        match value {
            true => Self::On,
            false => Self::Off,
        }
    }
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
    fn hostname() {
        let smartplug = HS110::new(&*TEST_TARGET_ADDR)
            .unwrap()
            .with_timeout(Duration::from_secs(3));
        assert!(smartplug.hostname().is_ok());

        let smartplug = HS110::new(&*TEST_TARGET_ADDR).unwrap();
        assert!(smartplug.hostname().is_ok());

        assert!(matches!(
            smartplug.hw_version(),
            Ok(HwVersion::Version1) | Ok(HwVersion::Version2)
        ));
    }

    #[test]
    fn switch_led_on_off() {
        let smartplug = HS110::new(&*TEST_TARGET_ADDR).unwrap();

        let original_state = smartplug.led_state().expect("failed to obtain LED state");

        assert!(smartplug.set_led_state(!original_state).is_ok());
        assert_eq!(
            smartplug.led_state().expect("failed to obtain LED state"),
            !original_state
        );

        assert!(smartplug.set_led_state(original_state).is_ok());
        assert_eq!(
            smartplug.led_state().expect("failed to obtain LED state"),
            original_state
        );
    }

    #[test]
    #[serial]
    #[ignore = "power-cycles devices connected to the plug"]
    fn switch_power_on_off() {
        let smartplug = HS110::new(&*TEST_TARGET_ADDR).unwrap();

        let original_state = smartplug
            .power_state()
            .expect("failed to obtain smartplug power state");

        assert!(smartplug.set_power_state(!original_state).is_ok());
        assert_eq!(
            smartplug
                .power_state()
                .expect("failed to obtain smartplug power state"),
            !original_state
        );

        assert!(smartplug.set_power_state(original_state).is_ok());
        assert_eq!(
            smartplug
                .power_state()
                .expect("failed to obtain smartplug power state"),
            original_state
        );
    }

    #[test]
    fn get_cloudinfo() {
        assert!(HS110::new(&*TEST_TARGET_ADDR).unwrap().cloudinfo().is_ok());
    }

    #[test]
    #[serial]
    fn access_points_list_and_scan() {
        let smartplug = HS110::new(&*TEST_TARGET_ADDR).unwrap();

        smartplug
            .ap_list(false)
            .expect("failed to obtain AP list")
            .as_array()
            .expect("json array is expected");

        assert!(
            !smartplug
                .ap_list(true)
                .expect("failed to obtain AP list")
                .as_array()
                .expect("json array is expected")
                .is_empty(),
            "list of access points is not expected to be empty"
        );
    }

    #[test]
    #[serial]
    #[ignore = "power-cycles devices connected to the plug"]
    fn reboot() {
        let hs110 = HS110::new(&*TEST_TARGET_ADDR).unwrap();
        assert!(hs110.reboot(None).is_ok());

        let hs110 = hs110.with_timeout(Duration::from_secs(1));
        assert!(
            hs110.reboot(Some(1)).is_err(),
            "device is expected to be unreachable right after reboot command"
        );

        // Wait till the device is back online after reboot.
        let hs110 = hs110.with_timeout(Duration::from_secs(10));
        for _ in 0..20 {
            if hs110.hostname().is_ok() {
                return;
            }
        }
        panic!("device didn't back online after reboot");
    }
}
