//! A simple Driver for the Waveshare 2.7" B Tri-Color E-Ink Display via SPI
//!
//! [Documentation](https://www.waveshare.com/wiki/2.7inch_e-Paper_HAT_(B))

use embedded_hal::{
  blocking::{delay::*, spi::Write},
  digital::v2::*,
};

use crate::interface::DisplayInterface;
use crate::traits::{
  InternalWiAdditions, RefreshLut, WaveshareDisplay,
};

// The Lookup Tables for the Display
mod constants;
use crate::epd2in7::constants::*;

/// Width of the display
pub const WIDTH: u32 = 176;
/// Height of the display
pub const HEIGHT: u32 = 264;
/// Default Background Color
pub const DEFAULT_BACKGROUND_COLOR: Color = Color::White;
const IS_BUSY_LOW: bool = true;

use crate::color::Color;

pub(crate) mod command;
use self::command::Command;

#[cfg(feature = "graphics")]
mod graphics;
#[cfg(feature = "graphics")]
pub use self::graphics::Display2in7;

/// Epd2in7 driver
pub struct Epd2in7<SPI, CS, BUSY, DC, RST, DELAY> {
  /// Connection Interface
  interface: DisplayInterface<SPI, CS, BUSY, DC, RST, DELAY>,
  /// Background Color
  color: Color,
}

impl<SPI, CS, BUSY, DC, RST, DELAY> InternalWiAdditions<SPI, CS, BUSY, DC, RST, DELAY>
  for Epd2in7<SPI, CS, BUSY, DC, RST, DELAY>
where
  SPI: Write<u8>,
  CS: OutputPin,
  BUSY: InputPin,
  DC: OutputPin,
  RST: OutputPin,
  DELAY: DelayMs<u8>,
{
  fn init(&mut self, spi: &mut SPI, delay: &mut DELAY) -> Result<(), SPI::Error> {
      // reset the device
      self.interface.reset(delay, 2);

      // set the power settings
      self.interface.cmd_with_data(
          spi,
          Command::PowerSetting,
          &[0x03, 0x00, 0x2b, 0x2b, 0x09],
      )?;

      // start the booster
      self.interface
          .cmd_with_data(spi, Command::BoosterSoftStart, &[0x07, 0x07, 0x17])?;

      // power optimization
      self.interface
          .cmd_with_data(spi, Command::PowerOptimization, &[0x60, 0xa5])?;
      self.interface
          .cmd_with_data(spi, Command::PowerOptimization, &[0x89, 0xa5])?;
      self.interface
          .cmd_with_data(spi, Command::PowerOptimization, &[0x90, 0x00])?;
      self.interface
          .cmd_with_data(spi, Command::PowerOptimization, &[0x93, 0x2a])?;
      self.interface
          .cmd_with_data(spi, Command::PowerOptimization, &[0xA0, 0xA5])?;
      self.interface
          .cmd_with_data(spi, Command::PowerOptimization, &[0xA1, 0x00])?;
      self.interface
          .cmd_with_data(spi, Command::PowerOptimization, &[0x73, 0x41])?;

      self.interface
          .cmd_with_data(spi, Command::PartialDisplayRefresh, &[0x00])?;

      // power on
      self.interface.cmd(spi, Command::PowerOn)?;
      self.wait_until_idle(spi)?;

      // set panel settings, 0xbf is bw, 0xaf is multi-color
      self.interface
          .cmd_with_data(spi, Command::PanelSetting, &[0xaf])?;

      // pll control
      self.interface
          .cmd_with_data(spi, Command::PllControl, &[0x3a])?;

      self.interface
          .cmd_with_data(spi, Command::VcmDcSetting, &[0x12])?;

      // self.interface
      //     .cmd_with_data(spi, Command::VcomAndDataIntervalSetting, &[0x87])?;

      self.set_lut(spi, None)?;
      Ok(())
  }
}

impl<SPI, CS, BUSY, DC, RST, DELAY> WaveshareDisplay<SPI, CS, BUSY, DC, RST, DELAY>
  for Epd2in7<SPI, CS, BUSY, DC, RST, DELAY>
