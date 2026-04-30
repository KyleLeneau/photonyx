pub mod calibrate_observation;
pub mod error;
pub mod master_bias;
pub mod master_dark;
pub mod master_flat;

pub trait PipelineReporter {
    fn step_started(&self, message: &str) -> usize;
    fn step_ended(&self, id: usize, message: &str, success: bool);
}
