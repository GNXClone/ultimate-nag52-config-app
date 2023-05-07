use core::fmt;
use std::{
    borrow::{Borrow, BorrowMut},
    sync::{Arc, Mutex, RwLock, mpsc::{Receiver, self}},
};

use ecu_diagnostics::hardware::{
    passthru::*, Hardware, HardwareError, HardwareInfo, HardwareResult, HardwareScanner,
};
use ecu_diagnostics::{
    channel::*,
    dynamic_diag::{
        DiagProtocol, DiagServerAdvancedOptions, DiagServerBasicOptions, DiagSessionMode,
        DynamicDiagSession, TimeoutConfig,
    },
};
use ecu_diagnostics::{kwp2000::*, DiagServerResult};

#[cfg(unix)]
use ecu_diagnostics::hardware::socketcan::{SocketCanDevice, SocketCanScanner};

use crate::hw::{
    usb::{EspLogMessage, Nag52USB},
    usb_scanner::Nag52UsbScanner,
};

pub mod flash;
pub mod ident;
pub mod settings;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AdapterType {
    USB,
    Passthru,
    #[cfg(unix)]
    SocketCAN,
}

#[derive(Debug, Clone)]
pub enum DataState<T> {
    LoadOk(T),
    Unint,
    LoadErr(String)
}

impl<T> DataState<T> {
    pub fn is_ok(&self) -> bool {
        matches!(self, Self::LoadOk(_))
    }

    pub fn get_err(&self) -> String {
        match self {
            DataState::LoadOk(_) => "".into(),
            DataState::Unint => "Uninitialized".into(),
            DataState::LoadErr(e) => e.clone(),
        }
    }
}

#[derive(Clone)]
pub enum AdapterHw {
    Usb(Arc<Mutex<Nag52USB>>),
    Passthru(Arc<Mutex<PassthruDevice>>),
    #[cfg(unix)]
    SocketCAN(Arc<Mutex<SocketCanDevice>>),
}

impl fmt::Debug for AdapterHw {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usb(_) => f.debug_tuple("Usb").finish(),
            Self::Passthru(_) => f.debug_tuple("Passthru").finish(),
            #[cfg(unix)]
            Self::SocketCAN(_) => f.debug_tuple("SocketCAN").finish(),
        }
    }
}

impl AdapterHw {
    pub fn try_connect(info: &HardwareInfo, ty: AdapterType) -> HardwareResult<Self> {
        Ok(match ty {
            AdapterType::USB => Self::Usb(Nag52USB::try_connect(info)?),
            AdapterType::Passthru => Self::Passthru(PassthruDevice::try_connect(info)?),
            #[cfg(unix)]
            AdapterType::SocketCAN => Self::SocketCAN(SocketCanDevice::try_connect(info)?),
        })
    }

    fn get_type(&self) -> AdapterType {
        match self {
            Self::Usb(_) => AdapterType::USB,
            Self::Passthru(_) => AdapterType::Passthru,
            #[cfg(unix)]
            Self::SocketCAN(_) => AdapterType::SocketCAN,
        }
    }

    pub fn create_isotp_channel(&self) -> HardwareResult<Box<dyn IsoTPChannel>> {
        match self {
            Self::Usb(u) => Hardware::create_iso_tp_channel(u.clone()),
            Self::Passthru(p) => Hardware::create_iso_tp_channel(p.clone()),
            #[cfg(unix)]
            Self::SocketCAN(s) => Hardware::create_iso_tp_channel(s.clone()),
        }
    }

    pub fn get_hw_info(&self) -> HardwareInfo {
        match self {
            Self::Usb(u) => u.lock().unwrap().get_info().clone(),
            Self::Passthru(p) => p.lock().unwrap().get_info().clone(),
            #[cfg(unix)]
            Self::SocketCAN(s) => s.lock().unwrap().get_info().clone(),
        }
    }
}
pub trait Nag52Endpoint: Hardware {
    fn read_log_message(this: Arc<Mutex<Self>>) -> Arc<Option<Receiver<EspLogMessage>>>;
    fn is_connected(&self) -> bool;
    fn try_connect(info: &HardwareInfo) -> HardwareResult<Arc<Mutex<Self>>>;
    fn get_device_desc(this: Arc<Mutex<Self>>) -> String;
}

#[cfg(unix)]
impl Nag52Endpoint for SocketCanDevice {
    fn read_log_message(_this: Arc<Mutex<Self>>) -> Arc<Option<Receiver<EspLogMessage>>> {
        Arc::new(None)
    }

    fn is_connected(&self) -> bool {
        self.is_iso_tp_channel_open()
    }

    fn try_connect(info: &HardwareInfo) -> HardwareResult<Arc<Mutex<Self>>> {
        SocketCanScanner::new().open_device_by_name(&info.name)
    }

    fn get_device_desc(this: Arc<Mutex<Self>>) -> String {
        this.lock().unwrap().get_info().name.clone()
    }
}

impl Nag52Endpoint for PassthruDevice {
    fn read_log_message(_this: Arc<Mutex<Self>>) -> Arc<Option<Receiver<EspLogMessage>>> {
        Arc::new(None)
    }

    fn is_connected(&self) -> bool {
        self.is_iso_tp_channel_open()
    }

    fn try_connect(info: &HardwareInfo) -> HardwareResult<Arc<Mutex<Self>>> {
        PassthruScanner::new().open_device_by_name(&info.name)
    }

    fn get_device_desc(this: Arc<Mutex<Self>>) -> String {
        this.lock().unwrap().get_info().name.clone()
    }
}

