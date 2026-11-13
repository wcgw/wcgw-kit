//! Pictures as the terminal shows them (D15): fitted to cells, then sent with the kitty graphics protocol and shown
//! through Unicode placeholder cells, or drawn with half blocks where the protocol is missing. Both are text, so a
//! picture scrolls, clips and folds with the rest of a buffer.
//!
//! What a terminal can do is guessed from its environment ([`Terminal::detect`]). Still pictures go to a local
//! terminal as raw pixels in a `/dev/shm` file it reads (no encoding, no escapes to parse); to a remote one, and
//! for animations, as PNG in the escapes themselves. Animations are run by the terminal.

#[cfg(test)]
mod tests;

use std::fmt::Write as _;
use std::io::Cursor;
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use image::codecs::gif::GifDecoder;
use image::codecs::png::PngDecoder;
use image::codecs::webp::WebPDecoder;
use image::imageops::FilterType;
use image::{AnimationDecoder, DynamicImage, Frames, ImageFormat, ImageReader, RgbaImage};

/// The most cells a picture takes across: no more than the placeholder diacritics below can number.
pub const MAX_COLS: u16 = 296;

/// The most cells a picture takes down.
pub const MAX_ROWS: u16 = 296;

/// The most pixels a picture may have to be decoded, as the Go client allows.
const MAX_PIXELS: u64 = 8_000_000;

/// Pixels sent per cell, across and down, for a sharp picture on dense screens without sending more than needed.
const CELL_PIXELS: (u32, u32) = (16, 32);

/// Base64 bytes per chunk of a transmission, as the kitty protocol requires at most.
const CHUNK: usize = 4096;

/// The character of a placeholder cell.
const PLACEHOLDER: char = '\u{10EEEE}';

