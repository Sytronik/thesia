use std::sync::{RwLock, RwLockReadGuard};
use std::time::Instant;

use lazy_static::{initialize, lazy_static};
use ndarray::{Array3, ArrayViewMut3, Axis, Slice};
use neon::prelude::*;
use rayon::prelude::*;

mod neon_utils;

use neon_utils::*;
use thesia_backend::*;

const MAX_IMAGE_SIZE: u32 = 8192;

lazy_static! {
    static ref TM: RwLock<TrackManager> = RwLock::new(TrackManager::new());
    static ref DRAWOPTION: RwLock<DrawOption> = RwLock::new(DrawOption {
        px_per_sec: 0.,
        height: 1,
    });
    static ref DRAWOPTION_FOR_WAV: RwLock<DrawOptionForWav> = RwLock::new(DrawOptionForWav {
        amp_range: (-1., 1.)
    });
    static ref SPEC_IMAGES: RwLock<IdChMap<Array3<u8>>> = RwLock::new(IdChMap::new());
    static ref WAV_IMAGES: RwLock<IdChMap<Array3<u8>>> = RwLock::new(IdChMap::new());
}

fn add_tracks(mut cx: FunctionContext) -> JsResult<JsUndefined> {
    let new_track_ids: Vec<usize> = get_arr_arg!(cx, 0, JsNumber, usize);
    let new_paths: Vec<String> = get_arr_arg!(cx, 1, JsString, "");
    let callback = cx.argument::<JsFunction>(2)?;
    let mut tm = TM.write().unwrap();
    match tm.add_tracks(&new_track_ids[..], new_paths) {
        Ok(should_draw_all) => {
            RenderingTask {
                id_ch_tuples: if should_draw_all {
                    tm.id_ch_tuples()
                } else {
                    tm.id_ch_tuples_from(&new_track_ids[..])
                },
                option: *DRAWOPTION.read().unwrap(),
                option_for_wav: *DRAWOPTION_FOR_WAV.read().unwrap(),
            }
            .schedule(callback);
            Ok(cx.undefined())
        }
        Err(_) => cx.throw_error("Unsupported file type!"),
    }
}

fn remove_track(mut cx: FunctionContext) -> JsResult<JsUndefined> {
    // TODO: multiple track_ids
    let track_id = get_num_arg!(cx, 0, usize);
    let callback = cx.argument::<JsFunction>(1)?;
    let mut tm = TM.write().unwrap();
    let mut spec_images = SPEC_IMAGES.write().unwrap();
    let mut wav_images = WAV_IMAGES.write().unwrap();
    for tup in tm.id_ch_tuples_from(&[track_id]).iter() {
        spec_images.remove(tup);
        wav_images.remove(tup);
    }
    if tm.remove_track(track_id) {
        RenderingTask {
            id_ch_tuples: tm.id_ch_tuples(),
            option: *DRAWOPTION.read().unwrap(),
            option_for_wav: *DRAWOPTION_FOR_WAV.read().unwrap(),
        }
        .schedule(callback);
    }
    Ok(cx.undefined())
}

fn crop_cached_images_<'a>(
    cx: &mut FunctionContext<'a>,
    images: &RwLockReadGuard<IdChMap<Array3<u8>>>,
    id_ch_tuples: &[(usize, usize)],
    sec: f64,
    width: u32,
    option: DrawOption,
) -> JsResult<'a, JsArrayBuffer> {
    let mut buf = JsArrayBuffer::new(cx, id_ch_tuples.len() as u32 * width * option.height * 4)?;
    cx.borrow_mut(&mut buf, |borrowed| {
        let chunk_iter = borrowed
            .as_mut_slice()
            .par_chunks_exact_mut((width * option.height * 4) as usize);

        id_ch_tuples
            .into_par_iter()
            .zip(chunk_iter)
            .for_each(|(id_ch, output)| {
                let image = images.get(&id_ch).unwrap();
                let total_width = image.len() / 4 / option.height as usize;
                let i_w = (sec * option.px_per_sec) as isize;
                let (i_w_eff, width_eff) = match calc_effective_w(i_w, width as usize, total_width)
                {
                    Some((i, w)) => (i as isize, w as isize),
                    None => return,
                };
                let slice = Slice::new(i_w_eff, Some(i_w_eff + width_eff), 1);

                let shape = (option.height as usize, width as usize, 4);
                let mut out_view = ArrayViewMut3::from_shape(shape, output).unwrap();
                image
                    .slice_axis(Axis(1), slice)
                    .indexed_iter()
                    .for_each(|((h, w, i), x)| {
                        out_view[[h, (w as isize - i_w.min(0)) as usize, i]] = *x;
                    });
            });
    });
    Ok(buf)
}

