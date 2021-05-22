use std::convert::TryInto;
use std::sync::{Arc, Mutex, MutexGuard, RwLock};
use std::task::{Context, Poll};

use napi::{
    CallContext, ContextlessResult, Env, JsBuffer, JsNumber, JsObject, JsString, JsUndefined,
    Result as JsResult,
};
use napi_derive::*;

use futures::task;
use lazy_static::{initialize, lazy_static};
use ndarray::{Array3, Axis, Slice};
use rayon::prelude::*;
use tokio::{
    runtime::{Builder, Runtime},
    sync::mpsc::{self, Receiver, Sender},
    task::JoinHandle,
};

mod backend;
mod napi_utils;

use backend::*;
use napi_utils::*;

#[cfg(all(unix, not(target_env = "musl"), not(target_arch = "aarch64")))]
#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

#[cfg(all(windows, target_arch = "x86_64"))]
#[global_allocator]
static ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;

lazy_static! {
    static ref TM: RwLock<TrackManager> = RwLock::new(TrackManager::new());
    static ref RUNTIME: Runtime = Builder::new_multi_thread()
        .worker_threads(2)
        .thread_name("thesia-tokio")
        .build()
        .unwrap();
}

static mut IMG_MNGR_TX: Option<Sender<ImgMsg>> = None;
static mut IMG_RX: Option<Receiver<Images>> = None;

type Images = IdChMap<Vec<u8>>;
type ArcImgCaches = Arc<Mutex<IdChMap<Array3<u8>>>>;
type GuardImgCaches<'a> = MutexGuard<'a, IdChMap<Array3<u8>>>;

#[derive(Clone, PartialEq)]
struct DrawParams {
    sec: f64,
    width: u32,
    option: DrawOption,
    opt_for_wav: DrawOptionForWav,
    blend: f64,
}

impl Default for DrawParams {
    fn default() -> Self {
        DrawParams {
            sec: 0.,
            width: 1,
            option: DrawOption {
                px_per_sec: 0.,
                height: 1,
            },
            opt_for_wav: DrawOptionForWav {
                amp_range: (-1., 1.),
            },
            blend: 1.,
        }
    }
}

enum ImgMsg {
    Draw((IdChVec, DrawParams)),
    Remove(IdChVec),
}

#[derive(Default)]
struct CategorizedIdChVec {
    use_caches: IdChVec,
    need_parts: IdChVec,
    need_new_caches: IdChVec,
}

#[js_function(2)]
fn add_tracks(ctx: CallContext) -> JsResult<JsObject> {
    let new_track_ids: Vec<usize> = vec_usize_from(&ctx, 0)?;
    let new_paths: Vec<String> = vec_str_from(&ctx, 1)?;
    assert!(new_track_ids.len() > 0 && new_track_ids.len() == new_paths.len());

    let added_ids = TM
        .write()
        .unwrap()
        .add_tracks(&new_track_ids[..], new_paths);
    convert_vec_usize_to_jsarr(ctx.env, added_ids.iter(), added_ids.len())
}

#[js_function(1)]
fn reload_tracks(ctx: CallContext) -> JsResult<JsObject> {
    let track_ids: Vec<usize> = vec_usize_from(&ctx, 0)?;
    assert!(track_ids.len() > 0);

    let no_err_ids = TM.write().unwrap().reload_tracks(&track_ids[..]);
    convert_vec_usize_to_jsarr(ctx.env, no_err_ids.iter(), no_err_ids.len())
}

#[js_function(1)]
fn remove_tracks(ctx: CallContext) -> JsResult<JsUndefined> {
    let track_ids: Vec<usize> = vec_usize_from(&ctx, 0)?;
    assert!(track_ids.len() > 0);

    let mut tm = TM.write().unwrap();
    let img_mngr_tx = unsafe { IMG_MNGR_TX.clone().unwrap() };
    if let Err(e) = img_mngr_tx.blocking_send(ImgMsg::Remove(tm.id_ch_tuples_from(&track_ids[..])))
    {
        panic!("DRAW_TX error: {}", e);
    }
    tm.remove_tracks(&track_ids[..]);
    ctx.env.get_undefined()
}

