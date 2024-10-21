use std::num::Wrapping;
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, OnceLock};
use std::task::{Context, Poll};

use approx::abs_diff_eq;
use napi::bindgen_prelude::spawn;
use napi::tokio::sync::{mpsc, RwLock};
use napi::tokio::task::JoinHandle;
use napi::tokio::{self, join};
use ndarray::prelude::*;
use num_traits::{AsPrimitive, Num, NumOps};
use rayon::prelude::*;

use crate::visualize::*;
use crate::{IdChArr, IdChDMap, IdChMap, IdChValueArr, IdChValueVec, IdChVec, Pad, TM, TRACK_LIST};

type Images = IdChValueVec<Vec<u8>>;
type ArcImgCaches = Arc<IdChDMap<Array3<u8>>>;

const MAX_IMG_CACHE_WIDTH: u32 = 16384;

static MSG_TX: OnceLock<mpsc::Sender<ImgMsg>> = OnceLock::new();
static mut IMG_RX: OnceLock<mpsc::Receiver<(Wrapping<usize>, Images)>> = OnceLock::new();

pub enum ImgMsg {
    Draw((IdChVec, DrawParams)),
    Remove(IdChVec),
}

#[derive(Default, Debug)]
struct CategorizedIdChVec {
    use_caches: IdChVec,
    need_parts: IdChVec,
    need_new_caches: IdChVec,
}

pub async fn send(msg: ImgMsg) {
    let img_mgr_tx = MSG_TX.get().unwrap().clone();
    if let Err(e) = img_mgr_tx.send(msg).await {
        panic!("DRAW_TX error: {}", e);
    }
}

trait SlightlyLarger {
    fn slightly_larger_than_or_equal_to(&self, other: Self) -> bool;
}

impl<T> SlightlyLarger for Wrapping<T>
where
    T: Copy + Num + Ord + num_traits::bounds::Bounded + 'static,
    usize: AsPrimitive<T>,
    Wrapping<T>: NumOps,
{
    fn slightly_larger_than_or_equal_to(&self, other: Self) -> bool {
        let other_big_skip = other + Wrapping(T::max_value() / 2.as_());
        let other_wrapped = other_big_skip < other;
        let self_big_skip = *self + Wrapping(T::max_value() / 2.as_());
        let self_wrapped = self_big_skip < *self;
        // assume self and other is not too far
        // 0--self--other-------------LIMIT  -> X
        // 0-------------self--other--LIMIT  -> X
        // 0--------self--other-------LIMIT  -> X
        // 0--other-------------self--LIMIT  -> X
        *self == other
        // 0--other--self-------------LIMIT
        // 0-------------other--self--LIMIT
        || (self_wrapped == other_wrapped) && other < *self
        // 0--------other--self-------LIMIT
        || self_wrapped && !other_wrapped && self_big_skip < other
        // 0--self-------------other--LIMIT
        || !self_wrapped && other_wrapped && *self < other_big_skip
    }
}

pub fn recv() -> Option<Images> {
    static RECENT_REQ_ID: AtomicUsize = AtomicUsize::new(0);
    let waker = futures::task::noop_waker();
    let mut cx = Context::from_waker(&waker);

    let img_rx = unsafe { IMG_RX.get_mut().unwrap() };
    let mut max_req_id = Wrapping(RECENT_REQ_ID.load(std::sync::atomic::Ordering::Acquire));
    let mut opt_images: Option<Images> = None;
    while let Poll::Ready(Some((curr_req_id, imgs))) = img_rx.poll_recv(&mut cx) {
        if curr_req_id.slightly_larger_than_or_equal_to(max_req_id) {
            max_req_id = curr_req_id;
            opt_images = Some(imgs);
        }
    }

    /* if opt_images.is_some() {
        println!("return req_id={}", max_req_id);
    } */

    RECENT_REQ_ID.store(max_req_id.0, std::sync::atomic::Ordering::Release);
    opt_images
}

