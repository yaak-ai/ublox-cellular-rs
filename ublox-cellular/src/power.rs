use atat::blocking::AtatClient;
use embedded_hal::digital::{InputPin, OutputPin};
use fugit::{ExtU32, MillisDurationU32};

use crate::{
    client::Device,
    command::{
        mobile_control::{
            types::{Functionality, ResetMode},
            ModuleSwitchOff, SetModuleFunctionality,
        },
        system_features::{
            types::{FSFactoryRestoreType, NVMFactoryRestoreType},
            SetFactoryConfiguration,
        },
        AT,
    },
    error::{from_clock, Error, GenericError},
    module_timing::{pwr_off_time, pwr_on_time, reset_time},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum PowerState {
    Off,
    On,
}

impl<C, CLK, RST, DTR, PWR, VINT, const TIMER_HZ: u32, const N: usize, const L: usize>
    Device<C, CLK, RST, DTR, PWR, VINT, TIMER_HZ, N, L>
where
    C: AtatClient,
    CLK: fugit_timer::Timer<TIMER_HZ>,
    RST: OutputPin,
    PWR: OutputPin,
    DTR: OutputPin,
    VINT: InputPin,
{
    /// Check that the cellular module is alive.
    ///
    /// See if the cellular module is responding at the AT interface by poking
    /// it with "AT" up to `attempts` times, waiting 1 second for an "OK"
    /// response each time
    pub(crate) fn is_alive(&mut self, attempts: u8) -> Result<(), Error> {
        let mut error = Error::BaudDetection;
        for _ in 0..attempts {
            match self.network.at_tx.send_ignore_timeout(&AT) {
                Ok(_) => {
                    return Ok(());
                }
                Err(e) => error = e.into(),
            };
        }
        Err(error)
    }

    /// Perform at full factory reset of the module, clearing all NVM sectors in the process
    pub fn factory_reset(&mut self) -> Result<(), Error> {
        self.network.send_internal(
            &SetFactoryConfiguration {
                fs_op: FSFactoryRestoreType::AllFiles,
                nvm_op: NVMFactoryRestoreType::NVMFlashSectors,
            },
            false,
        )?;

        info!("Successfully factory reset modem!");

        if self.soft_reset(true).is_err() {
            self.hard_reset()?;
        }

        Ok(())
    }

    /// Reset the module by sending AT CFUN command
    pub(crate) fn soft_reset(&mut self, sim_reset: bool) -> Result<(), Error> {
        trace!(
            "Attempting to soft reset of the modem with sim reset: {}.",
            sim_reset
        );

        let fun = {
            #[cfg(any(
                feature = "toby-l2",
                feature = "mpci-l2",
                feature = "lisa-u1",
                feature = "lisa-u2",
                feature = "sara-u2",
                feature = "toby-r2",
                feature = "lara-r2",
                feature = "lara-r6",
                feature = "toby-l4",
                feature = "leon-g1",
                feature = "sara-g3",
                feature = "sara-g4"
            ))]
            if sim_reset {
                Functionality::SilentResetWithSimReset
            } else {
                Functionality::SilentReset
            }
            #[cfg(not(any(
                feature = "toby-l2",
                feature = "mpci-l2",
                feature = "lisa-u1",
                feature = "lisa-u2",
                feature = "sara-u2",
                feature = "toby-r2",
                feature = "lara-r2",
                feature = "lara-r6",
                feature = "toby-l4",
                feature = "leon-g1",
                feature = "sara-g3",
                feature = "sara-g4"
            )))]
            {
                if sim_reset {
                    warn!("Sim reset is not available on this device!");
                }
                Functionality::SilentResetWithSimReset
            }
        };

        self.network.send_internal(
            &SetModuleFunctionality {
                fun,
                // SARA-R5 This parameter can be used only when <fun> is 1, 4 or 19
                #[cfg(feature = "sara-r5")]
                rst: None,
                #[cfg(not(feature = "sara-r5"))]
                rst: Some(ResetMode::DontReset),
            },
            false,
        )?;

        self.wait_power_state(PowerState::On, 30_000.millis())
            .map_err(from_clock)?;

        Ok(())
    }

    /// Reset the module by driving it's `RESET_N` pin low for 50 ms
    ///
    /// **NOTE** This function will reset NVM settings!
    pub fn hard_reset(&mut self) -> Result<(), Error> {
        trace!("Attempting to hard reset of the modem.");
        if let Some(ref mut rst) = self.config.rst_pin {
            rst.set_low().ok();

            self.network
                .status
                .timer
                .start(reset_time::<TIMER_HZ>())
                .map_err(from_clock)?;
            nb::block!(self.network.status.timer.wait()).map_err(from_clock)?;

            rst.set_high().ok();
            self.network
                .status
                .timer
                .start(5.secs())
                .map_err(from_clock)?;
            nb::block!(self.network.status.timer.wait()).map_err(from_clock)?;
        }

        self.power_state = PowerState::Off;

        self.power_on()?;

        Ok(())
    }

    pub fn power_on(&mut self) -> Result<(), Error> {
        info!(
            "Attempting to power on the modem with PWR_ON pin: {} and VInt pin: {}.",
            self.config.pwr_pin.is_some(),
            self.config.vint_pin.is_some(),
        );

        if self.power_state()? != PowerState::On {
            trace!("Powering modem on.");
            match self.config.pwr_pin {
                // Apply Low pulse on PWR_ON for 50 microseconds to power on
                Some(ref mut pwr) => {
                    pwr.set_low().ok();
                    self.network
                        .status
                        .timer
                        .start(pwr_on_time::<TIMER_HZ>())
                        .map_err(from_clock)?;
                    nb::block!(self.network.status.timer.wait()).map_err(from_clock)?;

                    pwr.set_high().ok();
                    self.network
                        .status
                        .timer
                        .start(1.secs())
                        .map_err(from_clock)?;
                    nb::block!(self.network.status.timer.wait()).map_err(from_clock)?;

                    if let Err(e) = self.wait_power_state(PowerState::On, 10.secs()) {
                        error!("Failed to power on modem");
                        return Err(e);
                    } else {
                        trace!("Modem powered on");
                    }
                }
                _ => {
                    // Software restart
                    if self.soft_reset(false).is_err() {
                        self.hard_reset()?;
                    }
                }
            }
        } else {
            debug!("module is already on");
        }
        Ok(())
    }

    pub fn soft_power_off(&mut self) -> Result<(), Error> {
        trace!("Attempting to soft power off the modem.");

        self.network.send_internal(&ModuleSwitchOff, false)?;

        self.power_state = PowerState::Off;
        trace!("Modem powered off");

        self.network
            .status
            .timer
            .start(10.secs())
            .map_err(from_clock)?;
        nb::block!(self.network.status.timer.wait()).map_err(from_clock)?;

        Ok(())
    }

    pub fn hard_power_off(&mut self) -> Result<(), Error> {
        trace!("Attempting to hard power off the modem.");

        if self.power_state()? == PowerState::On {
            match self.config.pwr_pin {
                Some(ref mut pwr) => {
                    // Apply Low pulse on PWR_ON >= 1 second to power off
                    pwr.set_low().ok();
                    self.network
                        .status
                        .timer
                        .start(pwr_off_time::<TIMER_HZ>())
                        .map_err(from_clock)?;
                    nb::block!(self.network.status.timer.wait()).map_err(from_clock)?;

                    pwr.set_high().ok();
                    self.power_state = PowerState::Off;
                    trace!("Modem powered off");
                }
                _ => {
                    return Err(Error::Generic(GenericError::Unsupported));
                }
            }
        }

        Ok(())
    }

    /// Check the power state of the module, by probing `Vint` pin if available,
    /// fallbacking to checking for AT responses through `is_alive`
    pub fn power_state(&mut self) -> Result<PowerState, Error> {
        match self.config.vint_pin {
            Some(ref mut vint) => {
                if vint
                    .is_high()
                    .map_err(|_| Error::Generic(GenericError::Unsupported))?
                {
                    Ok(PowerState::On)
                } else {
                    Ok(PowerState::Off)
                }
            }
            _ => Ok(self.is_alive(2).map_or(PowerState::Off, |_| PowerState::On)),
        }
    }

    /// Wait for the power state to change into `expected`, with a timeout
    fn wait_power_state(
        &mut self,
        expected: PowerState,
        timeout: MillisDurationU32,
    ) -> Result<(), Error> {
        let start = self.network.status.timer.now();

        let mut res = false;

        trace!("Waiting for the modem to reach {:?}.", expected);
        while self
            .network
            .status
            .timer
            .now()
            .checked_duration_since(start)
            .map_or(false, |dur| dur < timeout)
        {
            if self.power_state()? == expected {
                res = true;
                break;
            }

            self.network
                .status
                .timer
                .start(5.millis())
                .map_err(from_clock)?;
            nb::block!(self.network.status.timer.wait()).map_err(from_clock)?;
        }

        if res {
            trace!("Success.");
            Ok(())
        } else {
            error!("Modem never reach {:?}.", expected);
            Err(Error::Generic(GenericError::Timeout))
        }
    }
}
