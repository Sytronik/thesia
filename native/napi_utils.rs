use std::convert::TryInto;

use napi::{CallContext, Env, JsNumber, JsObject, JsString, Result as JsResult};

use crate::backend::{DrawOption, DrawOptionForWav, IdChVec};

pub trait TryIntoUsize: TryInto<u32> {
    fn try_into_usize(self) -> Result<usize, Self::Error>;
}

impl TryIntoUsize for JsNumber {
    fn try_into_usize(self) -> Result<usize, Self::Error> {
        Ok(TryInto::<u32>::try_into(self)? as usize)
    }
}

pub trait TryIntoF32: TryInto<f64> {
    fn try_into_f32(self) -> Result<f32, Self::Error>;
}

impl TryIntoF32 for JsNumber {
    fn try_into_f32(self) -> Result<f32, Self::Error> {
        Ok(TryInto::<f64>::try_into(self)? as f32)
    }
}

#[macro_export(local_inner_macros)]
macro_rules! get_track {
    ($ctx: expr, $i_id_arg: expr, $tm_read: expr) => {
        $tm_read
            .tracks
            .get(&$ctx.get::<JsNumber>(0)?.try_into_usize()?)
            .unwrap()
    };
}

#[macro_export(local_inner_macros)]
macro_rules! this_is {
    (let $tm_name:ident = $type:ident in $ctx:expr) => {
        let $tm_name: &mut $type = $ctx.env.unwrap(&mut $ctx.this_unchecked())?;
    };
}

pub fn id_ch_tuples_from(ctx: &CallContext, i_arg: usize) -> JsResult<IdChVec> {
    let arr = ctx.get::<JsObject>(i_arg)?;
    let len = arr.get_array_length()?;
    let mut vec = IdChVec::with_capacity(len as usize);
    for i in 0..len {
        let js_str = arr.get_element::<JsString>(i)?.into_utf8()?;
        let s = js_str.as_str()?;
        let mut iter = s.split("_").map(|x| x.parse::<usize>());
        match (iter.next(), iter.next()) {
            (Some(Ok(id)), Some(Ok(ch))) => {
                vec.push((id, ch));
            }
            _ => {
                return Err(ctx
                    .env
                    .throw_error("The array element should be \"int_int\".", None)
                    .err()
                    .unwrap())
            }
        };
    }
    Ok(vec)
}

pub fn vec_usize_from(ctx: &CallContext, i_arg: usize) -> JsResult<Vec<usize>> {
    let arr = ctx.get::<JsObject>(i_arg)?;
    let len = arr.get_array_length()?;
    let mut vec = Vec::<usize>::with_capacity(len as usize);
    for i in 0..len {
        if let Ok(x) = arr.get_element::<JsNumber>(i)?.try_into_usize() {
            vec.push(x);
        }
    }
    Ok(vec)
}

pub fn vec_str_from(ctx: &CallContext, i_arg: usize) -> JsResult<Vec<String>> {
    let arr = ctx.get::<JsObject>(i_arg)?;
    let len = arr.get_array_length()?;
    let mut vec = Vec::<String>::with_capacity(len as usize);
    for i in 0..len {
        let jsstr = arr.get_element::<JsString>(i)?.into_utf8()?;
        vec.push(jsstr.into_owned()?);
    }
    Ok(vec)
}

pub fn convert_id_ch_arr_to_jsarr<'a>(env: &Env, arr: &[(usize, usize)]) -> JsResult<JsObject> {
    let mut obj = env.create_array_with_length(arr.len())?;
    for (i, &(id, ch)) in arr.iter().enumerate() {
        obj.set_element(
            i as u32,
            env.create_string_from_std(format!("{}_{}", id, ch))?,
        )?;
    }
    Ok(obj)
}

pub fn convert_usize_arr_to_jsarr<'a>(env: &Env, arr: &[usize]) -> JsResult<JsObject> {
    let mut obj = env.create_array_with_length(arr.len())?;
    for (i, &x) in arr.iter().enumerate() {
        obj.set_element(i as u32, env.create_double(x as f64)?)?;
    }
    Ok(obj)
}

pub fn convert_vec_tup_f64_to_jsarr<'a>(env: &Env, vec: Vec<(f64, f64)>) -> JsResult<JsObject> {
    let mut obj = env.create_array_with_length(vec.len())?;
    for (i, x) in vec.into_iter().enumerate() {
        let mut tup_arr = env.create_array_with_length(2)?;
        tup_arr.set_element(0, env.create_double(x.0)?)?;
        tup_arr.set_element(1, env.create_double(x.1)?)?;
        obj.set_element(i as u32, tup_arr)?;
    }
    Ok(obj)
}

#[inline]
pub fn draw_option_from_js_obj(js_obj: JsObject) -> JsResult<DrawOption> {
    Ok(DrawOption {
        px_per_sec: js_obj
            .get_named_property::<JsNumber>("px_per_sec")?
            .try_into()?,
        height: js_obj
            .get_named_property::<JsNumber>("height")?
            .try_into()?,
    })
}

#[inline]
pub fn draw_opt_for_wav_from_js_obj(js_obj: JsObject) -> JsResult<DrawOptionForWav> {
    Ok(DrawOptionForWav {
        amp_range: (
            js_obj
                .get_named_property::<JsNumber>("min_amp")?
                .try_into_f32()?,
            js_obj
                .get_named_property::<JsNumber>("max_amp")?
                .try_into_f32()?,
        ),
    })
}