/// Row and column numbers, in order: `DIACRITICS[n]` encodes `n`. Copied from kitty's `rowcolumn-diacritics.txt`,
/// as slackers did; Ghostty uses the identical table.
#[rustfmt::skip]
const DIACRITICS: [char; 297] = [
    '\u{0305}', '\u{030D}', '\u{030E}', '\u{0310}', '\u{0312}', '\u{033D}', '\u{033E}', '\u{033F}',
    '\u{0346}', '\u{034A}', '\u{034B}', '\u{034C}', '\u{0350}', '\u{0351}', '\u{0352}', '\u{0357}',
    '\u{035B}', '\u{0363}', '\u{0364}', '\u{0365}', '\u{0366}', '\u{0367}', '\u{0368}', '\u{0369}',
    '\u{036A}', '\u{036B}', '\u{036C}', '\u{036D}', '\u{036E}', '\u{036F}', '\u{0483}', '\u{0484}',
    '\u{0485}', '\u{0486}', '\u{0487}', '\u{0592}', '\u{0593}', '\u{0594}', '\u{0595}', '\u{0597}',
    '\u{0598}', '\u{0599}', '\u{059C}', '\u{059D}', '\u{059E}', '\u{059F}', '\u{05A0}', '\u{05A1}',
    '\u{05A8}', '\u{05A9}', '\u{05AB}', '\u{05AC}', '\u{05AF}', '\u{05C4}', '\u{0610}', '\u{0611}',
    '\u{0612}', '\u{0613}', '\u{0614}', '\u{0615}', '\u{0616}', '\u{0617}', '\u{0657}', '\u{0658}',
    '\u{0659}', '\u{065A}', '\u{065B}', '\u{065D}', '\u{065E}', '\u{06D6}', '\u{06D7}', '\u{06D8}',
    '\u{06D9}', '\u{06DA}', '\u{06DB}', '\u{06DC}', '\u{06DF}', '\u{06E0}', '\u{06E1}', '\u{06E2}',
    '\u{06E4}', '\u{06E7}', '\u{06E8}', '\u{06EB}', '\u{06EC}', '\u{0730}', '\u{0732}', '\u{0733}',
    '\u{0735}', '\u{0736}', '\u{073A}', '\u{073D}', '\u{073F}', '\u{0740}', '\u{0741}', '\u{0743}',
    '\u{0745}', '\u{0747}', '\u{0749}', '\u{074A}', '\u{07EB}', '\u{07EC}', '\u{07ED}', '\u{07EE}',
    '\u{07EF}', '\u{07F0}', '\u{07F1}', '\u{07F3}', '\u{0816}', '\u{0817}', '\u{0818}', '\u{0819}',
    '\u{081B}', '\u{081C}', '\u{081D}', '\u{081E}', '\u{081F}', '\u{0820}', '\u{0821}', '\u{0822}',
    '\u{0823}', '\u{0825}', '\u{0826}', '\u{0827}', '\u{0829}', '\u{082A}', '\u{082B}', '\u{082C}',
    '\u{082D}', '\u{0951}', '\u{0953}', '\u{0954}', '\u{0F82}', '\u{0F83}', '\u{0F86}', '\u{0F87}',
    '\u{135D}', '\u{135E}', '\u{135F}', '\u{17DD}', '\u{193A}', '\u{1A17}', '\u{1A75}', '\u{1A76}',
    '\u{1A77}', '\u{1A78}', '\u{1A79}', '\u{1A7A}', '\u{1A7B}', '\u{1A7C}', '\u{1B6B}', '\u{1B6D}',
    '\u{1B6E}', '\u{1B6F}', '\u{1B70}', '\u{1B71}', '\u{1B72}', '\u{1B73}', '\u{1CD0}', '\u{1CD1}',
    '\u{1CD2}', '\u{1CDA}', '\u{1CDB}', '\u{1CE0}', '\u{1DC0}', '\u{1DC1}', '\u{1DC3}', '\u{1DC4}',
    '\u{1DC5}', '\u{1DC6}', '\u{1DC7}', '\u{1DC8}', '\u{1DC9}', '\u{1DCB}', '\u{1DCC}', '\u{1DD1}',
    '\u{1DD2}', '\u{1DD3}', '\u{1DD4}', '\u{1DD5}', '\u{1DD6}', '\u{1DD7}', '\u{1DD8}', '\u{1DD9}',
    '\u{1DDA}', '\u{1DDB}', '\u{1DDC}', '\u{1DDD}', '\u{1DDE}', '\u{1DDF}', '\u{1DE0}', '\u{1DE1}',
    '\u{1DE2}', '\u{1DE3}', '\u{1DE4}', '\u{1DE5}', '\u{1DE6}', '\u{1DFE}', '\u{20D0}', '\u{20D1}',
    '\u{20D4}', '\u{20D5}', '\u{20D6}', '\u{20D7}', '\u{20DB}', '\u{20DC}', '\u{20E1}', '\u{20E7}',
    '\u{20E9}', '\u{20F0}', '\u{2CEF}', '\u{2CF0}', '\u{2CF1}', '\u{2DE0}', '\u{2DE1}', '\u{2DE2}',
    '\u{2DE3}', '\u{2DE4}', '\u{2DE5}', '\u{2DE6}', '\u{2DE7}', '\u{2DE8}', '\u{2DE9}', '\u{2DEA}',
    '\u{2DEB}', '\u{2DEC}', '\u{2DED}', '\u{2DEE}', '\u{2DEF}', '\u{2DF0}', '\u{2DF1}', '\u{2DF2}',
    '\u{2DF3}', '\u{2DF4}', '\u{2DF5}', '\u{2DF6}', '\u{2DF7}', '\u{2DF8}', '\u{2DF9}', '\u{2DFA}',
    '\u{2DFB}', '\u{2DFC}', '\u{2DFD}', '\u{2DFE}', '\u{2DFF}', '\u{A66F}', '\u{A67C}', '\u{A67D}',
    '\u{A6F0}', '\u{A6F1}', '\u{A8E0}', '\u{A8E1}', '\u{A8E2}', '\u{A8E3}', '\u{A8E4}', '\u{A8E5}',
    '\u{A8E6}', '\u{A8E7}', '\u{A8E8}', '\u{A8E9}', '\u{A8EA}', '\u{A8EB}', '\u{A8EC}', '\u{A8ED}',
    '\u{A8EE}', '\u{A8EF}', '\u{A8F0}', '\u{A8F1}', '\u{AAB0}', '\u{AAB2}', '\u{AAB3}', '\u{AAB7}',
    '\u{AAB8}', '\u{AABE}', '\u{AABF}', '\u{AAC1}', '\u{FE20}', '\u{FE21}', '\u{FE22}', '\u{FE23}',
    '\u{FE24}', '\u{FE25}', '\u{FE26}', '\u{10A0F}', '\u{10A38}', '\u{1D185}', '\u{1D186}', '\u{1D187}',
    '\u{1D188}', '\u{1D189}', '\u{1D1AA}', '\u{1D1AB}', '\u{1D1AC}', '\u{1D1AD}', '\u{1D242}', '\u{1D243}',
    '\u{1D244}',
];

