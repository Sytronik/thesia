use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ndarray::{prelude::*, ArcArray1};
use ndarray_stats::QuantileExt;
use thesia::{audio, decibel::DeciBelInplace, display, mel, perform_stft, windows};

fn get_melspectrogram(wav: ArrayView1<f32>, window: ArcArray1<f32>, mel_fb: ArrayView2<f32>) -> Array2<f32> {
    let stft = perform_stft(wav, 1920, 480, 2048, Some(window), None, true);
    let linspec = stft.mapv(|x| x.norm());
    let mut melspec = linspec.dot(&mel_fb);
    melspec.amp_to_db_default();
    melspec
}

fn draw_spec(spec: ArrayView2<f32>, nwidth: u32) {
    display::spec_to_image(
        spec,
        nwidth,
        500,
        *spec.max().unwrap(),
        *spec.min().unwrap(),
    );
    // im.save("spec.png").unwrap();
}

fn benchmark_get_melspec(c: &mut Criterion) {
    let (wav, sr) = audio::open_audio_file("samples/sample.wav").unwrap();
    let wav = wav.sum_axis(Axis(0));
    let wav = wav.slice_move(s![..sr as usize]);
    let window = (windows::hann(1920, false) / 2048.).into_shared();
    let mel_fb = mel::calc_mel_fb_default(sr, 2048);
    c.bench_function("get mel spectrogram", move |b| {
        b.iter(|| {
            get_melspectrogram(
                black_box(wav.view()),
                black_box(ArcArray::clone(&window)),
                black_box(mel_fb.view()),
            )
        })
    });
}

fn benchmark_draw_spec(c: &mut Criterion) {
    let (wav, sr) = audio::open_audio_file("samples/sample.wav").unwrap();
    let wav = wav.sum_axis(Axis(0));
    let wav = wav.slice_move(s![..sr as usize]);
    let window = (windows::hann(1920, false) / 2048.).into_shared();
    let mel_fb = mel::calc_mel_fb_default(sr, 2048);
    let spec = get_melspectrogram(wav.view(), window, mel_fb.view());
    c.bench_function("draw spectrogram", move |b| {
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
