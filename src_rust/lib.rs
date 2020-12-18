use std::collections::{HashMap, HashSet};
use std::io;
use std::ops::*;

use approx::abs_diff_ne;
use ndarray::{prelude::*, ScalarOperand};
use ndarray_stats::QuantileExt;
use rayon::prelude::*;
use rustfft::{num_complex::Complex, num_traits::Float, FFTnum};
use wasm_bindgen::prelude::*;

pub mod audio;
pub mod decibel;
pub mod display;
pub mod mel;
pub mod realfft;
pub mod utils;
pub mod windows;
use decibel::DeciBelInplace;
use display::GreyF32Image;
use realfft::RealFFT;
use utils::{calc_proper_n_fft, pad, par_collect_to_hashmap, PadMode};

pub enum FreqScale {
    Linear,
    Mel,
}

pub struct AudioTrack {
    path: String,
    wav: Array1<f32>,
    sr: u32,
    win_length: usize,
    hop_length: usize,
    n_fft: usize,
}

impl AudioTrack {
    pub fn new(path: &str, setting: &SpecSetting) -> io::Result<Self> {
        let (wav, sr) = audio::open_audio_file(path)?;
        let wav = wav.sum_axis(Axis(0)); // TODO: stereo support
        let win_length = setting.win_ms * sr as f32 / 1000.;
        let hop_length = (win_length / setting.t_overlap as f32).round() as usize;
        let win_length = hop_length * setting.t_overlap;
        let n_fft = calc_proper_n_fft(win_length) * setting.f_overlap;
        Ok(AudioTrack {
            path: path.to_string(),
            wav,
            sr,
            win_length,
            hop_length,
            n_fft,
        })
    }

    pub fn reload(&mut self, setting: &SpecSetting) -> io::Result<()> {
        let new = AudioTrack::new(&self.path[..], setting)?;
        *self = new;
        Ok(())
    }
}

pub struct SpecSetting {
    win_ms: f32,
    t_overlap: usize,
    f_overlap: usize,
    freq_scale: FreqScale,
    db_range: f32,
}

#[wasm_bindgen]
pub struct MultiTrack {
    tracks: HashMap<usize, AudioTrack>,
    setting: SpecSetting,
    windows: HashMap<u32, Array1<f32>>,
    mel_fbs: HashMap<u32, Array2<f32>>,
    specs: HashMap<usize, Array2<f32>>,
    spec_greys: HashMap<usize, GreyF32Image>,
    max_db: f32,
    min_db: f32,
    max_sec: f32,
    id_max_sec: usize,
}

