use std::sync::Mutex;
// use std::time::{Duration, Instant};

use lazy_static::{initialize, lazy_static};
use neon::prelude::*;

use thesia_backend::*;

lazy_static! {
    static ref TM: Mutex<TrackManager> = Mutex::new(TrackManager::new());
}

fn add_tracks(mut cx: FunctionContext) -> JsResult<JsBoolean> {
    let id_list: Vec<usize> = cx
        .argument::<JsArray>(0)?
        .to_vec(&mut cx)?
        .into_iter()
        .map(|jsv| jsv.downcast::<JsNumber>().unwrap_or(cx.number(0)).value())
        .map(|x| x as usize)
        .collect();
    let path_list: Vec<String> = cx
        .argument::<JsArray>(1)?
        .to_vec(&mut cx)?
        .into_iter()
        .map(|jsv| jsv.downcast::<JsString>().unwrap_or(cx.string("")).value())
        .collect();

    match TM.lock().unwrap().add_tracks(id_list, path_list) {
        Ok(b) => Ok(cx.boolean(b)),
        Err(_) => cx.throw_error("Unsupported file type!"),
    }
}

fn js_get_colormap(mut cx: FunctionContext) -> JsResult<JsArrayBuffer> {
    let vec = get_colormap();
    let mut buf = JsArrayBuffer::new(&mut cx, vec.len() as u32)?;
    cx.borrow_mut(&mut buf, |slice| {
        slice
            .as_mut_slice()
            .iter_mut()
            .zip(vec.into_iter())
            .for_each(|(x, y)| {
                *x = y;
            })
    });
    Ok(buf)
}

fn get_spec_image(mut cx: FunctionContext) -> JsResult<JsArrayBuffer> {
    let track_id = cx.argument::<JsNumber>(0)?.value() as usize;
    let width = cx.argument::<JsNumber>(1)?.value() as u32;
    let height = cx.argument::<JsNumber>(2)?.value() as u32;
    let mut buf = JsArrayBuffer::new(&mut cx, width * height * 4u32)?;
    cx.borrow_mut(&mut buf, |slice| {
        let locked = TM.lock().unwrap();
        locked.get_spec_image(slice.as_mut_slice(), track_id, width, height);
    });
    Ok(buf)
}

register_module!(mut m, {
    initialize(&TM);
    m.export_function("addTracks", add_tracks)?;
    m.export_function("getSpecImage", get_spec_image)?;
    m.export_function("getColormap", js_get_colormap)?;
    Ok(())
});
