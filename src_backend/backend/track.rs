use std::fmt;
use std::ops::Index;
use std::path::PathBuf;

use identity_hash::{IntMap, IntSet};
use kittyaudio::Frame;
use ndarray::prelude::*;
use parking_lot::RwLock;
use rayon::prelude::*;
use symphonia::core::errors::Error as SymphoniaError;

use super::IdChVec;
use super::audio::{Audio, AudioFormatInfo, open_audio_file};
use super::dynamics::{
    AudioStats, GuardClippingMode, GuardClippingResult, GuardClippingStats, Normalize,
    NormalizeTarget, StatCalculator,
};
use super::simd::find_min_max;
use super::spectrogram::{SpecSetting, SrWinNfft};
use super::tuple_hasher::TupleIntSet;
use super::utils::unique_filenames;
use super::visualize::{WavDrawingInfoCache, WavDrawingInfoInternal};

const CACHE_PX_PER_SEC: f32 = 2. / (1. / 20.); // 2px per period of 20Hz sine wave

macro_rules! iter_filtered {
    ($vec: expr) => {
        $vec.iter().filter_map(|x| x.as_ref())
    };
}

macro_rules! par_iter_mut_filtered {
    ($vec: expr) => {
        $vec.par_iter_mut().filter_map(|x| x.as_mut())
    };
}

macro_rules! indexed_iter_filtered {
    ($vec: expr) => {
        $vec.iter()
            .enumerate()
            .filter_map(|(i, x)| x.as_ref().map(|x| (i, x)))
    };
}

macro_rules! indexed_par_iter_mut_filtered {
    ($vec: expr) => {
        $vec.par_iter_mut()
            .enumerate()
            .filter_map(|(i, x)| x.as_mut().map(|x| (i, x)))
    };
}

#[readonly::make]
pub struct AudioTrack {
    pub format_info: AudioFormatInfo,
    path: PathBuf,
    original: Audio,
    audio: Audio,
    interleaved: Vec<Frame>,
    stat_calculator: StatCalculator,
    wav_drawing_info_cache: RwLock<Option<WavDrawingInfoCache>>,
}

impl AudioTrack {
    pub fn new(path: String) -> Result<Self, SymphoniaError> {
        let (wavs, format_info) = open_audio_file(&path)?;
        let mut stat_calculator = StatCalculator::new(wavs.shape()[0] as u32, format_info.sr);
        let original = Audio::new(wavs, format_info.sr, &mut stat_calculator);

        let audio = original.clone();
        let interleaved = (&audio).into();

        Ok(AudioTrack {
            format_info,
            path: PathBuf::from(path).canonicalize().unwrap(),
            original,
            audio,
            interleaved,
            stat_calculator,
            wav_drawing_info_cache: RwLock::new(None),
        })
    }

    pub fn reload(&mut self) -> Result<bool, SymphoniaError> {
        let (wavs, format_info) = open_audio_file(self.path.to_string_lossy().as_ref())?;
        if wavs.view() == self.original.view() && format_info == self.format_info {
            return Ok(false);
        }
        self.stat_calculator
            .change_parameters(wavs.shape()[0] as u32, format_info.sr);
        let original = Audio::new(wavs, format_info.sr, &mut self.stat_calculator);

        self.format_info = format_info;
        self.original = original.clone();
        self.audio = original;
        self.interleaved = (&self.audio).into();
        self.refresh_wav_drawing_info_cache();

        Ok(true)
    }

    #[inline]
    pub fn channel(&self, ch: usize) -> ArrayView1<f32> {
        self.audio.channel(ch)
    }

    #[inline]
    pub fn interleaved_frames(&self) -> &[Frame] {
        &self.interleaved
    }

    #[inline]
    pub fn channel_for_drawing(&self, ch: usize) -> (ArrayView1<f32>, bool) {
        match self.guard_clip_result() {
            GuardClippingResult::WavBeforeClip(before_clip) => {
                (before_clip.slice(s![ch, ..]), true)
            }
            _ => (self.channel(ch), false),
        }
    }