/// How pictures are drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Graphics {
    /// Sent with the kitty graphics protocol, shown through placeholder cells.
    Kitty,
    /// Drawn with half blocks in true colour.
    HalfBlocks,
    /// Not drawn: what a picture is about shows instead.
    Off,
}

impl Graphics {
    /// Every way pictures are drawn.
    pub const ALL: [Self; 3] = [Self::Kitty, Self::HalfBlocks, Self::Off];

    /// Its name, as `:set graphics=` takes it.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Kitty => "kitty",
            Self::HalfBlocks => "halfblocks",
            Self::Off => "off",
        }
    }

    /// The graphics of that name.
    #[must_use]
    pub fn named(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|graphics| graphics.name() == name)
    }
}

/// What the terminal can do, as its environment tells.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)] // Independent facts about the terminal.
pub struct Terminal {
    /// How pictures are best drawn there.
    pub graphics: Graphics,
    /// Whether it runs animations sent with the kitty protocol itself: kitty, and Ghostty from 1.4 (slackers'
    /// ADR 0002). Elsewhere a picture sent so stays on its first frame.
    pub animates: bool,
    /// Whether it runs on this machine, so it can read files the kit writes: not over SSH.
    pub local: bool,
    /// Whether it draws in true colour, rather than with its sixteen colours (D8).
    pub truecolor: bool,
    /// Whether it draws Nerd Font icons without the user installing a Nerd Font: Ghostty, kitty and WezTerm ship
    /// the symbols (muttrs' finding, D9).
    pub nerd_font_symbols: bool,
}

impl Terminal {
    /// What the terminal can likely do, from the environment variables `var` reads: `TERM`, `TERM_PROGRAM` and its
    /// version, kitty's and Ghostty's own, multiplexers' (`TMUX`, `ZELLIJ`, `STY`), SSH's and `COLORTERM`. Under a
    /// multiplexer pictures are half blocks: placeholders would need passing through (`:set graphics=kitty`
    /// says they are).
    ///
    /// The kitty protocol's own query (`a=q`) is not asked: it says whether the protocol is there, not whether
    /// Unicode placeholders are (WezTerm and Konsole have one without the other), and its answer would race the
    /// first keys typed.
    #[must_use]
    pub fn detect(var: impl Fn(&str) -> Option<String>) -> Self {
        let set = |name: &str| var(name).is_some_and(|value| !value.is_empty());
        let named = |name: &str| {
            ["TERM", "TERM_PROGRAM"]
                .into_iter()
                .filter_map(&var)
                .any(|value| value.to_lowercase().contains(name))
        };
        let kitty = set("KITTY_WINDOW_ID") || named("kitty");
        let ghostty = set("GHOSTTY_RESOURCES_DIR") || named("ghostty");
        let wezterm = named("wezterm");
        let multiplexed = set("TMUX") || set("ZELLIJ") || set("STY");
        let ghostty_animates = var("TERM_PROGRAM_VERSION")
            .filter(|_| named("ghostty"))
            .and_then(|version| {
                let mut parts = version.split(['.', '-']).map(str::parse::<u32>);
                Some((parts.next()?.ok()?, parts.next()?.ok()?))
            })
            // A version it doesn't say, as over SSH, is taken to be new enough.
            .is_none_or(|version| version >= (1, 4));
        let graphics = if (kitty || ghostty) && !multiplexed {
            Graphics::Kitty
        } else {
            Graphics::HalfBlocks
        };
        let colorterm = var("COLORTERM").unwrap_or_default().to_lowercase();
        Self {
            graphics,
            animates: kitty || (ghostty && ghostty_animates),
            local: !(set("SSH_CONNECTION") || set("SSH_TTY") || set("SSH_CLIENT")),
            truecolor: colorterm == "truecolor"
                || colorterm == "24bit"
                || kitty
                || ghostty
                || wezterm,
            nerd_font_symbols: kitty || ghostty || wezterm,
        }
    }
}