/// Choose image caches and crop.
/// Returns tuple of cropped images and vec of (left padding length, effective width)
/// This function gets DrawParams but doesn't use DrawParams.blend
fn crop_caches(
    images: &ArcImgCaches,
    id_ch_tuples: &IdChArr,
    params: &DrawParams,
) -> (Images, IdChValueVec<LeftWidth>) {
    let width_usize = params.width as usize;
    let height_usize = params.height as usize;
    // let start = Instant::now();
    let i_w = (params.start_sec * params.px_per_sec).round() as isize;
    let pad_left = (-i_w.min(0)) as usize;
    let zipped: Vec<_> = id_ch_tuples
        .par_iter()
        .filter_map(|tup| images.get(tup).map(|image| (tup, image)))
        .map(|(tup, image)| {
            let total_width = image.len() / 4 / height_usize;
            let (i_w_eff, width_eff) = match calc_effective_slice(i_w, width_usize, total_width) {
                Some((i, w)) => (i as isize, w as isize),
                None => {
                    let zeros = vec![0u8; width_usize * height_usize * 4];
                    return ((*tup, zeros), (*tup, (0, 0)));
                }
            };
            let img_slice = image.slice(s![.., i_w_eff..i_w_eff + width_eff, ..]);

            let pad_right = width_usize - width_eff as usize - pad_left;
            if pad_left + pad_right == 0 {
                let (img_slice_vec, _) = img_slice.to_owned().into_raw_vec_and_offset();
                ((*tup, img_slice_vec), (*tup, (0, params.width)))
            } else {
                let img_pad = img_slice.pad((pad_left, pad_right), Axis(1), Default::default());
                let (img_pad_vec, _) = img_pad.into_raw_vec_and_offset();
                (
                    (*tup, img_pad_vec),
                    (*tup, (pad_left as u32, width_eff as u32)),
                )
            }
        })
        .collect();
    itertools::multiunzip(zipped)
    // println!("crop: {:?}", start.elapsed());
}

fn categorize_id_ch(
    id_ch_tuples: &IdChArr,
    total_widths: &IdChMap<u32>,
    spec_caches: &ArcImgCaches,
    wav_caches: &ArcImgCaches,
    blend: f64,
) -> (CategorizedIdChVec, CategorizedIdChVec, IdChVec) {
    let categorize = |images: &ArcImgCaches| {
        let mut result = CategorizedIdChVec::default();
        for &tup in id_ch_tuples {
            let not_long_w = *total_widths.get(&tup).unwrap() <= MAX_IMG_CACHE_WIDTH;
            // let not_long_w = true;
            match (images.contains_key(&tup), not_long_w) {
                (true, _) => result.use_caches.push(tup),
                (false, true) => {
                    result.need_parts.push(tup);
                    result.need_new_caches.push(tup);
                }
                (false, false) => {
                    result.need_parts.push(tup);
                }
            }
        }
        result
    };
    let (cat_by_spec, mut cat_by_wav) = rayon::join(
        || {
            if blend > 0. {
                categorize(spec_caches)
            } else {
                Default::default()
            }
        },
        || {
            if blend < 1. {
                categorize(wav_caches)
            } else {
                Default::default()
            }
        },
    );
    let mut need_wav_parts_only = Vec::new();
    {
        let (mut i, mut j) = (0, 0);
        while i < cat_by_spec.use_caches.len() {
            if j < cat_by_wav.use_caches.len()
                && cat_by_spec.use_caches[i] == cat_by_wav.use_caches[j]
            {
                i += 1;
                j += 1;
            } else {
                need_wav_parts_only.push(cat_by_spec.use_caches[i]);
                if let Some(index) = cat_by_wav
                    .need_parts
                    .iter()
                    .position(|x| *x == cat_by_spec.use_caches[i])
                {
                    cat_by_wav.need_parts.remove(index);
                }
                i += 1;
            }
        }
    }
    debug_assert!(
        cat_by_spec.need_parts.is_empty()
            || cat_by_wav.need_parts.is_empty()
            || cat_by_spec.need_parts.len() == cat_by_wav.need_parts.len()
    );
    (cat_by_spec, cat_by_wav, need_wav_parts_only)
}

