use std::collections::{HashMap, HashSet};
use std::fmt;
use std::ops::Index;
use std::path::PathBuf;

use ndarray::prelude::*;
use symphonia::core::errors::Error as SymphoniaError;

use super::audio::{open_audio_file, Audio};
use super::display::{CalcWidth, IdxLen, PartGreyInfo};
use super::normalize::{GuardClippingMode, Normalize, NormalizeTarget};
use super::spectrogram::SrWinNfft;
use super::stats::{AudioStats, StatCalculator};
use super::utils::unique_filenames;
use super::{IdChVec, SpecSetting};

macro_rules! iter_filtered {
    ($vec: expr) => {
        $vec.iter().filter_map(|x| x.as_ref())
    };
}

macro_rules! iter_mut_filtered {
    ($vec: expr) => {
        $vec.iter_mut().filter_map(|x| x.as_mut())
    };
}

macro_rules! indexed_iter_filtered {
    ($vec: expr) => {
        $vec.iter()
            .enumerate()
            .filter_map(|(i, x)| x.as_ref().map(|x| (i, x)))
    };
}

#[readonly::make]
pub struct AudioTrack {
    pub format_desc: String,
    pub normalize_target: NormalizeTarget,
    path: PathBuf,
    original: Audio,
    audio: Audio,
    stat_calculator: StatCalculator,
    guard_clipping_mode: GuardClippingMode,
}

impl AudioTrack {
    pub fn new(path: String) -> Result<Self, SymphoniaError> {
        let (wavs, sr, format_desc) = open_audio_file(&path)?;
        let mut stat_calculator = StatCalculator::new(wavs.shape()[0] as u32, sr);
        let original = Audio::new(wavs, sr, &mut stat_calculator);

        let audio = original.clone();

        Ok(AudioTrack {
            format_desc,
            path: PathBuf::from(path).canonicalize().unwrap(),
            original,
            audio,
            stat_calculator,
            normalize_target: Default::default(),
            guard_clipping_mode: Default::default(),
        })
    }

    pub fn reload(&mut self) -> Result<bool, SymphoniaError> {
        let (wavs, sr, format_desc) = open_audio_file(self.path.to_string_lossy().as_ref())?;
        if wavs.view() == self.original.view() && sr == self.sr() && format_desc == self.format_desc
        {
            return Ok(false);
        }
        let original = Audio::new(wavs, sr, &mut self.stat_calculator);
        self.original = original.clone();
        self.audio = original;
        self.format_desc = format_desc;
        self.normalize(self.normalize_target, self.guard_clipping_mode);
        Ok(true)
    }

    #[inline]
    pub fn get_wav(&self, ch: usize) -> ArrayView1<f32> {
        self.audio.get_ch(ch)
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
            .map_or(false, |x| x == self.path)
    }

    #[inline]
    pub fn stats(&self) -> &AudioStats {
        &self.audio.stats
    }
}

impl CalcWidth for AudioTrack {
    fn calc_width(&self, px_per_sec: f64) -> u32 {
        self.audio.calc_width(px_per_sec)
    }

    fn calc_part_grey_info(
        &self,
        grey_width: u64,
        start_sec: f64,
        target_width: u32,
        px_per_sec: f64,
    ) -> PartGreyInfo {
        self.audio
            .calc_part_grey_info(grey_width, start_sec, target_width, px_per_sec)
    }

    fn calc_part_wav_info(&self, start_sec: f64, width: u32, px_per_sec: f64) -> IdxLen {
        self.audio.calc_part_wav_info(start_sec, width, px_per_sec)
    }

    fn decompose_width_of(&self, start_sec: f64, width: u32, px_per_sec: f64) -> (u32, u32, u32) {
        self.audio.decompose_width_of(start_sec, width, px_per_sec)
    }
}

impl Normalize for AudioTrack {
    #[inline]
    fn normalize(&mut self, target: NormalizeTarget, guard_clipping_mode: GuardClippingMode) {
        self.normalize_target = target;
        self.guard_clipping_mode = guard_clipping_mode;
        self.normalize_default(target, guard_clipping_mode);
    }

    fn stats_for_normalize(&self) -> &AudioStats {
        &self.original.stats
    }

    fn apply_gain(&mut self, gain: f32, guard_clipping_mode: GuardClippingMode) {
        if !gain.is_finite() {
            return;
        }
        self.audio.mutate(
            |wavs| {
                azip!((y in wavs, x in self.original.view()) *y = gain * x);
            },
            &mut self.stat_calculator,
            guard_clipping_mode,
        );
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
    tracks: Vec<Option<AudioTrack>>,
    filenames: Vec<Option<String>>,
    id_max_sec: usize,
}

impl TrackList {
    pub fn new() -> Self {
        TrackList {
            max_sec: 0.,
            tracks: Vec::new(),
            filenames: Vec::new(),
            id_max_sec: 0,
        }
    }

