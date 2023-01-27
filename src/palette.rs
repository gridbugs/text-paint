use gridbugs::rgb_int::Rgb24;
use std::{fs, path::Path};
use toml;

#[derive(Debug)]
pub struct Palette {
    pub fg: Vec<Rgb24>,
    pub bg: Vec<Rgb24>,
    pub ch: Vec<char>,
}

mod hex_rgb24 {
    use super::Rgb24;
    use nom::{
        bytes::complete::{tag, take_while_m_n},
        combinator::map_res,
        sequence::tuple,
        IResult,
    };

    fn from_hex(input: &str) -> Result<u8, std::num::ParseIntError> {
        u8::from_str_radix(input, 16)
    }

    fn is_hex_digit(c: char) -> bool {
        c.is_digit(16)
    }

    fn hex_primary(input: &str) -> IResult<&str, u8> {
        map_res(take_while_m_n(2, 2, is_hex_digit), from_hex)(input)
    }

    pub fn parse_hex_rgb24(input: &str) -> IResult<&str, Rgb24> {
        let (input, _) = tag("#")(input)?;
        let (input, (red, green, blue)) = tuple((hex_primary, hex_primary, hex_primary))(input)?;
        Ok((input, Rgb24::new(red, green, blue)))
    }
}

mod palette_toml {
    use super::Rgb24;

    fn parse_hex_rgb24_str(s: &str) -> Result<Rgb24, String> {
        let (_, rgb24) = super::hex_rgb24::parse_hex_rgb24(s)
            .map_err(|e| format!("failed to parse hex rgb ({:?})", e))?;
        Ok(rgb24)
    }

    fn parse_rgb24(toml: &toml::Value) -> Result<Rgb24, String> {
        let str = toml
            .as_str()
            .ok_or_else(|| format!("expected string, got {:?}", toml))?;
        parse_hex_rgb24_str(str)
    }

    fn parse_ch(toml: &toml::Value) -> Result<char, String> {
        let str = toml
            .as_str()
            .ok_or_else(|| format!("expected string, got {:?}", toml))?;
        let chars = str.chars().collect::<Vec<_>>();
        if chars.len() == 1 {
            Ok(chars[0])
        } else {
            Err(format!("expected string of length 1, got {}", str))
        }
    }

    fn parse_array<T, F: FnMut(&toml::Value) -> Result<T, String>>(
        toml: &toml::Value,
        mut parse_element: F,
    ) -> Result<Vec<T>, String> {
        let array = toml
            .as_array()
            .ok_or_else(|| format!("expected array, got {:?}", toml))?;
        let mut ret = Vec::new();
        for element in array {
            ret.push(parse_element(element)?);
        }
        Ok(ret)
    }

    fn parse_field<T, F: FnMut(&toml::Value) -> Result<T, String>>(
        toml: &toml::Value,
        field: &str,
        mut parse_contents: F,
    ) -> Result<T, String> {
        let contents = toml
            .get(field)
            .ok_or_else(|| format!("no such field \"{}\"", field))?;
        parse_contents(contents)
    }

    pub fn parse_palette(toml: &toml::Value) -> Result<super::Palette, String> {
        let fg = parse_field(toml, "fg", |v| parse_array(v, parse_rgb24))?;
        let bg = parse_field(toml, "bg", |v| parse_array(v, parse_rgb24))?;
        let ch = parse_field(toml, "ch", |v| parse_array(v, parse_ch))?;
        if fg.is_empty() {
            return Err("fg must not be empty".to_string());
        }
        if bg.is_empty() {
            return Err("bg must not be empty".to_string());
        }
        if ch.is_empty() {
            return Err("ch must not be empty".to_string());
        }
        Ok(super::Palette { fg, bg, ch })
    }
}

impl Palette {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        use toml::Value;
        let string =
            fs::read_to_string(path).map_err(|e| format!("failed to read file ({})", e))?;
        let toml = string
            .parse::<Value>()
            .map_err(|e| format!("failed to parse file ({})", e))?;
        palette_toml::parse_palette(&toml)
    }
}
