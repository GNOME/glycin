use jxl::api::{JxlParallelRunner, JxlParallelRunnerFun};
use rayon::prelude::*;

pub struct RayonParallelRunner;

impl JxlParallelRunner for RayonParallelRunner {
    fn run(&mut self, num: usize, fun: &JxlParallelRunnerFun) -> jxl::error::Result<()> {
        (0..num).into_par_iter().try_for_each(fun)
    }

    fn num_threads(&self) -> usize {
        rayon::current_num_threads()
    }
}
