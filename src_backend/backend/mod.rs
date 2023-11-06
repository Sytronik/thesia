use std::collections::{HashMap, HashSet};
use std::iter;

use approx::abs_diff_ne;
use napi_derive::napi;
use ndarray::prelude::*;
use ndarray_stats::QuantileExt;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

mod audio;
mod decibel;
pub mod display;
pub mod normalize;
pub mod plot_axis;
mod resample;
mod sinc;
mod spectrogram;
mod stats;
mod track;
#[macro_use]
pub mod utils;
mod windows;

use decibel::DeciBelInplace;
use spectrogram::{calc_up_ratio, mel, perform_stft, AnalysisParamManager, FreqScale, SrWinNfft};
use track::TrackList;

pub use display::{DrawOption, DrawOptionForWav, TrackDrawer};
pub use plot_axis::{PlotAxis, PlotAxisCreator};
pub use utils::{Pad, PadMode};

use self::normalize::{GuardClippingMode, NormalizeTarget};
use self::track::AudioTrack;

pub type IdChVec = Vec<(usize, usize)>;
pub type IdChArr = [(usize, usize)];
pub type IdChMap<T> = HashMap<(usize, usize), T>;
type FramingParams = (usize, usize, usize);

#[napi(object)]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SpecSetting {
    #[napi(js_name = "winMillisec")]
    pub win_ms: f64,
    pub t_overlap: u32,
    pub f_overlap: u32,
    pub freq_scale: FreqScale,
}

impl SpecSetting {
    #[inline]
    pub fn calc_win_length(&self, sr: u32) -> usize {
        self.calc_hop_length(sr) * self.t_overlap as usize
    }

    #[inline]
    pub fn calc_hop_length(&self, sr: u32) -> usize {
        (self.calc_win_length_float(sr) / self.t_overlap as f64).round() as usize
    }

    #[inline]
    pub fn calc_n_fft(&self, sr: u32) -> usize {
        self.calc_win_length(sr).next_power_of_two() * self.f_overlap as usize
    }

    #[inline]
    pub fn calc_framing_params(&self, sr: u32) -> FramingParams {
        let hop_length = self.calc_hop_length(sr);
        let win_length = self.calc_win_length_from_hop_length(hop_length);
        let n_fft = self.calc_n_fft_from_win_length(win_length);
        (hop_length, win_length, n_fft)
    }

    #[inline]
    pub fn calc_sr_win_nfft(&self, sr: u32) -> SrWinNfft {
        let win_length = self.calc_win_length(sr);
        let n_fft = self.calc_n_fft_from_win_length(win_length);
        SrWinNfft {
            sr,
            win_length,
            n_fft,
        }
    }

    #[inline]
    fn calc_win_length_from_hop_length(&self, hop_length: usize) -> usize {
        hop_length * self.t_overlap as usize
    }

    #[inline]
    fn calc_win_length_float(&self, sr: u32) -> f64 {
        self.win_ms * sr as f64 / 1000.
    }

    #[inline]
    fn calc_n_fft_from_win_length(&self, win_length: usize) -> usize {
        win_length.next_power_of_two() * self.f_overlap as usize
    }
}

#[readonly::make]
pub struct TrackManager {
    pub tracklist: TrackList,
    pub max_db: f32,
    pub min_db: f32,
    pub max_sr: u32,
    pub spec_greys: IdChMap<Array2<f32>>,
    pub setting: SpecSetting,
    pub db_range: f32,
    analysis_mgr: AnalysisParamManager,
    specs: IdChMap<Array2<f32>>,
    no_grey_ids: Vec<usize>,
}

impl TrackManager {
    pub fn new() -> Self {
        TrackManager {
            max_db: -f32::INFINITY,
            min_db: f32::INFINITY,
            max_sr: 0,
            spec_greys: HashMap::new(),
            tracklist: TrackList::new(),
            setting: SpecSetting {
                win_ms: 40.,
                t_overlap: 4,
                f_overlap: 1,
                freq_scale: FreqScale::Mel,
            },
            db_range: 120.,
            analysis_mgr: AnalysisParamManager::new(),
            specs: HashMap::new(),
            no_grey_ids: Vec::new(),
        }
    }