#[inline]
fn blend_imgs(
    spec_imgs: Images,
    mut wav_imgs: Images,
    spec_eff_l_w_vec: &IdChValueArr<LeftWidth>,
    wav_eff_l_w_vec: &IdChValueArr<LeftWidth>,
    width: u32,
    height: u32,
    blend: f64,
) -> Images {
    if abs_diff_eq!(blend, 1.) {
        return spec_imgs;
    }
    if abs_diff_eq!(blend, 0.) {
        wav_imgs
            .par_iter_mut()
            .zip_eq(wav_eff_l_w_vec)
            .for_each(|((_, wav_img), (_, eff_l_w))| {
                let arr = ArrayViewMut3::from_shape((height as usize, width as usize, 4), wav_img)
                    .unwrap();
                let &(left, eff_width) = eff_l_w;
                make_opaque(arr, left, eff_width);
            });
        return wav_imgs;
    }
    spec_imgs
        .into_par_iter()
        .zip_eq(spec_eff_l_w_vec)
        .enumerate()
        .filter_map(|(i, (spec_kv, eff_l_w_kv))| {
            let (k, mut spec_img) = spec_kv;
            let &(_, eff_l_w) = eff_l_w_kv;
            let wav_img_opt = if i < wav_imgs.len() && wav_imgs[i].0 == k {
                Some(&wav_imgs[i].1)
            } else {
                // This rarely happens
                wav_imgs
                    .iter()
                    .find_map(|(wav_k, wav_img)| (*wav_k == k).then_some(wav_img))
            };
            wav_img_opt.map(|wav_img| {
                blend_img_to(&mut spec_img, wav_img, width, height, blend, eff_l_w);
                (k, spec_img)
            })
        })
        .collect()
}

async fn categorize_blend_caches(
    id_ch_tuples: IdChVec,
    params: &DrawParams,
    spec_caches: ArcImgCaches,
    wav_caches: ArcImgCaches,
) -> (IdChMap<u32>, CategorizedIdChVec, CategorizedIdChVec, Images) {
    let id_ch_tuples: IdChVec = {
        let tm = TM.read().await;
        id_ch_tuples.into_iter().filter(|x| tm.exists(x)).collect()
    };
    let total_widths: IdChMap<_> = {
        let tracklist = TRACK_LIST.read().await;
        id_ch_tuples
            .iter()
            .map(|&(id, ch)| {
                let width = tracklist
                    .get(id)
                    .map_or(0, |track| track.calc_width(params.px_per_sec));
                ((id, ch), width)
            })
            .collect()
    };

    let (cat_by_spec, cat_by_wav, need_wav_parts_only) = categorize_id_ch(
        &id_ch_tuples,
        &total_widths,
        &spec_caches,
        &wav_caches,
        params.blend,
    );

    // crop image cache
    let ((spec_imgs, spec_eff_l_w_vec), (mut wav_imgs, wav_eff_l_w_vec)) = rayon::join(
        || {
            if !cat_by_spec.use_caches.is_empty() {
                crop_caches(&spec_caches, &cat_by_spec.use_caches, params)
            } else {
                (Vec::new(), Vec::new())
            }
        },
        || {
            if !cat_by_wav.use_caches.is_empty() {
                crop_caches(&wav_caches, &cat_by_wav.use_caches, params)
            } else {
                (Vec::new(), Vec::new())
            }
        },
    );
    if !need_wav_parts_only.is_empty() {
        let params = DrawParams {
            blend: -1.,
            ..params.clone()
        };
        let (tracklist, tm) = join!(TRACK_LIST.read(), TM.read());
        wav_imgs.extend(tm.draw_part_imgs(&tracklist, &need_wav_parts_only, &params, None));
    };
    (
        total_widths,
        cat_by_spec,
        cat_by_wav,
        blend_imgs(
            spec_imgs,
            wav_imgs,
            &spec_eff_l_w_vec,
            &wav_eff_l_w_vec,
            params.width,
            params.height,
            params.blend,
        ),
    )
}

