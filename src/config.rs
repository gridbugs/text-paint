use crate::parse_colour::parse_hex_rgb24;
use gridbugs::rgb_int::Rgb24;
use std::{fs, path::Path};
use toml;

#[derive(Debug)]
pub struct PaletteEntry {
    pub character: char,
    pub foreground: Rgb24,
    pub background: Option<Rgb24>,
}

#[derive(Debug)]
pub struct Config {
    palette: Vec<PaletteEntry>,
}

fn parse_palette_entry_toml(toml: &toml::Value) -> Result<PaletteEntry, String> {
    let mut parts = Vec::new();
    for part in toml
        .as_array()
        .ok_or_else(|| format!("palette entry is not array ({:?})", toml))?
    {
        let part = part
            .as_str()
            .ok_or_else(|| format!("palette entry part is not string ({:?})", part))?;
        parts.push(part);
    }
    let (character_str, foreground_str, maybe_background_str) = match &parts[..] {
        [character_str, foreground_str] => (character_str, foreground_str, None),
        [character_str, foreground_str, background_str] => {
            (character_str, foreground_str, Some(background_str))
        }
        _ => {
            return Err(format!(
                "palette entry must have 2 or 3 components ({:?})",
                parts
            ))
        }
    };
    let character = if character_str.len() == 1 {
        character_str.chars().next().unwrap()
    } else {
        return Err(format!(
            "first part must be single character (got \"{}\")",
            character_str
        ));
    };
    let (_, foreground) = parse_hex_rgb24(foreground_str)
        .map_err(|e| format!("failed to parse foreground string ({:?})", e))?;
    let background = if let Some(background_str) = maybe_background_str {
        Some(
            parse_hex_rgb24(background_str)
                .map_err(|e| format!("failed to parse background string ({:?})", e))?
                .1,
        )
    } else {
        None
    };
    Ok(PaletteEntry {
        character,
        foreground,
        background,
    })
}

fn parse_palette_toml(toml: &toml::Value) -> Result<Vec<PaletteEntry>, String> {
    let mut palette = Vec::new();
    for entry in toml.as_array().ok_or("\"palette\" is not an array")?.iter() {
        let palette_entry = parse_palette_entry_toml(entry)?;
        palette.push(palette_entry);
    }
    Ok(palette)
}

impl Config {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        use toml::Value;
        let string =
            fs::read_to_string(path).map_err(|e| format!("failed to read file ({})", e))?;
        let toml = string
            .parse::<Value>()
            .map_err(|e| format!("failed to parse file ({})", e))?;
        let palette = parse_palette_toml(
            toml.get("palette")
                .ok_or("config is missing \"palette\" field")?,
        )?;
        Ok(Self { palette })
    }

    pub fn palette(&self) -> &[PaletteEntry] {
        &self.palette
    }
}
