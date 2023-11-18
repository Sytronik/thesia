#![allow(dead_code)]

use std::collections::HashMap;
use std::ops::Deref;
use std::sync::Arc;

use lazy_static::{initialize, lazy_static};

use napi::bindgen_prelude::*;
use napi_derive::napi;
use serde_json::json;
use tokio::sync::RwLock;

#[warn(dead_code)]
mod backend;
#[warn(dead_code)]
mod img_mgr;

use backend::*;
use img_mgr::{DrawParams, ImgMsg};

#[cfg(all(
    any(windows, unix),
    target_arch = "x86_64",
    not(target_env = "musl"),
    not(debug_assertions)
))]
#[global_allocator]
static ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;

lazy_static! {
    static ref TM: Arc<RwLock<TrackManager>> = Arc::new(RwLock::new(TrackManager::new()));
}

#[napi]
fn init() {
    initialize(&TM);
    img_mgr::spawn_runtime();
}

#[napi]
async fn add_tracks(id_list: Vec<u32>, path_list: Vec<String>) -> Vec<u32> {
    // let id_list: Vec<usize> = vec_usize_from(&ctx, 0)?;
    // let path_list: Vec<String> = vec_str_from(&ctx, 1)?;
    assert!(!id_list.is_empty() && id_list.len() == path_list.len());

    let added_ids = TM
        .write()
        .await
        .add_tracks(id_list.into_iter().map(|x| x as usize).collect(), path_list);
    // convert_usize_arr_to_jsarr(ctx.env, &added_ids)
    added_ids.into_iter().map(|x| x as u32).collect()
}

#[napi]
async fn reload_tracks(track_ids: Vec<u32>) -> Vec<u32> {
    assert!(!track_ids.is_empty());

    let track_ids: Vec<_> = track_ids.into_iter().map(|x| x as usize).collect();
    let no_err_ids = TM.write().await.reload_tracks(&track_ids);
    no_err_ids.into_iter().map(|x| x as u32).collect()
}

#[napi]
async fn remove_tracks(track_ids: Vec<u32>) {
    assert!(!track_ids.is_empty());

    let track_ids: Vec<_> = track_ids.into_iter().map(|x| x as usize).collect();
    let mut tm = TM.write().await;
    tokio::spawn(img_mgr::send(ImgMsg::Remove(
        tm.id_ch_tuples_from(&track_ids),
    )));
    tm.remove_tracks(&track_ids);
}

#[napi]
async fn apply_track_list_changes() -> Vec<String> {
    let id_ch_tuples = {
        let mut tm = TM.write().await;
        let updated_ids: Vec<usize> = tm.apply_track_list_changes().into_iter().collect();
        tm.id_ch_tuples_from(&updated_ids)
    };
    let id_ch_strs = id_ch_tuples
        .iter()
        .map(|&(id, ch)| format_id_ch(id, ch))
        .collect();
    tokio::spawn(img_mgr::send(ImgMsg::Remove(id_ch_tuples)));
    id_ch_strs
}

#[napi]
async fn set_image_state(
    id_ch_strs: Vec<String>,
    start_sec: f64,
    width: u32,
    option: DrawOption,
    opt_for_wav: serde_json::Value,
    blend: f64,
) -> Result<()> {
    // let start = Instant::now();
    let opt_for_wav: DrawOptionForWav = serde_json::from_value(opt_for_wav)?;
    assert!(!id_ch_strs.is_empty());
    assert!(width >= 1);
    assert!(option.px_per_sec.is_finite());
    assert!(option.px_per_sec >= 0.);
    assert!(option.height >= 1);
    assert!(opt_for_wav.amp_range.0 <= opt_for_wav.amp_range.1);

    let id_ch_tuples = {
        let tm = TM.read().await;
        parse_id_ch_tuples(id_ch_strs)?
            .into_iter()
            .filter(|id_ch| tm.exists(id_ch))
            .collect()
    };
    tokio::spawn(img_mgr::send(ImgMsg::Draw((
        id_ch_tuples,
        DrawParams::new(start_sec, width, option, opt_for_wav, blend),
    ))));
    Ok(())
}