    pub fn add_tracks(&mut self, id_list: Vec<usize>, path_list: Vec<String>) -> Vec<usize> {
        let added_ids = self.tracklist.add_tracks(id_list, path_list);
        let sr_win_nfft_set = self
            .tracklist
            .construct_sr_win_nfft_set(&added_ids, &self.setting);

        self.update_specs(self.id_ch_tuples_from(&added_ids), Some(&sr_win_nfft_set));
        self.no_grey_ids.extend(added_ids.iter().cloned());
        added_ids
    }

    pub fn reload_tracks(&mut self, id_list: &[usize]) -> Vec<usize> {
        let (reloaded_ids, no_err_ids) = self.tracklist.reload_tracks(id_list);
        let sr_win_nfft_set = self
            .tracklist
            .construct_sr_win_nfft_set(&reloaded_ids, &self.setting);

        self.update_specs(
            self.id_ch_tuples_from(&reloaded_ids),
            Some(&sr_win_nfft_set),
        );
        self.no_grey_ids.extend(reloaded_ids.iter().cloned());
        no_err_ids
    }

    pub fn remove_tracks(&mut self, id_list: &[usize]) {
        let removed_id_ch_tuples = self.tracklist.remove_tracks(id_list);
        for tup in removed_id_ch_tuples {
            self.specs.remove(&tup);
            self.spec_greys.remove(&tup);
        }

        self.analysis_mgr.retain(
            &self.tracklist.construct_all_sr_win_nfft_set(&self.setting),
            self.setting.freq_scale,
        );
    }

    pub fn apply_track_list_changes(&mut self) -> HashSet<usize> {
        self.update_greys(false)
    }

    #[inline]
    pub fn id_ch_tuples(&self) -> IdChVec {
        self.id_ch_tuples_from(&self.tracklist.all_ids())
    }

