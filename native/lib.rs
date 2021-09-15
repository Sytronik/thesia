use std::convert::TryInto;
use std::sync::Arc;

use lazy_static::{initialize, lazy_static};
use napi::{
    CallContext, ContextlessResult, Env, JsBuffer, JsNumber, JsObject, JsString, JsUndefined,
    Result as JsResult,
};
use napi_derive::*;
use parking_lot::RwLock;

mod backend;
mod img_mgr;
mod napi_utils;

use backend::*;
use img_mgr::{DrawParams, ImgMsg};
use napi_utils::*;

#[cfg(all(unix, not(target_env = "musl"), not(target_arch = "aarch64")))]
#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

#[cfg(all(windows, target_arch = "x86_64"))]
#[global_allocator]
static ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;

lazy_static! {
    static ref TM: Arc<RwLock<TrackManager>> = Arc::new(RwLock::new(TrackManager::new()));
}

#[js_function(2)]
fn add_tracks(ctx: CallContext) -> JsResult<JsObject> {
    let id_list: Vec<usize> = vec_usize_from(&ctx, 0)?;
    let path_list: Vec<String> = vec_str_from(&ctx, 1)?;
    assert!(!id_list.is_empty() && id_list.len() == path_list.len());

    let added_ids = TM.write().add_tracks(id_list, path_list);
    convert_usize_arr_to_jsarr(ctx.env, &added_ids)
}

#[js_function(1)]
fn reload_tracks(ctx: CallContext) -> JsResult<JsObject> {
    let track_ids: Vec<usize> = vec_usize_from(&ctx, 0)?;
    assert!(!track_ids.is_empty());

    let no_err_ids = TM.write().reload_tracks(&track_ids);
    convert_usize_arr_to_jsarr(ctx.env, &no_err_ids)
}

#[js_function(1)]
fn remove_tracks(ctx: CallContext) -> JsResult<JsUndefined> {
    let track_ids: Vec<usize> = vec_usize_from(&ctx, 0)?;
    assert!(!track_ids.is_empty());

    let mut tm = TM.write();
    img_mgr::send(ImgMsg::Remove(tm.id_ch_tuples_from(&track_ids)));
    tm.remove_tracks(&track_ids);
    ctx.env.get_undefined()
}

#[contextless_function]
fn apply_track_list_changes(env: Env) -> ContextlessResult<JsObject> {
    let id_ch_tuples = {
        let mut tm = TM.write();
        let updated_ids: Vec<usize> = tm.apply_track_list_changes().into_iter().collect();
        tm.id_ch_tuples_from(&updated_ids)
    };

    img_mgr::send(ImgMsg::Remove(id_ch_tuples.clone()));
    convert_id_ch_arr_to_jsarr(&env, &id_ch_tuples).map(Some)
}

#[js_function(6)]
fn set_img_state(ctx: CallContext) -> JsResult<JsUndefined> {
    // let start = Instant::now();
    let id_ch_tuples = id_ch_tuples_from(&ctx, 0)?;
    let start_sec: f64 = ctx.get::<JsNumber>(1)?.try_into()?;
    let width: u32 = ctx.get::<JsNumber>(2)?.try_into()?;
    let option = draw_option_from_js_obj(ctx.get::<JsObject>(3)?)?;
    let opt_for_wav = draw_opt_for_wav_from_js_obj(ctx.get::<JsObject>(4)?)?;
    let blend: f64 = ctx.get::<JsNumber>(5)?.try_into()?;

    assert!(!id_ch_tuples.is_empty());
    assert!(width >= 1);
    assert!(option.px_per_sec.is_finite());
    assert!(option.px_per_sec >= 0.);
    assert!(1 <= option.height && option.height < display::MAX_SIZE);
    assert!(opt_for_wav.amp_range.0 <= opt_for_wav.amp_range.1);

    img_mgr::send(ImgMsg::Draw((
        id_ch_tuples,
        DrawParams::new(start_sec, width, option, opt_for_wav, blend),
    )));
    ctx.env.get_undefined()
}

