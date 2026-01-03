use std::collections::HashMap;
use std::mem::MaybeUninit;
use std::path::{self, Path, PathBuf};

use identity_hash::IntMap;
use itertools::Itertools;
use ndarray::prelude::*;
use ndarray::{OwnedRepr, RemoveAxis};
use num_traits::Num;

pub fn unique_filenames(paths: IntMap<usize, PathBuf>) -> IntMap<usize, String> {
    let mut groups = HashMap::<String, IntMap<usize, PathBuf>>::new();
    let mut result = IntMap::<usize, String>::default();
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
                        let mut hm = IntMap::<usize, PathBuf>::with_capacity_and_hasher(
                            1,
                            Default::default(),
                        );
                        hm.insert(id, parent);
                        groups.insert(name, hm);
                    }
                };
            }
            None => {
                result.insert(id, path.to_string_lossy().into());
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
                if Path::new(parent).parent().is_none() {
                    *parent = format!("{}{}", parent, name);
                    *parent = dunce::canonicalize(Path::new(parent))
                        .unwrap()
                        .to_string_lossy()
                        .into();
                } else {
                    *parent = format!("{}{}{}", parent, path::MAIN_SEPARATOR, name);
                }
            }
            result.extend(parents);
        }
    }
    result
}

pub enum PadMode<A> {
    Constant(A),
    Reflect,
}

impl<A: Num> Default for PadMode<A> {
    fn default() -> Self {
        PadMode::Constant(A::zero())
    }
}

pub trait Pad<A> {
    type WithOwnedA;
    fn pad(&self, n_pads: (usize, usize), axis: Axis, mode: PadMode<A>) -> Self::WithOwnedA;
}

impl<A, D> Pad<A> for ArrayRef<A, D>
where
    A: Copy,
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
            (n_pad_left as isize..-(n_pad_right as isize)).into()
        } else {
            (n_pad_left as isize..).into()
        };
        self.assign_to(result.slice_axis_mut(axis, s_result_main));
        match mode {
            PadMode::Constant(constant) => {
                let constant = MaybeUninit::new(constant);
                result
                    .slice_axis_mut(axis, (..n_pad_left).into())
                    .mapv_inplace(|_| constant);
                if n_pad_right > 0 {
                    result
                        .slice_axis_mut(axis, (-(n_pad_right as isize)..).into())
                        .mapv_inplace(|_| constant);
                }
            }
            PadMode::Reflect => {
                let pad_left = itertools::chain(
                    self.axis_iter(axis).skip(1),
                    self.axis_iter(axis).rev().skip(1),
                )
                .cycle()
                .take(n_pad_left);
                result
                    .axis_iter_mut(axis)
                    .take(n_pad_left)
                    .rev()
                    .zip(pad_left)
                    .for_each(|(tgt, src)| src.assign_to(tgt));

                if n_pad_right > 0 {
                    let pad_right = itertools::chain(
                        self.axis_iter(axis).rev().skip(1),
                        self.axis_iter(axis).skip(1),
                    )
                    .cycle()
                    .take(n_pad_right);
                    result
                        .axis_iter_mut(axis)
                        .tail(n_pad_right)
                        .zip(pad_right)
                        .for_each(|(tgt, src)| src.assign_to(tgt));
                }
            }
        }
        unsafe { result.assume_init() }
    }
}

pub trait Planes<A> {
    fn planes(&self) -> Vec<&[A]>;
}

impl<A, D> Planes<A> for ArrayRef<A, D>
where
    D: Dimension + RemoveAxis,
{
    fn planes(&self) -> Vec<&[A]> {
        self.axis_iter(Axis(0))
            .map(|x| x.to_slice().unwrap())
            .collect()
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
