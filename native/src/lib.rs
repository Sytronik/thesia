use std::sync::RwLock;
// use std::time::{Duration, Instant};

use lazy_static::{initialize, lazy_static};
use neon::prelude::*;

use thesia_backend::*;

lazy_static! {
    static ref TM: RwLock<TrackManager> = RwLock::new(TrackManager::new());
}

macro_rules! get_track {
    ($locked:expr, $cx:expr, $i_arg_id:expr) => {
        match $locked
            .tracks
            .get(&($cx.argument::<JsNumber>($i_arg_id)?.value() as usize))
        {
            Some(t) => t,
            None => return $cx.throw_error("Wrong track id!"),
        }
    };
}

macro_rules! get_num_arg {
    ($cx:expr, $i_arg:expr $(, $type:ident)?) => {
        $cx.argument::<JsNumber>($i_arg)?.value() $(as $type)?
    };
}

macro_rules! get_arr_arg {
    ($cx:expr, $i_arg:expr, JsNumber $(, $type:ident)?) => {
        $cx.argument::<JsArray>($i_arg)?
            .to_vec(&mut $cx)?
            .into_iter()
            .map(|jsv| {
                jsv.downcast::<JsNumber>()
                    .unwrap_or($cx.number(0.))
                    .value() $(as $type)?
            })
            .collect()
    };
    ($cx:expr, $i_arg:expr, JsNumber, $default:expr $(, $type:ident)?) => {
        $cx.argument::<JsArray>($i_arg)?
            .to_vec(&mut $cx)?
            .into_iter()
            .map(|jsv| {
                jsv.downcast::<JsNumber>()
                    .unwrap_or($cx.number($default))
                    .value() $(as $type)?
            })
            .collect()
    };
    ($cx:expr, $i_arg:expr, $js_type:ident, $default:expr) => {
        $cx.argument::<JsArray>($i_arg)?
            .to_vec(&mut $cx)?
            .into_iter()
            .map(|jsv| {
                jsv.downcast::<$js_type>()
                    .unwrap_or($js_type::new(&mut $cx, $default))
                    .value()
            })
            .collect()
    };
}

fn add_tracks(mut cx: FunctionContext) -> JsResult<JsBoolean> {
    let new_track_ids: Vec<usize> = get_arr_arg!(cx, 0, JsNumber, usize);
    let new_paths: Vec<String> = get_arr_arg!(cx, 1, JsString, "");
    match TM.write().unwrap().add_tracks(new_track_ids, new_paths) {
        Ok(b) => Ok(cx.boolean(b)),
        Err(_) => cx.throw_error("Unsupported file type!"),
    }
}

fn remove_track(mut cx: FunctionContext) -> JsResult<JsBoolean> {
    let track_id = get_num_arg!(cx, 0, usize);

    Ok(cx.boolean(TM.write().unwrap().remove_track(track_id)))
}

fn get_spec_wav_image(mut cx: FunctionContext) -> JsResult<JsArrayBuffer> {
    let track_id = get_num_arg!(cx, 0, usize);
    let width = get_num_arg!(cx, 1, u32);
    let height = get_num_arg!(cx, 2, u32);
    let blend = get_num_arg!(cx, 3);
    let mut buf = JsArrayBuffer::new(&mut cx, width * height * 4u32)?;
    cx.borrow_mut(&mut buf, |slice| {
        let locked = TM.read().unwrap();
        locked
            .get_spec_wav_image(slice.as_mut_slice(), track_id, width, height, blend)
            .unwrap();
    });
    Ok(buf)
}

fn get_max_db(mut cx: FunctionContext) -> JsResult<JsNumber> {
    Ok(cx.number(TM.read().unwrap().max_db))
}

fn get_min_db(mut cx: FunctionContext) -> JsResult<JsNumber> {
    Ok(cx.number(TM.read().unwrap().min_db))
}

fn get_sec(mut cx: FunctionContext) -> JsResult<JsNumber> {
    let locked = TM.read().unwrap();
    let track = get_track!(locked, cx, 0);
    let sec = track.wav.len() as f32 / track.sr as f32;
    Ok(cx.number(sec))
}

fn get_sr(mut cx: FunctionContext) -> JsResult<JsNumber> {
    let locked = TM.read().unwrap();
    let track = get_track!(locked, cx, 0);
    Ok(cx.number(track.sr))
}

fn get_path(mut cx: FunctionContext) -> JsResult<JsString> {
    let locked = TM.read().unwrap();
    let track = get_track!(locked, cx, 0);
    Ok(cx.string(track.get_path()))
}

fn get_filename(mut cx: FunctionContext) -> JsResult<JsString> {
    let locked = TM.read().unwrap();
    let track = get_track!(locked, cx, 0);
    Ok(cx.string(track.get_filename()))
}

fn get_colormap(mut cx: FunctionContext) -> JsResult<JsArrayBuffer> {
    let (vec, len) = get_colormap_iter_size();
    let mut buf = JsArrayBuffer::new(&mut cx, len as u32)?;
    cx.borrow_mut(&mut buf, |slice| {
        slice
            .as_mut_slice()
            .iter_mut()
            .zip(vec)
            .for_each(|(x, &y)| {
                *x = y;
            })
    });
    Ok(buf)
}

register_module!(mut m, {
    initialize(&TM);
    m.export_function("addTracks", add_tracks)?;
    m.export_function("removeTrack", remove_track)?;
    m.export_function("getSpecWavImage", get_spec_wav_image)?;
    m.export_function("getMaxdB", get_max_db)?;
    m.export_function("getMindB", get_min_db)?;
    m.export_function("getSec", get_sec)?;
    m.export_function("getSr", get_sr)?;
    m.export_function("getPath", get_path)?;
    m.export_function("getFileName", get_filename)?;
    m.export_function("getColormap", get_colormap)?;
    Ok(())
});
