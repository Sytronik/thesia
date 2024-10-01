use std::collections::{HashMap, HashSet};

use dashmap::DashMap;
use fast_image_resize::pixels::U16;
use ndarray::prelude::*;
use ndarray_stats::QuantileExt;
use rayon::prelude::*;

mod audio;
mod dynamics;
mod sinc;
mod spectrogram;
mod track;
mod utils;
pub mod visualize;
mod windows;

pub use audio::AudioFormatInfo;
pub use dynamics::{DeciBel, GuardClippingMode, NormalizeTarget};
pub use spectrogram::SpecSetting;
pub use track::TrackList;
pub use utils::Pad;
pub use visualize::{
    calc_amp_axis_markers, calc_dB_axis_markers, calc_freq_axis_markers, calc_time_axis_markers,
    convert_freq_label_to_hz, convert_hz_to_label, convert_sec_to_label, convert_time_label_to_sec,
    DrawOption, DrawOptionForWav, TrackDrawer,
};

pub type IdChVec = Vec<(usize, usize)>;
pub type IdChArr = [(usize, usize)];
pub type IdChValueVec<T> = Vec<((usize, usize), T)>;
pub type IdChValueArr<T> = [((usize, usize), T)];
pub type IdChMap<T> = HashMap<(usize, usize), T>;
pub type IdChDMap<T> = DashMap<(usize, usize), T>;

use spectrogram::{SpectrogramAnalyzer, SrWinNfft};

#[readonly::make]
#[allow(non_snake_case)]
pub struct TrackManager {
    pub max_dB: f32,
    pub min_dB: f32,
    pub max_sr: u32,
    pub spec_greys: IdChMap<Array2<U16>>,
    pub setting: SpecSetting,
    pub dB_range: f32,
    hz_range: (f32, f32),
    spec_analyzer: SpectrogramAnalyzer,
    specs: IdChMap<Array2<f32>>,
    no_grey_ids: Vec<usize>,
}

impl TrackManager {
    pub fn new() -> Self {
        TrackManager {
            max_dB: -f32::INFINITY,
            min_dB: f32::INFINITY,
            max_sr: 0,
            spec_greys: HashMap::new(),
            setting: Default::default(),
            dB_range: 100.,
            hz_range: (0., f32::INFINITY),
            spec_analyzer: SpectrogramAnalyzer::new(),
            specs: HashMap::new(),
            no_grey_ids: Vec::new(),
        }
    }

    pub fn add_tracks(
        &mut self,
        tracklist: &mut TrackList,
        id_list: Vec<usize>,
        path_list: Vec<String>,
    ) -> Vec<usize> {
        let added_ids = tracklist.add_tracks(id_list, path_list);
        let sr_win_nfft_set = tracklist.construct_sr_win_nfft_set(&added_ids, &self.setting);

        self.update_specs(
            tracklist,
            tracklist.id_ch_tuples_from(&added_ids),
            Some(&sr_win_nfft_set),
        );
        self.no_grey_ids.extend(added_ids.iter().cloned());
        added_ids
    }

    pub fn reload_tracks(&mut self, tracklist: &mut TrackList, id_list: &[usize]) -> Vec<usize> {
        let (reloaded_ids, no_err_ids) = tracklist.reload_tracks(id_list);
        let sr_win_nfft_set = tracklist.construct_sr_win_nfft_set(&reloaded_ids, &self.setting);

        self.update_specs(
            tracklist,
            tracklist.id_ch_tuples_from(&reloaded_ids),
            Some(&sr_win_nfft_set),
        );
        self.no_grey_ids.extend(reloaded_ids.iter().cloned());
        no_err_ids
    }

    pub fn remove_tracks(
        &mut self,
        tracklist: &mut TrackList,
        id_list: &[usize],
    ) -> Option<(f32, f32)> {
        let removed_id_ch_tuples = tracklist.remove_tracks(id_list);
        for tup in removed_id_ch_tuples {
            self.specs.remove(&tup);
            self.spec_greys.remove(&tup);
        }

        self.spec_analyzer.retain(
            &tracklist.construct_all_sr_win_nfft_set(&self.setting),
            self.setting.freq_scale,
        );
        if tracklist.is_empty() {
            let hz_range = (0., f32::INFINITY);
            self.set_hz_range(tracklist, hz_range);
            Some(hz_range)
        } else {
            None
        }
    }

