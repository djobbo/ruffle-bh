//! Decoder for Brawlhalla's custom `.swz` containers.
//!
//! These are **not** Adobe SWZ files (which are ZIPs). They are a keyed container
//! built from WELL512 output, zlib-deflated entries and an XOR mask. Adobe AIR
//! reads them through the `RawData` native extension, which Brawlhalla wrappers
//! call as `RawData_Init(key)` / `RawData_SetData(bytes)` / `RawData_GetData()`.
//! Ruffle has no ANE support, so we decode them natively and surface the entries
//! through `flash.external.ExtensionContext`.
//!
//! The format and this implementation mirror `packages/codecs/src/swz` in the
//! gimped repository.

use flate2::read::ZlibDecoder;
use std::fmt;
use std::io::Read;

const HEADER_CHECKSUM: u32 = 771_006_925;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SwzError {
    Truncated,
    HeaderChecksum { expected: u32, actual: u32 },
    EntryChecksum { index: usize, expected: u32, actual: u32 },
    InvalidCompressedSize(usize, u32),
    InvalidUncompressedSize(usize, u32, usize),
    InvalidDeflate(usize),
}

impl fmt::Display for SwzError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated => write!(f, "SWZ header is truncated"),
            Self::HeaderChecksum { expected, actual } => {
                write!(f, "SWZ header checksum mismatch: {actual} != {expected}")
            }
            Self::EntryChecksum {
                index,
                expected,
                actual,
            } => write!(
                f,
                "SWZ entry {index} checksum mismatch: {actual} != {expected}"
            ),
            Self::InvalidCompressedSize(index, size) => {
                write!(f, "SWZ entry {index} has invalid compressed size {size}")
            }
            Self::InvalidUncompressedSize(index, expected, actual) => write!(
                f,
                "SWZ entry {index} inflated to {actual} bytes, expected {expected}"
            ),
            Self::InvalidDeflate(index) => {
                write!(f, "SWZ entry {index} is not valid deflate data")
            }
        }
    }
}

impl std::error::Error for SwzError {}

/// Brawlhalla's variant of WELL512, seeded like MT19937 (`init_by_array`-free).
struct Well512 {
    state: [u32; 16],
    index: usize,
}

impl Well512 {
    fn new(seed: u32) -> Self {
        let mut state = [0u32; 16];
        state[0] = seed;
        for i in 1..16 {
            let previous = state[i - 1];
            let modified = previous ^ (previous >> 30);
            state[i] = (i as u32).wrapping_add(1_812_433_253u32.wrapping_mul(modified));
        }
        Self { state, index: 0 }
    }

    fn next(&mut self) -> u32 {
        let a = self.state[self.index];
        let b = self.state[(self.index.wrapping_sub(3)) & 0xf];
        let c = a ^ b ^ ((b ^ a.wrapping_mul(2)) << 15);
        let e = self.state[(self.index.wrapping_sub(7)) & 0xf];
        let d = (e >> 11) ^ e;
        let new_index = self.index.wrapping_sub(1) & 0xf;
        self.state[self.index] = d ^ c;
        self.state[new_index] = self.state[new_index]
            ^ d
            ^ ((d ^ c) & 0xfed2_2169).wrapping_mul(32)
            ^ (self.state[new_index] ^ ((c ^ (d << 10)) << 16)).wrapping_mul(4);
        self.index = new_index;
        self.state[new_index]
    }
}

fn mask_byte(random: u32, index: usize) -> u8 {
    let shift = index & 0xf;
    (((0xffu32 << shift) & random) >> shift) as u8
}

fn xor_count_for(key: u32) -> u32 {
    (key % 0x1f) + 5
}

