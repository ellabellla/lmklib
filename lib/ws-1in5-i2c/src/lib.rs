#![doc = include_str!("../README.md")]

use std::{fmt::{Display}, thread, time::Duration};

use image::{buffer::{EnumeratePixels}, Luma, GrayImage, DynamicImage, ImageBuffer};
use imageproc::drawing;
use rppal::{gpio::{Gpio, OutputPin, self}, i2c::{I2c, self}};
use rusttype::{Scale, Font, point};

#[derive(Debug)]
/// Screen Error
pub enum Error {
    /// GPIO error
    GPIO(gpio::Error),
    /// i2c error
    I2C(i2c::Error),
    /// Out of bounds error
    OutOfBounds,
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::GPIO(e) => f.write_fmt(format_args!("{}", e)),
            Error::I2C(e) => f.write_fmt(format_args!("{}", e)),
            Error::OutOfBounds => f.write_str("Buffer index out of bounds"),
        }
    }
}


/// Screen height
pub const OLED_WIDTH: usize = 128;
/// Screen height
pub const OLED_HEIGHT: usize = 128; 

pub struct WS1in5 {
    reset_pin: OutputPin,
    i2c_bus: I2c,

    cleared: bool,
}

impl WS1in5 {
    /// Create new
    pub fn new(address: u16, bus: u8, reset: u8) -> Result<WS1in5, Error> {
        let gpio = Gpio::new().map_err(|e| Error::GPIO(e))?;
        let mut reset_pin = gpio.get(reset).map_err(|e| Error::GPIO(e))?.into_output();
        reset_pin.set_low();

        let mut i2c_bus = I2c::with_bus(bus).map_err(|e| Error::I2C(e))?;
        i2c_bus.set_slave_address(address).map_err(|e| Error::I2C(e))?;

        let mut this = WS1in5 { reset_pin, i2c_bus, cleared: true };
        this.init()?;

        Ok(this)
    }

    fn command(&self, cmd: u8) -> Result<(), Error> {
        self.i2c_bus.smbus_write_byte(0x00, cmd).map_err(|e| Error::I2C(e))
    }

    fn init(&mut self) -> Result<(), Error> {
        self.reset();

        self.command(0xae)?;

        self.command(0x15)?;
        self.command(0x00)?;
        self.command(0x7f)?;

        self.command(0x75)?;
        self.command(0x00)?;
        self.command(0x7f)?;

        self.command(0x81)?;
        self.command(0x80)?;

        self.command(0xa0)?;
        self.command(0x51)?;

        self.command(0xa1)?;
        self.command(0x00)?;

        self.command(0xa2)?;
        self.command(0x00)?;

        self.command(0xa4)?;
        self.command(0xa8)?;
        self.command(0x7f)?;

        self.command(0xb1)?;
        self.command(0xf1)?;

        self.command(0xb3)?;
        self.command(0x00)?;
 
        self.command(0xab)?;
        self.command(0x01)?;

        self.command(0xb6)?;
        self.command(0x0f)?;

        self.command(0xbe)?;
        self.command(0x0f)?;

        self.command(0xbc)?;
        self.command(0x08)?;

        self.command(0xd5)?;
        self.command(0x62)?;

        self.command(0xfd)?;
        self.command(0x12)?;

        thread::sleep(Duration::from_millis(100));
        self.command(0xAF)?;
        Ok(())
    }

    pub fn reset(&mut self) {
        self.reset_pin.set_high();
        thread::sleep(Duration::from_millis(100));
        self.reset_pin.set_low();
        thread::sleep(Duration::from_millis(100));
        self.reset_pin.set_high();
        thread::sleep(Duration::from_millis(100));
        
        self.cleared = true;
    }

    fn set_windows(&self, xstart: u8, ystart: u8, xend: u8, yend: u8) -> Result<(), Error>{
        if (xstart > OLED_WIDTH as u8) || (ystart > OLED_HEIGHT as u8) || (xend > OLED_WIDTH as u8) || (yend > OLED_HEIGHT as u8) {
            return Ok(())
        }

        self.command(0x15)?;
        self.command(xstart/2)?;
        self.command(xend/2 - 1)?;

        self.command(0x75)?;
        self.command(ystart)?;
        self.command(yend - 1)?;

        Ok(())
    }

    /// Returns true if the screen been cleared since it was last written too
    pub fn has_cleared(&self) -> bool {
        self.cleared
    }

    /// Clear a section of the screen
    pub fn clear(&mut self, x: usize, y: usize, width: usize, height: usize) -> Result<(), Error> {
        self.cleared = true;

        let buffer: Vec<u8> = vec![0x00; (width  /2) * height];
        self.show_image(buffer, x, y, width, height)
    }

    /// Clear the whole screen
    pub fn clear_all(&mut self) -> Result<(), Error> {
        self.clear(0, 0, OLED_WIDTH, OLED_HEIGHT)
    }

    /// Convert image to buffer data
    pub fn get_buffer(&self, pixels: EnumeratePixels<Luma<u8>>, width: usize, height: usize) -> Result<Vec<u8>, Error> {
        let mut buf: Vec<u8> = vec![0xff; (width/2) * height];
        
        if pixels.len() != height * width {
            return Err(Error::OutOfBounds)
        }
        
        for (x, y, pixel) in pixels {
            let (x, y, pixel) = (x as usize, y as usize, pixel.0[0]);

            let addr = x/2 + y*(width/2);
            let color = pixel % 16;
            let data: u8 = buf[addr] & ((!0xf0u8).rotate_right((x as u32 % 2) *4));
            buf[addr] &= data | ((color<<4) >> ((x%2)*4));
        }
        Ok(buf)   
    }

