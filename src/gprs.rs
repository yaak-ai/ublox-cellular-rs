use crate::GSMClient;
use embedded_hal::digital::v2::OutputPin;

// use crate::command::*;

use heapless::{consts, String};

#[derive(Debug)]
pub enum Error {
    ATError(atat::Error),
}

pub struct APNInfo {
    pub apn: String<consts::U128>,
    pub user_name: Option<String<consts::U128>>,
    pub password: Option<String<consts::U128>>,
}

impl APNInfo {
    pub fn new(apn: &str) -> Self {
        APNInfo {
            apn: String::from(apn),
            user_name: None,
            password: None,
        }
    }
}

pub trait GPRS {
    fn attach_gprs(&mut self, apn_info: APNInfo) -> Result<(), Error>;
    fn detach_gprs(&mut self) -> Result<(), Error>;
}

impl<C, RST, DTR> GPRS for GSMClient<C, RST, DTR>
where
    C: atat::ATATInterface,
    RST: OutputPin,
    DTR: OutputPin,
{
    fn attach_gprs(&mut self, _apn_info: APNInfo) -> Result<(), Error> {
        // Attach GPRS
        // self.send_at(Command::SetGPRSAttached { state: true })?;

        // Set APN info
        // self.send_at(Command::SetPacketSwitchedConfig {
        //     profile_id: 0,
        //     param: PacketSwitchedParam::APN(apn_info.apn),
        // })?;

        // // Set auth mode
        // // Set username
        // if let Some(user_name) = apn_info.user_name {
        //     self.send_at(Command::SetPacketSwitchedConfig {
        //         profile_id: 0,
        //         param: PacketSwitchedParam::Username(user_name),
        //     })?;
        // }

        // // Set password
        // if let Some(password) = apn_info.password {
        //     self.send_at(Command::SetPacketSwitchedConfig {
        //         profile_id: 0,
        //         param: PacketSwitchedParam::Password(password),
        //     })?;
        // }

        // // Set dynamic IP

        // // Activate IP
        // self.send_at(Command::SetPacketSwitchedAction {
        //     profile_id: 0,
        //     action: PacketSwitchedAction::Activate,
        // })?;

        // // Check profile status
        // self.send_at(Command::GetPacketSwitchedNetworkData {
        //     profile_id: 0,
        //     param: PacketSwitchedNetworkDataParam::PsdProfileStatus,
        // })?;

        Ok(())
    }

    fn detach_gprs(&mut self) -> Result<(), Error> {
        // // Deactivate IP
        // self.send_at(Command::SetPacketSwitchedAction {
        //     profile_id: 0,
        //     action: PacketSwitchedAction::Deactivate,
        // })?;

        // // Detach from network
        // self.send_at(Command::SetGPRSAttached { state: false })?;

        Ok(())
    }
}

impl From<atat::Error> for Error {
    fn from(e: atat::Error) -> Self {
        Error::ATError(e)
    }
}
