use fast_image_resize::pixels;
use identity_hash::IntSet;
use itertools::Itertools;
use ndarray::prelude::*;
use rayon::prelude::*;

mod audio;
mod dynamics;
mod sinc;
mod spectrogram;
mod track;
mod tuple_hasher;
mod utils;
pub mod visualize;
mod windows;

pub use audio::AudioFormatInfo;
pub use dynamics::{DeciBel, GuardClippingMode, GuardClippingStats};
pub use spectrogram::SpecSetting;
pub use track::TrackList;
pub use tuple_hasher::TupleIntMap;
use tuple_hasher::TupleIntSet;
use visualize::Mipmaps;
pub use visualize::{
    ArrWithSliceInfo, OverviewDrawingInfoInternal, SpectrogramSliceArgs, WavDrawingInfoInternal,
    calc_amp_axis_markers, calc_dB_axis_markers, calc_freq_axis_markers, calc_time_axis_markers,
    convert_freq_label_to_hz, convert_hz_to_label, convert_sec_to_label, convert_time_label_to_sec,
};

pub type IdCh = (usize, usize);
pub type IdChVec = Vec<IdCh>;
pub type IdChArr = [IdCh];
pub type IdChValueVec<T> = Vec<(IdCh, T)>;
pub type IdChMap<T> = TupleIntMap<IdCh, T>;

use spectrogram::{SpectrogramAnalyzer, SrWinNfft};

#[readonly::make]
#[allow(non_snake_case)]
pub struct TrackManager {
    pub max_dB: f32,
    pub min_dB: f32,
    pub max_sr: u32,
    pub spec_mipmaps: IdChMap<Mipmaps>,
    pub setting: SpecSetting,
    pub dB_range: f32,
    pub colormap_length: u32,
    pub max_spectrogram_size: u32,
    spec_analyzer: SpectrogramAnalyzer,
    specs: IdChMap<Array2<f32>>,
    no_mipmap_ids: Vec<usize>,
}

impl TrackManager {
    pub fn new() -> Self {
        TrackManager {
            max_dB: -f32::INFINITY,
            min_dB: f32::INFINITY,
            max_sr: 0,
            spec_mipmaps: IdChMap::with_capacity_and_hasher(2, Default::default()),
            setting: Default::default(),
            dB_range: 100.,
            colormap_length: 258,
            max_spectrogram_size: 8192,
            spec_analyzer: SpectrogramAnalyzer::new(),
            specs: IdChMap::with_capacity_and_hasher(2, Default::default()),
            no_mipmap_ids: Vec::new(),
        }
    }

    pub fn with_max_spectrogram_size(max_spectrogram_size: u32) -> Self {
        Self {
            max_spectrogram_size,
            ..TrackManager::new()
        }
    }

    pub fn add_tracks(&mut self, tracklist: &TrackList, added_ids: &[usize]) {
        let sr_win_nfft_set = tracklist.construct_sr_win_nfft_set(added_ids, &self.setting);

        self.update_specs(
            tracklist,
            tracklist.id_ch_tuples_from(added_ids),
            &sr_win_nfft_set,
        );
        self.no_mipmap_ids.extend(added_ids.iter().copied());
    }

    pub fn reload_tracks(&mut self, tracklist: &TrackList, reloaded_ids: &[usize]) {
        let sr_win_nfft_set = tracklist.construct_sr_win_nfft_set(reloaded_ids, &self.setting);

        self.update_specs(
            tracklist,
            tracklist.id_ch_tuples_from(reloaded_ids),
            &sr_win_nfft_set,
        );
        self.no_mipmap_ids.extend(reloaded_ids.iter().copied());
    }

    pub fn remove_tracks(&mut self, tracklist: &TrackList, removed_id_ch_tuples: &IdChArr) {
        for tup in removed_id_ch_tuples {
            self.specs.remove(tup);
            self.spec_mipmaps.remove(tup);
        }
        if self.specs.capacity() > 2 * self.specs.len() {
            self.specs.shrink_to(2);
        }
        if self.spec_mipmaps.capacity() > 2 * self.spec_mipmaps.len() {
            self.spec_mipmaps.shrink_to(2);
        }

        self.spec_analyzer.retain(
            &tracklist.construct_all_sr_win_nfft_set(&self.setting),
            self.setting.freq_scale,
        );
    }

    pub fn apply_track_list_changes(&mut self, tracklist: &TrackList) -> (IntSet<usize>, u32) {
        let set = self.update_mipmaps(tracklist, false);
        (set, self.max_sr)
    }

    pub fn set_setting(&mut self, tracklist: &TrackList, setting: SpecSetting) {
        let sr_win_nfft_set = tracklist.construct_sr_win_nfft_set(&tracklist.all_ids(), &setting);

        self.setting = setting;
        self.spec_analyzer
            .retain(&sr_win_nfft_set, self.setting.freq_scale);
        self.update_specs(tracklist, tracklist.id_ch_tuples(), &sr_win_nfft_set);
        self.update_mipmaps(tracklist, true);
    }

    pub fn update_all_specs_mipmaps(&mut self, tracklist: &TrackList) {
        self.update_specs(tracklist, tracklist.id_ch_tuples(), None);
        self.update_mipmaps(tracklist, true);
    }