#[contextless_function]
fn apply_track_list_changes(env: Env) -> ContextlessResult<JsObject> {
    let id_ch_tuples = {
        let mut tm = TM.write().unwrap();
        let updated_ids: Vec<usize> = tm.apply_track_list_changes().into_iter().collect();
        tm.id_ch_tuples_from(&updated_ids)
    };

    let img_mngr_tx = unsafe { IMG_MNGR_TX.clone().unwrap() };
    if let Err(e) = img_mngr_tx.blocking_send(ImgMsg::Remove(id_ch_tuples.clone())) {
        panic!("DRAW_TX error: {}", e);
    }
    convert_id_ch_vec_to_jsarr(&env, id_ch_tuples.iter(), id_ch_tuples.len()).map(|x| Some(x))
}

#[js_function(6)]
fn set_img_state(ctx: CallContext) -> JsResult<JsUndefined> {
    // let start = Instant::now();
    let id_ch_tuples = id_ch_tuples_from(&ctx, 0)?;
    let sec: f64 = ctx.get::<JsNumber>(1)?.try_into()?;
    let width: u32 = ctx.get::<JsNumber>(2)?.try_into()?;
    let option = draw_option_from_js_obj(ctx.get::<JsObject>(3)?)?;
    let opt_for_wav = draw_opt_for_wav_from_js_obj(ctx.get::<JsObject>(4)?)?;
    let blend: f64 = ctx.get::<JsNumber>(5)?.try_into()?;

    assert!(id_ch_tuples.len() > 0);
    assert!(width >= 1);
    assert!(option.px_per_sec.is_finite());
    assert!(option.px_per_sec >= 0.);
    assert!(option.height >= 1);
    assert!(opt_for_wav.amp_range.0 <= opt_for_wav.amp_range.1);

    let img_mngr_tx = unsafe { IMG_MNGR_TX.clone().unwrap() };
    if let Err(e) = img_mngr_tx.blocking_send(ImgMsg::Draw((
        id_ch_tuples,
        DrawParams {
            sec,
            width,
            option,
            opt_for_wav,
            blend,
        },
    ))) {
        panic!("DRAW_TX error: {}", e);
    }
    ctx.env.get_undefined()
}

#[contextless_function]
fn get_images(env: Env) -> ContextlessResult<JsObject> {
    let waker = task::noop_waker();
    let mut cx = Context::from_waker(&waker);

    let img_rx = unsafe { IMG_RX.as_mut().unwrap() };
    let mut opt_images: Option<Images> = None;
    while let Poll::Ready(Some(x)) = img_rx.poll_recv(&mut cx) {
        opt_images = Some(x);
    }

    let mut result = env.create_object()?;
    if let Some(images) = opt_images {
        for ((id, ch), im) in images.into_iter() {
            let name = format!("{}_{}", id, ch);
            let buf = env.create_buffer_with_data(im)?.into_raw();
            result.set_named_property(name.as_str(), buf)?;
        }
    }
    Ok(Some(result))
}

#[js_function(1)]
fn find_id_by_path(ctx: CallContext) -> JsResult<JsNumber> {
    let path = ctx.get::<JsString>(0)?.into_utf8()?;
    let tm = TM.read().unwrap();
    for (id, track) in tm.tracks.iter() {
        if track.is_path_same(path.as_str()?) {
            return ctx.env.create_int64(*id as i64);
        }
    }
    ctx.env.create_int64(-1)
}

#[js_function(3)]
fn get_overview(ctx: CallContext) -> JsResult<JsBuffer> {
    let id: usize = ctx.get::<JsNumber>(0)?.try_into_usize()?;
    let width: u32 = ctx.get::<JsNumber>(1)?.try_into()?;
    let height: u32 = ctx.get::<JsNumber>(2)?.try_into()?;
    assert!(width >= 1 && height >= 1);

    let tm = TM.read().unwrap();
    ctx.env
        .create_buffer_with_data(tm.get_overview_of(id, width, height))
        .map(|x| x.into_raw())
}

#[js_function(2)]
fn get_hz_at(ctx: CallContext) -> JsResult<JsNumber> {
    let y: u32 = ctx.get::<JsNumber>(0)?.try_into()?;
    let height: u32 = ctx.get::<JsNumber>(1)?.try_into()?;
    assert!(height >= 1 && y < height);

    let tm = TM.read().unwrap();
    ctx.env.create_double(tm.calc_hz_of(y, height) as f64)
}