/// Decode an entire `.swz` container into its UTF-8 entries.
///
/// Brawlhalla's `RawData_GetData` returns one entry at a time as a string;
/// the caller feeds each entry into `ResourceManager` (XML or `name\npayload`).
pub fn decode(bytes: &[u8], key: u32) -> Result<Vec<String>, SwzError> {
    if bytes.len() < 8 {
        return Err(SwzError::Truncated);
    }

    let mut offset = 0usize;
    let expected_header_checksum = u32::from_be_bytes(bytes[0..4].try_into().unwrap());
    let seed_xor = u32::from_be_bytes(bytes[4..8].try_into().unwrap());
    offset += 8;

    let seed = seed_xor ^ key;
    let mut prng = Well512::new(seed);

    let mut actual_header_checksum = HEADER_CHECKSUM;
    for _ in 0..xor_count_for(key) {
        actual_header_checksum ^= prng.next();
    }
    if actual_header_checksum != expected_header_checksum {
        return Err(SwzError::HeaderChecksum {
            expected: expected_header_checksum,
            actual: actual_header_checksum,
        });
    }

    let mut entries = Vec::new();
    let mut index = 0usize;
    while bytes.len().saturating_sub(offset) > 12 {
        let compressed_size = read_u32(bytes, &mut offset)? ^ prng.next();
        let uncompressed_size = read_u32(bytes, &mut offset)? ^ prng.next();

        let remaining = bytes.len() - offset;
        if compressed_size == 0 || compressed_size as usize > remaining.saturating_sub(4) {
            return Err(SwzError::InvalidCompressedSize(index, compressed_size));
        }

        let expected_checksum = read_u32(bytes, &mut offset)?;
        let mut actual_checksum = prng.next();
        let mut compressed = Vec::with_capacity(compressed_size as usize);
        for i in 0..compressed_size as usize {
            let plain = bytes[offset + i] ^ mask_byte(prng.next(), i);
            compressed.push(plain);
            actual_checksum = (plain as u32) ^ actual_checksum.rotate_right(((i % 7) + 1) as u32);
        }
        offset += compressed_size as usize;

        if actual_checksum != expected_checksum {
            return Err(SwzError::EntryChecksum {
                index,
                expected: expected_checksum,
                actual: actual_checksum,
            });
        }

        let mut inflated = Vec::with_capacity(uncompressed_size as usize);
        ZlibDecoder::new(compressed.as_slice())
            .read_to_end(&mut inflated)
            .map_err(|_| SwzError::InvalidDeflate(index))?;
        if inflated.len() != uncompressed_size as usize {
            return Err(SwzError::InvalidUncompressedSize(
                index,
                uncompressed_size,
                inflated.len(),
            ));
        }

        entries.push(String::from_utf8_lossy(&inflated).into_owned());
        index += 1;
    }

    // Brawlhalla's `ResourceManager` processes entries in order and aborts the
    // whole container if one handler throws. The `MasterFileList` is what
    // enqueues the preload set, so surface it first; this keeps a failure in
    // e.g. `LanguageTypes` from preventing UI assets from loading.
    entries.sort_by_key(|entry| {
        if entry.starts_with("<MasterFileList") {
            0
        } else {
            1
        }
    });

    // The standalone client preloads every "Game" stage asset before the menu
    // (all legend skins, weapons, SFX...). That is ~328 MB of SWFs which native
    // handles fine but is far beyond wasm32's 4 GiB heap once decoded. Most of
    // it is loaded on demand later; keep only the two stage containers here.
    for entry in &mut entries {
        if entry.starts_with("<MasterFileList") {
            let (filtered, removed) = filter_master_file_list(entry);
            if removed > 0 {
                tracing::warn!("MasterFileList: filtered {removed} Game-stage preload entries");
            }
            *entry = filtered;
        }
    }

    Ok(entries)
}

