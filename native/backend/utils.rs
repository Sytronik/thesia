use std::collections::HashMap;
use std::mem::MaybeUninit;
use std::path::{self, PathBuf};

use ndarray::OwnedRepr;
use ndarray::{prelude::*, Data, RemoveAxis, Slice, Zip};

#[macro_export(local_inner_macros)]
macro_rules! iter_filtered {
    ($vec: expr) => {
        $vec.iter().filter_map(|x| x.as_ref())
    };
}

#[macro_export(local_inner_macros)]
macro_rules! iter_mut_filtered {
    ($vec: expr) => {
        $vec.iter_mut().filter_map(|x| x.as_mut())
    };
}

#[macro_export(local_inner_macros)]
macro_rules! indexed_iter_filtered {
    ($vec: expr) => {
        $vec.iter()
            .enumerate()
            .filter_map(|(i, x)| x.as_ref().map(|x| (i, x)))
    };
}

pub fn unique_filenames(paths: HashMap<usize, PathBuf>) -> HashMap<usize, String> {
    let mut groups = HashMap::<String, HashMap<usize, PathBuf>>::new();
    let mut result = HashMap::<usize, String>::new();
    for (id, path) in paths {
        match path.file_name() {
            Some(x) => {
                let name = x.to_string_lossy().into_owned();
                let parent = path.parent().unwrap().to_path_buf();
                match groups.get_mut(&name) {
                    Some(value) => {
                        value.insert(id, parent);
                    }
                    None => {
                        let mut hm = HashMap::<usize, PathBuf>::with_capacity(1);
                        hm.insert(id, parent);
                        groups.insert(name, hm);
                    }
                };
            }
            None => {
                result.insert(id, path.to_string_lossy().to_string());
            }
        };
    }
    for (name, hm) in groups {
        if hm.len() == 1 {
            let (id, _) = hm.into_iter().next().unwrap();
            result.insert(id, name);
        } else {
            let mut parents = unique_filenames(hm);
            for parent in parents.values_mut() {
                *parent = format!("{}{}{}", parent, path::MAIN_SEPARATOR, name);
            }
            result.extend(parents);
        }
    }
    result
}

#[inline]
pub fn calc_proper_n_fft(win_length: usize) -> usize {
    2usize.pow((win_length as f32).log2().ceil() as u32)
}

pub enum PadMode<T> {
    Constant(T),
    Reflect,
}

pub trait Pad<A> {
    type WithOwnedA;
    fn pad(&self, n_pads: (usize, usize), axis: Axis, mode: PadMode<A>) -> Self::WithOwnedA;
}

impl<A, S, D> Pad<A> for ArrayBase<S, D>
where
    A: Copy,
    S: Data<Elem = A>,
    D: Dimension + RemoveAxis,
{
    type WithOwnedA = ArrayBase<OwnedRepr<A>, D>;
    fn pad(
        &self,
        (n_pad_left, n_pad_right): (usize, usize),
        axis: Axis,
        mode: PadMode<A>,
    ) -> Self::WithOwnedA {
        let mut shape = self.raw_dim();
        shape[axis.index()] += n_pad_left + n_pad_right;
        let mut result = Self::WithOwnedA::uninit(shape);

        let s_result_main = if n_pad_right > 0 {
            Slice::from(n_pad_left as isize..-(n_pad_right as isize))
        } else {
            Slice::from(n_pad_left as isize..)
        };
        Zip::from(self).map_assign_into(result.slice_axis_mut(axis, s_result_main), |x| *x);
        match mode {
            PadMode::Constant(constant) => {
                result
                    .slice_axis_mut(axis, Slice::from(..n_pad_left))
                    .mapv_inplace(|_| MaybeUninit::new(constant));
                if n_pad_right > 0 {
                    result
                        .slice_axis_mut(axis, Slice::from(-(n_pad_right as isize)..))
                        .mapv_inplace(|_| MaybeUninit::new(constant));
                }
            }
            PadMode::Reflect => {
                let pad_left = self
                    .axis_iter(axis)
                    .skip(1)
                    .chain(self.axis_iter(axis).rev().skip(1))
                    .cycle()
                    .take(n_pad_left);
                result
                    .axis_iter_mut(axis)
                    .take(n_pad_left)
                    .rev()
                    .zip(pad_left)
                    .for_each(|(y, x)| Zip::from(x).map_assign_into(y, |x| *x));

                if n_pad_right > 0 {
                    let pad_right = self
                        .axis_iter(axis)
                        .rev()
                        .skip(1)
                        .chain(self.axis_iter(axis).skip(1))
                        .cycle()
                        .take(n_pad_right);
                    result
                        .axis_iter_mut(axis)
                        .rev()
                        .take(n_pad_right)
                        .rev()
                        .zip(pad_right)
                        .for_each(|(y, x)| Zip::from(x).map_assign_into(y, |x| *x));
                }
            }
        }
        unsafe { result.assume_init() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use ndarray::arr2;

    #[test]
    fn pad_works() {
        assert_eq!(
            arr2(&[[1, 2, 3]]).pad((1, 2), Axis(0), PadMode::Constant(10)),
            arr2(&[[10, 10, 10], [1, 2, 3], [10, 10, 10], [10, 10, 10]])
        );
        assert_eq!(
            arr2(&[[1, 2, 3]]).pad((3, 4), Axis(1), PadMode::Reflect),
            arr2(&[[2, 3, 2, 1, 2, 3, 2, 1, 2, 3]])
        );
    }
}
