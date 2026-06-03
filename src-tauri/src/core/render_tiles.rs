use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::Hash;
use std::mem::{align_of, size_of};
use std::sync::LazyLock;

use fast_image_resize::images::{TypedImage, TypedImageRef};
use fast_image_resize::{FilterType, ResizeAlg, ResizeOptions, Resizer, pixels};
use ndarray::ArrayView2;
use serde::Serialize;

pub const WAVEFORM_TILE_BINS: usize = 1024;
pub const SPECTROGRAM_TILE_SIZE: usize = 512;
const SPECTROGRAM_TILE_GUTTER: usize = 4;
const DEFAULT_CACHE_BUDGET_BYTES: usize = 512 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum TileKind {
    Waveform,
    Spectrogram,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum RenderTileKey {
    Waveform {
        id: usize,
        ch: usize,
        revision: u64,
        level: u32,
        tile_index: u32,
    },
    Spectrogram {
        id: usize,
        ch: usize,
        revision: u64,
        level_x: u32,
        level_y: u32,
        tile_x: u32,
        tile_y: u32,
    },
}

impl RenderTileKey {
    fn kind(&self) -> TileKind {
        match self {
            Self::Waveform { .. } => TileKind::Waveform,
            Self::Spectrogram { .. } => TileKind::Spectrogram,
        }
    }
}

struct CacheEntry {
    bytes: Vec<u8>,
    last_used: u64,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderTileCacheStats {
    pub bytes: usize,
    pub entries: usize,
    pub budget_bytes: usize,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioRenderMetadata {
    pub waveform_revision: u64,
    pub spectrogram_revision: u64,
    pub sample_rate: u32,
    pub sample_count: usize,
    pub track_sec: f64,
    pub is_clipped: bool,
    pub spectrogram_width: usize,
    pub spectrogram_height: usize,
    pub waveform_tile_bins: usize,
    pub spectrogram_tile_size: usize,
}

pub struct RenderTileCache {
    entries: HashMap<RenderTileKey, CacheEntry>,
    bytes: usize,
    budget_bytes: usize,
    tick: u64,
    waveform_revision: u64,
    spectrogram_revision: u64,
    colormap_rgba: Vec<u8>,
}

impl Default for RenderTileCache {
    fn default() -> Self {
        Self::with_budget(DEFAULT_CACHE_BUDGET_BYTES)
    }
}

impl RenderTileCache {
    pub fn with_budget(budget_bytes: usize) -> Self {
        Self {
            entries: HashMap::new(),
            bytes: 0,
            budget_bytes,
            tick: 0,
            waveform_revision: 1,
            spectrogram_revision: 1,
            colormap_rgba: vec![0, 0, 0, 255, 255, 255, 255, 255],
        }
    }

    pub fn set_colormap(&mut self, colormap_rgba: Vec<u8>) {
        if colormap_rgba.len() >= 4 && colormap_rgba.len().is_multiple_of(4) {
            self.colormap_rgba = colormap_rgba;
        }
        self.invalidate_spectrogram();
    }

    pub fn invalidate_waveform(&mut self) {
        self.waveform_revision = self.waveform_revision.wrapping_add(1).max(1);
        self.remove_kind(TileKind::Waveform);
    }

    pub fn invalidate_spectrogram(&mut self) {
        self.spectrogram_revision = self.spectrogram_revision.wrapping_add(1).max(1);
        self.remove_kind(TileKind::Spectrogram);
    }

    pub fn invalidate_all(&mut self) {
        self.invalidate_waveform();
        self.invalidate_spectrogram();
    }

    pub fn metadata(
        &self,
        wav: &[f32],
        sample_rate: u32,
        track_sec: f64,
        is_clipped: bool,
        spectrogram_shape: Option<(usize, usize)>,
    ) -> AudioRenderMetadata {
        let (spectrogram_height, spectrogram_width) = spectrogram_shape.unwrap_or_default();
        AudioRenderMetadata {
            waveform_revision: self.waveform_revision,
            spectrogram_revision: self.spectrogram_revision,
            sample_rate,
            sample_count: wav.len(),
            track_sec,
            is_clipped,
            spectrogram_width,
            spectrogram_height,
            waveform_tile_bins: WAVEFORM_TILE_BINS,
            spectrogram_tile_size: SPECTROGRAM_TILE_SIZE,
        }
    }

    pub fn stats(&self) -> RenderTileCacheStats {
        RenderTileCacheStats {
            bytes: self.bytes,
            entries: self.entries.len(),
            budget_bytes: self.budget_bytes,
        }
    }

    pub fn waveform_tile(
        &mut self,
        id: usize,
        ch: usize,
        wav: &[f32],
        level: u32,
        tile_index: u32,
    ) -> Vec<u8> {
        let key = RenderTileKey::Waveform {
            id,
            ch,
            revision: self.waveform_revision,
            level,
            tile_index,
        };
        if let Some(bytes) = self.get(&key) {
            return bytes;
        }

        let bytes = encode_waveform_tile(wav, self.waveform_revision, level, tile_index);
        self.insert(key, bytes.clone());
        bytes
    }

    pub fn spectrogram_tile(
        &mut self,
        id: usize,
        ch: usize,
        spectrogram: ArrayView2<'_, u16>,
        level_x: u32,
        level_y: u32,
        tile_x: u32,
        tile_y: u32,
    ) -> Vec<u8> {
        let key = RenderTileKey::Spectrogram {
            id,
            ch,
            revision: self.spectrogram_revision,
            level_x,
            level_y,
            tile_x,
            tile_y,
        };
        if let Some(bytes) = self.get(&key) {
            return bytes;
        }

        let bytes = encode_spectrogram_tile(
            spectrogram,
            &self.colormap_rgba,
            self.spectrogram_revision,
            level_x,
            level_y,
            tile_x,
            tile_y,
        );
        self.insert(key, bytes.clone());
        bytes
    }

    fn get(&mut self, key: &RenderTileKey) -> Option<Vec<u8>> {
        self.tick = self.tick.wrapping_add(1);
        let entry = self.entries.get_mut(key)?;
        entry.last_used = self.tick;
        Some(entry.bytes.clone())
    }

    fn insert(&mut self, key: RenderTileKey, bytes: Vec<u8>) {
        self.tick = self.tick.wrapping_add(1);
        self.bytes += bytes.len();
        self.entries.insert(
            key,
            CacheEntry {
                bytes,
                last_used: self.tick,
            },
        );
        self.evict();
    }

    fn evict(&mut self) {
        while self.bytes > self.budget_bytes {
            let Some(key) = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.last_used)
                .map(|(key, _)| key.clone())
            else {
                break;
            };
            if let Some(entry) = self.entries.remove(&key) {
                self.bytes -= entry.bytes.len();
            }
        }
    }

    fn remove_kind(&mut self, kind: TileKind) {
        self.entries.retain(|key, entry| {
            if key.kind() == kind {
                self.bytes -= entry.bytes.len();
                false
            } else {
                true
            }
        });
    }
}

fn encode_waveform_tile(wav: &[f32], revision: u64, level: u32, tile_index: u32) -> Vec<u8> {
    let samples_per_bin = 1usize.checked_shl(level).unwrap_or(usize::MAX);
    let tile_samples = WAVEFORM_TILE_BINS.saturating_mul(samples_per_bin);
    let start = (tile_index as usize).saturating_mul(tile_samples);
    let end = wav.len().min(start.saturating_add(tile_samples));
    let bin_count = if start >= end {
        0
    } else {
        (end - start).div_ceil(samples_per_bin)
    };

    let mut bytes = Vec::with_capacity(24 + bin_count * 12);
    bytes.extend_from_slice(&revision.to_le_bytes());
    bytes.extend_from_slice(&(bin_count as u32).to_le_bytes());
    bytes.extend_from_slice(&(samples_per_bin.min(u32::MAX as usize) as u32).to_le_bytes());
    bytes.extend_from_slice(&tile_index.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    for bin in 0..bin_count {
        let bin_start = start + bin * samples_per_bin;
        let bin_end = end.min(bin_start.saturating_add(samples_per_bin));
        let slice = &wav[bin_start..bin_end];
        let mut min = f32::INFINITY;
        let mut max = f32::NEG_INFINITY;
        let mut sum = 0.0;
        for &sample in slice {
            min = min.min(sample);
            max = max.max(sample);
            sum += sample;
        }
        let representative = sum / slice.len() as f32;
        bytes.extend_from_slice(&min.to_le_bytes());
        bytes.extend_from_slice(&max.to_le_bytes());
        bytes.extend_from_slice(&representative.to_le_bytes());
    }
    bytes
}

fn encode_spectrogram_tile(
    spectrogram: ArrayView2<'_, u16>,
    colormap_rgba: &[u8],
    revision: u64,
    level_x: u32,
    level_y: u32,
    tile_x: u32,
    tile_y: u32,
) -> Vec<u8> {
    let scale_x = 1usize.checked_shl(level_x).unwrap_or(usize::MAX);
    let scale_y = 1usize.checked_shl(level_y).unwrap_or(usize::MAX);
    let lod_width = spectrogram.shape()[1].div_ceil(scale_x);
    let lod_height = spectrogram.shape()[0].div_ceil(scale_y);
    let start_x = (tile_x as usize).saturating_mul(SPECTROGRAM_TILE_SIZE);
    let start_y = (tile_y as usize).saturating_mul(SPECTROGRAM_TILE_SIZE);
    let core_width = lod_width.saturating_sub(start_x).min(SPECTROGRAM_TILE_SIZE);
    let core_height = lod_height
        .saturating_sub(start_y)
        .min(SPECTROGRAM_TILE_SIZE);
    let origin_x = start_x.saturating_sub(SPECTROGRAM_TILE_GUTTER);
    let origin_y = start_y.saturating_sub(SPECTROGRAM_TILE_GUTTER);
    let (width, height) = if core_width == 0 || core_height == 0 {
        (0, 0)
    } else {
        (
            lod_width
                .min(start_x + core_width + SPECTROGRAM_TILE_GUTTER)
                .saturating_sub(origin_x),
            lod_height
                .min(start_y + core_height + SPECTROGRAM_TILE_GUTTER)
                .saturating_sub(origin_y),
        )
    };

    let mut bytes = Vec::with_capacity(40 + width * height * 4);
    bytes.extend_from_slice(&revision.to_le_bytes());
    bytes.extend_from_slice(&(width as u32).to_le_bytes());
    bytes.extend_from_slice(&(height as u32).to_le_bytes());
    bytes.extend_from_slice(&level_x.to_le_bytes());
    bytes.extend_from_slice(&level_y.to_le_bytes());
    bytes.extend_from_slice(&tile_x.to_le_bytes());
    bytes.extend_from_slice(&tile_y.to_le_bytes());
    bytes.extend_from_slice(&(origin_x as u32).to_le_bytes());
    bytes.extend_from_slice(&(origin_y as u32).to_le_bytes());

    if width == 0 || height == 0 {
        return bytes;
    }

    let lod_pixels = resize_spectrogram_tile(
        spectrogram,
        lod_width,
        lod_height,
        origin_x,
        origin_y,
        width,
        height,
    );
    let color_count = colormap_rgba.len() / 4;
    for row in lod_pixels.chunks_exact(width).rev() {
        for value in row {
            let color_index = if color_count <= 1 {
                0
            } else {
                (value.0 as usize * (color_count - 1) + u16::MAX as usize / 2) / u16::MAX as usize
            };
            let offset = color_index * 4;
            bytes.extend_from_slice(&colormap_rgba[offset..offset + 4]);
        }
    }
    bytes
}

fn resize_spectrogram_tile(
    spectrogram: ArrayView2<'_, u16>,
    lod_width: usize,
    lod_height: usize,
    start_x: usize,
    start_y: usize,
    width: usize,
    height: usize,
) -> Vec<pixels::U16> {
    static RESIZE_OPTIONS: LazyLock<ResizeOptions> = LazyLock::new(|| {
        ResizeOptions::new().resize_alg(ResizeAlg::Convolution(FilterType::Lanczos3))
    });
    thread_local! {
        static RESIZER: RefCell<Resizer> = RefCell::new(Resizer::new());
    }

    let src_width = spectrogram.shape()[1];
    let src_height = spectrogram.shape()[0];
    let src_pixels = as_u16_pixels(
        spectrogram
            .as_slice()
            .expect("spectrogram must be contiguous"),
    );
    let src_image = TypedImageRef::new(src_width as u32, src_height as u32, src_pixels).unwrap();
    let mut dst_pixels = vec![pixels::U16::new(0); width * height];
    let mut dst_image =
        TypedImage::<pixels::U16>::from_pixels_slice(width as u32, height as u32, &mut dst_pixels)
            .unwrap();
    let left = start_x as f64 * src_width as f64 / lod_width as f64;
    let top = start_y as f64 * src_height as f64 / lod_height as f64;
    let right = (start_x + width) as f64 * src_width as f64 / lod_width as f64;
    let bottom = (start_y + height) as f64 * src_height as f64 / lod_height as f64;
    let options = RESIZE_OPTIONS.crop(left, top, right - left, bottom - top);
    RESIZER.with_borrow_mut(|resizer| {
        resizer
            .resize_typed(&src_image, &mut dst_image, &options)
            .unwrap();
    });
    dst_pixels
}

fn as_u16_pixels(values: &[u16]) -> &[pixels::U16] {
    // fast_image_resize::pixels::U16 is a repr(C) single-u16 pixel wrapper.
    debug_assert_eq!(size_of::<u16>(), size_of::<pixels::U16>());
    debug_assert_eq!(align_of::<u16>(), align_of::<pixels::U16>());
    unsafe { std::slice::from_raw_parts(values.as_ptr().cast(), values.len()) }
}

#[cfg(test)]
mod tests {
    use ndarray::array;

    use super::*;

    #[test]
    fn waveform_tile_contains_min_max_and_representative() {
        let bytes = encode_waveform_tile(&[-1.0, 0.0, 0.5, 1.0], 3, 1, 0);
        let view = bytes.as_slice();
        assert_eq!(u32::from_le_bytes(view[8..12].try_into().unwrap()), 2);
        assert_eq!(f32::from_le_bytes(view[24..28].try_into().unwrap()), -1.0);
        assert_eq!(f32::from_le_bytes(view[28..32].try_into().unwrap()), 0.0);
        assert_eq!(f32::from_le_bytes(view[32..36].try_into().unwrap()), -0.5);
    }

    #[test]
    fn waveform_tile_handles_partial_last_tile() {
        let wav = vec![0.25; WAVEFORM_TILE_BINS + 1];
        let bytes = encode_waveform_tile(&wav, 1, 0, 1);
        assert_eq!(u32::from_le_bytes(bytes[8..12].try_into().unwrap()), 1);
    }

    #[test]
    fn spectrogram_tile_handles_lod_and_edges() {
        let spec = array![[0u16, u16::MAX], [u16::MAX, u16::MAX]];
        let colors = [0, 0, 0, 255, 255, 0, 0, 255];
        let bytes = encode_spectrogram_tile(spec.view(), &colors, 4, 1, 1, 0, 0);
        assert_eq!(u32::from_le_bytes(bytes[8..12].try_into().unwrap()), 1);
        assert_eq!(u32::from_le_bytes(bytes[12..16].try_into().unwrap()), 1);
        assert_eq!(&bytes[40..], &[255, 0, 0, 255]);
    }

    #[test]
    fn spectrogram_tile_handles_partial_last_tile() {
        let spec = ndarray::Array2::from_elem(
            (SPECTROGRAM_TILE_SIZE + 1, SPECTROGRAM_TILE_SIZE + 1),
            u16::MAX,
        );
        let colors = [0, 0, 0, 255, 255, 0, 0, 255];
        let bytes = encode_spectrogram_tile(spec.view(), &colors, 4, 0, 0, 1, 1);
        assert_eq!(u32::from_le_bytes(bytes[8..12].try_into().unwrap()), 5);
        assert_eq!(u32::from_le_bytes(bytes[12..16].try_into().unwrap()), 5);
        assert_eq!(u32::from_le_bytes(bytes[32..36].try_into().unwrap()), 508);
        assert_eq!(u32::from_le_bytes(bytes[36..40].try_into().unwrap()), 508);
        assert!(
            bytes[40..]
                .chunks_exact(4)
                .all(|pixel| pixel == [255, 0, 0, 255])
        );
    }

    #[test]
    fn spectrogram_tile_outputs_high_frequencies_first() {
        let spec = array![[0u16], [u16::MAX]];
        let colors = [0, 0, 0, 255, 255, 0, 0, 255];
        let bytes = encode_spectrogram_tile(spec.view(), &colors, 4, 0, 0, 0, 0);
        assert_eq!(&bytes[40..44], &[255, 0, 0, 255]);
        assert_eq!(&bytes[44..48], &[0, 0, 0, 255]);
    }

    #[test]
    fn cache_evicts_and_invalidates() {
        let mut cache = RenderTileCache::with_budget(24 + WAVEFORM_TILE_BINS * 12);
        let wav = vec![0.0; WAVEFORM_TILE_BINS * 2];
        cache.waveform_tile(1, 0, &wav, 0, 0);
        cache.waveform_tile(1, 0, &wav, 0, 1);
        assert_eq!(cache.stats().entries, 1);
        assert!(cache.stats().bytes <= cache.stats().budget_bytes);
        let revision = cache.waveform_revision;
        cache.invalidate_waveform();
        assert!(cache.stats().entries == 0);
        assert!(cache.waveform_revision > revision);

        let spec = array![[0u16]];
        cache.spectrogram_tile(1, 0, spec.view(), 0, 0, 0, 0);
        let revision = cache.spectrogram_revision;
        cache.invalidate_spectrogram();
        assert!(cache.stats().entries == 0);
        assert!(cache.spectrogram_revision > revision);
    }

    #[test]
    fn metadata_reports_clipped_waveform_and_dimensions() {
        let cache = RenderTileCache::default();
        let metadata = cache.metadata(&[0.0, 1.0], 48_000, 2.0 / 48_000.0, true, Some((2, 3)));
        assert!(metadata.is_clipped);
        assert_eq!(metadata.sample_count, 2);
        assert_eq!(metadata.spectrogram_height, 2);
        assert_eq!(metadata.spectrogram_width, 3);
    }
}