#[js_function(1)]
fn get_freq_axis(ctx: CallContext) -> JsResult<JsObject> {
    let max_ticks: u32 = ctx.get::<JsNumber>(0)?.try_into()?;
    assert!(max_ticks >= 2);

    convert_vec_tup_f64_to_jsarr(ctx.env, TM.read().unwrap().get_freq_axis(max_ticks))
}

#[contextless_function]
fn get_max_db(env: Env) -> ContextlessResult<JsNumber> {
    env.create_double(TM.read().unwrap().max_db as f64)
        .map(Some)
}

#[contextless_function]
fn get_min_db(env: Env) -> ContextlessResult<JsNumber> {
    env.create_double(TM.read().unwrap().min_db as f64)
        .map(Some)
}

#[contextless_function]
fn get_max_sec(env: Env) -> ContextlessResult<JsNumber> {
    env.create_double(TM.read().unwrap().max_sec as f64)
        .map(Some)
}

#[js_function(1)]
fn get_n_ch(ctx: CallContext) -> JsResult<JsNumber> {
    let tm = TM.read().unwrap();
    let track = get_track!(ctx, 0, tm);
    ctx.env.create_uint32(track.n_ch() as u32)
}

#[js_function(1)]
fn get_sec(ctx: CallContext) -> JsResult<JsNumber> {
    let tm = TM.read().unwrap();
    let track = get_track!(ctx, 0, tm);
    ctx.env.create_double(track.sec())
}

#[js_function(1)]
fn get_sr(ctx: CallContext) -> JsResult<JsNumber> {
    let tm = TM.read().unwrap();
    let track = get_track!(ctx, 0, tm);
    ctx.env.create_uint32(track.sr)
}

#[js_function(1)]
fn get_sample_format(ctx: CallContext) -> JsResult<JsString> {
    let tm = TM.read().unwrap();
    let track = get_track!(ctx, 0, tm);
    ctx.env.create_string(&track.sample_format_str)
}

#[js_function(1)]
fn get_path(ctx: CallContext) -> JsResult<JsString> {
    let tm = TM.read().unwrap();
    let track = get_track!(ctx, 0, tm);
    ctx.env.create_string_from_std(track.path_string())
}

#[js_function(1)]
fn get_filename(ctx: CallContext) -> JsResult<JsString> {
    let id: usize = ctx.get::<JsNumber>(0)?.try_into_usize()?;
    let tm = TM.read().unwrap();
    ctx.env
        .create_string_from_std(tm.filenames.get(&id).unwrap().clone())
}

#[contextless_function]
fn get_colormap(env: Env) -> ContextlessResult<JsBuffer> {
    env.create_buffer_with_data(display::get_colormap_rgba())
        .map(|x| Some(x.into_raw()))
}

fn _crop_caches(
    images: &GuardImgCaches,
    id_ch_tuples: &IdChArr,
    sec: f64,
    width: u32,
    option: &DrawOption,
) -> (Images, IdChMap<(u32, u32)>) {
    // let start = Instant::now();
    let mut imgs = Images::new();
    let mut eff_l_w_map = IdChMap::new();
    let vec: Vec<_> = id_ch_tuples
        .par_iter()
        .filter_map(|tup| {
            let image = images.get(&tup)?;
            let total_width = image.len() / 4 / option.height as usize;
            let i_w = (sec * option.px_per_sec) as isize;
            let (i_w_eff, width_eff) = match calc_effective_w(i_w, width as usize, total_width) {
                Some((i, w)) => (i as isize, w as isize),
                None => {
                    return Some((
                        *tup,
                        (
                            vec![0u8; width as usize * option.height as usize * 4],
                            (0, 0),
                        ),
                    ));
                }
            };
            let slice = Slice::from(i_w_eff..i_w_eff + width_eff);
            let im_slice = image.slice_axis(Axis(1), slice);

            let pad_left = (-i_w.min(0)) as usize;
            let pad_right = width as usize - width_eff as usize - pad_left;
            if pad_left + pad_right == 0 {
                Some((*tup, (im_slice.to_owned().into_raw_vec(), (0, width))))
            } else {
                let arr = utils::pad(
                    im_slice,
                    (pad_left, pad_right),
                    Axis(1),
                    utils::PadMode::Constant(0),
                );
                Some((
                    *tup,
                    (arr.into_raw_vec(), (pad_left as u32, width_eff as u32)),
                ))
            }
        })
        .collect();
    eff_l_w_map.extend(
        vec.iter()
            .map(|(k, (_, eff_left_width))| (*k, *eff_left_width)),
    );
    imgs.extend(vec.into_iter().map(|(k, (img, _))| (k, img)));
    // println!("crop: {:?}", start.elapsed());
    (imgs, eff_l_w_map)
}

