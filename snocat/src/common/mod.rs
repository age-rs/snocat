// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license OR Apache 2.0
use crate::util::{
  self,
  framed::{read_frame_vec, write_frame},
  validators::parse_socketaddr,
};
use anyhow::{Context as AnyhowContext, Error as AnyErr, Result};
use futures::future;
use futures::future::*;
use quinn::{
  Certificate, CertificateChain, ClientConfig, ClientConfigBuilder, Endpoint, Incoming, PrivateKey,
  ServerConfig, ServerConfigBuilder, TransportConfig,
};
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::{
  path::{Path, PathBuf},
  pin::Pin,
  sync::Arc,
  task::{Context, Poll},
};

pub mod authentication;
pub mod daemon;
pub mod protocol;
pub mod tunnel_source;

#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct MetaStreamHeader {}

impl MetaStreamHeader {
  pub fn new() -> MetaStreamHeader {
    MetaStreamHeader {}
  }
}
