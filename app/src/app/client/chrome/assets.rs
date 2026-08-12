use super::super::*;

use std::{borrow::Cow, fs};

pub(in crate::app) struct Assets {
    pub(in crate::app) base: PathBuf,
}

pub(in crate::app::client) struct ModalFrame {
    social: Arc<RenderImage>,
    connection: Arc<RenderImage>,
    join: Arc<RenderImage>,
    settings: Arc<RenderImage>,
    update: Arc<RenderImage>,
}

pub(in crate::app::client) struct ButtonFrames {
    images: HashMap<(u32, u32, bool, bool), Arc<RenderImage>>,
}

pub(in crate::app::client) struct PortraitRegistry {
    atlases: Vec<Option<image::RgbaImage>>,
    cells: BTreeMap<ImageTableEntry, Arc<RenderImage>>,
}

impl PortraitRegistry {
    const ATLAS_COUNT: usize = 17;
    const CELLS_PER_ROW: u16 = 6;
    const CELL_SIZE: u32 = 152;

    pub(in crate::app::client) fn load(root: &std::path::Path) -> Self {
        let atlases = (0..Self::ATLAS_COUNT)
            .map(|index| {
                image::open(root.join(format!("images/portrait-atlases/atlas-{index:02}.png")))
                    .ok()
                    .map(image::DynamicImage::into_rgba8)
            })
            .collect();
        Self {
            atlases,
            cells: BTreeMap::new(),
        }
    }

    pub(in crate::app::client) fn image(
        &mut self,
        entry: ImageTableEntry,
    ) -> Option<Arc<RenderImage>> {
        if let Some(image) = self.cells.get(&entry) {
            return Some(image.clone());
        }
        if entry.offset >= Self::CELLS_PER_ROW * Self::CELLS_PER_ROW {
            return None;
        }
        let atlas = self.atlases.get(usize::from(entry.table_id))?.as_ref()?;
        let column = u32::from(entry.offset % Self::CELLS_PER_ROW);
        let row = u32::from(entry.offset / Self::CELLS_PER_ROW);
        let mut cell = image::imageops::crop_imm(
            atlas,
            column * Self::CELL_SIZE,
            row * Self::CELL_SIZE,
            Self::CELL_SIZE,
            Self::CELL_SIZE,
        )
        .to_image();
        // renderimage consumes Blade's native BGRA ordering. Resource-backed
        // images are converted by the loader; atlas cells bypass that path.
        for pixel in cell.chunks_exact_mut(4) {
            pixel.swap(0, 2);
        }
        let image = Arc::new(RenderImage::new(vec![image::Frame::new(cell)]));
        self.cells.insert(entry, image.clone());
        Some(image)
    }
}

/// blade's atlas sampler bleeds horizontally when a one-pixel texture is
/// stretched across the window. preserve the same authored column by widening
/// it before it reaches the image cache.
pub(in crate::app::client) fn load_top_nav_background(
    root: &std::path::Path,
) -> Option<Arc<RenderImage>> {
    let source = image::open(root.join("images/curated/controls/top-nav-background.png"))
        .ok()?
        .into_rgba8();
    let mut buffer = image::RgbaImage::new(4, source.height());
    for y in 0..source.height() {
        let source = source.get_pixel(0, y).0;
        let alpha = u16::from(source[3]);
        let backdrop = [12_u8, 15, 21];
        let pixel = image::Rgba([
            ((u16::from(source[0]) * alpha + u16::from(backdrop[0]) * (255 - alpha) + 127) / 255)
                as u8,
            ((u16::from(source[1]) * alpha + u16::from(backdrop[1]) * (255 - alpha) + 127) / 255)
                as u8,
            ((u16::from(source[2]) * alpha + u16::from(backdrop[2]) * (255 - alpha) + 127) / 255)
                as u8,
            255,
        ]);
        for x in 0..buffer.width() {
            buffer.put_pixel(x, y, pixel);
        }
    }
    for pixel in buffer.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    Some(Arc::new(RenderImage::new(vec![image::Frame::new(buffer)])))
}