/// How pixels reach the terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transport {
    /// In the escapes, as PNG: works everywhere, over SSH too.
    InBand,
    /// As raw pixels in a file under `/dev/shm`, which the terminal reads and deletes: only for a terminal on
    /// this machine. The name says which application wrote it.
    SharedMemory(&'static str),
}

/// A picture sent with the kitty protocol: its escapes, and the file they name, if any, to delete should they never
/// be sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sent {
    /// The escapes, to write as they are.
    pub escapes: String,
    /// The file holding the pixels, which the terminal deletes once it read it.
    pub staged: Option<std::path::PathBuf>,
}

/// The cells a `width` × `height` pixel picture takes when as wide as `max_cols` and no taller than `max_rows`
/// (both within the protocol's limits), as (columns, rows); cells are twice as tall as they are wide. `(0, 0)` for
/// an empty picture.
#[must_use]
pub fn fit(width: u32, height: u32, max_cols: u16, max_rows: u16) -> (u16, u16) {
    if width == 0 || height == 0 || max_cols == 0 || max_rows == 0 {
        return (0, 0);
    }
    let (width, height) = (f64::from(width), f64::from(height));
    let max_rows = max_rows.min(MAX_ROWS);
    let mut cols = max_cols.min(MAX_COLS);
    let mut rows = rounded(f64::from(cols) * height / width / 2.0).max(1);
    if rows > max_rows {
        rows = max_rows;
        cols = cols
            .min(rounded(f64::from(rows) * 2.0 * width / height))
            .max(1);
    }
    (cols, rows)
}

/// The cells a `width` × `height` pixel picture takes at its own size, a cell being `cell` pixels (across, down):
/// those its pixels cover, keeping its shape within the protocol's limits, as (columns, rows). `(0, 0)` for an empty
/// picture.
#[must_use]
pub fn natural(width: u32, height: u32, cell: (u16, u16)) -> (u16, u16) {
    let covered = |pixels: u32, per_cell: u16| {
        u16::try_from(pixels.div_ceil(u32::from(per_cell.max(1)))).unwrap_or(u16::MAX)
    };
    fit(
        width,
        height,
        covered(width, cell.0),
        covered(height, cell.1),
    )
}

/// `value`, which is positive, rounded to cells.
fn rounded(value: f64) -> u16 {
    // Cells are counted in the thousands at most: the cast neither truncates nor loses a sign.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let cells = value.round().min(f64::from(u16::MAX)) as u16;
    cells
}

/// The most frames an animation keeps; a longer one stays on its first.
const MAX_FRAMES: usize = 200;

/// A frame said to show this briefly or less shows for [`BRIEF_FRAME`] instead, as browsers do.
const BRIEFEST_FRAME: Duration = Duration::from_millis(10);

/// How long a frame said to show too briefly shows.
const BRIEF_FRAME: Duration = Duration::from_millis(100);

/// A decoded picture: a still one, or an animation.
#[derive(Debug, Clone)]
pub struct Picture {
    /// Never empty: a still picture is one frame.
    frames: Vec<Frame>,
}

/// A frame of a picture, all of it.
#[derive(Debug, Clone)]
struct Frame {
    pixels: RgbaImage,
    /// How long it shows before the next, in an animation.
    delay: Duration,
}

/// A frame of a picture drawn with half blocks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HalfBlockFrame {
    /// Its cells, by rows.
    pub cells: Vec<Vec<HalfBlock>>,
    /// How long it shows before the next, in an animation.
    pub delay: Duration,
}

/// One cell of a picture drawn with half blocks: `▀` in the upper pixel's colour over the lower's. `None` is a
/// transparent pixel, which shows the background.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HalfBlock {
    /// The upper pixel, as RGB.
    pub upper: Option<(u8, u8, u8)>,
    /// The lower pixel, as RGB.
    pub lower: Option<(u8, u8, u8)>,
}

