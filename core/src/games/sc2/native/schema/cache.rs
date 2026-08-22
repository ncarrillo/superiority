#![allow(dead_code, unused_imports, clippy::all)]

use bsn_derive::FromBsn;
use superiority_core::bsn::{BsnBitArray, Bytes, FourCc};

#[derive(Clone, Debug, FromBsn)]
pub struct CacheStreamItem {
    #[bsn(name = "m_publicationTime")]
    pub publication_time: i32,
    #[bsn(name = "m_contentHandle")]
    pub content_handle: Bytes,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientCacheGetStreamItems {}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientCacheGetStreamItemsResponse {
    #[bsn(name = "GetStreamItems")]
    pub get_stream_items: super::cache::ClientCacheGetStreamItems,
    #[bsn(name = "m_items")]
    pub items: Vec<super::cache::CacheStreamItem>,
    #[bsn(name = "m_offset")]
    pub offset: u16,
    #[bsn(name = "m_totalNumItems")]
    pub total_num_items: u16,
    #[bsn(name = "m_token")]
    pub token: u32,
}