fn _categorize_id_ch(
    id_ch_tuples: IdChVec,
    total_widths: &IdChMap<u32>,
    spec_caches: &GuardImgCaches,
    wav_caches: &GuardImgCaches,
    option: &DrawOption,
    blend: f64,
) -> (CategorizedIdChVec, CategorizedIdChVec, IdChVec) {
    let categorize = |images: &GuardImgCaches| {
        if option.height <= display::MAX_SIZE {
            let mut result = CategorizedIdChVec::default();
            for tup in id_ch_tuples.iter() {
                let not_long_w = *total_widths.get(tup).unwrap() <= display::MAX_SIZE;
                match (images.contains_key(tup), not_long_w) {
                    (true, _) => result.use_caches.push(*tup),
                    (false, true) => {
                        result.need_parts.push(*tup);
                        result.need_new_caches.push(*tup);
                    }
                    (false, false) => {
                        result.need_parts.push(*tup);
                    }
                }
            }
            result
        } else {
            CategorizedIdChVec {
                use_caches: Vec::new(),
                need_parts: id_ch_tuples.to_owned(),
                need_new_caches: Vec::new(),
            }
        }
    };
    let cat_by_spec = if blend > 0. {
        categorize(spec_caches)
    } else {
        CategorizedIdChVec::default()
    };
    let mut cat_by_wav = if blend < 1. {
        categorize(wav_caches)
    } else {
        CategorizedIdChVec::default()
    };
    let mut need_wav_parts_only = Vec::new();
    {
        let (mut i, mut j) = (0, 0);
        while i < cat_by_spec.use_caches.len() && j < cat_by_wav.use_caches.len() {
            if cat_by_spec.use_caches[i] == cat_by_wav.use_caches[j] {
                i += 1;
                j += 1;
            } else {
                need_wav_parts_only.push(cat_by_spec.use_caches[i]);
                i += 1;
                let index = cat_by_wav
                    .need_parts
                    .iter()
                    .position(|x| *x == cat_by_spec.use_caches[i])
                    .unwrap();
                cat_by_wav.need_parts.remove(index);
            }
        }
    }
    assert!(
        cat_by_spec.need_parts.len() == 0
            || cat_by_wav.need_parts.len() == 0
            || cat_by_spec.need_parts.len() == cat_by_wav.need_parts.len()
    );
    (cat_by_spec, cat_by_wav, need_wav_parts_only)
}