    #[inline]
    pub fn id_ch_tuples_from(&self, id_list: &[usize]) -> IdChVec {
        id_list
            .iter()
            .filter(|&&id| self.has_id(id))
            .flat_map(|&id| {
                let n_ch = self.tracklist[id].n_ch();
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

    #[inline]
    pub fn exists(&self, &(id, ch): &(usize, usize)) -> bool {
        self.specs.contains_key(&(id, ch))
    }

    #[inline]
    pub fn has_id(&self, id: usize) -> bool {
        self.tracklist.has(id)
    }

    #[inline]
    pub fn track(&self, id: usize) -> Option<&AudioTrack> {
        self.tracklist.get(id)
    }

    pub fn set_setting(&mut self, setting: SpecSetting) {
        let sr_win_nfft_set = self
            .tracklist
            .construct_sr_win_nfft_set(&self.tracklist.all_ids(), &setting);

        self.setting = setting;
        self.analysis_mgr
            .retain(&sr_win_nfft_set, self.setting.freq_scale);
        self.update_specs(self.id_ch_tuples(), Some(&sr_win_nfft_set));
        self.update_greys(true);
    }

    pub fn common_guard_clipping(&self) -> GuardClippingMode {
        self.tracklist.common_guard_clipping
    }

    pub fn set_common_guard_clipping(&mut self, mode: GuardClippingMode) {
        self.tracklist.set_common_guard_clipping(mode);

        self.update_specs(self.id_ch_tuples(), None);
        self.update_greys(true);
    }

    pub fn common_normalize(&self) -> NormalizeTarget {
        self.tracklist.common_normalize
    }

    pub fn set_common_normalize(&mut self, target: NormalizeTarget) {
        self.tracklist.set_common_normalize(target);

        self.update_specs(self.id_ch_tuples(), None);
        self.update_greys(true);
    }

    pub fn set_db_range(&mut self, db_range: f32) {
        self.db_range = db_range;
        self.update_greys(true);
    }

    fn calc_spec_of(&self, id: usize, ch: usize, parallel: bool) -> Array2<f32> {
        let track = &self.tracklist[id];
        let (hop_length, win_length, n_fft) = self.setting.calc_framing_params(track.sr());
        let window = self.analysis_mgr.window(win_length, n_fft);
        let fft_module = self.analysis_mgr.fft_module(n_fft);
        let stft = perform_stft(
            track.channel(ch),
            win_length,
            hop_length,
            n_fft,
            Some(window),
            Some(fft_module),
            parallel,
        );
        let mut linspec = stft.mapv(|x| x.norm());
        match self.setting.freq_scale {
            FreqScale::Linear => {
                linspec.dB_from_amp_inplace_default();
                linspec
            }
            FreqScale::Mel => {
                let mut melspec = linspec.dot(&self.analysis_mgr.mel_fb(track.sr(), n_fft));
                melspec.dB_from_amp_inplace_default();
                melspec
            }
        }
    }

    fn update_specs(&mut self, id_ch_tuples: IdChVec, framing_params: Option<&HashSet<SrWinNfft>>) {
        match framing_params {
            Some(p) => {
                self.analysis_mgr.prepare(p, self.setting.freq_scale);
            }
            None => {
                let p = self.tracklist.construct_all_sr_win_nfft_set(&self.setting);
                self.analysis_mgr.prepare(&p, self.setting.freq_scale);
            }
        }
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

        let max_sr = self.tracklist.max_sr();
        if self.max_sr != max_sr {
            self.max_sr = max_sr;
            has_changed_all = true;
        }
        let ids_need_update: HashSet<usize> = if has_changed_all {
            self.no_grey_ids.clear();
            self.tracklist.all_id_set()
        } else {
            self.no_grey_ids.drain(..).collect()
        };

        if !ids_need_update.is_empty() {
            let mut new_spec_greys = IdChMap::with_capacity(self.specs.len());
            new_spec_greys.par_extend(self.specs.par_iter().filter_map(|(&(id, ch), spec)| {
                if ids_need_update.contains(&id) {
                    let up_ratio = calc_up_ratio(
                        self.tracklist[id].sr(),
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
}

#[cfg(test)]
mod tests {
    use image::RgbaImage;

    use super::display::{DrawOption, DrawOptionForWav, ImageKind};
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
        path_list.push("samples/stereo/sample_48k.wav".into());
        let mut tm = TrackManager::new();
        let added_ids = tm.add_tracks(id_list[0..3].to_owned(), path_list[0..3].to_owned());
        assert_eq!(&added_ids, &id_list[0..3]);
        let added_ids = tm.add_tracks(id_list[3..].to_owned(), path_list[3..].to_owned());
        assert_eq!(&added_ids, &id_list[3..]);
        assert_eq!(tm.tracklist.all_ids().len(), id_list.len());

        assert_eq!(tm.spec_greys.len(), 0);
        let mut updated_ids: Vec<usize> = tm.apply_track_list_changes().into_iter().collect();
        updated_ids.sort();
        assert_eq!(updated_ids, id_list);

        dbg!(&tm.tracklist[0]);
        dbg!(tm.tracklist.filename(5));
        dbg!(tm.tracklist.filename(6));
        let option = DrawOption {
            px_per_sec: 200.,
            height: 500,
        };
        let opt_for_wav = DrawOptionForWav {
            amp_range: (-1., 1.),
            dpr: 1.,
        };
        let spec_imgs = tm.draw_entire_imgs(&tm.id_ch_tuples(), option, ImageKind::Spec);
        let mut wav_imgs =
            tm.draw_entire_imgs(&tm.id_ch_tuples(), option, ImageKind::Wav(opt_for_wav));
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
            .draw_part_imgs(
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