impl Picture {
    /// The picture `bytes` hold: PNG, JPEG, WebP or GIF, of at most 8 megapixels. An animated GIF, WebP or PNG
    /// keeps its frames, up to [`MAX_FRAMES`] and 8 megapixels in all; a longer or larger one only its first. `None`
    /// for anything else.
    #[must_use]
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        let reader = ImageReader::new(Cursor::new(bytes))
            .with_guessed_format()
            .ok()?;
        let format = reader.format()?;
        if !matches!(
            format,
            ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::WebP | ImageFormat::Gif
        ) {
            return None;
        }
        let (width, height) = reader.into_dimensions().ok()?;
        let canvas = u64::from(width) * u64::from(height);
        if canvas == 0 || canvas > MAX_PIXELS {
            return None;
        }
        if let Some(frames) = animation(bytes, format, canvas) {
            return Some(Self { frames });
        }
        let picture = image::load_from_memory_with_format(bytes, format).ok()?;
        Some(Self {
            frames: vec![Frame {
                pixels: picture.to_rgba8(),
                delay: Duration::ZERO,
            }],
        })
    }

    /// Its size in pixels, as (width, height).
    #[must_use]
    pub fn size(&self) -> (u32, u32) {
        self.frames[0].pixels.dimensions()
    }

    /// The kitty protocol escapes that send it as image `id`, shown in `cols` × `rows` cells by placeholders (a
    /// virtual placement), quietly, at most [`CELL_PIXELS`] per cell. A still picture goes as `transport` says; an
    /// animation as PNG in the escapes, its other frames too, then has the terminal run it, looping, when it
    /// `animates` (else it stays on its first frame).
    #[must_use]
    pub fn kitty(
        &self,
        id: u32,
        cols: u16,
        rows: u16,
        transport: Transport,
        animates: bool,
    ) -> Sent {
        let (width, height) = (
            u32::from(cols) * CELL_PIXELS.0,
            u32::from(rows) * CELL_PIXELS.1,
        );
        let frames = if animates {
            &self.frames[..]
        } else {
            &self.frames[..1]
        };
        if let ([frame], Transport::SharedMemory(app)) = (frames, transport)
            && let Some(sent) = shared(
                id,
                cols,
                rows,
                &boxed(&frame.pixels, width, height, Fill::Shape),
                app,
            )
        {
            return sent;
        }
        let mut sent = String::new();
        for (i, frame) in frames.iter().enumerate() {
            let Some(png) = png(&boxed(&frame.pixels, width, height, Fill::Shape)) else {
                return Sent {
                    escapes: String::new(),
                    staged: None,
                };
            };
            if i == 0 {
                let keys = format!("a=T,f=100,t=d,i={id},c={cols},r={rows},U=1,q=2");
                transmit(&mut sent, &keys, "", &png);
            } else {
                let keys = format!("a=f,f=100,i={id},z={},q=2", frame.delay.as_millis());
                transmit(&mut sent, &keys, "a=f,", &png);
            }
        }
        if let [first, _, ..] = frames {
            // The root frame's gap can only be set once it is there.
            let gap = first.delay.as_millis();
            let _ = write!(sent, "\x1b_Ga=a,i={id},r=1,z={gap},q=2\x1b\\");
            let _ = write!(sent, "\x1b_Ga=a,i={id},s=3,v=1,q=2\x1b\\");
        }
        Sent {
            escapes: sent,
            staged: None,
        }
    }

    /// How many bytes of pixels the terminal keeps for it sent at `cols` × `rows` cells: what memory budgets count.
    #[must_use]
    pub fn kitty_bytes(&self, cols: u16, rows: u16, animates: bool) -> usize {
        let frames = if animates { self.frames.len() } else { 1 };
        let per_frame = u64::from(cols)
            * u64::from(CELL_PIXELS.0)
            * u64::from(rows)
            * u64::from(CELL_PIXELS.1)
            * 4;
        usize::try_from(per_frame.saturating_mul(frames as u64)).unwrap_or(usize::MAX)
    }

    /// The picture as `cols` × `rows` cells of half blocks, frame by frame.
    #[must_use]
    pub fn half_blocks(&self, cols: u16, rows: u16) -> Vec<HalfBlockFrame> {
        self.frames
            .iter()
            .map(|frame| HalfBlockFrame {
                cells: half_blocks(&frame.pixels, cols, rows),
                delay: frame.delay,
            })
            .collect()
    }
}

