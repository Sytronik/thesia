use std::collections::{HashMap, HashSet};
use std::fmt;
use std::io;
use std::iter;
use std::path::PathBuf;
use std::sync::Arc;

use approx::abs_diff_ne;
use ndarray::prelude::*;
use ndarray_stats::QuantileExt;
use rayon::prelude::*;
use realfft::{RealFftPlanner, RealToComplex};

mod audio;
mod decibel;
pub mod display;
mod mel;
mod resample;
mod sinc;
mod stft;
#[macro_use]
pub mod utils;
mod windows;

use decibel::DeciBelInplace;
use stft::{calc_up_ratio, perform_stft, FreqScale};
use utils::{calc_proper_n_fft, unique_filenames, Pad, PadMode};
use windows::{calc_normalized_win, WindowType};

use display::{ArrWithSliceInfo, PlotAxis};

pub type IdChVec = Vec<(usize, usize)>;
pub type IdChArr = [(usize, usize)];
pub type IdChMap<T> = HashMap<(usize, usize), T>;
pub type SrMap<T> = HashMap<u32, T>;
pub type FftModules = HashMap<usize, Arc<dyn RealToComplex<f32>>>;
pub type FramingParams = (u32, usize, usize);

#[derive(Debug)]
pub struct SpecSetting {
    pub win_ms: f32,
    pub t_overlap: usize,
    pub f_overlap: usize,
    pub freq_scale: FreqScale,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct DrawOption {
    pub px_per_sec: f64,
    pub height: u32,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct DrawOptionForWav {
    pub amp_range: (f32, f32),
}

pub enum ImageKind {
    Spec,
    Wav(DrawOptionForWav),
}

#[readonly::make]
pub struct AudioTrack {
    pub sr: u32,
    pub sample_format_str: String,
    path: PathBuf,
    wavs: Array2<f32>,
    win_length: usize,
    hop_length: usize,
    n_fft: usize,
}

impl AudioTrack {
    pub fn new(path: String, setting: &SpecSetting) -> io::Result<Self> {
        let (wavs, sr, sample_format_str) = audio::open_audio_file(path.as_str())?;
        let (win_length, hop_length, n_fft) = AudioTrack::calc_framing_params(sr, setting);
        Ok(AudioTrack {
            sr,
            sample_format_str,
            path: PathBuf::from(path).canonicalize()?,
            wavs,
            win_length,
            hop_length,
            n_fft,
        })
    }

    pub fn reload(&mut self, setting: &SpecSetting) -> io::Result<bool> {
        let (wavs, sr, sample_format_str) =
            audio::open_audio_file(self.path.to_string_lossy().as_ref())?;
        if sr == self.sr && sample_format_str == self.sample_format_str && wavs == self.wavs {
            return Ok(false);
        }
        self.sr = sr;
        self.sample_format_str = sample_format_str;
        self.wavs = wavs;
        self.set_framing_params(setting);
        Ok(true)
    }

    pub fn set_framing_params(&mut self, setting: &SpecSetting) {
        let (win_length, hop_length, n_fft) = AudioTrack::calc_framing_params(self.sr, setting);
        self.win_length = win_length;
        self.hop_length = hop_length;
        self.n_fft = n_fft;
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
    pub fn wavlen(&self) -> usize {
        self.wavs.shape()[1]
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

    fn calc_framing_params(sr: u32, setting: &SpecSetting) -> (usize, usize, usize) {
        let win_length = setting.win_ms * sr as f32 / 1000.;
        let hop_length = (win_length / setting.t_overlap as f32).round() as usize;
        let win_length = hop_length * setting.t_overlap;
        let n_fft = calc_proper_n_fft(win_length) * setting.f_overlap;
        (win_length, hop_length, n_fft)
    }
}

impl fmt::Debug for AudioTrack {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "AudioTrack {{\n\
                path: {},\n sr: {} Hz, n_ch: {}, length: {}, sec: {}\n\
                win_length: {}, hop_length: {}, n_fft: {}\n\
            }}",
            self.path.to_str().unwrap_or("err on path-to-str"),
            self.sr,
            self.n_ch(),
            self.wavs.shape()[1],
            self.sec(),
            self.win_length,
            self.hop_length,
            self.n_fft,
        )
    }
}

#[readonly::make]
pub struct TrackManager {
    pub tracks: Vec<Option<AudioTrack>>,
    pub filenames: Vec<Option<String>>,
    pub max_db: f32,
    pub min_db: f32,
    pub max_sec: f64,
    pub max_sr: u32,
    setting: SpecSetting,
    db_range: f32,
    windows: SrMap<Array1<f32>>,
    fft_modules: FftModules,
    mel_fbs: SrMap<Array2<f32>>,
    specs: IdChMap<Array2<f32>>,
    no_grey_ids: Vec<usize>,
    spec_greys: IdChMap<Array2<f32>>,
    id_max_sec: usize,
}

impl TrackManager {
    pub fn new() -> Self {
        TrackManager {
            tracks: Vec::new(),
            filenames: Vec::new(),
            max_db: -f32::INFINITY,
            min_db: f32::INFINITY,
            max_sec: 0.,
            max_sr: 0,
            setting: SpecSetting {
                win_ms: 40.,
                t_overlap: 4,
                f_overlap: 1,
                freq_scale: FreqScale::Mel,
            },
            db_range: 120.,
            windows: HashMap::new(),
            fft_modules: HashMap::new(),
            mel_fbs: HashMap::new(),
            specs: HashMap::new(),
            no_grey_ids: Vec::new(),
            spec_greys: HashMap::new(),
            id_max_sec: 0,
        }
    }

    pub fn add_tracks(&mut self, id_list: Vec<usize>, path_list: Vec<String>) -> Vec<usize> {
        let mut new_params_set = HashSet::<FramingParams>::new();
        let mut added_ids = Vec::new();
        for (id, path) in id_list.into_iter().zip(path_list) {
            if let Ok(track) = AudioTrack::new(path, &self.setting) {
                let sec = track.sec();
                if sec > self.max_sec {
                    self.max_sec = sec;
                    self.id_max_sec = id;
                }
                if self.windows.get(&track.sr).is_none() {
                    new_params_set.insert((track.sr, track.win_length, track.n_fft));
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
        self.update_srmaps(Some(new_params_set), None, None);
        self.update_specs(self.id_ch_tuples_from(&added_ids));
        self.no_grey_ids.extend(added_ids.iter().cloned());
        added_ids
    }

    pub fn reload_tracks(&mut self, id_list: &[usize]) -> Vec<usize> {
        let mut new_params_set = HashSet::<FramingParams>::new();
        let mut reloaded_ids = Vec::new();
        let mut no_err_ids = Vec::new();
        for &id in id_list {
            let track = self.tracks[id].as_mut().unwrap();
            match track.reload(&self.setting) {
                Ok(true) => {
                    let sec = track.sec();
                    if sec > self.max_sec {
                        self.max_sec = sec;
                        self.id_max_sec = id;
                    }
                    if self.windows.get(&track.sr).is_none() {
                        new_params_set.insert((track.sr, track.win_length, track.n_fft));
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

        self.update_srmaps(Some(new_params_set), None, None);
        self.update_specs(self.id_ch_tuples_from(&reloaded_ids));
        self.no_grey_ids.extend(reloaded_ids.iter().cloned());
        no_err_ids
    }

    pub fn remove_tracks(&mut self, id_list: &[usize]) {
        let mut removed_sr_set = HashSet::<u32>::new();
        let mut removed_nfft_set = HashSet::<usize>::new();
        let mut need_update_max_sec = false;
        for &id in id_list {
            if let Some(removed) = self.tracks[id].take() {
                for ch in 0..removed.n_ch() {
                    self.specs.remove(&(id, ch));
                    self.spec_greys.remove(&(id, ch));
                }
                if !removed_sr_set.contains(&removed.sr)
                    && iter_filtered!(self.tracks).all(|tr| tr.sr != removed.sr)
                {
                    removed_sr_set.insert(removed.sr);
                    removed_nfft_set.insert(removed.n_fft);
                }
                if id == self.id_max_sec {
                    need_update_max_sec = true;
                }
            } else {
                println!("Track_id {} does not exist! Ignore it ...", id);
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
        self.update_srmaps(None, Some(removed_sr_set), Some(removed_nfft_set));
        self.update_filenames();
    }

    pub fn apply_track_list_changes(&mut self) -> HashSet<usize> {
        self.update_greys(false)
    }

    pub fn get_entire_imgs(
        &self,
        id_ch_tuples: &IdChArr,
        option: DrawOption,
        kind: ImageKind,
    ) -> IdChMap<Array3<u8>> {
        // let start = Instant::now();
        let DrawOption { px_per_sec, height } = option;
        let mut result = IdChMap::with_capacity(id_ch_tuples.len());
        result.par_extend(id_ch_tuples.par_iter().map(|&(id, ch)| {
            let track = self.tracks[id].as_ref().unwrap();
            let width = track.calc_width(px_per_sec);
            let arr = match kind {
                ImageKind::Spec => {
                    let grey = self.spec_greys.get(&(id, ch)).unwrap().view();
                    let vec = display::colorize_grey_with_size(
                        ArrWithSliceInfo::entire(&grey),
                        width,
                        height,
                        false,
                    );
                    Array3::from_shape_vec((height as usize, width as usize, 4), vec).unwrap()
                }
                ImageKind::Wav(option_for_wav) => {
                    let mut arr = Array3::zeros((height as usize, width as usize, 4));
                    display::draw_wav_to(
                        arr.as_slice_mut().unwrap(),
                        ArrWithSliceInfo::entire(&track.get_wav(ch)),
                        width,
                        height,
                        option_for_wav.amp_range,
                        None,
                    );
                    arr
                }
            };
            ((id, ch), arr)
        }));
        // println!("draw entire: {:?}", start.elapsed());
        result
    }

    pub fn get_part_imgs(
        &self,
        id_ch_tuples: &IdChArr,
        start_sec: f64,
        width: u32,
        option: DrawOption,
        opt_for_wav: DrawOptionForWav,
        blend: f64,
        fast_resize_vec: Option<Vec<bool>>,
    ) -> IdChMap<Vec<u8>> {
        // let start = Instant::now();
        let DrawOption { px_per_sec, height } = option;
        let mut result = IdChMap::with_capacity(id_ch_tuples.len());
        let par_iter = id_ch_tuples.par_iter().enumerate().map(|(i, &(id, ch))| {
            let (pad_left, drawing_width, pad_right) =
                self.decompose_width_of(id, start_sec, width, px_per_sec);

            if drawing_width == 0 {
                return ((id, ch), vec![0u8; height as usize * width as usize * 4]);
            }
            let spec_grey = ArrWithSliceInfo::from_ref(
                &self.spec_greys.get(&(id, ch)).unwrap(),
                self.calc_part_grey_info(id, ch, start_sec, width, px_per_sec),
            );
            let wav = ArrWithSliceInfo::from(
                self.tracks[id].as_ref().unwrap().get_wav(ch),
                self.calc_part_wav_info(id, start_sec, width, px_per_sec),
            );
            let vec = display::draw_blended_spec_wav(
                spec_grey,
                wav,
                drawing_width,
                height,
                opt_for_wav.amp_range,
                blend,
                fast_resize_vec.as_ref().map_or(false, |v| v[i]),
            );
            let mut arr =
                Array3::from_shape_vec((height as usize, drawing_width as usize, 4), vec).unwrap();

            if width != drawing_width {
                arr = arr.pad(
                    (pad_left as usize, pad_right as usize),
                    Axis(1),
                    PadMode::Constant(0),
                );
            }
            ((id, ch), arr.into_raw_vec())
        });
        result.par_extend(par_iter);

        // println!("draw: {:?}", start.elapsed());
        result
    }

    pub fn get_overview_of(&self, id: usize, width: u32, height: u32) -> Vec<u8> {
        let track = self.tracks[id].as_ref().unwrap();
        let ch_h = height / track.n_ch() as u32;
        let i_start = (height % track.n_ch() as u32 / 2 * width * 4) as usize;
        let i_end = i_start + (track.n_ch() as u32 * ch_h * width * 4) as usize;
        let mut result = vec![0u8; width as usize * height as usize * 4];
        result[i_start..i_end]
            .par_chunks_exact_mut(ch_h as usize * width as usize * 4)
            .enumerate()
            .for_each(|(ch, x)| {
                display::draw_wav_to(
                    x,
                    ArrWithSliceInfo::entire(&track.get_wav(ch)),
                    width,
                    ch_h,
                    (-1., 1.),
                    None,
                )
            });
        result
    }

    #[allow(dead_code)]
    #[inline]
    pub fn id_ch_tuples(&self) -> IdChVec {
        self.specs.keys().cloned().collect()
    }

    #[inline]
    pub fn id_ch_tuples_from(&self, id_list: &[usize]) -> IdChVec {
        id_list
            .iter()
            .flat_map(|&id| {
                let n_ch = self.tracks[id].as_ref().unwrap().n_ch();
                iter::repeat(id).zip(0..n_ch)
            })
            .collect()
    }

    pub fn calc_hz_of(&self, y: u32, height: u32) -> f32 {
        let half_sr = self.max_sr as f32 / 2.;
        let relative_freq = 1. - y as f32 / height as f32;

        match self.setting.freq_scale {
            FreqScale::Linear => half_sr * relative_freq,
            FreqScale::Mel => mel::to_hz(mel::from_hz(half_sr) * relative_freq),
        }
    }

    pub fn get_freq_axis(&self, height: u32, max_num_ticks: u32, max_num_labels: u32) -> PlotAxis {
        display::create_freq_axis(
            self.setting.freq_scale,
            self.max_sr,
            max_num_ticks,
            max_num_labels,
        )
        .into_iter()
        .map(|(relative_y, s)| {
            let y = (relative_y * (height as f32)).round() as u32;
            (y.clamp(0, height), s)
        })
        .collect()
    }

    pub fn get_db_axis(&self, height: u32, max_num_ticks: u32, max_num_labels: u32) -> PlotAxis {
        display::create_db_axis(
            height,
            max_num_ticks,
            max_num_labels,
            (self.min_db, self.max_db),
        )
    }

    #[inline]
    pub fn exists(&self, &(id, ch): &(usize, usize)) -> bool {
        self.tracks[id]
            .as_ref()
            .map_or(false, |track| ch < track.n_ch())
    }

    #[inline]
    pub fn get_setting(&self) -> &SpecSetting {
        &self.setting
    }

    pub fn set_setting(&mut self, setting: SpecSetting) {
        let mut params_set = HashSet::new();
        let mut removed_nfft_set: HashSet<_> = self.fft_modules.keys().cloned().collect();
        for track in iter_mut_filtered!(self.tracks) {
            track.set_framing_params(&setting);
            params_set.insert((track.sr, track.win_length, track.n_fft));
            removed_nfft_set.remove(&track.n_fft);
        }

        self.setting = setting;
        self.update_srmaps(Some(params_set), None, Some(removed_nfft_set));
        self.update_specs(self.id_ch_tuples());
        self.update_greys(true);
    }

    #[inline]
    pub fn get_db_range(&self) -> f32 {
        self.db_range
    }

    pub fn set_db_range(&mut self, db_range: f32) {
        self.db_range = db_range;
        self.update_greys(true);
    }

    fn update_srmaps(
        &mut self,
        new_params_set: Option<HashSet<FramingParams>>,
        removed_sr_set: Option<HashSet<u32>>,
        removed_nfft_set: Option<HashSet<usize>>,
    ) {
        if let Some(removed_sr_set) = removed_sr_set {
            for sr in removed_sr_set {
                self.windows.remove(&sr);
                self.mel_fbs.remove(&sr);
            }
        }
        if let Some(removed_nfft_set) = removed_nfft_set {
            for n_fft in removed_nfft_set {
                self.fft_modules.remove(&n_fft);
            }
        }
        if let Some(new_params_set) = new_params_set {
            self.windows
                .par_extend(new_params_set.par_iter().map(|&(sr, win_length, n_fft)| {
                    (sr, calc_normalized_win(WindowType::Hann, win_length, n_fft))
                }));

            let mut real_fft_planner = RealFftPlanner::<f32>::new();
            for &(_, _, n_fft) in &new_params_set {
                self.fft_modules
                    .entry(n_fft)
                    .or_insert_with(|| real_fft_planner.plan_fft_forward(n_fft));
            }

            if let FreqScale::Mel = self.setting.freq_scale {
                self.mel_fbs.par_extend(
                    new_params_set
                        .par_iter()
                        .map(|&(sr, _, n_fft)| (sr, mel::calc_mel_fb_default(sr, n_fft))),
                );
            } else {
                self.mel_fbs.clear();
            }
        }
    }

    fn calc_spec_of(&self, id: usize, ch: usize, parallel: bool) -> Array2<f32> {
        let track = self.tracks[id].as_ref().unwrap();
        let window = self
            .windows
            .get(&track.sr)
            .map(|x| CowArray::from(x.view()));
        let fft_module = self.fft_modules.get(&track.n_fft).map(Arc::clone);
        let stft = perform_stft(
            track.get_wav(ch),
            track.win_length,
            track.hop_length,
            track.n_fft,
            window,
            fft_module,
            parallel,
        );
        let mut linspec = stft.mapv(|x| x.norm());
        match self.setting.freq_scale {
            FreqScale::Linear => {
                linspec.amp_to_db_default();
                linspec
            }
            FreqScale::Mel => {
                let mut melspec = linspec.dot(self.mel_fbs.get(&track.sr).unwrap());
                melspec.amp_to_db_default();
                melspec
            }
        }
    }

    fn update_specs(&mut self, id_ch_tuples: IdChVec) {
        let len = id_ch_tuples.len();
        let mut specs = IdChMap::with_capacity(len);
        specs.par_extend(
            id_ch_tuples
                .into_par_iter()
                .map(|(id, ch)| ((id, ch), self.calc_spec_of(id, ch, len == 1))),
        );
        self.specs.extend(specs);
    }

    /// update spec_greys, max_db, min_db, max_sr
    /// clear no_grey_ids
    fn update_greys(&mut self, force_update_all: bool) -> HashSet<usize> {
        let (mut max, mut min) = self
            .specs
            .par_iter()
            .map(|(_, spec)| {
                let max = *spec.max().unwrap_or(&-f32::INFINITY);
                let min = *spec.min().unwrap_or(&f32::INFINITY);
                (max, min)
            })
            .reduce(
                || (-f32::INFINITY, f32::INFINITY),
                |(max, min), (current_max, current_min)| {
                    (max.max(current_max), min.min(current_min))
                },
            );
        max = max.min(0.);
        min = min.max(max - self.db_range);
        let mut has_changed_all = if force_update_all {
            true
        } else {
            let mut has_changed_all = false;
            if abs_diff_ne!(self.max_db, max, epsilon = 1e-3) {
                self.max_db = max;
                has_changed_all = true;
            }
            if abs_diff_ne!(self.min_db, min, epsilon = 1e-3) {
                self.min_db = min;
                has_changed_all = true;
            }
            has_changed_all
        };

        let max_sr = iter_filtered!(self.tracks)
            .map(|track| track.sr)
            .fold(0u32, |max, x| max.max(x));
        if self.max_sr != max_sr {
            self.max_sr = max_sr;
            has_changed_all = true;
        }
        let ids_need_update: HashSet<usize> = if has_changed_all {
            self.no_grey_ids.clear();
            indexed_iter_filtered!(self.tracks)
                .map(|(id, _)| id)
                .collect()
        } else {
            self.no_grey_ids.drain(..).collect()
        };

        if !ids_need_update.is_empty() {
            let mut new_spec_greys = IdChMap::with_capacity(self.specs.len());
            new_spec_greys.par_extend(self.specs.par_iter().filter_map(|(&(id, ch), spec)| {
                if ids_need_update.contains(&id) {
                    let up_ratio = calc_up_ratio(
                        self.tracks[id].as_ref().unwrap().sr,
                        self.max_sr,
                        self.setting.freq_scale,
                    );
                    let grey = display::convert_spec_to_grey(
                        spec.view(),
                        up_ratio,
                        self.max_db,
                        self.min_db,
                    );
                    Some(((id, ch), grey))
                } else {
                    None
                }
            }));

            if has_changed_all {
                self.spec_greys = new_spec_greys;
            } else {
                self.spec_greys.extend(new_spec_greys)
            }
        }
        ids_need_update
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

    fn calc_part_grey_info(
        &self,
        id: usize,
        ch: usize,
        start_sec: f64,
        target_width: u32,
        px_per_sec: f64,
    ) -> (isize, usize) {
        let track = self.tracks[id].as_ref().unwrap();
        let spec_grey = self.spec_greys.get(&(id, ch)).unwrap();
        let total_width = spec_grey.shape()[1] as u64;
        let wavlen = track.wavlen() as f64;
        let sr = track.sr as u64;
        let i_w = ((total_width * sr) as f64 * start_sec / wavlen).round() as isize;
        let width =
            (((total_width * target_width as u64 * sr) as f64 / wavlen / px_per_sec).round()
                as usize)
                .max(1);
        (i_w, width)
    }

    fn calc_part_wav_info(
        &self,
        id: usize,
        start_sec: f64,
        width: u32,
        px_per_sec: f64,
    ) -> (isize, usize) {
        let track = self.tracks[id].as_ref().unwrap();
        let i = (start_sec * track.sr as f64).round() as isize;
        let length = ((track.sr as u64 * width as u64) as f64 / px_per_sec).round() as usize;
        (i, length)
    }

    fn decompose_width_of(
        &self,
        id: usize,
        start_sec: f64,
        width: u32,
        px_per_sec: f64,
    ) -> (u32, u32, u32) {
        let track = self.tracks[id].as_ref().unwrap();

        let total_width = (px_per_sec * track.wavlen() as f64 / track.sr as f64).max(1.);
        let pad_left = ((-start_sec * px_per_sec).max(0.).round() as u32).min(width);
        let pad_right = ((start_sec * px_per_sec + width as f64 - total_width)
            .max(0.)
            .round() as u32)
            .min(width - pad_left);

        (pad_left, width - pad_left - pad_right, pad_right)
    }
}

#[cfg(test)]
mod tests {
    use image::RgbaImage;

    use super::*;

    #[test]
    fn trackmanager_works() {
        let tags = ["8k", "16k", "22k05", "24k", "44k1", "48k", "stereo_48k"];
        let id_list: Vec<usize> = (0..tags.len()).collect();
        let mut path_list: Vec<String> = tags
            .iter()
            .take(6)
            .map(|x| format!("samples/sample_{}.wav", x))
            .collect();
        path_list.push(String::from("samples/stereo/sample_48k.wav"));
        let mut tm = TrackManager::new();
        let added_ids = tm.add_tracks(id_list[0..3].to_owned(), path_list[0..3].to_owned());
        assert_eq!(&added_ids, &id_list[0..3]);
        let added_ids = tm.add_tracks(id_list[3..].to_owned(), path_list[3..].to_owned());
        assert_eq!(&added_ids, &id_list[3..]);
        assert_eq!(tm.tracks.len(), id_list.len());

        assert_eq!(tm.spec_greys.len(), 0);
        let mut updated_ids: Vec<usize> = tm.apply_track_list_changes().into_iter().collect();
        updated_ids.sort();
        assert_eq!(updated_ids, id_list);

        dbg!(tm.tracks[0].as_ref().unwrap());
        dbg!(tm.filenames[5].as_ref().unwrap());
        dbg!(tm.filenames[6].as_ref().unwrap());
        let option = DrawOption {
            px_per_sec: 200.,
            height: 500,
        };
        let opt_for_wav = DrawOptionForWav {
            amp_range: (-1., 1.),
        };
        let spec_imgs = tm.get_entire_imgs(&tm.id_ch_tuples(), option, ImageKind::Spec);
        let mut wav_imgs =
            tm.get_entire_imgs(&tm.id_ch_tuples(), option, ImageKind::Wav(opt_for_wav));
        for ((id, ch), spec) in spec_imgs {
            let sr_str = tags[id];
            let width = spec.shape()[1] as u32;
            let img = RgbaImage::from_vec(width, option.height, spec.into_raw_vec()).unwrap();
            img.save(format!("samples/spec_{}_{}.png", sr_str, ch))
                .unwrap();
            let wav_img = wav_imgs.remove(&(id, ch)).unwrap().into_raw_vec();
            let img = RgbaImage::from_vec(width, option.height, wav_img).unwrap();
            img.save(format!("samples/wav_{}_{}.png", sr_str, ch))
                .unwrap();
        }

        let imvec = tm
            .get_part_imgs(
                &[(0, 0)],
                20.,
                1000,
                DrawOption {
                    px_per_sec: 16000.,
                    height: option.height,
                },
                opt_for_wav,
                0.,
                Some(vec![false]),
            )
            .remove(&(0, 0))
            .unwrap();
        let im = RgbaImage::from_vec(imvec.len() as u32 / option.height / 4, option.height, imvec)
            .unwrap();
        im.save("samples/wav_part.png").unwrap();

        tm.remove_tracks(&[0]);
        let updated_ids = tm.apply_track_list_changes();
        assert!(updated_ids.is_empty());
    }
}
