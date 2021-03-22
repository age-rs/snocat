// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license OR Apache 2.0
//! Sources both listen- and connection-based tunnels

use futures::stream::{BoxStream, Stream, StreamExt};
use std::{net::SocketAddr, pin::Pin, task::Poll};

use super::protocol::tunnel::{from_quinn_endpoint, BoxedTunnelPair, TunnelSide};
use std::fmt::Debug;
use std::hash::Hash;
use std::sync::{Arc, TryLockError};
use std::task::Context;
use tokio_stream::StreamMap;

pub struct QuinnListenEndpoint<Session: quinn::crypto::Session> {
  bind_addr: SocketAddr,
  quinn_config: quinn::generic::ServerConfig<Session>,
  endpoint: quinn::generic::Endpoint<Session>,
  incoming: BoxStream<'static, quinn::generic::NewConnection<Session>>,
}

impl<Session: quinn::crypto::Session + 'static> QuinnListenEndpoint<Session> {
  pub fn bind(
    bind_addr: SocketAddr,
    quinn_config: quinn::generic::ServerConfig<Session>,
  ) -> Result<Self, quinn::EndpointError> {
    let mut builder = quinn::generic::Endpoint::builder();
    builder.listen(quinn_config.clone());
    let (endpoint, incoming) = builder.bind(&bind_addr)?;
    let incoming = incoming
      .filter_map(|connecting| async move { connecting.await.ok() })
      .boxed();
    Ok(Self {
      bind_addr,
      quinn_config,
      endpoint,
      incoming,
    })
  }
}

impl<Session> Stream for QuinnListenEndpoint<Session>
where
  Session: quinn::crypto::Session + 'static,
  Self: Unpin,
{
  type Item = BoxedTunnelPair<'static>;

  fn poll_next(
    mut self: std::pin::Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
  ) -> std::task::Poll<Option<Self::Item>> {
    let res = futures::ready!(Stream::poll_next(Pin::new(&mut self.incoming), cx));
    match res {
      None => Poll::Ready(None),
      Some(new_connection) => {
        let (tunnel, incoming) = from_quinn_endpoint(new_connection, TunnelSide::Listen);
        Poll::Ready(Some((Box::new(tunnel), incoming)))
      }
    }
  }
}

/// Structure used to hold boxed streams which have an ID associated with them
///
/// Primarily for use alongside StreamMap or DynamicStreamSet.
pub struct NamedBoxedStream<Id, StreamItem> {
  id: Id,
  stream: BoxStream<'static, StreamItem>,
}

impl<Id, StreamItem> NamedBoxedStream<Id, StreamItem> {
  pub fn new<TStream>(id: Id, stream: TStream) -> Self
  where
    TStream: Stream<Item = StreamItem> + Send + Sync + 'static,
  {
    Self::new_pre_boxed(id, stream.boxed())
  }

  pub fn new_pre_boxed(id: Id, stream: BoxStream<'static, StreamItem>) -> Self {
    Self { id, stream }
  }
}

impl<Id, StreamItem> Stream for NamedBoxedStream<Id, StreamItem>
where
  Id: Unpin,
{
  type Item = StreamItem;

  fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
    Stream::poll_next(Pin::new(&mut self.stream), cx)
  }

  fn size_hint(&self) -> (usize, Option<usize>) {
    self.stream.size_hint()
  }
}

impl<Id, StreamItem> std::fmt::Debug for NamedBoxedStream<Id, StreamItem>
where
  Id: Debug,
{
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct(stringify!(DynamicConnection))
      .field("id", &self.id)
      .finish_non_exhaustive()
  }
}

/// A set of connections / endpoints that can be updated dynamically, to allow runtime addition and
/// removal of connections / "Tunnel sources" to those being handled by a tunnel server.
pub type DynamicConnectionSet<Id> = DynamicStreamSet<Id, BoxedTunnelPair<'static>>;

