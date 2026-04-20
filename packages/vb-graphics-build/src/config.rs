use std::{
    collections::{BTreeMap, HashSet},
    env,
    fs::File,
    io::BufWriter,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

pub struct Options {
    config_file: PathBuf,
    input_dir: PathBuf,
    output_dir: PathBuf,
    emit_cargo: bool,
    seen: HashSet<PathBuf>,
}

impl Options {
    pub fn cargo_defaults() -> Result<Self> {
        Ok(Self {
            config_file: PathBuf::from("assets.toml"),
            input_dir: env::current_dir()?,
            output_dir: PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR not defined")),
            emit_cargo: true,
            seen: HashSet::new(),
        })
    }

    fn config_file_path(&mut self) -> PathBuf {
        self.input_path(&self.config_file.clone())
    }

    fn input_path(&mut self, path: &Path) -> PathBuf {
        let result = self.input_dir.join(path);
        if self.emit_cargo && self.seen.insert(result.clone()) {
            println!("cargo:rerun-if-changed={}", result.display());
        }
        result
    }

    pub(crate) fn output_file(&self, path: &str) -> Result<BufWriter<File>> {
        let file = File::create(self.output_dir.join(path))?;
        Ok(BufWriter::new(file))
    }
}

#[derive(Deserialize, Debug)]
struct RawAnimationSerde {
    chardata: String,
    frames: Vec<RawImageData>,
}
impl From<RawAnimationSerde> for RawAnimation {
    fn from(value: RawAnimationSerde) -> Self {
        Self {
            chardata: value.chardata.clone(),
            images: value
                .frames
                .into_iter()
                .map(|f| RawImage {
                    chardata: value.chardata.clone(),
                    palette: None,
                    data: f,
                })
                .collect(),
        }
    }
}

#[derive(Deserialize, Debug)]
struct RawAssetFile {
    #[serde(default)]
    pub imports: Vec<PathBuf>,
    #[serde(default)]
    pub spritesheets: Vec<PathBuf>,
    #[serde(default)]
    pub palette: Option<[u8; 3]>,
    #[serde(rename = "image", default)]
    pub images: BTreeMap<String, RawImage>,
    #[serde(rename = "animation", default)]
    pub animations: BTreeMap<String, RawAnimationSerde>,
    #[serde(rename = "mask", default)]
    pub masks: BTreeMap<String, RawMask>,
    #[serde(rename = "font", default)]
    pub fonts: BTreeMap<String, RawFont>,
    #[serde(rename = "bgspritemap", default)]
    pub bg_sprite_maps: BTreeMap<String, RawBgSpriteMap>,
}

#[derive(Deserialize, Debug)]
struct RawSpritesheet {
    chardata: String,
    #[serde(default)]
    palette: Option<[u8; 3]>,
    file: PathBuf,
    #[serde(default)]
    offset: (isize, isize),
    sprite_size: (usize, usize),
    #[serde(default)]
    sprite_margin: (isize, isize),
    #[serde(rename = "sprite", default)]
    sprites: BTreeMap<String, RawSprite>,
    #[serde(rename = "animation", default)]
    animations: BTreeMap<String, Vec<RawSprite>>,
}

#[derive(Deserialize, Debug)]
struct RawSprite {
    #[serde(default)]
    pub hflip: bool,
    #[serde(default)]
    pub vflip: bool,
    #[serde(default)]
    pub transpose: bool,
    #[serde(default)]
    pub rotate: usize,
    #[serde(default = "no_zoom")]
    pub scale: f64,
    position: (isize, isize),
}

#[derive(Deserialize, Debug)]
pub struct RawBgSpriteMap {
    pub base: Option<String>,
    #[serde(default)]
    pub bgmap_start: u8,
    #[serde(default)]
    spritesheets: Vec<PathBuf>,
    #[serde(rename = "sprite", default)]
    pub sprites: BTreeMap<String, RawBgSprite>,
}
impl RawBgSpriteMap {
    fn fix_files(self, opts: &mut Options, dir: &Path) -> Self {
        Self {
            spritesheets: self
                .spritesheets
                .into_iter()
                .map(|p| opts.input_path(&dir.join(&p)))
                .collect(),
            ..self
        }
    }
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum RawBgSprite {
    Image {
        image: String,
    },
    Subregion {
        parent: String,
        position: (usize, usize),
        size: (usize, usize),
    },
    Region {
        size: (usize, usize),
        frames: Option<usize>,
        #[serde(default)]
        stereo: bool,
    },
}

#[derive(Debug)]
pub struct RawAssets {
    pub animations: BTreeMap<String, RawAnimation>,
    pub images: BTreeMap<String, RawImage>,
    pub bg_sprite_maps: BTreeMap<String, RawBgSpriteMap>,
    pub masks: BTreeMap<String, RawMask>,
    pub fonts: BTreeMap<String, RawFont>,
}

#[derive(Debug)]
pub struct RawAnimation {
    pub chardata: String,
    pub images: Vec<RawImage>,
}
impl RawAnimation {
    fn fix(self, opts: &mut Options, dir: &Path, palette: Option<[u8; 3]>) -> Self {
        Self {
            chardata: self.chardata,
            images: self
                .images
                .into_iter()
                .map(|i| i.fix(opts, dir, palette))
                .collect(),
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct RawImage {
    pub chardata: String,
    #[serde(default)]
    pub palette: Option<[u8; 3]>,
    #[serde(flatten)]
    pub data: RawImageData,
}
impl RawImage {
    fn fix(self, opts: &mut Options, dir: &Path, palette: Option<[u8; 3]>) -> Self {
        Self {
            palette: self.palette.or(palette),
            data: self.data.fix_files(opts, dir),
            ..self
        }
    }
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum RawImageData {
    Mono(RawImageRegion),
    Stereo {
        left: RawImageRegion,
        right: RawImageRegion,
    },
}
impl RawImageData {
    fn fix_files(self, opts: &mut Options, dir: &Path) -> Self {
        match self {
            Self::Mono(region) => Self::Mono(region.fix_files(opts, dir)),
            Self::Stereo { left, right } => Self::Stereo {
                left: left.fix_files(opts, dir),
                right: right.fix_files(opts, dir),
            },
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct RawMask {
    #[serde(flatten)]
    pub region: RawImageRegion,
}
impl RawMask {
    fn fix_files(self, opts: &mut Options, dir: &Path) -> Self {
        Self {
            region: self.region.fix_files(opts, dir),
        }
    }
}

const fn no_zoom() -> f64 {
    1.0
}

#[derive(Deserialize, Debug)]
pub struct RawImageRegion {
    pub file: PathBuf,
    #[serde(default)]
    pub hflip: bool,
    #[serde(default)]
    pub vflip: bool,
    #[serde(default)]
    pub transpose: bool,
    #[serde(default)]
    pub rotate: usize,
    #[serde(default = "no_zoom")]
    pub scale: f64,
    pub position: Option<(isize, isize)>,
    pub size: Option<(usize, usize)>,
}
impl RawImageRegion {
    fn fix_files(self, opts: &mut Options, dir: &Path) -> Self {
        Self {
            file: opts.input_path(&dir.join(self.file)),
            ..self
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct RawFont {
    pub file: PathBuf,
    pub size: f32,
}
impl RawFont {
    fn fix_files(self, opts: &mut Options, dir: &Path) -> Self {
        Self {
            file: opts.input_path(&dir.join(self.file)),
            ..self
        }
    }
}

pub fn parse(opts: &mut Options) -> Result<RawAssets> {
    let mut assets = RawAssets {
        animations: BTreeMap::new(),
        images: BTreeMap::new(),
        bg_sprite_maps: BTreeMap::new(),
        masks: BTreeMap::new(),
        fonts: BTreeMap::new(),
    };
    let mut files = vec![(opts.config_file_path(), None)];
    let mut spritesheet_sprites = BTreeMap::new();
    while let Some((path, parent_palette)) = files.pop() {
        let file = std::fs::read_to_string(&path)
            .with_context(|| format!("could not read config file {}", path.display()))?;
        let file: RawAssetFile = toml::from_str(&file)?;
        let palette = file.palette.or(parent_palette);
        let Some(dir) = path.parent() else {
            bail!("invalid config file path {}", path.display());
        };

        for import in file.imports {
            files.push((opts.input_path(&dir.join(import)), palette));
        }
        for spritesheet in file.spritesheets {
            let path = opts.input_path(&dir.join(spritesheet));
            let Some(dir) = path.parent() else {
                bail!("invalid spritesheet file path {}", path.display());
            };
            let parsed = parse_spritesheet(&path)?;
            let mut sprites = vec![];
            for (name, image) in parsed.images {
                sprites.push(name.clone());
                assets.images.insert(name, image.fix(opts, dir, palette));
            }
            for (name, animation) in parsed.animations {
                sprites.push(name.clone());
                assets
                    .animations
                    .insert(name, animation.fix(opts, dir, palette));
            }
            spritesheet_sprites.insert(path, sprites);
        }

        for (name, font) in file.fonts {
            assets.fonts.insert(name, font.fix_files(opts, dir));
        }
        for (name, image) in file.images {
            assets.images.insert(name, image.fix(opts, dir, palette));
        }
        for (name, animation) in file.animations {
            let animation: RawAnimation = animation.into();
            assets
                .animations
                .insert(name, animation.fix(opts, dir, palette));
        }
        for (name, mask) in file.masks {
            assets.masks.insert(name, mask.fix_files(opts, dir));
        }
        for (name, bg_sprite_map) in file.bg_sprite_maps {
            assets
                .bg_sprite_maps
                .insert(name, bg_sprite_map.fix_files(opts, dir));
        }
    }
    for (name, bg_sprite_map) in &mut assets.bg_sprite_maps {
        for spritesheet in &bg_sprite_map.spritesheets {
            let Some(sprites) = spritesheet_sprites.get(spritesheet) else {
                bail!(
                    "unrecognized spritesheet \"{}\" for bg sprite map \"{}\"",
                    spritesheet.display(),
                    name
                );
            };
            for sprite in sprites {
                bg_sprite_map.sprites.insert(
                    sprite.clone(),
                    RawBgSprite::Image {
                        image: sprite.clone(),
                    },
                );
            }
        }
    }
    Ok(assets)
}

struct ParsedSpritesheet {
    images: Vec<(String, RawImage)>,
    animations: Vec<(String, RawAnimation)>,
}

fn parse_spritesheet(path: &Path) -> Result<ParsedSpritesheet> {
    let file = std::fs::read_to_string(path)
        .with_context(|| format!("could not read config file {}", path.display()))?;
    let file: RawSpritesheet = toml::from_str(&file)?;
    let palette = file.palette;

    let mut sprites = vec![];
    let mut animations = vec![];
    let spacing = (
        file.sprite_size.0 as isize + file.sprite_margin.0,
        file.sprite_size.1 as isize + file.sprite_margin.1,
    );
    let sprite_to_image = |sprite: RawSprite| {
        let position = (
            file.offset.0 + spacing.0 * sprite.position.0,
            file.offset.1 + spacing.1 * sprite.position.1,
        );
        let region = RawImageRegion {
            file: file.file.clone(),
            hflip: sprite.hflip,
            vflip: sprite.vflip,
            transpose: sprite.transpose,
            rotate: sprite.rotate,
            scale: sprite.scale,
            position: Some(position),
            size: Some(file.sprite_size),
        };
        RawImage {
            chardata: file.chardata.clone(),
            palette,
            data: RawImageData::Mono(region),
        }
    };
    for (name, sprite) in file.sprites {
        sprites.push((name, sprite_to_image(sprite)));
    }
    for (name, animation) in file.animations {
        if animation.is_empty() {
            bail!("animation {name} has no frames");
        }
        animations.push((
            name,
            RawAnimation {
                chardata: file.chardata.clone(),
                images: animation.into_iter().map(sprite_to_image).collect(),
            },
        ));
    }
    Ok(ParsedSpritesheet {
        images: sprites,
        animations,
    })
}