fn get_part_images_<'a>(
    cx: &mut FunctionContext<'a>,
    id_ch_tuples: &[(usize, usize)],
    sec: f64,
    width: u32,
    option: DrawOption,
    kind: ImageKind,
    fast_resize: bool,
) -> JsResult<'a, JsArrayBuffer> {
    let tm = TM.read().unwrap();
    let mut buf = JsArrayBuffer::new(cx, id_ch_tuples.len() as u32 * width * option.height * 4)?;
    cx.borrow_mut(&mut buf, |borrowed| {
        tm.draw_part_images_to(
            &id_ch_tuples[..],
            borrowed.as_mut_slice(),
            sec,
            width,
            option,
            kind,
            fast_resize,
        );
    });
    Ok(buf)
}

fn get_spec_wav_images(mut cx: FunctionContext) -> JsResult<JsObject> {
    let id_ch_jsarr = cx.argument::<JsArray>(0)?;
    let sec = get_num_arg!(cx, 1);
    let width = get_num_arg!(cx, 2, u32);
    let option = get_drawoption_arg_(&mut cx, 3)?;
    let option_for_wav = get_drawoption_for_wav_arg_(&mut cx, 4)?;
    let callback = cx.argument::<JsFunction>(5)?;

    let id_ch_tuples = vec_from_jsarr(&mut cx, id_ch_jsarr, str_to_id_ch_tuple)?;

    // TODO: TM에 실제로 있는 id, ch 만 거르고
    // TODO: id_ch_tuple을 두 파트로 나눠서 high로 가거나 low로 가거나
    let obj = JsObject::new(&mut cx);
    obj.set(&mut cx, "id_ch_arr", id_ch_jsarr)?;

    // dbg!(sec, width, &option);
    let same_option = option == *DRAWOPTION.read().unwrap()
        && option_for_wav == *DRAWOPTION_FOR_WAV.read().unwrap();
    let total_width = option.px_per_sec * TM.read().unwrap().max_sec;
    let too_large = total_width > MAX_IMAGE_SIZE as f64 || option.height > MAX_IMAGE_SIZE;
    if same_option && !too_large && SPEC_IMAGES.try_read().is_ok() {
        let images = SPEC_IMAGES.read().unwrap();
        let start = Instant::now();

        let buf_specs =
            crop_cached_images_(&mut cx, &images, &id_ch_tuples[..], sec, width, option)?;
        obj.set(&mut cx, "specs", buf_specs)?;
        // println!("Copy high q spec: {:?}", start.elapsed());
        // }
        // if same_option && !too_large && WAV_IMAGES.try_read().is_ok() {
        let images = WAV_IMAGES.read().unwrap();
        // let start = Instant::now();
        let buf_wavs =
            crop_cached_images_(&mut cx, &images, &id_ch_tuples[..], sec, width, option)?;
        obj.set(&mut cx, "wavs", buf_wavs)?;
        println!("Copy high q: {:?}", start.elapsed());
    } else {
        let start = Instant::now();
        if !same_option && !too_large {
            RenderingTask {
                id_ch_tuples: id_ch_tuples.clone(),
                option,
                option_for_wav,
            }
            .schedule(callback);
        }
        if let Ok(mut images) = SPEC_IMAGES.try_write() {
            if too_large && !images.is_empty() {
                images.clear();
            }
        }
        if let Ok(mut images) = WAV_IMAGES.try_write() {
            if too_large && !images.is_empty() {
                images.clear();
            }
        }
        let buf_specs = get_part_images_(
            &mut cx,
            &id_ch_tuples[..],
            sec,
            width,
            option,
            ImageKind::Spec,
            !too_large,
        )?;
        let buf_wavs = get_part_images_(
            &mut cx,
            &id_ch_tuples[..],
            sec,
            width,
            option,
            ImageKind::Wav(option_for_wav),
            !too_large,
        )?;
        obj.set(&mut cx, "specs", buf_specs)?;
        obj.set(&mut cx, "wavs", buf_wavs)?;
        {
            let high_or_low = if too_large { "high" } else { "low" };
            println!("Draw {} q: {:?}", high_or_low, start.elapsed());
        }
    }
    Ok(obj)
}
#[derive(Clone)]
struct RenderingTask {
    id_ch_tuples: IdChVec,
    option: DrawOption,
    option_for_wav: DrawOptionForWav,
}

impl Task for RenderingTask {
    type Output = Option<IdChVec>;
    type Error = ();
    type JsEvent = JsArray;

