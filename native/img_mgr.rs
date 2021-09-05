use std::sync::Arc;
use std::task::{Context, Poll};

use approx::abs_diff_eq;
use futures::task;
use lazy_static::{initialize, lazy_static};
use ndarray::{Array3, Axis, Slice};
use parking_lot::{Mutex, MutexGuard, RwLock};
use rayon::prelude::*;
use tokio::{
    runtime::{Builder, Runtime},
    sync::mpsc::{self, Receiver, Sender},
    task::JoinHandle,
};

use crate::backend::{
    display::{self, calc_effective_slice},
    utils::{Pad, PadMode},
    DrawOption, DrawOptionForWav, IdChArr, IdChMap, IdChVec, ImageKind,
};
use crate::TM;

type Images = IdChMap<Vec<u8>>;
type ArcImgCaches = Arc<Mutex<IdChMap<Array3<u8>>>>;
type GuardImgCaches<'a> = MutexGuard<'a, IdChMap<Array3<u8>>>;

const MAX_IMG_CACHE_WIDTH: u32 = 2 * display::LARGE_WIDTH_SPLIT_HOP as u32;

lazy_static! {
    static ref RUNTIME: Runtime = Builder::new_multi_thread()
        .worker_threads(2)
        .thread_name("thesia-tokio")
        .build()
        .unwrap();
}

static mut MSG_TX: Option<Sender<ImgMsg>> = None;
static mut IMG_RX: Option<Receiver<Images>> = None;

#[derive(Clone, PartialEq)]
pub struct DrawParams {
    start_sec: f64,
    width: u32,
    option: DrawOption,
    opt_for_wav: DrawOptionForWav,
    blend: f64,
}

impl DrawParams {
    pub fn new(
        start_sec: f64,
        width: u32,
        option: DrawOption,
        opt_for_wav: DrawOptionForWav,
        blend: f64,
    ) -> Self {
        DrawParams {
            start_sec,
            width,
            option,
            opt_for_wav,
            blend,
        }
    }
}