impl ModalFrame {
    pub(in crate::app::client) fn load(root: &std::path::Path) -> Option<Self> {
        let source = image::open(root.join("images/nine-patch/dialogs/modal-frame.png"))
            .ok()?
            .into_rgba8();

        let render = |width, height| {
            let mut image = resize_modal_frame(&source, width, height);
            for pixel in image.chunks_exact_mut(4) {
                pixel.swap(0, 2);
            }
            Arc::new(RenderImage::new(vec![image::Frame::new(image)]))
        };

        Some(Self {
            social: render(400, 440),
            connection: render(540, 250),
            join: render(640, 580),
            settings: render(944, 620),
            update: render(780, 590),
        })
    }

    pub(in crate::app::client) fn image(&self, width: f32, height: f32) -> Arc<RenderImage> {
        match (width.round() as u32, height.round() as u32) {
            (400, 440) => self.social.clone(),
            (540, 250) => self.connection.clone(),
            (640, 580) => self.join.clone(),
            (944, 620) => self.settings.clone(),
            (780, 590) => self.update.clone(),
            _ => self.social.clone(),
        }
    }
}

impl ButtonFrames {
    pub(in crate::app::client) fn load(root: &std::path::Path) -> Option<Self> {
        let standard_idle = image::open(root.join("images/nine-patch/controls/button-idle.png"))
            .ok()?
            .into_rgba8();
        let standard_active =
            image::open(root.join("images/nine-patch/controls/button-active.png"))
                .ok()?
                .into_rgba8();
        let warning_idle =
            image::open(root.join("images/nine-patch/controls/warning-button-idle.png"))
                .ok()?
                .into_rgba8();
        let warning_active =
            image::open(root.join("images/nine-patch/controls/warning-button-active.png"))
                .ok()?
                .into_rgba8();
        let mut images = HashMap::new();
        for (width, height) in [
            (132, 36),
            (132, 44),
            (136, 42),
            (142, 42),
            (148, 42),
            (185, 50),
            (190, 64),
            (260, 42),
        ] {
            for (warning, active, source) in [
                (false, false, &standard_idle),
                (false, true, &standard_active),
                (true, false, &warning_idle),
                (true, true, &warning_active),
            ] {
                let mut rendered = resize_button_frame(
                    source,
                    width,
                    height + (BUTTON_ART_VERTICAL_BLEED as u32) * 2,
                );
                for pixel in rendered.chunks_exact_mut(4) {
                    pixel.swap(0, 2);
                }
                images.insert(
                    (width, height, warning, active),
                    Arc::new(RenderImage::new(vec![image::Frame::new(rendered)])),
                );
            }
        }
        Some(Self { images })
    }

    pub(in crate::app::client) fn image(
        &self,
        width: f32,
        height: f32,
        warning: bool,
        active: bool,
    ) -> Option<Arc<RenderImage>> {
        self.images
            .get(&(width.round() as u32, height.round() as u32, warning, active))
            .cloned()
    }
}

/// match the authored 18-point cap insets before handing the image to the renderer.
/// stretching the entire 288x76 source makes the rails and end caps enormous.
pub(in crate::app::client) fn resize_button_frame(
    source: &image::RgbaImage,
    width: u32,
    height: u32,
) -> image::RgbaImage {
    const CAP: u32 = 18;
    let source_center_width = source.width() - CAP * 2;
    let source_center_height = source.height() - CAP * 2;
    let target_center_width = width.saturating_sub(CAP * 2);
    let target_center_height = height.saturating_sub(CAP * 2);
    let mut target = image::RgbaImage::new(width, height);
    let x_segments = [
        (0, CAP, 0, CAP),
        (CAP, source_center_width, CAP, target_center_width),
        (source.width() - CAP, CAP, width - CAP, CAP),
    ];
    let y_segments = [
        (0, CAP, 0, CAP),
        (CAP, source_center_height, CAP, target_center_height),
        (source.height() - CAP, CAP, height - CAP, CAP),
    ];
    for (source_x, source_width, target_x, target_width) in x_segments {
        for (source_y, source_height, target_y, target_height) in y_segments {
            if target_width == 0 || target_height == 0 {
                continue;
            }
            let patch =
                image::imageops::crop_imm(source, source_x, source_y, source_width, source_height)
                    .to_image();
            let patch = if source_width == target_width && source_height == target_height {
                patch
            } else {
                image::imageops::resize(
                    &patch,
                    target_width,
                    target_height,
                    image::imageops::FilterType::Triangle,
                )
            };
            image::imageops::replace(
                &mut target,
                &patch,
                i64::from(target_x),
                i64::from(target_y),
            );
        }
    }
    target
}