where
  SPI: Write<u8>,
  CS: OutputPin,
  BUSY: InputPin,
  DC: OutputPin,
  RST: OutputPin,
  DELAY: DelayMs<u8>,
{
  type DisplayColor = Color;
  fn new(
      spi: &mut SPI,
      cs: CS,
      busy: BUSY,
      dc: DC,
      rst: RST,
      delay: &mut DELAY,
  ) -> Result<Self, SPI::Error> {
      let interface = DisplayInterface::new(cs, busy, dc, rst);
      let color = DEFAULT_BACKGROUND_COLOR;

      let mut epd = Epd2in7 { interface, color };

      epd.init(spi, delay)?;

      Ok(epd)
  }

  fn wake_up(&mut self, spi: &mut SPI, delay: &mut DELAY) -> Result<(), SPI::Error> {
      self.init(spi, delay)
  }

  fn sleep(&mut self, spi: &mut SPI, _delay: &mut DELAY) -> Result<(), SPI::Error> {
      self.wait_until_idle(spi)?;
      self.interface
          .cmd_with_data(spi, Command::VcomAndDataIntervalSetting, &[0xf7])?;

      self.interface.cmd(spi, Command::PowerOff)?;
      self.wait_until_idle(spi)?;
      self.interface
          .cmd_with_data(spi, Command::DeepSleep, &[0xA5])?;
      Ok(())
  }

  fn update_frame(
      &mut self,
      spi: &mut SPI,
      buffer: &[u8],
      _delay: &mut DELAY,
  ) -> Result<(), SPI::Error> {
      self.interface.cmd_with_data(spi, Command::DataStartTransmission2, buffer)?;
      Ok(())
  }

  fn update_partial_frame(
      &mut self,
      spi: &mut SPI,
      buffer: &[u8],
      x: u32,
      y: u32,
      width: u32,
      height: u32,
  ) -> Result<(), SPI::Error> {
      self.interface
          .cmd(spi, Command::PartialDataStartTransmission1)?;

      self.interface.data(spi, &[(x >> 8) as u8])?;
      self.interface.data(spi, &[(x & 0xf8) as u8])?;
      self.interface.data(spi, &[(y >> 8) as u8])?;
      self.interface.data(spi, &[(y & 0xff) as u8])?;
      self.interface.data(spi, &[(width >> 8) as u8])?;
      self.interface.data(spi, &[(width & 0xf8) as u8])?;
      self.interface.data(spi, &[(height >> 8) as u8])?;
      self.interface.data(spi, &[(height & 0xff) as u8])?;

      self.interface.data(spi, buffer)?;

      self.interface.cmd(spi, Command::DisplayRefresh)?;

      self.wait_until_idle(spi)?;
      Ok(())
  }

  fn display_frame(&mut self, spi: &mut SPI, _delay: &mut DELAY) -> Result<(), SPI::Error> {
      self.interface.cmd(spi, Command::DisplayRefresh)?;
      self.wait_until_idle(spi)?;
      Ok(())
  }

  fn update_and_display_frame(
      &mut self,
      spi: &mut SPI,
      buffer: &[u8],
      delay: &mut DELAY,
  ) -> Result<(), SPI::Error> {
      self.update_frame(spi, buffer, delay)?;
      self.display_frame(spi, delay)?;
      Ok(())
  }

  fn clear_frame(&mut self, spi: &mut SPI, _delay: &mut DELAY) -> Result<(), SPI::Error> {
      let color_value = self.color.get_byte_value();
      self.interface.cmd(spi, Command::DataStartTransmission1)?;
      self.interface
          .data_x_times(spi, color_value, WIDTH * HEIGHT / 8)?;

      // self.interface.cmd(spi, Command::DataStop)?;

      self.interface.cmd(spi, Command::DataStartTransmission2)?;
      self.interface
          .data_x_times(spi, color_value, WIDTH * HEIGHT / 8)?;
      // self.interface.cmd(spi, Command::DataStop)?;
      Ok(())
  }

  fn set_background_color(&mut self, color: Color) {
      self.color = color;
  }

  fn background_color(&self) -> &Color {
      &self.color
  }

  fn width(&self) -> u32 {
      WIDTH
  }

  fn height(&self) -> u32 {
      HEIGHT
  }

  fn set_lut(
      &mut self,
      spi: &mut SPI,
      _refresh_rate: Option<RefreshLut>,
  ) -> Result<(), SPI::Error> {
      self.interface.cmd_with_data(spi, Command::LutForVcom, &LUT_VCOM_DC)?;
      self.interface.cmd_with_data(spi, Command::LutWhiteToWhite, &LUT_WW)?;
      self.interface.cmd_with_data(spi, Command::LutBlackToWhite, &LUT_BW)?;
      self.interface.cmd_with_data(spi, Command::LutWhiteToBlack, &LUT_BB)?;
      self.interface.cmd_with_data(spi, Command::LutBlackToBlack, &LUT_WB)?;

      Ok(())
  }

  fn is_busy(&self) -> bool {
      self.interface.is_busy(IS_BUSY_LOW)
  }
}

impl<SPI, CS, BUSY, DC, RST, DELAY> Epd2in7<SPI, CS, BUSY, DC, RST, DELAY>
where
  SPI: Write<u8>,
  CS: OutputPin,
  BUSY: InputPin,
  DC: OutputPin,
  RST: OutputPin,
  DELAY: DelayMs<u8>,
{
  fn wait_until_idle(&mut self, spi: &mut SPI,) -> Result<(), SPI::Error> {
      self.interface.cmd(spi, Command::GetStatus)?;
      let _ = self.interface.wait_until_idle(IS_BUSY_LOW);
      Ok(())
  }

  /// Refresh display for partial frame
  pub fn display_partial_frame(
      &mut self,
      spi: &mut SPI,
      x: u32,
      y: u32,
      width: u32,
      height: u32,
  ) -> Result<(), SPI::Error> {
      self.interface.cmd(spi, Command::PartialDisplayRefresh)?;
      self.interface.data(spi, &[(x >> 8) as u8])?;
      self.interface.data(spi, &[(x & 0xf8) as u8])?;
      self.interface.data(spi, &[(y >> 8) as u8])?;
      self.interface.data(spi, &[(y & 0xff) as u8])?;
      self.interface.data(spi, &[(width >> 8) as u8])?;
      self.interface.data(spi, &[(width & 0xf8) as u8])?;
      self.interface.data(spi, &[(height >> 8) as u8])?;
      self.interface.data(spi, &[(height & 0xff) as u8])?;
      self.wait_until_idle(spi)?;
      Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn epd_size() {
      assert_eq!(WIDTH, 176);
      assert_eq!(HEIGHT, 264);
      assert_eq!(DEFAULT_BACKGROUND_COLOR, Color::White);
  }
}