async fn _draw_imgs(
    id_ch_tuples: IdChVec,
    params: Arc<RwLock<DrawParams>>,
    spec_caches: ArcImgCaches,
    wav_caches: ArcImgCaches,
    img_tx: Sender<Images>,
) {
    let params_backup = params.read().unwrap().clone();
    let DrawParams {
        sec,
        width,
        option,
        opt_for_wav,
        blend,
    } = params_backup;
    let blend_images = |spec_imgs: Images, wav_imgs: Images, eff_l_w_map: IdChMap<(u32, u32)>| {
        if blend == 1. {
            spec_imgs
        } else if blend == 0. {
            wav_imgs
        } else {
            spec_imgs
                .par_iter()
                .filter_map(|(k, spec_img)| {
                    let wav_img = wav_imgs.get(k)?;
                    let eff_l_w = eff_l_w_map.get(k).cloned().unwrap_or((0, 0));
                    let img =
                        display::blend(spec_img, wav_img, width, option.height, blend, eff_l_w);
                    Some((*k, img))
                })
                .collect()
        }
    };
    let (total_widths, cat_by_spec, cat_by_wav, blended_imgs) = {
        let tm = TM.read().unwrap();
        let id_ch_tuples: IdChVec = id_ch_tuples.into_iter().filter(|x| tm.exists(x)).collect();
        let mut total_widths = IdChMap::<u32>::with_capacity(id_ch_tuples.len());
        total_widths.extend(id_ch_tuples.iter().map(|&(id, ch)| {
            let width = tm.tracks.get(&id).unwrap().calc_width(option.px_per_sec);
            ((id, ch), width)
        }));

        let spec_caches_lock = spec_caches.lock().unwrap();
        let wav_caches_lock = wav_caches.lock().unwrap();
        let (cat_by_spec, cat_by_wav, need_wav_parts_only) = _categorize_id_ch(
            id_ch_tuples,
            &total_widths,
            &spec_caches_lock,
            &wav_caches_lock,
            &option,
            blend,
        );

        // crop image cache
        let (spec_imgs, eff_l_w_map) = if !cat_by_spec.use_caches.is_empty() {
            _crop_caches(
                &spec_caches_lock,
                &cat_by_spec.use_caches[..],
                sec,
                width,
                &option,
            )
        } else {
            (Images::new(), IdChMap::new())
        };
        let mut wav_imgs = if !cat_by_wav.use_caches.is_empty() {
            _crop_caches(
                &wav_caches_lock,
                &cat_by_wav.use_caches[..],
                sec,
                width,
                &option,
            )
            .0
        } else {
            Images::new()
        };
        if !need_wav_parts_only.is_empty() {
            wav_imgs.extend(tm.get_part_images(
                &need_wav_parts_only[..],
                sec,
                width,
                option,
                ImageKind::Wav(opt_for_wav),
                None,
            ));
        };
        (
            total_widths,
            cat_by_spec,
            cat_by_wav,
            blend_images(spec_imgs, wav_imgs, eff_l_w_map),
        )
    };
    if !blended_imgs.is_empty() {
        // println!("send cached images");
        img_tx.send(blended_imgs).await.unwrap();
    }
    if *params.read().unwrap() != params_backup {
        return;
    }

    // draw part
    let blended_imgs = {
        let tm = TM.read().unwrap();
        if !cat_by_spec.need_parts.is_empty() {
            let fast_resize_vec = if option.height <= display::MAX_SIZE {
                Some(
                    cat_by_spec
                        .need_parts
                        .iter()
                        .map(|tup| *total_widths.get(tup).unwrap() <= display::MAX_SIZE)
                        .collect(),
                )
            } else {
                None
            };
            tm.get_blended_part_images(
                &cat_by_spec.need_parts[..],
                sec,
                width,
                option,
                opt_for_wav,
                blend,
                fast_resize_vec,
            )
        } else {
            IdChMap::new()
        }
    };
    if !blended_imgs.is_empty() {
        // println!("send part images");
        img_tx.send(blended_imgs).await.unwrap();
    }
    if *params.read().unwrap() != params_backup {
        return;
    }

    let blended_imgs = {
        let tm = TM.read().unwrap();
        let mut spec_caches_lock = spec_caches.lock().unwrap();
        let mut wav_caches_lock = wav_caches.lock().unwrap();
        let (spec_imgs, eff_l_w_map) = if !cat_by_spec.need_new_caches.is_empty() {
            spec_caches_lock.extend(tm.get_entire_images(
                &cat_by_spec.need_new_caches[..],
                option,
                ImageKind::Spec,
            ));
            _crop_caches(
                &spec_caches_lock,
                &cat_by_spec.need_new_caches[..],
                sec,
                width,
                &option,
            )
        } else {
            (Images::new(), IdChMap::new())
        };
        let wav_imgs = if !cat_by_wav.need_new_caches.is_empty() {
            wav_caches_lock.extend(tm.get_entire_images(
                &cat_by_wav.need_new_caches[..],
                option,
                ImageKind::Wav(opt_for_wav),
            ));
            _crop_caches(
                &wav_caches_lock,
                &cat_by_wav.need_new_caches[..],
                sec,
                width,
                &option,
            )
            .0
        } else {
            IdChMap::new()
        };
        blend_images(spec_imgs, wav_imgs, eff_l_w_map)
    };
    if !blended_imgs.is_empty() {
        // println!("send new cached images");
        img_tx.send(blended_imgs).await.unwrap();
    }
}