async fn draw_part_imgs(
    total_widths: &IdChMap<u32>,
    need_parts_spec: &IdChArr,
    need_parts_wav: &IdChArr,
    params: &DrawParams,
) -> Images {
    if need_parts_spec.is_empty() && need_parts_wav.is_empty() {
        return Vec::new();
    }
    let need_parts = if !need_parts_spec.is_empty() {
        need_parts_spec
    } else {
        need_parts_wav
    };
    let fast_resize_vec: Vec<_> = need_parts
        .iter()
        .map(|tup| *total_widths.get(tup).unwrap() <= MAX_IMG_CACHE_WIDTH)
        .collect();
    // let fast_resize_vec = vec![true; cat_by_spec.need_parts.len()];
    let (tracklist, tm) = join!(TRACK_LIST.read(), TM.read());
    tm.draw_part_imgs(&tracklist, need_parts, params, fast_resize_vec)
}

async fn draw_new_caches(
    spec_caches: ArcImgCaches,
    wav_caches: ArcImgCaches,
    need_new_spec_caches: IdChVec,
    need_new_wav_caches: IdChVec,
    params: &DrawParams,
) -> Images {
    let &DrawParams {
        width,
        height,
        px_per_sec,
        blend,
        ..
    } = params;

    // draw new caches
    let (new_spec_caches, new_wav_caches) = {
        let (tracklist, tm) = join!(TRACK_LIST.read(), TM.read());
        let new_spec_caches = tm.draw_entire_imgs(
            &tracklist,
            &need_new_spec_caches,
            height,
            px_per_sec,
            ImageKind::Spec,
        );
        let new_wav_caches = tm.draw_entire_imgs(
            &tracklist,
            &need_new_wav_caches,
            height,
            px_per_sec,
            ImageKind::Wav(&params.opt_for_wav),
        );
        (new_spec_caches, new_wav_caches)
    };

    new_spec_caches.into_par_iter().for_each(|(k, v)| {
        spec_caches.insert(k, v);
    });
    new_wav_caches.into_par_iter().for_each(|(k, v)| {
        wav_caches.insert(k, v);
    });

    // blend new caches (and existing caches if needed)
    let id_ch_vec_for_blend = {
        let mut vec = need_new_spec_caches;
        for id_ch in need_new_wav_caches.into_iter() {
            if !vec.contains(&id_ch) {
                vec.push(id_ch);
            }
        }
        vec
    };
    if id_ch_vec_for_blend.is_empty() {
        return Images::new();
    }
    let (spec_imgs, spec_eff_l_w_vec) = crop_caches(&spec_caches, &id_ch_vec_for_blend, params);
    let id_ch_vec_for_blend: IdChVec = spec_imgs.iter().map(|(id_ch, _)| *id_ch).collect();
    if id_ch_vec_for_blend.is_empty() {
        return Images::new();
    }
    let (wav_imgs, wav_eff_l_w_vec) = crop_caches(&wav_caches, &id_ch_vec_for_blend, params);
    blend_imgs(
        spec_imgs,
        wav_imgs,
        &spec_eff_l_w_vec,
        &wav_eff_l_w_vec,
        width,
        height,
        blend,
    )
}