#[napi(js_name = "getdBRange")]
#[allow(non_snake_case)]
fn get_dB_range() -> f64 {
    TM.blocking_read().dB_range as f64
}

#[napi(js_name = "setdBRange")]
#[allow(non_snake_case)]
fn set_dB_range(dB_range: f64) {
    assert!(dB_range > 0.);
    let mut tm = TM.blocking_write();
    tm.set_dB_range(dB_range as f32);
    remove_all_imgs(tm);
}

#[napi]
fn get_spec_setting() -> SpecSetting {
    TM.blocking_read().setting.clone()
}

#[napi]
async fn set_spec_setting(spec_setting: SpecSetting) {
    assert!(spec_setting.win_ms > 0.);
    assert!(spec_setting.t_overlap >= 1);
    assert!(spec_setting.f_overlap >= 1);
    let mut tm = TM.write().await;
    tm.set_setting(spec_setting);
    remove_all_imgs(tm);
}

#[napi]
fn get_common_guard_clipping() -> dynamics::GuardClippingMode {
    TM.blocking_read().common_guard_clipping()
}

#[napi]
async fn set_common_guard_clipping(mode: dynamics::GuardClippingMode) {
    let mut tm = TM.write().await;
    tm.set_common_guard_clipping(mode);
    remove_all_imgs(tm);
}

#[napi]
fn get_common_normalize() -> serde_json::Value {
    serde_json::to_value(TM.blocking_read().common_normalize()).unwrap()
}

#[napi]
async fn set_common_normalize(target: serde_json::Value) -> Result<()> {
    let mut tm = TM.write().await;
    let target = serde_json::from_value(target)?;
    tm.set_common_normalize(target);
    remove_all_imgs(tm);
    Ok(())
}

#[napi]
fn get_images() -> HashMap<String, Buffer> {
    if let Some(images) = img_mgr::recv() {
        images
            .into_iter()
            .map(|((id, ch), img)| (format_id_ch(id, ch), img.into()))
            .collect()
    } else {
        HashMap::new()
    }
}

#[napi]
async fn find_id_by_path(path: String) -> i32 {
    TM.read()
        .await
        .tracklist
        .find_id_by_path(&path)
        .map_or(-1, |id| id as i32)
}

#[napi]
async fn get_overview(track_id: u32, width: u32, height: u32, dpr: f64) -> Buffer {
    assert!(width >= 1 && height >= 1);

    TM.read()
        .await
        .draw_overview(track_id as usize, width, height, dpr as f32)
        .into()
}

#[napi]
async fn get_hz_at(y: u32, height: u32) -> f64 {
    assert!(height >= 1 && y <= height);

    TM.read().await.calc_hz_of(y, height) as f64
}

#[napi]
async fn get_time_axis_markers(
    start_sec: f64,
    end_sec: f64,
    tick_unit: f64,
    label_interval: u32,
) -> serde_json::Value {
    assert!(start_sec <= end_sec);
    assert!(label_interval > 0);
    json!(&TM
        .read()
        .await
        .time_axis_markers(start_sec, end_sec, tick_unit, label_interval))
}

#[napi]
async fn get_freq_axis_markers(max_num_ticks: u32, max_num_labels: u32) -> serde_json::Value {
    assert_axis_params(max_num_ticks, max_num_labels);

    json!(TM
        .read()
        .await
        .freq_axis_markers(max_num_ticks, max_num_labels))
}

#[napi]
async fn get_amp_axis_markers(
    max_num_ticks: u32,
    max_num_labels: u32,
    amp_range: (f64, f64),
) -> Result<serde_json::Value> {
    assert_axis_params(max_num_ticks, max_num_labels);
    assert!(amp_range.0 < amp_range.1);

    Ok(json!(TrackManager::amp_axis_markers(
        max_num_ticks,
        max_num_labels,
        (amp_range.0 as f32, amp_range.1 as f32),
    )))
}