async fn _manage_imgs(mut rx: Receiver<ImgMsg>, img_tx: Sender<Images>) {
    let spec_caches = Arc::new(Mutex::new(IdChMap::new()));
    let wav_caches = Arc::new(Mutex::new(IdChMap::new()));
    let prev_params = Arc::new(RwLock::new(DrawParams::default()));
    let mut task_handle: Option<JoinHandle<()>> = None;
    while let Some(msg) = rx.recv().await {
        match msg {
            ImgMsg::Draw((id_ch_tuples, draw_params)) => {
                {
                    let mut prev_params_write = prev_params.write().unwrap();
                    if let Some(prev_task) = task_handle.take() {
                        prev_task.abort();
                    }
                    // if draw_params != *prev_params_write {
                    //     let waker = task::noop_waker();
                    //     let mut cx = Context::from_waker(&waker);
                    //     let img_rx = unsafe { IMG_RX.as_mut().unwrap() };
                    //     while let Poll::Ready(Some(_)) = img_rx.poll_recv(&mut cx) {}
                    // }
                    if draw_params.option != prev_params_write.option {
                        spec_caches.lock().unwrap().clear();
                        wav_caches.lock().unwrap().clear();
                    } else if draw_params.opt_for_wav != prev_params_write.opt_for_wav {
                        wav_caches.lock().unwrap().clear();
                    }
                    *prev_params_write = draw_params;
                }
                task_handle = Some(RUNTIME.spawn(_draw_imgs(
                    id_ch_tuples,
                    Arc::clone(&prev_params),
                    Arc::clone(&spec_caches),
                    Arc::clone(&wav_caches),
                    img_tx.clone(),
                )));
            }
            ImgMsg::Remove(id_ch_tuples) => {
                if let Some(prev_task) = task_handle.take() {
                    prev_task.await.ok();
                }
                let mut spec_caches = spec_caches.lock().unwrap();
                let mut wav_caches = wav_caches.lock().unwrap();
                for tup in id_ch_tuples.iter() {
                    spec_caches.remove(tup);
                    wav_caches.remove(tup);
                }
            }
        }
    }
}

#[module_exports]
fn init(mut exports: JsObject) -> JsResult<()> {
    initialize(&TM);
    initialize(&RUNTIME);

    let (img_mngr_tx, img_mngr_rx) = mpsc::channel::<ImgMsg>(60);
    let (img_tx, img_rx) = mpsc::channel(60);
    unsafe {
        IMG_MNGR_TX = Some(img_mngr_tx);
        IMG_RX = Some(img_rx);
    }
    RUNTIME.spawn(_manage_imgs(img_mngr_rx, img_tx));

    exports.create_named_method("addTracks", add_tracks)?;
    exports.create_named_method("reloadTracks", reload_tracks)?;
    exports.create_named_method("removeTracks", remove_tracks)?;
    exports.create_named_method("applyTrackListChanges", apply_track_list_changes)?;
    exports.create_named_method("setImgState", set_img_state)?;
    exports.create_named_method("getImages", get_images)?;
    exports.create_named_method("findIDbyPath", find_id_by_path)?;
    exports.create_named_method("getOverview", get_overview)?;
    exports.create_named_method("getHzAt", get_hz_at)?;
    exports.create_named_method("getFreqAxis", get_freq_axis)?;
    exports.create_named_method("getMaxdB", get_max_db)?;
    exports.create_named_method("getMindB", get_min_db)?;
    exports.create_named_method("getMaxSec", get_max_sec)?;
    exports.create_named_method("getNumCh", get_n_ch)?;
    exports.create_named_method("getSec", get_sec)?;
    exports.create_named_method("getSr", get_sr)?;
    exports.create_named_method("getSampleFormat", get_sample_format)?;
    exports.create_named_method("getPath", get_path)?;
    exports.create_named_method("getFileName", get_filename)?;
    exports.create_named_method("getColormap", get_colormap)?;
    Ok(())
}
