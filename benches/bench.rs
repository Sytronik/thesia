extern crate multi_spectrogram_viewer;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use multi_spectrogram_viewer::{audio, decibel::DeciBelInplace, display, mel, stft};
use ndarray::prelude::*;

fn get_melspectrogram(wav: ArrayView1<f32>, sr: u32) -> Array2<f32> {
    let spec = stft(wav, 1920, 480, false);
    let mag = spec.mapv(|x| x.norm());
    let mut melspec = mag.dot(&mel::mel_filterbanks(sr, 2048, 128, 0f32, None));
    melspec.amp_to_db_default();
    melspec
}

fn draw_spec(spec: ArrayView2<f32>, nwidth: u32) {
    display::spec_to_image(spec, nwidth, 100);
    // im.save("spec.png").unwrap();
}

fn benchmark_get_melspec(c: &mut Criterion) {
    let (wav, sr) = audio::open_audio_file("samples/sample.wav").unwrap();
    let wav = wav.sum_axis(Axis(0));
    let wav = wav.slice_move(s![..sr as usize]);
    c.bench_function("get mel spectrogram", |b| {
        b.iter(|| get_melspectrogram(black_box(wav.view()), black_box(sr)))
    });
}

fn benchmark_draw_spec(c: &mut Criterion) {
    let (wav, sr) = audio::open_audio_file("samples/sample.wav").unwrap();
    let wav = wav.sum_axis(Axis(0));
    let wav = wav.slice_move(s![..sr as usize]);
    let spec = get_melspectrogram(wav.view(), sr);
    c.bench_function("draw spectrogram", |b| {
        b.iter(|| {
            draw_spec(
                black_box(spec.view()),
                black_box(100 * wav.len() as u32 / sr),
            )
        })
    });
}

criterion_group!(benches, benchmark_get_melspec, benchmark_draw_spec);
criterion_main!(benches);