/// render the 176x22 cap-inset image once for each authored modal size.
/// a single stretched image distorts the rails, while nine separate image
/// elements can expose atlas texels at their joins. Precomposing the nine-patch
/// preserves the fixed rails and gives Blade one seamless texture to sample.
fn resize_modal_frame(source: &image::RgbaImage, width: u32, height: u32) -> image::RgbaImage {
    const LEFT: u32 = 176;
    const RIGHT: u32 = 176;
    const TOP: u32 = 22;
    const BOTTOM: u32 = 22;

    let source_width = source.width();
    let source_height = source.height();
    let source_center_width = source_width - LEFT - RIGHT;
    let source_center_height = source_height - TOP - BOTTOM;
    let target_center_width = width - LEFT - RIGHT;
    let target_center_height = height - TOP - BOTTOM;
    let mut target = image::RgbaImage::new(width, height);

    let x_segments = [
        (0, LEFT, 0, LEFT),
        (LEFT, source_center_width, LEFT, target_center_width),
        (source_width - RIGHT, RIGHT, width - RIGHT, RIGHT),
    ];
    let y_segments = [
        (0, TOP, 0, TOP),
        (TOP, source_center_height, TOP, target_center_height),
        (source_height - BOTTOM, BOTTOM, height - BOTTOM, BOTTOM),
    ];

    for (source_x, source_patch_width, target_x, target_patch_width) in x_segments {
        for (source_y, source_patch_height, target_y, target_patch_height) in y_segments {
            let patch = image::imageops::crop_imm(
                source,
                source_x,
                source_y,
                source_patch_width,
                source_patch_height,
            )
            .to_image();
            let patch = if source_patch_width == target_patch_width
                && source_patch_height == target_patch_height
            {
                patch
            } else {
                image::imageops::resize(
                    &patch,
                    target_patch_width,
                    target_patch_height,
                    image::imageops::FilterType::Triangle,
                )
            };
            image::imageops::replace(
                &mut target,
                &patch,
                i64::from(target_x),
                i64::from(target_y),
            );
        }
    }

    target
}

impl AssetSource for Assets {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        fs::read(self.base.join(path))
            .map(|data| Some(Cow::Owned(data)))
            .map_err(Into::into)
    }

    fn list(&self, path: &str) -> gpui::Result<Vec<SharedString>> {
        fs::read_dir(self.base.join(path))
            .map(|entries| {
                entries
                    .filter_map(|entry| {
                        entry
                            .ok()
                            .and_then(|entry| entry.file_name().into_string().ok())
                            .map(SharedString::from)
                    })
                    .collect()
            })
            .map_err(Into::into)
    }
}

pub(in crate::app) fn load_fonts(root: &std::path::Path, cx: &mut App) {
    let fonts = [
        "fonts/eurostileext-med.otf",
        "fonts/eurostile-reg.otf",
        "fonts/eurostile-bol.otf",
        "fonts/bl.ttf",
    ]
    .into_iter()
    .filter_map(|path| fs::read(root.join(path)).ok().map(Cow::Owned))
    .collect();
    if let Err(error) = cx.text_system().add_fonts(fonts) {
        eprintln!("could not load bundled ui fonts: {error:#}");
    }
}