    pub fn apply_track_list_changes(&mut self, tracklist: &TrackList) -> (HashSet<usize>, u32) {
        let set = self.update_greys(tracklist, false);
        (set, self.max_sr)
    }

    #[inline]
    pub fn exists(&self, &(id, ch): &(usize, usize)) -> bool {
        self.specs.contains_key(&(id, ch))
    }

    pub fn set_setting(&mut self, tracklist: &mut TrackList, setting: SpecSetting) {
        let sr_win_nfft_set = tracklist.construct_sr_win_nfft_set(&tracklist.all_ids(), &setting);

        self.setting = setting;
        self.spec_analyzer
            .retain(&sr_win_nfft_set, self.setting.freq_scale);
        self.update_specs(tracklist, tracklist.id_ch_tuples(), Some(&sr_win_nfft_set));
        self.update_greys(tracklist, true);
    }

    pub fn set_common_guard_clipping(
        &mut self,
        tracklist: &mut TrackList,
        mode: GuardClippingMode,
    ) {
        tracklist.set_common_guard_clipping(mode);

        self.update_specs(tracklist, tracklist.id_ch_tuples(), None);
        self.update_greys(tracklist, true);
    }

    pub fn set_common_normalize(&mut self, tracklist: &mut TrackList, target: NormalizeTarget) {
        tracklist.set_common_normalize(target);

        self.update_specs(tracklist, tracklist.id_ch_tuples(), None);
        self.update_greys(tracklist, true);
    }

    #[allow(non_snake_case)]
    pub fn set_dB_range(&mut self, tracklist: &TrackList, dB_range: f32) {
        self.dB_range = dB_range;
        self.update_greys(tracklist, true);
    }

    #[inline]
    fn get_hz_range(&self) -> (f32, f32) {
        Self::calc_valid_hz_range(&self.hz_range, self.max_sr as f32 / 2.)
    }

    pub fn calc_valid_hz_range(hz_range: &(f32, f32), max_track_hz: f32) -> (f32, f32) {
        let max_hz = if hz_range.1.is_finite() {
            hz_range.1
        } else {
            max_track_hz
        };
        (hz_range.0, max_hz)
    }

    /// set self.hz_range.
    /// If it's different from the existing value, update greys and return true.
    /// else, return false.
    pub fn set_hz_range(&mut self, tracklist: &TrackList, hz_range: (f32, f32)) -> bool {
        let prev_hz_range = self.get_hz_range();
        self.hz_range = hz_range;
        let curr_hz_range = self.get_hz_range();
        if (prev_hz_range.0 - curr_hz_range.0).abs() < 1e-2
            && (prev_hz_range.1 - curr_hz_range.1).abs() < 1e-2
        {
            return false;
        }
        self.update_greys(tracklist, true);
        true
    }

    fn update_specs(
        &mut self,
        tracklist: &TrackList,
        id_ch_tuples: IdChVec,
        framing_params: Option<&HashSet<SrWinNfft>>,
    ) {
        match framing_params {
            Some(p) => {
                self.spec_analyzer.prepare(p, self.setting.freq_scale);
            }
            None => {
                let p = tracklist.construct_all_sr_win_nfft_set(&self.setting);
                self.spec_analyzer.prepare(&p, self.setting.freq_scale);
            }
        }
        let len = id_ch_tuples.len();
        let mut specs = IdChMap::with_capacity(len);
        specs.par_extend(id_ch_tuples.into_par_iter().map(|(id, ch)| {
            let track = &tracklist[id];
            let spec = self.spec_analyzer.calc_spec(
                track.channel(ch),
                track.sr(),
                &self.setting,
                len == 1,
            );
            ((id, ch), spec)
        }));
        self.specs.extend(specs);
    }