    pub fn get_wav_drawing_info(
        &self,
        ch: usize,
        sec_range: (f64, f64),
        width: f32,
        height: f32,
        amp_range: (f32, f32),
        wav_stroke_width: f32,
        topbottom_context_size: f32,
        margin_ratio: f64,
    ) -> WavDrawingInfoInternal {
        let px_per_sec = width / (sec_range.1 - sec_range.0) as f32;
        let (return_cache, need_new_cache) = {
            if let Some(cache) = self.wav_drawing_info_cache.read().as_ref() {
                let return_cache = px_per_sec <= cache.px_per_sec;
                let need_new_cache = cache.wav_stroke_width != wav_stroke_width
                    || cache.topbottom_context_size != topbottom_context_size;
                (return_cache, need_new_cache)
            } else {
                self.set_wav_drawing_info_cache(wav_stroke_width, topbottom_context_size);
                let return_cache = px_per_sec
                    <= self
                        .wav_drawing_info_cache
                        .read()
                        .as_ref()
                        .unwrap()
                        .px_per_sec;
                (return_cache, false)
            }
        };

        // first, update cache
        if need_new_cache {
            self.set_wav_drawing_info_cache(wav_stroke_width, topbottom_context_size);
        }

        // second, calculate sliced wav drawing info
        if !return_cache {
            let (wav, is_clipped) = self.channel_for_drawing(ch);

            WavDrawingInfoInternal::from_wav_with_slicing(
                wav,
                self.sr(),
                sec_range,
                margin_ratio,
                width,
                height,
                amp_range,
                wav_stroke_width,
                topbottom_context_size,
                is_clipped,
                false,
            )
            .unwrap_or_default()
        } else {
            let cache = self.wav_drawing_info_cache.read();
            cache.as_ref().unwrap().slice(
                ch,
                sec_range,
                self.sec(),
                margin_ratio,
                width,
                height,
                amp_range,
                wav_stroke_width,
            )
        }
    }

    fn set_wav_drawing_info_cache(&self, wav_stroke_width: f32, topbottom_context_size: f32) {
        let px_per_sec = CACHE_PX_PER_SEC;
        let px_per_samples = (px_per_sec / self.sr() as f32).min(0.1);
        let width = self.audio.len() as f32 * px_per_samples;
        let (amp_ranges, kinds): (Vec<_>, Vec<_>) = (0..self.n_ch())
            .into_par_iter()
            .map(|ch| {
                let amp_range = {
                    let (wav, _) = self.channel_for_drawing(ch);
                    let (min, max) = find_min_max(wav.as_slice().unwrap());
                    (min.min(-1.), max.max(1.))
                };

                let (wav, is_clipped) = self.channel_for_drawing(ch);
                let kind = WavDrawingInfoInternal::from_wav(
                    wav,
                    self.sr(),
                    width,
                    10000., // just a large number. doesn't affect the performance
                    amp_range,
                    wav_stroke_width,
                    topbottom_context_size,
                    is_clipped,
                    true,
                )
                .into();
                (amp_range, kind)
            })
            .unzip();
        self.wav_drawing_info_cache
            .write()
            .replace(WavDrawingInfoCache {
                wav_stroke_width,
                topbottom_context_size,
                px_per_sec,
                drawing_info_kinds: kinds,
                amp_ranges,
            });
    }

    fn refresh_wav_drawing_info_cache(&mut self) {
        let (wav_stroke_width, topbottom_context_size) =
            if let Some(cache) = self.wav_drawing_info_cache.read().as_ref() {
                (cache.wav_stroke_width, cache.topbottom_context_size)
            } else {
                return;
            };
        self.set_wav_drawing_info_cache(wav_stroke_width, topbottom_context_size);
    }

    #[inline]
    pub fn path_string(&self) -> String {
        self.path.as_os_str().to_string_lossy().into_owned()
    }

    #[inline]
    pub fn sr(&self) -> u32 {
        self.audio.sr
    }

    #[inline]
    pub fn n_ch(&self) -> usize {
        self.audio.n_ch()
    }

    #[inline]
    pub fn sec(&self) -> f64 {
        self.audio.sec()
    }

    #[inline]
    pub fn is_path_same(&self, path: &str) -> bool {
        PathBuf::from(path)
            .canonicalize()
            .is_ok_and(|x| x == self.path)
    }

    #[inline]
    pub fn stats(&self) -> &AudioStats {
        &self.audio.stats
    }

    #[inline]
    pub fn guard_clip_result(&self) -> &GuardClippingResult<Ix2> {
        &self.audio.guard_clip_result
    }

    #[inline]
    pub fn guard_clip_stats(&self) -> ArrayView1<GuardClippingStats> {
        self.audio.guard_clip_stats.view()
    }
}

impl Normalize for AudioTrack {
    #[inline]
    fn stats_for_normalize(&self) -> &AudioStats {
        &self.original.stats
    }