impl Nag52Endpoint for Nag52USB {
    fn read_log_message(this: Arc<Mutex<Self>>) -> Arc<Option<Receiver<EspLogMessage>>> {
        this.lock().unwrap().consume_log_receiver()
    }

    fn is_connected(&self) -> bool {
        self.is_connected()
    }

    fn try_connect(info: &HardwareInfo) -> HardwareResult<Arc<Mutex<Self>>> {
        Nag52UsbScanner::new().open_device_by_name(&info.name)
    }

    fn get_device_desc(this: Arc<Mutex<Self>>) -> String {
        let info_name = this.lock().unwrap().get_info().name.clone();
        format!("Ultimate-NAG52 USB on {}", info_name)
    }
}

#[derive(Debug, Clone)]
pub struct Nag52Diag {
    info: HardwareInfo,
    endpoint: Option<AdapterHw>,
    endpoint_type: AdapterType,
    server: Option<Arc<DynamicDiagSession>>,
    log_receiver: Arc<Option<Receiver<EspLogMessage>>>
}

unsafe impl Sync for Nag52Diag {}
unsafe impl Send for Nag52Diag {}

impl Nag52Diag {
    pub fn new(hw: AdapterHw) -> DiagServerResult<Self> {

        let mut channel_cfg = IsoTPSettings {
            block_size: 0,
            st_min: 0,
            extended_addresses: None,
            pad_frame: true,
            can_speed: 500_000,
            can_use_ext_addr: false,
        };

        #[cfg(unix)]
        if let AdapterHw::SocketCAN(_) = hw {
            channel_cfg.block_size = 8;
            channel_cfg.st_min = 0x20;
        }

        let basic_opts = DiagServerBasicOptions {
            send_id: 0x07E1,
            recv_id: 0x07E9,
            timeout_cfg: TimeoutConfig {
                read_timeout_ms: 10000,
                write_timeout_ms: 10000,
            },
        };

        let adv_opts = DiagServerAdvancedOptions {
            global_tp_id: 0,
            tester_present_interval_ms: 2000,
            tester_present_require_response: true,
            global_session_control: false,
            tp_ext_id: None,
            command_cooldown_ms: 0,
        };

        let mut protocol = Kwp2000Protocol::default();
        protocol.register_session_type(DiagSessionMode {
            id: 0x93,
            tp_require: true,
            name: "UN52DevMode".into(),
        });

        let kwp = DynamicDiagSession::new_over_iso_tp(
            protocol,
            hw.create_isotp_channel()?,
            channel_cfg,
            basic_opts,
            Some(adv_opts),
        )?;

        let logger = if let AdapterHw::Usb(usb) = &hw {
            usb.lock().unwrap().consume_log_receiver()
        } else {
            Arc::new(None)
        };

        Ok(Self {
            info: hw.get_hw_info(),
            endpoint_type: hw.get_type(),
            endpoint: Some(hw),
            server: Some(Arc::new(kwp)),
            log_receiver: logger,
        })
    }

    pub fn try_reconnect(&mut self) -> DiagServerResult<()> {
        {
            let _ = self.server.take();
            let _ = self.endpoint.take();
        }
        // Now try to reconnect

        println!("Trying to find {}", self.info.name);
        let dev = AdapterHw::try_connect(&self.info, self.endpoint_type)?;
        *self = Self::new(dev)?;
        Ok(())
    }

    pub fn with_kwp<F, X>(&self, mut kwp_fn: F) -> DiagServerResult<X>
    where
        F: FnMut(&DynamicDiagSession) -> DiagServerResult<X>,
    {
        match self.server.borrow() {
            None => Err(HardwareError::DeviceNotOpen.into()),
            Some(s) => kwp_fn(&s),
        }
    }

    pub fn can_read_log(&self) -> bool {
        self.log_receiver.is_some()
    }

    pub fn read_log_msg(&self) -> Option<EspLogMessage> {
        if let Some(receiver) = &self.log_receiver.borrow() {
            receiver.try_recv().ok()
        } else {
            None
        }
    }

}

#[cfg(test)]
pub mod test_diag {
    use ecu_diagnostics::{hardware::HardwareScanner, DiagError};

    use crate::{diag::AdapterHw, hw::usb_scanner::Nag52UsbScanner};

    use super::Nag52Diag;

    #[ignore]
    #[test]
    pub fn test_kwp_reconnect() {
        let scanner = Nag52UsbScanner::new();
        let dev = scanner.open_device_by_name("/dev/ttyUSB0").unwrap();
        let mut kwp = match Nag52Diag::new(AdapterHw::Usb(dev)) {
            Ok(kwp) => kwp,
            Err(e) => {
                eprintln!("Error starting KWP {e}");
                return;
            }
        };
        println!("{:?}", kwp.query_ecu_data());
        println!("Please unplug NAG");
        std::thread::sleep(std::time::Duration::from_millis(5000));
        let failable = kwp.with_kwp(|k| k.kwp_read_daimler_identification());
        assert!(failable.is_err());
        println!("{:?}", failable);
        let e = failable.err().unwrap();
        if let DiagError::ECUError { code: _, def: _ } = e {
        } else {
            for i in 0..5 {
                println!("Reconnect attempt {}/5", i + 1);
                match kwp.try_reconnect() {
                    Ok(_) => {
                        println!("Reconnect OK!");
                        break;
                    }
                    Err(e) => {
                        println!("Reconnect failed! {e}!");
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(2000));
            }
        }
        let must_ok = kwp.with_kwp(|k| k.kwp_read_daimler_identification());
        assert!(must_ok.is_ok());
    }
}