#[wasm_bindgen]
impl MultiTrack {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        MultiTrack {
            tracks: HashMap::new(),
            setting: SpecSetting {
                win_ms: 40.,
                t_overlap: 4,
                f_overlap: 1,
                freq_scale: FreqScale::Mel,
                db_range: 120.,
            },
            windows: HashMap::new(),
            mel_fbs: HashMap::new(),
            specs: HashMap::new(),
            spec_greys: HashMap::new(),
            max_db: -f32::INFINITY,
            min_db: f32::INFINITY,
            max_sec: 0.,
            id_max_sec: 0,
        }
    }

    fn calc_spec_of(&self, id: usize, parallel: bool) -> Array2<f32> {
        let track = self.tracks.get(&id).unwrap();
        let window = Some(CowArray::from(self.windows.get(&track.sr).unwrap().view()));
        let stft = perform_stft(
            track.wav.view(),
            track.win_length,
            track.hop_length,
            track.n_fft,
            window,
            None,
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

    fn calc_window(win_length: usize, n_fft: usize) -> Array1<f32> {
        windows::hann(win_length, false) / n_fft as f32
    }

    fn update_specs(&mut self, id_list: &[usize], new_sr_set: HashSet<(u32, usize, usize)>) {
        let new_windows = par_collect_to_hashmap(
            new_sr_set
                .par_iter()
                .map(|&(sr, win_length, n_fft)| (sr, MultiTrack::calc_window(win_length, n_fft))),
            Some(new_sr_set.len()),
        );
        self.windows.extend(new_windows);

        if let FreqScale::Mel = self.setting.freq_scale {
            let mel_fbs = par_collect_to_hashmap(
                new_sr_set
                    .par_iter()
                    .map(|&(sr, _, n_fft)| (sr, mel::calc_mel_fb_default(sr, n_fft))),
                Some(new_sr_set.len()),
            );
            self.mel_fbs.extend(mel_fbs);
        }

        let specs = par_collect_to_hashmap(
            id_list
                .par_iter()
                .map(|&id| (id, self.calc_spec_of(id, id_list.len() == 1))),
            Some(id_list.len()),
        );
        self.specs.extend(specs);
    }

    #[wasm_bindgen(catch)]
    pub fn add_tracks(&mut self, id_list: &[usize], path_list: &str) -> Result<bool, JsValue> {
        let mut new_sr_set = HashSet::<(u32, usize, usize)>::new();
        for (&id, path) in id_list.iter().zip(path_list.split("\n").into_iter()) {
            let track = match AudioTrack::new(path, &self.setting) {
                Ok(track) => track,
                Err(err) => return Err(JsValue::from(err.to_string())),
            };
            let sec = track.wav.len() as f32 / track.sr as f32;
            if sec > self.max_sec {
                self.max_sec = sec;
                self.id_max_sec = id;
            }
            if let None = self.windows.get(&track.sr) {
                new_sr_set.insert((track.sr, track.win_length, track.n_fft));
            }
            self.tracks.insert(id, track);
        }

        self.update_specs(id_list, new_sr_set);
        Ok(self.update_db_scale())
    }

    fn update_db_scale(&mut self) -> bool {
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
                |(max, min): (f32, f32), (current_max, current_min)| {
                    (max.max(current_max), min.min(current_min))
                },
            );
        max = max.min(0.);
        min = min.max(max - self.setting.db_range);
        let mut changed = false;
        if abs_diff_ne!(self.max_db, max, epsilon = 1e-3) {
            self.max_db = max;
            changed = true;
        }
        if abs_diff_ne!(self.min_db, min, epsilon = 1e-3) {
            self.min_db = min;
            changed = true;
        }

        if changed {
            self.spec_greys = par_collect_to_hashmap(
                self.specs.par_iter().map(|(&id, spec)| {
                    let grey = display::spec_to_grey(spec.view(), self.max_db, self.min_db);
                    (id, grey)
                }),
                Some(self.specs.len()),
            );
        }
        changed
    }

    pub fn remove_track(&mut self, id: usize) -> bool {
        let sr = self.tracks.remove_entry(&id).unwrap().1.sr;
        self.specs.remove(&id);
        self.spec_greys.remove(&id);
        if self.id_max_sec == id {
            let (id, max_sec) = self
                .tracks
                .par_iter()
                .map(|(&id, track)| (id, track.wav.len() as f32 / track.sr as f32))
                .reduce(
                    || (0, 0.),
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
        if self.tracks.par_iter().all(|(_, track)| track.sr != sr) {
            self.windows.remove(&sr);
            self.mel_fbs.remove(&sr);
        }
        self.update_db_scale()
    }

    pub fn get_spec_image(&mut self, id: usize, px_per_sec: f32, nheight: u32) -> Vec<u8> {
        let track = self.tracks.get(&id).unwrap();
        let nwidth = (px_per_sec * track.wav.len() as f32 / track.sr as f32) as u32;
        display::grey_to_rgb(self.spec_greys.get(&id).unwrap(), nwidth, nheight).into_raw()
    }

    pub fn get_frequency_hz(&self, id: usize, relative_freq: f32) -> f32 {
        let half_sr = self.tracks.get(&id).unwrap().sr as f32 / 2.;

        match self.setting.freq_scale {
            FreqScale::Linear => half_sr * relative_freq,
            FreqScale::Mel => mel::mel_to_hz(mel::hz_to_mel(half_sr) * relative_freq),
        }
    }

    pub fn get_max_db(&self) -> f32 {
        self.max_db
    }

    pub fn get_min_db(&self) -> f32 {
        self.min_db
    }

    pub fn get_max_sec(&self) -> f32 {
        self.max_sec
    }
}

fn to_windowed_frames<A: Float>(
    input: ArrayView1<A>,
    window: ArrayView1<A>,
    hop_length: usize,
    (n_pad_left, n_pad_right): (usize, usize),
) -> Vec<Array1<A>> {
    input
        .windows(window.len())
        .into_iter()
        .step_by(hop_length)
        .map(|x| {
            pad(
                (&x * &window).view(),
                (n_pad_left, n_pad_right),
                Axis(0),
                PadMode::Constant(A::zero()),
            )
        })
        .collect()
}

pub fn perform_stft<A>(
    input: ArrayView1<A>,
    win_length: usize,
    hop_length: usize,
    n_fft: usize,
    window: Option<CowArray<A, Ix1>>,
    fft_module: Option<&mut RealFFT<A>>,
    parallel: bool,
) -> Array2<Complex<A>>
where
    A: FFTnum + Float + DivAssign + ScalarOperand,
{
    let n_pad_left = (n_fft - win_length) / 2;
    let n_pad_right = (((n_fft - win_length) as f32) / 2.).ceil() as usize;

    let window = if let Some(w) = window {
        assert_eq!(w.len(), win_length);
        w
    } else {
        CowArray::from(windows::hann(win_length, false) / A::from(n_fft).unwrap())
    };

    let to_frames_wrapper =
        |x| to_windowed_frames(x, window.view(), hop_length, (n_pad_left, n_pad_right));
    let front_wav = pad(
        input.slice(s![..(win_length - 1)]),
        (win_length / 2, 0),
        Axis(0),
        PadMode::Reflect,
    );
    let mut front_frames = to_frames_wrapper(front_wav.view());

    let mut first_idx = front_frames.len() * hop_length - win_length / 2;
    let mut frames: Vec<Array1<A>> = to_frames_wrapper(input.slice(s![first_idx..]));

    first_idx += frames.len() * hop_length;
    let back_wav_start_idx = first_idx.min(input.len() - win_length / 2 - 1);

    let mut back_wav = pad(
        input.slice(s![back_wav_start_idx..]),
        (0, win_length / 2),
        Axis(0),
        PadMode::Reflect,
    );
    back_wav.slice_collapse(s![(first_idx - back_wav_start_idx).max(0)..]);
    let mut back_frames = to_frames_wrapper(back_wav.view());

    let n_frames = front_frames.len() + frames.len() + back_frames.len();
    let mut output = Array2::<Complex<A>>::zeros((n_frames, n_fft / 2 + 1));
    let out_frames: Vec<&mut [Complex<A>]> = output
        .axis_iter_mut(Axis(0))
        .map(|x| x.into_slice().unwrap())
        .collect();

    let mut new_module;
    let fft_module = if let Some(m) = fft_module {
        m
    } else {
        new_module = RealFFT::<A>::new(n_fft).unwrap();
        &mut new_module
    };
    if parallel {
        let in_frames = front_frames
            .par_iter_mut()
            .chain(frames.par_iter_mut())
            .chain(back_frames.par_iter_mut());
        in_frames.zip(out_frames).for_each(|(x, y)| {
            let mut fft_module = RealFFT::<A>::new(n_fft).unwrap();
            let x = x.as_slice_mut().unwrap();
            fft_module.process(x, y).unwrap();
        });
    } else {
        let in_frames = front_frames
            .iter_mut()
            .chain(frames.iter_mut())
            .chain(back_frames.iter_mut());
        in_frames.zip(out_frames).for_each(|(x, y)| {
            let x = x.as_slice_mut().unwrap();
            fft_module.process(x, y).unwrap();
        });
    }

    output
}

#[wasm_bindgen]
pub fn get_colormap() -> Vec<u8> {
    display::COLORMAP
        .iter()
        .flat_map(|x| x.iter())
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use image::RgbImage;
    use ndarray::{arr2, Array1};
    use rustfft::num_complex::Complex;

    use super::utils::Impulse;
    use super::*;

    #[test]
    fn stft_works() {
        let impulse = Array1::<f32>::impulse(4, 2);
        assert_eq!(
            perform_stft(impulse.view(), 4, 2, 4, None, None, false),
            arr2(&[
                [
                    Complex::<f32>::new(0., 0.),
                    Complex::<f32>::new(0., 0.),
                    Complex::<f32>::new(0., 0.)
                ],
                [
                    Complex::<f32>::new(1. / 4., 0.),
                    Complex::<f32>::new(-1. / 4., 0.),
                    Complex::<f32>::new(1. / 4., 0.)
                ],
                [
                    Complex::<f32>::new(1. / 4., 0.),
                    Complex::<f32>::new(-1. / 4., 0.),
                    Complex::<f32>::new(1. / 4., 0.)
                ]
            ])
        );
    }

    #[test]
    fn multitrack_works() {
        let mut multitrack = MultiTrack::new();
        multitrack
            .add_tracks(
                &[0, 1, 2, 3, 4, 5],
                ["samples/sample.wav"; 6].join("\n").as_str(),
            )
            .unwrap();
        let imvec = multitrack.get_spec_image(0, 100., 500);
        let im = RgbImage::from_vec((imvec.len() / 500 / 3) as u32, 500, imvec).unwrap();
        im.save("spec.png").unwrap();
        multitrack.remove_track(0);
    }
}
