use criterion::{black_box, criterion_group, criterion_main, Criterion};
use display::GreyF32Image;
use ndarray::prelude::*;
use ndarray_stats::QuantileExt;
use thesia::{audio, decibel::DeciBelInplace, display, mel, perform_stft, windows};

fn get_melspectrogram(
    wav: ArrayView1<f32>,
    window: ArrayView1<f32>,
    mel_fb: ArrayView2<f32>,
) -> Array2<f32> {
    let stft = perform_stft(wav, 1920, 480, 2048, Some(CowArray::from(window)), None, true);
    let linspec = stft.mapv(|x| x.norm());
    let mut melspec = linspec.dot(&mel_fb);
    melspec.amp_to_db_default();
    melspec
}

fn draw_spec(spec_grey: &GreyF32Image, nwidth: u32) {
    display::grey_to_rgb(spec_grey, nwidth, 500);
    // im.save("spec.png").unwrap();
}

fn benchmark_get_melspec(c: &mut Criterion) {
    let (wav, sr) = audio::open_audio_file("samples/sample.wav").unwrap();
    let wav = wav.sum_axis(Axis(0));
    let wav = wav.slice_move(s![..sr as usize]);
    let window = CowArray::from(windows::hann(1920, false) / 2048.);
    let mel_fb = mel::calc_mel_fb_default(sr, 2048);
    c.bench_function("get mel spectrogram", move |b| {
        b.iter(|| {
            get_melspectrogram(
                black_box(wav.view()),
                black_box(window.view()),
                black_box(mel_fb.view()),
            )
        })
    });
}

fn benchmark_draw_spec(c: &mut Criterion) {
    let (wav, sr) = audio::open_audio_file("samples/sample.wav").unwrap();
    let wav = wav.sum_axis(Axis(0));
    let wav = wav.slice_move(s![..sr as usize]);
    let window = windows::hann(1920, false) / 2048.;
    let mel_fb = mel::calc_mel_fb_default(sr, 2048);
    let spec = get_melspectrogram(wav.view(), window.view(), mel_fb.view());
    let spec_grey = display::spec_to_grey(spec.view(), *spec.max().unwrap(), *spec.min().unwrap());
    c.bench_function("draw spectrogram", move |b| {
        b.iter(|| {
            draw_spec(
                black_box(&spec_grey),
                black_box(100 * wav.len() as u32 / sr),
            )
        })
    });
}

criterion_group!(benches, benchmark_get_melspec, benchmark_draw_spec);
criterion_main!(benches);