impl Default for DrawParams {
    fn default() -> Self {
        DrawParams {
            start_sec: 0.,
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

pub enum ImgMsg {
    Draw((IdChVec, DrawParams)),
    Remove(IdChVec),
}

#[derive(Default)]
struct CategorizedIdChVec {
    use_caches: IdChVec,
    need_parts: IdChVec,
    need_new_caches: IdChVec,
}

pub fn send(msg: ImgMsg) {
    let img_mngr_tx = unsafe { MSG_TX.clone().unwrap() };
    if let Err(e) = img_mngr_tx.blocking_send(msg) {
        panic!("DRAW_TX error: {}", e);
    }
}

pub fn recv() -> Option<Images> {
    let waker = task::noop_waker();
    let mut cx = Context::from_waker(&waker);

    let img_rx = unsafe { IMG_RX.as_mut().unwrap() };
    let mut opt_images: Option<Images> = None;
    while let Poll::Ready(Some(x)) = img_rx.poll_recv(&mut cx) {
        opt_images = Some(x);
    }

    opt_images
}

fn crop_caches(
    images: &GuardImgCaches,
    id_ch_tuples: &IdChArr,
    start_sec: f64,
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
            let i_w = (start_sec * option.px_per_sec) as isize;
            let (i_w_eff, width_eff) = match calc_effective_slice(i_w, width as usize, total_width)
            {
                Some((i, w)) => (i as isize, w as isize),
                None => {
                    let zeros = vec![0u8; width as usize * option.height as usize * 4];
                    return Some((*tup, (zeros, (0, 0))));
                }
            };
            let img_slice = image.slice_axis(Axis(1), Slice::from(i_w_eff..i_w_eff + width_eff));

            let pad_left = (-i_w.min(0)) as usize;
            let pad_right = width as usize - width_eff as usize - pad_left;
            if pad_left + pad_right == 0 {
                Some((*tup, (img_slice.to_owned().into_raw_vec(), (0, width))))
            } else {
                let arr = img_slice.pad((pad_left, pad_right), Axis(1), PadMode::Constant(0));
                Some((
                    *tup,
                    (arr.into_raw_vec(), (pad_left as u32, width_eff as u32)),
                ))
            }
        })
        .collect();
    eff_l_w_map.extend(vec.iter().map(|(k, (_, eff_l_w))| (*k, *eff_l_w)));
    imgs.extend(vec.into_iter().map(|(k, (img, _))| (k, img)));
    // println!("crop: {:?}", start.elapsed());
    (imgs, eff_l_w_map)
}

fn categorize_id_ch(
    id_ch_tuples: IdChVec,
    total_widths: &IdChMap<u32>,
    spec_caches: &GuardImgCaches,
    wav_caches: &GuardImgCaches,
    blend: f64,
) -> (CategorizedIdChVec, CategorizedIdChVec, IdChVec) {
    let categorize = |images: &GuardImgCaches| {
        let mut result = CategorizedIdChVec::default();
        for &tup in &id_ch_tuples {
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
        cat_by_spec.need_parts.is_empty()
            || cat_by_wav.need_parts.is_empty()
            || cat_by_spec.need_parts.len() == cat_by_wav.need_parts.len()
    );
    (cat_by_spec, cat_by_wav, need_wav_parts_only)
}

#[inline]
fn blend_imgs(
    spec_imgs: Images,
    wav_imgs: Images,
    eff_l_w_map: IdChMap<(u32, u32)>,
    width: u32,
    height: u32,
    blend: f64,
) -> Images {
    if abs_diff_eq!(blend, 1.) {
        return spec_imgs;
    }
    if abs_diff_eq!(blend, 0.) {
        return wav_imgs;
    }
    spec_imgs
        .par_iter()
        .filter_map(|(k, spec_img)| {
            let wav_img = wav_imgs.get(k)?;
            let eff_l_w = eff_l_w_map.get(k).cloned();
            let img = display::blend(spec_img, wav_img, width, height, blend, eff_l_w);
            Some((*k, img))
        })
        .collect()
}

async fn draw_imgs(
    id_ch_tuples: IdChVec,
    params: Arc<RwLock<DrawParams>>,
    spec_caches: ArcImgCaches,
    wav_caches: ArcImgCaches,
    img_tx: Sender<Images>,
) {
    let params_backup = params.read().clone();
    let DrawParams {
        start_sec,
        width,
        option,
        opt_for_wav,
        blend,
    } = params_backup;
    let (total_widths, cat_by_spec, cat_by_wav, blended_imgs) = {
        let tm = TM.read();
        let id_ch_tuples: IdChVec = id_ch_tuples.into_iter().filter(|x| tm.exists(x)).collect();
        let mut total_widths = IdChMap::<u32>::with_capacity(id_ch_tuples.len());
        total_widths.extend(id_ch_tuples.iter().map(|&(id, ch)| {
            let width = tm.tracks[id]
                .as_ref()
                .unwrap()
                .calc_width(option.px_per_sec);
            ((id, ch), width)
        }));

        let spec_caches_lock = spec_caches.lock();
        let wav_caches_lock = wav_caches.lock();
        let (cat_by_spec, cat_by_wav, need_wav_parts_only) = categorize_id_ch(
            id_ch_tuples,
            &total_widths,
            &spec_caches_lock,
            &wav_caches_lock,
            blend,
        );

        // crop image cache
        let (spec_imgs, eff_l_w_map) = if !cat_by_spec.use_caches.is_empty() {
            crop_caches(
                &spec_caches_lock,
                &cat_by_spec.use_caches,
                start_sec,
                width,
                &option,
            )
        } else {
            (Images::new(), IdChMap::new())
        };
        let mut wav_imgs = if !cat_by_wav.use_caches.is_empty() {
            crop_caches(
                &wav_caches_lock,
                &cat_by_wav.use_caches,
                start_sec,
                width,
                &option,
            )
            .0
        } else {
            IdChMap::new()
        };
        if !need_wav_parts_only.is_empty() {
            wav_imgs.extend(tm.get_part_imgs(
                &need_wav_parts_only,
                start_sec,
                width,
                option,
                opt_for_wav,
                0.,
                None,
            ));
        };
        (
            total_widths,
            cat_by_spec,
            cat_by_wav,
            blend_imgs(
                spec_imgs,
                wav_imgs,
                eff_l_w_map,
                width,
                option.height,
                blend,
            ),
        )
    };
    if !blended_imgs.is_empty() {
        // println!("send cached images");
        img_tx.send(blended_imgs).await.unwrap();
    }
    if *params.read() != params_backup {
        return;
    }

    // draw part
    let blended_imgs = {
        let tm = TM.read();
        if !cat_by_spec.need_parts.is_empty() {
            let fast_resize_vec = Some(
                cat_by_spec
                    .need_parts
                    .iter()
                    .map(|tup| *total_widths.get(tup).unwrap() <= MAX_IMG_CACHE_WIDTH)
                    .collect(),
            );
            // let fast_resize_vec = Some(vec![true; cat_by_spec.need_parts.len()]);
            tm.get_part_imgs(
                &cat_by_spec.need_parts,
                start_sec,
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
    if *params.read() != params_backup {
        return;
    }

    let blended_imgs = {
        let tm = TM.read();
        let mut spec_caches_lock = spec_caches.lock();
        let mut wav_caches_lock = wav_caches.lock();
        let (spec_imgs, eff_l_w_map) = if !cat_by_spec.need_new_caches.is_empty() {
            spec_caches_lock.extend(tm.get_entire_imgs(
                &cat_by_spec.need_new_caches,
                option,
                ImageKind::Spec,
            ));
            crop_caches(
                &spec_caches_lock,
                &cat_by_spec.need_new_caches,
                start_sec,
                width,
                &option,
            )
        } else {
            (Images::new(), IdChMap::new())
        };
        let wav_imgs = if !cat_by_wav.need_new_caches.is_empty() {
            wav_caches_lock.extend(tm.get_entire_imgs(
                &cat_by_wav.need_new_caches,
                option,
                ImageKind::Wav(opt_for_wav),
            ));
            crop_caches(
                &wav_caches_lock,
                &cat_by_wav.need_new_caches,
                start_sec,
                width,
                &option,
            )
            .0
        } else {
            IdChMap::new()
        };
        blend_imgs(
            spec_imgs,
            wav_imgs,
            eff_l_w_map,
            width,
            option.height,
            blend,
        )
    };
    if !blended_imgs.is_empty() {
        // println!("send new cached images");
        img_tx.send(blended_imgs).await.unwrap();
    }
}

async fn main_loop(mut msg_rx: Receiver<ImgMsg>, img_tx: Sender<Images>) {
    let spec_caches = Arc::new(Mutex::new(IdChMap::new()));
    let wav_caches = Arc::new(Mutex::new(IdChMap::new()));
    let prev_params = Arc::new(RwLock::new(DrawParams::default()));
    let mut task_handle: Option<JoinHandle<()>> = None;
    while let Some(msg) = msg_rx.recv().await {
        match msg {
            ImgMsg::Draw((id_ch_tuples, draw_params)) => {
                {
                    let mut prev_params_write = prev_params.write();
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
                        spec_caches.lock().clear();
                        wav_caches.lock().clear();
                    } else if draw_params.opt_for_wav != prev_params_write.opt_for_wav {
                        wav_caches.lock().clear();
                    }
                    *prev_params_write = draw_params;
                }
                task_handle = Some(RUNTIME.spawn(draw_imgs(
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
                let mut spec_caches = spec_caches.lock();
                let mut wav_caches = wav_caches.lock();
                for tup in &id_ch_tuples {
                    spec_caches.remove(tup);
                    wav_caches.remove(tup);
                }
            }
        }
    }
}

pub fn spawn_runtime() {
    initialize(&RUNTIME);

    let (msg_tx, msg_rx) = mpsc::channel::<ImgMsg>(60);
    let (img_tx, img_rx) = mpsc::channel(60);
    unsafe {
        MSG_TX = Some(msg_tx);
        IMG_RX = Some(img_rx);
    }
    RUNTIME.spawn(main_loop(msg_rx, img_tx));
}