    fn apply_gain(&mut self, gain: f32, guard_clipping_mode: GuardClippingMode) {
        if !gain.is_finite() || gain == 1. {
            self.audio.clone_from(&self.original);
        } else {
            self.audio.mutate(
                |wavs| {
                    azip!((y in wavs, x in self.original.view()) *y = gain * x);
                },
                &mut self.stat_calculator,
                guard_clipping_mode,
            );
        }
        self.interleaved = (&self.audio).into();
        self.refresh_wav_drawing_info_cache();
    }
}

impl fmt::Debug for AudioTrack {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "AudioTrack {{\n\
                path: {},\n sr: {} Hz, n_ch: {}, length: {}, sec: {}\n\
            }}",
            self.path.to_str().unwrap_or("err on path-to-str"),
            self.sr(),
            self.n_ch(),
            self.audio.len(),
            self.sec(),
        )
    }
}

#[readonly::make]
pub struct TrackList {
    pub max_sec: f64,
    pub common_normalize: NormalizeTarget,
    pub common_guard_clipping: GuardClippingMode,
    tracks: Vec<Option<AudioTrack>>,
    filenames: Vec<Option<String>>,
    id_max_sec: usize,
}

impl TrackList {
    pub fn new() -> Self {
        TrackList {
            max_sec: 0.,
            tracks: vec![None],
            filenames: Vec::new(),
            id_max_sec: 0,
            common_normalize: NormalizeTarget::Off,
            common_guard_clipping: GuardClippingMode::ReduceGlobalLevel,
        }
    }

    pub fn add_tracks(&mut self, id_list: Vec<usize>, path_list: Vec<String>) -> Vec<usize> {
        let id_tracks: Vec<_> = id_list
            .into_par_iter()
            .zip(path_list.into_par_iter())
            .filter_map(|(id, path)| {
                AudioTrack::new(path).ok().map(|mut track| {
                    track.normalize(self.common_normalize, self.common_guard_clipping);
                    (id, track)
                })
            })
            .collect();
        let mut added_ids = Vec::with_capacity(id_tracks.len());
        for (id, track) in id_tracks.into_iter() {
            let sec = track.sec();
            if sec > self.max_sec {
                self.max_sec = sec;
                self.id_max_sec = id;
            }
            if id >= self.tracks.len() {
                self.tracks
                    .extend((self.tracks.len()..(id + 1)).map(|_| None));
            }
            self.tracks[id].replace(track);
            added_ids.push(id);
        }

        self.update_filenames();
        added_ids
    }

    pub fn reload_tracks(&mut self, id_list: &[usize]) -> (Vec<usize>, Vec<usize>) {
        let reload_results: Vec<_> = indexed_par_iter_mut_filtered!(self.tracks)
            .filter(|(id, _)| id_list.contains(id))
            .map(|(id, track)| {
                let result = track.reload();
                if let Ok(true) = result {
                    track.normalize(self.common_normalize, self.common_guard_clipping);
                }
                result.map(|res| (id, track.sec(), res))
            })
            .collect();

        if reload_results.len() != id_list.len() {
            panic!("[reload_tracks] Wrong Track IDs {:?}!", id_list);
        }

        let mut reloaded_ids = Vec::new();
        let mut no_err_ids = Vec::new();
        for result in reload_results.into_iter() {
            match result {
                Ok((id, sec, true)) => {
                    if sec > self.max_sec {
                        self.max_sec = sec;
                        self.id_max_sec = id;
                    }
                    reloaded_ids.push(id);
                    no_err_ids.push(id);
                }
                Ok((id, _, false)) => {
                    no_err_ids.push(id);
                }
                Err(_) => {}
            }
        }
        (reloaded_ids, no_err_ids)
    }

    pub fn remove_tracks(&mut self, id_list: &[usize]) -> IdChVec {
        let mut need_update_max_sec = false;
        let mut removed_id_ch_tuples = IdChVec::new();
        for &id in id_list {
            if let Some(removed) = self.tracks[id].take() {
                for ch in 0..removed.n_ch() {
                    removed_id_ch_tuples.push((id, ch));
                }
                if id == self.id_max_sec {
                    need_update_max_sec = true;
                }
            } else {
                eprintln!("Track ID {} does not exist! Skip removing it ...", id);
            }
        }
        let last_id = self
            .tracks
            .iter()
            .enumerate()
            .rev()
            .find_map(|(i, x)| if x.is_some() { Some(i) } else { None });
        match last_id {
            Some(last_id) if self.tracks.len() > 2 * (last_id + 1) => {
                self.tracks.truncate(last_id + 1);
                self.tracks.shrink_to_fit();
            }
            None => {
                self.tracks.truncate(1);
                self.tracks.shrink_to_fit();
            }
            _ => {}
        }

        if need_update_max_sec {
            let (id, max_sec) = indexed_iter_filtered!(self.tracks)
                .map(|(id, track)| (id, track.sec()))
                .fold(
                    (0, 0.),
                    |(id_max, max), (id, sec)| {
                        if sec > max { (id, sec) } else { (id_max, max) }
                    },
                );
            self.id_max_sec = id;
            self.max_sec = max_sec;
        }
        self.update_filenames();
        removed_id_ch_tuples
    }