    /// update spec_greys, max_dB, min_dB, max_sr
    /// clear no_grey_ids
    fn update_greys(&mut self, tracklist: &TrackList, force_update_all: bool) -> HashSet<usize> {
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
        min = min.max(max - self.dB_range);
        let mut need_update_all = force_update_all;
        if self.max_dB != max {
            self.max_dB = max;
            need_update_all = true;
        }
        if self.min_dB != min {
            self.min_dB = min;
            need_update_all = true;
        }

        let max_sr = tracklist.max_sr();
        if self.max_sr != max_sr {
            self.max_sr = max_sr;
            need_update_all = true;
        }
        let ids_need_update: HashSet<usize> = if need_update_all {
            self.no_grey_ids.clear();
            tracklist.all_id_set()
        } else {
            self.no_grey_ids.drain(..).collect()
        };

        if !ids_need_update.is_empty() {
            let mut new_spec_greys = IdChMap::with_capacity(self.specs.len());
            new_spec_greys.par_extend(self.specs.par_iter().filter_map(|(&(id, ch), spec)| {
                if ids_need_update.contains(&id) {
                    let sr = tracklist[id].sr();
                    let i_freq_range = self.setting.freq_scale.hz_range_to_idx(
                        self.get_hz_range(),
                        sr,
                        spec.shape()[1],
                    );
                    let grey = visualize::convert_spec_to_grey(
                        spec.view(),
                        i_freq_range,
                        (self.min_dB, self.max_dB),
                    );
                    Some(((id, ch), grey))
                } else {
                    None
                }
            }));

            if need_update_all {
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

    use super::visualize::ImageKind;
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
        let mut tracklist = TrackList::new();
        let mut tm = TrackManager::new();
        let added_ids = tm.add_tracks(
            &mut tracklist,
            id_list[0..3].to_owned(),
            path_list[0..3].to_owned(),
        );
        assert_eq!(&added_ids, &id_list[0..3]);
        let added_ids = tm.add_tracks(
            &mut tracklist,
            id_list[3..].to_owned(),
            path_list[3..].to_owned(),
        );
        assert_eq!(&added_ids, &id_list[3..]);
        assert_eq!(tracklist.all_ids().len(), id_list.len());

        assert_eq!(tm.spec_greys.len(), 0);
        let mut updated_ids: Vec<usize> = tm
            .apply_track_list_changes(&tracklist)
            .0
            .into_iter()
            .collect();
        updated_ids.sort();
        assert_eq!(updated_ids, id_list);

        dbg!(&tracklist[0]);
        dbg!(tracklist.filename(5));
        dbg!(tracklist.filename(6));
        let option = DrawOption {
            px_per_sec: 200.,
            height: 500,
        };
        let opt_for_wav = Default::default();
        let spec_imgs = tm.draw_entire_imgs(
            &tracklist,
            &tracklist.id_ch_tuples(),
            option,
            ImageKind::Spec,
        );
        let wav_imgs = tm.draw_entire_imgs(
            &tracklist,
            &tracklist.id_ch_tuples(),
            option,
            ImageKind::Wav(opt_for_wav),
        );
        for ((id, ch), spec) in spec_imgs {
            let sr_str = tags[id];
            let width = spec.shape()[1] as u32;
            let img = RgbaImage::from_vec(width, option.height, spec.into_raw_vec_and_offset().0)
                .unwrap();
            img.save(format!("samples/spec_{}_{}.png", sr_str, ch))
                .unwrap();
            let (wav_img, _) = wav_imgs
                .iter()
                .find_map(|(id_ch, v)| (*id_ch == (id, ch)).then_some(v))
                .unwrap()
                .to_owned()
                .into_raw_vec_and_offset();

            let img = RgbaImage::from_vec(width, option.height, wav_img).unwrap();
            img.save(format!("samples/wav_{}_{}.png", sr_str, ch))
                .unwrap();
        }

        let (id_ch, imvec) = tm
            .draw_part_imgs(
                &tracklist,
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
            .pop()
            .unwrap();
        assert_eq!(id_ch, (0, 0));
        let im = RgbaImage::from_vec(imvec.len() as u32 / option.height / 4, option.height, imvec)
            .unwrap();
        im.save("samples/wav_part.png").unwrap();

        tm.remove_tracks(&mut tracklist, &[0]);
        let (updated_ids, _) = tm.apply_track_list_changes(&tracklist);
        assert!(updated_ids.is_empty());
    }
}