    pub fn add_tracks(&mut self, id_list: Vec<usize>, path_list: Vec<String>) -> Vec<usize> {
        let mut added_ids = Vec::new();
        for (id, path) in id_list.into_iter().zip(path_list) {
            if let Ok(track) = AudioTrack::new(path) {
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
        }

        self.update_filenames();
        added_ids
    }

    pub fn reload_tracks(&mut self, id_list: &[usize]) -> (Vec<usize>, Vec<usize>) {
        let mut reloaded_ids = Vec::new();
        let mut no_err_ids = Vec::new();
        for &id in id_list {
            let track = self.tracks[id]
                .as_mut()
                .expect(&format!("[reload_tracks] Wrong Track ID {}!", id));
            match track.reload() {
                Ok(true) => {
                    let sec = track.sec();
                    if sec > self.max_sec {
                        self.max_sec = sec;
                        self.id_max_sec = id;
                    }
                    reloaded_ids.push(id);
                    no_err_ids.push(id);
                }
                Ok(false) => {
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

        if need_update_max_sec {
            let (id, max_sec) = indexed_iter_filtered!(self.tracks)
                .map(|(id, track)| (id, track.sec()))
                .fold(
                    (0, 0.),
                    |(id_max, max), (id, sec)| {
                        if sec > max {
                            (id, sec)
                        } else {
                            (id_max, max)
                        }
                    },
                );
            self.id_max_sec = id;
            self.max_sec = max_sec;
        }
        self.update_filenames();
        removed_id_ch_tuples
    }

    #[inline]
    pub fn all_ids(&self) -> Vec<usize> {
        indexed_iter_filtered!(self.tracks)
            .map(|(id, _)| id)
            .collect()
    }

    #[inline]
    pub fn all_id_set(&self) -> HashSet<usize> {
        indexed_iter_filtered!(self.tracks)
            .map(|(id, _)| id)
            .collect()
    }

    #[inline]
    pub fn max_sr(&self) -> u32 {
        iter_filtered!(self.tracks)
            .map(|track| track.sr())
            .fold(0u32, |max, x| max.max(x))
    }

    #[inline]
    pub fn construct_sr_win_nfft_set(
        &self,
        ids: &[usize],
        setting: &SpecSetting,
    ) -> HashSet<SrWinNfft> {
        ids.iter()
            .map(|&id| setting.calc_sr_win_nfft(self[id].sr()))
            .collect()
    }

    #[inline]
    pub fn construct_all_sr_win_nfft_set(&self, setting: &SpecSetting) -> HashSet<SrWinNfft> {
        self.construct_sr_win_nfft_set(&self.all_ids(), &setting)
    }

    #[inline]
    pub fn has(&self, id: usize) -> bool {
        id < self.tracks.len() && self.tracks[id].is_some()
    }

    #[inline]
    pub fn get(&self, id: usize) -> Option<&AudioTrack> {
        self.tracks[id].as_ref()
    }

    #[inline]
    pub fn find_id_by_path(&self, path: &str) -> Option<usize> {
        indexed_iter_filtered!(self.tracks).find_map(|(id, track)| {
            if track.is_path_same(path) {
                Some(id)
            } else {
                None
            }
        })
    }

    #[inline]
    pub fn get_filename(&self, id: usize) -> &str {
        self.filenames[id]
            .as_ref()
            .expect(&format!("[get_filename] Wrong ID {}!", id))
    }

    fn update_filenames(&mut self) {
        let mut paths = HashMap::with_capacity(self.tracks.len());
        paths.extend(
            indexed_iter_filtered!(self.tracks).map(|(id, track)| (id, track.path.clone())),
        );
        let mut filenames = unique_filenames(paths);
        self.filenames = (0..self.tracks.len())
            .map(|i| filenames.remove(&i))
            .collect();
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
    fn calc_width_works() {
        let track = AudioTrack::new("samples/sample_48k.wav".into()).unwrap();
        assert_eq!(track.calc_width(1.), 44);
        assert_eq!(
            track.calc_part_grey_info(44, 1., 22, 2.),
            PartGreyInfo {
                i_w_and_width: (0, 12),
                start_sec_with_margin: 0.,
                width_with_margin: 24,
            }
        );
        assert_eq!(
            track.calc_part_wav_info(1., 43, 1.),
            (track.sr() as isize, (track.sr() * 43) as usize)
        );
    }

    #[test]
    fn calc_loudness_works() {
        let track = AudioTrack::new("samples/sample_48k.wav".into()).unwrap();
        assert_abs_diff_eq!(track.stats().global_lufs, -26.20331705029079);
    }
}
