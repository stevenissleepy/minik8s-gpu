use anyhow::{Result, anyhow};
use apimachinery::{ObjectMeta, Resource, TypeMeta, Validatable};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

use crate::{GROUP, VERSION};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct HpcCredential {
    #[serde(flatten)]
    pub types: TypeMeta,
    pub metadata: ObjectMeta,
    pub spec: HpcCredentialSpec,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HpcCredentialSpec {
    pub username: String,
    #[serde(rename = "privateKey")]
    pub private_key: String,
    #[serde(rename = "knownHosts", default)]
    pub known_hosts: String,
}

impl Resource for HpcCredential {
    type DynamicType = ();

    fn kind(_: &()) -> Cow<'_, str> {
        "HPCCredential".into()
    }

    fn plural(_: &()) -> Cow<'_, str> {
        "hpccredentials".into()
    }

    fn group(_: &()) -> Cow<'_, str> {
        GROUP.into()
    }

    fn version(_: &()) -> Cow<'_, str> {
        VERSION.into()
    }

    fn metadata(&self) -> &ObjectMeta {
        &self.metadata
    }

    fn metadata_mut(&mut self) -> &mut ObjectMeta {
        &mut self.metadata
    }
}

impl Validatable for HpcCredential {
    fn validate_spec(&self) -> Result<()> {
        if self.spec.username.trim().is_empty() {
            return Err(anyhow!("hpccredential spec.username is required"));
        }
        if self.spec.private_key.trim().is_empty() {
            return Err(anyhow!("hpccredential spec.privateKey is required"));
        }
        Ok(())
    }
}