/// A strict wrapper for StreamMap that requires boxing of the items and handles locking for updates
/// Can be used to merges outputs from a runtime-editable set of endpoint ports
pub struct DynamicStreamSet<Id, TStream> {
  // RwLock is semantically better here but poll_next is a mutation, so we'd have to
  // trick it by using something like a refcell internally, losing most of the benefits.
  //
  // As this is to facilitate async, this is likely to be a near-uncontested mutex, but
  // we use a std::sync::Mutex instead of an async one as we only expect to lock briefly.
  streams: Arc<std::sync::Mutex<StreamMap<Id, NamedBoxedStream<Id, TStream>>>>,
}

pub struct DynamicStreamSetHandle<Id, TStream> {
  // RwLock is semantically better here but poll_next is a mutation, so we'd have to
  // trick it by using something like a refcell internally, losing most of the benefits.
  //
  // As this is to facilitate async, this is likely to be a near-uncontested mutex, but
  // we use a std::sync::Mutex instead of an async one as we only expect to lock briefly.
  streams: Arc<std::sync::Mutex<StreamMap<Id, NamedBoxedStream<Id, TStream>>>>,
}

impl<Id, StreamItem> DynamicStreamSet<Id, StreamItem> {
  pub fn new() -> Self {
    Self {
      streams: Arc::new(std::sync::Mutex::new(StreamMap::new())),
    }
  }

  pub fn attach(
    &self,
    source: NamedBoxedStream<Id, StreamItem>,
  ) -> Option<NamedBoxedStream<Id, StreamItem>>
  where
    Id: Clone + Hash + Eq,
  {
    let mut streams = self.streams.lock().expect("Mutex poisoned");
    streams.insert(source.id.clone(), source)
  }

  pub fn attach_stream(
    &self,
    id: Id,
    source: BoxStream<'static, StreamItem>,
  ) -> Option<NamedBoxedStream<Id, StreamItem>>
  where
    Id: Clone + Hash + Eq,
  {
    let endpoint = NamedBoxedStream::new_pre_boxed(id.clone(), source);
    self.attach(endpoint)
  }

  pub fn detach(&self, id: &Id) -> Option<NamedBoxedStream<Id, StreamItem>>
  where
    Id: Hash + Eq,
  {
    let mut streams = self.streams.lock().expect("Mutex poisoned");
    streams.remove(id)
  }

  pub fn handle(&self) -> DynamicStreamSetHandle<Id, StreamItem> {
    DynamicStreamSetHandle {
      streams: self.streams.clone(),
    }
  }

  pub fn into_handle(self) -> DynamicStreamSetHandle<Id, StreamItem> {
    DynamicStreamSetHandle {
      streams: self.streams,
    }
  }

  fn poll_next(
    streams: &std::sync::Mutex<StreamMap<Id, NamedBoxedStream<Id, StreamItem>>>,
    cx: &mut Context<'_>,
  ) -> Poll<Option<(Id, StreamItem)>>
  where
    Id: Clone + Unpin,
  {
    // Use try_lock to ensure that we don't deadlock in a single-threaded async scenario
    let mut streams = match streams.try_lock() {
      Ok(s) => s,
      Err(TryLockError::WouldBlock) => {
        // Queue for another wake, to retry the mutex; essentially, yield for other async
        // Note that this effectively becomes a spin-lock if the mutex is held while the
        // async runtime has nothing else to work on.
        cx.waker().wake_by_ref();
        return Poll::Pending;
      }
      Err(TryLockError::Poisoned(poison)) => Err(poison).expect("Lock poisoned"),
    };
    Stream::poll_next(Pin::new(&mut *streams), cx)
  }
}

impl<Id, StreamItem> Stream for DynamicStreamSet<Id, StreamItem>
where
  Id: Clone + Unpin,
{
  type Item = (Id, StreamItem);

  fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
    Self::poll_next(&*self.streams, cx)
  }

  // Size is hintable but slow to calculate and only useful if all sub-stream hints are precise
  // Implement this only if the maintainability cost of a membership-update driven design is lower
  // than that of the performance cost of doing so. Also consider the cost of mutex locking.
  // fn size_hint(&self) -> (usize, Option<usize>) { (0, None) }
}