    #[allow(non_snake_case)]
    pub fn set_dB_range(&mut self, tracklist: &TrackList, dB_range: f32) {
        self.dB_range = dB_range;
        self.update_mipmaps(tracklist, true);
    }

    pub fn set_colormap_length(&mut self, tracklist: &TrackList, colormap_length: u32) {
        self.colormap_length = colormap_length;
        self.update_mipmaps(tracklist, true);
    }

    pub fn get_sliced_spec_mipmap(
        &self,
        (id, ch): IdCh,
        track_sec: f64,
        sec_range: (f64, f64),
        hz_range: (f32, f32),
        margin_px: usize,
    ) -> Option<(SpectrogramSliceArgs, Array2<pixels::F32>)> {
        let spec_hz_range = (0., self.max_sr as f32 / 2.);
        let hz_range = ((hz_range.0).max(0.), (hz_range.1).min(spec_hz_range.1));

        self.spec_mipmaps.get(&(id, ch)).map(|spec_mipmap| {
            let (args, sliced_arr) = spec_mipmap.get_sliced_mipmap(
                track_sec,
                (sec_range.0, sec_range.1),
                spec_hz_range,
                hz_range,
                margin_px,
                &self.setting,
            );
            (args, sliced_arr.to_owned())
        })
    }

    fn update_specs<'a>(
        &mut self,
        tracklist: &TrackList,
        id_ch_tuples: IdChVec,
        framing_params: impl Into<Option<&'a TupleIntSet<SrWinNfft>>>,
    ) {
        match framing_params.into() {
            Some(p) => {
                self.spec_analyzer.prepare(p, self.setting.freq_scale);
            }
            None => {
                let p = tracklist.construct_all_sr_win_nfft_set(&self.setting);
                self.spec_analyzer.prepare(&p, self.setting.freq_scale);
            }
        }
        let parallel = id_ch_tuples.len() < rayon::current_num_threads();
        let specs: Vec<_> = id_ch_tuples
            .into_par_iter()
            .map(|(id, ch)| {
                let track = &tracklist[id];
                let spec = self.spec_analyzer.calc_spec(
                    track.channel(ch),
                    track.sr(),
                    &self.setting,
                    parallel,
                );
                ((id, ch), spec)
            })
            .collect();
        self.specs.extend(specs);
    }

    /// update spec_mipmaps, max_dB, min_dB, max_sr
    /// clear no_mipmap_ids
    fn update_mipmaps(&mut self, tracklist: &TrackList, force_update_all: bool) -> IntSet<usize> {
        let (mut min, mut max) = self
            .specs
            .par_iter()
            .filter_map(|(_, spec)| spec.iter().minmax().into_option())
            .map(|(&min, &max)| (min, max))
            .reduce(
                || (f32::INFINITY, f32::NEG_INFINITY),
                |(min, max), (current_min, current_max)| {
                    (min.min(current_min), max.max(current_max))
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
        let ids_need_update: IntSet<usize> = if need_update_all {
            self.no_mipmap_ids.clear();
            tracklist.all_id_set()
        } else {
            self.no_mipmap_ids.drain(..).collect()
        };

        if !ids_need_update.is_empty() {
            let new_spec_mipmaps: Vec<_> = self
                .specs
                .par_iter()
                .filter(|((id, _), _)| ids_need_update.contains(id))
                .map(|(&(id, ch), spec)| {
                    let sr = tracklist[id].sr();
                    let i_freq_range = self.setting.freq_scale.hz_range_to_idx(
                        (0., self.max_sr as f32 / 2.),
                        sr,
                        spec.shape()[1],
                    );
                    let spec_img = visualize::convert_spec_to_img(
                        spec.view(),
                        i_freq_range,
                        (self.min_dB, self.max_dB),
                        Some(self.colormap_length),
                    );
                    ((id, ch), Mipmaps::new(spec_img, self.max_spectrogram_size))
                })
                .collect();

            if need_update_all {
                self.spec_mipmaps = new_spec_mipmaps.into_iter().collect();
            } else {
                self.spec_mipmaps.extend(new_spec_mipmaps)
            }
        }
        ids_need_update
    }
}

#[cfg(test)]
mod tests {
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
        let added_ids = tracklist.add_tracks(id_list[0..3].to_owned(), path_list[0..3].to_owned());
        tm.add_tracks(&tracklist, &added_ids);
        assert_eq!(&added_ids, &id_list[0..3]);
        let added_ids = tracklist.add_tracks(id_list[3..].to_owned(), path_list[3..].to_owned());
        tm.add_tracks(&tracklist, &added_ids);
        assert_eq!(&added_ids, &id_list[3..]);
        assert_eq!(tracklist.all_ids().len(), id_list.len());

        assert_eq!(tm.spec_mipmaps.len(), 0);
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

        let removed_id_ch_tuples = tracklist.remove_tracks(&[0]);
        tm.remove_tracks(&tracklist, &removed_id_ch_tuples);
        let (updated_ids, _) = tm.apply_track_list_changes(&tracklist);
        assert!(updated_ids.is_empty());
    }
}