    pub fn set_common_normalize(&mut self, target: NormalizeTarget) {
        self.common_normalize = target;
        self.apply_normalize_guard_clipping();
    }

    pub fn set_common_guard_clipping(&mut self, guard_clipping_mode: GuardClippingMode) {
        self.common_guard_clipping = guard_clipping_mode;
        self.apply_normalize_guard_clipping();
    }

    #[inline]
    pub fn all_ids(&self) -> Vec<usize> {
        indexed_iter_filtered!(self.tracks)
            .map(|(id, _)| id)
            .collect()
    }

    #[inline]
    pub fn all_id_set(&self) -> IntSet<usize> {
        indexed_iter_filtered!(self.tracks)
            .map(|(id, _)| id)
            .collect()
    }

    #[inline]
    pub fn id_ch_tuples(&self) -> IdChVec {
        self.id_ch_tuples_from(&self.all_ids())
    }

    #[inline]
    pub fn id_ch_tuples_from(&self, id_list: &[usize]) -> IdChVec {
        id_list
            .iter()
            .filter(|&&id| self.has(id))
            .flat_map(|&id| {
                let n_ch = self[id].n_ch();
                (0..n_ch).map(move |ch| (id, ch))
            })
            .collect()
    }

    #[inline]
    pub fn max_sr(&self) -> u32 {
        iter_filtered!(self.tracks)
            .map(|track| track.sr())
            .max()
            .unwrap_or(0)
    }

    #[inline]
    pub fn construct_sr_win_nfft_set(
        &self,
        ids: &[usize],
        setting: &SpecSetting,
    ) -> TupleIntSet<SrWinNfft> {
        ids.iter()
            .map(|&id| setting.calc_sr_win_nfft(self[id].sr()))
            .collect()
    }

    #[inline]
    pub fn construct_all_sr_win_nfft_set(&self, setting: &SpecSetting) -> TupleIntSet<SrWinNfft> {
        self.construct_sr_win_nfft_set(&self.all_ids(), setting)
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.tracks.is_empty() || self.tracks.iter().all(|x| x.is_none())
    }

    #[inline]
    pub fn has(&self, id: usize) -> bool {
        id < self.tracks.len() && self.tracks[id].is_some()
    }

    #[inline]
    pub fn get(&self, id: usize) -> Option<&AudioTrack> {
        (id < self.tracks.len())
            .then(|| self.tracks[id].as_ref())
            .flatten()
    }

    #[inline]
    pub fn find_id_by_path(&self, path: &str) -> Option<usize> {
        indexed_iter_filtered!(self.tracks)
            .find_map(|(id, track)| track.is_path_same(path).then_some(id))
    }

    #[inline]
    pub fn filename(&self, id: usize) -> &str {
        self.filenames[id].as_ref().map_or("", |x| x)
    }

    fn update_filenames(&mut self) {
        let paths: IntMap<_, _> = indexed_iter_filtered!(self.tracks)
            .map(|(id, track)| (id, track.path.clone()))
            .collect();
        let mut filenames = unique_filenames(paths);
        self.filenames = (0..self.tracks.len())
            .map(|i| filenames.remove(&i))
            .collect();
    }

    fn apply_normalize_guard_clipping(&mut self) {
        par_iter_mut_filtered!(self.tracks).for_each(|track| {
            track.normalize(self.common_normalize, self.common_guard_clipping);
        });
    }
}

impl Index<usize> for TrackList {
    type Output = AudioTrack;
    fn index(&self, i: usize) -> &Self::Output {
        self.tracks[i]
            .as_ref()
            .expect("[get_track] Wrong Track ID!")
    }
}

#[cfg(test)]
mod tests {
    use approx::assert_abs_diff_eq;

    use super::*;

    #[test]
    fn calc_loudness_works() {
        let track = AudioTrack::new("samples/sample_48k.wav".into()).unwrap();
        assert_abs_diff_eq!(track.stats().global_lufs, -26.20331705029079);
    }
}
