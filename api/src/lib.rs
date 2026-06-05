pub mod credential;
pub mod gpujob;

pub use credential::{HpcCredential, HpcCredentialSpec};
pub use gpujob::{
    CredentialRef, GpuJob, GpuJobPhase, GpuJobRunSpec, GpuJobSourceFile, GpuJobSourceSpec,
    GpuJobSpec, GpuJobStatus, HpcSpec, SlurmSpec,
};

pub const GROUP: &str = "gpu.minik8s.io";
pub const VERSION: &str = "v1alpha1";
