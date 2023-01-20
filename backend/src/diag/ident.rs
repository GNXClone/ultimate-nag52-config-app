use ecu_diagnostics::{DiagServerResult, kwp2000::DaimlerEcuIdent};

use super::{Nag52Diag, Nag52Endpoint};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EgsMode {
    EGS51,
    EGS52,
    EGS53,
    Unknown(u16)
}

impl From<u16> for EgsMode {
    fn from(diag_var_code: u16) -> Self {
        match diag_var_code {
            0x0251 => Self::EGS51,
            0x0252 => Self::EGS51,
            0x0253 => Self::EGS51,
            _ => Self::Unknown(diag_var_code)
        }
    }
}

impl ToString for EgsMode {
    fn to_string(&self) -> String {
        match self {
            EgsMode::EGS51 => "EGS51".to_string(),
            EgsMode::EGS52 => "EGS52".to_string(),
            EgsMode::EGS53 => "EGS53".to_string(),
            EgsMode::Unknown(x) => format!("Unknown(0x{:08X})", x),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PCBVersion {
    OnePointOne,
    OnePointTwo,
    OnePointThree,
    Unknown
}

impl PCBVersion {
    fn from_date(w: u32, y: u32) -> Self {
        if w == 49 && y == 21 {
            Self::OnePointOne
        } else if w == 27 && y == 22 {
            Self::OnePointTwo
        } else if w == 49 && y == 22 {
            Self::OnePointThree
        } else {
            Self::Unknown
        }
    }
}

impl ToString for PCBVersion {
    fn to_string(&self) -> String {
        match self {
            PCBVersion::OnePointOne => "V1.1",
            PCBVersion::OnePointTwo => "V1.2",
            PCBVersion::OnePointThree => "V1.3",
            PCBVersion::Unknown => "V_NDEF",
        }.to_string()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct IdentData {
    pub egs_mode: EgsMode,
    pub board_ver: PCBVersion,

    pub manf_day: u32,
    pub manf_month: u32,
    pub manf_year: u32,

    pub hw_week: u32,
    pub hw_year: u32,

    pub sw_week: u32,
    pub sw_year: u32
}

fn bcd_decode_to_int(u: u8) -> u32 {
    10 * (u as u32 / 16) + (u as u32 % 16)
}

impl Nag52Diag where {
    pub fn query_ecu_data(&mut self) -> DiagServerResult<IdentData> {
        self.with_kwp(|k| {
            let ident = k.read_daimler_identification()?;
            Ok(IdentData {
                egs_mode: EgsMode::from(ident.diag_info.get_info_id()),
                board_ver: PCBVersion::from_date(bcd_decode_to_int(ident.ecu_hw_build_week), bcd_decode_to_int(ident.ecu_hw_build_year)),
                manf_day: bcd_decode_to_int(ident.ecu_production_day),
                manf_month: bcd_decode_to_int(ident.ecu_production_month),
                manf_year: bcd_decode_to_int(ident.ecu_production_year),
                hw_week: bcd_decode_to_int(ident.ecu_hw_build_week),
                hw_year: bcd_decode_to_int(ident.ecu_hw_build_year),
                sw_week: bcd_decode_to_int(ident.ecu_sw_build_week),
                sw_year: bcd_decode_to_int(ident.ecu_sw_build_year),
            })
        })
    }
}