impl<Id, StreamItem> Stream for DynamicStreamSetHandle<Id, StreamItem>
where
  Id: Clone + Unpin,
{
  type Item = (Id, StreamItem);

  fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
    DynamicStreamSet::poll_next(&*self.streams, cx)
  }

  // See size_hint note on [DynamicStreamSet] for why we do not implement this
  // fn size_hint(&self) -> (usize, Option<usize>) { (0, None) }
}

#[cfg(test)]
mod tests {
  use super::{DynamicStreamSet, NamedBoxedStream};
  use crate::common::protocol::tunnel::BoxedTunnelPair;
  use futures::task::Context;
  use futures::{future, stream, FutureExt, Stream, StreamExt};
  use std::collections::HashSet;
  use std::iter::FromIterator;
  use std::pin::Pin;

  #[tokio::test]
  async fn add_and_remove() {
    let set = DynamicStreamSet::<u32, char>::new();
    let a = stream::iter(vec!['a']).boxed();
    let b = stream::iter(vec!['b']).boxed();
    let c = stream::iter(vec!['c']).boxed();
    set
      .attach_stream(1u32, a)
      .expect_none("Must attach to blank");
    set
      .attach_stream(2u32, b)
      .expect_none("Must attach to non-blank with new key");
    let mut replaced_b = set
      .attach_stream(2u32, c)
      .expect("Must overwrite keys and return an old one");
    let mut detached_a = set.detach(&1u32).expect("Must detach fresh keys by ID");
    let mut detached_c = set.detach(&2u32).expect("Must detach replaced keys by ID");
    assert_eq!(detached_a.id, 1u32);
    assert_eq!(
      detached_a.stream.next().await.expect("Must have item"),
      'a',
      "Fresh-key stream identity mismatch"
    );
    assert_eq!(replaced_b.id, 2u32);
    assert_eq!(
      replaced_b.stream.next().await.expect("Must have item"),
      'b',
      "Replaced stream identity mismatch"
    );
    assert_eq!(detached_c.id, 2u32);
    assert_eq!(
      detached_c.stream.next().await.expect("Must have item"),
      'c',
      "Replacement stream identity mismatch"
    );
  }

  #[tokio::test]
  async fn poll_contents() {
    let set = DynamicStreamSet::<u32, char>::new();
    let a = stream::iter(vec!['a']).boxed();
    let b = stream::iter(vec!['b']).boxed();
    let c = stream::iter(vec!['c']).boxed();
    set
      .attach_stream(1u32, a)
      .expect_none("Must attach to blank");
    set
      .attach_stream(2u32, b)
      .expect_none("Must attach to non-blank with new key");
    set
      .attach_stream(2u32, c)
      .expect("Must replace existing keys");
    // We use a hashset because we don't specify a strict ordering, that's internal to StreamMap
    let results = set.collect::<HashSet<_>>().await;
    // Note that 'b' must not occur here because we've detached it
    assert_eq!(
      results,
      HashSet::from_iter(vec![(1, 'a'), (2, 'c')].into_iter())
    );
  }

  #[tokio::test]
  async fn end_of_stream_removal() {
    use std::sync::Arc;
    let set = Arc::new(DynamicStreamSet::<u32, i32>::new());
    let a = stream::iter(vec![1, 2, 3]).boxed();
    set
      .attach_stream(1u32, a)
      .expect_none("Must attach to blank");
    let collected = set.handle().collect::<Vec<_>>().await;
    assert_eq!(collected.as_slice(), &[(1, 1), (1, 2), (1, 3)]);
    set
      .detach(&1u32)
      .expect_none("Must have already detached if polled to empty");
  }
}