    /// Show an image of a certain size on the screen at the specified coord
    pub fn show_image(&mut self, buffer: Vec<u8>, x: usize, y: usize, width: usize, height: usize) -> Result<(), Error> {
        self.set_windows(x as u8, y as u8, x as u8 + width as u8, y as u8 + height as u8)?;
        if buffer.len() < (width /2) * height {
            return Err(Error::OutOfBounds)
        }

        self.cleared = false;

        for i in 0..height {
            for j in 0..(width/2) {
                self.i2c_bus.smbus_write_byte(0x40, buffer[j + width / 2 * i])
                    .map_err(|e| Error::I2C(e))?;
            }
        }
            
        Ok(())
    }

    pub fn size_to_pow_2(mut size: (i32, i32)) -> (i32, i32) {
        if size.0 % 2 != 0 {
            size.0 += 1
        }
        if size.1 % 2 != 0 {
            size.1 += 1
        }
        size
    }

    /// Get the size of some text (ignoring new lines), returns the size of each character too.
    pub fn get_text_size_full(&self, text: &str, scale: &Scale, font: &Font) -> (usize, usize, usize) {
        let v_metrics = font.v_metrics(*scale);
        let height = (v_metrics.ascent - v_metrics.descent).ceil() as i32;

        let layout = font.layout("_", *scale,  point(0.0, 0.0));
        let min_x = layout.clone()
            .next()
            .map(|g| g.pixel_bounding_box().unwrap().min.x)
            .unwrap();
        let max_x = layout.clone()
            .last()
            .map(|g| g.pixel_bounding_box().unwrap().max.x)
            .unwrap();
        let width = max_x - min_x;

        let (w, h) = WS1in5::size_to_pow_2((width, height));
        (w as usize * text.chars().count(), h as usize, w as usize)
    }

    /// Get the size of some text (ignoring new lines)
    pub fn get_text_size(&self, text: &str, scale: &Scale, font: &Font) -> (usize, usize) {
        let (w, h, _) =self.get_text_size_full(text, scale, font);
        (w, h)
    }

    /// Create image from text
    pub fn create_text(&self, text: &str, scale: &Scale, font: &Font) -> (ImageBuffer<Luma<u8>, Vec<u8>>, usize, usize) {
        let (width, height, char_width) = self.get_text_size_full(text, scale, font);
        let mut image = GrayImage::new(width as u32, height as u32);
        for (i, char) in text.chars().enumerate() {
            image = drawing::draw_text(&image, Luma([15]), (i * char_width) as i32, 0, *scale, font, &char.to_string());
        }
        (DynamicImage::ImageLuma8(image).rotate180().to_luma8(), width as usize, height as usize)
    }

    /// Draw text to the screen at the specified coord (ignores new lines) (when flip = true, the screen is assumed to be upside down)
    pub fn draw_text(&mut self, x: usize, y: usize, text: &str, scale: &Scale, font: &Font, flip: bool) -> Result<(usize, usize), Error> {
        let (image, width, height) = self.create_text(text, scale, font);
        let buffer = self.get_buffer(image.enumerate_pixels(), width, height)?;

        if flip {
            self.show_image(buffer, OLED_WIDTH - width - x, OLED_HEIGHT - height - y, width, height)?;
        } else {
            self.show_image(buffer, x, y, width, height)?;
        }

        Ok((x + width, y + height))
    }

    /// Draw text centered on the screen with a given offset (ignores new lines) (when flip = true, the screen is assumed to be upside down)
    pub fn draw_centered_text(&mut self, x: usize, y: usize, text: &str, scale: &Scale, font: &Font, flip: bool) -> Result<(usize, usize), Error> {
        let (image, width, height) = self.create_text(text, scale, font);
        let buffer = self.get_buffer(image.enumerate_pixels(), width, height)?;

        if flip {
            self.show_image(buffer, OLED_WIDTH - width - (OLED_WIDTH / 2 - width / 2 - x), OLED_HEIGHT - height - (OLED_HEIGHT / 2 - height / 2 - y), width, height)?;
        } else {
            self.show_image(buffer, OLED_WIDTH / 2 - width / 2 - x, OLED_HEIGHT / 2 - height / 2 - y, width, height)?;
        }

        Ok((x + width, y + height))
    }

    /// Draw a paragraph, wraps text across the screen (ignores new lines) (when flip = true, the screen is assumed to be upside down)
    pub fn draw_paragraph(&mut self, text: &str, scale: &Scale, font: &Font, flip: bool) -> Result<(usize, usize), Error> {
        self.draw_paragraph_at(0, 0, text, scale, font, flip)
    }

    /// Draw a paragraph starting at a coord, wraps text across the screen (ignores new lines) (when flip = true, the screen is assumed to be upside down)
    pub fn draw_paragraph_at(&mut self, mut x: usize, mut y: usize, text: &str, scale: &Scale, font: &Font, flip: bool) -> Result<(usize, usize), Error> {
        for char in text.chars() {
             let (image, width, height) = if char.is_whitespace() {
                self.create_text("_", scale, font)
            } else {
                self.create_text(&format!("{}", char), scale, font)
            };

            let buffer = self.get_buffer(image.enumerate_pixels(), width, height)?;
            if !char.is_whitespace() {
                if flip {
                    self.show_image(buffer, OLED_WIDTH - width - x, OLED_HEIGHT - height - y, width, height)?;
                } else {
                    self.show_image(buffer, x, y, width, height)?;
                }
            }

            x += width;

            if x+width > OLED_WIDTH {
                x = 0;
                y += height;
            }
        }

        Ok((x, y))
    }
}