async fn draw_imgs(
    id_ch_tuples: IdChVec,
    params: Arc<RwLock<DrawParams>>,
    spec_caches: ArcImgCaches,
    wav_caches: ArcImgCaches,
    img_tx: mpsc::Sender<(Wrapping<usize>, Images)>,
    req_id: Wrapping<usize>,
) {
    let params_backup = params.read().await.clone();
    let (total_widths, cat_by_spec, cat_by_wav, blended_imgs) = categorize_blend_caches(
        id_ch_tuples,
        &params_backup,
        Arc::clone(&spec_caches),
        Arc::clone(&wav_caches),
    )
    .await;
    if !blended_imgs.is_empty() {
        /* println!(
            "[cached] req_id: {}, blend: {}",
            req_id, params_backup.blend
        ); */
        img_tx.send((req_id, blended_imgs)).await.unwrap();
    }
    if *params.read().await != params_backup {
        return;
    }

    // draw part
    let blended_imgs = draw_part_imgs(
        &total_widths,
        &cat_by_spec.need_parts,
        &cat_by_wav.need_parts,
        &params_backup,
    )
    .await;
    if !blended_imgs.is_empty() {
        // println!("[part] req_id: {}, blend: {}", req_id, params_backup.blend);
        img_tx.send((req_id, blended_imgs)).await.unwrap();
    }
    if *params.read().await != params_backup {
        return;
    }
    tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;

    let blended_imgs = draw_new_caches(
        Arc::clone(&spec_caches),
        Arc::clone(&wav_caches),
        cat_by_spec.need_new_caches,
        cat_by_wav.need_new_caches,
        &params_backup,
    )
    .await;
    if !blended_imgs.is_empty() {
        /* println!(
            "[new cache] req_id: {}, blend: {}",
            req_id, params_backup.blend
        ) */
        img_tx.send((req_id, blended_imgs)).await.unwrap();
    }
}

async fn take_abort_await(task_handle: &mut Option<JoinHandle<()>>) {
    if let Some(prev_task) = task_handle.take() {
        prev_task.abort();
        prev_task.await.ok();
    }
}

async fn main_loop(
    mut msg_rx: mpsc::Receiver<ImgMsg>,
    img_tx: mpsc::Sender<(Wrapping<usize>, Images)>,
) {
    let spec_caches = Arc::new(IdChDMap::default());
    let wav_caches = Arc::new(IdChDMap::default());
    let prev_params = Arc::new(RwLock::new(DrawParams::default()));
    let mut req_id = Wrapping(0);
    let mut task_handle: Option<JoinHandle<()>> = None;
    while let Some(msg) = msg_rx.recv().await {
        match msg {
            ImgMsg::Draw((id_ch_tuples, draw_params)) => {
                {
                    let mut prev_params_write = prev_params.write().await;
                    take_abort_await(&mut task_handle).await;
                    if draw_params.is_params_for_different_img_cache(&prev_params_write) {
                        spec_caches.clear();
                        wav_caches.clear();
                    } else if draw_params.is_params_for_different_wav_cache(&prev_params_write) {
                        wav_caches.clear();
                    }
                    *prev_params_write = draw_params;
                }
                task_handle = Some(spawn(draw_imgs(
                    id_ch_tuples,
                    Arc::clone(&prev_params),
                    Arc::clone(&spec_caches),
                    Arc::clone(&wav_caches),
                    img_tx.clone(),
                    req_id,
                )));
                req_id += 1;
            }
            ImgMsg::Remove(id_ch_tuples) => {
                take_abort_await(&mut task_handle).await;
                id_ch_tuples.par_iter().for_each(|tup| {
                    spec_caches.remove(tup);
                    wav_caches.remove(tup);
                });
            }
        }
    }
}

pub fn spawn_task() {
    if MSG_TX.get().is_some() && unsafe { IMG_RX.get().is_some() } {
        return;
    }

    let (msg_tx, msg_rx) = mpsc::channel(70);
    let (img_tx, img_rx) = mpsc::channel(70);
    MSG_TX.set(msg_tx).unwrap();
    unsafe {
        IMG_RX.set(img_rx).unwrap();
    }
    spawn(main_loop(msg_rx, img_tx));
}