#[napi(js_name = "getdBAxisMarkers")]
#[allow(non_snake_case)]
async fn get_dB_axis_markers(max_num_ticks: u32, max_num_labels: u32) -> serde_json::Value {
    assert_axis_params(max_num_ticks, max_num_labels);

    json!(TM
        .read()
        .await
        .dB_axis_markers(max_num_ticks, max_num_labels))
}

#[napi(js_name = "getMaxdB")]
#[allow(non_snake_case)]
fn get_max_dB() -> f64 {
    TM.blocking_read().max_dB as f64
}

#[napi(js_name = "getMindB")]
#[allow(non_snake_case)]
fn get_min_dB() -> f64 {
    TM.blocking_read().min_dB as f64
}

#[napi]
fn get_longest_track_length_sec() -> f64 {
    TM.blocking_read().tracklist.max_sec
}

#[napi]
fn get_channel_counts(track_id: u32) -> u32 {
    TM.blocking_read()
        .track(track_id as usize)
        .map_or(0, |track| track.n_ch() as u32)
}

#[napi]
fn get_length_sec(track_id: u32) -> f64 {
    TM.blocking_read()
        .track(track_id as usize)
        .map_or(0., |track| track.sec())
}

#[napi]
fn get_sample_rate(track_id: u32) -> u32 {
    TM.blocking_read()
        .track(track_id as usize)
        .map_or(0, |track| track.sr())
}

#[napi]
fn get_sample_format(track_id: u32) -> String {
    TM.blocking_read()
        .track(track_id as usize)
        .map_or_else(String::new, |track| track.format_desc.to_owned())
}

#[napi(js_name = "getGlobalLUFS")]
fn get_global_lufs(track_id: u32) -> f64 {
    TM.blocking_read()
        .track(track_id as usize)
        .map_or(f64::NEG_INFINITY, |track| track.stats().global_lufs)
}

#[napi(js_name = "getRMSdB")]
#[allow(non_snake_case)]
fn get_rms_dB(track_id: u32) -> f64 {
    TM.blocking_read()
        .track(track_id as usize)
        .map_or(f64::NEG_INFINITY, |track| track.stats().rms_dB as f64)
}

#[napi(js_name = "getMaxPeakdB")]
#[allow(non_snake_case)]
fn get_max_peak_dB(track_id: u32) -> f64 {
    TM.blocking_read()
        .track(track_id as usize)
        .map_or(f64::NEG_INFINITY, |track| track.stats().max_peak_dB as f64)
}

#[napi]
fn get_path(track_id: u32) -> String {
    TM.blocking_read()
        .track(track_id as usize)
        .map_or_else(String::new, |track| track.path_string())
}

#[napi]
fn get_file_name(track_id: u32) -> String {
    TM.blocking_read()
        .tracklist
        .filename(track_id as usize)
        .to_owned()
}

#[napi]
fn get_color_map() -> Buffer {
    visualize::get_colormap_rgb().into()
}

#[inline]
pub fn assert_axis_params(max_num_ticks: u32, max_num_labels: u32) {
    assert!(max_num_ticks >= 2);
    assert!(max_num_labels >= 2);
    assert!(max_num_ticks >= max_num_labels);
}

#[inline]
fn format_id_ch(id: usize, ch: usize) -> String {
    format!("{}_{}", id, ch)
}

pub fn parse_id_ch_tuples(id_ch_strs: Vec<String>) -> Result<IdChVec> {
    let mut result = IdChVec::with_capacity(id_ch_strs.len());
    for s in id_ch_strs {
        let mut iter = s.split('_').map(|x| x.parse::<usize>());
        match (iter.next(), iter.next()) {
            (Some(Ok(id)), Some(Ok(ch))) => {
                result.push((id, ch));
            }
            _ => {
                return Err(Error::new(
                    Status::Unknown,
                    "The array element should be \"{unsigned_int}_{unsigned_int}\".",
                ));
            }
        }
    }
    Ok(result)
}

#[inline]
fn remove_all_imgs(tm: impl Deref<Target = TrackManager>) {
    tokio::spawn(img_mgr::send(ImgMsg::Remove(tm.id_ch_tuples())));
}