/// The frames of the GIF, or animated WebP or PNG, `bytes` hold as `format`, each of `canvas` pixels, how long each
/// shows made sensible; `None` for anything else, or when they are more than [`MAX_FRAMES`] or [`MAX_PIXELS`] in all.
fn animation(bytes: &[u8], format: ImageFormat, canvas: u64) -> Option<Vec<Frame>> {
    // A still WebP or PNG is not decoded as frames, then again as a picture.
    let frames: Frames<'_> = match format {
        ImageFormat::Gif => GifDecoder::new(Cursor::new(bytes)).ok()?.into_frames(),
        ImageFormat::WebP => {
            let decoder = WebPDecoder::new(Cursor::new(bytes)).ok()?;
            if !decoder.has_animation() {
                return None;
            }
            decoder.into_frames()
        }
        ImageFormat::Png => {
            let decoder = PngDecoder::new(Cursor::new(bytes)).ok()?;
            if !decoder.is_apng().ok()? {
                return None;
            }
            decoder.apng().ok()?.into_frames()
        }
        _ => return None,
    };
    let most = usize::try_from(MAX_PIXELS / canvas)
        .unwrap_or(usize::MAX)
        .min(MAX_FRAMES);
    let mut kept = Vec::new();
    for frame in frames {
        if kept.len() == most {
            return None;
        }
        let frame = frame.ok()?;
        let delay = Duration::from(frame.delay());
        kept.push(Frame {
            delay: if delay <= BRIEFEST_FRAME {
                BRIEF_FRAME
            } else {
                delay
            },
            pixels: frame.into_buffer(),
        });
    }
    (!kept.is_empty()).then_some(kept)
}

/// The escapes that send `pixels` as image `id` through a file under `/dev/shm` named for `app`; `None` when the
/// file can't be written, for the escapes to carry them instead.
fn shared(id: u32, cols: u16, rows: u16, pixels: &RgbaImage, app: &str) -> Option<Sent> {
    // Ghostty reads temporary files only under /dev/shm or its temp dir, and only if they're named this way. The
    // counter keeps names unique within the process (slackers' finding).
    static STAGED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let serial = STAGED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::path::PathBuf::from(format!(
        "/dev/shm/tty-graphics-protocol-{app}-{}-{serial}.rgba",
        std::process::id()
    ));
    std::fs::write(&path, pixels.as_raw()).ok()?;
    let (width, height) = pixels.dimensions();
    let name = STANDARD.encode(path.as_os_str().as_encoded_bytes());
    let escapes = format!(
        "\x1b_Ga=T,f=32,s={width},v={height},t=t,i={id},c={cols},r={rows},U=1,q=2;{name}\x1b\\"
    );
    Some(Sent {
        escapes,
        staged: Some(path),
    })
}

/// `pixels` as a PNG, if it can be encoded.
fn png(pixels: &RgbaImage) -> Option<Vec<u8>> {
    let mut png = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(pixels.clone())
        .write_to(&mut png, ImageFormat::Png)
        .ok()?;
    Some(png.into_inner())
}

/// Appends to `sent` the escapes that send `data` in base64 chunks: the first with `keys`, the others with
/// `continued` (`a=f,` for a frame, which every chunk must say) before their own.
fn transmit(sent: &mut String, keys: &str, continued: &str, data: &[u8]) {
    let encoded = STANDARD.encode(data);
    let chunks: Vec<&[u8]> = encoded.as_bytes().chunks(CHUNK).collect();
    sent.reserve(encoded.len() + chunks.len() * 64);
    for (i, chunk) in chunks.iter().enumerate() {
        let more = u8::from(i + 1 < chunks.len());
        // Chunks are base64, so ASCII.
        let chunk = std::str::from_utf8(chunk).unwrap_or_default();
        if i == 0 {
            let _ = write!(sent, "\x1b_G{keys},m={more};{chunk}\x1b\\");
        } else {
            let _ = write!(sent, "\x1b_G{continued}m={more},q=2;{chunk}\x1b\\");
        }
    }
}