#[contextless_function]
fn get_images(env: Env) -> ContextlessResult<JsObject> {
    let mut result = env.create_object()?;
    if let Some(images) = img_mgr::recv() {
        for ((id, ch), im) in images {
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
    let tm = TM.read();
    for (id, track) in indexed_iter_filtered!(tm.tracks) {
        if track.is_path_same(path.as_str()?) {
            return ctx.env.create_int64(id as i64);
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

    ctx.env
        .create_buffer_with_data(TM.read().draw_overview(id, width, height))
        .map(|x| x.into_raw())
}

#[js_function(2)]
fn get_hz_at(ctx: CallContext) -> JsResult<JsNumber> {
    let y: u32 = ctx.get::<JsNumber>(0)?.try_into()?;
    let height: u32 = ctx.get::<JsNumber>(1)?.try_into()?;
    assert!(height >= 1 && y < height);

    ctx.env
        .create_double(TM.read().calc_hz_of(y, height) as f64)
}

#[js_function(5)]
fn get_time_axis(ctx: CallContext) -> JsResult<JsObject> {
    let width: u32 = ctx.get::<JsNumber>(0)?.try_into()?;
    let start_sec: f64 = ctx.get::<JsNumber>(1)?.try_into()?;
    let px_per_sec: f64 = ctx.get::<JsNumber>(2)?.try_into()?;
    let tick_unit: f64 = ctx.get::<JsNumber>(3)?.try_into()?;
    let label_interval: u32 = ctx.get::<JsNumber>(4)?.try_into()?;
    assert!(width >= 1);
    assert!(px_per_sec.is_finite());
    assert!(px_per_sec >= 0.);
    assert!(label_interval > 0);
    convert_axis_to_jsarr(
        ctx.env,
        TM.read()
            .create_time_axis(width, start_sec, px_per_sec, tick_unit, label_interval),
    )
}

#[js_function(3)]
fn get_freq_axis(ctx: CallContext) -> JsResult<JsObject> {
    let height: u32 = ctx.get::<JsNumber>(0)?.try_into()?;
    let max_num_ticks: u32 = ctx.get::<JsNumber>(1)?.try_into()?;
    let max_num_labels: u32 = ctx.get::<JsNumber>(2)?.try_into()?;
    assert_axis_params(height, max_num_ticks, max_num_labels);

    convert_axis_to_jsarr(
        ctx.env,
        TM.read()
            .create_freq_axis(height, max_num_ticks, max_num_labels),
    )
}

#[js_function(4)]
fn get_amp_axis(ctx: CallContext) -> JsResult<JsObject> {
    let height: u32 = ctx.get::<JsNumber>(0)?.try_into()?;
    let max_num_ticks: u32 = ctx.get::<JsNumber>(1)?.try_into()?;
    let max_num_labels: u32 = ctx.get::<JsNumber>(2)?.try_into()?;
    let opt_for_wav = draw_opt_for_wav_from_js_obj(ctx.get::<JsObject>(3)?)?;
    assert_axis_params(height, max_num_ticks, max_num_labels);
    assert!(opt_for_wav.amp_range.0 <= opt_for_wav.amp_range.1);

    convert_axis_to_jsarr(
        ctx.env,
        TrackManager::create_amp_axis(height, max_num_ticks, max_num_labels, opt_for_wav.amp_range),
    )
}

#[js_function(3)]
fn get_db_axis(ctx: CallContext) -> JsResult<JsObject> {
    let height: u32 = ctx.get::<JsNumber>(0)?.try_into()?;
    let max_num_ticks: u32 = ctx.get::<JsNumber>(1)?.try_into()?;
    let max_num_labels: u32 = ctx.get::<JsNumber>(2)?.try_into()?;
    assert_axis_params(height, max_num_ticks, max_num_labels);

    convert_axis_to_jsarr(
        ctx.env,
        TM.read()
            .create_db_axis(height, max_num_ticks, max_num_labels),
    )
}

#[contextless_function]
fn get_max_db(env: Env) -> ContextlessResult<JsNumber> {
    env.create_double(TM.read().max_db as f64).map(Some)
}

#[contextless_function]
fn get_min_db(env: Env) -> ContextlessResult<JsNumber> {
    env.create_double(TM.read().min_db as f64).map(Some)
}

#[contextless_function]
fn get_max_sec(env: Env) -> ContextlessResult<JsNumber> {
    env.create_double(TM.read().max_sec as f64).map(Some)
}

#[js_function(1)]
fn get_n_ch(ctx: CallContext) -> JsResult<JsNumber> {
    let tm = TM.read();
    let track = get_track!(ctx, 0, tm);
    ctx.env.create_uint32(track.n_ch() as u32)
}

#[js_function(1)]
fn get_sec(ctx: CallContext) -> JsResult<JsNumber> {
    let tm = TM.read();
    let track = get_track!(ctx, 0, tm);
    ctx.env.create_double(track.sec())
}

#[js_function(1)]
fn get_sr(ctx: CallContext) -> JsResult<JsNumber> {
    let tm = TM.read();
    let track = get_track!(ctx, 0, tm);
    ctx.env.create_uint32(track.sr)
}

#[js_function(1)]
fn get_sample_format(ctx: CallContext) -> JsResult<JsString> {
    let tm = TM.read();
    let track = get_track!(ctx, 0, tm);
    ctx.env.create_string(&track.sample_format_str)
}

#[js_function(1)]
fn get_path(ctx: CallContext) -> JsResult<JsString> {
    let tm = TM.read();
    let track = get_track!(ctx, 0, tm);
    ctx.env.create_string_from_std(track.path_string())
}

#[js_function(1)]
fn get_filename(ctx: CallContext) -> JsResult<JsString> {
    let id: usize = ctx.get::<JsNumber>(0)?.try_into_usize()?;
    ctx.env
        .create_string_from_std(TM.read().filenames[id].to_owned().unwrap())
}

#[contextless_function]
fn get_colormap(env: Env) -> ContextlessResult<JsBuffer> {
    env.create_buffer_with_data(display::get_colormap_rgb())
        .map(|x| Some(x.into_raw()))
}

#[module_exports]
fn init(mut exports: JsObject) -> JsResult<()> {
    initialize(&TM);
    img_mgr::spawn_runtime();

    exports.create_named_method("addTracks", add_tracks)?;
    exports.create_named_method("reloadTracks", reload_tracks)?;
    exports.create_named_method("removeTracks", remove_tracks)?;
    exports.create_named_method("applyTrackListChanges", apply_track_list_changes)?;
    exports.create_named_method("setImgState", set_img_state)?;
    exports.create_named_method("getImages", get_images)?;
    exports.create_named_method("findIDbyPath", find_id_by_path)?;
    exports.create_named_method("getOverview", get_overview)?;
    exports.create_named_method("getHzAt", get_hz_at)?;
    exports.create_named_method("getTimeAxis", get_time_axis)?;
    exports.create_named_method("getFreqAxis", get_freq_axis)?;
    exports.create_named_method("getAmpAxis", get_amp_axis)?;
    exports.create_named_method("getdBAxis", get_db_axis)?;
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