    fn perform(&self) -> Result<Self::Output, Self::Error> {
        if *DRAWOPTION.read().unwrap() == self.option {
            Ok(None)
        } else if let Ok(mut spec_images) = SPEC_IMAGES.try_write() {
            // while (true) {}
            // TODO: Check id_ch_tuples
            // TODO: WAV_IMAGES 따로
            let mut wav_images = WAV_IMAGES.write().unwrap();
            let new_specs = TM.read().unwrap().get_entire_images(
                &self.id_ch_tuples[..],
                self.option,
                ImageKind::Spec,
            );
            spec_images.par_extend(
                self.id_ch_tuples
                    .par_iter()
                    .cloned()
                    .zip_eq(new_specs.into_par_iter()),
            );
            let new_wavs = TM.read().unwrap().get_entire_images(
                &self.id_ch_tuples[..],
                self.option,
                ImageKind::Wav(self.option_for_wav),
            );
            wav_images.par_extend(
                self.id_ch_tuples
                    .par_iter()
                    .cloned()
                    .zip_eq(new_wavs.into_par_iter()),
            );
            *DRAWOPTION.write().unwrap() = self.option;
            Ok(Some(self.id_ch_tuples.clone()))
        } else {
            Ok(None)
        }
    }

    fn complete<'a>(
        self,
        mut cx: TaskContext<'a>,
        id_ch_tuples: Result<Self::Output, Self::Error>,
    ) -> JsResult<Self::JsEvent> {
        match id_ch_tuples {
            // Ok(Some(tuples)) => {
            Ok(Some(tuples)) if self.option == *DRAWOPTION.read().unwrap() => {
                let arr = jsarr_from_strings(&mut cx, &tuples_to_str_vec!(tuples)[..])?;
                Ok(arr)
            }
            Ok(Some(_)) => Ok(cx.empty_array()),
            Ok(None) => cx.throw_error("no need to refresh."),
            Err(_) => cx.throw_error("Unknown error!"),
        }
    }
}

fn get_max_db(mut cx: FunctionContext) -> JsResult<JsNumber> {
    Ok(cx.number(TM.read().unwrap().max_db))
}

fn get_min_db(mut cx: FunctionContext) -> JsResult<JsNumber> {
    Ok(cx.number(TM.read().unwrap().min_db))
}

fn get_n_ch(mut cx: FunctionContext) -> JsResult<JsNumber> {
    let tm = TM.read().unwrap();
    let track = get_track!(tm, cx, 0);
    Ok(cx.number(track.n_ch as u32))
}

fn get_sec(mut cx: FunctionContext) -> JsResult<JsNumber> {
    let tm = TM.read().unwrap();
    let track = get_track!(tm, cx, 0);
    Ok(cx.number(track.sec()))
}

fn get_sr(mut cx: FunctionContext) -> JsResult<JsNumber> {
    let tm = TM.read().unwrap();
    let track = get_track!(tm, cx, 0);
    Ok(cx.number(track.sr))
}

fn get_path(mut cx: FunctionContext) -> JsResult<JsString> {
    let tm = TM.read().unwrap();
    let track = get_track!(tm, cx, 0);
    Ok(cx.string(track.path_string()))
}

fn get_filename(mut cx: FunctionContext) -> JsResult<JsString> {
    let tm = TM.read().unwrap();
    let track = get_track!(tm, cx, 0);
    Ok(cx.string(track.filename()))
}

fn get_colormap(mut cx: FunctionContext) -> JsResult<JsArrayBuffer> {
    let (c_iter, len) = get_colormap_iter_size();
    let mut buf = JsArrayBuffer::new(&mut cx, len as u32)?;
    cx.borrow_mut(&mut buf, |borrowed| {
        for (x, &y) in borrowed.as_mut_slice().iter_mut().zip(c_iter) {
            *x = y;
        }
    });
    Ok(buf)
}

register_module!(mut m, {
    initialize(&TM);
    initialize(&DRAWOPTION);
    initialize(&DRAWOPTION_FOR_WAV);
    initialize(&SPEC_IMAGES);
    initialize(&WAV_IMAGES);
    m.export_function("addTracks", add_tracks)?;
    m.export_function("removeTrack", remove_track)?;
    m.export_function("getMaxdB", get_max_db)?;
    m.export_function("getMindB", get_min_db)?;
    m.export_function("getNumCh", get_n_ch)?;
    m.export_function("getSec", get_sec)?;
    m.export_function("getSr", get_sr)?;
    m.export_function("getPath", get_path)?;
    m.export_function("getFileName", get_filename)?;
    m.export_function("getColormap", get_colormap)?;
    m.export_function("getSpecWavImages", get_spec_wav_images)?;
    Ok(())
});