/// `pixels` as `cols` × `rows` cells of half blocks, by rows.
fn half_blocks(pixels: &RgbaImage, cols: u16, rows: u16) -> Vec<Vec<HalfBlock>> {
    if cols == 0 || rows == 0 {
        return Vec::new();
    }
    let scaled = boxed(pixels, u32::from(cols), u32::from(rows) * 2, Fill::Exactly);
    let colour = |x: u32, y: u32| {
        let [r, g, b, a] = scaled.get_pixel(x, y).0;
        (a >= 128).then_some((r, g, b))
    };
    (0..u32::from(rows))
        .map(|row| {
            (0..u32::from(cols))
                .map(|col| HalfBlock {
                    upper: colour(col, row * 2),
                    lower: colour(col, row * 2 + 1),
                })
                .collect()
        })
        .collect()
}

/// How [`Picture::boxed`] fills its box.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fill {
    /// Exactly the box's pixels: the picture scaled up or down to fit.
    Exactly,
    /// The box's shape, at most its size: the picture scaled down to fit, never up, for a terminal that stretches
    /// it over the cells.
    Shape,
}

/// `source` centred in a transparent canvas filling a `width` × `height` pixel box as `fill` says, keeping its
/// own shape.
fn boxed(source: &RgbaImage, width: u32, height: u32, fill: Fill) -> RgbaImage {
    let (w, h) = source.dimensions();
    let (box_w, box_h) = (f64::from(width.max(1)), f64::from(height.max(1)));
    let mut scale = (box_w / f64::from(w)).min(box_h / f64::from(h));
    if fill == Fill::Shape {
        scale = scale.min(1.0);
    }
    let scaled_w = pixels(f64::from(w) * scale);
    let scaled_h = pixels(f64::from(h) * scale);
    let scaled = if (scaled_w, scaled_h) == (w, h) {
        source.clone()
    } else {
        image::imageops::resize(source, scaled_w, scaled_h, FilterType::CatmullRom)
    };
    let (canvas_w, canvas_h) = match fill {
        Fill::Exactly => (width.max(1), height.max(1)),
        Fill::Shape => (
            scaled_w.max(pixels(f64::from(scaled_h) * box_w / box_h)),
            scaled_h.max(pixels(f64::from(scaled_w) * box_h / box_w)),
        ),
    };
    let mut canvas = RgbaImage::new(canvas_w, canvas_h);
    image::imageops::overlay(
        &mut canvas,
        &scaled,
        i64::from((canvas_w - scaled_w.min(canvas_w)) / 2),
        i64::from((canvas_h - scaled_h.min(canvas_h)) / 2),
    );
    canvas
}

/// `value`, a positive size in pixels, rounded to at least one.
fn pixels(value: f64) -> u32 {
    // Pictures are 8 megapixels at most, and boxes a few thousand pixels: neither truncates nor loses a sign.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let pixels = value.round().clamp(1.0, f64::from(u32::MAX)) as u32;
    pixels
}

/// The text of the placeholder cell at `row` and `col` of a picture shown through placeholders; its foreground
/// colour names the picture (see [`image_colour`]).
#[must_use]
pub fn placeholder(row: u16, col: u16) -> String {
    let diacritic = |n: u16| DIACRITICS[usize::from(n).min(DIACRITICS.len() - 1)];
    [PLACEHOLDER, diacritic(row), diacritic(col)]
        .into_iter()
        .collect()
}

/// The foreground colour, as RGB, of the placeholder cells of image `id` (24 bits).
#[must_use]
pub fn image_colour(id: u32) -> (u8, u8, u8) {
    let [_, r, g, b] = id.to_be_bytes();
    (r, g, b)
}

/// The kitty protocol escape that frees every image with an id from `first` to `last` and their placements: by id,
/// because deleting all (`d=A`) leaves virtual placements, which Ghostty skips (slackers' finding).
#[must_use]
pub fn kitty_delete_range(first: u32, last: u32) -> String {
    format!("\x1b_Ga=d,d=R,x={first},y={last},q=2\x1b\\")
}

/// The kitty protocol escape that frees image `id`, quietly.
#[must_use]
pub fn kitty_delete(id: u32) -> String {
    format!("\x1b_Ga=d,d=I,i={id},q=2\x1b\\")
}