/// Removes `<File Stage="Game" ...>` preload entries except the required
/// `Game.swz`/`Dynamic.swz` containers. Resources omitted here are still
/// registered and loaded on demand by the game (`ResourceManager.load`).
fn filter_master_file_list(xml: &str) -> (String, usize) {
    #[cfg(not(target_family = "wasm"))]
    if std::env::var_os("RUFFLE_BRAWLHALLA_FULL").is_some() {
        return (xml.to_string(), 0);
    }

    const KEEP_GAME: &[&str] = &[
        "Game.swz",
        "Dynamic.swz",
        // Core weapon symbols are needed for the character-select paper doll.
        // Variant weapon atlases are intentionally not preloaded on wasm; they
        // cost minutes of AVM2 init and are only cosmetic.
        "Gfx_Weapons.swf",
        "Gfx_Weapons_Chakram.swf",
    ];
    const KEEP_GAME_HEADLESS: &[&str] = &["Game.swz", "Dynamic.swz"];

    // Native experiments: RUFFLE_BRAWLHALLA_KEEP holds comma-separated name
    // prefixes to keep from the Game preload stage (in addition to the stage
    // containers). When unset, use the compiled-in visual set.
    #[cfg(not(target_family = "wasm"))]
    const DEFAULT_KEEP: &[&str] = &[
        "Gfx_Skins",
        "Gfx_Player",
        "Gfx_Hands",
        "Gfx_Weapons",
        "Gfx_Weapon_",
        "Gfx_SplashArt",
        "Gfx_ClientThemeLogos",
        "Gfx_PlayerThemes",
        "Gfx_GameUI",
        "UI_",
        "Gfx_Gamemodes",
        "bones/Bones_Player",
        "bones/Bones_CharacterSelect",
        "bones/Bones_Items",
        "bones/Bones_Axe",
        "bones/Bones_Bow",
        "bones/Bones_Blaster",
        "bones/Bones_Cannon",
        "bones/Bones_Fists",
        "bones/Bones_Gauntlets",
        "bones/Bones_Hammer",
        "bones/Bones_Katar",
        "bones/Bones_Orb",
        "bones/Bones_Scythe",
        "bones/Bones_Spear",
        "bones/Bones_Sword",
        "bones/Bones_Boots",
        "bones/Bones_GreatSword",
        "bones/Bones_Lance",
        "bones/Bones_Pistols",
        "bones/Bones_Chakram",
        "bones/Bones_Unarmed",
    ];

    // wasm32 has a 4 GiB linear-memory ceiling, so skin/weapon atlases are
    // loaded on demand instead of all at startup. Keep the menu, UI, hands,
    // player skeleton and podium assets needed for the first screen.
    #[cfg(target_family = "wasm")]
    const DEFAULT_KEEP: &[&str] = &[
        "Gfx_Skins",
        "Gfx_Player",
        "Gfx_Hands",
        "Gfx_SplashArt",
        "Gfx_GameUI",
        "UI_",
        "Gfx_Gamemodes",
    ];

    let extra = std::env::var("RUFFLE_BRAWLHALLA_KEEP").unwrap_or_default();
    #[cfg(target_family = "wasm")]
    let headless = crate::headless_mode();
    #[cfg(not(target_family = "wasm"))]
    let headless = false;
    // Keep the menu/UI boot but skip the large Required-stage Gfx preload when
    // the caller only needs the replay simulation. Dropping UI_* as well stops
    // the client before the menu.
    #[cfg(not(target_family = "wasm"))]
    let replay_only = std::env::var_os("RUFFLE_REPLAY_ONLY").is_some();
    #[cfg(target_family = "wasm")]
    let replay_only = headless;
    let keep_game = if replay_only || headless {
        KEEP_GAME_HEADLESS
    } else {
        KEEP_GAME
    };
    let prefixes: Vec<&str> = if (replay_only || headless) && extra.is_empty() {
        Vec::new()
    } else if extra.is_empty() {
        DEFAULT_KEEP.to_vec()
    } else {
        extra
            .split(',')
            .map(str::trim)
            .filter(|prefix| !prefix.is_empty())
            .collect()
    };

    let mut output = String::with_capacity(xml.len());
    let mut removed = 0usize;
    let mut rest = xml;

    // Keep the root element intact. Otherwise the scanner can pair the root
    // opening with the first child's `/>` and drop both when that child is
    // filtered, leaving a malformed document.
    if let Some(root_start) = rest.find("<MasterFileList") {
        output.push_str(&rest[..root_start]);
        let tail = &rest[root_start..];
        if let Some(root_end) = tail.find('>') {
            output.push_str(&tail[..=root_end]);
            rest = &tail[root_end + 1..];
        }
    }

    while let Some(start) = rest.find('<') {
        output.push_str(&rest[..start]);
        let tail = &rest[start..];
        if let Some(end) = tail.find("/>") {
            let tag = &tail[..end + 2];
            let keep = match attribute(tag, "Stage") {
                Some("Game") => attribute(tag, "Name")
                    .map(|name| {
                        keep_game.contains(&name)
                            || prefixes.iter().any(|prefix| name.starts_with(prefix))
                    })
                    .unwrap_or(false),
                Some("Required") if replay_only => attribute(tag, "Name")
                    .map(is_logic_asset)
                    .unwrap_or(true),
                _ => true,
            };

            if keep {
                output.push_str(tag);
            } else {
                removed += 1;
            }
            rest = &tail[end + 2..];
        } else {
            output.push_str(tail);
            rest = "";
        }
    }
    output.push_str(rest);

    (output, removed)
}

/// Everything named UI_*, Gfx_*, audio, images, sprites, icons or fonts is
/// presentation-only and can be filtered. Everything else (game XML, anim,
/// bone and level data) is kept for the simulation.
fn is_logic_asset(name: &str) -> bool {
    !(name.starts_with("UI_")
        || name.starts_with("Gfx_")
        || name.starts_with("audio/")
        || name.starts_with("images/")
        || name.starts_with("sprites/")
        || name.starts_with("icons/")
        || name.starts_with("fontData/"))
}

fn attribute<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let needle = format!("{name}=\"");
    let start = tag.find(&needle)? + needle.len();
    let end = tag[start..].find('"')? + start;
    Some(&tag[start..end])
}

fn read_u32(bytes: &[u8], offset: &mut usize) -> Result<u32, SwzError> {
    let end = offset.checked_add(4).ok_or(SwzError::Truncated)?;
    let slice = bytes.get(*offset..end).ok_or(SwzError::Truncated)?;
    *offset = end;
    Ok(u32::from_be_bytes(slice.try_into().unwrap()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn well512_matches_reference_stream() {
        // Values produced by the TypeScript implementation in packages/codecs.
        let mut prng = Well512::new(0x1f3a_7c2d);
        assert_eq!(prng.next(), 0x50ff_07fa);
        assert_eq!(prng.next(), 0x23a7_2500);
        assert_eq!(prng.next(), 0xdd63_8fb2);
    }
}
