use std::collections::{HashMap, HashSet};
use std::fmt;
use std::ops::Index;
use std::path::PathBuf;

use creak::DecoderError;
use ndarray::prelude::*;

use super::audio;
use super::spectrogram::SrWinNfft;
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
    pub sr: u32,
    pub sample_format_str: String,
    path: PathBuf,
    wavs: Array2<f32>,
}

impl AudioTrack {
    pub fn new(path: String) -> Result<Self, DecoderError> {
        let (wavs, sr, sample_format_str) = audio::open_audio_file(path.as_str())?;
        Ok(AudioTrack {
            sr,
            sample_format_str,
            path: PathBuf::from(path).canonicalize().unwrap(),
            wavs,
        })
    }

    pub fn reload(&mut self) -> Result<bool, DecoderError> {
        let (wavs, sr, sample_format_str) =
            audio::open_audio_file(self.path.to_string_lossy().as_ref())?;
        if sr == self.sr && sample_format_str == self.sample_format_str && wavs == self.wavs {
            return Ok(false);
        }
        self.sr = sr;
        self.sample_format_str = sample_format_str;
        self.wavs = wavs;
        Ok(true)
    }

    #[inline]
    pub fn get_wav(&self, ch: usize) -> ArrayView1<f32> {
        self.wavs.index_axis(Axis(0), ch)
    }

    #[inline]
    pub fn path_string(&self) -> String {
        self.path.as_os_str().to_string_lossy().into_owned()
    }

    #[inline]
    pub fn n_ch(&self) -> usize {
        self.wavs.shape()[0]
    }

    #[inline]
    pub fn sec(&self) -> f64 {
        self.wavs.shape()[1] as f64 / self.sr as f64
    }

    #[inline]
    pub fn calc_width(&self, px_per_sec: f64) -> u32 {
        ((px_per_sec * self.wavs.shape()[1] as f64 / self.sr as f64).round() as u32).max(1)
    }

    #[inline]
    pub fn is_path_same(&self, path: &str) -> bool {
        PathBuf::from(path)
            .canonicalize()
            .map_or(false, |x| x == self.path)
    }

    pub fn calc_part_grey_info(
        &self,
        grey_width: u64,
        start_sec: f64,
        target_width: u32,
        px_per_sec: f64,
    ) -> (isize, usize) {
        let wavlen = self.wavs.shape()[1] as f64;
        let sr = self.sr as u64;
        let i_w = ((grey_width * sr) as f64 * start_sec / wavlen).round() as isize;
        let width = (((grey_width * target_width as u64 * sr) as f64 / wavlen / px_per_sec).round()
            as usize)
            .max(1);
        (i_w, width)
    }

    pub fn calc_part_wav_info(
        &self,
        start_sec: f64,
        width: u32,
        px_per_sec: f64,
    ) -> (isize, usize) {
        let i = (start_sec * self.sr as f64).round() as isize;
        let length = ((self.sr as u64 * width as u64) as f64 / px_per_sec).round() as usize;
        (i, length)
    }

    pub fn decompose_width_of(
        &self,
        start_sec: f64,
        width: u32,
        px_per_sec: f64,
    ) -> (u32, u32, u32) {
        let total_width = (px_per_sec * self.wavs.shape()[1] as f64 / self.sr as f64).max(1.);
        let pad_left = ((-start_sec * px_per_sec).max(0.).round() as u32).min(width);
        let pad_right = ((start_sec * px_per_sec + width as f64 - total_width)
            .max(0.)
            .round() as u32)
            .min(width - pad_left);

        (pad_left, width - pad_left - pad_right, pad_right)
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
            self.sr,
            self.n_ch(),
            self.wavs.shape()[1],
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
                .expect(format!("[reload_tracks] Wrong Track ID {}!", id).as_str());
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
            .map(|track| track.sr)
            .fold(0u32, |max, x| max.max(x))
    }

    #[inline]
    pub fn construct_sr_win_nfft_set(
        &self,
        ids: &[usize],
        setting: &SpecSetting,
    ) -> HashSet<SrWinNfft> {
        ids.iter()
            .map(|&id| setting.calc_sr_win_nfft(self[id].sr))
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
            .expect(format!("[get_filename] Wrong ID {}!", id).as_str())
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
